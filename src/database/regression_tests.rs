use super::*;

async fn staff(pool: &PgPool) -> Result<Staff> {
    Ok(sqlx::query_as("INSERT INTO staff (first_name, last_name, email, comment) VALUES ('Test', 'Member', 'old@example.org', '') RETURNING *")
        .fetch_one(pool).await?)
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
