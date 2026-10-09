use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{email_change::EmailChange, models::Staff};

pub async fn create(pool: &PgPool, change: &EmailChange) -> Result<()> {
    sqlx::query(
        "INSERT INTO email_change_tokens (token, staff, expires_at) VALUES ($1, $2, to_timestamp($3))
         ON CONFLICT (staff) DO UPDATE SET token = EXCLUDED.token, expires_at = EXCLUDED.expires_at",
    ).bind(change.token).bind(change.staff_id).bind(change.expires_at as f64).execute(pool).await?;
    Ok(())
}

pub async fn cancel(pool: &PgPool, token: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM email_change_tokens WHERE token = $1")
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn pending_staff(pool: &PgPool, change: &EmailChange) -> Result<Option<Staff>> {
    Ok(sqlx::query_as::<_, Staff>(
        "SELECT s.* FROM staff s JOIN email_change_tokens t ON t.staff = s.id
         WHERE t.token = $1 AND s.id = $2 AND t.expires_at > now() AND s.email = $3",
    )
    .bind(change.token)
    .bind(change.staff_id)
    .bind(&change.old_email)
    .fetch_optional(pool)
    .await?)
}

/// Consume the token and apply the new address atomically. The expected old
/// address prevents a stale confirmation from undoing a later account update.
pub async fn apply(pool: &PgPool, change: &EmailChange) -> Result<Option<Staff>> {
    let mut tx = pool.begin().await?;
    let consumed = sqlx::query(
        "DELETE FROM email_change_tokens WHERE token = $1 AND staff = $2 AND expires_at > now()",
    )
    .bind(change.token)
    .bind(change.staff_id)
    .execute(&mut *tx)
    .await?;
    if consumed.rows_affected() == 0 {
        return Ok(None);
    }
    let staff = sqlx::query_as::<_, Staff>(
        "UPDATE staff SET email = $1, updated_at = now()
         WHERE id = $2 AND email = $3 RETURNING *",
    )
    .bind(&change.new_email)
    .bind(change.staff_id)
    .bind(&change.old_email)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(staff)
}

pub async fn update_phone(pool: &PgPool, staff_id: Uuid, phone: Option<&str>) -> Result<()> {
    sqlx::query("UPDATE staff SET phone = $2, updated_at = now() WHERE id = $1")
        .bind(staff_id)
        .bind(phone)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn is_pending(pool: &PgPool, staff_id: Uuid) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM email_change_tokens WHERE staff = $1 AND expires_at > now())",
    )
    .bind(staff_id)
    .fetch_one(pool)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
    async fn email_changes_are_pending_single_use_and_person_specific() -> Result<()> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&std::env::var("DATABASE_URL")?)
            .await?;
        sqlx::raw_sql(
            "CREATE TEMP TABLE staff (id uuid PRIMARY KEY, first_name text, last_name text, email text, phone text, comment text, is_admin bool DEFAULT false, is_god bool DEFAULT false, no_import_emails bool DEFAULT false, no_weekly_emails bool DEFAULT false, newsletter_status text DEFAULT 'subscribed', created_at timestamptz DEFAULT now(), updated_at timestamptz DEFAULT now());
             CREATE TEMP TABLE email_change_tokens (token uuid PRIMARY KEY, staff uuid UNIQUE REFERENCES staff(id), expires_at timestamptz NOT NULL);",
        ).execute(&pool).await?;
        let staff_id = Uuid::new_v4();
        let child = Uuid::new_v4();
        for id in [staff_id, child] {
            sqlx::query("INSERT INTO staff (id, first_name, last_name, email, comment) VALUES ($1, 'Test', 'Family', 'old@example.org', '')")
                .bind(id).execute(&pool).await?;
        }
        let mut change = EmailChange {
            staff_id,
            token: Uuid::new_v4(),
            old_email: "old@example.org".into(),
            new_email: "new@example.org".into(),
            expires_at: (chrono::Utc::now() + crate::email_change::LIFETIME).timestamp(),
        };
        create(&pool, &change).await?;
        assert!(is_pending(&pool, staff_id).await?);
        update_phone(&pool, staff_id, Some("+33 6 12 34 56 78")).await?;
        for _ in 0..3 {
            let staff = pending_staff(&pool, &change).await?.unwrap();
            assert_eq!(staff.email, "old@example.org");
            assert_eq!(staff.phone.as_deref(), Some("+33 6 12 34 56 78"));
        }
        let old_token = change.token;
        change.token = Uuid::new_v4();
        create(&pool, &change).await?;
        let replacement = change.token;
        change.token = old_token;
        assert!(apply(&pool, &change).await?.is_none());
        change.token = replacement;
        change.staff_id = child;
        assert!(apply(&pool, &change).await?.is_none());
        change.staff_id = staff_id;
        let applied = apply(&pool, &change).await?.unwrap();
        assert_eq!(applied.email, "new@example.org");
        assert_eq!(applied.phone.as_deref(), Some("+33 6 12 34 56 78"));
        assert!(!is_pending(&pool, staff_id).await?);
        assert!(apply(&pool, &change).await?.is_none());
        assert_eq!(
            crate::database::get_staff_by_id(&pool, child)
                .await?
                .unwrap()
                .email,
            "old@example.org"
        );

        change.old_email = "new@example.org".into();
        change.new_email = "old@example.org".into();
        change.token = Uuid::new_v4();
        create(&pool, &change).await?;
        assert_eq!(
            apply(&pool, &change).await?.unwrap().email,
            "old@example.org"
        );
        assert_eq!(
            crate::database::get_staff_by_id(&pool, child)
                .await?
                .unwrap()
                .email,
            "old@example.org"
        );

        change.old_email = "old@example.org".into();
        change.new_email = "expired@example.org".into();
        change.token = Uuid::new_v4();
        change.expires_at = chrono::Utc::now().timestamp() - 1;
        create(&pool, &change).await?;
        assert!(pending_staff(&pool, &change).await?.is_none());
        assert!(apply(&pool, &change).await?.is_none());

        change.expires_at = (chrono::Utc::now() + crate::email_change::LIFETIME).timestamp();
        change.token = Uuid::new_v4();
        create(&pool, &change).await?;
        sqlx::query("UPDATE staff SET email = 'admin-change@example.org' WHERE id = $1")
            .bind(staff_id)
            .execute(&pool)
            .await?;
        assert!(pending_staff(&pool, &change).await?.is_none());
        assert!(apply(&pool, &change).await?.is_none());
        assert_eq!(
            crate::database::get_staff_by_id(&pool, staff_id)
                .await?
                .unwrap()
                .email,
            "admin-change@example.org"
        );
        pool.close().await;
        Ok(())
    }
}
