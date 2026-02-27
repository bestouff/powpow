use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use axum_extra::extract::cookie::SignedCookieJar;
use maud::html;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::error;

use crate::{
    AppState,
    auth::{RequireAdmin, RequireChief, RequireStaff},
    database, get_prefix, models, templates,
};

pub async fn calendar_view(
    RequireStaff(me_staff): RequireStaff,
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
            return (StatusCode::NOT_FOUND, html! { p { "Atelier not found" } }).into_response();
        }
        Err(e) => {
            error!("Error fetching atelier: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                html! { p { "Error: " (e) } },
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
                html! { p { "Error: " (e) } },
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
                html! { p { "Error: " (e) } },
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
                html! { p { "Error: " (e) } },
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
                html! { p { "Error: " (e) } },
            )
                .into_response();
        }
    };

    // Build HashMap<(need_id, staff_id), (first_half, second_half)>
    let mut presence_map = HashMap::new();
    for (need_id, staff_id, first_half, second_half) in presence_rows {
        presence_map.insert((need_id, staff_id), (first_half, second_half));
    }

    // Fetch opening day status for the days shown
    let calendar_days: Vec<chrono::NaiveDate> = needs.iter().map(|n| n.day).collect();
    let opening_days = database::get_opening_days_for_dates(&state.db, &calendar_days)
        .await
        .unwrap_or_default();

    templates::calendar(
        &atelier,
        &needs,
        &staff_list,
        &presence_map,
        &all_ateliers,
        &prefix,
        me.as_ref().map(|s| s.id),
        me.as_ref().is_some_and(|s| s.is_admin),
        &opening_days,
    )
    .into_response()
}

#[derive(Debug, Deserialize)]
pub struct TogglePresenceRequest {
    needs_id: uuid::Uuid,
    staff_id: uuid::Uuid,
    half: String,
    value: bool,
}

pub async fn toggle_presence_api(
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

// --- Opening day management (admin only) ---

#[derive(Debug, Deserialize)]
pub struct CreateOpeningDayRequest {
    day: String,
}

pub async fn api_create_opening_day(
    RequireAdmin(me): RequireAdmin,
    State(state): State<AppState>,
    Json(payload): Json<CreateOpeningDayRequest>,
) -> impl IntoResponse {
    let Ok(day) = chrono::NaiveDate::parse_from_str(&payload.day, "%Y-%m-%d") else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Format de date invalide (YYYY-MM-DD)"})),
        );
    };

    // Create the opening day record
    match database::create_opening_day(&state.db, day).await {
        Ok(_) => {}
        Err(e) => {
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({"error": e.to_string()})),
            );
        }
    }

    // Create needs for all ateliers with opening_day_typical_needed > 0
    let ateliers = match database::get_all_ateliers(&state.db).await {
        Ok(a) => a,
        Err(e) => {
            error!("Error fetching ateliers for opening day: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            );
        }
    };

    let mut created_count = 0_u32;
    for atelier in &ateliers {
        if atelier.opening_day_typical_needed > 0 {
            match database::upsert_need(
                &state.db,
                atelier.id,
                day,
                atelier.opening_day_typical_needed,
                atelier.default_nightly,
            )
            .await
            {
                Ok(_) => created_count += 1,
                Err(e) => {
                    error!(
                        "Error creating need for atelier {} on {}: {}",
                        atelier.name, day, e
                    );
                }
            }
        }
    }

    let _ = database::insert_audit(
        &state.db,
        Some(me.id),
        &format!("{} {}", me.first_name, me.last_name),
        "Jour d'ouverture créé",
        &format!("{day} — {created_count} besoins créés"),
    )
    .await;

    (
        StatusCode::OK,
        Json(
            serde_json::json!({"success": true, "day": payload.day, "needs_created": created_count}),
        ),
    )
}

#[derive(Debug, Deserialize)]
pub struct UpdateOpeningDayStatusRequest {
    day: String,
    status: String,
}

pub async fn api_update_opening_day_status(
    RequireAdmin(me): RequireAdmin,
    State(state): State<AppState>,
    Json(payload): Json<UpdateOpeningDayStatusRequest>,
) -> impl IntoResponse {
    let Ok(day) = chrono::NaiveDate::parse_from_str(&payload.day, "%Y-%m-%d") else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Format de date invalide (YYYY-MM-DD)"})),
        );
    };

    let status = match payload.status.as_str() {
        "validated" => models::OpeningDayStatus::Validated,
        "canceled" => models::OpeningDayStatus::Canceled,
        "reserved" => models::OpeningDayStatus::Reserved,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Statut invalide"})),
            );
        }
    };

    // Note: we intentionally keep needs in place when canceling, so the day
    // column stays visible in calendar views with the "Annulé" tag.

    match database::update_opening_day_status(&state.db, day, status).await {
        Ok(true) => {
            let status_label = match status {
                models::OpeningDayStatus::Validated => "Confirmé",
                models::OpeningDayStatus::Canceled => "Annulé",
                models::OpeningDayStatus::Reserved => "Prévu",
            };
            let _ = database::insert_audit(
                &state.db,
                Some(me.id),
                &format!("{} {}", me.first_name, me.last_name),
                "Jour d'ouverture modifié",
                &format!("{day} → {status_label}"),
            )
            .await;
            (
                StatusCode::OK,
                Json(serde_json::json!({"success": true, "status": payload.status})),
            )
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Jour d'ouverture non trouvé"})),
        ),
        Err(e) => {
            error!("Error updating opening day status: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }
}

// --- Calendar landing (public: redirect to first atelier, or editor for chiefs) ---

pub async fn calendar_landing(
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
            return html! { p { "Error: " (e) } }.into_response();
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

    // Collect all days shown in the calendar and fetch opening day status
    let calendar_days: Vec<chrono::NaiveDate> =
        future_needs.iter().map(|(n, _, _)| n.day).collect();
    let opening_days = database::get_opening_days_for_dates(&state.db, &calendar_days)
        .await
        .unwrap_or_default();

    let is_admin = staff.as_ref().is_some_and(|s| s.is_admin || s.is_god);

    templates::calendar_editor(
        &ateliers,
        &editable_ids,
        &future_needs,
        &prefix,
        staff.is_some(),
        is_admin,
        &opening_days,
    )
    .into_response()
}

// --- Calendar editor (needs management) ---

#[derive(Debug, Deserialize)]
pub struct NeedsQuery {
    atelier_id: uuid::Uuid,
}

pub async fn api_get_needs(
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

pub async fn api_get_need_days(
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
pub struct NeedsByDayQuery {
    day: String,
}

pub async fn api_get_needs_by_day(
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
pub struct UpsertNeedRequest {
    atelier_id: uuid::Uuid,
    day: String,
    quantity: i16,
    #[serde(default)]
    nightly: bool,
}

pub async fn api_upsert_need(
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
pub struct DeleteNeedRequest {
    atelier_id: uuid::Uuid,
    day: String,
}

pub async fn api_delete_need(
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
