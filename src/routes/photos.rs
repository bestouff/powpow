use axum::{
    Json,
    body::Body,
    extract::{Multipart, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
};
use tracing::error;

use crate::{
    AppState,
    auth::{RequireAdmin, RequireStaff},
    database, get_prefix, templates,
};

pub async fn photo_page(
    RequireStaff(staff): RequireStaff,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);

    match database::get_all_photos(&state.db).await {
        Ok(photos) => templates::photo_page(&prefix, &photos, staff.is_admin),
        Err(e) => {
            error!("Failed to get photos: {}", e);
            templates::photo_page(&prefix, &[], false)
        }
    }
}

pub async fn display_photo(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> impl IntoResponse {
    match database::get_photo_by_id(&state.db, id).await {
        Ok(Some(photo)) => {
            let mut response = Response::new(Body::from(photo.photo_data));
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_str(&photo.mime_type).unwrap(),
            );
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                header::HeaderValue::from_static("public, max-age=86400, immutable"),
            );
            response
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            error!("Failed to get photo: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn upload_photo(
    RequireAdmin(_staff): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    tracing::info!("upload_photo handler entered");
    let prefix = get_prefix(&headers);
    let mut photographer_id: Option<uuid::Uuid> = None;
    let mut photo_data: Option<(Vec<u8>, String)> = None;
    let mut multipart_error: Option<String> = None;

    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                let name = field.name().unwrap_or("").to_string();
                match name.as_str() {
                    "photographer_id" => {
                        let text = field.text().await.unwrap_or_default();
                        photographer_id = uuid::Uuid::parse_str(text.trim()).ok();
                    }
                    "photo" => {
                        let content_type = field
                            .content_type()
                            .unwrap_or("application/octet-stream")
                            .to_string();
                        match field.bytes().await {
                            Ok(data) => {
                                tracing::info!("Photo upload: received {} bytes", data.len());
                                photo_data = Some((data.to_vec(), content_type));
                            }
                            Err(e) => {
                                error!("Failed to read photo data: {}", e);
                                multipart_error = Some(format!("Erreur lecture fichier: {}", e));
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(None) => break,
            Err(e) => {
                error!("Multipart stream error: {}", e);
                multipart_error = Some(format!("Erreur upload: {}", e));
                break;
            }
        }
    }

    if let Some(err) = multipart_error {
        return (StatusCode::BAD_REQUEST, Html(format!(
            r#"<html><body><h2>Erreur</h2><p>{}</p><a href="{}/photos">Retour</a></body></html>"#,
            templates::escape_html_public(&err), prefix
        ))).into_response();
    }

    match (photo_data, photographer_id) {
        (Some((data, content_type)), Some(pid)) => {
            match database::create_photo(&state.db, data, content_type, pid).await {
                Ok(_) => Redirect::to(&format!("{}/photos", prefix)).into_response(),
                Err(e) => {
                    error!("Failed to upload photo: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, Html(format!(
                        r#"<html><body><h2>Erreur</h2><p>Échec de l'enregistrement: {}</p><a href="{}/photos">Retour</a></body></html>"#,
                        templates::escape_html_public(&e.to_string()), prefix
                    ))).into_response()
                }
            }
        }
        (None, _) => {
            error!("Photo upload: no photo data received");
            (StatusCode::BAD_REQUEST, Html(format!(
                r#"<html><body><h2>Erreur</h2><p>Aucune photo reçue</p><a href="{}/photos">Retour</a></body></html>"#,
                prefix
            ))).into_response()
        }
        (_, None) => {
            error!("Photo upload: no photographer selected");
            (StatusCode::BAD_REQUEST, Html(format!(
                r#"<html><body><h2>Erreur</h2><p>Aucun photographe sélectionné</p><a href="{}/photos">Retour</a></body></html>"#,
                prefix
            ))).into_response()
        }
    }
}

pub async fn delete_photo(
    RequireAdmin(_staff): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    match database::delete_photo(&state.db, id).await {
        Ok(success) => {
            if success {
                Redirect::to(&format!("{}/photos", prefix)).into_response()
            } else {
                StatusCode::NOT_FOUND.into_response()
            }
        }
        Err(e) => {
            error!("Failed to delete photo: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Return all photo IDs for the hero slideshow.
pub async fn api_photo_ids(State(state): State<AppState>) -> impl IntoResponse {
    match database::get_all_photo_ids(&state.db).await {
        Ok(ids) => Json(ids).into_response(),
        Err(e) => {
            error!("Failed to get photo IDs: {}", e);
            Json(Vec::<uuid::Uuid>::new()).into_response()
        }
    }
}
