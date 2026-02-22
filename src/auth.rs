use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Key, SignedCookieJar};
use tracing::error;

use crate::{AppState, database, models::Staff};

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

enum AuthErrorKind {
    NotLoggedIn,
    InsufficientPrivilege,
    InternalError,
}

pub struct AuthError {
    kind: AuthErrorKind,
    prefix: String,
    is_api: bool,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        match (&self.kind, self.is_api) {
            (AuthErrorKind::NotLoggedIn, true) => (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Not logged in"})),
            )
                .into_response(),

            (AuthErrorKind::InsufficientPrivilege, true) => (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({"error": "Insufficient privileges"})),
            )
                .into_response(),

            (AuthErrorKind::NotLoggedIn, false) => {
                Redirect::to(&format!("{}/login", self.prefix)).into_response()
            }

            (AuthErrorKind::InsufficientPrivilege, false) => (
                StatusCode::FORBIDDEN,
                axum::response::Html(
                    "<h1>403 — Accès interdit</h1><p>Vous n'avez pas les droits nécessaires.</p>"
                        .to_string(),
                ),
            )
                .into_response(),

            (AuthErrorKind::InternalError, _) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal server error"})),
            )
                .into_response(),
        }
    }
}

// ---------------------------------------------------------------------------
// Shared authenticate helper
// ---------------------------------------------------------------------------

fn get_prefix(parts: &Parts) -> String {
    parts
        .headers
        .get("X-Forwarded-Prefix")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim_end_matches('/').to_string())
        .unwrap_or_default()
}

fn is_api_path(parts: &Parts) -> bool {
    parts.uri.path().starts_with("/api/")
}

async fn authenticate(parts: &mut Parts, state: &AppState) -> Result<Staff, AuthError> {
    let prefix = get_prefix(parts);
    let is_api = is_api_path(parts);

    // Extract signed cookie jar (infallible for SignedCookieJar)
    let jar = SignedCookieJar::<Key>::from_request_parts(parts, state)
        .await
        .expect("SignedCookieJar extraction is infallible");

    let staff_id = match jar.get("aghil_session") {
        Some(cookie) => match cookie.value().parse::<uuid::Uuid>() {
            Ok(id) => id,
            Err(_) => {
                return Err(AuthError {
                    kind: AuthErrorKind::NotLoggedIn,
                    prefix,
                    is_api,
                });
            }
        },
        None => {
            return Err(AuthError {
                kind: AuthErrorKind::NotLoggedIn,
                prefix,
                is_api,
            });
        }
    };

    match database::get_staff_by_id(&state.db, staff_id).await {
        Ok(Some(staff)) => Ok(staff),
        Ok(None) => Err(AuthError {
            kind: AuthErrorKind::NotLoggedIn,
            prefix,
            is_api,
        }),
        Err(e) => {
            error!("Auth: DB error looking up staff {}: {}", staff_id, e);
            Err(AuthError {
                kind: AuthErrorKind::InternalError,
                prefix,
                is_api,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Extractor structs
// ---------------------------------------------------------------------------

pub struct RequireStaff(pub Staff);
pub struct RequireChief(pub Staff);
pub struct RequireAdmin(pub Staff);
pub struct RequireGod(pub Staff);

// ---------------------------------------------------------------------------
// FromRequestParts impls
// ---------------------------------------------------------------------------

impl FromRequestParts<AppState> for RequireStaff {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let staff = authenticate(parts, state).await?;
        Ok(RequireStaff(staff))
    }
}

impl FromRequestParts<AppState> for RequireChief {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let prefix = get_prefix(parts);
        let is_api = is_api_path(parts);
        let staff = authenticate(parts, state).await?;

        // is_admin or is_god implies chief-level access
        if staff.is_admin || staff.is_god {
            return Ok(RequireChief(staff));
        }

        // Check if chief of any atelier
        match database::is_chief(&state.db, staff.id).await {
            Ok(true) => Ok(RequireChief(staff)),
            Ok(false) => Err(AuthError {
                kind: AuthErrorKind::InsufficientPrivilege,
                prefix,
                is_api,
            }),
            Err(e) => {
                error!("Auth: DB error checking chief for {}: {}", staff.id, e);
                Err(AuthError {
                    kind: AuthErrorKind::InternalError,
                    prefix,
                    is_api,
                })
            }
        }
    }
}

impl FromRequestParts<AppState> for RequireAdmin {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let prefix = get_prefix(parts);
        let is_api = is_api_path(parts);
        let staff = authenticate(parts, state).await?;

        if staff.is_admin {
            Ok(RequireAdmin(staff))
        } else {
            Err(AuthError {
                kind: AuthErrorKind::InsufficientPrivilege,
                prefix,
                is_api,
            })
        }
    }
}

impl FromRequestParts<AppState> for RequireGod {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let prefix = get_prefix(parts);
        let is_api = is_api_path(parts);
        let staff = authenticate(parts, state).await?;

        if staff.is_god {
            Ok(RequireGod(staff))
        } else {
            Err(AuthError {
                kind: AuthErrorKind::InsufficientPrivilege,
                prefix,
                is_api,
            })
        }
    }
}
