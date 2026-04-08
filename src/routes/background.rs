use chrono::{TimeDelta, Timelike};
use futures_util::FutureExt as _;
use tracing::{error, info};

use crate::{
    AppState, database, dicton, get_current_season, news, send_notification_email, templates,
};

/// Compute the next occurrence of 5:00 AM local time strictly after `from_when`.
///
/// Used only in tests; retained as a utility for potential future scheduling needs.
#[cfg(test)]
fn next_5am_local(
    from_when: chrono::DateTime<chrono::Local>,
) -> Option<chrono::DateTime<chrono::Local>> {
    let today_5am = from_when.date_naive().and_hms_opt(5, 0, 0)?;
    let today_5am_local =
        chrono::TimeZone::from_local_datetime(&from_when.timezone(), &today_5am).single()?;

    if from_when < today_5am_local {
        Some(today_5am_local)
    } else {
        // Already past 5 AM today, schedule for tomorrow
        let tomorrow_5am = today_5am + TimeDelta::days(1);
        chrono::TimeZone::from_local_datetime(&from_when.timezone(), &tomorrow_5am).single()
    }
}

/// Interval between news-feed sync runs (15 minutes).
const NEWS_SYNC_INTERVAL: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// Background loop that preloads the dicton du jour and news feed.
///
/// Runs both dicton + news once at startup, then:
/// - re-syncs news every 15 minutes,
/// - regenerates the dicton once a day at 5:00 AM local time.
pub async fn daily_preload_loop(state: AppState) {
    let feed_url = state.config.rss_news_feed.clone();
    let has_feed = !feed_url.is_empty();

    // ── Startup: one-shot preload of both dicton and news ───────
    let season = get_current_season();
    let hf_token = state.config.huggingface_token.clone();

    info!("preload: generating dicton du jour");
    let _ = dicton::get_or_generate(&state.db, season, &hf_token).await;

    if has_feed {
        info!("preload: syncing news feed");
        news::sync_news(&state.db, &feed_url).await;
    }

    info!("preload: startup done");

    // ── Steady-state loop ───────────────────────────────────────
    // Sleep in 15-minute increments for news sync.
    // Regenerate dicton when we cross the 5 AM boundary.
    // Each iteration is wrapped in catch_unwind so a panic in one
    // cycle does not kill the whole background task.
    let mut last_dicton_date = chrono::Local::now().date_naive();

    loop {
        tokio::time::sleep(NEWS_SYNC_INTERVAL).await;

        let result = std::panic::AssertUnwindSafe(preload_tick(
            &state,
            has_feed,
            &feed_url,
            last_dicton_date,
        ))
        .catch_unwind()
        .await;

        match result {
            Ok(new_date) => last_dicton_date = new_date,
            Err(panic) => {
                let msg = panic
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| panic.downcast_ref::<&str>().copied())
                    .unwrap_or("unknown panic");
                error!("preload: background tick panicked: {msg}");
            }
        }
    }
}

/// One tick of the background preload loop.
///
/// Returns the (possibly updated) `last_dicton_date`.
async fn preload_tick(
    state: &AppState,
    has_feed: bool,
    feed_url: &str,
    last_dicton_date: chrono::NaiveDate,
) -> chrono::NaiveDate {
    // News sync every 15 minutes
    if has_feed {
        news::sync_news(&state.db, feed_url).await;
    }

    // Daily tasks: regenerate dicton and full HelloAsso re-sync at/after 5 AM
    let now = chrono::Local::now();
    let today = now.date_naive();
    if today > last_dicton_date && now.hour() >= 5 {
        let season = get_current_season();
        let hf_token = state.config.huggingface_token.clone();
        info!("preload: daily dicton regeneration");
        let _ = dicton::get_or_generate(&state.db, season, &hf_token).await;

        // Daily full HelloAsso re-sync as safety net
        info!("preload: daily HelloAsso full re-sync");
        match super::sync::sync_users_from_helloasso(state).await {
            Ok((u, m)) => {
                info!(
                    "preload: daily HelloAsso sync complete — {} users, {} memberships",
                    u, m
                );
                let _ = database::insert_audit(
                    &state.db,
                    None,
                    "Système",
                    "Synchronisation HelloAsso quotidienne",
                    &format!("{} utilisateurs, {} adhésions", u, m),
                )
                .await;
            }
            Err(e) => {
                error!("preload: daily HelloAsso sync failed: {}", e);
                let _ = database::insert_audit(
                    &state.db,
                    None,
                    "Système",
                    "Synchronisation HelloAsso quotidienne (échec)",
                    &e.to_string(),
                )
                .await;
            }
        }

        return today;
    }

    last_dicton_date
}

pub fn next_monday_8am_local(
    from_when: chrono::DateTime<chrono::Local>,
) -> Option<chrono::DateTime<chrono::Local>> {
    use chrono::Datelike;

    let today = from_when.date_naive();
    let days_ahead = 8 - i64::from(today.weekday().number_from_monday());
    let monday_8am =
        from_when.date_naive().and_hms_opt(8, 0, 0).unwrap() + TimeDelta::days(days_ahead);
    let Some(monday_8am_local) =
        chrono::TimeZone::from_local_datetime(&from_when.timezone(), &monday_8am).single()
    else {
        // TZ failure, bail out
        return None;
    };

    let target = if from_when >= monday_8am_local {
        // Already past 8 AM today, schedule for next Monday
        monday_8am_local + TimeDelta::days(7)
    } else if from_when < monday_8am_local - TimeDelta::days(7) {
        // Next Monday 8 AM is too far away
        monday_8am_local - TimeDelta::days(7)
    } else {
        // Schedule for this coming Monday 8 AM
        monday_8am_local
    };
    Some(target)
}

/// Background loop that sends a daily summary email to admins at 8:00 AM local time.
pub async fn weekly_morning_email_loop(state: AppState) {
    loop {
        // Calculate duration until next Monday 8:00 AM local time
        let now = chrono::Local::now();

        let Some(target) = next_monday_8am_local(now) else {
            // Fallback: sleep 1 hour and retry
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
            continue;
        };

        let sleep_duration = (target - now)
            .to_std()
            .unwrap_or(tokio::time::Duration::from_secs(3600));
        info!(
            "Daily email: next run in {} seconds",
            sleep_duration.as_secs()
        );
        tokio::time::sleep(sleep_duration).await;

        // Gather data
        let current_season = get_current_season();
        let unimported = database::list_unimported_names(&state.db, current_season)
            .await
            .unwrap_or_default();

        let today = chrono::Local::now().date_naive();
        let week_end = today + chrono::Duration::days(7);
        let upcoming = database::get_upcoming_needs_deficit(&state.db, today, week_end)
            .await
            .unwrap_or_default();

        // Only send if there is content
        if unimported.is_empty() && upcoming.is_empty() {
            info!("Weekly email: nothing to report, skipping");
            // Sleep 60s to avoid double-send on the same minute
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            continue;
        }

        let admin_emails = database::get_admin_emails(&state.db)
            .await
            .unwrap_or_default();
        if admin_emails.is_empty() {
            info!("Weekly email: no admin emails configured, skipping");
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            continue;
        }

        // Build email body
        let mut body =
            String::from("<p>Bonjour,</p>\n<p>Voici le récapitulatif du lundi matin :</p>\n");

        if !unimported.is_empty() {
            body.push_str("<h3>Adhésions à importer</h3>\n<ul>\n");
            for (first_name, last_name, source) in &unimported {
                body.push_str(&format!(
                    "<li>{} {} <em>({})</em></li>\n",
                    first_name, last_name, source,
                ));
            }
            body.push_str("</ul>\n");
        }

        if !upcoming.is_empty() {
            body.push_str("<h3>Semaine à venir</h3>\n");
            body.push_str(&templates::render_upcoming_week_email(&upcoming));
        }

        body.push_str(&crate::email_signature(&state.config.entity_name));

        let subject = format!(
            "{} — Récapitulatif du lundi matin",
            state.config.entity_name
        );
        send_notification_email(&state, &admin_emails, &subject, &body).await;
        info!("Weekly email: sent to {} admins", admin_emails.len());

        // Sleep 60s to avoid double-send on the same minute
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn test_next_monday_8am_from_1am() {
        let monday_1am = chrono::Local
            .with_ymd_and_hms(2026, 2, 23, 1, 0, 0)
            .unwrap();
        let result = next_monday_8am_local(monday_1am).unwrap();
        assert_eq!(
            result,
            chrono::Local
                .with_ymd_and_hms(2026, 2, 23, 8, 0, 0)
                .unwrap()
        );
    }

    #[test]
    fn test_next_monday_8am_from_9am() {
        let monday_9am = chrono::Local
            .with_ymd_and_hms(2026, 2, 23, 9, 0, 0)
            .unwrap();
        let result = next_monday_8am_local(monday_9am).unwrap();
        assert_eq!(
            result,
            chrono::Local.with_ymd_and_hms(2026, 3, 2, 8, 0, 0).unwrap()
        );
    }

    #[test]
    fn test_next_monday_8am_from_tuesday() {
        let tuesday_5am = chrono::Local
            .with_ymd_and_hms(2026, 2, 24, 5, 0, 0)
            .unwrap();
        let result = next_monday_8am_local(tuesday_5am).unwrap();
        assert_eq!(
            result,
            chrono::Local.with_ymd_and_hms(2026, 3, 2, 8, 0, 0).unwrap()
        );
    }

    #[test]
    fn test_next_5am_before_5am() {
        // 3 AM → should get 5 AM the same day
        let at_3am = chrono::Local
            .with_ymd_and_hms(2026, 2, 24, 3, 0, 0)
            .unwrap();
        let result = next_5am_local(at_3am).unwrap();
        assert_eq!(
            result,
            chrono::Local
                .with_ymd_and_hms(2026, 2, 24, 5, 0, 0)
                .unwrap()
        );
    }

    #[test]
    fn test_next_5am_after_5am() {
        // 10 AM → should get 5 AM the next day
        let at_10am = chrono::Local
            .with_ymd_and_hms(2026, 2, 24, 10, 0, 0)
            .unwrap();
        let result = next_5am_local(at_10am).unwrap();
        assert_eq!(
            result,
            chrono::Local
                .with_ymd_and_hms(2026, 2, 25, 5, 0, 0)
                .unwrap()
        );
    }

    #[test]
    fn test_next_5am_at_exactly_5am() {
        // Exactly 5 AM → should get 5 AM the next day
        let at_5am = chrono::Local
            .with_ymd_and_hms(2026, 2, 24, 5, 0, 0)
            .unwrap();
        let result = next_5am_local(at_5am).unwrap();
        assert_eq!(
            result,
            chrono::Local
                .with_ymd_and_hms(2026, 2, 25, 5, 0, 0)
                .unwrap()
        );
    }
}
