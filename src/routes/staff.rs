use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::SignedCookieJar;
use maud::html;
use serde::Deserialize;
use tracing::{error, warn};

use crate::{
    AppState,
    auth::{RequireAdmin, RequireStaff},
    database, get_current_season, get_prefix, send_notification_email, templates,
};

pub async fn list_staff(
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
            return html! { p { "Error loading staff: " (e) } };
        }
    };

    let ateliers = match database::get_all_ateliers(&state.db).await {
        Ok(list) => list,
        Err(e) => {
            error!("Error fetching ateliers: {}", e);
            return html! { p { "Error loading ateliers: " (e) } };
        }
    };

    let roles = match database::get_all_roles(&state.db).await {
        Ok(list) => list,
        Err(e) => {
            error!("Error fetching roles: {}", e);
            return html! { p { "Error loading roles: " (e) } };
        }
    };

    let qualifications = match database::get_all_qualifications(&state.db).await {
        Ok(list) => list,
        Err(e) => {
            error!("Error fetching qualifications: {}", e);
            return html! { p { "Error loading qualifications: " (e) } };
        }
    };

    let staff_qualifs = match database::get_all_staff_qualifications(&state.db).await {
        Ok(list) => list,
        Err(e) => {
            error!("Error fetching staff qualifications: {}", e);
            return html! { p { "Error loading staff qualifications: " (e) } };
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

    templates::staff_list(
        staff_list,
        &ateliers,
        &roles,
        &qualifications,
        &staff_qualifs,
        current_season,
        &prefix,
        show_contact,
    )
}

#[derive(Debug, Deserialize)]
pub struct PersonQuery {
    token: Option<uuid::Uuid>,
}

pub async fn view_person(
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
                cookie.set_secure(true);
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
            return (StatusCode::NOT_FOUND, html! { p { "Staff not found" } }).into_response();
        }
        Err(e) => {
            error!("Error fetching staff: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                html! { p { "Error: " (e) } },
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
                html! { p { "Error: " (e) } },
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
                html! { p { "Error: " (e) } },
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
                        r#"<a href="{}/online"><strong>{}</strong> adhésion(s) HelloAsso à importer</a>"#,
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

    let person_qualifications =
        match database::get_staff_qualifications_for_person(&state.db, id).await {
            Ok(q) => q,
            Err(e) => {
                error!("Error fetching person qualifications: {}", e);
                Vec::new()
            }
        };

    // Fetch person calendar (upcoming needs + presence across all ateliers)
    // Visible to self, admins, and chiefs
    let person_calendar = if is_self || is_viewer_admin || is_viewer_chief {
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

    templates::person_detail(
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
        &person_qualifications,
    )
    .into_response()
}

#[derive(Debug, Deserialize)]
pub(crate) struct ToggleRoleRequest {
    atelier_id: uuid::Uuid,
    #[serde(default)]
    add: Option<bool>,
    #[serde(default)]
    validated: Option<bool>,
    #[serde(default)]
    chief: Option<bool>,
}

pub async fn toggle_role(
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
                                    "{} — {} demande à rejoindre {}",
                                    state_clone.config.entity_name, staff_name, atelier_name
                                );
                                let html_body = format!(
                                    r"<p>Bonjour,</p>
<p><strong>{staff}</strong> souhaite rejoindre l'atelier <strong>{atelier}</strong> et attend votre validation.</p>
<p>Connectez-vous à PowPow pour valider ou refuser cette demande.</p>
{sig}",
                                    staff = staff_name,
                                    atelier = atelier_name,
                                    sig = crate::email_signature(&state_clone.config.entity_name),
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
                        let subject = format!(
                            "{} — Votre rôle dans {} a été validé",
                            state_clone.config.entity_name, atelier_name
                        );
                        let html_body = format!(
                            r"<p>Bonjour {},</p>
<p>Votre demande pour rejoindre l'atelier <strong>{}</strong> a été validée. Vous pouvez dès maintenant vous inscrire aux créneaux sur le calendrier.</p>
{}",
                            staff.first_name,
                            atelier_name,
                            crate::email_signature(&state_clone.config.entity_name),
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
                                format!(
                                    "{} — Vous êtes maintenant chef de {}",
                                    state_clone.config.entity_name, atelier_name
                                ),
                                format!(
                                    r"<p>Bonjour {},</p>
<p>Vous avez été nommé(e) <strong>chef</strong> de l'atelier <strong>{}</strong>.</p>
<p>Vous recevrez désormais les notifications liées à cet atelier.</p>
{}",
                                    staff.first_name,
                                    atelier_name,
                                    crate::email_signature(&state_clone.config.entity_name),
                                ),
                            )
                        } else {
                            (
                                format!(
                                    "{} — Vous n'êtes plus chef de {}",
                                    state_clone.config.entity_name, atelier_name
                                ),
                                format!(
                                    r"<p>Bonjour {},</p>
<p>Vous n'êtes plus chef de l'atelier <strong>{}</strong>.</p>
{}",
                                    staff.first_name,
                                    atelier_name,
                                    crate::email_signature(&state_clone.config.entity_name),
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
pub(crate) struct UpdateCommentPayload {
    comment: String,
}

pub async fn update_comment(
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
pub(crate) struct UpdateContactPayload {
    email: String,
    phone: Option<String>,
}

pub async fn update_contact(
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

#[derive(Debug, Deserialize)]
pub(crate) struct CreateStaffMinimalRequest {
    first_name: String,
    last_name: String,
    email: Option<String>,
    phone: Option<String>,
}

pub async fn api_create_staff_minimal(
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
