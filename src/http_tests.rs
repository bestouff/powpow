use super::*;
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use tower::ServiceExt;

async fn response_text(response: Response) -> anyhow::Result<String> {
    Ok(String::from_utf8(
        to_bytes(response.into_body(), 1_000_000).await?.to_vec(),
    )?)
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn today_requires_staff_and_shows_only_todays_assignments(
    pool: PgPool,
) -> anyhow::Result<()> {
    let state = state(pool);
    let today = chrono::Local::now().date_naive();
    let (viewer, cookie) = member(&state, "today-viewer@example.org").await?;
    let app = build_router(state.clone());
    for session in [None, Some("aghil_session=invalid")] {
        let response = request(&app, "GET", "/today", session, serde_json::Value::Null).await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers()["location"], "/login");
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/today")
                .header("x-forwarded-prefix", "/powpow")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.headers()["location"], "/powpow/login");
    let response = request(
        &app,
        "GET",
        "/today",
        Some(&cookie),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    assert!(
        response_text(response)
            .await?
            .contains("Aucun atelier prévu aujourd'hui.")
    );

    let mut staff = Vec::new();
    for (index, name) in [
        "Alice <matin>",
        "Béatrice",
        "Charlie",
        "Diane",
        "Hors-date",
        "Intéressé",
    ]
    .into_iter()
    .enumerate()
    {
        let (person, _) = member(&state, &format!("today-{index}@example.org")).await?;
        sqlx::query("UPDATE staff SET first_name = $1, last_name = 'Bénévole' WHERE id = $2")
            .bind(name)
            .bind(person.id)
            .execute(&state.db)
            .await?;
        staff.push(person.id);
    }
    let mut ateliers = Vec::new();
    for (name, slug, nightly) in [
        ("Accueil <pistes>", "today-accueil", false),
        ("Buvette", "today-buvette", false),
        ("Nocturne", "today-nocturne", true),
    ] {
        let id: uuid::Uuid = sqlx::query_scalar("INSERT INTO ateliers (name, slug, needs_validation, default_nightly, icon) VALUES ($1, $2, false, $3, 'users') RETURNING id")
            .bind(name).bind(slug).bind(nightly).fetch_one(&state.db).await?;
        ateliers.push(id);
        database::upsert_need(&state.db, id, today, 4, 1, nightly).await?;
    }
    let needs = database::get_needs_for_day(&state.db, today).await?;
    let day_need = needs
        .iter()
        .find(|need| need.atelier == ateliers[0])
        .unwrap()
        .id;
    let night_need = needs
        .iter()
        .find(|need| need.atelier == ateliers[2])
        .unwrap()
        .id;
    for (person, first, second) in [
        (staff[0], true, false),
        (staff[1], false, true),
        (staff[2], true, true),
    ] {
        database::upsert_presence(&state.db, day_need, person, first, second).await?;
    }
    database::upsert_presence(&state.db, night_need, staff[3], true, true).await?;
    database::add_role(&state.db, staff[5], ateliers[0], true).await?;
    for day in [today.pred_opt().unwrap(), today.succ_opt().unwrap()] {
        database::upsert_need(&state.db, ateliers[0], day, 4, 1, false).await?;
        let need = database::get_needs_for_day(&state.db, day).await?.remove(0);
        database::upsert_presence(&state.db, need.id, staff[4], true, true).await?;
    }
    sqlx::query("INSERT INTO opening_days (day, status) VALUES ($1, 'validated')")
        .bind(today)
        .execute(&state.db)
        .await?;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/today")
                .header("cookie", &cookie)
                .header("x-forwarded-prefix", "/powpow")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_text(response).await?;
    assert!(body.contains("Infos du jour"));
    assert!(body.contains(&today.format("%d/%m/%Y").to_string()));
    assert!(body.contains("Ouverture confirmée."));
    assert!(body.contains("navbar-staff is-active\" href=\"/powpow/today\""));
    assert!(body.contains("href=\"/powpow/calendar/today-accueil\""));
    assert!(body.contains(&format!("href=\"/powpow/person/{}\"", staff[0])));
    assert!(body.contains("Accueil &lt;pistes&gt;"));
    assert!(body.contains("Alice &lt;matin&gt;"));
    assert!(!body.contains("<matin>"));
    for name in ["Hors-date", "Intéressé"] {
        assert!(!body.contains(name));
    }
    let morning = body
        .split_once(">Matin</h4>")
        .unwrap()
        .1
        .split("</section>")
        .next()
        .unwrap();
    assert!(morning.contains("Alice &lt;matin&gt;"));
    assert!(morning.contains("Charlie"));
    assert!(!morning.contains("Béatrice"));
    let afternoon = body
        .split_once(">Après-midi</h4>")
        .unwrap()
        .1
        .split("</section>")
        .next()
        .unwrap();
    assert!(afternoon.contains("Béatrice"));
    assert!(afternoon.contains("Charlie"));
    assert!(!afternoon.contains("Alice"));
    for label in ["Soirée", "Nuit"] {
        let shift = body
            .split_once(&format!(">{label}</h4>"))
            .unwrap()
            .1
            .split("</section>")
            .next()
            .unwrap();
        assert!(shift.contains("Diane"));
    }
    assert_eq!(body.matches("Aucun bénévole inscrit.").count(), 2);

    database::upsert_presence(&state.db, day_need, staff[0], false, false).await?;
    sqlx::query("UPDATE opening_days SET status = 'canceled' WHERE day = $1")
        .bind(today)
        .execute(&state.db)
        .await?;
    let body = response_text(
        request(
            &app,
            "GET",
            "/today",
            Some(&cookie),
            serde_json::Value::Null,
        )
        .await,
    )
    .await?;
    assert!(!body.contains("Alice"));
    assert!(body.contains("Journée d'ouverture annulée."));
    assert!(body.contains("Charlie"));

    sqlx::query("DELETE FROM payments WHERE staff = $1")
        .bind(viewer.id)
        .execute(&state.db)
        .await?;
    let response = request(
        &app,
        "GET",
        "/today",
        Some(&cookie),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()["location"], "/?membership_required=true");
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn today_shows_forecast_and_current_equipment_statuses(pool: PgPool) -> anyhow::Result<()> {
    let state = state(pool);
    let today = chrono::Local::now().date_naive();
    let (_, cookie) = member(&state, "today-conditions@example.org").await?;
    sqlx::query("DELETE FROM equipments")
        .execute(&state.db)
        .await?;
    for kind in [
        models::EquipmentType::SkiSlope,
        models::EquipmentType::SkiTow,
    ] {
        for status in [
            models::EquipmentStatus::Open,
            models::EquipmentStatus::Partial,
            models::EquipmentStatus::Closed,
        ] {
            sqlx::query(
                "INSERT INTO equipments (name, equipment_type, status) VALUES ($1, $2, $3)",
            )
            .bind(format!("{kind} <{status}>"))
            .bind(kind)
            .bind(status)
            .execute(&state.db)
            .await?;
        }
    }
    let app = build_router(state.clone());
    let body = response_text(
        request(
            &app,
            "GET",
            "/today",
            Some(&cookie),
            serde_json::Value::Null,
        )
        .await,
    )
    .await?;
    assert!(body.contains("Prévisions temporairement indisponibles."));
    assert!(body.contains("Aucun atelier prévu aujourd'hui."));

    let response: forecast::WeatherResponse = serde_json::from_value(serde_json::json!({
        "daily": {
            "time": [today],
            "weather_code": [73],
            "temperature_2m_min": [-2.5],
            "temperature_2m_max": [1.5],
            "precipitation_sum": [6.0],
            "snowfall_sum": [4.5],
            "wind_speed_10m_max": [12.0],
            "wind_gusts_10m_max": [30.0]
        }
    }))?;
    state
        .forecast
        .store(response.forecast(today, chrono::Utc::now()).unwrap());
    let body = response_text(
        request(
            &app,
            "GET",
            "/today",
            Some(&cookie),
            serde_json::Value::Null,
        )
        .await,
    )
    .await?;
    assert!(body.find("Prévisions météo").unwrap() < body.find("Planning du jour").unwrap());
    for value in [
        "Neige",
        "-2,5 °C",
        "1,5 °C",
        "6,0 mm",
        "4,5 cm",
        "12,0 km/h",
        "30,0 km/h",
        "Open-Meteo",
    ] {
        assert!(body.contains(value), "Missing forecast value: {value}");
    }
    assert!(!body.contains("Prévisions temporairement indisponibles."));
    for (name, label, class) in [
        ("ski-slope &lt;open&gt;", "Ouverte", "is-success"),
        ("ski-slope &lt;partial&gt;", "Partielle", "is-warning"),
        ("ski-slope &lt;closed&gt;", "Fermée", "is-danger"),
        ("ski-tow &lt;open&gt;", "Ouvert", "is-success"),
        ("ski-tow &lt;partial&gt;", "Partiel", "is-warning"),
        ("ski-tow &lt;closed&gt;", "Fermé", "is-danger"),
    ] {
        let row = body
            .split_once(name)
            .unwrap()
            .1
            .split("</li>")
            .next()
            .unwrap();
        assert!(row.contains(&format!("class=\"tag {class}\">{label}</span>")));
    }
    sqlx::query("UPDATE equipments SET status = 'closed'")
        .execute(&state.db)
        .await?;
    let mut old_forecast = state.forecast.for_date(today).unwrap();
    old_forecast.date = today.pred_opt().unwrap();
    state.forecast.store(old_forecast);
    let body = response_text(
        request(
            &app,
            "GET",
            "/today",
            Some(&cookie),
            serde_json::Value::Null,
        )
        .await,
    )
    .await?;
    assert!(body.contains("Prévisions temporairement indisponibles."));
    assert!(body.contains("0 / 3 ouvertes"));
    assert!(body.contains("0 / 3 ouverts"));
    assert!(body.contains("Aucun atelier prévu aujourd'hui."));
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn webcam_is_public_with_immutable_images_and_uncached_latest(
    pool: PgPool,
) -> anyhow::Result<()> {
    let mut state = state(pool);
    let directory = tempfile::tempdir()?;
    state.webcam = webcam::Store::open(
        directory.path().into(),
        true,
        webcam::metadata::Settings::default(),
    )
    .await?;
    state.config.webcam.title = "Pistes <nord>".into();
    state.config.webcam.direct = Some(webcam::config::DirectConfig {
        url: "https://camera.example/snapshot".parse()?,
        period: std::time::Duration::from_mins(6),
        name: "pistes_sud".into(),
        title: Some("Pistes <sud>".into()),
        insecure_tls: false,
    });
    let store = state.webcam.clone();
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(4, 3).write_to(&mut bytes, image::ImageFormat::Png)?;
    let bytes = bytes.into_inner();
    let frame = state
        .webcam
        .save(bytes.clone(), "station".into(), "png".into())
        .await?;
    let (person, cookie) = member(&state, "unpaid-webcam@example.org").await?;
    sqlx::query("DELETE FROM payments WHERE staff = $1")
        .bind(person.id)
        .execute(&state.db)
        .await?;
    let app = build_router(state);
    for session in [None, Some(cookie.as_str())] {
        let response = request(&app, "GET", "/webcam", session, serde_json::Value::Null).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let body = response_text(response).await?;
        assert!(body.contains("Pistes &lt;nord&gt;"));
        assert!(body.contains(&frame.filename));
        let response = request(
            &app,
            "GET",
            "/api/webcam/latest",
            session,
            serde_json::Value::Null,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&response_text(response).await?)?["image"]["filename"],
            frame.filename
        );
    }
    let image_path = format!("/webcam/images/{}", frame.filename);
    let response = request(
        &app,
        "GET",
        &image_path,
        Some(&cookie),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/png");
    assert!(
        response.headers()["cache-control"]
            .to_str()?
            .contains("immutable")
    );
    let etag = response.headers()["etag"].clone();
    assert_eq!(
        to_bytes(response.into_body(), 1_000_000).await?.as_ref(),
        std::fs::read(store.path(&frame))?
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&image_path)
                .header("if-none-match", etag)
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/webcam")
                .header("x-forwarded-prefix", "/powpow")
                .body(Body::empty())?,
        )
        .await?;
    assert!(
        response_text(response)
            .await?
            .contains("/powpow/webcam/images/")
    );
    for path in [
        "/webcam?image=missing.jpg",
        "/webcam/images/.upload-secret",
        "/webcam/images/..%2Fsecret.jpg",
    ] {
        assert_eq!(
            request(&app, "GET", path, None, serde_json::Value::Null)
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        request(
            &app,
            "GET",
            "/webcam?date=bad",
            None,
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let response = request(
        &app,
        "GET",
        "/webcam?date=2000-01-01",
        None,
        serde_json::Value::Null,
    )
    .await;
    assert!(response_text(response).await?.contains("Aucune image"));
    let direct = store.save(bytes, "pistes_sud".into(), "png".into()).await?;
    for (path, expected_title, other_title, redirect) in [
        (
            "/webcam".into(),
            "Pistes &lt;sud&gt;",
            "Pistes &lt;nord&gt;",
            None,
        ),
        (
            format!("/webcam?image={}", direct.filename),
            "Pistes &lt;sud&gt;",
            "Pistes &lt;nord&gt;",
            Some("/webcam"),
        ),
        (
            format!("/webcam?image={}", frame.filename),
            "Pistes &lt;nord&gt;",
            "Pistes &lt;sud&gt;",
            Some("/webcam?camera=station"),
        ),
    ] {
        let mut response = request(&app, "GET", &path, None, serde_json::Value::Null).await;
        if let Some(redirect) = redirect {
            assert_eq!(response.status(), StatusCode::SEE_OTHER);
            assert_eq!(response.headers()["location"], redirect);
            assert_eq!(response.headers()["cache-control"], "no-store");
            response = request(&app, "GET", redirect, None, serde_json::Value::Null).await;
        }
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_text(response).await?;
        assert!(body.contains(&format!("<strong>{expected_title}</strong>")));
        assert!(!body.contains(other_title));
    }
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn webcam_previews_and_latest_jpeg_are_public_and_revalidate(
    pool: PgPool,
) -> anyhow::Result<()> {
    let mut state = state(pool);
    let directory = tempfile::tempdir()?;
    state.webcam = webcam::Store::open(
        directory.path().into(),
        true,
        webcam::metadata::Settings::default(),
    )
    .await?;
    let store = state.webcam.clone();
    let mut encoded = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(4, 3).write_to(&mut encoded, image::ImageFormat::Png)?;
    let bytes = encoded.into_inner();
    let frame = store
        .save(bytes.clone(), "tkecole".into(), "png".into())
        .await?;
    store
        .save(bytes.clone(), "other".into(), "png".into())
        .await?;
    let (unpaid, cookie) = member(&state, "webcam-assets@example.org").await?;
    sqlx::query("DELETE FROM payments WHERE staff = $1")
        .bind(unpaid.id)
        .execute(&state.db)
        .await?;
    let app = build_router(state);
    let list_path = format!(
        "/webcam/{}",
        webcam::previews::list_name(frame.date(), "tkecole")
    );
    let thumbnail_path = format!("/webcam/images/{}", frame.thumbnail());
    for session in [None, Some(cookie.as_str())] {
        for path in [&list_path, &thumbnail_path, "/webcam/tkecole.jpg"] {
            assert_eq!(
                request(&app, "GET", path, session, serde_json::Value::Null)
                    .await
                    .status(),
                StatusCode::OK
            );
        }
    }
    let response = request(&app, "GET", &list_path, None, serde_json::Value::Null).await;
    let list_etag = response.headers()["etag"].clone();
    let list: serde_json::Value = serde_json::from_str(&response_text(response).await?)?;
    assert_eq!(list["camera"], "tkecole");
    assert_eq!(list["date"], frame.date().to_string());
    assert_eq!(list["images"].as_array().unwrap().len(), 1);
    assert_eq!(list["images"][0]["filename"], frame.filename);
    assert_eq!(list["images"][0]["thumbnail"], frame.thumbnail());
    assert_eq!(list["images"][0]["label"], frame.local_label());
    let response = request(&app, "GET", &thumbnail_path, None, serde_json::Value::Null).await;
    assert!(
        response.headers()["cache-control"]
            .to_str()?
            .contains("immutable")
    );
    assert_eq!(response.headers()["content-type"], "image/jpeg");
    assert_eq!(
        image::guess_format(&to_bytes(response.into_body(), 1_000_000).await?)?,
        image::ImageFormat::Jpeg
    );

    let response = request(
        &app,
        "GET",
        "/webcam/tkecole.jpg",
        None,
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(
        response.headers()["cache-control"],
        "public, max-age=60, must-revalidate"
    );
    assert_eq!(response.headers()["content-type"], "image/jpeg");
    let etag = response.headers()["etag"].clone();
    let modified = response.headers()["last-modified"].clone();
    let length = response.headers()["content-length"].clone();
    let body = to_bytes(response.into_body(), 1_000_000).await?;
    assert_eq!(image::guess_format(&body)?, image::ImageFormat::Jpeg);
    assert_eq!(body.as_ref(), std::fs::read(store.jpeg_path(&frame))?);
    for (header, value) in [
        ("if-none-match", format!("\"other\", W/{}", etag.to_str()?)),
        ("if-none-match", "*".into()),
        ("if-modified-since", modified.to_str()?.into()),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/webcam/tkecole.jpg")
                    .header(header, value)
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(response.headers()["etag"], etag);
        assert!(to_bytes(response.into_body(), 100).await?.is_empty());
    }
    let head = request(
        &app,
        "HEAD",
        "/webcam/tkecole.jpg",
        None,
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(head.status(), StatusCode::OK);
    assert_eq!(head.headers()["content-length"], length);
    assert!(to_bytes(head.into_body(), 100).await?.is_empty());
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/webcam/tkecole.jpg")
                .header("if-none-match", "\"different-image\"")
                .header("if-modified-since", &modified)
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&list_path)
                .header("if-none-match", &list_etag)
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);

    let next = store.save(bytes, "tkecole".into(), "png".into()).await?;
    for prefix in ["", "/powpow"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/webcam?image={}", next.filename))
                    .header("x-forwarded-prefix", prefix)
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers()["location"], format!("{prefix}/webcam"));
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
    let live =
        response_text(request(&app, "GET", "/webcam", None, serde_json::Value::Null).await).await?;
    assert!(live.contains("data-follow-latest=\"true\""));
    // Once a newer capture arrives, the old latest URL becomes an ordinary archive view.
    let response = request(
        &app,
        "GET",
        &format!("/webcam?image={}", frame.filename),
        None,
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let historical = response_text(response).await?;
    assert!(historical.contains("data-follow-latest=\"false\""));
    assert!(historical.contains(&format!("data-image=\"{}\"", frame.filename)));
    let response = request(
        &app,
        "GET",
        &format!("/webcam?date={}", next.date()),
        None,
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()["location"], "/webcam");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/webcam/tkecole.jpg")
                .header("if-none-match", &etag)
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert_ne!(response.headers()["etag"], etag);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&list_path)
                .header("if-none-match", &list_etag)
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let list: serde_json::Value = serde_json::from_str(&response_text(response).await?)?;
    assert_eq!(list["images"].as_array().unwrap().len(), 2);
    assert_eq!(list["images"][1]["filename"], next.filename);
    let response = request(
        &app,
        "GET",
        "/api/webcam/latest?camera=other",
        None,
        serde_json::Value::Null,
    )
    .await;
    let latest: serde_json::Value = serde_json::from_str(&response_text(response).await?)?;
    assert_eq!(latest["image"]["username"], "other");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/webcam?camera=tkecole")
                .header("x-forwarded-prefix", "/powpow")
                .body(Body::empty())?,
        )
        .await?;
    let body = response_text(response).await?;
    assert!(body.contains("id=\"webcam-slider\""));
    assert!(body.contains(&format!("data-list=\"/powpow{list_path}\"")));
    for label in ["Actuelle", "Il y a 1 h", "Il y a 3 h", "Il y a 5 h"] {
        assert!(body.contains(label));
    }
    assert!(!body.contains("webcam-times"));
    assert!(!body.contains("webcam-pagination"));
    for path in [
        "/webcam/missing.jpg",
        "/webcam/2026-11-12_missing_list.json",
        "/webcam/..%2Fsecret.jpg",
        "/webcam/images/missing.png_small.jpg",
    ] {
        assert_eq!(
            request(&app, "GET", path, None, serde_json::Value::Null)
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn webcam_weather_tracks_live_updates_and_historical_image_times(
    pool: PgPool,
) -> anyhow::Result<()> {
    use chrono::{SecondsFormat, TimeDelta, Utc};

    let mut state = state(pool.clone());
    state.config.meteo = meteo::Config::parse(|key| match key {
        "METEO_FRANCE_STATION_ID" => "38395004".into(),
        "METEO_FRANCE_TOKEN" => "test-token".into(),
        _ => String::new(),
    })?;
    let directory = tempfile::tempdir()?;
    let now = Utc::now();
    let image_time = now - TimeDelta::minutes(65);
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(4, 3).write_to(&mut bytes, image::ImageFormat::Png)?;
    let mut filenames = Vec::new();
    for time in [image_time, image_time - TimeDelta::hours(2)] {
        let filename = format!(
            "{}_station.png",
            time.to_rfc3339_opts(SecondsFormat::Nanos, true)
        );
        tokio::fs::write(directory.path().join(&filename), bytes.get_ref()).await?;
        filenames.push(filename);
    }
    state.webcam = webcam::Store::open(
        directory.path().into(),
        false,
        webcam::metadata::Settings::default(),
    )
    .await?;
    for (station, time, temperature) in [
        ("38395004", image_time - TimeDelta::minutes(6), 1.5),
        ("38395004", image_time + TimeDelta::seconds(1), 2.5),
        ("38395004", now - TimeDelta::minutes(6), 4.6),
        ("38395403", now - TimeDelta::minutes(1), 99.0),
        ("38395004", now + TimeDelta::hours(1), 98.0),
    ] {
        sqlx::query("INSERT INTO meteo (station_id, observed_at, reference_time, insert_time, latitude, longitude, temperature_c, rainfall_mm, raw_data)
            VALUES ($1, $2, $2, $2, 45.315, 5.884833, $3, 0, '{}')")
            .bind(station).bind(time).bind(temperature).execute(&pool).await?;
    }
    sqlx::query(
        "UPDATE meteo SET wind_speed_m_s = 1.5, wind_direction_deg = 180,
        gust_speed_m_s = 3, gust_direction_deg = 190, humidity_percent = 82,
        snow_depth_m = 0.12, pressure_pa = 90000 WHERE observed_at = $1",
    )
    .bind(now - TimeDelta::minutes(6))
    .execute(&pool)
    .await?;
    for (end, rainfall) in [
        (image_time - TimeDelta::minutes(6), 0.1),
        (now - TimeDelta::minutes(6), 0.2),
    ] {
        sqlx::query(
            "INSERT INTO meteo (station_id, observed_at, reference_time, insert_time,
                                latitude, longitude, rainfall_mm, raw_data)
             SELECT '38395004', sample, sample, sample, 45.315, 5.884833, $2, '{}'
             FROM generate_series($1 - interval '54 minutes', $1, interval '6 minutes') AS sample
             ON CONFLICT (station_id, observed_at) DO UPDATE SET rainfall_mm = EXCLUDED.rainfall_mm",
        )
        .bind(end)
        .bind(rainfall)
        .execute(&pool)
        .await?;
    }
    let store = state.webcam.clone();
    let app = build_router(state);
    let body =
        response_text(request(&app, "GET", "/webcam", None, serde_json::Value::Null).await).await?;
    for text in [
        "4,6 °C",
        "Pluie (1 h)",
        "2,0 mm",
        "5,4 km/h",
        "10,8 km/h",
        "180°",
        "190°",
        "82,0 %",
        "12,0 cm",
        "900,0 hPa",
        "Météo-France · relevé du",
    ] {
        assert!(body.contains(text), "{text}");
    }
    assert!(!body.contains("Point de rosée"));
    assert!(!body.contains("Pluie (6 min)"));
    assert!(!body.contains("99,0 °C"));
    assert!(!body.contains("98,0 °C"));
    assert!(!body.contains("UTC"));
    assert!(
        body.contains(
            &image_time
                .with_timezone(&chrono::Local)
                .format("%d/%m/%Y à %H:%M:%S")
                .to_string()
        )
    );
    assert!(
        body.contains(
            &(now - TimeDelta::minutes(6))
                .with_timezone(&chrono::Local)
                .format("%d/%m/%Y à %H:%M")
                .to_string()
        )
    );
    let response = request(
        &app,
        "GET",
        "/api/webcam/latest",
        None,
        serde_json::Value::Null,
    )
    .await;
    let before: serde_json::Value = serde_json::from_str(&response_text(response).await?)?;
    assert!(body.contains(before["weather_revision"].as_str().unwrap()));
    sqlx::query("UPDATE meteo SET temperature_c = 5.8, fetched_at = fetched_at + interval '1 second' WHERE observed_at = $1")
        .bind(now - TimeDelta::minutes(6)).execute(&pool).await?;
    let response = request(
        &app,
        "GET",
        "/api/webcam/latest",
        None,
        serde_json::Value::Null,
    )
    .await;
    let after: serde_json::Value = serde_json::from_str(&response_text(response).await?)?;
    assert_eq!(before["image"], after["image"]);
    assert_ne!(before["weather_revision"], after["weather_revision"]);
    let body =
        response_text(request(&app, "GET", "/webcam", None, serde_json::Value::Null).await).await?;
    assert!(body.contains("5,8 °C"));

    store
        .save(bytes.into_inner(), "station".into(), "png".into())
        .await?;
    let path = format!("/webcam?image={}", filenames[0]);
    let body =
        response_text(request(&app, "GET", &path, None, serde_json::Value::Null).await).await?;
    assert!(body.contains("data-follow-latest=\"false\""));
    assert!(body.contains("1,5 °C"));
    assert!(body.contains("Pluie (1 h)"));
    assert!(body.contains("1,0 mm"));
    assert!(!body.contains("2,5 °C"));
    assert!(!body.contains("5,8 °C"));
    let path = format!("/webcam?image={}", filenames[1]);
    let body =
        response_text(request(&app, "GET", &path, None, serde_json::Value::Null).await).await?;
    assert!(!body.contains("class=\"webcam-weather\""));
    sqlx::query("DELETE FROM meteo WHERE observed_at <= $1")
        .bind(image_time)
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO meteo (station_id, observed_at, reference_time, insert_time, latitude, longitude, temperature_c, raw_data)
        VALUES ('38395004', $1, $1, $1, 45.315, 5.884833, -10, '{}')")
        .bind(image_time - TimeDelta::minutes(31)).execute(&pool).await?;
    let path = format!("/webcam?image={}", filenames[0]);
    let body =
        response_text(request(&app, "GET", &path, None, serde_json::Value::Null).await).await?;
    assert!(!body.contains("class=\"webcam-weather\""));
    Ok(())
}

async fn subscription_url(
    app: &Router,
    person: uuid::Uuid,
    cookie: &str,
    regenerate: bool,
) -> anyhow::Result<String> {
    let response = request(
        app,
        "POST",
        &format!("/api/person/{person}/calendar-subscription"),
        Some(cookie),
        serde_json::json!({"regenerate": regenerate}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    let data: serde_json::Value = serde_json::from_str(&response_text(response).await?)?;
    Ok(data["subscription"]["url"].as_str().unwrap().to_string())
}

fn subscription_path(url: &str) -> String {
    reqwest::Url::parse(url)
        .unwrap()
        .path()
        .strip_prefix("/powpow")
        .unwrap()
        .to_string()
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn calendar_subscription_access_and_rotation(pool: PgPool) -> anyhow::Result<()> {
    let state = state(pool);
    let (owner, cookie) = member(&state, "family@example.org").await?;
    let (_, household) = member(&state, "family@example.org").await?;
    let (outsider, other_cookie) = member(&state, "other@example.org").await?;
    sqlx::query("UPDATE staff SET is_admin = true WHERE id = $1")
        .bind(outsider.id)
        .execute(&state.db)
        .await?;
    let app = build_router(state.clone());
    let endpoint = format!("/api/person/{}/calendar-subscription", owner.id);
    for method in ["GET", "POST"] {
        assert_eq!(
            request(&app, method, &endpoint, None, serde_json::json!({}))
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            request(
                &app,
                method,
                &endpoint,
                Some(&other_cookie),
                serde_json::json!({})
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
    }
    let empty = request(
        &app,
        "GET",
        &endpoint,
        Some(&cookie),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(empty.status(), StatusCode::OK);
    assert!(
        serde_json::from_str::<serde_json::Value>(&response_text(empty).await?)?["subscription"]
            .is_null()
    );
    assert!(
        database::calendar_feed::token(&state.db, owner.id)
            .await?
            .is_none()
    );

    let url = subscription_url(&app, owner.id, &cookie, false).await?;
    assert!(url.starts_with("https://station.example/powpow/calendar/feed/"));
    assert_eq!(
        subscription_url(&app, owner.id, &household, false).await?,
        url
    );
    let feed_path = subscription_path(&url);
    // Subscriptions work without a browser session, including on an unpaid
    // member's browser. Calendar tokens do not grant a normal API session.
    sqlx::query("DELETE FROM payments WHERE staff = $1")
        .bind(outsider.id)
        .execute(&state.db)
        .await?;
    for session in [None, Some(other_cookie.as_str())] {
        let response = request(&app, "GET", &feed_path, session, serde_json::Value::Null).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()["content-type"],
            "text/calendar; charset=utf-8"
        );
        assert!(!response.headers().contains_key("set-cookie"));
        let body = response_text(response).await?;
        assert!(body.contains("BEGIN:VCALENDAR\r\n"));
        assert!(!body.contains("BEGIN:VEVENT"));
    }
    let response = request(&app, "HEAD", &feed_path, None, serde_json::Value::Null).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_text(response).await?, "");
    let denied = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&endpoint)
                .header("cookie", &cookie)
                .header("origin", "https://attacker.example")
                .header("content-type", "application/json")
                .body(Body::from("{\"regenerate\":true}"))?,
        )
        .await?;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    assert_eq!(subscription_url(&app, owner.id, &cookie, false).await?, url);
    let new_url = subscription_url(&app, owner.id, &cookie, true).await?;
    assert_ne!(url, new_url);
    assert_eq!(
        request(&app, "GET", &feed_path, None, serde_json::Value::Null)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &app,
            "GET",
            &subscription_path(&new_url),
            None,
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request(
            &app,
            "GET",
            "/calendar/feed/invalid.ics",
            None,
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let token = database::calendar_feed::token(&state.db, owner.id)
        .await?
        .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/me")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn calendar_feed_tracks_attendance_with_stable_events(pool: PgPool) -> anyhow::Result<()> {
    let state = state(pool);
    let (owner, cookie) = member(&state, "owner@example.org").await?;
    let (other, _) = member(&state, "other@example.org").await?;
    let atelier: uuid::Uuid = sqlx::query_scalar("INSERT INTO ateliers (name, slug, needs_validation) VALUES ('Buvette', 'feed-test', false) RETURNING id").fetch_one(&state.db).await?;
    let mut needs = vec![];
    for (day, nightly) in [
        ("2026-03-29", false),
        ("2026-10-25", true),
        ("2026-12-31", false),
    ] {
        let id: uuid::Uuid = sqlx::query_scalar("INSERT INTO needs (day, atelier, quantity, nightly) VALUES ($1::text::date, $2, 2, $3) RETURNING id")
            .bind(day).bind(atelier).bind(nightly).fetch_one(&state.db).await?;
        needs.push(id);
    }
    // Historical attendance must survive missing current roles.
    database::upsert_presence(&state.db, needs[0], owner.id, true, false).await?;
    database::upsert_presence(&state.db, needs[1], owner.id, true, true).await?;
    database::upsert_presence(&state.db, needs[2], other.id, true, true).await?;
    let app = build_router(state.clone());
    let url = subscription_url(&app, owner.id, &cookie, false).await?;
    let path = subscription_path(&url);
    let response = request(&app, "GET", &path, None, serde_json::Value::Null).await;
    assert_eq!(response.status(), StatusCode::OK);
    let etag = response.headers()["etag"].clone();
    let body = response_text(response).await?;
    assert_eq!(body.matches("BEGIN:VEVENT").count(), 2);
    assert!(body.contains("SUMMARY:Buvette — matin\r\n"));
    assert!(body.contains("SUMMARY:Buvette — soirée et nuit\r\n"));
    assert!(body.contains("DTSTART;VALUE=DATE:20260329\r\nDTEND;VALUE=DATE:20260330\r\n"));
    assert!(body.contains("DTSTART;VALUE=DATE:20261025\r\nDTEND;VALUE=DATE:20261026\r\n"));
    assert!(!body.contains("20261231"));
    assert!(!body.contains("owner@example.org"));
    let uid = format!("{}-{}@powpow", owner.id, needs[0]);
    assert!(
        body.replace("\r\n ", "")
            .contains(&format!("UID:{uid}\r\n"))
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&path)
                .header("if-none-match", &etag)
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(response_text(response).await?, "");
    let repeat = request(&app, "GET", &path, None, serde_json::Value::Null).await;
    assert_eq!(response_text(repeat).await?, body);

    database::upsert_presence(&state.db, needs[0], owner.id, true, true).await?;
    let changed = request(&app, "GET", &path, None, serde_json::Value::Null).await;
    assert_ne!(changed.headers()["etag"], etag);
    let body = response_text(changed).await?.replace("\r\n ", "");
    assert!(body.contains(&format!("UID:{uid}\r\n")));
    assert!(body.contains("SUMMARY:Buvette — matin et après-midi\r\n"));
    assert_eq!(body.matches("BEGIN:VEVENT").count(), 2);
    sqlx::query("UPDATE ateliers SET name = 'Accueil' WHERE id = $1")
        .bind(atelier)
        .execute(&state.db)
        .await?;
    let renamed = request(&app, "GET", &path, None, serde_json::Value::Null).await;
    assert!(
        response_text(renamed)
            .await?
            .contains("SUMMARY:Accueil — matin et après-midi")
    );
    database::upsert_presence(&state.db, needs[0], owner.id, false, false).await?;
    let removed = response_text(request(&app, "GET", &path, None, serde_json::Value::Null).await)
        .await?
        .replace("\r\n ", "");
    assert!(!removed.contains(&uid));
    assert_eq!(removed.matches("BEGIN:VEVENT").count(), 1);

    database::upsert_presence(&state.db, needs[2], owner.id, false, true).await?;
    let year_end =
        response_text(request(&app, "GET", &path, None, serde_json::Value::Null).await).await?;
    assert!(year_end.contains("DTSTART;VALUE=DATE:20261231\r\nDTEND;VALUE=DATE:20270101\r\n"));
    let backup = database::backup_all_tables(&state.db).await?;
    let old_token = database::calendar_feed::token(&state.db, owner.id).await?;
    database::calendar_feed::create(&state.db, owner.id, true).await?;
    database::restore_from_sql(&state.db, &backup).await?;
    assert_eq!(
        database::calendar_feed::token(&state.db, owner.id).await?,
        old_token
    );
    let restored = request(&app, "GET", &path, None, serde_json::Value::Null).await;
    assert_eq!(response_text(restored).await?, year_end);
    Ok(())
}

#[test]
fn proxy_prefix_cannot_be_an_external_or_parent_relative_url() {
    let mut headers = HeaderMap::new();
    for prefix in ["//evil.example", "relative", "/../evil", "/a/./b"] {
        headers.insert("x-forwarded-prefix", prefix.parse().unwrap());
        assert_eq!(get_prefix(&headers), "");
    }
    headers.insert("x-forwarded-prefix", "/powpow/".parse().unwrap());
    assert_eq!(get_prefix(&headers), "/powpow");
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn staffing_thresholds_validate_settings_and_flow_into_calendars(
    pool: PgPool,
) -> anyhow::Result<()> {
    let state = state(pool);
    let (admin, cookie) = member(&state, "staffing-admin@example.org").await?;
    sqlx::query("UPDATE staff SET is_admin = true WHERE id = $1")
        .bind(admin.id)
        .execute(&state.db)
        .await?;
    let app = build_router(state.clone());
    let mut payload = serde_json::json!({
        "name": "Renfort", "slug": "renfort", "icon": "snowflake",
        "needs_validation": false, "default_nightly": false,
        "opening_day_typical_needed": 3, "opening_day_minimal_needed": 2
    });
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/ateliers",
            Some(&cookie),
            payload.clone()
        )
        .await
        .status(),
        StatusCode::OK
    );
    let atelier = database::get_atelier_by_slug(&state.db, "renfort")
        .await?
        .unwrap();
    let endpoint = format!("/api/ateliers/{}", atelier.id);
    for (comfortable, minimal) in [(1, 2), (-1, 0), (2, -1)] {
        payload["opening_day_typical_needed"] = comfortable.into();
        payload["opening_day_minimal_needed"] = minimal.into();
        for path in ["/api/ateliers", &endpoint] {
            assert_eq!(
                request(&app, "POST", path, Some(&cookie), payload.clone())
                    .await
                    .status(),
                StatusCode::BAD_REQUEST
            );
        }
    }
    for (comfortable, minimal) in [(0, 0), (3, 3), (3, 2)] {
        payload["opening_day_typical_needed"] = comfortable.into();
        payload["opening_day_minimal_needed"] = minimal.into();
        assert_eq!(
            request(&app, "POST", &endpoint, Some(&cookie), payload.clone())
                .await
                .status(),
            StatusCode::OK
        );
    }
    let settings = response_text(
        request(
            &app,
            "GET",
            "/settings",
            Some(&cookie),
            serde_json::Value::Null,
        )
        .await,
    )
    .await?;
    assert!(settings.contains("Besoin confortable / jour"));
    assert!(settings.contains("Besoin minimal / jour"));

    let day = chrono::Utc::now().date_naive() + chrono::Duration::days(7);
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/calendar/opening-day",
            Some(&cookie),
            serde_json::json!({"day": day})
        )
        .await
        .status(),
        StatusCode::OK
    );
    let need = database::get_needs_for_day(&state.db, day)
        .await?
        .into_iter()
        .find(|need| need.atelier == atelier.id)
        .unwrap();
    assert_eq!((need.quantity, need.minimal_quantity), (3, 2));

    let mut custom = serde_json::json!({"atelier_id": atelier.id, "day": day, "quantity": 3, "minimal_quantity": 2});
    for (comfortable, minimal) in [(1, 2), (-1, 0), (2, -1)] {
        custom["quantity"] = comfortable.into();
        custom["minimal_quantity"] = minimal.into();
        assert_eq!(
            request(
                &app,
                "POST",
                "/api/calendar/needs",
                Some(&cookie),
                custom.clone()
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
    }
    custom["quantity"] = 4.into();
    custom["minimal_quantity"] = 2.into();
    let response = request(&app, "POST", "/api/calendar/needs", Some(&cookie), custom).await;
    assert_eq!(response.status(), StatusCode::OK);
    let saved: serde_json::Value = serde_json::from_str(&response_text(response).await?)?;
    assert_eq!(saved["minimal_quantity"], 2);
    assert_eq!(saved["quantity"], 4);
    database::upsert_need(&state.db, atelier.id, day, 3, 2, false).await?;

    // First half is exactly minimal, second is exactly comfortable.
    for index in 0..3 {
        let (volunteer, _) = member(&state, &format!("staffing-{index}@example.org")).await?;
        database::add_role(&state.db, volunteer.id, atelier.id, true).await?;
        database::upsert_presence(&state.db, need.id, volunteer.id, index < 2, true).await?;
    }
    let overview = response_text(
        request(
            &app,
            "GET",
            "/calendar",
            Some(&cookie),
            serde_json::Value::Null,
        )
        .await,
    )
    .await?;
    assert!(overview.contains(&format!("cell-warning day-start\" data-day=\"{day}\" title=\"Besoin minimal : 2 ; confortable : 3\">2/3</td>")));
    assert!(overview.contains(&format!(
        "cell-ok\" data-day=\"{day}\" title=\"Besoin minimal : 2 ; confortable : 3\">3/3</td>"
    )));
    let detail = response_text(
        request(
            &app,
            "GET",
            "/calendar/renfort",
            Some(&cookie),
            serde_json::Value::Null,
        )
        .await,
    )
    .await?;
    assert!(detail.contains("has-text-warning-dark\">matin 2/3</span>"));
    assert!(detail.contains("has-text-success\">après-midi 3/3</span>"));
    assert!(detail.contains("data-minimal-quantity=\"2\""));

    let backup = database::backup_all_tables(&state.db).await?;
    database::upsert_need(&state.db, atelier.id, day, 8, 7, true).await?;
    database::restore_from_sql(&state.db, &backup).await?;
    let restored = database::get_need_by_id(&state.db, need.id).await?.unwrap();
    assert_eq!((restored.quantity, restored.minimal_quantity), (3, 2));
    let restored = database::get_atelier_by_id(&state.db, atelier.id)
        .await?
        .unwrap();
    assert_eq!(
        (
            restored.opening_day_typical_needed,
            restored.opening_day_minimal_needed
        ),
        (3, 2)
    );
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn whatsapp_group_settings_require_admin_and_validate_links(
    pool: PgPool,
) -> anyhow::Result<()> {
    let state = state(pool);
    let (admin, admin_cookie) = member(&state, "admin@example.org").await?;
    let (_, volunteer_cookie) = member(&state, "volunteer@example.org").await?;
    sqlx::query("UPDATE staff SET is_admin = true WHERE id = $1")
        .bind(admin.id)
        .execute(&state.db)
        .await?;
    let app = build_router(state.clone());
    let mut payload = serde_json::json!({
        "name": "Buvette", "slug": "buvette", "icon": "mug-hot",
        "needs_validation": false, "default_nightly": false, "opening_day_typical_needed": 2,
        "opening_day_minimal_needed": 2,
        "whatsapp_group_url": " https://chat.whatsapp.com/AbCdEfGhIjKlMnOpQrStUv?mode=ems_copy_t "
    });
    for (cookie, status) in [
        (None, StatusCode::UNAUTHORIZED),
        (Some(volunteer_cookie.as_str()), StatusCode::FORBIDDEN),
    ] {
        assert_eq!(
            request(&app, "POST", "/api/ateliers", cookie, payload.clone())
                .await
                .status(),
            status
        );
    }
    let response = request(
        &app,
        "POST",
        "/api/ateliers",
        Some(&admin_cookie),
        payload.clone(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let atelier = database::get_atelier_by_slug(&state.db, "buvette")
        .await?
        .unwrap();
    assert_eq!(
        atelier.whatsapp_group_url.as_deref(),
        Some("https://chat.whatsapp.com/AbCdEfGhIjKlMnOpQrStUv")
    );
    let endpoint = format!("/api/ateliers/{}", atelier.id);
    assert_eq!(
        request(
            &app,
            "POST",
            &endpoint,
            Some(&volunteer_cookie),
            payload.clone()
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );

    payload["whatsapp_group_url"] = serde_json::json!("javascript:alert(1)");
    for endpoint in ["/api/ateliers", endpoint.as_str()] {
        let response = request(&app, "POST", endpoint, Some(&admin_cookie), payload.clone()).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(
            response_text(response)
                .await?
                .contains("lien d'invitation WhatsApp")
        );
    }
    assert_eq!(
        database::get_atelier_by_id(&state.db, atelier.id)
            .await?
            .unwrap()
            .whatsapp_group_url,
        atelier.whatsapp_group_url
    );

    payload["whatsapp_group_url"] =
        serde_json::json!("https://chat.whatsapp.com/ZyXwVuTsRqPoNmLkJiHgFe");
    assert_eq!(
        request(
            &app,
            "POST",
            &endpoint,
            Some(&admin_cookie),
            payload.clone()
        )
        .await
        .status(),
        StatusCode::OK
    );
    payload["default_nightly"] = serde_json::json!(true);
    assert_eq!(
        request(
            &app,
            "POST",
            &endpoint,
            Some(&admin_cookie),
            payload.clone()
        )
        .await
        .status(),
        StatusCode::OK
    );
    let updated = database::get_atelier_by_id(&state.db, atelier.id)
        .await?
        .unwrap();
    assert!(updated.default_nightly);
    assert_eq!(
        updated.whatsapp_group_url.as_deref(),
        Some("https://chat.whatsapp.com/ZyXwVuTsRqPoNmLkJiHgFe")
    );
    let backup = database::backup_all_tables(&state.db).await?;
    payload["whatsapp_group_url"] = serde_json::json!("  ");
    assert_eq!(
        request(&app, "POST", &endpoint, Some(&admin_cookie), payload)
            .await
            .status(),
        StatusCode::OK
    );
    assert!(
        database::get_atelier_by_id(&state.db, atelier.id)
            .await?
            .unwrap()
            .whatsapp_group_url
            .is_none()
    );
    database::restore_from_sql(&state.db, &backup).await?;
    assert_eq!(
        database::get_atelier_by_id(&state.db, atelier.id)
            .await?
            .unwrap()
            .whatsapp_group_url,
        updated.whatsapp_group_url
    );
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn whatsapp_groups_follow_profile_atelier_subscriptions(pool: PgPool) -> anyhow::Result<()> {
    let state = state(pool);
    let (owner, owner_cookie) = member(&state, "family@example.org").await?;
    let (_, household_cookie) = member(&state, "family@example.org").await?;
    let (_, outsider_cookie) = member(&state, "other@example.org").await?;
    let (admin, admin_cookie) = member(&state, "admin@example.org").await?;
    sqlx::query("UPDATE staff SET is_admin = true WHERE id = $1")
        .bind(admin.id)
        .execute(&state.db)
        .await?;
    let subscribed = database::create_atelier(
        &state.db,
        "Buvette & accueil",
        "buvette",
        "mug-hot",
        true,
        false,
        0,
        0,
        Some("https://chat.whatsapp.com/AbCdEfGhIjKlMnOpQrStUv"),
    )
    .await?;
    database::create_atelier(
        &state.db,
        "Pistes",
        "pistes",
        "snowflake",
        false,
        false,
        0,
        0,
        Some("https://chat.whatsapp.com/ZyXwVuTsRqPoNmLkJiHgFe"),
    )
    .await?;
    let no_group = database::create_atelier(
        &state.db,
        "Location",
        "location",
        "person-skiing",
        false,
        false,
        0,
        0,
        None,
    )
    .await?;
    let shared_group = database::create_atelier(
        &state.db,
        "Renfort buvette",
        "renfort-buvette",
        "mug-hot",
        false,
        false,
        0,
        0,
        subscribed.whatsapp_group_url.as_deref(),
    )
    .await?;
    // Interest in an atelier is enough, including roles awaiting validation.
    database::add_role(&state.db, owner.id, subscribed.id, false).await?;
    database::add_role(&state.db, owner.id, no_group.id, true).await?;
    database::add_role(&state.db, owner.id, shared_group.id, true).await?;
    assert!(
        database::get_pending_validations(&state.db, None)
            .await?
            .iter()
            .any(|(_, a)| a.whatsapp_group_url == subscribed.whatsapp_group_url)
    );
    let app = build_router(state);
    let profile = format!("/person/{}", owner.id);
    for cookie in [&owner_cookie, &household_cookie, &admin_cookie] {
        let response = request(&app, "GET", &profile, Some(cookie), serde_json::Value::Null).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_text(response).await?;
        assert!(body.contains("id=\"whatsapp-groups\""));
        assert_eq!(
            body.matches("href=\"https://chat.whatsapp.com/AbCdEfGhIjKlMnOpQrStUv\"")
                .count(),
            1
        );
        assert!(body.contains("Buvette &amp; accueil / Renfort buvette"));
        assert!(!body.contains("ZyXwVuTsRqPoNmLkJiHgFe"));
    }
    let other_view = response_text(
        request(
            &app,
            "GET",
            &profile,
            Some(&outsider_cookie),
            serde_json::Value::Null,
        )
        .await,
    )
    .await?;
    assert!(!other_view.contains("chat.whatsapp.com"));
    let response = request(
        &app,
        "POST",
        &format!("/api/person/{}/role", owner.id),
        Some(&owner_cookie),
        serde_json::json!({"atelier_id": subscribed.id, "add": false}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let left = response_text(
        request(
            &app,
            "GET",
            &profile,
            Some(&owner_cookie),
            serde_json::Value::Null,
        )
        .await,
    )
    .await?;
    assert!(left.contains("href=\"https://chat.whatsapp.com/AbCdEfGhIjKlMnOpQrStUv\""));
    assert!(!left.contains("Buvette &amp; accueil / Renfort buvette"));
    let response = request(
        &app,
        "POST",
        &format!("/api/person/{}/role", owner.id),
        Some(&owner_cookie),
        serde_json::json!({"atelier_id": shared_group.id, "add": false}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let left = response_text(
        request(
            &app,
            "GET",
            &profile,
            Some(&owner_cookie),
            serde_json::Value::Null,
        )
        .await,
    )
    .await?;
    assert!(!left.contains("id=\"whatsapp-groups\""));
    assert!(!left.contains("chat.whatsapp.com"));
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn atelier_names_are_escaped_in_reminders_and_calendar_data(
    pool: PgPool,
) -> anyhow::Result<()> {
    let state = state(pool);
    let (chief, cookie) = member(&state, "chief@example.org").await?;
    let (pending, _) = member(&state, "pending@example.org").await?;
    let attack = "</script><script>window.reviewXss=1</script>";
    let atelier: uuid::Uuid = sqlx::query_scalar("INSERT INTO ateliers (name, slug, needs_validation) VALUES ($1, 'test-escaping', true) RETURNING id")
        .bind(attack).fetch_one(&state.db).await?;
    database::add_role(&state.db, chief.id, atelier, true).await?;
    database::update_role(&state.db, chief.id, atelier, None, Some(true)).await?;
    database::add_role(&state.db, pending.id, atelier, false).await?;
    sqlx::query("INSERT INTO needs (day, atelier, quantity, nightly) VALUES (current_date + 1, $1, 1, false)")
        .bind(atelier).execute(&state.db).await?;
    let app = build_router(state);
    for path in [format!("/person/{}", chief.id), "/calendar".into()] {
        let response = request(&app, "GET", &path, Some(&cookie), serde_json::Value::Null).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(to_bytes(response.into_body(), 1_000_000).await?.to_vec())?;
        assert!(body.contains("reviewXss"));
        assert!(!body.contains(attack));
    }
    Ok(())
}

fn state(db: PgPool) -> AppState {
    let config = AppConfig {
        meteo: None,
        webcam: webcam::config::WebcamConfig {
            coordinates: None,
            title: "Vue sur les pistes".into(),
            directory: "/tmp/opencode/powpow-test-webcam".into(),
            ftps: None,
            direct: None,
        },
        public_base_url: "https://station.example/powpow/".parse().unwrap(),
        listen_address: String::new(),
        database_url: String::new(),
        helloasso_client_id: String::new(),
        helloasso_client_secret: String::new(),
        helloasso_association_slug: String::new(),
        mailchimp_api_key: String::new(),
        mailchimp_server_prefix: String::new(),
        mailchimp_list_id: String::new(),
        mailchimp_from_name: String::new(),
        mailchimp_from_email: String::new(),
        smtp_host: String::new(),
        smtp_port: 587,
        smtp_user: String::new(),
        smtp_password: String::new(),
        smtp_from: "station@example.org".into(),
        gmail_client_id: String::new(),
        gmail_client_secret: String::new(),
        gmail_redirect_uri: String::new(),
        gmail_access_token: String::new(),
        gmail_refresh_token: String::new(),
        gmail_from: String::new(),
        mail_method: "smtp".into(),
        mail_destination_override: String::new(),
        cookie_secret: String::new(),
        sync_token: String::new(),
        backup_token: String::new(),
        huggingface_token: String::new(),
        rss_news_feed: String::new(),
        entity_name: "Test".into(),
        entity_email: "contact@example.org".into(),
    };
    AppState {
        forecast: forecast::Cache::default(),
        webcam: webcam::Store::new(
            config.webcam.directory.clone(),
            webcam::metadata::Settings::new(
                config.webcam.coordinates,
                &config.entity_name,
                &config.entity_email,
            ),
        ),
        db,
        config,
        cookie_key: Key::generate(),
        gmail_client: None,
        helloasso_client: HelloAssoClient::new(String::new(), String::new(), String::new()),
        mailchimp_client: MailchimpClient::new(
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ),
        sync_in_progress: std::sync::Arc::default(),
        mailchimp_sync_in_progress: std::sync::Arc::default(),
        mailchimp_sync_progress: std::sync::Arc::default(),
        mailchimp_sync_pending: std::sync::Arc::default(),
        mail_limits: std::sync::Arc::default(),
    }
}

async fn member(state: &AppState, email: &str) -> anyhow::Result<(models::Staff, String)> {
    let person: models::Staff = sqlx::query_as("INSERT INTO staff (first_name, last_name, email, comment) VALUES ('Test', 'Member', $1, '') RETURNING *")
        .bind(email).fetch_one(&state.db).await?;
    let cash: uuid::Uuid = sqlx::query_scalar("INSERT INTO cash (first_name, last_name, email, date, amount, is_membership) VALUES ('Test', 'Member', $1, current_date, 20, true) RETURNING id")
        .bind(email).fetch_one(&state.db).await?;
    sqlx::query("INSERT INTO payments (season, staff, cash_id) VALUES ($1, $2, $3)")
        .bind(get_current_season())
        .bind(person.id)
        .bind(cash)
        .execute(&state.db)
        .await?;
    let session = database::create_session(&state.db, person.id, None, None).await?;
    let jar = SignedCookieJar::new(state.cookie_key.clone()).add(
        axum_extra::extract::cookie::Cookie::new(auth::SESSION_COOKIE, session.to_string()),
    );
    let cookie = jar.into_response().headers()["set-cookie"]
        .to_str()?
        .to_string();
    Ok((person, cookie))
}

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    body: serde_json::Value,
) -> Response {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    app.clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn profile_privacy_and_privileged_identity_edits(pool: PgPool) -> anyhow::Result<()> {
    let state = state(pool);
    let (owner, owner_cookie) = member(&state, "private@example.org").await?;
    let (outsider, outsider_cookie) = member(&state, "outsider@example.org").await?;
    let (_, household_cookie) = member(&state, "private@example.org").await?;
    let qualification: i32 =
        sqlx::query_scalar("INSERT INTO qualifications (name) VALUES ('First aid') RETURNING id")
            .fetch_one(&state.db)
            .await?;
    let record = database::add_staff_qualif(
        &state.db,
        owner.id,
        qualification,
        chrono::Utc::now().date_naive(),
    )
    .await?;
    database::set_training_proof(&state.db, record.id, b"private proof", "image/png").await?;
    let app = build_router(state.clone());
    let profile = format!("/person/{}", owner.id);
    let response = request(
        &app,
        "GET",
        &profile,
        Some(&outsider_cookie),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(to_bytes(response.into_body(), 1_000_000).await?.to_vec())?;
    assert!(!body.contains("private@example.org"));
    assert!(!body.contains("Historique des cotisations"));
    let proof = format!("/staff-qualif/{}/proof", record.id);
    assert_eq!(
        request(
            &app,
            "GET",
            &proof,
            Some(&outsider_cookie),
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    for cookie in [&owner_cookie, &household_cookie] {
        assert_eq!(
            request(&app, "GET", &proof, Some(cookie), serde_json::Value::Null)
                .await
                .status(),
            StatusCode::OK
        );
    }
    sqlx::query("UPDATE staff SET is_god = true, is_admin = true WHERE id = $1")
        .bind(owner.id)
        .execute(&state.db)
        .await?;
    sqlx::query("UPDATE staff SET is_admin = true WHERE id = $1")
        .bind(outsider.id)
        .execute(&state.db)
        .await?;
    let path = format!("/api/person/{}/contact", owner.id);
    let response = request(
        &app,
        "POST",
        &path,
        Some(&outsider_cookie),
        serde_json::json!({"email":"attacker@example.org"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        database::get_staff_by_id(&state.db, owner.id)
            .await?
            .unwrap()
            .email,
        owner.email
    );
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn http_rejects_invalid_qualification_and_reports_mail_failure(
    pool: PgPool,
) -> anyhow::Result<()> {
    let state = state(pool);
    let (person, cookie) = member(&state, "member@example.org").await?;
    let app = build_router(state);
    assert_eq!(request(&app, "POST", "/api/my/staff-qualif", Some(&cookie), serde_json::json!({"staff_id":person.id,"qualification_id":1,"obtained_date":"+262142-12-31"})).await.status(), StatusCode::BAD_REQUEST);
    let form = serde_json::json!({"name":"Visitor", "email":"visitor@example.org", "subject":"Test", "message":"Test"});
    assert_eq!(
        request(&app, "POST", "/contact", None, form.clone())
            .await
            .status(),
        StatusCode::BAD_GATEWAY
    );
    for _ in 0..9 {
        request(&app, "POST", "/contact", None, form.clone()).await;
    }
    assert_eq!(
        request(&app, "POST", "/contact", None, form).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    Ok(())
}
