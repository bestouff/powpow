#![allow(clippy::uninlined_format_args)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::format_push_string)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::format_collect)]

use axum::{
    Json, Router,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Key, SignedCookieJar};
use chrono::Datelike;
use sqlx::PgPool;
use std::collections::HashMap;
use tower_http::cors::CorsLayer;
use tracing::{error, info, warn};

mod auth;
mod config;
mod database;
mod helloasso;
mod mailchimp;
mod models;
mod routes;
mod templates;

use config::AppConfig;
use helloasso::HelloAssoClient;
use mailchimp::MailchimpClient;

pub(crate) const POWPOW_CSS: &str = include_str!("../static/powpow.css");
pub(crate) const POWPOW_JS: &str = include_str!("../static/powpow.js");

/// Extract the URL prefix from X-Forwarded-Prefix header
pub(crate) fn get_prefix(headers: &HeaderMap) -> String {
    headers
        .get("X-Forwarded-Prefix")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim_end_matches('/').to_string())
        .unwrap_or_default()
}

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub helloasso_client: HelloAssoClient,
    pub mailchimp_client: MailchimpClient,
    pub config: AppConfig,
    pub cookie_key: Key,
    pub gmail_client: Option<std::sync::Arc<gmail::GmailClient>>,
}

impl axum::extract::FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.cookie_key.clone()
    }
}

pub(crate) type Season = i16;

#[must_use]
pub fn get_current_season() -> Season {
    get_season_for(chrono::Utc::now())
}

#[must_use]
pub fn get_season_for(date: chrono::DateTime<chrono::Utc>) -> Season {
    // Season runs from June to May (e.g., June 2023 - May 2024 = season 2024)
    // Payments from June onwards are for the next year's season
    let year = date.year();
    let month = date.month();
    if month >= 6 {
        (year + 1) as Season
    } else {
        year as Season
    }
}

/// Check sync token from query param or Authorization header.
/// Returns the caller name if authorized, or an error response.
#[allow(clippy::result_large_err)]
pub(crate) fn check_automation_token(
    params: &HashMap<String, String>,
    headers: &HeaderMap,
    expected_token: &str,
    label: &str,
) -> Result<String, Response> {
    let provided = params.get("token").map(String::as_str).or_else(|| {
        headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
    });
    match provided {
        Some(t) if !expected_token.is_empty() && t == expected_token => {
            Ok(format!("Automation ({label})"))
        }
        _ => Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Unauthorized"})),
        )
            .into_response()),
    }
}

/// Resolve logged-in staff if they are an admin.
pub(crate) async fn resolve_staff_if_admin(
    jar: &SignedCookieJar,
    state: &AppState,
) -> Option<models::Staff> {
    let id = jar
        .get("aghil_session")
        .and_then(|c| c.value().parse::<uuid::Uuid>().ok())?;
    let staff = database::get_staff_by_id(&state.db, id).await.ok()??;
    if staff.is_admin || staff.is_god {
        Some(staff)
    } else {
        None
    }
}

pub(crate) async fn resolve_staff_if_god(
    jar: &SignedCookieJar,
    state: &AppState,
) -> Option<models::Staff> {
    let id = jar
        .get("aghil_session")
        .and_then(|c| c.value().parse::<uuid::Uuid>().ok())?;
    let staff = database::get_staff_by_id(&state.db, id).await.ok()??;
    if staff.is_god { Some(staff) } else { None }
}

pub(crate) async fn send_notification_email(
    state: &AppState,
    to_addresses: &[String],
    subject: &str,
    html_body: &str,
) {
    use lettre::Transport;
    if to_addresses.is_empty() {
        return;
    }

    let mail_method = if state.config.mail_method.eq_ignore_ascii_case("gmail") {
        "gmail"
    } else {
        "smtp"
    };

    for to_addr in to_addresses {
        if mail_method == "gmail" {
            let Some(client) = &state.gmail_client else {
                warn!(
                    "Gmail not configured, cannot send notification to {}",
                    to_addr
                );
                continue;
            };
            let client = client.clone();
            let from = if state.config.gmail_from.is_empty() {
                "me".to_string()
            } else {
                state.config.gmail_from.clone()
            };
            let dest = if state.config.mail_destination_override.is_empty() {
                to_addr.as_str()
            } else {
                warn!(
                    "MAIL_DESTINATION_ADDRESS_OVERRIDE active: redirecting notification from {} to {}",
                    to_addr, state.config.mail_destination_override
                );
                &state.config.mail_destination_override
            };
            let encoded_subject = format!(
                "=?UTF-8?B?{}?=",
                base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    subject.as_bytes()
                )
            );
            let raw_message = format!(
                "From: {}\r\nTo: {}\r\nSubject: {}\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n{}",
                from, dest, encoded_subject, html_body
            );
            let message_body = httpclient::InMemoryBody::Text(raw_message);
            match client.messages_send("me", message_body, None).await {
                Ok(_) => info!("Notification email sent via Gmail to {}", to_addr),
                Err(e) => error!(
                    "Failed to send notification via Gmail to {}: {}",
                    to_addr, e
                ),
            }
        } else {
            if state.config.smtp_host.is_empty() {
                warn!(
                    "SMTP not configured, cannot send notification to {}",
                    to_addr
                );
                continue;
            }
            let from = match state.config.smtp_from.parse::<lettre::message::Mailbox>() {
                Ok(m) => m,
                Err(e) => {
                    error!("Invalid SMTP_FROM address: {}", e);
                    continue;
                }
            };
            let dest = if state.config.mail_destination_override.is_empty() {
                to_addr.as_str()
            } else {
                warn!(
                    "MAIL_DESTINATION_ADDRESS_OVERRIDE active: redirecting notification from {} to {}",
                    to_addr, state.config.mail_destination_override
                );
                &state.config.mail_destination_override
            };
            let to = match dest.parse::<lettre::message::Mailbox>() {
                Ok(m) => m,
                Err(e) => {
                    error!("Invalid destination email {}: {}", dest, e);
                    continue;
                }
            };
            let email = match lettre::Message::builder()
                .from(from)
                .to(to)
                .subject(subject)
                .header(lettre::message::header::ContentType::TEXT_HTML)
                .body(html_body.to_string())
            {
                Ok(m) => m,
                Err(e) => {
                    error!("Failed to build notification email: {}", e);
                    continue;
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
                    continue;
                }
            };
            match mailer.send(&email) {
                Ok(_) => info!("Notification email sent via SMTP to {}", to_addr),
                Err(e) => error!("Failed to send notification via SMTP to {}: {}", to_addr, e),
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load configuration: try /etc/powpow.conf first, then .env as fallback
    dotenvy::from_path("/etc/powpow.conf").ok();
    dotenvy::dotenv().ok();
    let config = AppConfig::from_env()?;

    // Setup database
    let db = database::setup_database(&config.database_url).await?;
    database::run_migrations(&db).await?;

    // Clone config for AppState before partial moves
    let app_config = config.clone();

    // Setup HelloAsso client
    let helloasso_client = HelloAssoClient::new(
        config.helloasso_client_id,
        config.helloasso_client_secret,
        config.helloasso_association_slug,
    );

    // Setup Mailchimp client
    let mailchimp_client = MailchimpClient::new(
        config.mailchimp_api_key,
        config.mailchimp_server_prefix,
        config.mailchimp_list_id,
        config.mailchimp_from_name,
        config.mailchimp_from_email,
    );
    if mailchimp_client.is_configured() {
        info!("Mailchimp client configured");
    } else {
        warn!("Mailchimp client not configured (set MAILCHIMP_* env vars)");
    }

    // Build cookie signing key from secret (or generate a random one if not configured)
    let cookie_key = if app_config.cookie_secret.len() >= 64 {
        Key::from(app_config.cookie_secret.as_bytes())
    } else {
        if app_config.cookie_secret.is_empty() {
            warn!(
                "COOKIE_SECRET not configured - generating a random key (sessions won't survive restarts)"
            );
        } else {
            warn!("COOKIE_SECRET too short (need >= 64 chars) - generating a random key");
        }
        Key::generate()
    };

    let gmail_client = if !app_config.gmail_access_token.is_empty()
        && !app_config.gmail_refresh_token.is_empty()
    {
        let client = gmail::GmailClient::with_auth(gmail::GmailAuth::oauth2(
            &app_config.gmail_access_token,
            &app_config.gmail_refresh_token,
            Some(Box::new(|data: httpclient::oauth2::RefreshData| {
                info!(
                    "Gmail OAuth token refreshed (expires_in={}s, new_refresh_token={})",
                    data.expires_in,
                    data.refresh_token.is_some()
                );
            })),
        ));
        info!("Gmail client initialized with OAuth2");
        Some(std::sync::Arc::new(client))
    } else {
        warn!("Gmail client not configured (missing GMAIL_ACCESS_TOKEN or GMAIL_REFRESH_TOKEN)");
        None
    };

    let listen_address = app_config.listen_address.clone();

    let app_state = AppState {
        db,
        helloasso_client,
        mailchimp_client,
        config: app_config,
        cookie_key,
        gmail_client,
    };

    // Clone state for background task before it moves into the router
    let state_for_weekly = app_state.clone();

    // Build router
    let app = Router::new()
        .route("/", get(routes::home::index))
        .route("/admin", get(routes::admin::admin_page_handler))
        .route("/online", get(routes::membership::list_users))
        .route("/online/{id}", get(routes::membership::get_user))
        .route("/staff", get(routes::staff::list_staff))
        .route("/person/{id}", get(routes::staff::view_person))
        .route("/api/person/{id}/role", post(routes::staff::toggle_role))
        .route(
            "/api/person/{id}/comment",
            post(routes::staff::update_comment),
        )
        .route(
            "/api/person/{id}/contact",
            post(routes::staff::update_contact),
        )
        .route("/import/{item_id}", get(routes::membership::import_staff))
        .route(
            "/import/{item_id}",
            post(routes::membership::do_import_staff),
        )
        .route(
            "/cash",
            get(routes::cash::list_cash).post(routes::cash::create_cash),
        )
        .route(
            "/cash-import/{id}",
            get(routes::cash::import_cash).post(routes::cash::do_import_cash),
        )
        .route(
            "/sync",
            get(routes::sync::sync_users).post(routes::sync::sync_webhook),
        )
        .route("/export/mailchimp", get(routes::admin::export_mailchimp))
        .route("/backup", get(routes::admin::backup_database))
        .route("/restore", get(routes::admin::restore_page))
        .route("/restore", post(routes::admin::restore_database))
        .route("/api/online", get(routes::membership::api_list_users))
        .route("/api/sync", post(routes::sync::api_sync_users))
        .route("/api/stats", get(routes::admin::api_get_stats))
        .route("/api/debug/order", get(routes::admin::debug_first_order))
        .route("/api/badge-counts", get(routes::home::api_badge_counts))
        .route(
            "/api/equipment/{id}",
            post(routes::admin::api_cycle_equipment),
        )
        .route("/calendar", get(routes::calendar::calendar_landing))
        .route("/calendar/", get(routes::calendar::calendar_landing))
        .route(
            "/api/calendar/needs",
            get(routes::calendar::api_get_needs)
                .post(routes::calendar::api_upsert_need)
                .delete(routes::calendar::api_delete_need),
        )
        .route(
            "/api/calendar/needs-by-day",
            get(routes::calendar::api_get_needs_by_day),
        )
        .route(
            "/api/calendar/need-days",
            get(routes::calendar::api_get_need_days),
        )
        .route("/calendar/{slug}", get(routes::calendar::calendar_view))
        .route(
            "/api/calendar/toggle",
            post(routes::calendar::toggle_presence_api),
        )
        .route(
            "/api/calendar/opening-day",
            post(routes::calendar::api_create_opening_day),
        )
        .route(
            "/api/calendar/opening-day/status",
            post(routes::calendar::api_update_opening_day_status),
        )
        .route(
            "/api/admin/flags",
            post(routes::admin::api_update_admin_flags),
        )
        .route("/audit", get(routes::admin::audit_page_handler))
        .route("/validation", get(routes::admin::validation_page_handler))
        .route("/login", get(routes::auth::login_page))
        .route("/api/staff/search", get(routes::auth::api_search_staff))
        .route(
            "/api/staff/create-minimal",
            post(routes::staff::api_create_staff_minimal),
        )
        .route("/api/login/send", post(routes::auth::api_send_login_email))
        .route("/api/me", get(routes::auth::api_me))
        .route("/logout", get(routes::auth::logout))
        .route("/health", get(routes::home::health_check))
        .route("/privacy", get(routes::static_pages::privacy_page))
        .route("/tos", get(routes::static_pages::tos_page))
        .route("/photos", get(routes::photos::photo_page))
        .route("/photos/upload", post(routes::photos::upload_photo))
        .route("/photos/{id}", get(routes::photos::display_photo))
        .route("/photos/{id}/delete", post(routes::photos::delete_photo))
        .route("/api/photos/ids", get(routes::photos::api_photo_ids))
        .route(
            "/content-images/{id}",
            get(routes::content::serve_content_image),
        )
        .route("/admin/contents", get(routes::content::content_list))
        .route(
            "/admin/contents/{slug}",
            get(routes::content::content_edit).post(routes::content::content_save),
        )
        .route("/static/powpow.css", get(routes::static_pages::serve_css))
        .route("/static/powpow.js", get(routes::static_pages::serve_js))
        .layer(axum::extract::DefaultBodyLimit::max(50 * 1024 * 1024))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    // Spawn daily morning email task
    tokio::spawn(routes::background::weekly_morning_email_loop(
        state_for_weekly,
    ));

    // Start server
    let listener = tokio::net::TcpListener::bind(&listen_address).await?;
    info!("Server running on {listen_address}");

    axum::serve(listener, app).await?;
    Ok(())
}
