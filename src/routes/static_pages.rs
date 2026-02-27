use axum::{
    body::Body,
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};

use crate::{POWPOW_CSS, POWPOW_JS, get_prefix, templates};

pub async fn privacy_page(headers: HeaderMap) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    templates::static_page(
        &prefix,
        "Politique de Confidentialité",
        include_str!("../../privacy.md"),
    )
}

pub async fn tos_page(headers: HeaderMap) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    templates::static_page(
        &prefix,
        "Conditions d'Utilisation",
        include_str!("../../tos.md"),
    )
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
