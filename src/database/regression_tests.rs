use super::*;

async fn staff(pool: &PgPool) -> Result<Staff> {
    Ok(sqlx::query_as("INSERT INTO staff (first_name, last_name, email, comment) VALUES ('Test', 'Member', 'old@example.org', '') RETURNING *")
        .fetch_one(pool).await?)
}

async fn need(pool: &PgPool, name: &str) -> Result<Need> {
    let atelier: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO ateliers (name, slug, needs_validation) VALUES ($1, $1, true) RETURNING id",
    )
    .bind(name)
    .fetch_one(pool)
    .await?;
    Ok(sqlx::query_as("INSERT INTO needs (day, atelier, quantity, nightly) VALUES (current_date + 7, $1, 2, false) RETURNING *")
        .bind(atelier).fetch_one(pool).await?)
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn planning_requires_validated_enrollment(pool: PgPool) -> Result<()> {
    let person = staff(&pool).await?;
    let need = need(&pool, "Validation").await?;
    assert!(!can_register_for_need(&pool, person.id, need.id).await?);
    add_role(&pool, person.id, need.atelier, false).await?;
    assert!(!can_register_for_need(&pool, person.id, need.id).await?);
    update_role(&pool, person.id, need.atelier, Some(true), None).await?;
    assert!(can_register_for_need(&pool, person.id, need.id).await?);
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn concurrent_planning_checks_are_atomic(pool: PgPool) -> Result<()> {
    let person = staff(&pool).await?;
    let first = need(&pool, "First").await?;
    let second = need(&pool, "Second").await?;
    for need in [&first, &second] {
        add_role(&pool, person.id, need.atelier, true).await?;
    }
    let (a, b) = tokio::join!(
        planning::toggle(&pool, person.id, first.id, "first", true),
        planning::toggle(&pool, person.id, second.id, "first", true)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    assert!(
        matches!(a, Err(planning::Error::Conflict(_)))
            || matches!(b, Err(planning::Error::Conflict(_)))
    );
    let winner = if a.is_ok() { first.id } else { second.id };
    let (a, b) = tokio::join!(
        planning::toggle(&pool, person.id, winner, "first", true),
        planning::toggle(&pool, person.id, winner, "second", true)
    );
    a?;
    b?;
    assert_eq!(
        get_presence(&pool, winner, person.id).await?,
        Some((true, true))
    );
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn leaving_or_revoking_an_atelier_clears_future_presence(pool: PgPool) -> Result<()> {
    let person = staff(&pool).await?;
    let future = need(&pool, "Leaving").await?;
    add_role(&pool, person.id, future.atelier, true).await?;
    planning::toggle(&pool, person.id, future.id, "first", true).await?;
    let past: uuid::Uuid = sqlx::query_scalar("INSERT INTO needs (day, atelier, quantity, nightly) VALUES (current_date - 7, $1, 2, false) RETURNING id")
        .bind(future.atelier).fetch_one(&pool).await?;
    upsert_presence(&pool, past, person.id, true, false).await?;
    remove_role(&pool, person.id, future.atelier).await?;
    assert!(get_presence(&pool, future.id, person.id).await?.is_none());
    assert!(get_presence(&pool, past, person.id).await?.is_some());
    add_role(&pool, person.id, future.atelier, true).await?;
    planning::toggle(&pool, person.id, future.id, "first", true).await?;
    update_role(&pool, person.id, future.atelier, Some(false), None).await?;
    assert!(get_presence(&pool, future.id, person.id).await?.is_none());
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn shortage_counts_the_least_filled_half(pool: PgPool) -> Result<()> {
    let need = need(&pool, "Coverage").await?;
    for _ in 0..2 {
        let person = staff(&pool).await?;
        upsert_presence(&pool, need.id, person.id, true, false).await?;
    }
    let coverage = get_upcoming_needs_deficit(&pool, need.day, need.day).await?;
    assert_eq!(coverage[0].3, 0);
    sqlx::query("UPDATE presence SET second_half = true")
        .execute(&pool)
        .await?;
    let coverage = get_upcoming_needs_deficit(&pool, need.day, need.day).await?;
    assert_eq!(coverage[0].3, 2);
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn unimport_uses_june_to_may_seasons(pool: PgPool) -> Result<()> {
    let person = staff(&pool).await?;
    let need = need(&pool, "Season").await?;
    for date in ["2025-05-31", "2025-06-01", "2026-05-31", "2026-06-01"] {
        let id: uuid::Uuid = sqlx::query_scalar("INSERT INTO needs (day, atelier, quantity, nightly) VALUES ($1::text::date, $2, 1, false) RETURNING id")
            .bind(date).bind(need.atelier).fetch_one(&pool).await?;
        upsert_presence(&pool, id, person.id, true, false).await?;
    }
    assert_eq!(
        get_unimport_consequences(&pool, person.id, uuid::Uuid::new_v4(), 2026)
            .await?
            .presences,
        2
    );
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn retired_addresses_preserve_household_delivery(pool: PgPool) -> Result<()> {
    let first = staff(&pool).await?;
    let second = staff(&pool).await?;
    update_staff_contact(&pool, first.id, "new@example.org", None).await?;
    let count = "SELECT count(*) FROM mailchimp_retired_emails r WHERE NOT EXISTS (SELECT 1 FROM staff s WHERE lower(trim(s.email)) = r.email)";
    assert_eq!(
        sqlx::query_scalar::<_, i64>(count).fetch_one(&pool).await?,
        0
    );
    update_staff_contact(&pool, second.id, "other@example.org", None).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(count).fetch_one(&pool).await?,
        1
    );
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn email_updates_revoke_old_links(pool: PgPool) -> Result<()> {
    let person = staff(&pool).await?;
    let token = create_login_token(&pool, person.id, &person.email).await?;
    update_staff_contact(&pool, person.id, "new@example.org", None).await?;
    assert!(
        consume_login_token(&pool, person.id, token)
            .await?
            .is_none()
    );
    assert!(
        create_login_token(&pool, person.id, &person.email)
            .await
            .is_err()
    );
    update_staff_contact(&pool, person.id, &person.email, None).await?;
    assert!(
        consume_login_token(&pool, person.id, token)
            .await?
            .is_none()
    );
    let token = create_login_token(&pool, person.id, &person.email).await?;
    assert!(
        consume_login_token(&pool, person.id, token)
            .await?
            .is_some()
    );
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn session_cap_keeps_recent_sessions(pool: PgPool) -> Result<()> {
    let person = staff(&pool).await?;
    let oldest = create_session(&pool, person.id, None, None).await?;
    sqlx::query("UPDATE sessions SET last_seen = now() - interval '1 day' WHERE id = $1")
        .bind(oldest)
        .execute(&pool)
        .await?;
    let mut recent = vec![];
    for _ in 0..MAX_SESSIONS_PER_STAFF {
        recent.push(create_session(&pool, person.id, None, None).await?);
    }
    assert!(get_session_staff(&pool, oldest).await?.is_none());
    for id in recent {
        assert!(get_session_staff(&pool, id).await?.is_some());
    }
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn backup_roundtrip_preserves_reviews_news_and_sequences(pool: PgPool) -> Result<()> {
    sqlx::raw_sql("INSERT INTO memberships (helloasso_order_id, helloasso_item_id) VALUES (1,1);
        INSERT INTO auto_import_reviews (helloasso_item_id, reason) VALUES (1, 'deliberately unimported');
        INSERT INTO news (guid, text, link) VALUES ('test', 'News', 'https://example.org');
        INSERT INTO qualifications (name) VALUES ('First');
        SELECT setval('qualifications_id_seq', 100);").execute(&pool).await?;
    let backup = backup_all_tables(&pool).await?;
    sqlx::raw_sql("TRUNCATE qualifications RESTART IDENTITY CASCADE; DELETE FROM news; DELETE FROM auto_import_reviews;").execute(&pool).await?;
    restore_from_sql(&pool, &backup).await?;
    let reason: String =
        sqlx::query_scalar("SELECT reason FROM auto_import_reviews WHERE helloasso_item_id = 1")
            .fetch_one(&pool)
            .await?;
    assert_eq!(reason, "deliberately unimported");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM news")
            .fetch_one(&pool)
            .await?,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "INSERT INTO qualifications (name) VALUES ('Next') RETURNING id"
        )
        .fetch_one(&pool)
        .await?,
        101
    );
    assert!(
        restore_from_sql(&pool, "TRUNCATE news;\nINVALID SQL;")
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM news")
            .fetch_one(&pool)
            .await?,
        1
    );
    Ok(())
}
