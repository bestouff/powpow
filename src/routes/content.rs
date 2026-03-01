use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use tracing::error;

use crate::{AppState, auth::RequireAdmin, database, get_prefix, templates};

/// Maximum CMS image upload size: 5 MB.
const MAX_IMAGE_SIZE: usize = 5 * 1024 * 1024;

/// Serve a CMS image by UUID (public, cached).
pub async fn serve_content_image(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    match database::get_content_image(&state.db, id).await {
        Ok(Some(img)) => {
            let mut response = Response::new(Body::from(img.data));
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_str(&img.content_type)
                    .unwrap_or(header::HeaderValue::from_static("application/octet-stream")),
            );
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                header::HeaderValue::from_static("public, max-age=3600"),
            );
            response
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            error!("Failed to get content image: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// List all content blocks (admin page).
pub async fn content_list(
    RequireAdmin(_staff): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    match database::get_all_contents(&state.db).await {
        Ok(contents) => {
            let mut blocks: Vec<_> = contents.into_values().collect();
            blocks.sort_by(|a, b| a.slug.cmp(&b.slug));
            templates::content_list_page(&prefix, &blocks)
        }
        Err(e) => {
            error!("Failed to list contents: {}", e);
            templates::content_list_page(&prefix, &[])
        }
    }
}

/// Edit form for a single content block (admin page).
pub async fn content_edit(
    RequireAdmin(_staff): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    match database::get_content(&state.db, &slug).await {
        Ok(Some(block)) => {
            let image_filename = if let Some(img_id) = block.image_id {
                database::get_content_image_filename(&state.db, img_id)
                    .await
                    .unwrap_or(None)
            } else {
                None
            };
            templates::content_edit_page(&prefix, &block, image_filename.as_deref()).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            error!("Failed to get content {}: {}", slug, e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Save a content block (multipart form: title, body, link fields + optional image).
#[allow(clippy::too_many_lines)]
pub async fn content_save(
    RequireAdmin(_staff): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(slug): Path<String>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);

    let mut title = String::new();
    let mut body = String::new();
    let mut link_url: Option<String> = None;
    let mut link_label: Option<String> = None;
    let mut image_data: Option<(Vec<u8>, String, String)> = None; // (data, content_type, filename)
    let mut remove_image = false;
    let mut multipart_error: Option<String> = None;

    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                let name = field.name().unwrap_or("").to_string();
                match name.as_str() {
                    "title" => {
                        title = field.text().await.unwrap_or_default();
                    }
                    "body" => {
                        body = field.text().await.unwrap_or_default();
                    }
                    "link_url" => {
                        let v = field.text().await.unwrap_or_default();
                        link_url = if v.trim().is_empty() {
                            None
                        } else {
                            Some(v.trim().to_string())
                        };
                    }
                    "link_label" => {
                        let v = field.text().await.unwrap_or_default();
                        link_label = if v.trim().is_empty() {
                            None
                        } else {
                            Some(v.trim().to_string())
                        };
                    }
                    "remove_image" => {
                        let v = field.text().await.unwrap_or_default();
                        remove_image = v == "1";
                    }
                    "image" => {
                        let content_type = field
                            .content_type()
                            .unwrap_or("application/octet-stream")
                            .to_string();
                        let filename = field.file_name().unwrap_or("upload").to_string();
                        match field.bytes().await {
                            Ok(data) if !data.is_empty() => {
                                if data.len() > MAX_IMAGE_SIZE {
                                    multipart_error =
                                        Some("Image trop volumineuse (max 5 Mo)".to_string());
                                } else {
                                    image_data = Some((data.to_vec(), content_type, filename));
                                }
                            }
                            Ok(_) => {} // empty file field, ignore
                            Err(e) => {
                                error!("Failed to read image data: {}", e);
                                multipart_error = Some(format!("Erreur lecture fichier: {e}"));
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(None) => break,
            Err(e) => {
                error!("Multipart stream error: {}", e);
                multipart_error = Some(format!("Erreur upload: {e}"));
                break;
            }
        }
    }

    if let Some(err) = multipart_error {
        return (
            StatusCode::BAD_REQUEST,
            axum::response::Html(format!(
                r#"<html><body><h2>Erreur</h2><p>{}</p><a href="{}/admin/contents/{}">Retour</a></body></html>"#,
                templates::escape_html_public(&err),
                prefix,
                slug
            )),
        )
            .into_response();
    }

    // Resolve image_id: upload new, remove, or keep existing
    let existing_block = database::get_content(&state.db, &slug).await.ok().flatten();
    let mut image_id = existing_block.as_ref().and_then(|b| b.image_id);

    if let Some((data, ct, fname)) = image_data {
        // Delete old image if replacing
        if let Some(old_id) = image_id {
            let _ = database::delete_content_image(&state.db, old_id).await;
        }
        match database::create_content_image(&state.db, data, &ct, &fname).await {
            Ok(new_id) => image_id = Some(new_id),
            Err(e) => {
                error!("Failed to create content image: {}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    } else if remove_image {
        if let Some(old_id) = image_id {
            let _ = database::delete_content_image(&state.db, old_id).await;
        }
        image_id = None;
    }

    match database::update_content(
        &state.db,
        &slug,
        &title,
        &body,
        image_id,
        link_url.as_deref(),
        link_label.as_deref(),
    )
    .await
    {
        Ok(()) => Redirect::to(&format!("{prefix}/admin/contents")).into_response(),
        Err(e) => {
            error!("Failed to update content {}: {}", slug, e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
