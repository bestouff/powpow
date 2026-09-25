use std::sync::atomic::Ordering;

use anyhow::Result;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use maud::html;
use serde_json::json;
use tracing::{error, info};

use crate::{AppState, auth::RequireAdmin, database, get_prefix, templates};

/// Synchronize the newsletter statuses and recipient list with Mailchimp.
///
/// 1. Pull every member of the audience (email + status) from Mailchimp and
///    store the status in `newsletter_status`.
/// 2. Push every staff member who is `subscribed` to the Mailchimp audience.
///
/// Returns a summary of the operation.
pub async fn sync_newsletter(state: &AppState) -> Result<(usize, usize, usize, usize, usize)> {
    let result = sync_newsletter_inner(state).await;
    if let Err(e) = &result {
        let mut progress = state.mailchimp_sync_progress.lock().await;
        progress.phase = "error".into();
        progress.error = Some(e.to_string());
    }
    result
}

async fn sync_newsletter_inner(state: &AppState) -> Result<(usize, usize, usize, usize, usize)> {
    let mut progress = state.mailchimp_sync_progress.lock().await;
    *progress = crate::mailchimp::MailchimpSyncProgress {
        phase: "pulling".into(),
        ..Default::default()
    };
    drop(progress);
    let members = state
        .mailchimp_client
        .list_member_statuses(Some(&state.mailchimp_sync_progress))
        .await?;
    {
        let mut progress = state.mailchimp_sync_progress.lock().await;
        progress.phase = "storing".into();
        progress.pulled_total = members.len() as u32;
        progress.pulled = members.len() as u32;
    }
    let stored =
        database::set_mailchimp_statuses(&state.db, &members, Some(&state.mailchimp_sync_progress))
            .await?;
    {
        let mut progress = state.mailchimp_sync_progress.lock().await;
        progress.phase = "pushing".into();
        progress.stored = stored as u32;
    }
    let recipients = database::get_newsletter_recipients(&state.db).await?;
    let (pushed_ok, push_errors) = state
        .mailchimp_client
        .sync_staff(&recipients, Some(&state.mailchimp_sync_progress))
        .await?;
    {
        let mut progress = state.mailchimp_sync_progress.lock().await;
        progress.phase = "done".into();
        progress.stored = stored as u32;
        progress.push_ok = pushed_ok as u32;
        progress.push_errors = push_errors as u32;
        progress.pushed_total = recipients.len() as u32;
        progress.pushed = (pushed_ok + push_errors) as u32;
    }
    info!(
        "Mailchimp newsletter sync: {} members pulled, {} statuses stored, {} recipients ({} ok, {} errors)",
        members.len(),
        stored,
        recipients.len(),
        pushed_ok,
        push_errors
    );
    Ok((
        members.len(),
        stored,
        recipients.len(),
        pushed_ok,
        push_errors,
    ))
}
/// Spawn a background Mailchimp newsletter sync.
///
/// Used after a membership change (import/delete) so the HTTP response is not
/// delayed by the Mailchimp round-trips. Guards against overlapping runs.
pub fn spawn_newsletter_sync(state: &AppState) {
    let flag = state.mailchimp_sync_in_progress.clone();
    if flag.swap(true, Ordering::SeqCst) {
        info!("Mailchimp newsletter sync already in progress, skipping");
        return;
    }
    let state = state.clone();
    tokio::spawn(async move {
        let result = sync_newsletter(&state).await;
        flag.store(false, Ordering::SeqCst);
        match result {
            Ok((pulled, stored, recpt, ok, err)) => info!(
                "Background Mailchimp newsletter sync done: {} pulled, {} stored, {} recipients ({} ok, {} errors)",
                pulled, stored, recpt, ok, err
            ),
            Err(e) => error!("Background Mailchimp newsletter sync failed: {}", e),
        }
    });
}

/// Page listing each staff member's newsletter status.
pub async fn mailchimp_page(
    RequireAdmin(_admin): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    let staff = match database::get_all_staff_ordered(&state.db).await {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to load staff for Mailchimp page: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                html! { p { "Erreur lors du chargement du staff" } },
            )
                .into_response();
        }
    };
    templates::mailchimp_page(&staff, &prefix).into_response()
}

/// Run a full Mailchimp newsletter sync from the confirm button.
pub async fn mailchimp_sync_now(
    RequireAdmin(admin): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Response {
    let prefix = get_prefix(&headers);
    let flag = state.mailchimp_sync_in_progress.clone();
    if flag.swap(true, Ordering::SeqCst) {
        return templates::mailchimp_sync_result(
            &prefix,
            false,
            "Une synchronisation Mailchimp est déjà en cours.",
        )
        .into_response();
    }
    let result = sync_newsletter(&state).await;
    flag.store(false, Ordering::SeqCst);

    match result {
        Ok((_pulled, stored, recpt, ok, err)) => {
            let _ = database::insert_audit(
                &state.db,
                Some(admin.id),
                &format!("{} {}", admin.first_name, admin.last_name),
                "Synchronisation newsletter (Mailchimp)",
                &format!(
                    "{} statuts enregistrés, {} destinataires ({} ok, {} erreurs)",
                    stored, recpt, ok, err
                ),
            )
            .await;
            templates::mailchimp_sync_result(
                &prefix,
                true,
                &format!(
                    "Synchronisation terminée : {} statut(s) enregistré(s), {} destinataire(s) envoyé(s) avec {} erreur(s).",
                    stored, ok, err
                ),
            )
            .into_response()
        }
        Err(e) => {
            let _ = database::insert_audit(
                &state.db,
                Some(admin.id),
                &format!("{} {}", admin.first_name, admin.last_name),
                "Synchronisation newsletter (Mailchimp) — échec",
                &e.to_string(),
            )
            .await;
            templates::mailchimp_sync_result(
                &prefix,
                false,
                &format!("Erreur de synchronisation Mailchimp : {e}"),
            )
            .into_response()
        }
    }
}

/// JSON progress of the currently running Mailchimp sync, for the live
/// counter on the newsletter page.
pub async fn mailchimp_progress(
    RequireAdmin(_admin): RequireAdmin,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let progress = state.mailchimp_sync_progress.lock().await;
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
    (
        headers,
        Json(json!({
            "phase": progress.phase,
            "pulled": progress.pulled,
            "pulled_total": progress.pulled_total,
            "stored": progress.stored,
            "pushed": progress.pushed,
            "pushed_total": progress.pushed_total,
            "push_ok": progress.push_ok,
            "push_errors": progress.push_errors,
            "error": progress.error,
        })),
    )
}
