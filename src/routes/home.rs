use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use axum_extra::extract::cookie::SignedCookieJar;

use crate::{
    AppState, database, dicton, get_current_season, get_prefix, models::ContentMap, templates,
};

/// Resolve the caller from the session cookie, if any.
async fn resolve_caller(jar: &SignedCookieJar, state: &AppState) -> Option<crate::models::Staff> {
    let id = jar
        .get("aghil_session")?
        .value()
        .parse::<uuid::Uuid>()
        .ok()?;
    database::get_staff_by_id(&state.db, id)
        .await
        .ok()
        .flatten()
}

pub async fn index(
    headers: HeaderMap,
    jar: SignedCookieJar,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    let current_season = get_current_season();
    let logged_in = resolve_caller(&jar, &state).await.is_some();

    // Public frontpage data
    let equipments = database::get_all_equipments(&state.db)
        .await
        .unwrap_or_default();
    let station_open = database::is_station_open_today(&state.db)
        .await
        .unwrap_or(false);
    let photo_ids = database::get_all_photo_ids(&state.db)
        .await
        .unwrap_or_default();
    let staff_photo_ids = database::get_staff_photo_ids(&state.db)
        .await
        .unwrap_or_default();
    let contents = ContentMap::new(
        database::get_all_contents(&state.db)
            .await
            .unwrap_or_default(),
    );

    // Generate (or retrieve from cache) the "dicton du jour"
    let dicton =
        dicton::get_or_generate(&state.db, current_season, &state.config.huggingface_token).await;

    // Fetch news items from the database (synced by background task)
    let news_items = database::get_recent_news(&state.db, 6)
        .await
        .unwrap_or_default();

    templates::index(
        &prefix,
        &equipments,
        station_open,
        &photo_ids,
        &staff_photo_ids,
        &contents,
        dicton.as_deref(),
        &news_items,
        logged_in,
    )
}

pub async fn api_badge_counts(
    State(state): State<AppState>,
    jar: SignedCookieJar,
) -> impl IntoResponse {
    let current_season = get_current_season();
    let users = database::count_unimported_memberships(&state.db, current_season)
        .await
        .unwrap_or(0);
    let cash = database::count_unimported_cash(&state.db)
        .await
        .unwrap_or(0);

    // Compute validation count based on caller role
    let caller = resolve_caller(&jar, &state).await;
    let validations = match &caller {
        Some(s) if s.is_admin || s.is_god => {
            // Admins/gods see all pending validations
            database::count_pending_validations(&state.db, None)
                .await
                .unwrap_or(0)
        }
        Some(s) => {
            // Chiefs see only their ateliers' pending validations
            let is_chief = database::is_chief(&state.db, s.id).await.unwrap_or(false);
            if is_chief {
                database::count_pending_validations(&state.db, Some(s.id))
                    .await
                    .unwrap_or(0)
            } else {
                0
            }
        }
        None => 0,
    };

    let admin_total = users + cash + validations;

    Json(serde_json::json!({
        "online": users,
        "cash": cash,
        "validations": validations,
        "admin": admin_total,
    }))
}

pub async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    // Check database connection
    match sqlx::query("SELECT 1").fetch_one(&state.db).await {
        Ok(_) => {
            // Check HelloAsso client configuration
            if !state.helloasso_client.is_configured() {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({
                        "status": "degraded",
                        "database": "ok",
                        "helloasso": "misconfigured",
                        "message": "HelloAsso client is misconfigured"
                    })),
                )
                    .into_response();
            }

            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "status": "healthy",
                    "database": "ok",
                    "helloasso": "configured",
                    "timestamp": chrono::Utc::now()
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Database health check failed: {}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "status": "unhealthy",
                    "database": "failed",
                    "helloasso": "unknown",
                    "error": e.to_string()
                })),
            )
                .into_response()
        }
    }
}
