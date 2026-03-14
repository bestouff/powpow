use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use axum_extra::extract::cookie::SignedCookieJar;
use tracing::{error, info, warn};

use crate::{
    AppState, database, dicton, email_signature, get_current_season, get_prefix,
    models::ContentMap, templates,
};

/// Resolve the caller from the session cookie, if any.
async fn resolve_caller(jar: &SignedCookieJar, state: &AppState) -> Option<crate::models::Staff> {
    let id = jar
        .get("aghil_session")?
        .value()
        .parse::<uuid::Uuid>()
        .ok()?;
    database::get_staff_by_id(&state.db, id)
        .await
        .ok()
        .flatten()
}

pub async fn index(
    headers: HeaderMap,
    jar: SignedCookieJar,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    let current_season = get_current_season();
    let logged_in = resolve_caller(&jar, &state).await.is_some();

    // Public frontpage data
    let equipments = database::get_all_equipments(&state.db)
        .await
        .unwrap_or_default();
    let station_open = database::is_station_open_today(&state.db)
        .await
        .unwrap_or(false);
    let photo_ids = database::get_all_photo_ids(&state.db)
        .await
        .unwrap_or_default();
    let staff_photo_ids = database::get_staff_photo_ids(&state.db)
        .await
        .unwrap_or_default();
    let contents = ContentMap::new(
        database::get_all_contents(&state.db)
            .await
            .unwrap_or_default(),
    );

    // Generate (or retrieve from cache) the "dicton du jour"
    let dicton =
        dicton::get_or_generate(&state.db, current_season, &state.config.huggingface_token).await;

    // Fetch news items from the database (synced by background task)
    let news_items = database::get_recent_news(&state.db, 6)
        .await
        .unwrap_or_default();

    templates::index(
        &prefix,
        &equipments,
        station_open,
        &photo_ids,
        &staff_photo_ids,
        &contents,
        dicton.as_deref(),
        &news_items,
        logged_in,
    )
}

pub async fn api_badge_counts(
    State(state): State<AppState>,
    jar: SignedCookieJar,
) -> impl IntoResponse {
    let current_season = get_current_season();
    let users = database::count_unimported_memberships(&state.db, current_season)
        .await
        .unwrap_or(0);
    let cash = database::count_unimported_cash(&state.db)
        .await
        .unwrap_or(0);

    // Compute validation count based on caller role
    let caller = resolve_caller(&jar, &state).await;
    let validations = match &caller {
        Some(s) if s.is_admin || s.is_god => {
            // Admins/gods see all pending validations
            database::count_pending_validations(&state.db, None)
                .await
                .unwrap_or(0)
        }
        Some(s) => {
            // Chiefs see only their ateliers' pending validations
            let is_chief = database::is_chief(&state.db, s.id).await.unwrap_or(false);
            if is_chief {
                database::count_pending_validations(&state.db, Some(s.id))
                    .await
                    .unwrap_or(0)
            } else {
                0
            }
        }
        None => 0,
    };

    let admin_total = users + cash + validations;

    Json(serde_json::json!({
        "online": users,
        "cash": cash,
        "validations": validations,
        "admin": admin_total,
    }))
}

pub async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    // Check database connection
    match sqlx::query("SELECT 1").fetch_one(&state.db).await {
        Ok(_) => {
            // Check HelloAsso client configuration
            if !state.helloasso_client.is_configured() {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({
                        "status": "degraded",
                        "database": "ok",
                        "helloasso": "misconfigured",
                        "message": "HelloAsso client is misconfigured"
                    })),
                )
                    .into_response();
            }

            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "status": "healthy",
                    "database": "ok",
                    "helloasso": "configured",
                    "timestamp": chrono::Utc::now()
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Database health check failed: {}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "status": "unhealthy",
                    "database": "failed",
                    "helloasso": "unknown",
                    "error": e.to_string()
                })),
            )
                .into_response()
        }
    }
}

/// JSON payload for the contact form.
#[derive(serde::Deserialize)]
pub struct ContactForm {
    pub name: String,
    pub email: String,
    pub subject: String,
    pub message: String,
}

/// Handle POST /contact — send an email to `ENTITY_EMAIL` on behalf of the visitor.
///
/// The `From` display-name is set to the sender's name from the form while the
/// actual `From` address remains the configured SMTP/Gmail address (required by
/// most providers).  The sender's email is placed in `Reply-To` so that hitting
/// "Reply" in the mail client reaches the right person.
pub async fn contact_submit(
    State(state): State<AppState>,
    Json(form): Json<ContactForm>,
) -> impl IntoResponse {
    if state.config.entity_email.is_empty() {
        warn!("Contact form submitted but ENTITY_EMAIL is not configured");
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Adresse de contact non configurée"})),
        );
    }

    let name = form.name.trim();
    let email = form.email.trim();
    let subject = form.subject.trim();
    let message = form.message.trim();

    // Basic validation
    if name.is_empty() || email.is_empty() || subject.is_empty() || message.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Tous les champs sont obligatoires"})),
        );
    }

    if !email.contains('@') || !email.contains('.') {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Adresse email invalide"})),
        );
    }

    // Build the HTML email body
    let entity = &state.config.entity_name;
    let sig = email_signature(entity);
    let html_body = format!(
        "<h3>Message via le formulaire de contact</h3>\
         <p><strong>De :</strong> {name} &lt;{email}&gt;</p>\
         <p><strong>Objet :</strong> {subject}</p>\
         <hr>\
         <div>{message_html}</div>\
         <hr>\
         {sig}",
        name = name,
        email = email,
        subject = subject,
        message_html = message.replace('\n', "<br>"),
        sig = sig,
    );

    let full_subject = format!("[Contact {}] {}", entity, subject);

    info!(
        "Sending contact email from {} <{}> to {}",
        name, email, state.config.entity_email
    );

    send_contact_email(
        &state,
        name,
        email,
        &state.config.entity_email.clone(),
        &full_subject,
        &html_body,
    )
    .await;

    (StatusCode::OK, Json(serde_json::json!({"ok": true})))
}

/// Send an email with `From` display-name set to `sender_name` and `Reply-To`
/// set to `sender_email`.  The actual envelope `From` address stays the
/// configured SMTP / Gmail account.
async fn send_contact_email(
    state: &AppState,
    sender_name: &str,
    sender_email: &str,
    to_addr: &str,
    subject: &str,
    html_body: &str,
) {
    use lettre::Transport;

    let mail_method = if state.config.mail_method.eq_ignore_ascii_case("gmail") {
        "gmail"
    } else {
        "smtp"
    };

    let dest = if state.config.mail_destination_override.is_empty() {
        to_addr
    } else {
        warn!(
            "MAIL_DESTINATION_ADDRESS_OVERRIDE active: redirecting contact from {} to {}",
            to_addr, state.config.mail_destination_override
        );
        &state.config.mail_destination_override
    };

    if mail_method == "gmail" {
        let Some(client) = &state.gmail_client else {
            warn!(
                "Gmail not configured, cannot send contact email to {}",
                dest
            );
            return;
        };
        let client = client.clone();
        let from_addr = if state.config.gmail_from.is_empty() {
            "me".to_string()
        } else {
            state.config.gmail_from.clone()
        };
        let encoded_subject = format!(
            "=?UTF-8?B?{}?=",
            base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                subject.as_bytes(),
            )
        );
        let encoded_name = format!(
            "=?UTF-8?B?{}?=",
            base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                sender_name.as_bytes(),
            )
        );
        let raw_message = format!(
            "From: {} <{}>\r\nReply-To: {}\r\nTo: {}\r\nSubject: {}\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n{}",
            encoded_name, from_addr, sender_email, dest, encoded_subject, html_body
        );
        let message_body = httpclient::InMemoryBody::Text(raw_message);
        match client.messages_send("me", message_body, None).await {
            Ok(_) => info!("Contact email sent via Gmail to {}", dest),
            Err(e) => error!("Failed to send contact email via Gmail to {}: {}", dest, e),
        }
    } else {
        if state.config.smtp_host.is_empty() {
            warn!("SMTP not configured, cannot send contact email to {}", dest);
            return;
        }
        // Parse the configured SMTP_FROM address, then override the display name
        let base_from = match state.config.smtp_from.parse::<lettre::message::Mailbox>() {
            Ok(m) => m,
            Err(e) => {
                error!("Invalid SMTP_FROM address: {}", e);
                return;
            }
        };
        let from = lettre::message::Mailbox::new(Some(sender_name.to_string()), base_from.email);
        let to = match dest.parse::<lettre::message::Mailbox>() {
            Ok(m) => m,
            Err(e) => {
                error!("Invalid destination email {}: {}", dest, e);
                return;
            }
        };
        let reply_to = match sender_email.parse::<lettre::message::Mailbox>() {
            Ok(m) => m,
            Err(e) => {
                error!("Invalid reply-to email {}: {}", sender_email, e);
                return;
            }
        };
        let email = match lettre::Message::builder()
            .from(from)
            .reply_to(reply_to)
            .to(to)
            .subject(subject)
            .header(lettre::message::header::ContentType::TEXT_HTML)
            .body(html_body.to_string())
        {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to build contact email: {}", e);
                return;
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
                error!("SMTP transport error: {}", e);
                return;
            }
        };
        match mailer.send(&email) {
            Ok(_) => info!("Contact email sent via SMTP to {}", dest),
            Err(e) => error!("Failed to send contact email via SMTP to {}: {}", dest, e),
        }
    }
}
