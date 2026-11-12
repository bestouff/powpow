use axum::{
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::Local;
use maud::html;

use crate::{AppState, auth::RequireStaff, database, get_prefix, templates};

pub async fn page(
    RequireStaff(_staff): RequireStaff,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Response {
    let today = Local::now().date_naive();
    let tomorrow = today.succ_opt().expect("current date has a next day");
    let dates = [today];
    match tokio::try_join!(
        database::planning::for_day(&state.db, today),
        database::get_opening_days_for_dates(&state.db, &dates),
        database::get_all_equipments(&state.db),
        database::planning::has_activity(&state.db, tomorrow),
    ) {
        Ok((planning, opening_days, equipments, tomorrow_has_activity)) => (
            [(header::CACHE_CONTROL, "private, no-store")],
            templates::today_page(
                &get_prefix(&headers),
                today,
                &planning,
                opening_days.first().map(|day| day.status),
                &equipments,
                state.forecast.for_date(today).as_ref(),
                tomorrow_has_activity.then_some(tomorrow),
            ),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "Cannot load today's information");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                html! { p { "Impossible de charger les informations du jour." } },
            )
                .into_response()
        }
    }
}
