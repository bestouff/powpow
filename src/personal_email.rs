//! Person-specific delivery for account changes; never group household recipients.

use anyhow::{Result, anyhow, ensure};
use lettre::{
    Transport,
    message::{Mailbox, header::ContentType},
};

use crate::AppState;

pub async fn send(
    state: &AppState,
    address: &str,
    name: &str,
    subject: &str,
    html: String,
) -> Result<()> {
    let gmail = state.config.mail_method.eq_ignore_ascii_case("gmail");
    let from = if gmail {
        &state.config.gmail_from
    } else {
        &state.config.smtp_from
    };
    let destination = if state.config.mail_destination_override.is_empty() {
        address
    } else {
        &state.config.mail_destination_override
    };
    let email = lettre::Message::builder()
        .from(from.parse::<Mailbox>()?)
        .to(Mailbox::new(Some(name.to_string()), destination.parse()?))
        .subject(subject)
        .header(ContentType::TEXT_HTML)
        .body(html)?;
    if gmail {
        let client = state
            .gmail_client
            .as_ref()
            .ok_or_else(|| anyhow!("Gmail is not configured"))?;
        client
            .messages_send(
                "me",
                httpclient::InMemoryBody::Text(String::from_utf8(email.formatted())?),
                None,
            )
            .await
            .map_err(|e| anyhow!("Gmail delivery failed: {e}"))?;
    } else {
        ensure!(!state.config.smtp_host.is_empty(), "SMTP is not configured");
        let credentials = lettre::transport::smtp::authentication::Credentials::new(
            state.config.smtp_user.clone(),
            state.config.smtp_password.clone(),
        );
        let mailer = lettre::SmtpTransport::relay(&state.config.smtp_host)?
            .port(state.config.smtp_port)
            .credentials(credentials)
            .build();
        tokio::task::spawn_blocking(move || mailer.send(&email)).await??;
    }
    Ok(())
}
