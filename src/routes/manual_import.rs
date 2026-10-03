use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::{
    AppState,
    auth::RequireAdmin,
    auto_import::Source,
    database::manual_import::{self as store, Error},
    get_prefix,
    manual_import::ImportRequest,
    templates,
};

fn failure(error: Error) -> Response {
    let (status, code) = match &error {
        Error::NotFound => (StatusCode::NOT_FOUND, "not_found"),
        Error::NotMembership | Error::Invalid(_) => (StatusCode::BAD_REQUEST, "invalid"),
        Error::AlreadyImported => (StatusCode::CONFLICT, "already_imported"),
        Error::StaleStaff => (StatusCode::CONFLICT, "stale_staff"),
        Error::DuplicateMembership => (StatusCode::CONFLICT, "duplicate_membership"),
        Error::IdentityConflict { .. } => (StatusCode::CONFLICT, "identity_conflict"),
        Error::Database(_) | Error::Internal(_) => {
            tracing::error!("Manual import failed: {error}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Impossible de traiter l'importation. Réessayez ou contactez un administrateur.", "code": "internal"}))).into_response();
        }
    };
    (
        status,
        Json(serde_json::json!({"error": error.to_string(), "code": code})),
    )
        .into_response()
}

async fn page(state: &AppState, headers: &HeaderMap, source: Source) -> Response {
    let prefix = get_prefix(headers);
    let mut connection = match state.db.acquire().await {
        Ok(connection) => connection,
        Err(error) => return failure(Error::Database(error)),
    };
    match store::context(&mut connection, source).await {
        Ok(context) => templates::manual_import_page(&context, &prefix).into_response(),
        Err(error) => {
            let status = match &error {
                Error::NotFound => StatusCode::NOT_FOUND,
                Error::NotMembership => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (
                status,
                templates::import_result(false, &error.to_string(), &prefix),
            )
                .into_response()
        }
    }
}

pub async fn online_page(
    RequireAdmin(_admin): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Response {
    page(&state, &headers, Source::HelloAsso(id)).await
}

pub async fn cash_page(
    RequireAdmin(_admin): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Response {
    page(&state, &headers, Source::Cash(id)).await
}

#[derive(Deserialize)]
pub struct SearchQuery {
    source: String,
    id: String,
    #[serde(default)]
    q: String,
}

pub async fn search(
    RequireAdmin(_admin): RequireAdmin,
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Response {
    let source = match query.source.as_str() {
        "online" => query.id.parse().ok().map(Source::HelloAsso),
        "cash" => query.id.parse().ok().map(Source::Cash),
        _ => None,
    };
    let Some(source) = source else {
        return failure(Error::Invalid("Source d'adhésion invalide.".into()));
    };
    let mut connection = match state.db.acquire().await {
        Ok(connection) => connection,
        Err(error) => return failure(Error::Database(error)),
    };
    let context = match store::context(&mut connection, source).await {
        Ok(context) => context,
        Err(error) => return failure(error),
    };
    drop(connection);
    match store::search(&state.db, &context, &query.q).await {
        Ok(candidates) => Json(candidates).into_response(),
        Err(error) => failure(error),
    }
}

async fn submit(
    state: &AppState,
    admin: &crate::models::Staff,
    source: Source,
    request: ImportRequest,
) -> Response {
    match store::submit(&state.db, source, request, admin).await {
        Ok(staff) => {
            super::mailchimp::spawn_newsletter_sync(state);
            Json(serde_json::json!({"success": true, "staff_id": staff})).into_response()
        }
        Err(error) => failure(error),
    }
}

pub async fn online_submit(
    RequireAdmin(admin): RequireAdmin,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<ImportRequest>,
) -> Response {
    submit(&state, &admin, Source::HelloAsso(id), request).await
}

pub async fn cash_submit(
    RequireAdmin(admin): RequireAdmin,
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(request): Json<ImportRequest>,
) -> Response {
    submit(&state, &admin, Source::Cash(id), request).await
}
