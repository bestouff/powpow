use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use maud::html;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::error;

use crate::{
    AppState, auth::RequireAdmin, database, get_current_season, get_prefix, get_season_for, models,
    templates,
};

pub async fn list_users(
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

                let is_non_membership = matches!(
                    membership.item_type.as_deref(),
                    Some("Registration" | "Donation")
                );

                // Update stats (skip non-membership items: Forfait, Don)
                if !is_non_membership {
                    total_count += 1;
                    if has_staff {
                        imported_count += 1;
                    } else {
                        not_imported_count += 1;
                    }
                }

                // Apply filter (hide imported and non-membership items in "À importer" view)
                if only_not_imported && (has_staff || is_non_membership) {
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

            templates::membership_list_with_filters(
                memberships_with_status,
                search.cloned(),
                only_not_imported,
                total_count,
                imported_count,
                not_imported_count,
                get_current_season(),
                &prefix,
            )
        }
        (Err(e), _) | (_, Err(e)) => {
            error!("Error fetching memberships: {}", e);
            html! { p { "Error loading memberships: " (e) } }
        }
    }
}

pub async fn get_user(
    RequireAdmin(_staff): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    axum::extract::Path(email): axum::extract::Path<String>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    match database::get_user_by_email(&state.db, email).await {
        Ok(Some(user)) => (StatusCode::OK, templates::user_detail(user, &prefix)),
        Ok(None) => (StatusCode::NOT_FOUND, html! { p { "User not found" } }),
        Err(e) => {
            error!("Error fetching user: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                html! { p { "Error loading user: " (e) } },
            )
        }
    }
}

pub async fn import_staff(
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
                    templates::already_imported_page(membership, season, &prefix),
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
                templates::import_staff_form(
                    membership,
                    season,
                    candidates,
                    payer_email.as_deref(),
                    false,
                    &prefix,
                ),
            )
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            html! { p { "Membership not found" } },
        ),
        Err(e) => {
            error!("Error fetching membership: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                html! { p { "Error loading membership: " (e) } },
            )
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ImportStaffForm {
    action: String,           // "create" or "update"
    staff_id: Option<String>, // UUID of existing staff (for update)
    first_name: String,
    last_name: String,
    email: String,
    phone: Option<String>,
    comment: Option<String>,
}

pub async fn do_import_staff(
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
                html! { p { "Membership not found" } },
            );
        }
        Err(e) => {
            error!("Error fetching membership: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                html! { p { "Error loading membership: " (e) } },
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
                return (StatusCode::BAD_REQUEST, html! { p { "Invalid staff ID" } });
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
            return (StatusCode::BAD_REQUEST, html! { p { "Invalid action" } });
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
                html! {
                    meta http-equiv="refresh" content=(format!("0;url={}/users", prefix)) {}
                    p { "Redirecting..." }
                },
            )
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("ALREADY_IMPORTED") {
                // Race condition: someone else already imported this membership
                (
                    StatusCode::CONFLICT,
                    templates::import_result(
                        false,
                        "Cette adhésion a déjà été importée par quelqu'un d'autre.",
                        &prefix,
                    ),
                )
            } else if error_msg.contains("DUPLICATE_NAME") {
                // A staff with this name already exists
                (
                    StatusCode::CONFLICT,
                    templates::import_result(
                        false,
                        "Un staff avec ce nom existe déjà. Utilisez l'option \"Mettre à jour\" pour ajouter une adhésion à un staff existant.",
                        &prefix,
                    ),
                )
            } else {
                error!("Error importing staff: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    templates::import_result(
                        false,
                        &format!("Erreur lors de l'import: {}", e),
                        &prefix,
                    ),
                )
            }
        }
    }
}

#[derive(serde::Serialize)]
pub struct UserListResponse {
    users: Vec<crate::models::User>,
    page: i64,
    limit: i64,
    total: i64,
    total_pages: i64,
}

pub async fn api_list_users(
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
