use axum::{
    Json,
    extract::{Form, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::SignedCookieJar;
use base64::Engine;
use serde::Deserialize;
use tracing::{error, info, warn};

use crate::{AppState, database, get_prefix, models, templates};

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

pub async fn confirm_login(
    headers: HeaderMap,
    State(state): State<AppState>,
    jar: SignedCookieJar,
    Form(form): Form<ConfirmLogin>,
) -> Response {
    let prefix = get_prefix(&headers);
    // Do not accept another website's form submission as a login confirmation.
    if let Some(origin) = headers.get(header::ORIGIN) {
        let proto = headers
            .get("X-Forwarded-Proto")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("http");
        let host = headers
            .get(header::HOST)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        let expected = reqwest::Url::parse(&format!("{proto}://{host}"));
        let submitted = origin
            .to_str()
            .ok()
            .and_then(|value| reqwest::Url::parse(value).ok());
        if !submitted
            .zip(expected.ok())
            .is_some_and(|(submitted, expected)| submitted.origin() == expected.origin())
        {
            return private_login_response(StatusCode::FORBIDDEN);
        }
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
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<SendLoginRequest>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);

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
    let token = match database::create_login_token(&state.db, staff.id).await {
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
    let proto = headers
        .get("X-Forwarded-Proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");
    let host = headers
        .get("X-Forwarded-Host")
        .or_else(|| headers.get("Host"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost:3000");
    let login_url = format!(
        "{}://{}{}/person/{}?token={}",
        proto, host, prefix, staff.id, token
    );

    // Determine mail method: "gmail" or "smtp" (default)
    let mail_method = if state.config.mail_method.eq_ignore_ascii_case("gmail") {
        "gmail"
    } else {
        "smtp"
    };
    info!(
        "Sending login email via {} (MAIL_METHOD={:?})",
        mail_method, state.config.mail_method
    );

    match mail_method {
        "gmail" => send_login_email_gmail(&state, &staff, &login_url).await,
        _ => send_login_email_smtp(&state, &staff, &login_url).await,
    }
}

#[allow(clippy::unused_async)]
async fn send_login_email_smtp(
    state: &AppState,
    staff: &models::Staff,
    login_url: &str,
) -> (StatusCode, Json<serde_json::Value>) {
    use lettre::Transport;
    if state.config.smtp_host.is_empty() {
        warn!("SMTP not configured - login URL: {}", login_url);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                serde_json::json!({"error": "L'envoi d'email n'est pas configuré (SMTP). Contactez l'administrateur."}),
            ),
        );
    }

    let html_body = format!(
        r#"<p>Bonjour {},</p>
<p>Cliquez sur le lien ci-dessous pour vous connecter à PowPow :</p>
<p><a href="{}" style="display:inline-block;padding:12px 24px;background:#3273dc;color:white;text-decoration:none;border-radius:4px;">Se connecter</a></p>
<p>Ou copiez ce lien : {}</p>
<p>Sur la page suivante, cliquez sur « Se connecter » pour confirmer votre connexion.</p>
<p><em>Ce lien est à usage unique.</em></p>
{}"#,
        staff.first_name,
        login_url,
        login_url,
        crate::email_signature(&state.config.entity_name),
    );

    let from = match state.config.smtp_from.parse::<lettre::message::Mailbox>() {
        Ok(m) => m,
        Err(e) => {
            error!("Invalid SMTP_FROM address: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Invalid SMTP_FROM configuration"})),
            );
        }
    };

    let to_address = if state.config.mail_destination_override.is_empty() {
        &staff.email
    } else {
        warn!(
            "MAIL_DESTINATION_ADDRESS_OVERRIDE active: redirecting email from {} to {}",
            staff.email, state.config.mail_destination_override
        );
        &state.config.mail_destination_override
    };

    let to = match to_address.parse::<lettre::message::Mailbox>() {
        Ok(m) => m,
        Err(e) => {
            error!("Invalid destination email address: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid email address"})),
            );
        }
    };

    let email = match lettre::Message::builder()
        .from(from)
        .to(to)
        .subject(format!("{} — Connexion PowPow", state.config.entity_name))
        .header(lettre::message::header::ContentType::TEXT_HTML)
        .body(html_body)
    {
        Ok(m) => m,
        Err(e) => {
            error!("Error building email: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to build email"})),
            );
        }
    };

    let creds = lettre::transport::smtp::authentication::Credentials::new(
        state.config.smtp_user.clone(),
        state.config.smtp_password.clone(),
    );

    let mailer = match lettre::SmtpTransport::relay(&state.config.smtp_host) {
        Ok(builder) => builder
            .port(state.config.smtp_port)
            .credentials(creds)
            .build(),
        Err(e) => {
            error!("Error creating SMTP transport: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "SMTP configuration error"})),
            );
        }
    };

    match mailer.send(&email) {
        Ok(_) => {
            info!("Login email sent via SMTP to {}", staff.email);
            (StatusCode::OK, Json(serde_json::json!({"success": true})))
        }
        Err(e) => {
            error!("Error sending email via SMTP: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Échec de l'envoi de l'email"})),
            )
        }
    }
}

async fn send_login_email_gmail(
    state: &AppState,
    staff: &models::Staff,
    login_url: &str,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(client) = &state.gmail_client else {
        warn!("Gmail not configured - login URL: {}", login_url);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                serde_json::json!({"error": "L'envoi d'email n'est pas configuré (Gmail). Contactez l'administrateur."}),
            ),
        );
    };
    let client = client.clone();

    let from = if state.config.gmail_from.is_empty() {
        "me".to_string()
    } else {
        state.config.gmail_from.clone()
    };

    let to_address = if state.config.mail_destination_override.is_empty() {
        &staff.email
    } else {
        warn!(
            "MAIL_DESTINATION_ADDRESS_OVERRIDE active: redirecting email from {} to {}",
            staff.email, state.config.mail_destination_override
        );
        &state.config.mail_destination_override
    };

    // Build RFC 2822 message with HTML content
    let sig = crate::email_signature(&state.config.entity_name);
    let subject_text = format!("{} \u{2014} Connexion PowPow", state.config.entity_name);
    let encoded_subject = format!(
        "=?UTF-8?B?{}?=",
        base64::engine::general_purpose::STANDARD.encode(subject_text.as_bytes())
    );
    let raw_message = format!(
        "From: {from}\r\nTo: {to_address}\r\nSubject: {encoded_subject}\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n<p>Bonjour {first_name},</p>\n<p>Cliquez sur le lien ci-dessous pour vous connecter à PowPow :</p>\n<p><a href=\"{url}\" style=\"display:inline-block;padding:12px 24px;background:#3273dc;color:white;text-decoration:none;border-radius:4px;\">Se connecter</a></p>\n<p>Ou copiez ce lien : {url}</p>\n<p>Sur la page suivante, cliquez sur « Se connecter » pour confirmer votre connexion.</p>\n<p><em>Ce lien est à usage unique.</em></p>\n{sig}",
        first_name = staff.first_name,
        url = login_url,
    );

    let message_body = httpclient::InMemoryBody::Text(raw_message);

    match client.messages_send("me", message_body, None).await {
        Ok(_) => {
            info!("Login email sent via Gmail to {}", staff.email);
            (StatusCode::OK, Json(serde_json::json!({"success": true})))
        }
        Err(e) => {
            error!("Error sending email via Gmail: {}", e);
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
