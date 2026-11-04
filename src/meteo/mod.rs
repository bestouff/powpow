//! Six-minute observations from Météo-France's `DPObs` API.

mod client;
#[cfg(test)]
mod tests;

use std::time::Duration;

use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::PgPool;
use tracing::{info, warn};

use crate::database;
pub use client::Client;

const POLL_INTERVAL: Duration = Duration::from_mins(6);

#[derive(Clone)]
pub struct Config {
    pub station_id: String,
    token: String,
    application_id: String,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MeteoConfig")
            .field("station_id", &self.station_id)
            .field("automatic_renewal", &!self.application_id.is_empty())
            .finish_non_exhaustive()
    }
}

impl Config {
    pub fn from_env() -> Result<Option<Self>> {
        Self::parse(|key| std::env::var(key).unwrap_or_default())
    }

    pub(crate) fn parse(value: impl Fn(&str) -> String) -> Result<Option<Self>> {
        let token = value("METEO_FRANCE_TOKEN").trim().to_owned();
        let application_id = value("METEO_FRANCE_APPLICATION_ID").trim().to_owned();
        if token.is_empty() && application_id.is_empty() {
            return Ok(None);
        }
        let station_id = value("METEO_FRANCE_STATION_ID").trim().to_owned();
        ensure!(
            station_id.len() == 8 && station_id.bytes().all(|c| c.is_ascii_digit()),
            "METEO_FRANCE_STATION_ID must contain exactly eight digits (Saint-Hilaire-du-Touvet: 38395004)"
        );
        Ok(Some(Self {
            station_id,
            token,
            application_id,
        }))
    }
}

#[derive(Debug, Deserialize)]
pub struct Observation {
    #[serde(rename = "geo_id_insee")]
    pub station_id: String,
    #[serde(rename = "validity_time")]
    pub observed_at: DateTime<Utc>,
    pub reference_time: DateTime<Utc>,
    pub insert_time: DateTime<Utc>,
    #[serde(rename = "lat")]
    pub latitude: f64,
    #[serde(rename = "lon")]
    pub longitude: f64,
    #[serde(rename = "t")]
    pub temperature_k: Option<f64>,
    #[serde(rename = "td")]
    pub dew_point_k: Option<f64>,
    #[serde(rename = "u")]
    pub humidity_percent: Option<i32>,
    #[serde(rename = "dd")]
    pub wind_direction_deg: Option<i32>,
    #[serde(rename = "ff")]
    pub wind_speed_m_s: Option<f64>,
    #[serde(rename = "ddraf10", alias = "dxi10")]
    pub gust_direction_deg: Option<i32>,
    #[serde(rename = "raf10", alias = "fxi10")]
    pub gust_speed_m_s: Option<f64>,
    #[serde(rename = "rr_per")]
    pub rainfall_mm: Option<f64>,
    #[serde(rename = "sss")]
    pub snow_depth_m: Option<f64>,
    #[serde(rename = "pres")]
    pub pressure_pa: Option<f64>,
    #[serde(rename = "pmer")]
    pub sea_level_pressure_pa: Option<f64>,
    #[serde(skip)]
    pub raw_data: serde_json::Value,
}

fn parse_observations(data: Vec<serde_json::Value>, station_id: &str) -> Result<Vec<Observation>> {
    data.into_iter()
        .map(|raw_data| {
            let mut observation: Observation = serde_json::from_value(raw_data.clone())
                .context("Invalid Météo-France observation")?;
            ensure!(
                observation.station_id == station_id,
                "Météo-France returned an observation for a different station"
            );
            observation.raw_data = raw_data;
            Ok(observation)
        })
        .collect()
}

/// Fetch and persist the latest measurement, including any provider correction.
pub async fn sync(pool: &PgPool, client: &mut Client) -> Result<u64> {
    let observations = client.observations().await?;
    if observations.is_empty() {
        warn!(station_id = %client.station_id(), "Météo-France returned no observations");
    }
    database::meteo::store(pool, &observations).await
}

/// Poll immediately on startup, then every six minutes without catch-up bursts.
pub async fn poll(pool: PgPool, mut client: Client) {
    let mut interval = tokio::time::interval(POLL_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        match sync(&pool, &mut client).await {
            Ok(rows) => {
                info!(station_id = %client.station_id(), rows, "Météo-France observations imported");
            }
            Err(error) => {
                warn!(station_id = %client.station_id(), error = %format!("{error:#}"), "Météo-France import failed; will retry next cycle");
            }
        }
    }
}
