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
use tracing::{debug, error, info};

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
    if !state.mailchimp_client.is_configured() {
        anyhow::bail!(
            "Mailchimp n'est pas configuré (variables d'environnement MAILCHIMP_* absentes)"
        );
    }
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
/// Debounce delay: after the last membership change, wait this long before
/// actually running a Mailchimp sync.
const SYNC_DEBOUNCE_DELAY: std::time::Duration = std::time::Duration::from_secs(600);

/// Arm (or re-arm) the debounced Mailchimp sync. Each membership change pushes
/// the sync back by `SYNC_DEBOUNCE_DELAY`; an already waiting task is cancelled
/// so a bulk import doesn't trigger a sync storm.
pub fn spawn_newsletter_sync(state: &AppState) {
    if !state.mailchimp_client.is_configured() {
        debug!("Mailchimp n'est pas configuré, sync newsletter ignorée");
        return;
    }
    let state = state.clone();
    tokio::spawn(async move {
        let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(());
        let state_inner = state.clone();
        let handle = tokio::spawn(async move {
            // Wait out the debounce delay, unless a newer sync cancels it.
            tokio::select! {
                () = tokio::time::sleep(SYNC_DEBOUNCE_DELAY) => {
                    // Another sync may have started in the meantime (manual, or
                    // a previous debounced sync that finished later).
                    sync_newsletter_guarded(&state_inner).await;
                }
                _ = cancel_rx.changed() => {}
            }
        });
        let mut pending = state.mailchimp_sync_pending.lock().await;
        if let Some((prev_handle, prev_cancel)) = pending.replace((handle, cancel_tx)) {
            // Stop the previous debounce delay. If it had already started
            // syncing, this is a no-op and the guard flag prevents our own
            // sync from running in parallel.
            let _ = prev_cancel.send(());
            drop(prev_handle);
        }
    });
}

/// Cancel a pending (debounced) sync, if any. Used by the manual sync button
/// and by the daily background sync so they don't piggyback on a stale timer.
pub async fn cancel_pending_sync(state: &AppState) {
    let mut pending = state.mailchimp_sync_pending.lock().await;
    if let Some((handle, cancel)) = pending.take() {
        let _ = cancel.send(());
        drop(handle);
    }
}

/// Run a Mailchimp sync protected by the "in progress" flag, logging the
/// outcome. Non-overlapping: if another sync is already running, skip.
async fn sync_newsletter_guarded(state: &AppState) {
    let flag = state.mailchimp_sync_in_progress.clone();
    if flag.swap(true, Ordering::SeqCst) {
        info!("Mailchimp newsletter sync already in progress, skipping");
        return;
    }
    let result = sync_newsletter(state).await;
    flag.store(false, Ordering::SeqCst);
    match result {
        Ok((pulled, stored, recpt, ok, err)) => info!(
            "Mailchimp newsletter sync done: {} pulled, {} stored, {} recipients ({} ok, {} errors)",
            pulled, stored, recpt, ok, err
        ),
        Err(e) => error!("Mailchimp newsletter sync failed: {}", e),
    }
}

/// Page listing each staff member's newsletter status.
pub async fn mailchimp_page(
    RequireAdmin(_admin): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let prefix = get_prefix(&headers);
    let configured = state.mailchimp_client.is_configured();
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
    templates::mailchimp_page(&staff, &prefix, configured).into_response()
}

/// Run a full Mailchimp newsletter sync from the confirm button.
pub async fn mailchimp_sync_now(
    RequireAdmin(admin): RequireAdmin,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Response {
    let prefix = get_prefix(&headers);
    if !state.mailchimp_client.is_configured() {
        return templates::mailchimp_sync_result(
            &prefix,
            false,
            "Mailchimp n'est pas configuré : synchronisation impossible. Vérifiez les \
             variables d'environnement MAILCHIMP_API_KEY, MAILCHIMP_SERVER_PREFIX, \
             MAILCHIMP_LIST_ID et MAILCHIMP_FROM_EMAIL.",
        )
        .into_response();
    }
    // A manual sync supersedes any pending debounced sync.
    cancel_pending_sync(&state).await;
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
