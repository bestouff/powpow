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
    routing::{delete, get, post},
};
use axum_extra::extract::cookie::{Key, SignedCookieJar};
use base64::Engine;
use chrono::Datelike;
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{error, info, warn};

mod auth;
mod config;
mod database;
mod dicton;
mod helloasso;
mod mailchimp;
mod models;
mod news;
mod routes;
mod templates;

use config::AppConfig;
use helloasso::HelloAssoClient;
use mailchimp::MailchimpClient;

pub(crate) const POWPOW_CSS: &str = include_str!("../static/powpow.css");
pub(crate) const POWPOW_JS: &str = include_str!("../static/powpow.js");

/// Extract the URL prefix from `X-Forwarded-Prefix` header.
///
/// Only characters safe for a URL path segment (`a-z A-Z 0-9 / _ - .`) are
/// accepted; anything else causes the header to be ignored (returns `""`).
/// This prevents injection when the value is interpolated into HTML or JS.
pub(crate) fn get_prefix(headers: &HeaderMap) -> String {
    headers
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

/// Middleware that adds security response headers (CSP, X-Content-Type-Options,
/// X-Frame-Options, Referrer-Policy).
async fn security_headers(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let mut response = next.run(request).await;
    let h = response.headers_mut();
    h.insert(
        axum::http::header::HeaderName::from_static("content-security-policy"),
        axum::http::header::HeaderValue::from_static(
            "default-src 'self'; \
             script-src 'self' 'unsafe-inline' https://cdn.jsdelivr.net; \
             style-src 'self' 'unsafe-inline' https://cdn.jsdelivr.net; \
             img-src 'self' data:; \
             font-src 'self' https://cdn.jsdelivr.net; \
             frame-ancestors 'none'; \
             base-uri 'self'; \
             form-action 'self'",
        ),
    );
    h.insert(
        axum::http::header::HeaderName::from_static("x-content-type-options"),
        axum::http::header::HeaderValue::from_static("nosniff"),
    );
    h.insert(
        axum::http::header::HeaderName::from_static("x-frame-options"),
        axum::http::header::HeaderValue::from_static("DENY"),
    );
    h.insert(
        axum::http::header::HeaderName::from_static("referrer-policy"),
        axum::http::header::HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    response
}

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub helloasso_client: HelloAssoClient,
    pub mailchimp_client: MailchimpClient,
    pub config: AppConfig,
    pub cookie_key: Key,
    pub gmail_client: Option<std::sync::Arc<gmail::GmailClient>>,
    pub sync_in_progress: std::sync::Arc<std::sync::atomic::AtomicBool>,
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

/// Standard HTML signature block for outgoing emails.
pub(crate) fn email_signature(entity: &str) -> String {
    format!(
        "<p><em>— PowPow v{version} pour {entity} — le gestionnaire de station qui ne dort jamais</em></p>",
        version = env!("CARGO_PKG_VERSION"),
    )
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
                base64::engine::general_purpose::STANDARD.encode(subject.as_bytes())
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
    // Initialize tracing with a default of `info` level when RUST_LOG is not set
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Load configuration: try /etc/powpow.conf first, then .env as fallback
    dotenvy::from_path("/etc/powpow.conf").ok();
    dotenvy::dotenv().ok();
    let config = AppConfig::from_env()?;

    // Setup database
    let db = database::setup_database(&config.database_url).await?;
    let migrations_applied = database::run_migrations(&db).await?;

    // Audit: log application startup with version
    let version = env!("CARGO_PKG_VERSION");
    let _ = database::insert_audit(
        &db,
        None,
        "Système",
        "Démarrage application",
        &format!("PowPow v{version}"),
    )
    .await;

    // Audit: log database migrations if any were applied
    if migrations_applied > 0 {
        let _ = database::insert_audit(
            &db,
            None,
            "Système",
            "Migrations base de données",
            &format!("{migrations_applied} migration(s) appliquée(s)"),
        )
        .await;
    }

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
        sync_in_progress: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };

    // Clone state for background tasks before it moves into the router
    let state_for_weekly = app_state.clone();
    let state_for_preload = app_state.clone();

    // Pre-load the navbar content block so all pages show the logo
    if let Ok(Some(block)) = database::get_content(&app_state.db, "navbar").await {
        templates::set_navbar_block(Some(block));
    }

    // Pre-load the favicon content block so all pages show the favicon
    if let Ok(Some(block)) = database::get_content(&app_state.db, "favicon").await {
        templates::set_favicon_block(Some(block));
    }

    // Pre-load footer content blocks so all pages show footer info
    if let Ok(footer_map) = database::get_contents_by_slugs(
        &app_state.db,
        &["footer-contact", "footer-calendar", "footer-summer"],
    )
    .await
    {
        templates::set_footer_blocks(models::ContentMap::new(footer_map));
    }

    // Set entity name for footer and email signatures
    templates::set_entity_name(app_state.config.entity_name.clone());

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
        .route(
            "/api/person/{id}/unimport/{payment_id}",
            get(routes::staff::unimport_consequences).post(routes::staff::do_unimport),
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
        .route(
            "/restore",
            post(routes::admin::restore_database)
                .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024 * 1024)),
        )
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
        .route(
            "/qualifications",
            get(routes::admin::qualifications_page_handler),
        )
        .route("/settings", get(routes::settings::settings_page_handler))
        .route("/api/ateliers", post(routes::settings::api_create_atelier))
        .route(
            "/api/ateliers/{id}",
            post(routes::settings::api_update_atelier).delete(routes::settings::api_delete_atelier),
        )
        .route(
            "/api/qualifications",
            post(routes::admin::api_create_qualification),
        )
        .route(
            "/api/qualifications/{id}",
            delete(routes::admin::api_delete_qualification),
        )
        .route(
            "/api/staff-qualif",
            post(routes::admin::api_add_staff_qualif),
        )
        .route(
            "/api/staff-qualif/{id}",
            delete(routes::admin::api_delete_staff_qualif),
        )
        .route(
            "/api/my/staff-qualif",
            post(routes::staff::api_add_own_staff_qualif),
        )
        .route(
            "/api/my/staff-qualif/{id}",
            delete(routes::staff::api_delete_own_staff_qualif),
        )
        .route(
            "/api/staff-qualif/{id}/proof",
            post(routes::staff::upload_training_proof).delete(routes::staff::delete_training_proof),
        )
        .route(
            "/staff-qualif/{id}/proof",
            get(routes::staff::serve_training_proof),
        )
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
        .route("/contact", post(routes::home::contact_submit))
        .route("/privacy", get(routes::legal::privacy_page))
        .route("/tos", get(routes::legal::tos_page))
        .route("/photos", get(routes::photos::photo_page))
        .route("/photos/upload", post(routes::photos::upload_photo))
        .route("/photos/{id}", get(routes::photos::display_photo))
        .route("/photos/{id}/delete", post(routes::photos::delete_photo))
        .route(
            "/api/photos/{id}/frontpage",
            post(routes::photos::api_toggle_frontpage),
        )
        .route(
            "/api/photos/{id}/staff",
            post(routes::photos::api_toggle_staff),
        )
        .route("/api/photos/ids", get(routes::photos::api_photo_ids))
        .route(
            "/content-images/{id}",
            get(routes::content::serve_content_image),
        )
        .route("/news-images/{id}", get(routes::content::serve_news_image))
        .route("/admin/contents", get(routes::content::content_list))
        .route(
            "/admin/contents/{slug}",
            get(routes::content::content_edit).post(routes::content::content_save),
        )
        .route("/static/powpow.css", get(routes::legal::serve_css))
        .route("/static/powpow.js", get(routes::legal::serve_js))
        .layer(axum::extract::DefaultBodyLimit::max(50 * 1024 * 1024))
        .layer(axum::middleware::from_fn(security_headers))
        .with_state(app_state);

    // Spawn daily morning email task
    tokio::spawn(routes::background::weekly_morning_email_loop(
        state_for_weekly,
    ));

    // Spawn background preload loop:
    // - runs dicton + news sync once at startup
    // - re-syncs news every 15 minutes
    // - regenerates dicton daily at 5 AM
    tokio::spawn(routes::background::daily_preload_loop(state_for_preload));

    // Start server
    let listener = tokio::net::TcpListener::bind(&listen_address).await?;
    info!("Server running on {listen_address}");

    axum::serve(listener, app).await?;
    Ok(())
}
