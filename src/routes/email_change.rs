use axum::{
    extract::{Form, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::error;

use crate::{
    AppState, database,
    email_change::{self, EmailChange},
    get_prefix,
    models::Staff,
    templates,
};

/// Account confirmation is person-specific even when several people share an address.
pub async fn request(
    state: &AppState,
    headers: &HeaderMap,
    staff: &Staff,
    email: &str,
) -> anyhow::Result<()> {
    let change = EmailChange {
        staff_id: staff.id,
        token: uuid::Uuid::new_v4(),
        old_email: staff.email.clone(),
        new_email: email.to_string(),
        expires_at: (chrono::Utc::now() + email_change::LIFETIME).timestamp(),
    };
    let token = email_change::seal(&state.cookie_key, &change)?;
    let proto = headers
        .get("X-Forwarded-Proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");
    let host = headers
        .get("X-Forwarded-Host")
        .or_else(|| headers.get("Host"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost:3000");
    let url = format!(
        "{proto}://{host}{}/email-change?token={token}",
        get_prefix(headers)
    );
    let name = format!("{} {}", staff.first_name, staff.last_name);
    let body = maud::html! {
        p { "Bonjour " (name) "," }
        p { "Vous avez demandé à remplacer votre adresse email par " strong { (email) } "." }
        p { a href=(url) { "Confirmer mon adresse email" } }
        p { "Sur la page suivante, cliquez sur « Confirmer mon adresse email »." }
        p { "Ce lien est valable 24 heures et ne peut être utilisé qu'une fois. Votre adresse actuelle reste active jusqu'à la confirmation." }
        p { "Si vous n'avez pas demandé cette modification, ignorez cet email." }
        (maud::PreEscaped(crate::email_signature(&state.config.entity_name)))
    }.into_string();
    database::email_change::create(&state.db, &change).await?;
    if let Err(e) = crate::personal_email::send(
        state,
        email,
        &name,
        &format!(
            "{} — Confirmer votre adresse email",
            state.config.entity_name
        ),
        body,
    )
    .await
    {
        database::email_change::cancel(&state.db, change.token).await?;
        return Err(e);
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct Confirmation {
    token: String,
}

fn private(response: impl IntoResponse) -> Response {
    let mut response = response.into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        axum::http::HeaderValue::from_static("strict-origin"),
    );
    response
}

fn invalid(prefix: &str) -> Response {
    private((
        StatusCode::BAD_REQUEST,
        templates::email_change_page(
            prefix,
            "Lien de confirmation invalide",
            "Ce lien a expiré, a déjà été utilisé ou a été remplacé. Votre adresse n'a pas été modifiée. Demandez une nouvelle confirmation depuis votre fiche.",
            None,
        ),
    ))
}

// GET/HEAD only preview the change: mail scanners must not validate an address.
pub async fn preview(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(form): Query<Confirmation>,
) -> Response {
    let prefix = get_prefix(&headers);
    let Ok(change) = email_change::open(
        &state.cookie_key,
        &form.token,
        chrono::Utc::now().timestamp(),
    ) else {
        return invalid(&prefix);
    };
    match database::email_change::pending_staff(&state.db, &change).await {
        Ok(Some(staff)) => private(templates::email_change_page(
            &prefix,
            "Confirmer votre adresse email",
            &format!(
                "{} {}, confirmez votre nouvelle adresse : {}.",
                staff.first_name, staff.last_name, change.new_email
            ),
            Some(&form.token),
        )),
        Ok(None) => invalid(&prefix),
        Err(e) => {
            error!("Cannot preview email change: {e}");
            private(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn confirm(
    headers: HeaderMap,
    State(state): State<AppState>,
    Form(form): Form<Confirmation>,
) -> Response {
    let prefix = get_prefix(&headers);
    if !super::auth::confirmation_origin_allowed(&headers) {
        return private(StatusCode::FORBIDDEN);
    }
    let Ok(change) = email_change::open(
        &state.cookie_key,
        &form.token,
        chrono::Utc::now().timestamp(),
    ) else {
        return invalid(&prefix);
    };
    match database::email_change::apply(&state.db, &change).await {
        Ok(Some(staff)) => {
            let _ = database::insert_audit(
                &state.db,
                Some(staff.id),
                &format!("{} {}", staff.first_name, staff.last_name),
                "Validation email",
                &format!("staff={} email={}", staff.id, staff.email),
            )
            .await;
            super::mailchimp::spawn_newsletter_sync(&state);
            private(templates::email_change_page(
                &prefix,
                "Adresse email confirmée",
                "Votre nouvelle adresse email est maintenant active. Vous pouvez l'utiliser pour recevoir vos liens de connexion.",
                None,
            ))
        }
        Ok(None) => invalid(&prefix),
        Err(e) => {
            error!("Cannot confirm email change: {e}");
            private(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
