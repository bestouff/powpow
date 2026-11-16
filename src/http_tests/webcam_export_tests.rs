use super::*;
use chrono::{Local, NaiveDate, TimeZone, Utc};

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn webcam_exports_require_staff_and_preserve_camera_date_and_timing(
    pool: PgPool,
) -> anyhow::Result<()> {
    let mut state = state(pool);
    let directory = tempfile::tempdir()?;
    let today = Local::now().date_naive();
    let date = today.pred_opt().unwrap();
    let at = |date: NaiveDate, hour, minute| {
        Local
            .from_local_datetime(&date.and_hms_opt(hour, minute, 0).unwrap())
            .single()
            .unwrap()
            .with_timezone(&Utc)
    };
    let sunrise = at(date, 8, 0);
    let sunset = at(date, 17, 0);
    state
        .webcam_sun
        .seed(date, Some(webcam::sun::Times { sunrise, sunset }))
        .await;
    state.webcam_sun.seed(today, None).await;
    let image = image::RgbImage::from_pixel(48, 30, image::Rgb([40, 150, 210]));
    for (camera, minute) in [
        ("station", 0),
        ("station", 2),
        ("station", 9),
        ("station", 10),
        ("other", 5),
    ] {
        let name = format!(
            "{}_{}.png",
            at(date, 10, minute).to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
            camera
        );
        image.save(directory.path().join(name))?;
    }
    state.webcam = webcam::Store::open(
        directory.path().into(),
        false,
        webcam::metadata::Settings::default(),
    )
    .await?;
    let (person, cookie) = member(&state, "webcam-export@example.org").await?;
    // Authentication is sufficient for webcam exports, including unpaid staff.
    sqlx::query("DELETE FROM payments WHERE staff = $1")
        .bind(person.id)
        .execute(&state.db)
        .await?;
    let app = build_router(state.clone());
    let video =
        format!("/webcam/export/video?date={date}&camera=station&start=10:00&end=10:10&speed=100");
    for path in ["/webcam/export", video.as_str()] {
        for session in [None, Some("aghil_session=invalid")] {
            let response = request(&app, "GET", path, session, serde_json::Value::Null).await;
            assert_eq!(response.status(), StatusCode::SEE_OTHER);
            assert_eq!(response.headers()["location"], "/login");
        }
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/webcam?camera=station")
                .header("x-forwarded-prefix", "/powpow")
                .body(Body::empty())?,
        )
        .await?;
    assert!(response_text(response).await?.contains(&format!(
        "href=\"/powpow/webcam/export?date={date}&amp;camera=station\""
    )));
    let response = request(
        &app,
        "GET",
        "/webcam/export?camera=station",
        Some(&cookie),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    let html = response_text(response).await?;
    assert!(html.contains(&format!("name=\"date\" value=\"{date}\"")));
    assert!(html.contains("value=\"2000\""));
    assert!(html.contains("id=\"export-range-start\""));
    assert!(html.contains("id=\"export-range-end\""));
    let response = request(
        &app,
        "GET",
        &format!("/webcam/export?camera=station&date={today}"),
        Some(&cookie),
        serde_json::Value::Null,
    )
    .await;
    let html = response_text(response).await?;
    assert!(html.contains(&format!("name=\"date\" value=\"{today}\"")));
    assert!(html.contains("horaires du soleil sont indisponibles"));
    assert!(html.contains("Aucune image disponible"));
    assert!(!html.contains("id=\"export-selection\""));
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/webcam/export?camera=station&date={date}&speed=200"
                ))
                .header("cookie", &cookie)
                .header("x-forwarded-prefix", "/powpow")
                .body(Body::empty())?,
        )
        .await?;
    let html = response_text(response).await?;
    assert!(html.contains("action=\"/powpow/webcam/export/video\""));
    assert!(html.contains("value=\"08:00:00\""));
    assert!(html.contains("value=\"17:00:00\""));
    assert!(html.contains("value=\"200\""));
    assert!(html.contains("value=\"station\""));
    assert!(html.contains(&format!(
        "data-list=\"/powpow/webcam/{date}_station_list.json\""
    )));
    assert!(html.contains(&format!(
        "href=\"/powpow/webcam?date={date}&amp;camera=station\""
    )));
    for side in ["start", "end"] {
        assert!(html.contains(&format!("id=\"export-preview-{side}\"")));
    }

    let response = request(&app, "GET", &video, Some(&cookie), serde_json::Value::Null).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "video/mp4");
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    assert!(
        response.headers()["content-disposition"]
            .to_str()?
            .contains(&format!("webcam-station-{date}-100000-101000-100x.mp4"))
    );
    let bytes = to_bytes(response.into_body(), 1_000_000).await?;
    let mp4 = mp4::Mp4Reader::read_header(std::io::Cursor::new(&bytes), bytes.len() as u64)?;
    assert_eq!(mp4.duration(), std::time::Duration::from_secs(6));
    assert_eq!(mp4.tracks()[&1].sample_count(), 3);

    for (path, expected) in [
        (
            video.replace("speed=100", "speed=0"),
            StatusCode::BAD_REQUEST,
        ),
        (
            video.replace("speed=100", "speed=10001"),
            StatusCode::BAD_REQUEST,
        ),
        (
            video.replace("speed=100", "speed=NaN"),
            StatusCode::BAD_REQUEST,
        ),
        (
            video.replace("start=10:00", "start=11:00"),
            StatusCode::BAD_REQUEST,
        ),
        (
            video.replace("start=10:00", "start=25:00"),
            StatusCode::BAD_REQUEST,
        ),
        (
            video.replace("camera=station", "camera=..%2Fsecret"),
            StatusCode::BAD_REQUEST,
        ),
        (
            video.replace("camera=station", "camera=unknown"),
            StatusCode::NOT_FOUND,
        ),
        (
            video.replace(&date.to_string(), &today.succ_opt().unwrap().to_string()),
            StatusCode::BAD_REQUEST,
        ),
        (
            video.replace("start=10:00&end=10:10", "start=00:00&end=01:00"),
            StatusCode::NOT_FOUND,
        ),
    ] {
        assert_eq!(
            request(&app, "GET", &path, Some(&cookie), serde_json::Value::Null)
                .await
                .status(),
            expected,
            "{path}"
        );
    }
    sqlx::query("DELETE FROM sessions WHERE staff = $1")
        .bind(person.id)
        .execute(&state.db)
        .await?;
    assert_eq!(
        request(&app, "GET", &video, Some(&cookie), serde_json::Value::Null)
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    Ok(())
}
