use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::{meteo::Observation, models::MeteoReading};

/// Find the most recent reading and its preceding hour of rainfall for this station.
pub async fn at_or_before(
    pool: &PgPool,
    station_id: &str,
    at: DateTime<Utc>,
) -> Result<Option<MeteoReading>> {
    Ok(sqlx::query_as(
        "WITH latest AS (
             SELECT * FROM meteo WHERE station_id = $1 AND observed_at <= $2
             ORDER BY observed_at DESC LIMIT 1
         )
         SELECT latest.station_id, latest.observed_at,
                GREATEST(latest.fetched_at, rainfall.fetched_at) AS fetched_at,
                latest.temperature_c, latest.dew_point_c, latest.humidity_percent,
                latest.wind_direction_deg, latest.wind_speed_m_s,
                latest.gust_direction_deg, latest.gust_speed_m_s,
                rainfall.total_mm AS rainfall_last_hour_mm,
                latest.snow_depth_m, latest.pressure_pa, latest.sea_level_pressure_pa
         FROM latest
         CROSS JOIN LATERAL (
             SELECT CASE WHEN count(reading.rainfall_mm) = 10
                         THEN sum(reading.rainfall_mm) END AS total_mm,
                    max(reading.fetched_at) AS fetched_at
             FROM generate_series(latest.observed_at - interval '54 minutes',
                                  latest.observed_at, interval '6 minutes') AS sample(at)
             LEFT JOIN meteo AS reading
                 ON reading.station_id = latest.station_id AND reading.observed_at = sample.at
         ) AS rainfall",
    )
    .bind(station_id)
    .bind(at)
    .fetch_optional(pool)
    .await?)
}

/// One row per station and sensor timestamp. Repeated polls are harmless, and
/// an older product cannot overwrite a more recent provider correction.
pub async fn store(pool: &PgPool, observations: &[Observation]) -> Result<u64> {
    let mut tx = pool.begin().await?;
    let mut changed = 0;
    for observation in observations {
        changed += sqlx::query(
            "INSERT INTO meteo (
                station_id, observed_at, reference_time, insert_time, latitude, longitude,
                temperature_c, dew_point_c, humidity_percent, wind_direction_deg, wind_speed_m_s,
                gust_direction_deg, gust_speed_m_s, rainfall_mm, snow_depth_m,
                pressure_pa, sea_level_pressure_pa, raw_data
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
             ON CONFLICT (station_id, observed_at) DO UPDATE SET
                reference_time = EXCLUDED.reference_time, insert_time = EXCLUDED.insert_time,
                fetched_at = now(), latitude = EXCLUDED.latitude, longitude = EXCLUDED.longitude,
                temperature_c = EXCLUDED.temperature_c, dew_point_c = EXCLUDED.dew_point_c,
                humidity_percent = EXCLUDED.humidity_percent, wind_direction_deg = EXCLUDED.wind_direction_deg,
                wind_speed_m_s = EXCLUDED.wind_speed_m_s, gust_direction_deg = EXCLUDED.gust_direction_deg,
                gust_speed_m_s = EXCLUDED.gust_speed_m_s, rainfall_mm = EXCLUDED.rainfall_mm,
                snow_depth_m = EXCLUDED.snow_depth_m, pressure_pa = EXCLUDED.pressure_pa,
                sea_level_pressure_pa = EXCLUDED.sea_level_pressure_pa, raw_data = EXCLUDED.raw_data
             WHERE EXCLUDED.reference_time >= meteo.reference_time AND EXCLUDED.raw_data IS DISTINCT FROM meteo.raw_data",
        )
        .bind(&observation.station_id)
        .bind(observation.observed_at)
        .bind(observation.reference_time)
        .bind(observation.insert_time)
        .bind(observation.latitude)
        .bind(observation.longitude)
        .bind(observation.temperature_k.map(|kelvin| kelvin - 273.15))
        .bind(observation.dew_point_k.map(|kelvin| kelvin - 273.15))
        .bind(observation.humidity_percent)
        .bind(observation.wind_direction_deg)
        .bind(observation.wind_speed_m_s)
        .bind(observation.gust_direction_deg)
        .bind(observation.gust_speed_m_s)
        .bind(observation.rainfall_mm)
        .bind(observation.snow_depth_m)
        .bind(observation.pressure_pa)
        .bind(observation.sea_level_pressure_pa)
        .bind(&observation.raw_data)
        .execute(&mut *tx).await?.rows_affected();
    }
    tx.commit().await?;
    Ok(changed)
}
