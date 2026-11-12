//! Station forecasts from Open-Meteo, refreshed independently of page requests.

use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDate, Utc};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct WeatherResponse {
    pub current: Option<CurrentWeather>,
    pub daily: Option<DailyWeather>,
}

#[derive(Debug, Deserialize)]
pub struct CurrentWeather {
    pub temperature_2m: Option<f64>,
    pub relative_humidity_2m: Option<f64>,
    pub apparent_temperature: Option<f64>,
    pub precipitation: Option<f64>,
    pub snowfall: Option<f64>,
    pub cloud_cover: Option<f64>,
    pub wind_speed_10m: Option<f64>,
    pub wind_gusts_10m: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct DailyWeather {
    pub time: Vec<NaiveDate>,
    pub weather_code: Option<Vec<Option<u8>>>,
    pub temperature_2m_max: Option<Vec<Option<f64>>>,
    pub temperature_2m_min: Option<Vec<Option<f64>>>,
    pub precipitation_sum: Option<Vec<Option<f64>>>,
    pub snowfall_sum: Option<Vec<Option<f64>>>,
    pub wind_speed_10m_max: Option<Vec<Option<f64>>>,
    pub wind_gusts_10m_max: Option<Vec<Option<f64>>>,
    pub sunrise: Option<Vec<String>>,
    pub sunset: Option<Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct Forecast {
    pub date: NaiveDate,
    pub fetched_at: DateTime<Utc>,
    pub weather_code: Option<u8>,
    pub temperature_min_c: Option<f64>,
    pub temperature_max_c: Option<f64>,
    pub precipitation_mm: Option<f64>,
    pub snowfall_cm: Option<f64>,
    pub wind_max_kmh: Option<f64>,
    pub gust_max_kmh: Option<f64>,
}

impl WeatherResponse {
    pub fn forecast(&self, date: NaiveDate, fetched_at: DateTime<Utc>) -> Option<Forecast> {
        let daily = self.daily.as_ref()?;
        let index = daily.time.iter().position(|day| *day == date)?;
        Some(Forecast {
            date,
            fetched_at,
            weather_code: daily_value(daily.weather_code.as_deref(), index),
            temperature_min_c: daily_value(daily.temperature_2m_min.as_deref(), index),
            temperature_max_c: daily_value(daily.temperature_2m_max.as_deref(), index),
            precipitation_mm: daily_value(daily.precipitation_sum.as_deref(), index),
            snowfall_cm: daily_value(daily.snowfall_sum.as_deref(), index),
            wind_max_kmh: daily_value(daily.wind_speed_10m_max.as_deref(), index),
            gust_max_kmh: daily_value(daily.wind_gusts_10m_max.as_deref(), index),
        })
    }
}

fn daily_value<T: Copy>(values: Option<&[Option<T>]>, index: usize) -> Option<T> {
    values?.get(index).copied().flatten()
}

#[derive(Clone, Default)]
pub struct Cache(Arc<RwLock<Option<Forecast>>>);

impl Cache {
    pub fn for_date(&self, date: NaiveDate) -> Option<Forecast> {
        self.0
            .read()
            .expect("forecast cache lock")
            .as_ref()
            .filter(|forecast| forecast.date == date)
            .cloned()
    }

    pub fn store(&self, forecast: Forecast) {
        *self.0.write().expect("forecast cache lock") = Some(forecast);
    }
}

pub async fn fetch_weather() -> Result<WeatherResponse> {
    // Plateau des Petites Roches, at the same elevation used for the daily saying.
    const URL: &str = "https://api.open-meteo.com/v1/forecast?\
        latitude=45.31&longitude=5.85&elevation=1200\
        &current=temperature_2m,relative_humidity_2m,apparent_temperature,\
        precipitation,snowfall,cloud_cover,wind_speed_10m,wind_gusts_10m\
        &daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_sum,\
        snowfall_sum,wind_speed_10m_max,wind_gusts_10m_max,sunrise,sunset\
        &temperature_unit=celsius&wind_speed_unit=kmh&precipitation_unit=mm\
        &timezone=Europe%2FParis&forecast_days=1";
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?
        .get(URL)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .context("Invalid Open-Meteo forecast")
}

/// Poll immediately, then every fifteen minutes. Keep the last successful forecast.
pub async fn poll(cache: Cache) {
    let mut interval = tokio::time::interval(Duration::from_mins(15));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        let result = async {
            fetch_weather()
                .await?
                .forecast(Local::now().date_naive(), Utc::now())
                .context("Open-Meteo returned no forecast for today")
        }
        .await;
        match result {
            Ok(forecast) => cache.store(forecast),
            Err(error) => tracing::warn!(%error, "Cannot refresh the station forecast"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forecast_selects_the_requested_day_and_preserves_missing_values() -> Result<()> {
        let response: WeatherResponse = serde_json::from_value(serde_json::json!({
            "daily": {
                "time": ["2026-11-11", "2026-11-12"],
                "weather_code": [0, 73],
                "temperature_2m_min": [5.0, -2.5],
                "temperature_2m_max": [12.0, null],
                "precipitation_sum": [0.0, 6.0],
                "snowfall_sum": [0.0, 4.5],
                "wind_speed_10m_max": [10.0],
                "wind_gusts_10m_max": null
            }
        }))?;
        let today = "2026-11-12".parse()?;
        let forecast = response.forecast(today, Utc::now()).unwrap();
        assert_eq!(forecast.weather_code, Some(73));
        assert_eq!(forecast.temperature_min_c, Some(-2.5));
        assert_eq!(forecast.temperature_max_c, None);
        assert_eq!(forecast.precipitation_mm, Some(6.0));
        assert_eq!(forecast.snowfall_cm, Some(4.5));
        assert_eq!(forecast.wind_max_kmh, None);
        assert_eq!(forecast.gust_max_kmh, None);
        assert!(
            response
                .forecast("2026-11-13".parse()?, Utc::now())
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn forecast_cache_never_serves_another_days_forecast() -> Result<()> {
        let cache = Cache::default();
        let today: NaiveDate = "2026-11-12".parse()?;
        assert!(cache.for_date(today).is_none());
        let response: WeatherResponse = serde_json::from_value(serde_json::json!({
            "daily": {"time": [today], "weather_code": [61]}
        }))?;
        cache.store(response.forecast(today, Utc::now()).unwrap());
        assert_eq!(
            cache.clone().for_date(today).unwrap().weather_code,
            Some(61)
        );
        assert!(cache.for_date(today.pred_opt().unwrap()).is_none());
        assert!(cache.for_date(today.succ_opt().unwrap()).is_none());
        let missing: WeatherResponse = serde_json::from_value(serde_json::json!({"daily": null}))?;
        assert!(missing.forecast(today, Utc::now()).is_none());
        Ok(())
    }
}
