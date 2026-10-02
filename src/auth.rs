use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Key, SignedCookieJar};
use tracing::error;

use crate::{AppState, database, models::Staff};

pub fn membership_redirect(prefix: &str, is_admin: bool) -> String {
    if is_admin {
        format!("{prefix}/admin?membership_required=true")
    } else {
        format!("{prefix}/?membership_required=true")
    }
}

fn membership_management_path(path: &str) -> bool {
    matches!(
        path,
        "/admin"
            | "/online"
            | "/cash"
            | "/sync"
            | "/api/online"
            | "/api/sync"
            | "/api/staff/create-minimal"
            | "/api/me"
            | "/api/badge-counts"
    ) || path.starts_with("/online/")
        || path.starts_with("/import/")
        || path.starts_with("/cash-import/")
}

fn public_path(path: &str, method: &axum::http::Method) -> bool {
    matches!(
        path,
        "/" | "/login"
            | "/logout"
            | "/health"
            | "/privacy"
            | "/tos"
            | "/contact"
            | "/api/login/send"
            | "/api/staff/search"
    ) || (*method == axum::http::Method::GET
        && (path.starts_with("/static/")
            || path.starts_with("/content-images/")
            || path.starts_with("/news-images/")
            || path
                .strip_prefix("/photos/")
                .is_some_and(|id| id.parse::<uuid::Uuid>().is_ok())))
}

pub async fn membership_gate(
    axum::extract::State(state): axum::extract::State<AppState>,
    jar: SignedCookieJar,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let path = request.uri().path();
    // Login tokens are checked before creating a session in view_person.
    if public_path(path, request.method())
        || (path.starts_with("/person/")
            && request
                .uri()
                .query()
                .is_some_and(|query| query.split('&').any(|part| part.starts_with("token="))))
    {
        return next.run(request).await;
    }
    let Some(session_id) = jar
        .get("aghil_session")
        .and_then(|cookie| cookie.value().parse::<uuid::Uuid>().ok())
    else {
        return next.run(request).await;
    };
    let staff = match database::get_session_staff(&state.db, session_id).await {
        Ok(Some(staff)) => staff,
        Ok(None) => return next.run(request).await,
        Err(e) => {
            error!("Error checking membership session: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    match database::has_current_membership(&state.db, staff.id).await {
        Ok(true) => next.run(request).await,
        Ok(false) => {
            let is_admin = staff.is_admin || staff.is_god;
            if is_admin && membership_management_path(path) {
                return next.run(request).await;
            }
            let prefix = crate::get_prefix(request.headers());
            let redirect = membership_redirect(&prefix, is_admin);
            if path.starts_with("/api/") {
                return (StatusCode::FORBIDDEN, Json(serde_json::json!({
                    "error": "Votre adhésion n'est pas à jour. Veuillez régler votre adhésion sur HelloAsso.",
                    "redirect": redirect,
                }))).into_response();
            }
            Redirect::to(&redirect).into_response()
        }
        Err(e) => {
            error!("Error checking current membership: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

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
        .map(|s| s.trim_end_matches('/'))
        .filter(|s| {
            !s.is_empty()
                && s.bytes().all(|b| {
                    b.is_ascii_alphanumeric() || b == b'/' || b == b'_' || b == b'-' || b == b'.'
                })
        })
        .map(String::from)
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

    let session_id = match jar.get("aghil_session") {
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

    match database::get_session_staff(&state.db, session_id).await {
        Ok(Some(staff)) => Ok(staff),
        Ok(None) => Err(AuthError {
            kind: AuthErrorKind::NotLoggedIn,
            prefix,
            is_api,
        }),
        Err(e) => {
            error!("Auth: DB error looking up session {}: {}", session_id, e);
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

pub async fn authorize_staff_management(
    state: &AppState,
    viewer: &Staff,
    staff_id: uuid::Uuid,
    atelier_id: Option<uuid::Uuid>,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if viewer.is_admin || viewer.is_god {
        return Ok(());
    }
    match database::chief_manages_staff(&state.db, viewer.id, staff_id, atelier_id).await {
        Ok(true) => Ok(()),
        Ok(false) => Err((
            StatusCode::FORBIDDEN,
            Json(
                serde_json::json!({"error": "Vous ne pouvez modifier que le staff de vos ateliers."}),
            ),
        )),
        Err(e) => {
            error!("Error checking chief staff permissions: {e}");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Impossible de vérifier les droits."})),
            ))
        }
    }
}

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

        if staff.is_admin || staff.is_god {
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

#[cfg(test)]
mod membership_tests {
    use super::{membership_management_path, membership_redirect, public_path};
    use axum::http::Method;

    #[test]
    fn unpaid_admin_can_only_manage_memberships() {
        for path in [
            "/admin",
            "/online",
            "/online/42",
            "/import/42",
            "/cash",
            "/cash-import/42",
            "/api/sync",
            "/api/staff/create-minimal",
        ] {
            assert!(membership_management_path(path), "should allow {path}");
        }
        for path in [
            "/staff",
            "/person/42",
            "/api/person/42/role",
            "/calendar",
            "/api/calendar/toggle",
            "/api/admin/flags",
            "/api/admin/clear-expired-roles",
            "/api/staff-qualif",
            "/backup",
            "/restore",
            "/mailchimp",
            "/admin/contents",
            "/photos/upload",
        ] {
            assert!(!membership_management_path(path), "should block {path}");
            assert!(
                !public_path(path, &Method::POST),
                "should not treat {path} as public"
            );
        }
    }

    #[test]
    fn membership_redirects_preserve_proxy_prefix() {
        assert_eq!(
            membership_redirect("/powpow", false),
            "/powpow/?membership_required=true"
        );
        assert_eq!(
            membership_redirect("/powpow", true),
            "/powpow/admin?membership_required=true"
        );
    }
}
