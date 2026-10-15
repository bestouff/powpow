use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{calendar_feed::CalendarEntry, models::Staff};

pub async fn token(pool: &PgPool, staff: Uuid) -> Result<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT token FROM calendar_subscriptions WHERE staff = $1")
            .bind(staff)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn create(pool: &PgPool, staff: Uuid, regenerate: bool) -> Result<String> {
    let token = crate::calendar_feed::new_token()?;
    Ok(sqlx::query_scalar("INSERT INTO calendar_subscriptions (staff, token) VALUES ($1, $2)
        ON CONFLICT (staff) DO UPDATE SET token = CASE WHEN $3 THEN EXCLUDED.token ELSE calendar_subscriptions.token END RETURNING token")
        .bind(staff).bind(token).bind(regenerate).fetch_one(pool).await?)
}

pub async fn load(pool: &PgPool, token: &str) -> Result<Option<(Staff, Vec<CalendarEntry>)>> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let staff: Option<Staff> = sqlx::query_as("SELECT s.* FROM staff s JOIN calendar_subscriptions c ON c.staff = s.id WHERE c.token = $1")
        .bind(token).fetch_optional(&mut *tx).await?;
    let Some(staff) = staff else {
        return Ok(None);
    };
    // Presence is the source of truth. Do not join current roles: leaving an
    // atelier must not erase historical registrations from subscribed calendars.
    let entries = sqlx::query_as(
        "SELECT n.id AS need_id, n.day, a.name AS atelier_name, n.nightly,
        p.first_half, p.second_half,
        GREATEST(p.calendar_updated_at, n.calendar_updated_at, a.calendar_updated_at) AS modified_at
        FROM presence p JOIN needs n ON n.id = p.needs JOIN ateliers a ON a.id = n.atelier
        WHERE p.staff = $1 AND (p.first_half OR p.second_half) ORDER BY n.day, n.id",
    )
    .bind(staff.id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some((staff, entries)))
}
