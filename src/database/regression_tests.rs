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
