use axum::{
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{Local, NaiveDate};

use crate::{AppState, auth::RequireStaff, database, planning_pdf};

pub async fn download(
    RequireStaff(_staff): RequireStaff,
    State(state): State<AppState>,
    Path(day): Path<NaiveDate>,
) -> Response {
    let mut response = match export(&state, day).await {
        Ok(Some(pdf)) => (
            [
                (header::CONTENT_TYPE, "application/pdf".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"planning-{day}.pdf\""),
                ),
            ],
            pdf,
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            "Aucune activité prévue pour cette date.",
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, %day, "Cannot export planning PDF");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Impossible d’exporter le planning. Veuillez réessayer.",
            )
                .into_response()
        }
    };
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}

async fn export(state: &AppState, day: NaiveDate) -> anyhow::Result<Option<Vec<u8>>> {
    let planning = database::planning::for_day(&state.db, day).await?;
    if planning.is_empty() {
        return Ok(None);
    }
    let dates = [day];
    let (opening_days, logo) = tokio::try_join!(
        database::get_opening_days_for_dates(&state.db, &dates),
        logo(&state.db),
    )?;
    let status = opening_days.first().map(|day| day.status);
    let entity = state.config.entity_name.clone();
    let exported_at = Local::now();
    // Font parsing, image decoding, and PDF layout must not block async requests.
    tokio::task::spawn_blocking(move || {
        planning_pdf::render(
            day,
            &planning,
            status,
            &entity,
            logo.as_deref(),
            exported_at,
        )
    })
    .await?
    .map(Some)
}

async fn logo(pool: &sqlx::PgPool) -> anyhow::Result<Option<Vec<u8>>> {
    for slug in ["hero-subtitle", "navbar"] {
        if let Some(block) = database::get_content(pool, slug).await?
            && let Some(image_id) = block.image_id
            && let Some(image) = database::get_content_image(pool, image_id).await?
        {
            return Ok(Some(image.data));
        }
    }
    Ok(None)
}
