use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::SignedCookieJar;
use serde::Deserialize;
use tracing::{error, info, warn};

use crate::{AppState, database, get_prefix, models, templates};

#[derive(Debug, Deserialize)]
pub struct StaffSearchQuery {
    q: Option<String>,
}

pub async fn login_page(headers: HeaderMap) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    templates::login_page(&prefix)
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
    let token = match database::set_staff_token(&state.db, staff.id).await {
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
<p>Cliquez sur le lien ci-dessous pour vous connecter à AGHIL :</p>
<p><a href="{}" style="display:inline-block;padding:12px 24px;background:#3273dc;color:white;text-decoration:none;border-radius:4px;">Se connecter</a></p>
<p>Ou copiez ce lien : {}</p>
<p><em>Ce lien est à usage unique.</em></p>"#,
        staff.first_name, login_url, login_url
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
        .subject("Connexion AGHIL")
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
    let raw_message = format!(
        "From: {from}\r\nTo: {to_address}\r\nSubject: Connexion AGHIL\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n<p>Bonjour {first_name},</p>\n<p>Cliquez sur le lien ci-dessous pour vous connecter à AGHIL :</p>\n<p><a href=\"{url}\" style=\"display:inline-block;padding:12px 24px;background:#3273dc;color:white;text-decoration:none;border-radius:4px;\">Se connecter</a></p>\n<p>Ou copiez ce lien : {url}</p>\n<p><em>Ce lien est à usage unique.</em></p>",
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
    let staff_id = match jar.get("aghil_session") {
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

    match database::get_staff_by_id(&state.db, staff_id).await {
        Ok(Some(staff)) => {
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
                    "is_admin": staff.is_admin,
                    "is_god": staff.is_god,
                    "is_chief": is_chief,
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

pub async fn logout(headers: HeaderMap, jar: SignedCookieJar) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    let mut cookie = axum_extra::extract::cookie::Cookie::new("aghil_session", "");
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
    cookie.set_secure(true);
    cookie.set_max_age(time::Duration::ZERO);
    let updated_jar = jar.remove(cookie);
    (updated_jar, Redirect::to(&format!("{}/", prefix)))
}
