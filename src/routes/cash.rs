use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use maud::html;
use serde::Deserialize;
use tracing::error;

use crate::{AppState, auth::RequireAdmin, database, get_current_season, get_prefix, templates};

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
                let staff_id = database::get_staff_for_cash(&state.db, cash.id)
                    .await
                    .unwrap_or(None);
                payments_with_status.push((cash, staff_id));
            }

            // Sort: not-yet-imported first, then by date (most recent first)
            payments_with_status.sort_by(|a, b| match a.1.is_some().cmp(&b.1.is_some()) {
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
            if is_membership && let Err(e) = super::auto_import::import_pending(&state).await {
                error!("Cash membership auto-import failed: {e}");
            }

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
