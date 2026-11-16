use axum::{
    body::Body,
    extract::{Query, State},
    http::{HeaderMap, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{Local, NaiveDate, NaiveTime, Utc};
use tokio_util::io::ReaderStream;

use crate::{
    AppState,
    auth::RequireStaff,
    get_prefix, templates,
    webcam::{
        config::valid_username,
        export::{DEFAULT_SPEED, Interval, MAX_SPEED, Plan},
    },
};

#[derive(serde::Deserialize)]
pub struct PageQuery {
    date: Option<NaiveDate>,
    camera: Option<String>,
    speed: Option<u32>,
}

pub async fn page(
    _staff: RequireStaff,
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PageQuery>,
) -> Response {
    let today = Local::now().date_naive();
    let speed = query.speed.unwrap_or(DEFAULT_SPEED);
    if query.date.is_some_and(|date| date > today)
        || !(1..=MAX_SPEED).contains(&speed)
        || query
            .camera
            .as_deref()
            .is_some_and(|camera| !valid_username(camera))
    {
        return (StatusCode::BAD_REQUEST, "Date, caméra ou vitesse invalide.").into_response();
    }
    let gallery = state
        .webcam
        .gallery(query.date, None, query.camera.as_deref());
    let sun = state
        .webcam_sun
        .get(gallery.date, state.config.webcam.coordinates)
        .await;
    let defaults = sun.map(|sun| sun.defaults(Utc::now()));
    let warning = if sun.is_none() {
        Some("Les horaires du soleil sont indisponibles. Saisissez les heures de début et de fin.")
    } else if defaults.is_some_and(|(start, end)| end <= start) {
        Some(
            "Le soleil ne s’est pas encore levé. Choisissez une autre plage horaire ou une autre journée.",
        )
    } else {
        None
    };
    (
        [(header::CACHE_CONTROL, "private, no-store")],
        templates::webcam_export_page(
            &get_prefix(&headers),
            &gallery,
            defaults.map(|times| times.0),
            defaults.map(|times| times.1),
            speed,
            warning,
        ),
    )
        .into_response()
}

#[derive(serde::Deserialize)]
pub struct VideoQuery {
    date: NaiveDate,
    camera: String,
    start: NaiveTime,
    end: NaiveTime,
    speed: u32,
}

pub async fn video(
    _staff: RequireStaff,
    State(state): State<AppState>,
    method: Method,
    Query(query): Query<VideoQuery>,
) -> Response {
    if method == Method::HEAD {
        return (
            [
                (header::CONTENT_TYPE, "video/mp4"),
                (header::CACHE_CONTROL, "private, no-store"),
            ],
            (),
        )
            .into_response();
    }
    if !valid_username(&query.camera) {
        return (StatusCode::BAD_REQUEST, "Caméra invalide.").into_response();
    }
    let interval = match Interval::new(query.date, query.start, query.end, query.speed, Utc::now())
    {
        Ok(interval) => interval,
        Err(error) => return (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
    };
    let gallery = state
        .webcam
        .gallery(Some(query.date), None, Some(&query.camera));
    let plan = match Plan::new(gallery.images, &interval) {
        Ok(plan) => plan,
        Err(error) => return (StatusCode::NOT_FOUND, error.to_string()).into_response(),
    };
    match state.webcam.export_video(plan).await {
        Ok(file) => {
            let filename = format!(
                "webcam-{}-{}-{}-{}-{}x.mp4",
                query.camera,
                query.date,
                query.start.format("%H%M%S"),
                query.end.format("%H%M%S"),
                query.speed
            );
            let mut response = (
                [
                    (header::CONTENT_TYPE, "video/mp4"),
                    (header::CACHE_CONTROL, "private, no-store"),
                    (
                        header::CONTENT_DISPOSITION,
                        &format!("attachment; filename=\"{filename}\""),
                    ),
                ],
                Body::from_stream(ReaderStream::new(tokio::fs::File::from_std(file))),
            )
                .into_response();
            response.headers_mut().insert(
                header::X_CONTENT_TYPE_OPTIONS,
                "nosniff".parse().expect("static header"),
            );
            response
        }
        Err(error)
            if error
                .downcast_ref::<tokio::sync::TryAcquireError>()
                .is_some() =>
        {
            (
                StatusCode::TOO_MANY_REQUESTS,
                [(header::RETRY_AFTER, "10")],
                error.to_string(),
            )
                .into_response()
        }
        Err(error) => {
            tracing::error!(%error, "Cannot encode webcam export");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "La vidéo n’a pas pu être créée. Réessayez dans quelques instants.",
            )
                .into_response()
        }
    }
}
