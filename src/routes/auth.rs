use axum::{
    Json,
    extract::{Form, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::SignedCookieJar;
use serde::Deserialize;
use tracing::{error, warn};

use crate::{AppState, database, get_prefix, templates};

#[derive(Debug, Deserialize)]
pub struct StaffSearchQuery {
    q: Option<String>,
}

pub async fn login_page(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    let block = database::get_content(&state.db, "about-association")
        .await
        .ok()
        .flatten();
    templates::login_page(&prefix, block.as_ref())
}

fn private_login_response(response: impl IntoResponse) -> Response {
    let mut response = response.into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin"),
    );
    response
}

/// Link previews use GET/HEAD. Authentication requires the explicit POST below.
pub async fn login_link_page(
    state: &AppState,
    headers: &HeaderMap,
    jar: &SignedCookieJar,
    staff_id: uuid::Uuid,
    token: uuid::Uuid,
) -> Response {
    let prefix = get_prefix(headers);
    match database::login_token_staff(&state.db, staff_id, token).await {
        Ok(Some(staff)) => {
            private_login_response(templates::login_confirmation(&staff, token, &prefix))
        }
        Ok(None) => {
            if let Some(session) = jar
                .get("aghil_session")
                .and_then(|cookie| cookie.value().parse::<uuid::Uuid>().ok())
                && let Ok(Some(viewer)) = database::get_session_staff(&state.db, session).await
                && viewer.id == staff_id
            {
                return private_login_response(Redirect::to(&format!(
                    "{prefix}/person/{staff_id}"
                )));
            }
            private_login_response((
                StatusCode::BAD_REQUEST,
                templates::invalid_login_link(&prefix),
            ))
        }
        Err(e) => {
            error!("Error validating login link for staff {staff_id}: {e}");
            private_login_response(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Deserialize)]
pub struct ConfirmLogin {
    staff_id: uuid::Uuid,
    token: uuid::Uuid,
}

/// Confirmation forms must not be submitted by another website.
pub(crate) fn confirmation_origin_allowed(headers: &HeaderMap, base: &reqwest::Url) -> bool {
    if let Some(origin) = headers.get(header::ORIGIN) {
        let submitted = origin
            .to_str()
            .ok()
            .and_then(|value| reqwest::Url::parse(value).ok());
        if !submitted.is_some_and(|submitted| submitted.origin() == base.origin()) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod origin_tests {
    use super::*;

    #[test]
    fn forwarded_headers_cannot_authorize_an_attacker_origin() {
        let base = reqwest::Url::parse("https://station.example/powpow/").unwrap();
        let mut headers = HeaderMap::new();
        for key in ["host", "x-forwarded-host"] {
            headers.insert(key, "attacker.example".parse().unwrap());
        }
        headers.insert("x-forwarded-proto", "https".parse().unwrap());
        headers.insert(header::ORIGIN, "https://attacker.example".parse().unwrap());
        assert!(!confirmation_origin_allowed(&headers, &base));
        headers.insert(header::ORIGIN, "https://station.example".parse().unwrap());
        assert!(confirmation_origin_allowed(&headers, &base));
    }
}

pub async fn confirm_login(
    headers: HeaderMap,
    State(state): State<AppState>,
    jar: SignedCookieJar,
    Form(form): Form<ConfirmLogin>,
) -> Response {
    let prefix = get_prefix(&headers);
    if !confirmation_origin_allowed(&headers, &state.config.public_base_url) {
        return private_login_response(StatusCode::FORBIDDEN);
    }
    match database::consume_login_token(&state.db, form.staff_id, form.token).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            warn!("Invalid login confirmation for staff {}", form.staff_id);
            return private_login_response((
                StatusCode::BAD_REQUEST,
                templates::invalid_login_link(&prefix),
            ));
        }
        Err(e) => {
            error!("Error consuming login token: {e}");
            return private_login_response(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }
    let staff = match database::get_staff_by_id(&state.db, form.staff_id).await {
        Ok(Some(staff)) => staff,
        Ok(None) => return private_login_response(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Error fetching login staff: {e}");
            return private_login_response(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let has_membership = match database::has_current_membership(&state.db, staff.id).await {
        Ok(current) => current,
        Err(e) => {
            error!("Error checking login membership: {e}");
            return private_login_response(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    if !has_membership && !staff.is_admin && !staff.is_god {
        if let Some(session) = jar
            .get("aghil_session")
            .and_then(|cookie| cookie.value().parse::<uuid::Uuid>().ok())
        {
            let _ = database::delete_session(&state.db, session).await;
        }
        let mut cookie = axum_extra::extract::cookie::Cookie::new("aghil_session", "");
        cookie.set_path("/");
        return private_login_response((
            jar.remove(cookie),
            Redirect::to(&crate::auth::membership_redirect(&prefix, false)),
        ));
    }
    let user_agent = headers
        .get("User-Agent")
        .and_then(|value| value.to_str().ok());
    let ip = headers
        .get("X-Forwarded-For")
        .and_then(|value| value.to_str().ok());
    let session = match database::create_session(&state.db, staff.id, user_agent, ip).await {
        Ok(session) => session,
        Err(e) => {
            error!("Error creating login session: {e}");
            return private_login_response(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let mut cookie = axum_extra::extract::cookie::Cookie::new("aghil_session", session.to_string());
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
    cookie.set_secure(true);
    cookie.set_max_age(time::Duration::days(90));
    let destination = if has_membership {
        format!("{prefix}/person/{}", staff.id)
    } else {
        crate::auth::membership_redirect(&prefix, true)
    };
    private_login_response((jar.add(cookie), Redirect::to(&destination)))
}

pub async fn api_search_staff(
    State(state): State<AppState>,
    Query(params): Query<StaffSearchQuery>,
) -> impl IntoResponse {
    let q = params.q.unwrap_or_default();
    if q.len() < 4 {
        return Json(serde_json::json!([]));
    }

    match database::search_staff_by_name(&state.db, &q).await {
        Ok(staff_list) => {
            let results: Vec<serde_json::Value> = staff_list
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "id": s.id,
                        "first_name": s.first_name,
                        "last_name": s.last_name,
                    })
                })
                .collect();
            Json(serde_json::json!(results))
        }
        Err(e) => {
            error!("Error searching staff: {}", e);
            Json(serde_json::json!([]))
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct SendLoginRequest {
    staff_id: uuid::Uuid,
}

pub async fn api_send_login_email(
    State(state): State<AppState>,
    Json(payload): Json<SendLoginRequest>,
) -> impl IntoResponse {
    if !state
        .mail_limits
        .allow("login:global", 60, std::time::Duration::from_secs(60))
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(
                serde_json::json!({"error": "Trop de demandes de connexion. Réessayez dans une minute."}),
            ),
        );
    }
    // Get staff to find their email
    let staff = match database::get_staff_by_id(&state.db, payload.staff_id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Staff not found"})),
            );
        }
        Err(e) => {
            error!("Error fetching staff: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            );
        }
    };

    if staff.email.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Staff has no email address"})),
        );
    }

    // Generate token
    if !state.mail_limits.allow(
        &format!("login:{}", staff.email.trim().to_lowercase()),
        3,
        std::time::Duration::from_secs(600),
    ) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(
                serde_json::json!({"error": "Un lien a déjà été envoyé récemment. Réessayez dans dix minutes."}),
            ),
        );
    }
    let token = match database::create_login_token(&state.db, staff.id, &staff.email).await {
        Ok(t) => t,
        Err(e) => {
            error!("Error setting token: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to generate token"})),
            );
        }
    };

    // Build login URL
    let login_url = state
        .config
        .public_base_url
        .join(&format!("person/{}?token={token}", staff.id))
        .expect("UUIDs form a valid relative URL")
        .to_string();

    let html = maud::html! {
        p { "Bonjour " (staff.first_name) "," }
        p { "Cliquez sur le lien ci-dessous pour vous connecter à PowPow :" }
        p { a href=(&login_url) { "Se connecter" } }
        p { "Ou copiez ce lien : " (&login_url) }
        p { "Sur la page suivante, cliquez sur « Se connecter » pour confirmer votre connexion." }
        p { em { "Ce lien est à usage unique et valable 24 heures." } }
        (maud::PreEscaped(crate::email_signature(&state.config.entity_name)))
    }
    .into_string();
    match crate::personal_email::send(
        &state,
        &staff.email,
        &format!("{} {}", staff.first_name, staff.last_name),
        &format!("{} — Connexion PowPow", state.config.entity_name),
        html,
    )
    .await
    {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => {
            error!("Login delivery failed: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Échec de l'envoi de l'email"})),
            )
        }
    }
}

pub async fn api_me(State(state): State<AppState>, jar: SignedCookieJar) -> impl IntoResponse {
    let session_id = match jar.get("aghil_session") {
        Some(cookie) => match cookie.value().parse::<uuid::Uuid>() {
            Ok(id) => id,
            Err(_) => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error": "Invalid session"})),
                );
            }
        },
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Not logged in"})),
            );
        }
    };

    match database::get_session_staff(&state.db, session_id).await {
        Ok(Some(staff)) => {
            let membership_only = match database::has_current_membership(&state.db, staff.id).await
            {
                Ok(current) => !current,
                Err(e) => {
                    error!("Error checking current membership for me: {e}");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": "Impossible de vérifier l'adhésion"})),
                    );
                }
            };
            let is_chief = staff.is_admin
                || staff.is_god
                || database::is_chief(&state.db, staff.id)
                    .await
                    .unwrap_or(false);
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "id": staff.id,
                    "first_name": staff.first_name,
                    "last_name": staff.last_name,
                    "email": staff.email,
                    "is_admin": staff.is_admin,
                    "is_god": staff.is_god,
                    "is_chief": is_chief,
                    "membership_only": membership_only,
                })),
            )
        }
        Ok(None) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Staff not found"})),
        ),
        Err(e) => {
            error!("Error fetching staff for session: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Server error"})),
            )
        }
    }
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: SignedCookieJar,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);

    // Revoke the server-side session if we have one.
    if let Some(cookie) = jar.get("aghil_session")
        && let Ok(session_id) = cookie.value().parse::<uuid::Uuid>()
        && let Err(e) = database::delete_session(&state.db, session_id).await
    {
        error!("Error deleting session {}: {}", session_id, e);
    }

    let mut cookie = axum_extra::extract::cookie::Cookie::new("aghil_session", "");
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
    cookie.set_secure(true);
    cookie.set_max_age(time::Duration::ZERO);
    let updated_jar = jar.remove(cookie);
    (updated_jar, Redirect::to(&format!("{}/", prefix)))
}
