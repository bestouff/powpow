//! Open-Meteo sun times for recent and archived days, expressed in server time.

use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDate, TimeDelta, Utc};
use serde::Deserialize;
use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

use super::config::Coordinates;

#[derive(Clone, Copy, Debug)]
pub struct Times {
    pub sunrise: DateTime<Utc>,
    pub sunset: DateTime<Utc>,
}

impl Times {
    pub fn defaults(self, now: DateTime<Utc>) -> (chrono::NaiveTime, chrono::NaiveTime) {
        (
            self.sunrise.with_timezone(&Local).time(),
            self.sunset.min(now).with_timezone(&Local).time(),
        )
    }
}

struct Entry {
    key: (NaiveDate, u64, u64),
    times: Option<Times>,
    fetched: Instant,
}

#[derive(Clone, Default)]
pub struct Cache(Arc<Mutex<VecDeque<Entry>>>);

impl Cache {
    #[cfg(test)]
    pub async fn seed(&self, date: NaiveDate, times: Option<Times>) {
        self.0.lock().await.push_back(Entry {
            key: (
                date,
                crate::forecast::STATION_LATITUDE.to_bits(),
                crate::forecast::STATION_LONGITUDE.to_bits(),
            ),
            times,
            fetched: Instant::now(),
        });
    }

    pub async fn get(&self, date: NaiveDate, coordinates: Option<Coordinates>) -> Option<Times> {
        let coordinates = coordinates.unwrap_or(Coordinates {
            latitude: crate::forecast::STATION_LATITUDE,
            longitude: crate::forecast::STATION_LONGITUDE,
        });
        let key = (
            date,
            coordinates.latitude.to_bits(),
            coordinates.longitude.to_bits(),
        );
        // Coalesce simultaneous page loads, including short-lived failures.
        let mut entries = self.0.lock().await;
        if let Some(entry) = entries.iter().find(|entry| {
            entry.key == key
                && entry.fetched.elapsed()
                    < Duration::from_secs(if entry.times.is_some() { 86_400 } else { 60 })
        }) {
            return entry.times;
        }
        let times = match fetch(date, coordinates).await {
            Ok(times) => Some(times),
            Err(error) => {
                tracing::warn!(%date, %error, "Cannot fetch Open-Meteo sun times");
                None
            }
        };
        entries.retain(|entry| entry.key != key);
        if entries.len() == 64 {
            entries.pop_front();
        }
        entries.push_back(Entry {
            key,
            times,
            fetched: Instant::now(),
        });
        times
    }
}

#[derive(Deserialize)]
struct Response {
    daily: Daily,
}

#[derive(Deserialize)]
struct Daily {
    sunrise: Vec<Option<i64>>,
    sunset: Vec<Option<i64>>,
}

impl Response {
    fn for_date(self, date: NaiveDate) -> Result<Times> {
        self.daily
            .sunrise
            .into_iter()
            .zip(self.daily.sunset)
            .find_map(|(rise, set)| {
                let sunrise = DateTime::from_timestamp(rise?, 0)?;
                let sunset = DateTime::from_timestamp(set?, 0)?;
                (sunrise.with_timezone(&Local).date_naive() == date
                    && sunset.with_timezone(&Local).date_naive() == date
                    && sunset > sunrise)
                    .then_some(Times { sunrise, sunset })
            })
            .context("No sunrise and sunset for the selected server-local date")
    }
}

async fn fetch(date: NaiveDate, coordinates: Coordinates) -> Result<Times> {
    let today = Local::now().date_naive();
    let endpoint = if date < today - TimeDelta::days(5) {
        "https://archive-api.open-meteo.com/v1/archive"
    } else {
        "https://api.open-meteo.com/v1/forecast"
    };
    // Fetch adjacent UTC dates too: the archive and form use the server's TZ.
    let start = date.pred_opt().context("Date out of range")?;
    let end = date.succ_opt().context("Date out of range")?;
    reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()?
        .get(endpoint)
        .query(&[
            ("latitude", coordinates.latitude.to_string()),
            ("longitude", coordinates.longitude.to_string()),
            ("daily", "sunrise,sunset".into()),
            ("timezone", "UTC".into()),
            ("timeformat", "unixtime".into()),
            ("start_date", start.to_string()),
            ("end_date", end.to_string()),
        ])
        .send()
        .await?
        .error_for_status()?
        .json::<Response>()
        .await?
        .for_date(date)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn sun_times_select_local_day_and_cap_only_todays_end() -> Result<()> {
        let date: NaiveDate = "2026-01-15".parse()?;
        let sunrise = Local
            .from_local_datetime(&date.and_hms_opt(8, 10, 0).unwrap())
            .single()
            .unwrap()
            .with_timezone(&Utc);
        let sunset = Local
            .from_local_datetime(&date.and_hms_opt(17, 20, 0).unwrap())
            .single()
            .unwrap()
            .with_timezone(&Utc);
        let json = serde_json::json!({"daily": {
            "sunrise": [null, (sunrise - TimeDelta::days(1)).timestamp(), sunrise.timestamp()],
            "sunset": [null, (sunset - TimeDelta::days(1)).timestamp(), sunset.timestamp()]
        }});
        let times = serde_json::from_value::<Response>(json.clone())?.for_date(date)?;
        assert_eq!(times.sunrise, sunrise);
        let noon = sunrise + TimeDelta::hours(4);
        assert_eq!(times.defaults(noon).1, noon.with_timezone(&Local).time());
        assert_eq!(
            times.defaults(sunset + TimeDelta::days(1)).1,
            sunset.with_timezone(&Local).time()
        );
        assert!(
            serde_json::from_value::<Response>(json)?
                .for_date(date.succ_opt().unwrap())
                .is_err()
        );
        assert!(
            serde_json::from_value::<Response>(
                serde_json::json!({"daily": {"sunrise": [null], "sunset": [null]}})
            )?
            .for_date(date)
            .is_err()
        );
        Ok(())
    }
}
