use axum::{
    Json,
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
};
use axum_extra::extract::cookie::SignedCookieJar;
use std::collections::HashMap;
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

/// Webhook handler (POST): returns 200 immediately, runs sync in background.
/// `HelloAsso` sends a POST with a JSON notification body; we must respond
/// quickly (200) or they will retry with exponential back-off.
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

    // Log the notification body for diagnostics
    if let Ok(json) = std::str::from_utf8(&body) {
        info!(
            "HelloAsso notification received ({} bytes): {}",
            body.len(),
            json
        );
    } else {
        info!(
            "HelloAsso notification received ({} bytes, non-UTF8)",
            body.len()
        );
    }

    // Spawn the full sync in the background so we can return 200 immediately
    tokio::spawn(async move {
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
            }
        }
    });

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
                "{} — {} nouvelle(s) adhésion(s) à importer",
                state.config.entity_name, new_unimported
            );
            let html_body = format!(
                r"<p>Bonjour,</p>
<p><strong>{count}</strong> nouvelle(s) adhésion(s) HelloAsso sont en attente d'import ({total} au total).</p>
<p>Connectez-vous à PowPow pour les traiter.</p>
{sig}",
                count = new_unimported,
                total = unimported_after,
                sig = crate::email_signature(&state.config.entity_name),
            );
            send_notification_email(state, &admin_emails, &subject, &html_body).await;
        }
    }

    Ok((user_count, membership_count))
}
