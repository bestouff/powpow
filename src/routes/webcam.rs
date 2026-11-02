use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::NaiveDate;
use tokio_util::io::ReaderStream;

use crate::{AppState, get_prefix, templates, webcam};

#[derive(Default, serde::Deserialize)]
pub struct GalleryQuery {
    date: Option<NaiveDate>,
    image: Option<String>,
    #[serde(default)]
    page: usize,
}

pub async fn page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<GalleryQuery>,
) -> Response {
    if query
        .image
        .as_ref()
        .is_some_and(|name| state.webcam.get(name).is_none())
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let gallery = state
        .webcam
        .gallery(query.date, query.image.as_deref(), query.page);
    let follow_latest = query.date.is_none() && query.image.is_none() && query.page == 0;
    (
        [(header::CACHE_CONTROL, "no-store")],
        templates::webcam_page(
            &get_prefix(&headers),
            &state.config.webcam.title,
            &gallery,
            follow_latest,
        ),
    )
        .into_response()
}

pub async fn latest(State(state): State<AppState>) -> impl IntoResponse {
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(serde_json::json!({ "image": state.webcam.latest() })),
    )
}

pub async fn image(
    State(state): State<AppState>,
    Path(filename): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(frame) = state.webcam.get(&filename) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let file = match webcam::open_image(&state.webcam.path(&frame)).await {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return StatusCode::NOT_FOUND.into_response();
        }
        Err(error) => {
            tracing::error!(%error, %filename, "Cannot read webcam image");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let etag = format!("\"{filename}\"");
    let cache_headers = [
        (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        (header::ETAG, etag.as_str()),
        (header::CONTENT_TYPE, frame.mime_type()),
    ];
    if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|tag| tag.trim().trim_start_matches("W/") == etag || tag.trim() == "*")
        })
    {
        return (StatusCode::NOT_MODIFIED, cache_headers).into_response();
    }
    (cache_headers, Body::from_stream(ReaderStream::new(file))).into_response()
}
