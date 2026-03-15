use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;
use tracing::error;

use crate::{AppState, auth::RequireAdmin, database, get_prefix, templates};

pub async fn settings_page_handler(
    RequireAdmin(_staff): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    let ateliers = database::get_all_ateliers(&state.db)
        .await
        .unwrap_or_default();
    templates::settings_page(&prefix, &ateliers)
}

#[derive(Debug, Deserialize)]
pub struct CreateAtelierRequest {
    name: String,
    slug: String,
    icon: String,
    needs_validation: bool,
    default_nightly: bool,
    opening_day_typical_needed: i16,
}

pub async fn api_create_atelier(
    RequireAdmin(admin): RequireAdmin,
    State(state): State<AppState>,
    Json(payload): Json<CreateAtelierRequest>,
) -> impl IntoResponse {
    let name = payload.name.trim();
    let slug = payload.slug.trim();
    if name.is_empty() || slug.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Nom et slug requis"})),
        );
    }

    match database::create_atelier(
        &state.db,
        name,
        slug,
        payload.icon.trim(),
        payload.needs_validation,
        payload.default_nightly,
        payload.opening_day_typical_needed,
    )
    .await
    {
        Ok(atelier) => {
            let _ = database::insert_audit(
                &state.db,
                Some(admin.id),
                &format!("{} {}", admin.first_name, admin.last_name),
                "Création atelier",
                &format!("name={} slug={}", atelier.name, atelier.slug),
            )
            .await;
            (
                StatusCode::OK,
                Json(serde_json::json!({"success": true, "id": atelier.id})),
            )
        }
        Err(e) => {
            error!("Error creating atelier: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }
}

pub async fn api_update_atelier(
    RequireAdmin(admin): RequireAdmin,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
    Json(payload): Json<CreateAtelierRequest>,
) -> impl IntoResponse {
    let name = payload.name.trim();
    let slug = payload.slug.trim();
    if name.is_empty() || slug.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Nom et slug requis"})),
        );
    }

    match database::update_atelier(
        &state.db,
        id,
        name,
        slug,
        payload.icon.trim(),
        payload.needs_validation,
        payload.default_nightly,
        payload.opening_day_typical_needed,
    )
    .await
    {
        Ok(atelier) => {
            let _ = database::insert_audit(
                &state.db,
                Some(admin.id),
                &format!("{} {}", admin.first_name, admin.last_name),
                "Modification atelier",
                &format!(
                    "id={} name={} slug={} icon={} validation={} nightly={} needed={}",
                    atelier.id,
                    atelier.name,
                    atelier.slug,
                    atelier.icon,
                    atelier.needs_validation,
                    atelier.default_nightly,
                    atelier.opening_day_typical_needed,
                ),
            )
            .await;
            (
                StatusCode::OK,
                Json(serde_json::json!({"success": true, "id": atelier.id})),
            )
        }
        Err(e) => {
            error!("Error updating atelier: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }
}

pub async fn api_delete_atelier(
    RequireAdmin(admin): RequireAdmin,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> impl IntoResponse {
    // Look up the atelier name for audit logging before deleting
    let atelier_name = database::get_atelier_by_id(&state.db, id)
        .await
        .ok()
        .flatten()
        .map_or_else(|| id.to_string(), |a| a.name);

    match database::delete_atelier(&state.db, id).await {
        Ok(()) => {
            let _ = database::insert_audit(
                &state.db,
                Some(admin.id),
                &format!("{} {}", admin.first_name, admin.last_name),
                "Suppression atelier",
                &format!("id={id} name={atelier_name}"),
            )
            .await;
            (StatusCode::OK, Json(serde_json::json!({"success": true})))
        }
        Err(e) => {
            error!("Error deleting atelier: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }
}
