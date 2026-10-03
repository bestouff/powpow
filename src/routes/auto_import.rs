use maud::html;
use tracing::info;

use crate::{AppState, database, send_notification_email};

/// Run after a complete `HelloAsso` order/batch or a cash membership is stored.
/// Only newly ambiguous sources produce an email; the existing daily digest
/// keeps reminding admins about unresolved records.
pub async fn import_pending(state: &AppState) -> anyhow::Result<()> {
    let summary = database::auto_import::auto_import_pending(&state.db).await?;
    if summary.imported > 0 {
        info!("Auto-import: {} memberships imported", summary.imported);
        super::mailchimp::spawn_newsletter_sync(state);
    }
    if !summary.reviews.is_empty() {
        info!(
            "Auto-import: {} memberships require review",
            summary.reviews.len()
        );
        let state = state.clone();
        tokio::spawn(async move {
            let emails = database::get_admin_emails_for_import(&state.db)
                .await
                .unwrap_or_default();
            let subject = format!(
                "{} — {} adhésion(s) à vérifier",
                state.config.entity_name,
                summary.reviews.len()
            );
            let body = html! {
                p { "Bonjour," }
                p { "L'import automatique a laissé ces adhésions en attente. Connectez-vous à PowPow pour les résoudre dans /online ou /cash :" }
                ul {
                    @for (source, reason) in &summary.reviews {
                        li { code { (source.import_path()) } " — " (reason) }
                    }
                }
            }.into_string() + &crate::email_signature(&state.config.entity_name);
            send_notification_email(&state, &emails, &subject, &body).await;
        });
    }
    Ok(())
}
