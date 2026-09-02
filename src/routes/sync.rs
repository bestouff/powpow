use axum::{
    Json,
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::SignedCookieJar;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use tracing::{debug, error, info, warn};

use crate::{
    AppState, check_automation_token, database, get_current_season, models, resolve_staff_if_admin,
    send_notification_email,
};
use models::User;

/// Helper function to extract custom field value by name
fn get_custom_field_value(
    custom_fields: &[models::HelloAssoCustomField],
    field_name: &str,
) -> Option<String> {
    custom_fields
        .iter()
        .find(|f| f.name.as_deref() == Some(field_name))
        .and_then(|f| f.answer.clone())
}

/// Trim an `Option<String>` in place (removes leading/trailing whitespace).
fn trimmed(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Process a single `HelloAsso` order: upsert users (payer + beneficiaries)
/// and membership records.  Returns `(users_upserted, memberships_upserted)`.
///
/// `user_map` is an accumulator shared across orders so we skip duplicates.
async fn process_order(
    order: &models::HelloAssoOrder,
    state: &AppState,
    user_map: &mut HashMap<String, User>,
) -> anyhow::Result<(usize, usize)> {
    let mut user_count: usize = 0;
    let mut membership_count: usize = 0;

    let payer = &order.payer;

    // Email is required — skip orders without payer email
    let payer_email = if let Some(email) = &payer.email {
        email.clone()
    } else {
        warn!("Skipping order {} — payer has no email", order.id);
        return Ok((0, 0));
    };

    // Extract phone from custom fields across all items in this order
    let custom_phone = order
        .items
        .iter()
        .find_map(|item| get_custom_field_value(&item.custom_fields, "Téléphone"))
        .or_else(|| {
            order
                .items
                .iter()
                .find_map(|item| get_custom_field_value(&item.custom_fields, "Telephone"))
        });

    // If we found a phone in custom fields and user already tracked, update their phone
    if let Some(phone) = &custom_phone
        && let Some(existing_user) = user_map.get_mut(&payer_email)
    {
        existing_user.phone = Some(phone.clone());
        existing_user.updated_at = chrono::Utc::now();
        match database::upsert_user(&state.db, existing_user).await {
            Ok(_) => debug!("Updated phone for existing user: {}", payer_email),
            Err(e) => error!("Failed to update phone for user {}: {}", payer_email, e),
        }
    }

    // Create payer user if not yet tracked
    if !user_map.contains_key(&payer_email) {
        let payer_user = User {
            email: payer_email.clone(),
            first_name: trimmed(payer.first_name.clone()),
            last_name: trimmed(payer.last_name.clone()),
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
            Err(e) => error!("Failed to create user from payer: {}", e),
        }
    }

    // Process each item in the order to create membership records
    for item in &order.items {
        // Determine beneficiary name (from item.user or fallback to payer)
        let (beneficiary_first, beneficiary_last) = if let Some(user) = &item.user {
            (
                trimmed(user.first_name.clone()),
                trimmed(user.last_name.clone()),
            )
        } else {
            (
                trimmed(payer.first_name.clone()),
                trimmed(payer.last_name.clone()),
            )
        };

        // Create user from beneficiary if not already tracked
        if let Some(beneficiary) = &item.user
            && let Some(beneficiary_email) = &beneficiary.email
            && !user_map.contains_key(beneficiary_email)
        {
            let custom_phone = get_custom_field_value(&item.custom_fields, "Téléphone")
                .or_else(|| get_custom_field_value(&item.custom_fields, "Telephone"));

            let beneficiary_user = User {
                email: beneficiary_email.clone(),
                first_name: trimmed(beneficiary.first_name.clone()),
                last_name: trimmed(beneficiary.last_name.clone()),
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

        // Build tier name from item name / price category / type
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

        let membership = models::Membership {
            helloasso_order_id: order.id,
            helloasso_item_id: item.id,
            payer_email: Some(payer_email.clone()),

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

    Ok((user_count, membership_count))
}

/// Envelope for `HelloAsso` webhook notifications.
#[derive(Debug, Deserialize)]
struct WebhookNotification {
    data: serde_json::Value,
    #[serde(rename = "eventType")]
    event_type: String,
}

/// Manual sync (GET): blocks until sync completes, returns HTML result.
/// Used by admins clicking "Synchronisation manuelle" in the web UI.
pub async fn sync_users(
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
            info!(
                "Manual sync completed: {} users, {} memberships",
                user_count, membership_count
            );
        }
        Err(e) => {
            error!("Error syncing users: {}", e);
            let _ = database::insert_audit(
                &state.db,
                staff_id,
                &caller_name,
                "Synchronisation HelloAsso (échec)",
                &e.to_string(),
            )
            .await;
        }
    }
    Redirect::to("/online").into_response()
}

/// Webhook handler (POST): parses the notification payload, immediately
/// upserts the user/membership from Order events, then spawns a debounced
/// full re-sync in the background. Returns 200 immediately so `HelloAsso`
/// considers the notification delivered.
pub async fn sync_webhook(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    body: Bytes,
) -> Response {
    // Check auth: sync token only (webhooks have no session cookie)
    if let Err(resp) =
        check_automation_token(&params, &headers, &state.config.sync_token, "sync token")
    {
        return resp;
    }

    // Log the raw notification body for diagnostics
    let body_str = std::str::from_utf8(&body).unwrap_or("<non-UTF8>");
    info!(
        "HelloAsso notification received ({} bytes): {}",
        body.len(),
        body_str
    );

    // ── Immediate import from webhook payload ───────────────────
    // Parse the notification envelope to check the event type.
    // Only "Order" events carry the full order data with custom fields;
    // "Payment" events have a different shape and are skipped here.
    if let Ok(notif) = serde_json::from_slice::<WebhookNotification>(&body) {
        if notif.event_type == "Order" {
            match serde_json::from_value::<models::HelloAssoOrder>(notif.data) {
                Ok(order) => {
                    info!(
                        "Webhook: processing Order {} directly ({} items)",
                        order.id,
                        order.items.len()
                    );
                    let unimported_before =
                        database::count_unimported_memberships(&state.db, get_current_season())
                            .await
                            .unwrap_or(0);
                    let mut user_map = HashMap::new();
                    match process_order(&order, &state, &mut user_map).await {
                        Ok((u, m)) => {
                            info!(
                                "Webhook: immediate import done — {} users, {} memberships",
                                u, m
                            );
                            let _ = database::insert_audit(
                                &state.db,
                                None,
                                "Automation (HelloAsso webhook)",
                                "Import direct webhook",
                                &format!(
                                    "Order {} — {} utilisateur(s), {} adhésion(s)",
                                    order.id, u, m
                                ),
                            )
                            .await;

                            // Notify admins if new unimported memberships appeared
                            if m > 0 {
                                notify_new_memberships(&state, unimported_before).await;
                            }
                        }
                        Err(e) => {
                            error!("Webhook: failed to process order {}: {}", order.id, e);
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "Webhook: failed to deserialize Order data: {} — will rely on background sync",
                        e
                    );
                }
            }
        } else {
            info!(
                "Webhook: event type '{}' — skipping direct import (background sync will handle it)",
                notif.event_type
            );
        }
    } else {
        warn!("Webhook: failed to parse notification envelope — will rely on background sync");
    }

    // ── Debounced background full re-sync ───────────────────────
    // Only spawn if no sync is already running.  This prevents the
    // duplicate parallel syncs from Order + Payment notifications.
    if state
        .sync_in_progress
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        tokio::spawn(async move {
            // Small delay to let HelloAsso finish indexing the order
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;

            info!("Background sync started (triggered by HelloAsso webhook)");
            match sync_users_from_helloasso(&state).await {
                Ok((user_count, membership_count)) => {
                    let _ = database::insert_audit(
                        &state.db,
                        None,
                        "Automation (HelloAsso webhook)",
                        "Synchronisation HelloAsso",
                        &format!(
                            "{} utilisateurs, {} adhésions",
                            user_count, membership_count
                        ),
                    )
                    .await;
                    info!(
                        "Background sync complete: {} users, {} memberships",
                        user_count, membership_count
                    );
                }
                Err(e) => {
                    error!("Background sync failed: {}", e);
                    let _ = database::insert_audit(
                        &state.db,
                        None,
                        "Automation (HelloAsso webhook)",
                        "Synchronisation HelloAsso (échec)",
                        &e.to_string(),
                    )
                    .await;
                }
            }
            state.sync_in_progress.store(false, Ordering::SeqCst);
        });
    } else {
        info!("Webhook: background sync already in progress, skipping");
    }

    // Return 200 immediately so HelloAsso considers the notification delivered
    StatusCode::OK.into_response()
}

pub async fn api_sync_users(
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
            let _ = database::insert_audit(
                &state.db,
                staff_id,
                &caller_name,
                "Synchronisation HelloAsso API (échec)",
                &e.to_string(),
            )
            .await;
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

pub async fn sync_users_from_helloasso(state: &AppState) -> anyhow::Result<(usize, usize)> {
    info!("Starting user synchronization from HelloAsso");

    let mut user_count = 0;
    let mut membership_count = 0;

    // Snapshot the pending queue before importing, so we only alert on a fresh batch.
    let unimported_before = database::count_unimported_memberships(&state.db, get_current_season())
        .await
        .unwrap_or(0);

    // Track users we've already upserted (keyed by email) to avoid duplicates
    let mut user_map: HashMap<String, User> = HashMap::new();

    // Fetch orders/payments from HelloAsso (these contain user information)
    info!("Fetching orders from HelloAsso API...");
    let orders = state.helloasso_client.get_orders().await.map_err(|e| {
        error!("Failed to fetch orders from HelloAsso: {e}");
        e
    })?;

    let orders_count = orders.len();
    info!(
        "Successfully fetched {} orders from HelloAsso",
        orders_count
    );

    for order in &orders {
        match process_order(order, state, &mut user_map).await {
            Ok((u, m)) => {
                user_count += u;
                membership_count += m;
            }
            Err(e) => {
                error!("Failed to process order {}: {}", order.id, e);
            }
        }
    }
    info!("Finished processing {} orders", orders_count);

    info!(
        "Synchronized {} users and {} memberships from HelloAsso",
        user_count, membership_count
    );

    // Check if new unimported memberships appeared and notify admins
    notify_new_memberships(state, unimported_before).await;

    Ok((user_count, membership_count))
}

/// Check for unimported memberships and email admins when new ones appear.
///
/// Only notifies when the queue was empty before the current import, so a burst
/// of memberships while a previous batch is still pending doesn't re-email admins.
async fn notify_new_memberships(state: &AppState, unimported_before: i64) {
    let current_season = get_current_season();
    let unimported = database::count_unimported_memberships(&state.db, current_season)
        .await
        .unwrap_or(0);
    if unimported_before == 0 && unimported > 0 {
        info!("{} unimported membership(s), notifying admins", unimported);
        let admin_emails = database::get_admin_emails(&state.db)
            .await
            .unwrap_or_default();
        if !admin_emails.is_empty() {
            let subject = format!(
                "{} — {} adhésion(s) à importer",
                state.config.entity_name, unimported
            );
            let html_body = format!(
                r"<p>Bonjour,</p>
<p><strong>{count}</strong> adhésion(s) HelloAsso sont en attente d'import.</p>
<p>Connectez-vous à PowPow pour les traiter.</p>
{sig}",
                count = unimported,
                sig = crate::email_signature(&state.config.entity_name),
            );
            send_notification_email(state, &admin_emails, &subject, &html_body).await;
        }
    }
}
