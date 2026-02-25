#![allow(clippy::uninlined_format_args)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::format_push_string)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::format_collect)]

use axum::{
    Json, Router,
    body::Body,
    extract::{Multipart, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Key, SignedCookieJar};
use chrono::{Datelike, TimeDelta};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use tower_http::cors::CorsLayer;
use tracing::{debug, error, info, warn};

mod auth;
mod config;
mod database;
mod helloasso;
mod mailchimp;
mod models;
mod templates;

use auth::{RequireAdmin, RequireChief, RequireGod, RequireStaff};
use config::AppConfig;
use helloasso::HelloAssoClient;
use mailchimp::MailchimpClient;
use models::User;

/// Extract the URL prefix from X-Forwarded-Prefix header
fn get_prefix(headers: &HeaderMap) -> String {
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

    // Set photo-of-the-day background for all pages
    if let Ok(Some((photo, name))) = database::get_photo_of_the_day(&app_state.db).await {
        templates::set_photo_bg(format!("/photos/{}", photo.id), name);
        info!("Photo of the day: {}", photo.id);
    }

    // Clone state for background task before it moves into the router
    let state_for_weekly = app_state.clone();

    // Build router
    let app = Router::new()
        .route("/", get(index))
        .route("/users", get(list_users))
        .route("/users/{id}", get(get_user))
        .route("/staff", get(list_staff))
        .route("/person/{id}", get(view_person))
        .route("/api/person/{id}/role", post(toggle_role))
        .route("/api/person/{id}/comment", post(update_comment))
        .route("/api/person/{id}/contact", post(update_contact))
        .route("/import/{item_id}", get(import_staff))
        .route("/import/{item_id}", post(do_import_staff))
        .route("/cash", get(list_cash).post(create_cash))
        .route("/cash-import/{id}", get(import_cash).post(do_import_cash))
        .route("/sync", get(sync_users).post(sync_users))
        .route("/export/mailchimp", get(export_mailchimp))
        .route("/backup", get(backup_database))
        .route("/restore", get(restore_page))
        .route("/restore", post(restore_database))
        .route("/api/users", get(api_list_users))
        .route("/api/sync", post(api_sync_users))
        .route("/api/stats", get(api_get_stats))
        .route("/api/debug/order", get(debug_first_order))
        .route("/api/badge-counts", get(api_badge_counts))
        .route("/calendar", get(calendar_landing))
        .route("/calendar/", get(calendar_landing))
        .route(
            "/api/calendar/needs",
            get(api_get_needs)
                .post(api_upsert_need)
                .delete(api_delete_need),
        )
        .route("/api/calendar/needs-by-day", get(api_get_needs_by_day))
        .route("/api/calendar/need-days", get(api_get_need_days))
        .route("/calendar/{slug}", get(calendar_view))
        .route("/api/calendar/toggle", post(toggle_presence_api))
        .route("/api/admin/flags", post(api_update_admin_flags))
        .route("/audit", get(audit_page_handler))
        .route("/validation", get(validation_page_handler))
        .route("/login", get(login_page))
        .route("/api/staff/search", get(api_search_staff))
        .route("/api/staff/create-minimal", post(api_create_staff_minimal))
        .route("/api/login/send", post(api_send_login_email))
        .route("/api/me", get(api_me))
        .route("/logout", get(logout))
        .route("/health", get(health_check))
        .route("/privacy", get(privacy_page))
        .route("/tos", get(tos_page))
        .route("/photos", get(photo_page))
        .route("/photos/upload", post(upload_photo))
        .route("/photos/{id}", get(display_photo))
        .route("/photos/{id}/delete", post(delete_photo))
        .layer(axum::extract::DefaultBodyLimit::max(50 * 1024 * 1024))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    // Spawn daily morning email task
    tokio::spawn(weekly_morning_email_loop(state_for_weekly));

    // Start server
    let listener = tokio::net::TcpListener::bind(&listen_address).await?;
    info!("Server running on {listen_address}");

    axum::serve(listener, app).await?;
    Ok(())
}

async fn index(
    headers: HeaderMap,
    State(state): State<AppState>,
    jar: SignedCookieJar,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);

    // Try to identify the logged-in user (anonymous access is fine)
    let staff = jar
        .get("aghil_session")
        .and_then(|c| c.value().parse::<uuid::Uuid>().ok());

    let staff = match staff {
        Some(id) => database::get_staff_by_id(&state.db, id)
            .await
            .ok()
            .flatten(),
        None => None,
    };

    let current_season = get_current_season();

    // Gather data depending on privilege level
    let has_paid = if let Some(ref s) = staff {
        database::has_staff_paid_season(&state.db, s.id, current_season)
            .await
            .unwrap_or(false)
    } else {
        false
    };

    let chief_ateliers = if let Some(ref s) = staff {
        if s.is_admin || s.is_god {
            // Admins/gods see all ateliers
            database::get_all_ateliers(&state.db)
                .await
                .unwrap_or_default()
        } else {
            database::get_chief_ateliers(&state.db, s.id)
                .await
                .unwrap_or_default()
        }
    } else {
        Vec::new()
    };

    let today = chrono::Local::now().date_naive();
    let week_end = today + chrono::Duration::days(7);
    let upcoming = database::get_upcoming_needs_deficit(&state.db, today, week_end)
        .await
        .unwrap_or_default();

    Html(templates::index(
        &prefix,
        staff.as_ref(),
        current_season,
        has_paid,
        &chief_ateliers,
        &upcoming,
    ))
}

async fn list_users(
    RequireAdmin(_staff): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    let search = params.get("search").filter(|s| !s.is_empty());
    let only_not_imported = params.get("filter").is_some_and(|f| f == "not_imported");

    let imported_result = database::get_all_imported_item_ids(&state.db).await;
    let memberships_result =
        database::get_all_memberships_filtered(&state.db, search.map(String::as_str)).await;

    match (memberships_result, imported_result) {
        (Ok(memberships_with_users), Ok(imported_set)) => {
            // Transform memberships to include staff import status and count stats
            let mut memberships_with_status = Vec::new();
            let mut total_count = 0;
            let mut imported_count = 0;
            let mut not_imported_count = 0;

            // Track imported memberships by (normalized_name, season) to detect doubles
            let mut imported_by_name_season: HashMap<(String, i16), Vec<usize>> = HashMap::new();

            for (user, membership) in memberships_with_users {
                // Calculate season for this membership
                let season = if let Some(order_date) = membership.order_date {
                    get_season_for(order_date)
                } else {
                    get_current_season()
                };

                // Check if staff exists for this membership+season (batch lookup)
                let has_staff = imported_set.contains(&(membership.helloasso_item_id, season));

                // Update stats
                total_count += 1;
                if has_staff {
                    imported_count += 1;
                } else {
                    not_imported_count += 1;
                }

                // Apply filter
                if only_not_imported && has_staff {
                    continue;
                }

                let idx = memberships_with_status.len();

                // Track imported memberships by name+season for double detection
                if has_staff {
                    let normalized_name = format!(
                        "{} {}",
                        membership
                            .beneficiary_first_name
                            .as_deref()
                            .unwrap_or("")
                            .trim()
                            .to_lowercase(),
                        membership
                            .beneficiary_last_name
                            .as_deref()
                            .unwrap_or("")
                            .trim()
                            .to_lowercase()
                    );
                    imported_by_name_season
                        .entry((normalized_name, season))
                        .or_default()
                        .push(idx);
                }

                memberships_with_status.push((
                    user,
                    models::MembershipWithStatus {
                        membership,
                        season,
                        has_staff,
                        is_double_subscription: false, // Will be updated below
                    },
                ));
            }

            // Mark double subscriptions (same name+season imported multiple times)
            for indices in imported_by_name_season.values() {
                if indices.len() > 1 {
                    for &idx in indices {
                        memberships_with_status[idx].1.is_double_subscription = true;
                    }
                }
            }

            // Sort: not-yet-imported first, then by date (most recent first)
            memberships_with_status.sort_by(|a, b| {
                // First compare by has_staff (false < true, so not imported comes first)
                match a.1.has_staff.cmp(&b.1.has_staff) {
                    std::cmp::Ordering::Equal => {
                        // Then by date descending (most recent first)
                        b.1.membership.order_date.cmp(&a.1.membership.order_date)
                    }
                    other => other,
                }
            });

            Html(templates::membership_list_with_filters(
                memberships_with_status,
                search.cloned(),
                only_not_imported,
                total_count,
                imported_count,
                not_imported_count,
                get_current_season(),
                &prefix,
            ))
        }
        (Err(e), _) | (_, Err(e)) => {
            error!("Error fetching memberships: {}", e);
            Html(format!("<p>Error loading memberships: {}</p>", e))
        }
    }
}

async fn list_staff(
    RequireStaff(viewer): RequireStaff,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    let current_season = get_current_season();

    // Chiefs and admins can see contact info (email/phone)
    let show_contact = viewer.is_admin
        || viewer.is_god
        || database::is_chief(&state.db, viewer.id)
            .await
            .unwrap_or(false);

    let staff_list = match database::get_all_staff_with_season(&state.db).await {
        Ok(list) => list,
        Err(e) => {
            error!("Error fetching staff: {}", e);
            return Html(format!("<p>Error loading staff: {}</p>", e));
        }
    };

    let ateliers = match database::get_all_ateliers(&state.db).await {
        Ok(list) => list,
        Err(e) => {
            error!("Error fetching ateliers: {}", e);
            return Html(format!("<p>Error loading ateliers: {}</p>", e));
        }
    };

    let roles = match database::get_all_roles(&state.db).await {
        Ok(list) => list,
        Err(e) => {
            error!("Error fetching roles: {}", e);
            return Html(format!("<p>Error loading roles: {}</p>", e));
        }
    };

    // Sort staff list: chiefs or gods first, then by name
    let mut staff_list = staff_list;
    staff_list.sort_by(|(staff_a, _), (staff_b, _)| {
        let a_priority = staff_a.is_god || roles.iter().any(|r| r.staff == staff_a.id && r.chief);
        let b_priority = staff_b.is_god || roles.iter().any(|r| r.staff == staff_b.id && r.chief);
        match (a_priority, b_priority) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => staff_a.last_name.cmp(&staff_b.last_name),
        }
    });

    Html(templates::staff_list(
        staff_list,
        &ateliers,
        &roles,
        current_season,
        &prefix,
        show_contact,
    ))
}

#[derive(Debug, Deserialize)]
struct PersonQuery {
    token: Option<uuid::Uuid>,
}

async fn view_person(
    headers: HeaderMap,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
    Query(query): Query<PersonQuery>,
    jar: SignedCookieJar,
) -> Response {
    let prefix = get_prefix(&headers);

    // If a login token is present, verify and set session cookie
    if let Some(token) = query.token {
        match database::verify_and_clear_token(&state.db, id, token).await {
            Ok(Some(_staff)) => {
                // Token valid: set session cookie and redirect to clean URL
                let mut cookie =
                    axum_extra::extract::cookie::Cookie::new("aghil_session", id.to_string());
                cookie.set_path("/");
                cookie.set_http_only(true);
                cookie.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
                cookie.set_max_age(time::Duration::days(30));
                let updated_jar = jar.add(cookie);
                return (
                    updated_jar,
                    Redirect::to(&format!("{}/person/{}", prefix, id)),
                )
                    .into_response();
            }
            Ok(None) => {
                // Token invalid: fall through to normal page render
                warn!("Invalid login token for staff {}", id);
            }
            Err(e) => {
                error!("Error verifying token: {}", e);
            }
        }
    }

    // Require a valid session (Staff level) for viewing person pages
    let Some(viewer_id) = jar
        .get("aghil_session")
        .and_then(|c| c.value().parse::<uuid::Uuid>().ok())
    else {
        return Redirect::to(&format!("{}/login", prefix)).into_response();
    };

    let current_season = get_current_season();

    // Get staff info
    let staff = match database::get_staff_by_id(&state.db, id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Html("<p>Staff not found</p>".to_string()),
            )
                .into_response();
        }
        Err(e) => {
            error!("Error fetching staff: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("<p>Error: {}</p>", e)),
            )
                .into_response();
        }
    };

    // Determine viewer permissions
    let is_self = viewer_id == id;
    let (is_viewer_admin, is_viewer_chief) = if is_self {
        (
            staff.is_admin,
            database::is_chief(&state.db, viewer_id)
                .await
                .unwrap_or(false),
        )
    } else {
        match database::get_staff_by_id(&state.db, viewer_id).await {
            Ok(Some(v)) => (
                v.is_admin || v.is_god,
                database::is_chief(&state.db, viewer_id)
                    .await
                    .unwrap_or(false),
            ),
            _ => (false, false),
        }
    };
    let show_contact = is_self || is_viewer_admin || is_viewer_chief;

    // Get all ateliers
    let ateliers = match database::get_all_ateliers(&state.db).await {
        Ok(a) => a,
        Err(e) => {
            error!("Error fetching ateliers: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("<p>Error: {}</p>", e)),
            )
                .into_response();
        }
    };

    // Get staff's current roles
    let roles = match database::get_staff_roles(&state.db, id).await {
        Ok(r) => r,
        Err(e) => {
            error!("Error fetching roles: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("<p>Error: {}</p>", e)),
            )
                .into_response();
        }
    };

    // Build TODO data (only for self-viewing)
    let todos = if is_self {
        let mut items = Vec::new();

        // Chiefs: pending validations
        if let Ok(pending) = database::count_pending_validations_for_chief(&state.db, id).await {
            for (atelier_name, count) in pending {
                items.push(templates::TodoItem {
                    icon: "fa-user-check",
                    color: "warning",
                    html: format!(
                        r#"<a href="{}/validation"><strong>{}</strong> demande(s) en attente de validation pour <strong>{}</strong></a>"#,
                        prefix, count, atelier_name,
                    ),
                });
            }
        }

        // Admins: pending imports
        if staff.is_admin {
            let unimported_ha = database::count_unimported_memberships(&state.db, current_season)
                .await
                .unwrap_or(0);
            let unimported_cash = database::count_unimported_cash(&state.db)
                .await
                .unwrap_or(0);
            if unimported_ha > 0 {
                items.push(templates::TodoItem {
                    icon: "fa-ticket-alt",
                    color: "danger",
                    html: format!(
                        r#"<a href="{}/users"><strong>{}</strong> adhésion(s) HelloAsso à importer</a>"#,
                        prefix, unimported_ha,
                    ),
                });
            }
            if unimported_cash > 0 {
                items.push(templates::TodoItem {
                    icon: "fa-money-bill-wave",
                    color: "danger",
                    html: format!(
                        r#"<a href="{}/cash"><strong>{}</strong> paiement(s) espèces/chèques à importer</a>"#,
                        prefix, unimported_cash,
                    ),
                });
            }
        }

        // No roles: remind to choose ateliers
        if roles.is_empty() {
            items.push(templates::TodoItem {
                icon: "fa-tools",
                color: "info",
                html: "Choisissez un ou plusieurs ateliers ci-dessous pour participer à la vie de la station !".to_string(),
            });
        }

        // Has roles but no upcoming presence
        if !roles.is_empty()
            && matches!(
                database::has_upcoming_presence(&state.db, id).await,
                Ok(false)
            )
        {
            items.push(templates::TodoItem {
                icon: "fa-calendar-alt",
                color: "info",
                html: "Vous n'avez pas encore indiqué vos disponibilités. Pensez à consulter les plannings ci-dessous pour indiquer quand vous êtes disponible\u{a0}!".to_string(),
            });
        }

        items
    } else {
        Vec::new()
    };

    let payment_history = match database::get_staff_payment_history(&state.db, id).await {
        Ok(h) => h,
        Err(e) => {
            error!("Error fetching payment history: {}", e);
            Vec::new()
        }
    };

    // Fetch person calendar (upcoming needs + presence across all ateliers)
    let person_calendar = if is_self {
        match database::get_person_calendar(&state.db, id).await {
            Ok(c) => c,
            Err(e) => {
                error!("Error fetching person calendar: {}", e);
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    Html(templates::person_detail(
        &staff,
        &ateliers,
        &roles,
        current_season,
        &prefix,
        is_self,
        is_viewer_admin,
        show_contact,
        &todos,
        &payment_history,
        &person_calendar,
    ))
    .into_response()
}

#[derive(Debug, Deserialize)]
struct ToggleRoleRequest {
    atelier_id: uuid::Uuid,
    #[serde(default)]
    add: Option<bool>,
    #[serde(default)]
    validated: Option<bool>,
    #[serde(default)]
    chief: Option<bool>,
}

async fn toggle_role(
    RequireStaff(me): RequireStaff,
    State(state): State<AppState>,
    axum::extract::Path(staff_id): axum::extract::Path<uuid::Uuid>,
    Json(payload): Json<ToggleRoleRequest>,
) -> impl IntoResponse {
    let is_self = me.id == staff_id;
    let is_admin = me.is_admin;

    // Check if caller is chief of the target atelier
    let is_chief_of_atelier = if is_admin {
        false
    } else {
        database::get_chief_ateliers(&state.db, me.id)
            .await
            .unwrap_or_default()
            .iter()
            .any(|a| a.id == payload.atelier_id)
    };

    // Authorization: validated changes require admin or chief of that atelier
    if payload.validated.is_some() && !is_admin && !is_chief_of_atelier {
        return (
            StatusCode::FORBIDDEN,
            Json(
                serde_json::json!({"error": "Seuls les admins ou chefs d'atelier peuvent modifier le statut de validation"}),
            ),
        );
    }
    // Authorization: chief changes require admin
    if payload.chief.is_some() && !is_admin {
        return (
            StatusCode::FORBIDDEN,
            Json(
                serde_json::json!({"error": "Seuls les admins peuvent modifier le statut de chef"}),
            ),
        );
    }
    // Authorization: add/remove requires self, admin, or chief of that atelier
    if payload.add.is_some() && !is_self && !is_admin && !is_chief_of_atelier {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Vous ne pouvez modifier que vos propres ateliers"})),
        );
    }

    // Handle add/remove role
    if let Some(add) = payload.add {
        if add {
            // Get atelier to check needs_validation
            let atelier = match database::get_atelier_by_id(&state.db, payload.atelier_id).await {
                Ok(Some(a)) => a,
                Ok(None) => {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(serde_json::json!({"error": "Atelier not found"})),
                    );
                }
                Err(e) => {
                    error!("Error fetching atelier: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": e.to_string()})),
                    );
                }
            };

            // validated = true if atelier doesn't need validation, false if it does
            let validated = !atelier.needs_validation;

            match database::add_role(&state.db, staff_id, payload.atelier_id, validated).await {
                Ok(()) => {
                    let _ = database::insert_audit(
                        &state.db,
                        Some(me.id),
                        &format!("{} {}", me.first_name, me.last_name),
                        "Ajout rôle",
                        &format!("staff={} atelier={}", staff_id, atelier.name),
                    )
                    .await;
                    // Notify chiefs if this role needs validation
                    if !validated {
                        let state_clone = state.clone();
                        let atelier_name = atelier.name.clone();
                        let atelier_id = atelier.id;
                        tokio::spawn(async move {
                            let staff_name = database::get_staff_by_id(&state_clone.db, staff_id)
                                .await
                                .ok()
                                .flatten()
                                .map_or_else(
                                    || "Quelqu'un".to_string(),
                                    |s| format!("{} {}", s.first_name, s.last_name),
                                );
                            let chiefs =
                                database::get_chiefs_for_atelier(&state_clone.db, atelier_id)
                                    .await
                                    .unwrap_or_default();
                            let chief_emails: Vec<String> =
                                chiefs.iter().map(|c| c.email.clone()).collect();
                            if !chief_emails.is_empty() {
                                let subject = format!(
                                    "AGHIL — {} demande à rejoindre {}",
                                    staff_name, atelier_name
                                );
                                let html_body = format!(
                                    r"<p>Bonjour,</p>
<p><strong>{staff}</strong> souhaite rejoindre l'atelier <strong>{atelier}</strong> et attend votre validation.</p>
<p>Connectez-vous à AGHIL pour valider ou refuser cette demande.</p>
<p><em>— PowPow pour AG'HIL</em></p>",
                                    staff = staff_name,
                                    atelier = atelier_name,
                                );
                                send_notification_email(
                                    &state_clone,
                                    &chief_emails,
                                    &subject,
                                    &html_body,
                                )
                                .await;
                            }
                        });
                    }
                    return (
                        StatusCode::OK,
                        Json(serde_json::json!({"success": true, "validated": validated})),
                    );
                }
                Err(e) => {
                    error!("Error adding role: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": e.to_string()})),
                    );
                }
            }
        }
        match database::remove_role(&state.db, staff_id, payload.atelier_id).await {
            Ok(()) => {
                let atelier_name = database::get_atelier_by_id(&state.db, payload.atelier_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|a| a.name)
                    .unwrap_or_default();
                let _ = database::insert_audit(
                    &state.db,
                    Some(me.id),
                    &format!("{} {}", me.first_name, me.last_name),
                    "Suppression rôle",
                    &format!("staff={} atelier={}", staff_id, atelier_name),
                )
                .await;
                return (StatusCode::OK, Json(serde_json::json!({"success": true})));
            }
            Err(e) => {
                error!("Error removing role: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                );
            }
        }
    }

    // Handle update validated/chief
    if payload.validated.is_some() || payload.chief.is_some() {
        match database::update_role(
            &state.db,
            staff_id,
            payload.atelier_id,
            payload.validated,
            payload.chief,
        )
        .await
        {
            Ok(()) => {
                // Audit
                let atelier_name = database::get_atelier_by_id(&state.db, payload.atelier_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|a| a.name)
                    .unwrap_or_default();
                let mut parts = Vec::new();
                if let Some(v) = payload.validated {
                    parts.push(format!("validated={}", v));
                }
                if let Some(c) = payload.chief {
                    parts.push(format!("chief={}", c));
                }
                let _ = database::insert_audit(
                    &state.db,
                    Some(me.id),
                    &format!("{} {}", me.first_name, me.last_name),
                    "Modification rôle",
                    &format!(
                        "staff={} atelier={} {}",
                        staff_id,
                        atelier_name,
                        parts.join(" ")
                    ),
                )
                .await;

                // Send notification emails for validated / chief changes
                let state_clone = state.clone();
                let atelier_id = payload.atelier_id;
                let validated = payload.validated;
                let chief = payload.chief;
                tokio::spawn(async move {
                    let staff = match database::get_staff_by_id(&state_clone.db, staff_id).await {
                        Ok(Some(s)) if !s.email.is_empty() => s,
                        _ => return,
                    };
                    let atelier_name = database::get_atelier_by_id(&state_clone.db, atelier_id)
                        .await
                        .ok()
                        .flatten()
                        .map_or_else(|| "atelier inconnu".to_string(), |a| a.name);

                    // Notification: role validated
                    if validated == Some(true) {
                        let subject =
                            format!("AGHIL — Votre rôle dans {} a été validé", atelier_name);
                        let html_body = format!(
                            r"<p>Bonjour {},</p>
<p>Votre demande pour rejoindre l'atelier <strong>{}</strong> a été validée. Vous pouvez dès maintenant vous inscrire aux créneaux sur le calendrier.</p>
<p><em>— PowPow pour AG'HIL</em></p>",
                            staff.first_name, atelier_name,
                        );
                        send_notification_email(
                            &state_clone,
                            std::slice::from_ref(&staff.email),
                            &subject,
                            &html_body,
                        )
                        .await;
                    }

                    // Notification: chief status changed
                    if let Some(is_chief) = chief {
                        let (subject, html_body) = if is_chief {
                            (
                                format!("AGHIL — Vous êtes maintenant chef de {}", atelier_name),
                                format!(
                                    r"<p>Bonjour {},</p>
<p>Vous avez été nommé(e) <strong>chef</strong> de l'atelier <strong>{}</strong>.</p>
<p>Vous recevrez désormais les notifications liées à cet atelier.</p>
<p><em>— PowPow pour AG'HIL</em></p>",
                                    staff.first_name, atelier_name,
                                ),
                            )
                        } else {
                            (
                                format!("AGHIL — Vous n'êtes plus chef de {}", atelier_name),
                                format!(
                                    r"<p>Bonjour {},</p>
<p>Vous n'êtes plus chef de l'atelier <strong>{}</strong>.</p>
<p><em>— PowPow pour AG'HIL</em></p>",
                                    staff.first_name, atelier_name,
                                ),
                            )
                        };
                        send_notification_email(
                            &state_clone,
                            std::slice::from_ref(&staff.email),
                            &subject,
                            &html_body,
                        )
                        .await;
                    }
                });

                (StatusCode::OK, Json(serde_json::json!({"success": true})))
            }
            Err(e) => {
                error!("Error updating role: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
            }
        }
    } else {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "No action specified"})),
        )
    }
}

#[derive(Deserialize)]
struct UpdateCommentPayload {
    comment: String,
}

async fn update_comment(
    RequireAdmin(admin): RequireAdmin,
    State(state): State<AppState>,
    axum::extract::Path(staff_id): axum::extract::Path<uuid::Uuid>,
    Json(payload): Json<UpdateCommentPayload>,
) -> impl IntoResponse {
    match database::update_staff_comment(&state.db, staff_id, &payload.comment).await {
        Ok(()) => {
            let _ = database::insert_audit(
                &state.db,
                Some(admin.id),
                &format!("{} {}", admin.first_name, admin.last_name),
                "Modification commentaire",
                &format!("staff={}", staff_id),
            )
            .await;
            (StatusCode::OK, Json(serde_json::json!({"success": true})))
        }
        Err(e) => {
            error!("Error updating comment: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }
}

#[derive(Deserialize)]
struct UpdateContactPayload {
    email: String,
    phone: Option<String>,
}

async fn update_contact(
    RequireStaff(me): RequireStaff,
    State(state): State<AppState>,
    axum::extract::Path(staff_id): axum::extract::Path<uuid::Uuid>,
    Json(payload): Json<UpdateContactPayload>,
) -> impl IntoResponse {
    // Only allow editing own contact info, or if admin/god
    if me.id != staff_id && !me.is_admin && !me.is_god {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Accès refusé"})),
        )
            .into_response();
    }

    let email = payload.email.trim().to_lowercase();
    if email.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Email requis"})),
        )
            .into_response();
    }

    let phone = payload
        .phone
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(templates::format_phone_international);

    match database::update_staff_contact(&state.db, staff_id, &email, phone.as_deref()).await {
        Ok(()) => {
            let _ = database::insert_audit(
                &state.db,
                Some(me.id),
                &format!("{} {}", me.first_name, me.last_name),
                "Modification contact",
                &format!(
                    "staff={} email={} phone={}",
                    staff_id,
                    email,
                    phone.as_deref().unwrap_or("")
                ),
            )
            .await;
            (StatusCode::OK, Json(serde_json::json!({"success": true}))).into_response()
        }
        Err(e) => {
            error!("Error updating contact: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

async fn calendar_view(
    auth::RequireStaff(me_staff): auth::RequireStaff,
    headers: HeaderMap,
    State(state): State<AppState>,
    axum::extract::Path(slug): axum::extract::Path<String>,
) -> Response {
    let prefix = get_prefix(&headers);
    let me = Some(me_staff);

    // Lookup atelier by slug
    let atelier = match database::get_atelier_by_slug(&state.db, &slug).await {
        Ok(Some(a)) => a,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Html("<p>Atelier not found</p>".to_string()),
            )
                .into_response();
        }
        Err(e) => {
            error!("Error fetching atelier: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("<p>Error: {}</p>", e)),
            )
                .into_response();
        }
    };

    // Fetch needs (only today and future), staff, all ateliers
    let today = chrono::Utc::now().date_naive();
    let needs = match database::get_needs_for_atelier(&state.db, atelier.id).await {
        Ok(n) => n
            .into_iter()
            .filter(|need| need.day >= today)
            .collect::<Vec<_>>(),
        Err(e) => {
            error!("Error fetching needs: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("<p>Error: {}</p>", e)),
            )
                .into_response();
        }
    };

    let staff_list = match database::get_staff_for_atelier(&state.db, atelier.id).await {
        Ok(s) => s,
        Err(e) => {
            error!("Error fetching staff: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("<p>Error: {}</p>", e)),
            )
                .into_response();
        }
    };

    let all_ateliers = match database::get_all_ateliers(&state.db).await {
        Ok(a) => a,
        Err(e) => {
            error!("Error fetching ateliers: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("<p>Error: {}</p>", e)),
            )
                .into_response();
        }
    };

    // Batch fetch presence
    let need_ids: Vec<uuid::Uuid> = needs.iter().map(|n| n.id).collect();
    let presence_rows = match database::get_presence_for_needs(&state.db, &need_ids).await {
        Ok(p) => p,
        Err(e) => {
            error!("Error fetching presence: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("<p>Error: {}</p>", e)),
            )
                .into_response();
        }
    };

    // Build HashMap<(need_id, staff_id), (first_half, second_half)>
    let mut presence_map = HashMap::new();
    for (need_id, staff_id, first_half, second_half) in presence_rows {
        presence_map.insert((need_id, staff_id), (first_half, second_half));
    }

    Html(templates::calendar(
        &atelier,
        &needs,
        &staff_list,
        &presence_map,
        &all_ateliers,
        &prefix,
        me.as_ref().map(|s| s.id),
        me.as_ref().is_some_and(|s| s.is_admin),
    ))
    .into_response()
}

#[derive(Debug, Deserialize)]
struct TogglePresenceRequest {
    needs_id: uuid::Uuid,
    staff_id: uuid::Uuid,
    half: String,
    value: bool,
}

async fn toggle_presence_api(
    RequireStaff(me): RequireStaff,
    State(state): State<AppState>,
    Json(payload): Json<TogglePresenceRequest>,
) -> impl IntoResponse {
    // Authorization: only the staff member themselves can toggle their own availability
    if payload.staff_id != me.id {
        return (
            StatusCode::FORBIDDEN,
            Json(
                serde_json::json!({"error": "Vous ne pouvez modifier que votre propre disponibilité"}),
            ),
        );
    }

    // Fetch current presence
    let (mut first_half, mut second_half) =
        match database::get_presence(&state.db, payload.needs_id, payload.staff_id).await {
            Ok(Some((f, s))) => (f, s),
            Ok(None) => (false, false),
            Err(e) => {
                error!("Error fetching presence: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                );
            }
        };

    // Apply the toggle
    match payload.half.as_str() {
        "first" => first_half = payload.value,
        "second" => second_half = payload.value,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "half must be 'first' or 'second'"})),
            );
        }
    }

    // Conflict check: prevent registering for the same half-day in two different ateliers
    if payload.value {
        // Look up the need to get the day
        let need = match database::get_need_by_id(&state.db, payload.needs_id).await {
            Ok(Some(n)) => n,
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error": "Besoin introuvable"})),
                );
            }
            Err(e) => {
                error!("Error fetching need: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                );
            }
        };

        match database::check_presence_conflict(
            &state.db,
            payload.staff_id,
            need.day,
            payload.needs_id,
            &payload.half,
        )
        .await
        {
            Ok(Some(conflicting_atelier)) => {
                return (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "error": format!(
                            "Vous êtes déjà inscrit(e) sur ce créneau pour l'atelier « {} »",
                            conflicting_atelier
                        )
                    })),
                );
            }
            Ok(None) => {} // No conflict
            Err(e) => {
                error!("Error checking presence conflict: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                );
            }
        }
    }

    // Upsert (or delete if both false)
    match database::upsert_presence(
        &state.db,
        payload.needs_id,
        payload.staff_id,
        first_half,
        second_half,
    )
    .await
    {
        Ok(()) => {
            let target_name = if payload.staff_id == me.id {
                format!("{} {}", me.first_name, me.last_name)
            } else {
                database::get_staff_by_id(&state.db, payload.staff_id)
                    .await
                    .ok()
                    .flatten()
                    .map_or_else(
                        || payload.staff_id.to_string(),
                        |s| format!("{} {}", s.first_name, s.last_name),
                    )
            };
            let _ = database::insert_audit(
                &state.db,
                Some(me.id),
                &format!("{} {}", me.first_name, me.last_name),
                "Modification présence",
                &format!(
                    "{} — {}={} (need={})",
                    target_name, payload.half, payload.value, payload.needs_id
                ),
            )
            .await;
            (
                StatusCode::OK,
                Json(
                    serde_json::json!({"success": true, "first_half": first_half, "second_half": second_half}),
                ),
            )
        }
        Err(e) => {
            error!("Error upserting presence: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }
}

// --- Calendar landing (public: redirect to first atelier, or editor for chiefs) ---

async fn calendar_landing(
    jar: SignedCookieJar,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Response {
    let prefix = get_prefix(&headers);

    // Check if logged-in user is a chief/admin — if so, show the editor
    let staff = if let Some(id) = jar
        .get("aghil_session")
        .and_then(|c| c.value().parse::<uuid::Uuid>().ok())
    {
        database::get_staff_by_id(&state.db, id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    // Fetch ateliers
    let ateliers = match database::get_all_ateliers(&state.db).await {
        Ok(a) => a,
        Err(e) => {
            error!("Error fetching ateliers: {}", e);
            return Html(format!("<p>Error: {}</p>", e)).into_response();
        }
    };

    // Determine which ateliers this user can edit (empty = read-only)
    let editable_ids: Vec<uuid::Uuid> = if let Some(ref s) = staff {
        if s.is_admin || s.is_god {
            ateliers.iter().map(|a| a.id).collect()
        } else {
            database::get_chief_ateliers(&state.db, s.id)
                .await
                .unwrap_or_default()
                .iter()
                .map(|a| a.id)
                .collect()
        }
    } else {
        vec![]
    };

    let today = chrono::Local::now().date_naive();
    let future_needs = database::get_all_future_needs_with_counts(&state.db, today)
        .await
        .unwrap_or_default();

    Html(templates::calendar_editor(
        &ateliers,
        &editable_ids,
        &future_needs,
        &prefix,
        staff.is_some(),
    ))
    .into_response()
}

// --- Calendar editor (needs management) ---

#[derive(Debug, Deserialize)]
struct NeedsQuery {
    atelier_id: uuid::Uuid,
}

async fn api_get_needs(
    RequireChief(_staff): RequireChief,
    State(state): State<AppState>,
    Query(params): Query<NeedsQuery>,
) -> impl IntoResponse {
    match database::get_needs_for_atelier(&state.db, params.atelier_id).await {
        Ok(needs) => {
            let json_needs: Vec<serde_json::Value> = needs
                .iter()
                .map(|n| {
                    serde_json::json!({
                        "id": n.id,
                        "day": n.day.format("%Y-%m-%d").to_string(),
                        "atelier": n.atelier,
                        "quantity": n.quantity,
                        "nightly": n.nightly,
                    })
                })
                .collect();
            (StatusCode::OK, Json(serde_json::json!(json_needs))).into_response()
        }
        Err(e) => {
            error!("Error fetching needs: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

async fn api_get_need_days(
    RequireChief(_staff): RequireChief,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match database::get_all_need_days(&state.db).await {
        Ok(days) => {
            let json_days: Vec<String> = days
                .iter()
                .map(|d| d.format("%Y-%m-%d").to_string())
                .collect();
            (StatusCode::OK, Json(serde_json::json!(json_days))).into_response()
        }
        Err(e) => {
            error!("Error fetching need days: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct NeedsByDayQuery {
    day: String,
}

async fn api_get_needs_by_day(
    RequireChief(_staff): RequireChief,
    State(state): State<AppState>,
    Query(params): Query<NeedsByDayQuery>,
) -> impl IntoResponse {
    let day = match chrono::NaiveDate::parse_from_str(&params.day, "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Invalid date: {}", e)})),
            )
                .into_response();
        }
    };

    match database::get_needs_for_day(&state.db, day).await {
        Ok(needs) => {
            let json_needs: Vec<serde_json::Value> = needs
                .iter()
                .map(|n| {
                    serde_json::json!({
                        "id": n.id,
                        "day": n.day.format("%Y-%m-%d").to_string(),
                        "atelier": n.atelier,
                        "quantity": n.quantity,
                        "nightly": n.nightly,
                    })
                })
                .collect();
            (StatusCode::OK, Json(serde_json::json!(json_needs))).into_response()
        }
        Err(e) => {
            error!("Error fetching needs by day: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct UpsertNeedRequest {
    atelier_id: uuid::Uuid,
    day: String,
    quantity: i16,
    #[serde(default)]
    nightly: bool,
}

async fn api_upsert_need(
    RequireChief(chief): RequireChief,
    State(state): State<AppState>,
    Json(payload): Json<UpsertNeedRequest>,
) -> impl IntoResponse {
    // Authorization: admins can edit all, chiefs only their ateliers
    if !chief.is_admin && !chief.is_god {
        let my_ateliers = database::get_chief_ateliers(&state.db, chief.id)
            .await
            .unwrap_or_default();
        if !my_ateliers.iter().any(|a| a.id == payload.atelier_id) {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({"error": "Vous n'êtes pas chef de cet atelier"})),
            );
        }
    }

    let day = match chrono::NaiveDate::parse_from_str(&payload.day, "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Invalid date: {}", e)})),
            );
        }
    };

    match database::upsert_need(
        &state.db,
        payload.atelier_id,
        day,
        payload.quantity,
        payload.nightly,
    )
    .await
    {
        Ok(need) => {
            let atelier_name = database::get_atelier_by_id(&state.db, payload.atelier_id)
                .await
                .ok()
                .flatten()
                .map(|a| a.name)
                .unwrap_or_default();
            let _ = database::insert_audit(
                &state.db,
                Some(chief.id),
                &format!("{} {}", chief.first_name, chief.last_name),
                "Création/modification besoin",
                &format!(
                    "{} {} qty={} nightly={}",
                    atelier_name, payload.day, payload.quantity, payload.nightly
                ),
            )
            .await;
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "id": need.id,
                    "day": need.day.format("%Y-%m-%d").to_string(),
                    "atelier": need.atelier,
                    "quantity": need.quantity,
                    "nightly": need.nightly,
                })),
            )
        }
        Err(e) => {
            error!("Error upserting need: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }
}

#[derive(Debug, Deserialize)]
struct DeleteNeedRequest {
    atelier_id: uuid::Uuid,
    day: String,
}

async fn api_delete_need(
    RequireChief(chief): RequireChief,
    State(state): State<AppState>,
    Json(payload): Json<DeleteNeedRequest>,
) -> impl IntoResponse {
    // Authorization: admins can edit all, chiefs only their ateliers
    if !chief.is_admin && !chief.is_god {
        let my_ateliers = database::get_chief_ateliers(&state.db, chief.id)
            .await
            .unwrap_or_default();
        if !my_ateliers.iter().any(|a| a.id == payload.atelier_id) {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({"error": "Vous n'êtes pas chef de cet atelier"})),
            );
        }
    }

    let day = match chrono::NaiveDate::parse_from_str(&payload.day, "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Invalid date: {}", e)})),
            );
        }
    };

    match database::delete_need(&state.db, payload.atelier_id, day).await {
        Ok(deleted) => {
            if deleted {
                let atelier_name = database::get_atelier_by_id(&state.db, payload.atelier_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|a| a.name)
                    .unwrap_or_default();
                let _ = database::insert_audit(
                    &state.db,
                    Some(chief.id),
                    &format!("{} {}", chief.first_name, chief.last_name),
                    "Suppression besoin",
                    &format!("{} {}", atelier_name, payload.day),
                )
                .await;
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({"deleted": deleted})),
            )
        }
        Err(e) => {
            error!("Error deleting need: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }
}

async fn get_user(
    RequireAdmin(_staff): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    axum::extract::Path(email): axum::extract::Path<String>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    match database::get_user_by_email(&state.db, email).await {
        Ok(Some(user)) => (StatusCode::OK, Html(templates::user_detail(user, &prefix))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Html("<p>User not found</p>".to_string()),
        ),
        Err(e) => {
            error!("Error fetching user: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("<p>Error loading user: {}</p>", e)),
            )
        }
    }
}

async fn import_staff(
    RequireAdmin(_staff): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    axum::extract::Path(item_id): axum::extract::Path<i64>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    match database::get_membership_by_item_id(&state.db, item_id).await {
        Ok(Some(membership)) => {
            let season = if let Some(order_date) = membership.order_date {
                get_season_for(order_date)
            } else {
                get_current_season()
            };

            // Check if already imported
            let already_imported = database::has_staff_for_membership(&state.db, item_id, season)
                .await
                .unwrap_or(false);

            if already_imported {
                return (
                    StatusCode::OK,
                    Html(templates::already_imported_page(
                        membership, season, &prefix,
                    )),
                );
            }

            // Get the email and name from the membership
            let membership_email = membership.email.as_deref().unwrap_or("");
            let first_name = membership.beneficiary_first_name.as_deref().unwrap_or("");
            let last_name = membership.beneficiary_last_name.as_deref().unwrap_or("");
            let payer_email = membership.payer_email.clone();
            let payer_email_str = payer_email.as_deref().unwrap_or("");

            // Find staff candidates (search both membership email and payer email)
            let candidates = database::find_staff_candidates(
                &state.db,
                membership_email,
                payer_email_str,
                first_name,
                last_name,
                season,
            )
            .await
            .unwrap_or_default();

            // Always allow creating a new staff (two people can have the same name)
            (
                StatusCode::OK,
                Html(templates::import_staff_form(
                    membership,
                    season,
                    candidates,
                    payer_email.as_deref(),
                    false,
                    &prefix,
                )),
            )
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Html("<p>Membership not found</p>".to_string()),
        ),
        Err(e) => {
            error!("Error fetching membership: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("<p>Error loading membership: {}</p>", e)),
            )
        }
    }
}

#[derive(Debug, Deserialize)]
struct ImportStaffForm {
    action: String,           // "create" or "update"
    staff_id: Option<String>, // UUID of existing staff (for update)
    first_name: String,
    last_name: String,
    email: String,
    phone: Option<String>,
    comment: Option<String>,
}

async fn do_import_staff(
    RequireAdmin(admin): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    axum::extract::Path(item_id): axum::extract::Path<i64>,
    axum::extract::Form(form): axum::extract::Form<ImportStaffForm>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);

    // Get the membership to find the season
    let membership = match database::get_membership_by_item_id(&state.db, item_id).await {
        Ok(Some(m)) => m,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Html("<p>Membership not found</p>".to_string()),
            );
        }
        Err(e) => {
            error!("Error fetching membership: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("<p>Error loading membership: {}</p>", e)),
            );
        }
    };

    let season = if let Some(order_date) = membership.order_date {
        get_season_for(order_date)
    } else {
        get_current_season()
    };

    let comment = form.comment.as_deref().unwrap_or("");

    let result = match form.action.as_str() {
        "create" => {
            database::create_staff_with_payment(
                &state.db,
                &form.first_name,
                &form.last_name,
                &form.email,
                form.phone.as_deref(),
                comment,
                item_id,
                season,
            )
            .await
        }
        "update" => {
            let Some(staff_id) = form
                .staff_id
                .as_ref()
                .and_then(|s| s.parse::<uuid::Uuid>().ok())
            else {
                return (
                    StatusCode::BAD_REQUEST,
                    Html("<p>Invalid staff ID</p>".to_string()),
                );
            };
            database::update_staff_with_payment(
                &state.db,
                staff_id,
                &form.first_name,
                &form.last_name,
                &form.email,
                form.phone.as_deref(),
                comment,
                item_id,
                season,
            )
            .await
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Html("<p>Invalid action</p>".to_string()),
            );
        }
    };

    match result {
        Ok(_staff) => {
            let _ = database::insert_audit(
                &state.db,
                Some(admin.id),
                &format!("{} {}", admin.first_name, admin.last_name),
                "Import adhésion HelloAsso",
                &format!(
                    "{} {} (item_id={})",
                    form.first_name, form.last_name, item_id
                ),
            )
            .await;
            // Redirect back to users page with filter to show remaining not-imported memberships
            (
                StatusCode::SEE_OTHER,
                Html(format!(
                    r#"<meta http-equiv="refresh" content="0;url={}/users"><p>Redirecting...</p>"#,
                    prefix
                )),
            )
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("ALREADY_IMPORTED") {
                // Race condition: someone else already imported this membership
                (
                    StatusCode::CONFLICT,
                    Html(templates::import_result(
                        false,
                        "Cette adhésion a déjà été importée par quelqu'un d'autre.",
                        &prefix,
                    )),
                )
            } else if error_msg.contains("DUPLICATE_NAME") {
                // A staff with this name already exists
                (
                    StatusCode::CONFLICT,
                    Html(templates::import_result(
                        false,
                        "Un staff avec ce nom existe déjà. Utilisez l'option \"Mettre à jour\" pour ajouter une adhésion à un staff existant.",
                        &prefix,
                    )),
                )
            } else {
                error!("Error importing staff: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Html(templates::import_result(
                        false,
                        &format!("Erreur lors de l'import: {}", e),
                        &prefix,
                    )),
                )
            }
        }
    }
}

async fn list_cash(
    RequireAdmin(_staff): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    let show_form = params.get("form").is_some_and(|f| f == "1");

    if show_form {
        return Html(templates::cash_form(&prefix));
    }

    let current_season = get_current_season();

    match database::get_all_cash_payments(&state.db).await {
        Ok(cash_payments) => {
            let mut payments_with_status = Vec::new();
            for cash in cash_payments {
                let has_staff = database::has_staff_for_cash(&state.db, cash.id)
                    .await
                    .unwrap_or(false);
                payments_with_status.push((cash, has_staff));
            }

            // Sort: not-yet-imported first, then by date (most recent first)
            payments_with_status.sort_by(|a, b| match a.1.cmp(&b.1) {
                std::cmp::Ordering::Equal => b.0.date.cmp(&a.0.date),
                other => other,
            });

            Html(templates::cash_list(
                payments_with_status,
                current_season,
                &prefix,
            ))
        }
        Err(e) => {
            error!("Error fetching cash payments: {}", e);
            Html(format!("<p>Error loading cash payments: {}</p>", e))
        }
    }
}

#[derive(Debug, Deserialize)]
struct CreateCashForm {
    first_name: String,
    last_name: String,
    email: Option<String>,
    phone: Option<String>,
    date: String,
    amount: i32,
    #[serde(default)]
    is_membership: Option<String>,
    payment_method: String,
}

async fn create_cash(
    RequireAdmin(admin): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    axum::extract::Form(form): axum::extract::Form<CreateCashForm>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);

    let date = match chrono::NaiveDate::parse_from_str(&form.date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            error!("Invalid date format: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Html(format!("<p>Date invalide: {}</p>", e)),
            );
        }
    };

    let email = form.email.as_deref().filter(|e| !e.is_empty());
    let phone = form.phone.as_deref().filter(|p| !p.is_empty());
    let is_membership = form.is_membership.is_some();

    match database::create_cash_payment(
        &state.db,
        &form.first_name,
        &form.last_name,
        email,
        phone,
        date,
        form.amount,
        is_membership,
        &form.payment_method,
    )
    .await
    {
        Ok(_) => {
            let _ = database::insert_audit(
                &state.db,
                Some(admin.id),
                &format!("{} {}", admin.first_name, admin.last_name),
                "Création paiement espèces",
                &format!("{} {} — {}€", form.first_name, form.last_name, form.amount),
            )
            .await;
            // Notify admins about new cash payment
            let state_clone = state.clone();
            let first_name = form.first_name.clone();
            let last_name = form.last_name.clone();
            let amount = form.amount;
            tokio::spawn(async move {
                let admin_emails = database::get_admin_emails(&state_clone.db)
                    .await
                    .unwrap_or_default();
                if !admin_emails.is_empty() {
                    let subject = "AGHIL — Nouveau paiement espèces à importer";
                    let html_body = format!(
                        r"<p>Bonjour,</p>
<p>Un nouveau paiement espèces/chèque a été enregistré :</p>
<p><strong>{} {}</strong> — {}€</p>
<p>Connectez-vous à AGHIL pour l'importer.</p>
<p><em>— PowPow pour AG'HIL</em></p>",
                        first_name, last_name, amount,
                    );
                    send_notification_email(&state_clone, &admin_emails, subject, &html_body).await;
                }
            });

            (
                StatusCode::SEE_OTHER,
                Html(format!(
                    r#"<meta http-equiv="refresh" content="0;url={}/cash"><p>Redirecting...</p>"#,
                    prefix
                )),
            )
        }
        Err(e) => {
            error!("Error creating cash payment: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("<p>Erreur: {}</p>", e)),
            )
        }
    }
}

async fn import_cash(
    RequireAdmin(_staff): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    axum::extract::Path(cash_id): axum::extract::Path<uuid::Uuid>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);

    let cash = match database::get_cash_by_id(&state.db, cash_id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Html("<p>Paiement non trouvé</p>".to_string()),
            );
        }
        Err(e) => {
            error!("Error fetching cash payment: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("<p>Erreur: {}</p>", e)),
            );
        }
    };

    // Derive season from the cash payment date
    let season = {
        let year = cash.date.year();
        let month = cash.date.month();
        if month >= 6 {
            (year + 1) as i16
        } else {
            year as i16
        }
    };

    // Check if already imported
    let already_imported = database::has_staff_for_cash(&state.db, cash_id)
        .await
        .unwrap_or(false);

    if already_imported {
        return (
            StatusCode::OK,
            Html(templates::import_result(
                true,
                "Ce paiement a déjà été importé.",
                &prefix,
            )),
        );
    }

    let cash_email = cash.email.as_deref().unwrap_or("");
    let first_name = &cash.first_name;
    let last_name = &cash.last_name;

    let candidates = database::find_staff_candidates(
        &state.db, cash_email, "", // no payer email for cash
        first_name, last_name, season,
    )
    .await
    .unwrap_or_default();

    (
        StatusCode::OK,
        Html(templates::cash_import_form(
            &cash, season, candidates, &prefix,
        )),
    )
}

#[derive(Debug, Deserialize)]
struct CashImportForm {
    action: String,
    staff_id: Option<String>,
    first_name: String,
    last_name: String,
    email: String,
    phone: Option<String>,
    comment: Option<String>,
}

async fn do_import_cash(
    RequireAdmin(admin): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    axum::extract::Path(cash_id): axum::extract::Path<uuid::Uuid>,
    axum::extract::Form(form): axum::extract::Form<CashImportForm>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);

    // Fetch the cash payment to derive season from its date
    let cash = match database::get_cash_by_id(&state.db, cash_id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Html("<p>Paiement non trouvé</p>".to_string()),
            );
        }
        Err(e) => {
            error!("Error fetching cash payment: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("<p>Erreur: {}</p>", e)),
            );
        }
    };

    let season = {
        let year = cash.date.year();
        let month = cash.date.month();
        if month >= 6 {
            (year + 1) as i16
        } else {
            year as i16
        }
    };

    // Check if already imported
    let already_imported = database::has_staff_for_cash(&state.db, cash_id)
        .await
        .unwrap_or(false);

    if already_imported {
        return (
            StatusCode::CONFLICT,
            Html(templates::import_result(
                false,
                "Ce paiement a déjà été importé.",
                &prefix,
            )),
        );
    }

    let comment = form.comment.as_deref().unwrap_or("");

    let result = match form.action.as_str() {
        "create" => {
            database::create_staff_with_cash_payment(
                &state.db,
                &form.first_name,
                &form.last_name,
                &form.email,
                form.phone.as_deref(),
                comment,
                cash_id,
                season,
            )
            .await
        }
        "update" => {
            let Some(staff_id) = form
                .staff_id
                .as_ref()
                .and_then(|s| s.parse::<uuid::Uuid>().ok())
            else {
                return (
                    StatusCode::BAD_REQUEST,
                    Html("<p>Invalid staff ID</p>".to_string()),
                );
            };
            database::update_staff_with_cash_payment(
                &state.db,
                staff_id,
                &form.first_name,
                &form.last_name,
                &form.email,
                form.phone.as_deref(),
                comment,
                cash_id,
                season,
            )
            .await
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Html("<p>Action invalide</p>".to_string()),
            );
        }
    };

    match result {
        Ok(_staff) => {
            let _ = database::insert_audit(
                &state.db,
                Some(admin.id),
                &format!("{} {}", admin.first_name, admin.last_name),
                "Import paiement espèces",
                &format!(
                    "{} {} (cash_id={})",
                    form.first_name, form.last_name, cash_id
                ),
            )
            .await;
            (
                StatusCode::SEE_OTHER,
                Html(format!(
                    r#"<meta http-equiv="refresh" content="0;url={}/cash"><p>Redirecting...</p>"#,
                    prefix
                )),
            )
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("ALREADY_IMPORTED") {
                (
                    StatusCode::CONFLICT,
                    Html(templates::import_result(
                        false,
                        "Ce paiement a déjà été importé par quelqu'un d'autre.",
                        &prefix,
                    )),
                )
            } else {
                error!("Error importing cash payment: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Html(templates::import_result(
                        false,
                        &format!("Erreur lors de l'import: {}", e),
                        &prefix,
                    )),
                )
            }
        }
    }
}

/// Check sync token from query param or Authorization header.
/// Returns the caller name if authorized, or an error response.
#[allow(clippy::result_large_err)]
fn check_automation_token(
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

async fn sync_users(
    jar: SignedCookieJar,
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    // Check auth: either logged-in admin or valid sync token
    let (caller_name, staff_id) = if let Some(admin) = resolve_staff_if_admin(&jar, &state).await {
        let name = format!("{} {}", admin.first_name, admin.last_name);
        (name, Some(admin.id))
    } else {
        match check_automation_token(&params, &headers, &state.config.sync_token, "sync token") {
            Ok(name) => (name, None),
            Err(resp) => return resp,
        }
    };

    match sync_users_from_helloasso(&state).await {
        Ok((user_count, membership_count)) => {
            let _ = database::insert_audit(
                &state.db,
                staff_id,
                &caller_name,
                "Synchronisation HelloAsso",
                &format!(
                    "{} utilisateurs, {} adhésions",
                    user_count, membership_count
                ),
            )
            .await;
            Html(format!(
                "<div class='alert alert-success'>Successfully synchronized {} users and {} memberships</div>",
                user_count, membership_count
            )).into_response()
        }
        Err(e) => {
            error!("Error syncing users: {}", e);
            Html(format!(
                "<div class='alert alert-danger'>Error syncing users: {}</div>",
                e
            ))
            .into_response()
        }
    }
}

/// Resolve logged-in staff if they are an admin.
async fn resolve_staff_if_admin(jar: &SignedCookieJar, state: &AppState) -> Option<models::Staff> {
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

async fn resolve_staff_if_god(jar: &SignedCookieJar, state: &AppState) -> Option<models::Staff> {
    let id = jar
        .get("aghil_session")
        .and_then(|c| c.value().parse::<uuid::Uuid>().ok())?;
    let staff = database::get_staff_by_id(&state.db, id).await.ok()??;
    if staff.is_god { Some(staff) } else { None }
}

async fn api_list_users(
    RequireAdmin(_staff): RequireAdmin,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    // Input validation
    let page = match params.get("page").and_then(|p| p.parse::<i64>().ok()) {
        Some(p) if p > 0 => p,
        _ => 1,
    };

    let limit = match params.get("limit").and_then(|p| p.parse::<i64>().ok()) {
        Some(l) if l > 0 && l <= 100 => l,
        _ => 20,
    };

    let offset = (page - 1) * limit;

    match database::get_users_paginated(&state.db, limit, offset).await {
        Ok(users) => {
            let total_users = database::count_users(&state.db).await.unwrap_or(0);
            let response = UserListResponse {
                users,
                page,
                limit,
                total: total_users,
                total_pages: (total_users + limit - 1) / limit,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            error!("Error fetching users: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to fetch users",
                    "details": e.to_string()
                })),
            )
                .into_response()
        }
    }
}

async fn api_sync_users(
    jar: SignedCookieJar,
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let (caller_name, staff_id) = if let Some(admin) = resolve_staff_if_admin(&jar, &state).await {
        let name = format!("{} {}", admin.first_name, admin.last_name);
        (name, Some(admin.id))
    } else {
        match check_automation_token(&params, &headers, &state.config.sync_token, "sync token") {
            Ok(name) => (name, None),
            Err(resp) => return resp,
        }
    };

    match sync_users_from_helloasso(&state).await {
        Ok((user_count, membership_count)) => {
            let _ = database::insert_audit(
                &state.db,
                staff_id,
                &caller_name,
                "Synchronisation HelloAsso (API)",
                &format!(
                    "{} utilisateurs, {} adhésions",
                    user_count, membership_count
                ),
            )
            .await;
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": true,
                    "synchronized_users": user_count,
                    "synchronized_memberships": membership_count,
                    "message": format!("Successfully synchronized {} users and {} memberships", user_count, membership_count)
                })),
            )
                .into_response()
        }

        Err(e) => {
            error!("Error syncing users: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": "Failed to sync users",
                    "details": e.to_string()
                })),
            )
                .into_response()
        }
    }
}

async fn debug_first_order(
    RequireGod(_staff): RequireGod,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.helloasso_client.get_orders().await {
        Ok(orders) => {
            if let Some(order) = orders.first() {
                (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "order": order,
                    })),
                )
                    .into_response()
            } else {
                (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "message": "No orders found"
                    })),
                )
                    .into_response()
            }
        }
        Err(e) => {
            error!("Error fetching orders: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": e.to_string()
                })),
            )
                .into_response()
        }
    }
}

async fn api_badge_counts(State(state): State<AppState>) -> impl IntoResponse {
    let current_season = get_current_season();
    let users = database::count_unimported_memberships(&state.db, current_season)
        .await
        .unwrap_or(0);
    let cash = database::count_unimported_cash(&state.db)
        .await
        .unwrap_or(0);

    Json(serde_json::json!({
        "users": users,
        "cash": cash,
    }))
}

#[derive(Debug, Deserialize)]
struct UpdateAdminFlagsRequest {
    staff_id: uuid::Uuid,
    is_admin: bool,
    is_god: bool,
}

async fn api_update_admin_flags(
    RequireGod(god): RequireGod,
    State(state): State<AppState>,
    Json(payload): Json<UpdateAdminFlagsRequest>,
) -> impl IntoResponse {
    // Fetch staff BEFORE update to compare old flags
    let old_staff = match database::get_staff_by_id(&state.db, payload.staff_id).await {
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

    // Enforce: is_god implies is_admin
    let is_admin = if payload.is_god {
        true
    } else {
        payload.is_admin
    };

    match database::update_staff_admin_flags(&state.db, payload.staff_id, is_admin, payload.is_god)
        .await
    {
        Ok(staff) => {
            // Send notification emails for flag changes
            if !staff.email.is_empty() {
                let state_clone = state.clone();
                let old_admin = old_staff.is_admin;
                let old_god = old_staff.is_god;
                let new_staff = staff.clone();
                tokio::spawn(async move {
                    // Admin flag changed
                    if !old_admin && new_staff.is_admin {
                        let subject = "AGHIL — Vous avez maintenant les droits administrateur";
                        let html_body = format!(
                            r"<p>Bonjour {},</p>
<p>Vous avez maintenant les <strong>droits administrateur</strong> sur AGHIL.</p>
<p>Vous pouvez gérer les adhésions, le staff et les paiements espèces.</p>
<p><em>— PowPow pour AG'HIL</em></p>",
                            new_staff.first_name,
                        );
                        send_notification_email(
                            &state_clone,
                            std::slice::from_ref(&new_staff.email),
                            subject,
                            &html_body,
                        )
                        .await;
                    } else if old_admin && !new_staff.is_admin {
                        let subject = "AGHIL — Vos droits administrateur ont été retirés";
                        let html_body = format!(
                            r"<p>Bonjour {},</p>
<p>Vos <strong>droits administrateur</strong> sur AGHIL ont été retirés.</p>
<p><em>— PowPow pour AG'HIL</em></p>",
                            new_staff.first_name,
                        );
                        send_notification_email(
                            &state_clone,
                            std::slice::from_ref(&new_staff.email),
                            subject,
                            &html_body,
                        )
                        .await;
                    }

                    // God flag changed
                    if !old_god && new_staff.is_god {
                        let subject = "AGHIL — Vous avez maintenant les droits God";
                        let html_body = format!(
                            r"<p>Bonjour {},</p>
<p>Vous avez maintenant les <strong>droits God</strong> sur AGHIL.</p>
<p><em>— PowPow pour AG'HIL</em></p>",
                            new_staff.first_name,
                        );
                        send_notification_email(
                            &state_clone,
                            std::slice::from_ref(&new_staff.email),
                            subject,
                            &html_body,
                        )
                        .await;
                    } else if old_god && !new_staff.is_god {
                        let subject = "AGHIL — Vos droits God ont été retirés";
                        let html_body = format!(
                            r"<p>Bonjour {},</p>
<p>Vos <strong>droits God</strong> sur AGHIL ont été retirés.</p>
<p><em>— PowPow pour AG'HIL</em></p>",
                            new_staff.first_name,
                        );
                        send_notification_email(
                            &state_clone,
                            std::slice::from_ref(&new_staff.email),
                            subject,
                            &html_body,
                        )
                        .await;
                    }
                });
            }

            let _ = database::insert_audit(
                &state.db,
                Some(god.id),
                &format!("{} {}", god.first_name, god.last_name),
                "Modification droits admin",
                &format!(
                    "{} {} — admin={} god={}",
                    staff.first_name, staff.last_name, staff.is_admin, staff.is_god
                ),
            )
            .await;

            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": true,
                    "is_admin": staff.is_admin,
                    "is_god": staff.is_god,
                })),
            )
        }
        Err(e) => {
            error!("Error updating admin flags: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": e.to_string(),
                })),
            )
        }
    }
}

// --- Audit log ---

async fn audit_page_handler(
    RequireAdmin(_staff): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    let page_size: i64 = 50;
    let current_page: i64 = params
        .get("page")
        .and_then(|p| p.parse().ok())
        .unwrap_or(1)
        .max(1);
    let offset = (current_page - 1) * page_size;

    let total = database::count_audit(&state.db).await.unwrap_or(0);
    let total_pages = ((total as f64) / (page_size as f64)).ceil() as i64;

    let entries = database::get_audit_log_paginated(&state.db, page_size, offset)
        .await
        .unwrap_or_default();

    Html(templates::audit_page(
        &entries,
        current_page,
        total_pages.max(1),
        &prefix,
    ))
}

async fn validation_page_handler(
    RequireStaff(me): RequireStaff,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);

    let pending = if me.is_admin {
        // Admins see all pending validations
        database::get_pending_validations(&state.db, None)
            .await
            .unwrap_or_default()
    } else {
        // Chiefs see only their ateliers
        let chief_ateliers = database::get_chief_ateliers(&state.db, me.id)
            .await
            .unwrap_or_default();
        if chief_ateliers.is_empty() {
            Vec::new()
        } else {
            let atelier_ids: Vec<uuid::Uuid> = chief_ateliers.iter().map(|a| a.id).collect();
            database::get_pending_validations(&state.db, Some(&atelier_ids))
                .await
                .unwrap_or_default()
        }
    };

    Html(templates::validation_page(&pending, &prefix))
}

// --- Login / Session handlers ---

async fn login_page(headers: HeaderMap) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    Html(templates::login_page(&prefix))
}

#[derive(Debug, Deserialize)]
struct StaffSearchQuery {
    q: Option<String>,
}

async fn api_search_staff(
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
struct CreateStaffMinimalRequest {
    first_name: String,
    last_name: String,
    email: Option<String>,
    phone: Option<String>,
}

async fn api_create_staff_minimal(
    RequireAdmin(_staff): RequireAdmin,
    State(state): State<AppState>,
    Json(payload): Json<CreateStaffMinimalRequest>,
) -> impl IntoResponse {
    let email = payload
        .email
        .as_deref()
        .map(|e| e.trim().to_lowercase())
        .unwrap_or_default();
    let phone = payload
        .phone
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(templates::format_phone_international);

    match database::create_staff_minimal(
        &state.db,
        &payload.first_name,
        &payload.last_name,
        &email,
        phone.as_deref(),
    )
    .await
    {
        Ok(staff) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "id": staff.id,
                "first_name": staff.first_name,
                "last_name": staff.last_name,
            })),
        ),
        Err(e) if e.to_string() == "DUPLICATE_NAME" => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": "Un bénévole avec ce nom existe déjà"})),
        ),
        Err(e) => {
            error!("Error creating staff: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }
}

#[derive(Debug, Deserialize)]
struct SendLoginRequest {
    staff_id: uuid::Uuid,
}

async fn api_send_login_email(
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
    staff: &crate::models::Staff,
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
    staff: &crate::models::Staff,
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

async fn api_me(State(state): State<AppState>, jar: SignedCookieJar) -> impl IntoResponse {
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
        Ok(Some(staff)) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "id": staff.id,
                "first_name": staff.first_name,
                "last_name": staff.last_name,
                "is_admin": staff.is_admin,
                "is_god": staff.is_god,
            })),
        ),
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

async fn logout(headers: HeaderMap, jar: SignedCookieJar) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    let mut cookie = axum_extra::extract::cookie::Cookie::new("aghil_session", "");
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::ZERO);
    let updated_jar = jar.remove(cookie);
    (updated_jar, Redirect::to(&format!("{}/", prefix)))
}

async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
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
            error!("Database health check failed: {}", e);
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

async fn privacy_page(headers: HeaderMap) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    Html(templates::static_page(
        &prefix,
        "Politique de Confidentialité",
        include_str!("../privacy.md"),
    ))
}

async fn tos_page(headers: HeaderMap) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    Html(templates::static_page(
        &prefix,
        "Conditions d'Utilisation",
        include_str!("../tos.md"),
    ))
}

async fn api_get_stats(
    RequireAdmin(_staff): RequireAdmin,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let total_users = database::count_users(&state.db).await.unwrap_or(0);
    let last_sync_users = database::get_recently_synced_users(&state.db, 24)
        .await
        .unwrap_or(0);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "total_users": total_users,
            "users_synced_last_24h": last_sync_users,
            "last_sync_timestamp": chrono::Utc::now(),
            "helloasso_association": state.helloasso_client.association_slug()
        })),
    )
        .into_response()
}

// Helper function to extract custom field value by name
fn get_custom_field_value(
    custom_fields: &[models::HelloAssoCustomField],
    field_name: &str,
) -> Option<String> {
    custom_fields
        .iter()
        .find(|f| f.name.as_deref() == Some(field_name))
        .and_then(|f| f.answer.clone())
}

fn next_monday_8am_local(
    from_when: chrono::DateTime<chrono::Local>,
) -> Option<chrono::DateTime<chrono::Local>> {
    let today = from_when.date_naive();
    let days_ahead = 8 - i64::from(today.weekday().number_from_monday());
    let monday_8am =
        from_when.date_naive().and_hms_opt(8, 0, 0).unwrap() + TimeDelta::days(days_ahead);
    let Some(monday_8am_local) =
        chrono::TimeZone::from_local_datetime(&from_when.timezone(), &monday_8am).single()
    else {
        // TZ failure, bail out
        return None;
    };

    let target = if from_when >= monday_8am_local {
        // Already past 8 AM today, schedule for next Monday
        monday_8am_local + TimeDelta::days(7)
    } else if from_when < monday_8am_local - TimeDelta::days(7) {
        // Next Monday 8 AM is too far away
        monday_8am_local - TimeDelta::days(7)
    } else {
        // Schedule for this coming Monday 8 AM
        monday_8am_local
    };
    Some(target)
}

/// Send a notification email to a list of recipients.
/// Uses the configured mail method (SMTP or Gmail).
/// Background loop that sends a daily summary email to admins at 8:00 AM local time.
async fn weekly_morning_email_loop(state: AppState) {
    loop {
        // Calculate duration until next Monday 8:00 AM local time
        let now = chrono::Local::now();

        let Some(target) = next_monday_8am_local(now) else {
            // Fallback: sleep 1 hour and retry
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
            continue;
        };

        let sleep_duration = (target - now)
            .to_std()
            .unwrap_or(tokio::time::Duration::from_secs(3600));
        info!(
            "Daily email: next run in {} seconds",
            sleep_duration.as_secs()
        );
        tokio::time::sleep(sleep_duration).await;

        // Gather data
        let current_season = get_current_season();
        let unimported = database::list_unimported_names(&state.db, current_season)
            .await
            .unwrap_or_default();

        let today = chrono::Local::now().date_naive();
        let week_end = today + chrono::Duration::days(7);
        let upcoming = database::get_upcoming_needs_deficit(&state.db, today, week_end)
            .await
            .unwrap_or_default();

        // Only send if there is content
        if unimported.is_empty() && upcoming.is_empty() {
            info!("Weekly email: nothing to report, skipping");
            // Sleep 60s to avoid double-send on the same minute
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            continue;
        }

        let admin_emails = database::get_admin_emails(&state.db)
            .await
            .unwrap_or_default();
        if admin_emails.is_empty() {
            info!("Weekly email: no admin emails configured, skipping");
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            continue;
        }

        // Build email body
        let mut body =
            String::from("<p>Bonjour,</p>\n<p>Voici le récapitulatif du lundi matin :</p>\n");

        if !unimported.is_empty() {
            body.push_str("<h3>Adhésions à importer</h3>\n<ul>\n");
            for (first_name, last_name, source) in &unimported {
                body.push_str(&format!(
                    "<li>{} {} <em>({})</em></li>\n",
                    first_name, last_name, source,
                ));
            }
            body.push_str("</ul>\n");
        }

        if !upcoming.is_empty() {
            body.push_str("<h3>Semaine à venir</h3>\n");
            body.push_str(&templates::render_upcoming_week_email(&upcoming));
        }

        body.push_str("<p><em>— PowPow pour AG'HIL</em></p>");

        let subject = "AGHIL — Récapitulatif du lundi matin";
        send_notification_email(&state, &admin_emails, subject, &body).await;
        info!("Weekly email: sent to {} admins", admin_emails.len());

        // Sleep 60s to avoid double-send on the same minute
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}

async fn send_notification_email(
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

async fn sync_users_from_helloasso(state: &AppState) -> anyhow::Result<(usize, usize)> {
    info!("Starting user synchronization from HelloAsso");

    let current_season = get_current_season();
    let unimported_before = database::count_unimported_memberships(&state.db, current_season)
        .await
        .unwrap_or(0);

    let mut user_count = 0;
    let mut membership_count = 0;

    // Fetch users directly from HelloAsso users endpoint
    info!("Fetching users from HelloAsso API...");
    let users_result = state.helloasso_client.get_users().await;

    // Store users in a HashMap for quick lookup
    let mut user_map: HashMap<String, User> = HashMap::new();

    match users_result {
        Ok(users) => {
            info!(
                "Successfully fetched {} users from HelloAsso users API",
                users.len()
            );
            for user in users {
                // Skip users without email since email is now the primary key
                if let Some(email) = user.email.clone() {
                    let db_user = User {
                        email: email.clone(),
                        first_name: user.first_name.clone(),
                        last_name: user.last_name.clone(),
                        phone: user.phone.clone(),
                        address: user.address.clone(),
                        city: user.city.clone(),
                        zip_code: user.zip_code.clone(),
                        country: user.country.clone(),
                        birth_date: user.birth_date,
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                        last_sync_at: Some(chrono::Utc::now()),
                    };

                    match database::upsert_user(&state.db, &db_user).await {
                        Ok(_) => {
                            debug!("Upserted user: {}", email);
                            user_map.insert(email.clone(), db_user);
                            user_count += 1;
                        }
                        Err(e) => {
                            error!("Failed to upsert user {}: {}", email, e);
                        }
                    }
                } else {
                    debug!("Skipping user without email");
                }
            }
            info!("Finished processing {} users from users API", user_count);
        }
        Err(e) => {
            error!("Failed to fetch users from HelloAsso: {}", e);
            // Try to get more detailed error information
            if let Some(source) = e.source() {
                error!("Underlying error: {}", source);
            }
        }
    }

    // Fetch orders/payments from HelloAsso (these contain user information)
    info!("Fetching orders from HelloAsso API...");
    match state.helloasso_client.get_orders().await {
        Ok(orders) => {
            let orders_count = orders.len();
            info!(
                "Successfully fetched {} orders from HelloAsso",
                orders_count
            );

            // Debug: Check custom fields in first 3 orders
            for (idx, order) in orders.iter().take(3).enumerate() {
                info!(
                    "Order {} (ID: {}) has {} items",
                    idx,
                    order.id,
                    order.items.len()
                );
                for (item_idx, item) in order.items.iter().enumerate() {
                    info!(
                        "  Item {} (ID: {}): custom_fields length = {}",
                        item_idx,
                        item.id,
                        item.custom_fields.len()
                    );
                    if !item.custom_fields.is_empty() {
                        for cf in &item.custom_fields {
                            info!(
                                "    CF: name={:?}, type={}, answer={:?}",
                                cf.name, cf.type_, cf.answer
                            );
                        }
                    }
                }
            }

            for order in orders {
                let payer = &order.payer;

                // Create user from payer if not already exists (fallback when users API fails)
                // Email is required for users now
                let payer_email = if let Some(email) = &payer.email {
                    email.clone()
                } else {
                    warn!("Skipping order {} - payer has no email", order.id);
                    continue;
                };

                // Extract phone from custom fields across all items in this order
                let custom_phone = order
                    .items
                    .iter()
                    .find_map(|item| get_custom_field_value(&item.custom_fields, "Téléphone"))
                    .or_else(|| {
                        order.items.iter().find_map(|item| {
                            get_custom_field_value(&item.custom_fields, "Telephone")
                        })
                    });

                // If we found a phone in custom fields and user exists, update their phone
                if let Some(phone) = &custom_phone
                    && let Some(existing_user) = user_map.get_mut(&payer_email)
                {
                    existing_user.phone = Some(phone.clone());
                    existing_user.updated_at = chrono::Utc::now();
                    // Update in database
                    match database::upsert_user(&state.db, existing_user).await {
                        Ok(_) => {
                            debug!("Updated phone for existing user: {}", payer_email);
                        }
                        Err(e) => {
                            error!("Failed to update phone for user {}: {}", payer_email, e);
                        }
                    }
                }

                if !user_map.contains_key(&payer_email) {
                    let payer_user = User {
                        email: payer_email.clone(),
                        first_name: payer.first_name.clone(),
                        last_name: payer.last_name.clone(),
                        phone: custom_phone.or_else(|| payer.phone.clone()),
                        address: payer.address.clone(),
                        city: payer.city.clone(),
                        zip_code: payer.zip_code.clone(),
                        country: payer.country.clone(),
                        birth_date: payer.birth_date,
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                        last_sync_at: Some(chrono::Utc::now()),
                    };

                    match database::upsert_user(&state.db, &payer_user).await {
                        Ok(_) => {
                            debug!("Created user from order payer: {}", payer_email);
                            user_map.insert(payer_email.clone(), payer_user);
                            user_count += 1;
                        }
                        Err(e) => {
                            error!("Failed to create user from payer: {}", e);
                        }
                    }
                }

                // Process each item in the order to create membership records
                for item in &order.items {
                    // Log custom fields for debugging
                    if item.custom_fields.is_empty() {
                        // Log that we got the item but no custom fields
                        if order.id == 168_183_762 {
                            // First order for debugging
                            info!("Order {} Item {} has NO custom fields", order.id, item.id);
                        }
                    } else {
                        info!(
                            "Order {} Item {} has {} custom fields",
                            order.id,
                            item.id,
                            item.custom_fields.len()
                        );
                        for field in &item.custom_fields {
                            info!(
                                "  Custom field: {} ({}): {:?}",
                                field.name.as_deref().unwrap_or("unnamed"),
                                field.type_,
                                field.answer
                            );
                        }
                    }

                    // Determine beneficiary name (from item.user or fallback to payer)
                    let (beneficiary_first, beneficiary_last) = if let Some(user) = &item.user {
                        (user.first_name.clone(), user.last_name.clone())
                    } else {
                        (payer.first_name.clone(), payer.last_name.clone())
                    };

                    // Create user from beneficiary if not already exists
                    if let Some(beneficiary) = &item.user
                        && let Some(beneficiary_email) = &beneficiary.email
                        && !user_map.contains_key(beneficiary_email)
                    {
                        // Extract phone from custom fields in this item
                        let custom_phone = get_custom_field_value(&item.custom_fields, "Téléphone")
                            .or_else(|| get_custom_field_value(&item.custom_fields, "Telephone"));

                        let beneficiary_user = User {
                            email: beneficiary_email.clone(),
                            first_name: beneficiary.first_name.clone(),
                            last_name: beneficiary.last_name.clone(),
                            phone: custom_phone.or_else(|| beneficiary.phone.clone()),
                            address: beneficiary.address.clone(),
                            city: beneficiary.city.clone(),
                            zip_code: beneficiary.zip_code.clone(),
                            country: beneficiary.country.clone(),
                            birth_date: beneficiary.birth_date,
                            created_at: chrono::Utc::now(),
                            updated_at: chrono::Utc::now(),
                            last_sync_at: Some(chrono::Utc::now()),
                        };

                        database::upsert_user(&state.db, &beneficiary_user).await?;
                        user_map.insert(beneficiary_email.clone(), beneficiary_user);
                        user_count += 1;
                        info!("Created user from order beneficiary: {}", beneficiary_email);
                    }

                    // Create a tier name from the item name or price category
                    let tier_name = item
                        .name
                        .clone()
                        .or_else(|| item.price_category.clone())
                        .or_else(|| Some(item.type_.clone()));

                    // Extract phone, email, and comment from this item's custom fields
                    let item_phone = get_custom_field_value(&item.custom_fields, "Téléphone")
                        .or_else(|| get_custom_field_value(&item.custom_fields, "Telephone"));
                    let item_email = get_custom_field_value(&item.custom_fields, "Adresse mail");
                    let item_comment = get_custom_field_value(&item.custom_fields, "Commentaire")
                        .or_else(|| get_custom_field_value(&item.custom_fields, "Comment"))
                        .or_else(|| get_custom_field_value(&item.custom_fields, "Message"));

                    // Create membership record
                    let membership = models::Membership {
                        helloasso_order_id: order.id,
                        helloasso_item_id: item.id,
                        payer_email: Some(payer_email.clone()), // Link to user via email

                        beneficiary_first_name: beneficiary_first,
                        beneficiary_last_name: beneficiary_last,
                        phone: item_phone,
                        email: item_email,

                        item_name: item.name.clone(),
                        item_type: Some(item.type_.clone()),
                        tier_name,
                        amount: Some(item.amount as i32),
                        order_date: Some(order.date),
                        comment: item_comment,

                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    };

                    match database::upsert_membership(&state.db, &membership).await {
                        Ok(_) => {
                            debug!("Created membership for order {} item {}", order.id, item.id);
                            membership_count += 1;
                        }
                        Err(e) => {
                            error!(
                                "Failed to upsert membership for order {} item {}: {}",
                                order.id, item.id, e
                            );
                        }
                    }
                }
            }
            info!("Finished processing {} orders", orders_count);
        }
        Err(e) => {
            error!("Failed to fetch orders from HelloAsso: {}", e);
            if let Some(source) = e.source() {
                error!("Underlying orders error: {}", source);
            }
        }
    }

    // Fetch events and their registrations
    match state.helloasso_client.get_events().await {
        Ok(events) => {
            for event in events {
                match state
                    .helloasso_client
                    .get_event_registrations(event.id)
                    .await
                {
                    Ok(users) => {
                        // Create membership records for event registrations
                        for user in users {
                            // Skip users without email
                            if let Some(user_email) = user.email.clone() {
                                let membership = models::Membership {
                                    helloasso_order_id: event.id,  // Using event id as order_id
                                    helloasso_item_id: 0, // No specific item for event registrations
                                    payer_email: Some(user_email), // Link to user via email

                                    beneficiary_first_name: user.first_name.clone(),
                                    beneficiary_last_name: user.last_name.clone(),
                                    phone: None, // Event registrations don't have custom fields
                                    email: None,

                                    item_name: Some(format!("Event registration: {}", event.title)),
                                    item_type: Some("event_registration".to_string()),
                                    tier_name: Some("event".to_string()),
                                    amount: Some(0), // No amount for event registrations
                                    order_date: event.start_date.or(Some(chrono::Utc::now())),
                                    comment: None,

                                    created_at: chrono::Utc::now(),
                                    updated_at: chrono::Utc::now(),
                                };

                                database::upsert_membership(&state.db, &membership).await?;
                                membership_count += 1;
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to fetch registrations for event {}: {}",
                            event.id, e
                        );
                    }
                }
            }
        }
        Err(e) => {
            error!("Failed to fetch events from HelloAsso: {}", e);
            // Try to get more detailed error information
            if let Some(source) = e.source() {
                error!("Underlying error: {}", source);
            }
        }
    }

    info!(
        "Synchronized {} users and {} memberships from HelloAsso",
        user_count, membership_count
    );

    // Check if new unimported memberships appeared and notify admins
    let unimported_after = database::count_unimported_memberships(&state.db, current_season)
        .await
        .unwrap_or(0);
    let new_unimported = unimported_after - unimported_before;
    if new_unimported > 0 {
        info!(
            "{} new memberships to import, notifying admins",
            new_unimported
        );
        let admin_emails = database::get_admin_emails(&state.db)
            .await
            .unwrap_or_default();
        if !admin_emails.is_empty() {
            let subject = format!(
                "AGHIL — {} nouvelle(s) adhésion(s) à importer",
                new_unimported
            );
            let html_body = format!(
                r"<p>Bonjour,</p>
<p><strong>{count}</strong> nouvelle(s) adhésion(s) HelloAsso sont en attente d'import ({total} au total).</p>
<p>Connectez-vous à AGHIL pour les traiter.</p>
<p><em>— PowPow pour AG'HIL</em></p>",
                count = new_unimported,
                total = unimported_after,
            );
            send_notification_email(state, &admin_emails, &subject, &html_body).await;
        }
    }

    Ok((user_count, membership_count))
}

// Database backup endpoint - pure Rust using COPY protocol
async fn backup_database(
    jar: SignedCookieJar,
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    // Check auth: either logged-in god or valid backup token
    let (caller_name, staff_id) = if let Some(god) = resolve_staff_if_god(&jar, &state).await {
        let name = format!("{} {}", god.first_name, god.last_name);
        (name, Some(god.id))
    } else {
        match check_automation_token(
            &params,
            &headers,
            &state.config.backup_token,
            "backup token",
        ) {
            Ok(name) => (name, None),
            Err(resp) => return resp,
        }
    };

    let _ = database::insert_audit(
        &state.db,
        staff_id,
        &caller_name,
        "Sauvegarde base de données",
        "",
    )
    .await;
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("aghil_backup_{}.sql", timestamp);

    match database::backup_all_tables(&state.db).await {
        Ok(sql) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/sql")
            .header(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            )
            .body(Body::from(sql))
            .unwrap(),
        Err(e) => {
            error!("Failed to create backup: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(Body::from(format!(
                    "<h1>Erreur</h1><p>Impossible de créer la sauvegarde: {}</p><p><a href=\"/\">Retour</a></p>",
                    e
                )))
                .unwrap()
        }
    }
}

// Mailchimp-compatible CSV export of all staff
async fn export_mailchimp(
    RequireAdmin(_staff): RequireAdmin,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match database::get_all_staff_with_ateliers(&state.db).await {
        Ok(staff_list) => {
            let mut csv =
                String::from("Email Address,First Name,Last Name,Address,Phone,Tags,Birthday\n");
            for (staff, atelier_names) in &staff_list {
                let phone = staff.phone.as_deref().unwrap_or("");
                let tags = atelier_names.join(", ");
                // CSV-escape fields that might contain commas or quotes
                csv.push_str(&format!(
                    "{},{},{},{},{},\"{}\",\n",
                    csv_escape(&staff.email),
                    csv_escape(&staff.first_name),
                    csv_escape(&staff.last_name),
                    "", // Address - not stored
                    csv_escape(phone),
                    tags.replace('"', "\"\""),
                ));
            }
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
                .header(
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"staff-mailchimp-export.csv\"",
                )
                .body(Body::from(csv))
                .unwrap()
        }
        Err(e) => {
            error!("Failed to export staff for Mailchimp: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::from(format!("Export error: {}", e)))
                .unwrap()
        }
    }
}

fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

// Restore page - shows the upload form
async fn restore_page(
    RequireGod(_staff): RequireGod,
    headers: HeaderMap,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    Html(templates::restore_page(&prefix))
}

// Database restore endpoint - accepts a SQL file upload and restores it (pure Rust)
async fn restore_database(
    RequireGod(god): RequireGod,
    headers: HeaderMap,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    let _ = database::insert_audit(
        &state.db,
        Some(god.id),
        &format!("{} {}", god.first_name, god.last_name),
        "Restauration base de données",
        "",
    )
    .await;

    // Extract the uploaded file
    let mut sql_content = Vec::new();
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("backup_file") {
            match field.bytes().await {
                Ok(bytes) => {
                    sql_content = bytes.to_vec();
                }
                Err(e) => {
                    error!("Failed to read uploaded file: {}", e);
                    return Html(templates::restore_result(
                        &prefix,
                        false,
                        &format!("Erreur de lecture du fichier: {}", e),
                    ));
                }
            }
        }
    }

    if sql_content.is_empty() {
        return Html(templates::restore_result(
            &prefix,
            false,
            "Aucun fichier reçu",
        ));
    }

    info!(
        "Restoring database from uploaded file ({} bytes)",
        sql_content.len()
    );

    let sql_str = match std::str::from_utf8(&sql_content) {
        Ok(s) => s,
        Err(e) => {
            error!("Uploaded file is not valid UTF-8: {}", e);
            return Html(templates::restore_result(
                &prefix,
                false,
                "Le fichier n'est pas un fichier SQL valide (encodage UTF-8 invalide)",
            ));
        }
    };

    match database::restore_from_sql(&state.db, sql_str).await {
        Ok(()) => {
            info!("Database restore completed successfully");
            Html(templates::restore_result(
                &prefix,
                true,
                "Base de données restaurée avec succès!",
            ))
        }
        Err(e) => {
            error!("Database restore failed: {}", e);
            Html(templates::restore_result(
                &prefix,
                false,
                &format!("Erreur de restauration: {}", e),
            ))
        }
    }
}

#[derive(Serialize)]
struct UserListResponse {
    users: Vec<User>,
    page: i64,
    limit: i64,
    total: i64,
    total_pages: i64,
}

type Season = i16;

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

// Photo handlers
async fn photo_page(
    RequireStaff(staff): RequireStaff,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);

    match database::get_all_photos(&state.db).await {
        Ok(photos) => Html(templates::photo_page(&prefix, &photos, staff.is_admin)),
        Err(e) => {
            error!("Failed to get photos: {}", e);
            Html(templates::photo_page(&prefix, &[], false))
        }
    }
}

async fn display_photo(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> impl IntoResponse {
    match database::get_photo_by_id(&state.db, id).await {
        Ok(Some(photo)) => {
            let mut response = Response::new(Body::from(photo.photo_data));
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_str(&photo.mime_type).unwrap(),
            );
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                header::HeaderValue::from_static("public, max-age=86400, immutable"),
            );
            response
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            error!("Failed to get photo: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn upload_photo(
    RequireAdmin(_staff): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    info!("upload_photo handler entered");
    let prefix = get_prefix(&headers);
    let mut photographer_id: Option<uuid::Uuid> = None;
    let mut photo_data: Option<(Vec<u8>, String)> = None;
    let mut multipart_error: Option<String> = None;

    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                let name = field.name().unwrap_or("").to_string();
                match name.as_str() {
                    "photographer_id" => {
                        let text = field.text().await.unwrap_or_default();
                        photographer_id = uuid::Uuid::parse_str(text.trim()).ok();
                    }
                    "photo" => {
                        let content_type = field
                            .content_type()
                            .unwrap_or("application/octet-stream")
                            .to_string();
                        match field.bytes().await {
                            Ok(data) => {
                                info!("Photo upload: received {} bytes", data.len());
                                photo_data = Some((data.to_vec(), content_type));
                            }
                            Err(e) => {
                                error!("Failed to read photo data: {}", e);
                                multipart_error = Some(format!("Erreur lecture fichier: {}", e));
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(None) => break,
            Err(e) => {
                error!("Multipart stream error: {}", e);
                multipart_error = Some(format!("Erreur upload: {}", e));
                break;
            }
        }
    }

    if let Some(err) = multipart_error {
        return (StatusCode::BAD_REQUEST, Html(format!(
            r#"<html><body><h2>Erreur</h2><p>{}</p><a href="{}/photos">Retour</a></body></html>"#,
            templates::escape_html_public(&err), prefix
        ))).into_response();
    }

    match (photo_data, photographer_id) {
        (Some((data, content_type)), Some(pid)) => {
            match database::create_photo(&state.db, data, content_type, pid).await {
                Ok(_) => {
                    // Refresh photo-of-the-day background
                    if let Ok(Some((photo, name))) = database::get_photo_of_the_day(&state.db).await
                    {
                        templates::set_photo_bg(format!("/photos/{}", photo.id), name);
                    }
                    Redirect::to(&format!("{}/photos", prefix)).into_response()
                }
                Err(e) => {
                    error!("Failed to upload photo: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, Html(format!(
                        r#"<html><body><h2>Erreur</h2><p>Échec de l'enregistrement: {}</p><a href="{}/photos">Retour</a></body></html>"#,
                        templates::escape_html_public(&e.to_string()), prefix
                    ))).into_response()
                }
            }
        }
        (None, _) => {
            error!("Photo upload: no photo data received");
            (StatusCode::BAD_REQUEST, Html(format!(
                r#"<html><body><h2>Erreur</h2><p>Aucune photo reçue</p><a href="{}/photos">Retour</a></body></html>"#,
                prefix
            ))).into_response()
        }
        (_, None) => {
            error!("Photo upload: no photographer selected");
            (StatusCode::BAD_REQUEST, Html(format!(
                r#"<html><body><h2>Erreur</h2><p>Aucun photographe sélectionné</p><a href="{}/photos">Retour</a></body></html>"#,
                prefix
            ))).into_response()
        }
    }
}

async fn delete_photo(
    RequireAdmin(_staff): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    match database::delete_photo(&state.db, id).await {
        Ok(success) => {
            if success {
                // Refresh photo-of-the-day background
                if let Ok(Some((photo, name))) = database::get_photo_of_the_day(&state.db).await {
                    templates::set_photo_bg(format!("/photos/{}", photo.id), name);
                }
                Redirect::to(&format!("{}/photos", prefix)).into_response()
            } else {
                StatusCode::NOT_FOUND.into_response()
            }
        }
        Err(e) => {
            error!("Failed to delete photo: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn test_next_monday_8am_from_1am() {
        let monday_1am = chrono::Local
            .with_ymd_and_hms(2026, 2, 23, 1, 0, 0)
            .unwrap();
        let next_monday_8am_local = next_monday_8am_local(monday_1am).unwrap();
        assert_eq!(
            next_monday_8am_local,
            chrono::Local
                .with_ymd_and_hms(2026, 2, 23, 8, 0, 0)
                .unwrap()
        );
    }

    #[test]
    fn test_next_monday_8am_from_9am() {
        let monday_9am = chrono::Local
            .with_ymd_and_hms(2026, 2, 23, 9, 0, 0)
            .unwrap();
        let next_monday_8am_local = next_monday_8am_local(monday_9am).unwrap();
        assert_eq!(
            next_monday_8am_local,
            chrono::Local.with_ymd_and_hms(2026, 3, 2, 8, 0, 0).unwrap()
        );
    }

    #[test]
    fn test_next_monday_8am_from_tuesday() {
        let tuesday_5am = chrono::Local
            .with_ymd_and_hms(2026, 2, 24, 5, 0, 0)
            .unwrap();
        let next_monday_8am_local = next_monday_8am_local(tuesday_5am).unwrap();
        assert_eq!(
            next_monday_8am_local,
            chrono::Local.with_ymd_and_hms(2026, 3, 2, 8, 0, 0).unwrap()
        );
    }
}
