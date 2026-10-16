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
    whatsapp_group_url: Option<String>,
}

/// Keep only the invite URL, without optional sharing parameters.
fn normalize_whatsapp_group_url(value: Option<&str>) -> Result<Option<String>, &'static str> {
    const ERROR: &str = "Utilisez un lien d'invitation WhatsApp de la forme https://chat.whatsapp.com/… ou laissez le champ vide.";
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let url = reqwest::Url::parse(value).map_err(|_| ERROR)?;
    let path = url.path().strip_prefix('/').ok_or(ERROR)?;
    let code = path.strip_suffix('/').unwrap_or(path);
    if value.chars().any(char::is_control)
        || url.scheme() != "https"
        || url.host_str() != Some("chat.whatsapp.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || code.len() != 22
        || !code.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return Err(ERROR);
    }
    Ok(Some(format!("https://chat.whatsapp.com/{code}")))
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

    let whatsapp_group_url =
        match normalize_whatsapp_group_url(payload.whatsapp_group_url.as_deref()) {
            Ok(url) => url,
            Err(error) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": error})),
                );
            }
        };

    match database::create_atelier(
        &state.db,
        name,
        slug,
        payload.icon.trim(),
        payload.needs_validation,
        payload.default_nightly,
        payload.opening_day_typical_needed,
        whatsapp_group_url.as_deref(),
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

    let whatsapp_group_url =
        match normalize_whatsapp_group_url(payload.whatsapp_group_url.as_deref()) {
            Ok(url) => url,
            Err(error) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": error})),
                );
            }
        };

    match database::update_atelier(
        &state.db,
        id,
        name,
        slug,
        payload.icon.trim(),
        payload.needs_validation,
        payload.default_nightly,
        payload.opening_day_typical_needed,
        whatsapp_group_url.as_deref(),
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
                    "id={} name={} slug={} icon={} validation={} nightly={} needed={} whatsapp={}",
                    atelier.id,
                    atelier.name,
                    atelier.slug,
                    atelier.icon,
                    atelier.needs_validation,
                    atelier.default_nightly,
                    atelier.opening_day_typical_needed,
                    atelier.whatsapp_group_url.is_some(),
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

#[cfg(test)]
mod tests {
    use super::normalize_whatsapp_group_url;

    #[test]
    fn whatsapp_links_allow_invites_and_reject_other_destinations() {
        assert_eq!(normalize_whatsapp_group_url(None), Ok(None));
        assert_eq!(normalize_whatsapp_group_url(Some("  ")), Ok(None));
        assert_eq!(
            normalize_whatsapp_group_url(Some(
                " https://chat.whatsapp.com/AbCdEfGhIjKlMnOpQrStUv/?mode=ems_copy_t "
            )),
            Ok(Some(
                "https://chat.whatsapp.com/AbCdEfGhIjKlMnOpQrStUv".into()
            ))
        );
        for invalid in [
            "group-id",
            "javascript:alert(1)",
            "http://chat.whatsapp.com/AbCdEfGhIjKlMnOpQrStUv",
            "https://example.org/AbCdEfGhIjKlMnOpQrStUv",
            "https://chat.whatsapp.com.evil.example/AbCdEfGhIjKlMnOpQrStUv",
            "https://chat.whatsapp.com@evil.example/AbCdEfGhIjKlMnOpQrStUv",
            "https://user:password@chat.whatsapp.com/AbCdEfGhIjKlMnOpQrStUv",
            "https://chat.whatsapp.com:8443/AbCdEfGhIjKlMnOpQrStUv",
            "https://chat.whatsapp.com/",
            "https://chat.whatsapp.com/TooShort",
            "https://chat.whatsapp.com/AbCdEfGhIjKlMnOpQrStUvW",
            "https://chat.whatsapp.com/AbCdEfGhIjKlMnOpQrStU_",
            "https://chat.whatsapp.com/AbCdEfGhIjKlMnOpQrStUv/extra",
            "https://chat.whatsapp.com/AbCdEfGhIjKlMnOpQrStUv%22",
            "https://chat.whats\napp.com/AbCdEfGhIjKlMnOpQrStUv",
        ] {
            assert!(
                normalize_whatsapp_group_url(Some(invalid)).is_err(),
                "{invalid}"
            );
        }
    }
}
