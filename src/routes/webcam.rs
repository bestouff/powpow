use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use std::path::Path as FilePath;
use tokio_util::io::ReaderStream;

use crate::{AppState, database, get_prefix, models::MeteoReading, templates, webcam};

async fn weather(state: &AppState, image_time: Option<DateTime<Utc>>) -> Option<MeteoReading> {
    let config = state.config.meteo.as_ref()?;
    let at = image_time.unwrap_or_else(Utc::now);
    match database::meteo::at_or_before(&state.db, &config.station_id, at).await {
        Ok(reading) => reading.filter(|reading| {
            image_time.is_none() || at - reading.observed_at <= TimeDelta::minutes(30)
        }),
        Err(error) => {
            tracing::warn!(%error, "Cannot load webcam weather");
            None
        }
    }
}

#[derive(Default, serde::Deserialize)]
pub struct GalleryQuery {
    date: Option<NaiveDate>,
    image: Option<String>,
    camera: Option<String>,
}

#[derive(Default, serde::Deserialize)]
pub struct CameraQuery {
    camera: Option<String>,
}

pub async fn page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<GalleryQuery>,
) -> Response {
    if query
        .camera
        .as_deref()
        .is_some_and(|camera| !webcam::config::valid_username(camera))
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    if query
        .image
        .as_ref()
        .is_some_and(|name| state.webcam.get(name).is_none())
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let gallery = state
        .webcam
        .gallery(query.date, query.image.as_deref(), query.camera.as_deref());
    let follow_latest = query.date.is_none() && query.image.is_none();
    let reading = if follow_latest {
        weather(&state, None).await
    } else if let Some(frame) = &gallery.selected {
        weather(&state, Some(frame.received_at)).await
    } else {
        None
    };
    let title = state
        .config
        .webcam
        .direct
        .as_ref()
        .filter(|direct| {
            gallery
                .selected
                .as_ref()
                .is_none_or(|frame| frame.username == direct.name)
        })
        .and_then(|direct| direct.title.as_deref())
        .unwrap_or(&state.config.webcam.title);
    (
        [(header::CACHE_CONTROL, "no-store")],
        templates::webcam_page(
            &get_prefix(&headers),
            title,
            &gallery,
            follow_latest,
            reading.as_ref(),
        ),
    )
        .into_response()
}

pub async fn latest(State(state): State<AppState>, Query(query): Query<CameraQuery>) -> Response {
    if query
        .camera
        .as_deref()
        .is_some_and(|camera| !webcam::config::valid_username(camera))
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let reading = weather(&state, None).await;
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(serde_json::json!({
            "image": query.camera.as_deref().map_or_else(|| state.webcam.latest(), |camera| state.webcam.latest_for(camera)),
            "weather_revision": reading.as_ref().map(MeteoReading::revision).unwrap_or_default(),
        })),
    ).into_response()
}

const REVALIDATE: &str = "public, max-age=60, must-revalidate";

/// Stable per-camera JPEG URLs and precomputed local-day thumbnail lists.
pub async fn asset(
    State(state): State<AppState>,
    Path(filename): Path<String>,
    headers: HeaderMap,
) -> Response {
    if let Some(path) = state.webcam.list_path(&filename) {
        return serve_file(&path, "application/json", REVALIDATE, None, &headers).await;
    }
    let Some(camera) = filename
        .strip_suffix(".jpg")
        .filter(|camera| webcam::config::valid_username(camera))
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(frame) = state.webcam.latest_for(camera) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    serve_file(
        &state.webcam.jpeg_path(&frame),
        "image/jpeg",
        REVALIDATE,
        Some(&frame.jpeg()),
        &headers,
    )
    .await
}

pub async fn image(
    State(state): State<AppState>,
    Path(filename): Path<String>,
    headers: HeaderMap,
) -> Response {
    let (path, mime) = if let Some(frame) = state.webcam.get(&filename) {
        (state.webcam.path(&frame), frame.mime_type())
    } else if let Some(path) = state.webcam.thumbnail(&filename) {
        (path, "image/jpeg")
    } else {
        return StatusCode::NOT_FOUND.into_response();
    };
    serve_file(
        &path,
        mime,
        "public, max-age=31536000, immutable",
        Some(&filename),
        &headers,
    )
    .await
}

async fn serve_file(
    path: &FilePath,
    mime: &str,
    cache_control: &str,
    version: Option<&str>,
    headers: &HeaderMap,
) -> Response {
    let file = match webcam::open_image(path).await {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return StatusCode::NOT_FOUND.into_response();
        }
        Err(error) => {
            tracing::error!(%error, path = %path.display(), "Cannot read webcam file");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let metadata = match file.metadata().await {
        Ok(metadata) => metadata,
        Err(error) => {
            tracing::error!(%error, "Cannot read webcam file metadata");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let modified: DateTime<Utc> = match metadata.modified() {
        Ok(modified) => modified.into(),
        Err(error) => {
            tracing::error!(%error, "Cannot read webcam file modification time");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let etag = version.map_or_else(
        || {
            format!(
                "\"{}-{}-{}\"",
                metadata.len(),
                modified.timestamp(),
                modified.timestamp_subsec_nanos()
            )
        },
        |version| format!("\"{version}\""),
    );
    let last_modified = modified.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
    let cache_headers = [
        (header::CACHE_CONTROL, cache_control),
        (header::ETAG, etag.as_str()),
        (header::LAST_MODIFIED, last_modified.as_str()),
        (header::CONTENT_TYPE, mime),
    ];
    let unchanged = if headers.contains_key(header::IF_NONE_MATCH) {
        headers
            .get_all(header::IF_NONE_MATCH)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .any(|value| {
                value
                    .split(',')
                    .any(|tag| tag.trim().trim_start_matches("W/") == etag || tag.trim() == "*")
            })
    } else {
        headers
            .get(header::IF_MODIFIED_SINCE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| DateTime::parse_from_rfc2822(value).ok())
            .is_some_and(|since| modified.timestamp() <= since.timestamp())
    };
    if unchanged {
        return (StatusCode::NOT_MODIFIED, cache_headers).into_response();
    }
    (
        cache_headers,
        [(header::CONTENT_LENGTH, metadata.len().to_string())],
        Body::from_stream(ReaderStream::new(file)),
    )
        .into_response()
}
