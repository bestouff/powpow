use chrono::TimeDelta;
use tracing::info;

use crate::{
    AppState, database, dicton, get_current_season, news, send_notification_email, templates,
};

/// Compute the next occurrence of 5:00 AM local time strictly after `from_when`.
pub fn next_5am_local(
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

/// Background loop that preloads the dicton du jour and news feed.
///
/// Runs once immediately at startup, then sleeps until the next 5:00 AM local
/// time and repeats every day so the first visitor of the morning gets an
/// instant page load.
pub async fn daily_preload_loop(state: AppState) {
    loop {
        // ── Preload ─────────────────────────────────────────────────
        let season = get_current_season();
        let hf_token = state.config.huggingface_token.clone();
        let feed_url = state.config.rss_news_feed.clone();

        info!("daily preload: generating dicton du jour");
        let _ = dicton::get_or_generate(&state.db, season, &hf_token).await;

        if !feed_url.is_empty() {
            info!("daily preload: syncing news feed");
            news::sync_news(&state.db, &feed_url).await;
        }

        info!("daily preload: done");

        // ── Sleep until next 5 AM ───────────────────────────────────
        let now = chrono::Local::now();
        let Some(target) = next_5am_local(now) else {
            // Fallback: sleep 1 hour and retry
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
            continue;
        };

        let sleep_duration = (target - now)
            .to_std()
            .unwrap_or(tokio::time::Duration::from_secs(3600));
        info!(
            "daily preload: next run in {} seconds (at ~05:00)",
            sleep_duration.as_secs()
        );
        tokio::time::sleep(sleep_duration).await;
    }
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

        body.push_str("<p><em>— PowPow pour AG'HIL</em></p>");

        let subject = "AGHIL — Récapitulatif du lundi matin";
        send_notification_email(&state, &admin_emails, subject, &body).await;
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
