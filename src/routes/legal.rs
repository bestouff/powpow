use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};

use crate::{AppState, POWPOW_CSS, POWPOW_JS, database, get_prefix, templates};

pub async fn privacy_page(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    let block = database::get_content(&state.db, "privacy")
        .await
        .ok()
        .flatten();
    templates::legal_page(&prefix, "Politique de Confidentialité", block.as_ref())
}

pub async fn tos_page(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    let block = database::get_content(&state.db, "tos").await.ok().flatten();
    templates::legal_page(&prefix, "Conditions d'Utilisation", block.as_ref())
}

pub async fn serve_css() -> impl IntoResponse {
    Response::builder()
        .header(header::CONTENT_TYPE, "text/css; charset=utf-8")
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(Body::from(POWPOW_CSS))
        .unwrap()
}

pub async fn serve_js() -> impl IntoResponse {
    Response::builder()
        .header(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(Body::from(POWPOW_JS))
        .unwrap()
}
