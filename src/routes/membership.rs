use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use maud::html;
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
        (Ok(memberships_with_users), Ok(imported_map)) => {
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
                let staff_id = imported_map
                    .get(&(membership.helloasso_item_id, season))
                    .copied();

                let is_non_membership = matches!(
                    membership.item_type.as_deref(),
                    Some("Registration" | "Donation")
                );

                // Update stats (skip non-membership items: Forfait, Don)
                if !is_non_membership {
                    total_count += 1;
                    if staff_id.is_some() {
                        imported_count += 1;
                    } else {
                        not_imported_count += 1;
                    }
                }

                // Apply filter (hide imported and non-membership items in "À importer" view)
                if only_not_imported && (staff_id.is_some() || is_non_membership) {
                    continue;
                }

                let idx = memberships_with_status.len();

                // Track imported memberships by name+season for double detection
                if staff_id.is_some() {
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
                        staff_id,
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

            // Sort: "À importer" first (not imported, not ignored), then rest, by date desc
            memberships_with_status.sort_by(|a, b| {
                let a_actionable = a.1.staff_id.is_none()
                    && !matches!(
                        a.1.membership.item_type.as_deref(),
                        Some("Registration" | "Donation")
                    );
                let b_actionable = b.1.staff_id.is_none()
                    && !matches!(
                        b.1.membership.item_type.as_deref(),
                        Some("Registration" | "Donation")
                    );
                // Actionable items first, then by date descending
                match b_actionable.cmp(&a_actionable) {
                    std::cmp::Ordering::Equal => {
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
