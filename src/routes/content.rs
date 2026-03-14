use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use tracing::error;

use crate::{AppState, auth::RequireAdmin, database, get_prefix, models::ContentMap, templates};

/// Maximum CMS image upload size: 5 MB.
const MAX_IMAGE_SIZE: usize = 5 * 1024 * 1024;

/// Allowed image MIME types for uploads.
const ALLOWED_IMAGE_TYPES: &[&str] = &[
    "image/jpeg",
    "image/png",
    "image/gif",
    "image/webp",
    "image/avif",
];

/// Return the MIME type if it is an allowed image type, or fall back to
/// `image/jpeg` for any unrecognised / non-image Content-Type.
fn sanitise_image_mime(ct: &str) -> &'static str {
    ALLOWED_IMAGE_TYPES
        .iter()
        .find(|&&allowed| allowed.eq_ignore_ascii_case(ct))
        .copied()
        .unwrap_or("image/jpeg")
}

/// Serve a CMS image by UUID (public, cached).
pub async fn serve_content_image(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    match database::get_content_image(&state.db, id).await {
        Ok(Some(img)) => {
            let safe_mime = sanitise_image_mime(&img.content_type);
            let mut response = Response::new(Body::from(img.data));
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_static(safe_mime),
            );
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                header::HeaderValue::from_static("public, max-age=3600"),
            );
            response.headers_mut().insert(
                header::HeaderName::from_static("x-content-type-options"),
                header::HeaderValue::from_static("nosniff"),
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

/// Serve a news-feed image by news row UUID (public, cached).
pub async fn serve_news_image(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    match database::get_news_image(&state.db, id).await {
        Ok(Some((data, mime))) => {
            let safe_mime = sanitise_image_mime(&mime);
            let mut response = Response::new(Body::from(data));
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_static(safe_mime),
            );
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                header::HeaderValue::from_static("public, max-age=3600"),
            );
            response.headers_mut().insert(
                header::HeaderName::from_static("x-content-type-options"),
                header::HeaderValue::from_static("nosniff"),
            );
            response
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            error!("Failed to get news image: {}", e);
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
    RequireAdmin(staff): RequireAdmin,
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
                                if ALLOWED_IMAGE_TYPES
                                    .iter()
                                    .any(|&a| a.eq_ignore_ascii_case(&content_type))
                                {
                                    if data.len() > MAX_IMAGE_SIZE {
                                        multipart_error =
                                            Some("Image trop volumineuse (max 5 Mo)".to_string());
                                    } else {
                                        image_data = Some((data.to_vec(), content_type, filename));
                                    }
                                } else {
                                    multipart_error = Some(format!(
                                        "Type de fichier non autorisé : {}. Formats acceptés : JPEG, PNG, GIF, WebP, AVIF.",
                                        content_type
                                    ));
                                }
                            }
                            Ok(_) => {} // empty file field (no file selected), ignore
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
    let mut image_changed = false;

    if let Some((data, ct, fname)) = image_data {
        image_changed = true;
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
        image_changed = true;
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
        Ok(()) => {
            // Audit trail
            let mut details = format!("slug={slug} title=«{title}»");
            if image_changed {
                if image_id.is_some() {
                    details.push_str(" image=modifiée");
                } else {
                    details.push_str(" image=supprimée");
                }
            }
            let _ = database::insert_audit(
                &state.db,
                Some(staff.id),
                &format!("{} {}", staff.first_name, staff.last_name),
                "Modification contenu CMS",
                &details,
            )
            .await;

            // Refresh the navbar logo cache if the navbar block was updated
            if slug == "navbar" {
                let block = database::get_content(&state.db, "navbar")
                    .await
                    .ok()
                    .flatten();
                templates::set_navbar_block(block);
            }
            // Refresh the favicon cache if the favicon block was updated
            if slug == "favicon" {
                let block = database::get_content(&state.db, "favicon")
                    .await
                    .ok()
                    .flatten();
                templates::set_favicon_block(block);
            }
            // Refresh the footer blocks cache if a footer slug was updated
            if templates::is_footer_slug(&slug)
                && let Ok(footer_map) = database::get_contents_by_slugs(
                    &state.db,
                    &["footer-contact", "footer-calendar", "footer-summer"],
                )
                .await
            {
                templates::set_footer_blocks(ContentMap::new(footer_map));
            }
            Redirect::to(&format!("{prefix}/admin/contents")).into_response()
        }
        Err(e) => {
            error!("Failed to update content {}: {}", slug, e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
