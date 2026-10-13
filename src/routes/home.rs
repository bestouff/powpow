use axum::{
    Json,
    extract::{Query, State},
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
    let session_id = jar
        .get("aghil_session")?
        .value()
        .parse::<uuid::Uuid>()
        .ok()?;
    database::get_session_staff(&state.db, session_id)
        .await
        .ok()
        .flatten()
}

#[derive(Default, serde::Deserialize)]
pub struct HomeQuery {
    #[serde(default)]
    membership_required: bool,
}

pub async fn index(
    headers: HeaderMap,
    jar: SignedCookieJar,
    State(state): State<AppState>,
    Query(query): Query<HomeQuery>,
) -> impl IntoResponse {
    const HOW_MANY_NEWS: i64 = 6;

    let prefix = get_prefix(&headers);
    let logged_in = match resolve_caller(&jar, &state).await {
        Some(staff) => {
            staff.is_admin
                || staff.is_god
                || database::has_current_membership(&state.db, staff.id)
                    .await
                    .unwrap_or(false)
        }
        None => false,
    };

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

    let dicton = dicton::cached();

    // Fetch news items from the database (synced by background task)
    // (Add 1 for the fake title news)
    let news_items = database::get_recent_news(&state.db, HOW_MANY_NEWS + 1)
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
        query.membership_required,
        &state.config.helloasso_association_slug,
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
    if !state
        .mail_limits
        .allow("contact:global", 10, std::time::Duration::from_secs(60))
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({"error": "Trop de messages. Réessayez dans une minute."})),
        );
    }
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

    if !crate::auto_import::valid_email(email) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Adresse email invalide"})),
        );
    }

    // Build the HTML email body
    let entity = &state.config.entity_name;
    let sig = email_signature(entity);
    let html_body = contact_body(name, email, subject, message, &sig);

    let full_subject = format!("[Contact {}] {}", entity, subject);

    info!(
        "Sending contact email from {} <{}> to {}",
        name, email, state.config.entity_email
    );

    if let Err(e) = send_contact_email(
        &state,
        name,
        email,
        &state.config.entity_email.clone(),
        &full_subject,
        &html_body,
    )
    .await
    {
        error!("Contact delivery failed: {e}");
        return (
            StatusCode::BAD_GATEWAY,
            Json(
                serde_json::json!({"error": "Le message n'a pas pu être envoyé. Veuillez réessayer plus tard."}),
            ),
        );
    }

    (StatusCode::OK, Json(serde_json::json!({"ok": true})))
}

fn contact_body(name: &str, email: &str, subject: &str, message: &str, signature: &str) -> String {
    maud::html! {
        h3 { "Message via le formulaire de contact" }
        p { strong { "De : " } (name) " <" (email) ">" }
        p { strong { "Objet : " } (subject) }
        hr;
        div { @for line in message.lines() { (line) br; } }
        hr;
        (maud::PreEscaped(signature))
    }
    .into_string()
}

#[cfg(test)]
mod contact_tests {
    #[test]
    fn contact_inputs_cannot_inject_headers_or_html() {
        assert!(!crate::auto_import::valid_email(
            "a@b.org\r\nBcc: other@example.org"
        ));
        let attack = "<img src=x onerror=alert(1)>";
        let body = super::contact_body(attack, "a@b.org", attack, attack, "");
        assert!(!body.contains("<img"));
        assert!(body.contains("&lt;img"));
    }
}

/// Use the visitor's typed mailbox only as Reply-To.
async fn send_contact_email(
    state: &AppState,
    sender_name: &str,
    sender_email: &str,
    to_addr: &str,
    subject: &str,
    html_body: &str,
) -> anyhow::Result<()> {
    let reply_to =
        lettre::message::Mailbox::new(Some(sender_name.to_string()), sender_email.parse()?);
    crate::personal_email::send_with_reply(
        state,
        to_addr,
        &state.config.entity_name,
        subject,
        html_body.to_string(),
        Some(reply_to),
    )
    .await
}
