use super::*;
use serde_json::{Value, json};
use sqlx::Row;

fn fixture() -> Vec<Value> {
    serde_json::from_str(include_str!("fixtures/saint_hilaire.json")).unwrap()
}

#[test]
fn configuration_is_optional_and_keeps_credentials_private() -> Result<()> {
    let config = |station: &str, token: &str, application: &str| {
        Config::parse(|key| match key {
            "METEO_FRANCE_STATION_ID" => station.into(),
            "METEO_FRANCE_TOKEN" => token.into(),
            "METEO_FRANCE_APPLICATION_ID" => application.into(),
            _ => unreachable!(),
        })
    };
    assert!(config("38395004", "", "")?.is_none());
    for station in ["", "3839500", "383950040", "abcdefgh"] {
        assert!(config(station, "secret-token", "").is_err());
    }
    let configured = config(" 38395004 ", "secret-token", "secret-application")?.unwrap();
    assert_eq!(configured.station_id, "38395004");
    let debug = format!("{configured:?}");
    assert!(!debug.contains("secret"));
    assert!(debug.contains("38395004"));
    assert!(config("01014002", "", "secret-application")?.is_some());
    Ok(())
}

#[test]
fn observations_preserve_missing_and_unknown_measurements() -> Result<()> {
    let mut data = fixture();
    data[0]["future_sensor"] = json!({"quality": 7});
    data[0]["t"] = Value::Null;
    data[0].as_object_mut().unwrap().remove("td");
    let observations = parse_observations(data.clone(), "38395004")?;
    assert!(observations[0].temperature_k.is_none());
    assert!(observations[0].dew_point_k.is_none());
    assert!(observations[0].wind_speed_m_s.is_none());
    assert_eq!(observations[0].raw_data, data[0]);

    let object = data[0].as_object_mut().unwrap();
    object.remove("ddraf10");
    object.remove("raf10");
    object.insert("dxi10".into(), json!(270));
    object.insert("fxi10".into(), json!(12.5));
    let observations = parse_observations(data, "38395004")?;
    assert_eq!(observations[0].gust_direction_deg, Some(270));
    assert!((observations[0].gust_speed_m_s.unwrap() - 12.5).abs() < 1e-9);
    Ok(())
}

#[test]
fn malformed_or_wrong_station_observations_fail_the_batch() {
    assert!(parse_observations(fixture(), "38395403").is_err());
    for (key, value) in [("validity_time", json!("invalid")), ("t", json!("invalid"))] {
        let mut data = fixture();
        let mut invalid = data[0].clone();
        invalid[key] = value;
        data.push(invalid);
        assert!(parse_observations(data, "38395004").is_err());
    }
    assert!(
        parse_observations(Vec::new(), "38395004")
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn hourly_rainfall_uses_ten_complete_periods_for_the_selected_station_and_time(
    pool: PgPool,
) -> Result<()> {
    let at: DateTime<Utc> = "2026-11-12T00:24:00Z".parse()?;
    sqlx::query(
        "INSERT INTO meteo (station_id, observed_at, reference_time, insert_time,
                            latitude, longitude, rainfall_mm, raw_data)
         SELECT '38395004', sample, sample, sample, 45.315, 5.884833,
                CASE WHEN sample > $1 - interval '1 hour' AND sample <= $1 THEN 0.2 ELSE 99 END,
                '{}'
         FROM generate_series($1 - interval '66 minutes', $1 + interval '6 minutes',
                              interval '6 minutes') AS sample",
    )
    .bind(at)
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO meteo (station_id, observed_at, reference_time, insert_time,
                            latitude, longitude, rainfall_mm, raw_data)
         VALUES ('38395403', $1, $1, $1, 45.315, 5.884833, 999, '{}')",
    )
    .bind(at)
    .execute(&pool)
    .await?;
    let reading =
        database::meteo::at_or_before(&pool, "38395004", at + chrono::TimeDelta::minutes(1))
            .await?
            .unwrap();
    assert_eq!(reading.observed_at, at);
    assert!((reading.rainfall_last_hour_mm.unwrap() - 2.0).abs() < 1e-9);

    sqlx::query("UPDATE meteo SET rainfall_mm = 1.2, fetched_at = $2 WHERE station_id = '38395004' AND observed_at = $1")
        .bind(at - chrono::TimeDelta::minutes(30))
        .bind(reading.fetched_at + chrono::TimeDelta::seconds(1))
        .execute(&pool).await?;
    let corrected = database::meteo::at_or_before(&pool, "38395004", at)
        .await?
        .unwrap();
    assert!((corrected.rainfall_last_hour_mm.unwrap() - 3.0).abs() < 1e-9);
    assert_eq!(corrected.observed_at, reading.observed_at);
    assert_ne!(corrected.revision(), reading.revision());

    sqlx::query("UPDATE meteo SET rainfall_mm = 0 WHERE station_id = '38395004'")
        .execute(&pool)
        .await?;
    let dry = database::meteo::at_or_before(&pool, "38395004", at)
        .await?
        .unwrap();
    assert!(dry.rainfall_last_hour_mm.unwrap().abs() < 1e-9);

    sqlx::query(
        "UPDATE meteo SET rainfall_mm = NULL WHERE station_id = '38395004' AND observed_at = $1",
    )
    .bind(at - chrono::TimeDelta::minutes(24))
    .execute(&pool)
    .await?;
    let incomplete = database::meteo::at_or_before(&pool, "38395004", at)
        .await?
        .unwrap();
    assert!(incomplete.rainfall_last_hour_mm.is_none());
    sqlx::query("DELETE FROM meteo WHERE station_id = '38395004' AND observed_at = $1")
        .bind(at - chrono::TimeDelta::minutes(24))
        .execute(&pool)
        .await?;
    let missing = database::meteo::at_or_before(&pool, "38395004", at)
        .await?
        .unwrap();
    assert!(missing.rainfall_last_hour_mm.is_none());
    assert!(
        database::meteo::at_or_before(&pool, "missing", at)
            .await?
            .is_none()
    );
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn archive_converts_units_deduplicates_corrects_and_survives_backup(
    pool: PgPool,
) -> Result<()> {
    let original = parse_observations(fixture(), "38395004")?;
    assert_eq!(database::meteo::store(&pool, &original).await?, 1);
    assert_eq!(database::meteo::store(&pool, &original).await?, 0);
    let row = sqlx::query("SELECT * FROM meteo").fetch_one(&pool).await?;
    assert!((row.try_get::<f64, _>("temperature_c")? - 4.7).abs() < 1e-9);
    assert!(row.try_get::<Option<f64>, _>("dew_point_c")?.is_none());
    assert!(row.try_get::<Option<f64>, _>("wind_speed_m_s")?.is_none());
    assert!(row.try_get::<f64, _>("rainfall_mm")?.abs() < 1e-9);
    assert_eq!(row.try_get::<Value, _>("raw_data")?, original[0].raw_data);
    assert_eq!(
        row.try_get::<DateTime<Utc>, _>("observed_at")?,
        original[0].observed_at
    );

    let mut correction = fixture();
    correction[0]["reference_time"] = json!("2026-11-04T13:25:00Z");
    correction[0]["t"] = json!(278.15);
    correction[0]["td"] = json!(273.15);
    correction[0]["u"] = json!(82);
    correction[0]["dd"] = json!(180);
    correction[0]["ff"] = json!(2.5);
    correction[0]["ddraf10"] = json!(190);
    correction[0]["raf10"] = json!(7.5);
    correction[0]["rr_per"] = json!(0.2);
    correction[0]["sss"] = json!(0.3);
    correction[0]["pres"] = json!(90_000.0);
    correction[0]["pmer"] = json!(101_300.0);
    let corrected = parse_observations(correction.clone(), "38395004")?;
    assert_eq!(database::meteo::store(&pool, &corrected).await?, 1);
    assert_eq!(database::meteo::store(&pool, &original).await?, 0);
    let row = sqlx::query("SELECT * FROM meteo").fetch_one(&pool).await?;
    for (column, expected) in [
        ("temperature_c", 5.0),
        ("dew_point_c", 0.0),
        ("wind_speed_m_s", 2.5),
        ("gust_speed_m_s", 7.5),
        ("rainfall_mm", 0.2),
        ("snow_depth_m", 0.3),
        ("pressure_pa", 90_000.0),
        ("sea_level_pressure_pa", 101_300.0),
    ] {
        assert!(
            (row.try_get::<f64, _>(column)? - expected).abs() < 1e-9,
            "{column}"
        );
    }
    assert_eq!(row.try_get::<i32, _>("humidity_percent")?, 82);
    assert_eq!(row.try_get::<i32, _>("wind_direction_deg")?, 180);
    assert_eq!(row.try_get::<i32, _>("gust_direction_deg")?, 190);

    let backup = database::backup_all_tables(&pool).await?;
    assert!(backup.contains("COPY meteo ("));
    correction[0]["validity_time"] = json!("2026-11-04T13:24:00Z");
    correction[0]["t"] = Value::Null;
    let next = parse_observations(correction, "38395004")?;
    assert_eq!(database::meteo::store(&pool, &next).await?, 1);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM meteo")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 2);
    database::restore_from_sql(&pool, &backup).await?;
    let restored: Vec<Value> = sqlx::query_scalar("SELECT raw_data FROM meteo")
        .fetch_all(&pool)
        .await?;
    assert_eq!(restored, vec![corrected[0].raw_data.clone()]);
    Ok(())
}
