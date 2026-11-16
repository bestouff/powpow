use super::*;

#[sqlx::test(migrations = false)]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn pole_rename_preserves_existing_data_and_triggers(pool: PgPool) -> Result<()> {
    let previous = sqlx::migrate::Migrator::with_migrations(
        sqlx::migrate!("./migrations")
            .iter()
            .filter(|migration| migration.version < 52)
            .cloned()
            .collect(),
    );
    previous.run(&pool).await?;
    let member: uuid::Uuid = sqlx::query_scalar("INSERT INTO staff (first_name, last_name, email, comment) VALUES ('Test', 'Member', 'test@example.org', 'atelier') RETURNING id")
        .fetch_one(&pool).await?;
    let pole: uuid::Uuid = sqlx::query_scalar("INSERT INTO ateliers (name, slug, needs_validation, default_nightly, icon, opening_day_typical_needed, opening_day_minimal_needed, whatsapp_group_url, calendar_updated_at)
        VALUES ('Atelier hérité', 'legacy', true, true, 'users', 3, 1, 'https://chat.whatsapp.com/AbCdEfGhIjKlMnOpQrStUv', '2000-01-01') RETURNING id")
        .fetch_one(&pool).await?;
    let other: uuid::Uuid = sqlx::query_scalar("INSERT INTO ateliers (name, slug, needs_validation) VALUES ('Second', 'second', false) RETURNING id")
        .fetch_one(&pool).await?;
    sqlx::query("INSERT INTO roles (staff, atelier, validated, chief) VALUES ($1, $2, true, true)")
        .bind(member)
        .bind(pole)
        .execute(&pool)
        .await?;
    for days in [-7, 7] {
        let need: uuid::Uuid = sqlx::query_scalar("INSERT INTO needs (day, atelier, quantity, minimal_quantity, nightly, calendar_updated_at)
            VALUES (current_date + $1, $2, 3, 1, true, '2000-01-01') RETURNING id")
            .bind(days).bind(pole).fetch_one(&pool).await?;
        upsert_presence(&pool, need, member, true, false).await?;
    }
    let before: serde_json::Value = sqlx::query_scalar("SELECT jsonb_build_object(
        'poles', (SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM ateliers a),
        'roles', (SELECT jsonb_agg(to_jsonb(r) - 'atelier' || jsonb_build_object('pole', r.atelier) ORDER BY staff, atelier) FROM roles r),
        'needs', (SELECT jsonb_agg(to_jsonb(n) - 'atelier' || jsonb_build_object('pole', n.atelier) ORDER BY id) FROM needs n),
        'presence', (SELECT jsonb_agg(to_jsonb(p) ORDER BY needs, staff) FROM presence p))")
        .fetch_one(&pool).await?;

    assert_eq!(run_migrations(&pool).await?, 1);
    assert_eq!(run_migrations(&pool).await?, 0);
    let after: serde_json::Value = sqlx::query_scalar(
        "SELECT jsonb_build_object(
        'poles', (SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM poles a),
        'roles', (SELECT jsonb_agg(to_jsonb(r) ORDER BY staff, pole) FROM roles r),
        'needs', (SELECT jsonb_agg(to_jsonb(n) ORDER BY id) FROM needs n),
        'presence', (SELECT jsonb_agg(to_jsonb(p) ORDER BY needs, staff) FROM presence p))",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(after, before);
    let old_names: Vec<String> = sqlx::query_scalar("SELECT name FROM (
        SELECT relname AS name FROM pg_class WHERE relnamespace = current_schema()::regnamespace
        UNION ALL SELECT conname FROM pg_constraint WHERE connamespace = current_schema()::regnamespace
        UNION ALL SELECT typname FROM pg_type WHERE typnamespace = current_schema()::regnamespace
        UNION ALL SELECT tgname FROM pg_trigger t JOIN pg_class c ON c.oid = t.tgrelid WHERE c.relnamespace = current_schema()::regnamespace
        UNION ALL SELECT attname FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid WHERE c.relnamespace = current_schema()::regnamespace AND NOT a.attisdropped
        ) names WHERE name LIKE '%atelier%'")
        .fetch_all(&pool).await?;
    assert!(old_names.is_empty(), "Old schema names: {old_names:?}");
    assert!(get_pole_by_id(&pool, pole).await?.is_some());
    assert_eq!(get_staff_roles(&pool, member).await?[0].pole, pole);

    let refreshed: bool = sqlx::query_scalar("UPDATE poles SET name = 'Nouveau nom' WHERE id = $1 RETURNING calendar_updated_at > '2000-01-01'::timestamptz")
        .bind(pole).fetch_one(&pool).await?;
    assert!(refreshed);
    // The existing WHEN expression must track the renamed needs.pole column.
    let refreshed: bool = sqlx::query_scalar("UPDATE needs SET pole = $1 WHERE pole = $2 AND day > current_date RETURNING calendar_updated_at > '2000-01-01'::timestamptz")
        .bind(other).bind(pole).fetch_one(&pool).await?;
    assert!(refreshed);
    sqlx::query("UPDATE needs SET pole = $1 WHERE pole = $2")
        .bind(pole)
        .bind(other)
        .execute(&pool)
        .await?;
    update_role(&pool, member, pole, Some(false), None).await?;
    let remaining: Vec<bool> = sqlx::query_scalar("SELECT n.day < current_date FROM presence p JOIN needs n ON n.id = p.needs WHERE p.staff = $1")
        .bind(member).fetch_all(&pool).await?;
    assert_eq!(remaining, vec![true]);
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn legacy_pole_backups_restore_and_reexport_without_changing_data(
    pool: PgPool,
) -> Result<()> {
    let member = uuid::Uuid::new_v4();
    let pole = uuid::Uuid::new_v4();
    let need = uuid::Uuid::new_v4();
    let legacy = format!(
        r#"TRUNCATE presence CASCADE;
TRUNCATE roles CASCADE;
TRUNCATE needs CASCADE;
TRUNCATE ateliers CASCADE;
TRUNCATE staff CASCADE;
COPY staff ("id", "first_name", "last_name", "email", "comment") FROM stdin;
{member}	Test	Member	test@example.org	atelier ateliers "atelier" COPY ateliers (
\.
COPY ateliers ("id", "name", "slug", "needs_validation", "default_nightly", "icon", "opening_day_typical_needed") FROM stdin;
{pole}	Atelier hérité	atelier-legacy	t	f	users	3
\.
COPY roles ("staff", "atelier", "validated", "chief") FROM stdin;
{member}	{pole}	t	f
\.
COPY needs ("id", "day", "atelier", "quantity", "nightly") FROM stdin;
{need}	2026-01-01	{pole}	4	f
\.
COPY presence ("needs", "staff", "first_half", "second_half") FROM stdin;
{need}	{member}	t	f
\.
"#
    );
    restore_from_sql(&pool, &legacy).await?;
    let original_pole = get_pole_by_id(&pool, pole).await?.unwrap();
    assert_eq!(original_pole.name, "Atelier hérité");
    assert_eq!(original_pole.slug, "atelier-legacy");
    assert_eq!(original_pole.opening_day_typical_needed, 3);
    assert_eq!(original_pole.opening_day_minimal_needed, 3);
    let original_need = get_need_by_id(&pool, need).await?.unwrap();
    assert_eq!(original_need.pole, pole);
    assert_eq!(original_need.quantity, 4);
    assert_eq!(original_need.minimal_quantity, 4);
    assert_eq!(get_staff_roles(&pool, member).await?[0].pole, pole);
    assert_eq!(
        get_presence(&pool, need, member).await?,
        Some((true, false))
    );
    let original_staff = get_staff_by_id(&pool, member).await?.unwrap();
    assert_eq!(
        original_staff.comment,
        "atelier ateliers \"atelier\" COPY ateliers ("
    );

    let backup = backup_all_tables(&pool).await?;
    assert!(backup.contains("TRUNCATE poles CASCADE;"));
    assert!(backup.contains("COPY poles ("));
    assert!(
        !backup.lines().any(
            |line| line.starts_with("COPY ateliers (") || line.starts_with("TRUNCATE ateliers")
        )
    );
    for table in ["roles", "needs"] {
        let header = backup
            .lines()
            .find(|line| line.starts_with(&format!("COPY {table} (")))
            .unwrap();
        assert!(header.contains("\"pole\""));
        assert!(!header.contains("\"atelier\""));
    }
    restore_from_sql(&pool, &backup).await?;
    assert_eq!(
        serde_json::to_value(get_pole_by_id(&pool, pole).await?.unwrap())?,
        serde_json::to_value(original_pole)?
    );
    assert_eq!(
        serde_json::to_value(get_need_by_id(&pool, need).await?.unwrap())?,
        serde_json::to_value(original_need)?
    );
    assert_eq!(
        get_staff_by_id(&pool, member).await?.unwrap().comment,
        original_staff.comment
    );
    assert_eq!(
        get_presence(&pool, need, member).await?,
        Some((true, false))
    );
    Ok(())
}
