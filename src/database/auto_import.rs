use anyhow::Result;
use chrono::{DateTime, Datelike, Utc};
use sqlx::{PgConnection, PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

use crate::auto_import::{
    DUPLICATE_REASON, Decision, Identity, Input, Matcher, Source, normalize_name,
};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Record {
    pub input: Input,
    pub target: Option<Uuid>,
    pub comment: String,
    pub eligible: bool,
    pub paid_season: Option<i16>,
}

/// Serialize automatic and manual imports across sources, including creation of
/// new identities. The unique payment-source constraints provide idempotency too.
pub async fn lock_import(connection: &mut PgConnection) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(703202401)")
        .execute(connection)
        .await?;
    Ok(())
}

pub async fn load_catalog(
    connection: &mut PgConnection,
) -> Result<(Vec<(Uuid, Identity)>, Vec<Record>)> {
    let staff = sqlx::query(
        "SELECT id, first_name, last_name, email, COALESCE(phone, '') AS phone FROM staff",
    )
    .fetch_all(&mut *connection)
    .await?
    .into_iter()
    .map(|row| {
        (
            row.get("id"),
            Identity {
                first_name: row.get("first_name"),
                last_name: row.get("last_name"),
                email: row.get("email"),
                phone: row.get("phone"),
            },
        )
    })
    .collect();
    let rows = sqlx::query(
        r"SELECT m.helloasso_item_id, NULL::uuid AS cash_id, m.helloasso_order_id AS order_id,
                 COALESCE(m.beneficiary_first_name, '') AS first_name,
                 COALESCE(m.beneficiary_last_name, '') AS last_name,
                 COALESCE(NULLIF(TRIM(m.email), ''), m.payer_email, '') AS email,
                 COALESCE(m.phone, '') AS phone, m.order_date AS date, p.staff AS target,
                 NULL::date AS cash_date, COALESCE(m.comment, '') AS source_comment,
                 to_jsonb(m)->>'source_state' AS source_state, p.season AS paid_season, m.amount
          FROM memberships m LEFT JOIN payments p USING (helloasso_item_id)
          WHERE m.item_type = 'Membership'
          UNION ALL
          SELECT NULL::bigint, c.id, NULL::bigint, c.first_name, c.last_name,
                 COALESCE(c.email, ''), COALESCE(c.phone, ''), c.date::timestamptz,
                 p.staff, c.date, ''::text, 'Processed'::text, p.season, c.amount
          FROM cash c LEFT JOIN payments p ON p.cash_id = c.id
          WHERE c.is_membership
          ORDER BY date NULLS LAST, helloasso_item_id, cash_id",
    )
    .fetch_all(connection)
    .await?;
    let mut names_per_order = HashMap::new();
    for row in &rows {
        if let Some(order) = row.get::<Option<i64>, _>("order_id") {
            let key = (
                order,
                normalize_name(row.get("first_name")),
                normalize_name(row.get("last_name")),
            );
            *names_per_order.entry(key).or_insert(0_usize) += 1;
        }
    }
    let records = rows
        .into_iter()
        .map(|row| {
            let date: Option<DateTime<Utc>> = row.get("date");
            let first_name: String = row.get("first_name");
            let last_name: String = row.get("last_name");
            let repeated_beneficiary = row.get::<Option<i64>, _>("order_id").is_some_and(|order| {
                names_per_order[&(
                    order,
                    normalize_name(&first_name),
                    normalize_name(&last_name),
                )] > 1
            });
            let source = if let Some(id) = row.get::<Option<i64>, _>("helloasso_item_id") {
                Source::HelloAsso(id)
            } else {
                Source::Cash(row.get("cash_id"))
            };
            // Cash dates are calendar dates, not instants: casting June 1 to
            // UTC must not turn it into May 31 in a non-UTC database timezone.
            let season = if matches!(source, Source::Cash(_)) {
                let cash_date: chrono::NaiveDate = row.get("cash_date");
                Some((cash_date.year() + i32::from(cash_date.month() >= 6)) as i16)
            } else {
                date.map(|date| (date.year() + i32::from(date.month() >= 6)) as i16)
            };
            Record {
                input: Input {
                    source,
                    identity: Identity {
                        first_name,
                        last_name,
                        email: row.get("email"),
                        phone: row.get("phone"),
                    },
                    season,
                    amount: row.get("amount"),
                    is_membership: true,
                    repeated_beneficiary,
                },
                target: row.get("target"),
                comment: row.get("source_comment"),
                eligible: row
                    .get::<Option<String>, _>("source_state")
                    .is_none_or(|state| state == "Processed"),
                paid_season: row.get("paid_season"),
            }
        })
        .collect();
    Ok((staff, records))
}

pub fn matcher_for(
    staff: &[(Uuid, Identity)],
    records: &[Record],
    exclude: Option<Source>,
) -> Matcher {
    let mut matcher = Matcher::default();
    for (id, identity) in staff {
        matcher.remember(*id, identity);
    }
    for record in records {
        if let Some(id) = record.target {
            matcher.confirm_staff(id);
            if Some(record.input.source) != exclude
                && let Some(season) = record.paid_season
            {
                matcher.remember_payment(id, season);
            }
        }
        if Some(record.input.source) != exclude
            && !record.input.repeated_beneficiary
            && let Some(id) = record.target
        {
            matcher.remember_imported(id, &record.input.identity);
        }
    }
    matcher
}

pub async fn mark_review(
    connection: &mut PgConnection,
    source: Source,
    reason: &str,
) -> Result<()> {
    let (online, cash) = source_ids(source);
    // Under the import lock there can be only one writer for a source.
    sqlx::query("DELETE FROM auto_import_reviews WHERE helloasso_item_id = $1 OR cash_id = $2")
        .bind(online)
        .bind(cash)
        .execute(&mut *connection)
        .await?;
    sqlx::query(
        "INSERT INTO auto_import_reviews (helloasso_item_id, cash_id, reason) VALUES ($1, $2, $3)",
    )
    .bind(online)
    .bind(cash)
    .bind(reason)
    .execute(connection)
    .await?;
    Ok(())
}

fn source_ids(source: Source) -> (Option<i64>, Option<Uuid>) {
    match source {
        Source::HelloAsso(id) => (Some(id), None),
        Source::Cash(id) => (None, Some(id)),
    }
}

#[derive(Default)]
pub struct ImportSummary {
    pub imported: usize,
    pub reviews: Vec<(Source, String)>,
}

/// Only membership records without an existing payment or review marker are
/// considered. Admin decisions (including deliberate unimports) are retained.
pub async fn auto_import_pending(pool: &PgPool) -> Result<ImportSummary> {
    let (_, records) = load_catalog(&mut *pool.acquire().await?).await?;
    let pending: Vec<_> = records
        .into_iter()
        .filter(|record| record.target.is_none() && record.eligible)
        .map(|record| record.input.source)
        .collect();
    let mut summary = ImportSummary::default();
    for source in pending {
        let mut tx = pool.begin().await?;
        lock_import(&mut tx).await?;
        let (online, cash) = source_ids(source);
        let reviewed: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM auto_import_reviews WHERE helloasso_item_id = $1 OR cash_id = $2)")
            .bind(online).bind(cash).fetch_one(&mut *tx).await?;
        if reviewed {
            continue;
        }
        let (staff, records) = load_catalog(&mut tx).await?;
        let Some(record) = records.iter().find(|record| {
            record.input.source == source && record.target.is_none() && record.eligible
        }) else {
            continue;
        };
        let decision = matcher_for(&staff, &records, Some(source)).decide(&record.input);
        let (target, rule) = match decision {
            Decision::Link { staff, rule } => (staff, rule),
            Decision::Create => {
                let identity = &record.input.identity;
                let id = sqlx::query_scalar::<_, Uuid>(
                    "INSERT INTO staff (first_name, last_name, email, phone, comment)
                     VALUES ($1, $2, $3, NULLIF($4, ''), $5) RETURNING id",
                )
                .bind(crate::auto_import::capitalize_words(&identity.first_name))
                .bind(crate::auto_import::capitalize_words(&identity.last_name))
                .bind(identity.email.trim().to_lowercase())
                .bind(identity.phone.trim())
                .bind(record.comment.trim())
                .fetch_one(&mut *tx)
                .await?;
                (id, "nouvelle_personne")
            }
            Decision::Review(reason) => {
                mark_review(&mut tx, source, reason).await?;
                tx.commit().await?;
                summary.reviews.push((source, reason.to_string()));
                continue;
            }
            Decision::Skip => continue,
        };
        // Automatic renewals preserve the established name, login email, phone,
        // comments, privileges and roles. Only the new payment is attached.
        let (online, cash) = source_ids(source);
        let already_paid: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM payments WHERE staff = $1 AND season = $2)",
        )
        .bind(target)
        .bind(record.input.season)
        .fetch_one(&mut *tx)
        .await?;
        if already_paid {
            mark_review(&mut tx, source, DUPLICATE_REASON).await?;
            tx.commit().await?;
            summary.reviews.push((source, DUPLICATE_REASON.to_string()));
            continue;
        }
        sqlx::query("INSERT INTO payments (staff, season, helloasso_item_id, cash_id) VALUES ($1, $2, $3, $4)")
            .bind(target).bind(record.input.season).bind(online).bind(cash).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO audit (staff_name, operation, detail) VALUES ('Auto-import', 'Import automatique adhésion', $1)")
            .bind(format!("{} staff={target} rule={rule}", source.label())).execute(&mut *tx).await?;
        tx.commit().await?;
        summary.imported += 1;
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
    async fn auto_import_is_atomic_idempotent_and_respects_manual_review() -> Result<()> {
        let schema = format!("auto_import_test_{}", Uuid::new_v4().simple());
        let connection_schema = schema.clone();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(3)
            .after_connect(move |connection, _| {
                let schema = connection_schema.clone();
                Box::pin(async move {
                    sqlx::query("SELECT set_config('search_path', $1, false)")
                        .bind(schema)
                        .execute(&mut *connection)
                        .await?;
                    sqlx::query("SET TIME ZONE 'Europe/Paris'")
                        .execute(connection)
                        .await?;
                    Ok(())
                })
            })
            .connect(&std::env::var("DATABASE_URL")?)
            .await?;
        // Only a generated hexadecimal identifier is interpolated.
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
            .execute(&pool)
            .await?;
        let result = async {
            sqlx::raw_sql(
                "CREATE TABLE staff (id uuid PRIMARY KEY DEFAULT gen_random_uuid(), first_name text, last_name text, email text, phone text, comment text, is_admin bool DEFAULT false);
                 CREATE TABLE memberships (helloasso_item_id bigint PRIMARY KEY CHECK (helloasso_item_id > 0), helloasso_order_id bigint, beneficiary_first_name text, beneficiary_last_name text, email text, payer_email text, phone text, order_date timestamptz, item_type text, comment text DEFAULT '', item_name text, tier_name text, amount integer, created_at timestamptz DEFAULT now(), updated_at timestamptz DEFAULT now());
                 CREATE TYPE membership_type AS (helloasso_order_id bigint, helloasso_item_id bigint, payer_email text, beneficiary_first_name text, beneficiary_last_name text, phone text, email text, item_name text, item_type text, tier_name text, amount integer, order_date timestamptz, comment text, created_at timestamptz, updated_at timestamptz);
                 CREATE TABLE cash (id uuid PRIMARY KEY, first_name text, last_name text, email text, phone text, date date, is_membership bool, amount integer DEFAULT 20);
                 CREATE TABLE payments (id uuid PRIMARY KEY DEFAULT gen_random_uuid(), staff uuid REFERENCES staff(id), season smallint NOT NULL, helloasso_item_id bigint UNIQUE REFERENCES memberships(helloasso_item_id), cash_id uuid UNIQUE REFERENCES cash(id), CHECK ((helloasso_item_id IS NOT NULL) <> (cash_id IS NOT NULL)));
                 CREATE TABLE audit (staff_name text, operation text, detail text);",
            ).execute(&pool).await?;
            sqlx::query("INSERT INTO memberships VALUES (6, 6, 'Existing', 'Pending', 'pending@example.org', NULL, NULL, now(), 'Membership')")
                .execute(&pool).await?;
            sqlx::raw_sql(include_str!("../../migrations/043_auto_import_reviews.sql")).execute(&pool).await?;
            let alice = Uuid::new_v4();
            sqlx::query("INSERT INTO staff VALUES ($1, 'Alice', 'Martin', 'canonical@example.org', '0612345678', 'Admin note', true)")
                .bind(alice).execute(&pool).await?;
            sqlx::raw_sql(
                "INSERT INTO memberships VALUES
                    (1, 1, 'Alice', 'Martin', 'changed@example.org', 'parent@example.org', '0712345678', '2026-06-01 12:00:00+00', 'Membership'),
                    (2, 2, 'Carol', 'Dupont', 'carol@example.org', NULL, NULL, '2026-06-01 12:00:00+00', 'Membership'),
                    (3, 3, 'Parent', 'Family', 'shared@example.org', NULL, NULL, now(), 'Membership'),
                    (4, 3, 'Parent', 'Family', 'shared@example.org', NULL, NULL, now(), 'Membership'),
                    (5, 5, 'Donor', 'Donation', 'donor@example.org', NULL, NULL, now(), 'Donation');
                 INSERT INTO cash VALUES
                    ('00000000-0000-0000-0000-000000000001', 'Alice', 'Martin', NULL, NULL, '2026-06-01', true),
                    ('00000000-0000-0000-0000-000000000002', 'Dana', 'Newperson', NULL, NULL, current_date, true);",
            ).execute(&pool).await?;
            sqlx::query("UPDATE memberships SET source_state = 'Processed', amount = 2000 WHERE helloasso_item_id BETWEEN 1 AND 5").execute(&pool).await?;
            sqlx::query("INSERT INTO memberships (helloasso_item_id, helloasso_order_id, beneficiary_first_name, beneficiary_last_name, email, order_date, item_type, source_state) VALUES (7, 7, 'Waiting', 'Payment', 'waiting@example.org', now(), 'Membership', 'Registered')")
                .execute(&pool).await?;
            // Two workers processing the same queue must not duplicate staff or payments.
            let (first, second) = tokio::join!(auto_import_pending(&pool), auto_import_pending(&pool));
            let first = first?;
            let second = second?;
            assert_eq!(first.imported + second.imported, 2);
            assert_eq!(first.reviews.len() + second.reviews.len(), 4);
            assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM staff").fetch_one(&pool).await?, 2);
            assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM payments WHERE staff = $1").bind(alice).fetch_one(&pool).await?, 1);
            assert_eq!(sqlx::query_scalar::<_, i16>("SELECT season FROM payments WHERE cash_id = '00000000-0000-0000-0000-000000000001'").fetch_one(&pool).await?, 2027);
            assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM payments WHERE helloasso_item_id = 6").fetch_one(&pool).await?, 0);
            let row = sqlx::query("SELECT email, phone, comment, is_admin FROM staff WHERE id = $1").bind(alice).fetch_one(&pool).await?;
            assert_eq!(row.get::<String, _>("email"), "canonical@example.org");
            assert_eq!(row.get::<String, _>("phone"), "0612345678");
            assert_eq!(row.get::<String, _>("comment"), "Admin note");
            assert!(row.get::<bool, _>("is_admin"));
            let again = auto_import_pending(&pool).await?;
            assert_eq!(again.imported, 0);
            assert_eq!(again.reviews.len(), 0);
            let payment = sqlx::query_scalar::<_, Uuid>("SELECT id FROM payments WHERE cash_id = '00000000-0000-0000-0000-000000000001'").fetch_one(&pool).await?;
            crate::database::delete_payment(&pool, payment).await?;
            assert_eq!(auto_import_pending(&pool).await?.imported, 0);
            assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM payments WHERE helloasso_item_id IN (1, 5, 7)").fetch_one(&pool).await?, 0);
            // A provider state transition to paid/Processed makes it eligible;
            // the earlier unpaid notification must not leave a review marker.
            sqlx::query("UPDATE memberships SET source_state = 'Processed', amount = 2000 WHERE helloasso_item_id = 7").execute(&pool).await?;
            assert_eq!(auto_import_pending(&pool).await?.imported, 1);
            let membership = |id| crate::models::Membership {
                helloasso_order_id: 42, helloasso_item_id: id,
                payer_email: Some("household@example.org".into()),
                beneficiary_first_name: Some("Shared".into()), beneficiary_last_name: Some("RawOrder".into()),
                phone: None, email: Some("household@example.org".into()),
                item_name: None, item_type: Some("Membership".into()), tier_name: None,
                amount: Some(2000), order_date: Some(Utc::now()), comment: Some("Original comment".into()),
                created_at: Utc::now(), updated_at: Utc::now(),
            };
            let batch = vec![(membership(9), "Processed".into()), (membership(10), "Processed".into())];
            assert_eq!(crate::database::upsert_memberships(&pool, &batch).await?, 2);
            let grouped = auto_import_pending(&pool).await?;
            assert_eq!(grouped.imported, 0);
            assert_eq!(grouped.reviews.len(), 2);
            let failed_batch = vec![(membership(21), "Processed".into()), (membership(-1), "Processed".into())];
            assert!(crate::database::upsert_memberships(&pool, &failed_batch).await.is_err());
            assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM memberships WHERE helloasso_item_id = 21").fetch_one(&pool).await?, 0);
            Ok::<_, anyhow::Error>(())
        }.await;
        sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
            .execute(&pool)
            .await?;
        pool.close().await;
        result
    }
}
