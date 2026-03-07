use axum::{
    Json,
    body::Body,
    extract::{Multipart, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use axum_extra::extract::cookie::SignedCookieJar;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::error;

use crate::{
    AppState,
    auth::{RequireAdmin, RequireChief, RequireGod, RequireStaff},
    check_automation_token, database, get_prefix, resolve_staff_if_god, send_notification_email,
    templates,
};

pub async fn admin_page_handler(
    RequireChief(staff): RequireChief,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    let equipments = if staff.is_admin || staff.is_god {
        database::get_all_equipments(&state.db)
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    templates::admin_page(&prefix, staff.is_admin, staff.is_god, &equipments)
}

#[derive(Debug, Deserialize)]
pub struct UpdateAdminFlagsRequest {
    staff_id: uuid::Uuid,
    is_admin: bool,
    is_god: bool,
}

pub async fn api_update_admin_flags(
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

pub async fn audit_page_handler(
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

    templates::audit_page(&entries, current_page, total_pages.max(1), &prefix)
}

pub async fn validation_page_handler(
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

    templates::validation_page(&pending, &prefix)
}

pub async fn debug_first_order(
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

pub async fn api_get_stats(
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

// Database backup endpoint - pure Rust using COPY protocol
pub async fn backup_database(
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

pub async fn restore_page(
    RequireGod(_staff): RequireGod,
    headers: HeaderMap,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    templates::restore_page(&prefix)
}

pub async fn restore_database(
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
                    return templates::restore_result(
                        &prefix,
                        false,
                        &format!("Erreur de lecture du fichier: {}", e),
                    );
                }
            }
        }
    }

    if sql_content.is_empty() {
        return templates::restore_result(&prefix, false, "Aucun fichier reçu");
    }

    tracing::info!(
        "Restoring database from uploaded file ({} bytes)",
        sql_content.len()
    );

    let sql_str = match std::str::from_utf8(&sql_content) {
        Ok(s) => s,
        Err(e) => {
            error!("Uploaded file is not valid UTF-8: {}", e);
            return templates::restore_result(
                &prefix,
                false,
                "Le fichier n'est pas un fichier SQL valide (encodage UTF-8 invalide)",
            );
        }
    };

    match database::restore_from_sql(&state.db, sql_str).await {
        Ok(()) => {
            tracing::info!("Database restore completed successfully");
            templates::restore_result(&prefix, true, "Base de données restaurée avec succès!")
        }
        Err(e) => {
            error!("Database restore failed: {}", e);
            templates::restore_result(&prefix, false, &format!("Erreur de restauration: {}", e))
        }
    }
}

// Mailchimp-compatible CSV export of all staff
pub async fn export_mailchimp(
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

// ── Equipment API ────────────────────────────────────────────────────

pub async fn api_cycle_equipment(
    RequireAdmin(_admin): RequireAdmin,
    State(state): State<AppState>,
    axum::extract::Path(equipment_id): axum::extract::Path<uuid::Uuid>,
) -> impl IntoResponse {
    // Fetch current status, then cycle to the next one
    let equipments = database::get_all_equipments(&state.db)
        .await
        .unwrap_or_default();
    let current = equipments.iter().find(|e| e.id == equipment_id);
    let next_status = current.map_or(crate::models::EquipmentStatus::Closed, |e| e.status.next());

    match database::set_equipment_status(&state.db, equipment_id, next_status).await {
        Ok(new_status) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": new_status.to_string()})),
        ),
        Err(e) => {
            error!("Failed to update equipment: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }
}

// --- Qualifications management ---

pub async fn qualifications_page_handler(
    RequireAdmin(_staff): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);

    let qualifications = database::get_all_qualifications(&state.db)
        .await
        .unwrap_or_default();

    let staff_qualifs = database::get_all_staff_qualifications_detailed(&state.db)
        .await
        .unwrap_or_default();

    templates::qualifications_page(&prefix, &qualifications, &staff_qualifs)
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateQualificationRequest {
    name: String,
    duration: Option<i16>,
}

pub async fn api_create_qualification(
    RequireAdmin(admin): RequireAdmin,
    State(state): State<AppState>,
    Json(payload): Json<CreateQualificationRequest>,
) -> impl IntoResponse {
    let name = payload.name.trim();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Nom requis"})),
        );
    }

    match database::create_qualification(&state.db, name, payload.duration).await {
        Ok(qual) => {
            let _ = database::insert_audit(
                &state.db,
                Some(admin.id),
                &format!("{} {}", admin.first_name, admin.last_name),
                "Création qualification",
                &format!("name={} duration={:?}", qual.name, qual.duration),
            )
            .await;
            (
                StatusCode::OK,
                Json(serde_json::json!({"success": true, "id": qual.id})),
            )
        }
        Err(e) => {
            error!("Error creating qualification: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }
}

pub async fn api_delete_qualification(
    RequireAdmin(admin): RequireAdmin,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i32>,
) -> impl IntoResponse {
    match database::delete_qualification(&state.db, id).await {
        Ok(()) => {
            let _ = database::insert_audit(
                &state.db,
                Some(admin.id),
                &format!("{} {}", admin.first_name, admin.last_name),
                "Suppression qualification",
                &format!("id={id}"),
            )
            .await;
            (StatusCode::OK, Json(serde_json::json!({"success": true})))
        }
        Err(e) => {
            error!("Error deleting qualification: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct AddStaffQualifRequest {
    staff_id: uuid::Uuid,
    qualification_id: i32,
    obtained_date: chrono::NaiveDate,
}

pub async fn api_add_staff_qualif(
    RequireAdmin(admin): RequireAdmin,
    State(state): State<AppState>,
    Json(payload): Json<AddStaffQualifRequest>,
) -> impl IntoResponse {
    match database::add_staff_qualif(
        &state.db,
        payload.staff_id,
        payload.qualification_id,
        payload.obtained_date,
    )
    .await
    {
        Ok(sq) => {
            let _ = database::insert_audit(
                &state.db,
                Some(admin.id),
                &format!("{} {}", admin.first_name, admin.last_name),
                "Ajout qualification staff",
                &format!(
                    "staff={} qualification={} date={}",
                    sq.staff, sq.qualification, sq.obtained_date
                ),
            )
            .await;
            (
                StatusCode::OK,
                Json(serde_json::json!({"success": true, "id": sq.id})),
            )
        }
        Err(e) => {
            error!("Error adding staff qualification: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }
}

pub async fn api_delete_staff_qualif(
    RequireAdmin(admin): RequireAdmin,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i32>,
) -> impl IntoResponse {
    match database::delete_staff_qualif(&state.db, id).await {
        Ok(()) => {
            let _ = database::insert_audit(
                &state.db,
                Some(admin.id),
                &format!("{} {}", admin.first_name, admin.last_name),
                "Suppression qualification staff",
                &format!("id={id}"),
            )
            .await;
            (StatusCode::OK, Json(serde_json::json!({"success": true})))
        }
        Err(e) => {
            error!("Error deleting staff qualification: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }
}
