use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use tracing::error;
use uuid::Uuid;

use crate::{
    AppState,
    auth::{RequireStaff, can_manage_profile},
    calendar_feed, database,
};

fn private(response: impl IntoResponse) -> Response {
    let mut response = response.into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

fn failed(error: &anyhow::Error) -> Response {
    error!("Calendar subscription failed: {error}");
    private((
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": "Impossible de charger l'abonnement au calendrier."})),
    ))
}

pub async fn subscription(
    RequireStaff(viewer): RequireStaff,
    State(state): State<AppState>,
    Path(staff): Path<Uuid>,
) -> Response {
    match can_manage_profile(&state, &viewer, staff).await {
        Ok(true) => {}
        Ok(false) => return private(StatusCode::FORBIDDEN),
        Err(response) => return private(response),
    }
    let result = async {
        let token = database::calendar_feed::token(&state.db, staff).await?;
        token
            .map(|token| calendar_feed::links(&state.config.public_base_url, &token))
            .transpose()
    }
    .await;
    match result {
        Ok(links) => private(Json(serde_json::json!({"subscription": links}))),
        Err(e) => failed(&e),
    }
}

#[derive(Deserialize)]
pub struct SubscriptionRequest {
    #[serde(default)]
    regenerate: bool,
}

pub async fn create_subscription(
    RequireStaff(viewer): RequireStaff,
    State(state): State<AppState>,
    Path(staff): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<SubscriptionRequest>,
) -> Response {
    if !super::auth::confirmation_origin_allowed(&headers, &state.config.public_base_url) {
        return private(StatusCode::FORBIDDEN);
    }
    match can_manage_profile(&state, &viewer, staff).await {
        Ok(true) => {}
        Ok(false) => return private(StatusCode::FORBIDDEN),
        Err(response) => return private(response),
    }
    let result = async {
        let token = database::calendar_feed::create(&state.db, staff, request.regenerate).await?;
        calendar_feed::links(&state.config.public_base_url, &token)
    }
    .await;
    match result {
        Ok(links) => private(Json(serde_json::json!({"subscription": links}))),
        Err(e) => failed(&e),
    }
}

/// Calendar applications authenticate with their dedicated read-only URL.
pub async fn feed(
    State(state): State<AppState>,
    Path(filename): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(token) = filename
        .strip_suffix(".ics")
        .filter(|token| calendar_feed::valid_token(token))
    else {
        return private(StatusCode::NOT_FOUND);
    };
    let (staff, entries) = match database::calendar_feed::load(&state.db, token).await {
        Ok(Some(calendar)) => calendar,
        Ok(None) => return private(StatusCode::NOT_FOUND),
        Err(e) => return failed(&e),
    };
    let calendar = match calendar_feed::render(
        &staff,
        &entries,
        &state.config.public_base_url,
        &state.config.entity_name,
    ) {
        Ok(calendar) => calendar,
        Err(e) => return failed(&e),
    };
    let etag = format!(
        "\"{}\"",
        URL_SAFE_NO_PAD
            .encode(ring::digest::digest(&ring::digest::SHA256, calendar.as_bytes()).as_ref())
    );
    let unchanged = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value.split(',').any(|candidate| {
                let candidate = candidate.trim();
                candidate == "*" || candidate.trim_start_matches("W/") == etag
            })
        });
    let mut response = private(if unchanged {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        calendar.into_response()
    });
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/calendar; charset=utf-8"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("inline; filename=\"planning.ics\""),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(&etag).expect("base64 ETag is a valid header"),
    );
    response
}
