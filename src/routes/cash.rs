use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::Datelike;
use maud::html;
use serde::Deserialize;
use tracing::error;

use crate::{
    AppState, auth::RequireAdmin, database, get_current_season, get_prefix,
    send_notification_email, templates,
};

pub async fn list_cash(
    RequireAdmin(_staff): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    let show_form = params.get("form").is_some_and(|f| f == "1");

    if show_form {
        return templates::cash_form(&prefix);
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

            templates::cash_list(payments_with_status, current_season, &prefix)
        }
        Err(e) => {
            error!("Error fetching cash payments: {}", e);
            html! { p { "Error loading cash payments: " (e) } }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateCashForm {
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

pub async fn create_cash(
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
                html! { p { "Date invalide: " (e) } },
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
                html! {
                    meta http-equiv="refresh" content=(format!("0;url={}/cash", prefix)) {}
                    p { "Redirecting..." }
                },
            )
        }
        Err(e) => {
            error!("Error creating cash payment: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                html! { p { "Erreur: " (e) } },
            )
        }
    }
}

pub async fn import_cash(
    RequireAdmin(_staff): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
    axum::extract::Path(cash_id): axum::extract::Path<uuid::Uuid>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);

    let cash = match database::get_cash_by_id(&state.db, cash_id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, html! { p { "Paiement non trouvé" } });
        }
        Err(e) => {
            error!("Error fetching cash payment: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                html! { p { "Erreur: " (e) } },
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
            templates::import_result(true, "Ce paiement a déjà été importé.", &prefix),
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
        templates::cash_import_form(&cash, season, candidates, &prefix),
    )
}

#[derive(Debug, Deserialize)]
pub struct CashImportForm {
    action: String,
    staff_id: Option<String>,
    first_name: String,
    last_name: String,
    email: String,
    phone: Option<String>,
    comment: Option<String>,
}

pub async fn do_import_cash(
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
            return (StatusCode::NOT_FOUND, html! { p { "Paiement non trouvé" } });
        }
        Err(e) => {
            error!("Error fetching cash payment: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                html! { p { "Erreur: " (e) } },
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
            templates::import_result(false, "Ce paiement a déjà été importé.", &prefix),
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
                return (StatusCode::BAD_REQUEST, html! { p { "Invalid staff ID" } });
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
            return (StatusCode::BAD_REQUEST, html! { p { "Action invalide" } });
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
                html! {
                    meta http-equiv="refresh" content=(format!("0;url={}/cash", prefix)) {}
                    p { "Redirecting..." }
                },
            )
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("ALREADY_IMPORTED") {
                (
                    StatusCode::CONFLICT,
                    templates::import_result(
                        false,
                        "Ce paiement a déjà été importé par quelqu'un d'autre.",
                        &prefix,
                    ),
                )
            } else {
                error!("Error importing cash payment: {}", e);
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
