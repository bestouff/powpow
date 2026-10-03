use chrono::{DateTime, Datelike, Utc};
use sqlx::{FromRow, PgConnection, PgPool, Row};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

use crate::{
    auto_import::{Identity, Source},
    manual_import::{
        Action, Candidate, Context, HistoryEntry, ImportRequest, StaffFields, matches,
    },
    models::{Cash, Membership},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Adhésion ou staff introuvable.")]
    NotFound,
    #[error("Ce paiement n'est pas une adhésion.")]
    NotMembership,
    #[error("Cette adhésion a déjà été importée. Rechargez la liste.")]
    AlreadyImported,
    #[error(
        "La fiche a changé depuis sa sélection. Recherchez et sélectionnez le staff à nouveau."
    )]
    StaleStaff,
    #[error(
        "Une adhésion existe déjà pour cette saison. Confirmez explicitement le doublon si vous souhaitez le conserver."
    )]
    DuplicateMembership,
    #[error(
        "Un autre staff utilise déjà ce prénom et ce nom : {name} ({id}). Sélectionnez cette fiche ou corrigez l'identité."
    )]
    IdentityConflict { id: Uuid, name: String },
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

fn ids(source: Source) -> (Option<i64>, Option<Uuid>) {
    match source {
        Source::HelloAsso(id) => (Some(id), None),
        Source::Cash(id) => (None, Some(id)),
    }
}

pub async fn context(connection: &mut PgConnection, source: Source) -> Result<Context, Error> {
    let mut warnings = vec![];
    let (season, fields, payer_email, details) = match source {
        Source::HelloAsso(id) => {
            let row = sqlx::query("SELECT m.*, to_jsonb(m)->>'source_state' AS provider_state FROM memberships m WHERE helloasso_item_id = $1")
                .bind(id).fetch_optional(&mut *connection).await?.ok_or(Error::NotFound)?;
            let membership = Membership::from_row(&row)?;
            if membership
                .item_type
                .as_deref()
                .is_some_and(|kind| kind != "Membership")
            {
                return Err(Error::NotMembership);
            }
            let state: Option<String> = row.get("provider_state");
            if let Some(state) = &state
                && state != "Processed"
            {
                warnings.push(format!(
                    "Statut HelloAsso : {state}. Vérifiez que l'adhésion doit être accordée."
                ));
            }
            if membership.order_date.is_none() {
                warnings
                    .push("Date inconnue : l'adhésion sera rattachée à la saison actuelle.".into());
            }
            if membership.amount.is_none_or(|amount| amount <= 0) {
                warnings.push(
                    "Montant absent, nul ou négatif : vérifiez le paiement avant de confirmer."
                        .into(),
                );
            }
            let season = membership
                .order_date
                .map_or_else(crate::get_current_season, crate::get_season_for);
            let payer = membership.payer_email.unwrap_or_default();
            let primary = membership.email.unwrap_or_default();
            let fields = StaffFields {
                first_name: membership.beneficiary_first_name.unwrap_or_default(),
                last_name: membership.beneficiary_last_name.unwrap_or_default(),
                email: if primary.trim().is_empty() {
                    payer.clone()
                } else {
                    primary.clone()
                },
                phone: membership.phone.unwrap_or_default(),
                comment: membership.comment.unwrap_or_default(),
            };
            let details = vec![
                (
                    "Bénéficiaire",
                    format!("{} {}", fields.first_name, fields.last_name),
                ),
                ("Email bénéficiaire", primary),
                ("Email payeur", payer.clone()),
                ("Téléphone", fields.phone.clone()),
                ("Article", membership.item_name.unwrap_or_default()),
                (
                    "Montant",
                    membership.amount.map_or_else(
                        || "Inconnu".into(),
                        |amount| format!("{:.2} €", f64::from(amount) / 100.0),
                    ),
                ),
                (
                    "Date",
                    membership.order_date.map_or_else(
                        || "Inconnue".into(),
                        |date| date.format("%d/%m/%Y").to_string(),
                    ),
                ),
                ("Statut", state.unwrap_or_else(|| "Inconnu".into())),
                ("Identifiant HelloAsso", id.to_string()),
                ("Commentaire", fields.comment.clone()),
            ];
            (season, fields, payer, details)
        }
        Source::Cash(id) => {
            let cash = sqlx::query_as::<_, Cash>("SELECT * FROM cash WHERE id = $1")
                .bind(id)
                .fetch_optional(&mut *connection)
                .await?
                .ok_or(Error::NotFound)?;
            if !cash.is_membership {
                return Err(Error::NotMembership);
            }
            if cash.amount <= 0 {
                warnings.push(
                    "Montant nul ou négatif : vérifiez le paiement avant de confirmer.".into(),
                );
            }
            let season = (cash.date.year() + i32::from(cash.date.month() >= 6)) as i16;
            let fields = StaffFields {
                first_name: cash.first_name,
                last_name: cash.last_name,
                email: cash.email.unwrap_or_default(),
                phone: cash.phone.unwrap_or_default(),
                comment: String::new(),
            };
            let details = vec![
                (
                    "Bénéficiaire",
                    format!("{} {}", fields.first_name, fields.last_name),
                ),
                ("Email", fields.email.clone()),
                ("Téléphone", fields.phone.clone()),
                (
                    "Moyen de paiement",
                    if cash.payment_method == "check" {
                        "Chèque"
                    } else {
                        "Espèces"
                    }
                    .into(),
                ),
                ("Montant", format!("{} €", cash.amount)),
                ("Date", cash.date.format("%d/%m/%Y").to_string()),
                ("Identifiant", id.to_string()),
            ];
            (season, fields, String::new(), details)
        }
    };
    let (online, cash) = ids(source);
    if let Some(reason) = sqlx::query_scalar::<_, String>(
        "SELECT reason FROM auto_import_reviews WHERE helloasso_item_id = $1 OR cash_id = $2",
    )
    .bind(online)
    .bind(cash)
    .fetch_optional(&mut *connection)
    .await?
    {
        warnings.push(reason);
    }
    let imported_to = sqlx::query_scalar::<_, Uuid>(
        "SELECT staff FROM payments WHERE helloasso_item_id = $1 OR cash_id = $2",
    )
    .bind(online)
    .bind(cash)
    .fetch_optional(connection)
    .await?;
    Ok(Context {
        source,
        season,
        fields,
        payer_email,
        details,
        warnings,
        imported_to,
    })
}

fn fields(row: &sqlx::postgres::PgRow) -> StaffFields {
    StaffFields {
        first_name: row.get("first_name"),
        last_name: row.get("last_name"),
        email: row.get("email"),
        phone: row.get::<Option<String>, _>("phone").unwrap_or_default(),
        comment: row.get("comment"),
    }
}

pub async fn search(
    pool: &PgPool,
    context: &Context,
    query: &str,
) -> Result<Vec<Candidate>, Error> {
    if query.len() > 200 {
        return Err(Error::Invalid("La recherche est trop longue.".into()));
    }
    let rows = sqlx::query(
        r"SELECT s.id, s.first_name, s.last_name, s.email, s.phone, s.comment, s.updated_at,
            MAX(p.season) AS latest_season,
            MAX(CASE WHEN m.item_type = 'Membership' OR (m.item_type IS NULL AND p.helloasso_item_id IS NOT NULL) OR c.is_membership THEN COALESCE(m.order_date, c.date::timestamp AT TIME ZONE 'UTC') END) AS last_membership_date,
            COALESCE(BOOL_OR(p.season = $1), false) AS paid_for_season
          FROM staff s LEFT JOIN payments p ON p.staff = s.id
          LEFT JOIN memberships m ON m.helloasso_item_id = p.helloasso_item_id
          LEFT JOIN cash c ON c.id = p.cash_id
          GROUP BY s.id ORDER BY s.last_name, s.first_name, s.id",
    ).bind(context.season).fetch_all(pool).await?;
    let aliases = sqlx::query(
        "SELECT p.staff, COALESCE(m.beneficiary_first_name, c.first_name, '') AS first_name,
           COALESCE(m.beneficiary_last_name, c.last_name, '') AS last_name,
           COALESCE(NULLIF(TRIM(m.email), ''), m.payer_email, c.email, '') AS email,
           COALESCE(m.phone, c.phone, '') AS phone,
           COALESCE(m.order_date, c.date::timestamp AT TIME ZONE 'UTC') AS date,
           p.season,
           CASE WHEN p.helloasso_item_id IS NOT NULL THEN 'HelloAsso'
                WHEN c.payment_method = 'check' THEN 'Chèque' ELSE 'Espèces' END AS method
         FROM payments p LEFT JOIN memberships m USING (helloasso_item_id)
         LEFT JOIN cash c ON c.id = p.cash_id
         WHERE m.item_type = 'Membership' OR (m.item_type IS NULL AND p.helloasso_item_id IS NOT NULL) OR c.is_membership
         ORDER BY date DESC NULLS LAST, p.season DESC, p.id",
    )
    .fetch_all(pool)
    .await?;
    let mut history: BTreeMap<Uuid, Vec<HistoryEntry>> = BTreeMap::new();
    for row in aliases {
        history
            .entry(row.get("staff"))
            .or_default()
            .push(HistoryEntry {
                identity: Identity {
                    first_name: row.get("first_name"),
                    last_name: row.get("last_name"),
                    email: row.get("email"),
                    phone: row.get("phone"),
                },
                date: row.get("date"),
                season: row.get("season"),
                method: row.get("method"),
            });
    }
    let mut candidates = vec![];
    for row in rows {
        let id = row.get("id");
        let fields = fields(&row);
        let current = Identity {
            first_name: fields.first_name.clone(),
            last_name: fields.last_name.clone(),
            email: fields.email.clone(),
            phone: fields.phone.clone(),
        };
        let entries = history.remove(&id).unwrap_or_default();
        let historical = entries
            .iter()
            .find(|entry| matches(&entry.identity, context, query))
            .map(|entry| &entry.identity);
        if !matches(&current, context, query) && historical.is_none() {
            continue;
        }
        candidates.push(Candidate {
            id,
            fields,
            updated_at: row.get("updated_at"),
            last_membership_date: row.get("last_membership_date"),
            latest_season: row.get("latest_season"),
            paid_for_season: row.get("paid_for_season"),
            historical_name: historical
                .filter(|old| {
                    old.first_name != current.first_name || old.last_name != current.last_name
                })
                .map(|old| format!("{} {}", old.first_name, old.last_name)),
            history: entries,
        });
    }
    Ok(candidates)
}

pub async fn submit(
    pool: &PgPool,
    source: Source,
    mut request: ImportRequest,
    admin: &crate::models::Staff,
) -> Result<Uuid, Error> {
    request.fields.validate().map_err(Error::Invalid)?;
    if (request.action == Action::Create
        && (request.staff_id.is_some() || request.expected_updated_at.is_some()))
        || (request.action == Action::Update
            && (request.staff_id.is_none() || request.expected_updated_at.is_none()))
    {
        return Err(Error::Invalid(
            "Sélectionnez un staff existant ou créez un nouvel adhérent.".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    super::auto_import::lock_import(&mut tx).await?;
    let context = context(&mut tx, source).await?;
    let (online, cash) = ids(source);
    let imported: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM payments WHERE helloasso_item_id = $1 OR cash_id = $2)",
    )
    .bind(online)
    .bind(cash)
    .fetch_one(&mut *tx)
    .await?;
    if imported {
        return Err(Error::AlreadyImported);
    }
    let old = if let Some(id) = request.staff_id {
        let row = sqlx::query("SELECT id, first_name, last_name, email, phone, comment, updated_at FROM staff WHERE id = $1 FOR UPDATE").bind(id).fetch_optional(&mut *tx).await?.ok_or(Error::NotFound)?;
        if Some(row.get::<DateTime<Utc>, _>("updated_at")) != request.expected_updated_at {
            return Err(Error::StaleStaff);
        }
        Some(fields(&row))
    } else {
        None
    };
    for row in sqlx::query("SELECT id, first_name, last_name, email, phone, comment FROM staff")
        .fetch_all(&mut *tx)
        .await?
    {
        let id: Uuid = row.get("id");
        if Some(id) != request.staff_id
            && request.fields.same_name(&fields(&row))
            && old.as_ref() != Some(&request.fields)
        {
            return Err(Error::IdentityConflict {
                id,
                name: format!(
                    "{} {}",
                    row.get::<String, _>("first_name"),
                    row.get::<String, _>("last_name")
                ),
            });
        }
    }
    // A wrong earlier rename must not make the original identity available for
    // creating a second person. Ambiguous household source names are not claims.
    let aliases = sqlx::query(
        "SELECT p.staff, COALESCE(m.beneficiary_first_name, c.first_name, '') AS first_name,
            COALESCE(m.beneficiary_last_name, c.last_name, '') AS last_name
         FROM payments p LEFT JOIN memberships m USING (helloasso_item_id)
         LEFT JOIN cash c ON c.id = p.cash_id
         WHERE m.item_type = 'Membership' OR (m.item_type IS NULL AND p.helloasso_item_id IS NOT NULL) OR c.is_membership",
    ).fetch_all(&mut *tx).await?;
    let first = crate::auto_import::normalize_name(&request.fields.first_name);
    let last = crate::auto_import::normalize_name(&request.fields.last_name);
    let owners: BTreeSet<Uuid> = aliases
        .iter()
        .filter(|row| {
            crate::auto_import::normalize_name(row.get("first_name")) == first
                && crate::auto_import::normalize_name(row.get("last_name")) == last
        })
        .map(|row| row.get("staff"))
        .collect();
    if owners.len() == 1 && request.staff_id != owners.first().copied() {
        return Err(Error::IdentityConflict {
            id: *owners.first().unwrap(),
            name: format!(
                "ancienne adhésion de {} {}",
                request.fields.first_name, request.fields.last_name
            ),
        });
    }
    if let Some(id) = request.staff_id {
        let duplicate: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM payments WHERE staff = $1 AND season = $2)",
        )
        .bind(id)
        .bind(context.season)
        .fetch_one(&mut *tx)
        .await?;
        if duplicate && !request.allow_duplicate {
            return Err(Error::DuplicateMembership);
        }
    }
    let value = &request.fields;
    let staff_id = if let Some(id) = request.staff_id {
        if old.as_ref() != Some(value) {
            sqlx::query("UPDATE staff SET first_name = $2, last_name = $3, email = $4, phone = NULLIF($5, ''), comment = $6, updated_at = clock_timestamp() WHERE id = $1")
                .bind(id).bind(&value.first_name).bind(&value.last_name).bind(&value.email).bind(&value.phone).bind(&value.comment).execute(&mut *tx).await?;
        }
        id
    } else {
        sqlx::query_scalar::<_, Uuid>("INSERT INTO staff (first_name, last_name, email, phone, comment) VALUES ($1, $2, $3, NULLIF($4, ''), $5) RETURNING id")
            .bind(&value.first_name).bind(&value.last_name).bind(&value.email).bind(&value.phone).bind(&value.comment).fetch_one(&mut *tx).await?
    };
    sqlx::query(
        "INSERT INTO payments (season, staff, helloasso_item_id, cash_id) VALUES ($1, $2, $3, $4)",
    )
    .bind(context.season)
    .bind(staff_id)
    .bind(online)
    .bind(cash)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM auto_import_reviews WHERE helloasso_item_id = $1 OR cash_id = $2")
        .bind(online)
        .bind(cash)
        .execute(&mut *tx)
        .await?;
    // Record both versions so future corrections do not depend on guesswork.
    let detail = serde_json::json!({ "source": source.label(), "staff": staff_id, "season": context.season, "before": old, "after": value, "duplicate_confirmed": request.allow_duplicate }).to_string();
    sqlx::query("INSERT INTO audit (staff_id, staff_name, operation, detail) VALUES ($1, $2, 'Import manuel adhésion', $3)")
        .bind(admin.id).bind(format!("{} {}", admin.first_name, admin.last_name)).bind(detail).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(staff_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(staff: Option<&crate::models::Staff>, values: StaffFields) -> ImportRequest {
        ImportRequest {
            action: if staff.is_some() {
                Action::Update
            } else {
                Action::Create
            },
            staff_id: staff.map(|staff| staff.id),
            expected_updated_at: staff.map(|staff| staff.updated_at),
            fields: values,
            allow_duplicate: false,
        }
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
    async fn manual_import_search_conflicts_updates_and_concurrency() -> anyhow::Result<()> {
        let schema = format!("manual_import_test_{}", Uuid::new_v4().simple());
        let connection_schema = schema.clone();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(3)
            .after_connect(move |connection, _| {
                let schema = connection_schema.clone();
                Box::pin(async move {
                    sqlx::query("SELECT set_config('search_path', $1, false)")
                        .bind(schema)
                        .execute(connection)
                        .await?;
                    Ok(())
                })
            })
            .connect(&std::env::var("DATABASE_URL")?)
            .await?;
        // The schema identifier is a fixed prefix plus generated hexadecimal digits.
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
            .execute(&pool)
            .await?;
        let result = async {
            sqlx::raw_sql(
                "CREATE TABLE staff (id uuid PRIMARY KEY DEFAULT gen_random_uuid(), first_name text NOT NULL, last_name text NOT NULL, email text NOT NULL, phone text, comment text NOT NULL DEFAULT '', is_admin bool NOT NULL DEFAULT false, is_god bool NOT NULL DEFAULT false, no_import_emails bool NOT NULL DEFAULT false, no_weekly_emails bool NOT NULL DEFAULT false, newsletter_status text NOT NULL DEFAULT 'subscribed', created_at timestamptz NOT NULL DEFAULT clock_timestamp(), updated_at timestamptz NOT NULL DEFAULT clock_timestamp());
                 CREATE TABLE memberships (helloasso_order_id bigint, helloasso_item_id bigint PRIMARY KEY, payer_email text, beneficiary_first_name text, beneficiary_last_name text, phone text, email text, item_name text, item_type text, tier_name text, amount integer, order_date timestamptz, comment text, created_at timestamptz DEFAULT now(), updated_at timestamptz DEFAULT now());
                 CREATE TABLE cash (id uuid PRIMARY KEY, first_name text, last_name text, email text, phone text, date date, amount integer, is_membership bool, payment_method text DEFAULT 'cash');
                 CREATE TABLE payments (id uuid PRIMARY KEY DEFAULT gen_random_uuid(), season smallint, staff uuid REFERENCES staff(id), helloasso_item_id bigint UNIQUE REFERENCES memberships(helloasso_item_id), cash_id uuid UNIQUE REFERENCES cash(id), CHECK ((helloasso_item_id IS NOT NULL) <> (cash_id IS NOT NULL)));
                 CREATE TABLE audit (staff_id uuid, staff_name text, operation text, detail text);",
            ).execute(&pool).await?;
            sqlx::raw_sql(include_str!("../../migrations/043_auto_import_reviews.sql")).execute(&pool).await?;
            let admin = sqlx::query_as::<_, crate::models::Staff>("INSERT INTO staff (first_name, last_name, email, is_admin) VALUES ('Admin', 'Tester', 'admin@example.org', true) RETURNING *").fetch_one(&pool).await?;
            let child = sqlx::query_as::<_, crate::models::Staff>("INSERT INTO staff (first_name,last_name,email,phone,comment,is_admin,is_god) VALUES ('Anne Celine','Thiery','family@example.org','0612345678','Keep this note',true,true) RETURNING *").fetch_one(&pool).await?;
            let mother = sqlx::query_as::<_, crate::models::Staff>("INSERT INTO staff (first_name,last_name,email,phone,comment) VALUES ('Anneceline','Thiery','family@example.org','0612345678','Mother note') RETURNING *").fetch_one(&pool).await?;
            sqlx::raw_sql(
                "INSERT INTO memberships (helloasso_order_id,helloasso_item_id,payer_email,beneficiary_first_name,beneficiary_last_name,phone,email,item_type,amount,order_date,source_state) VALUES
                 (1,1,'family@example.org','Titouan','Lakama','0612345678','family@example.org','Membership',2000,'2025-12-14 12:00:00+00','Processed'),
                 (2,2,'family@example.org','Anneceline','Thiery','0612345678','family@example.org','Membership',2000,'2026-02-08 12:00:00+00','Processed'),
                 (3,3,'family@example.org','Titouan','Lakama','0612345678','family@example.org','Membership',2000,'2026-10-01 12:00:00+00','Processed'),
                 (4,4,'family@example.org','Anne Celine','Thiery','0612345678','family@example.org','Membership',2000,'2026-10-02 12:00:00+00','Processed');
                 INSERT INTO cash (id,first_name,last_name,email,date,amount,is_membership) SELECT ('00000000-0000-0000-0000-' || lpad(n::text,12,'0'))::uuid,'Payment','Source','family@example.org','2026-10-02',20,true FROM generate_series(5,9) AS n;",
            ).execute(&pool).await?;
            sqlx::query("INSERT INTO payments (staff,season,helloasso_item_id) VALUES ($1,2026,1),($2,2026,2)").bind(child.id).bind(mother.id).execute(&pool).await?;
            let ctx = context(&mut *pool.acquire().await?, Source::HelloAsso(3)).await?;
            assert_eq!(ctx.season, 2027);
            assert!(matches!(submit(&pool, Source::HelloAsso(3), request(None, ctx.fields.clone()), &admin).await, Err(Error::IdentityConflict { id, .. }) if id == child.id));
            let defaults = search(&pool, &ctx, "").await?;
            assert!(defaults.iter().any(|candidate| candidate.id == child.id));
            assert!(defaults.iter().any(|candidate| candidate.id == mother.id));
            let historical = search(&pool, &ctx, "Titouan Lakama").await?;
            assert_eq!(historical.len(), 1);
            assert_eq!(historical[0].id, child.id);
            assert_eq!(historical[0].historical_name.as_deref(), Some("Titouan Lakama"));
            assert_eq!(historical[0].history.len(), 1);
            assert_eq!(historical[0].history[0].identity.first_name, "Titouan");
            assert_eq!(historical[0].history[0].identity.email, "family@example.org");
            assert_eq!(historical[0].history[0].identity.phone, "0612345678");
            assert_eq!(historical[0].history[0].method, "HelloAsso");
            assert_eq!(historical[0].last_membership_date.unwrap().date_naive(), chrono::NaiveDate::from_ymd_opt(2025,12,14).unwrap());
            for query in ["@example.org", "+3361234", "061234", "anne thierri"] { assert!(search(&pool, &ctx, query).await?.iter().any(|candidate| candidate.id == mother.id)); }
            // Existing legacy name collisions may be linked without changing fields.
            submit(&pool, Source::HelloAsso(4), request(Some(&mother), StaffFields::from(&mother)), &admin).await?;
            // Explicit edits repair the wrongly renamed child and can clear fields.
            let repaired = StaffFields { first_name: "Titouan".into(), last_name: "Lakama".into(), email: "family@example.org".into(), phone: String::new(), comment: String::new() };
            submit(&pool, Source::HelloAsso(3), request(Some(&child), repaired.clone()), &admin).await?;
            let child = crate::database::get_staff_by_id(&pool, child.id).await?.unwrap();
            assert_eq!(child.first_name, "Titouan");
            assert_eq!(child.phone, None);
            assert_eq!(child.comment, "");
            assert!(child.is_admin && child.is_god);
            let cash = |n| Source::Cash(Uuid::parse_str(&format!("00000000-0000-0000-0000-{n:012}")).unwrap());
            assert!(matches!(submit(&pool, cash(5), request(Some(&child), StaffFields::from(&mother)), &admin).await, Err(Error::IdentityConflict { .. })));
            let duplicate_name = StaffFields { first_name: " ANNE CÉLINE ".into(), last_name: "THIERY".into(), email: "other@example.org".into(), ..Default::default() };
            assert!(matches!(submit(&pool, cash(5), request(None, duplicate_name), &admin).await, Err(Error::IdentityConflict { .. })));
            let bob = StaffFields { first_name: "Bob".into(), last_name: "Perrin".into(), email: "family@example.org".into(), ..Default::default() };
            let bob_id = submit(&pool, cash(5), request(None, bob.clone()), &admin).await?;
            let bob = crate::database::get_staff_by_id(&pool, bob_id).await?.unwrap();
            assert!(!bob.is_admin && !bob.is_god);
            let mut second = request(Some(&bob), StaffFields { comment: "Do not save yet".into(), ..StaffFields::from(&bob) });
            assert!(matches!(submit(&pool, cash(6), request(Some(&bob), second.fields.clone()), &admin).await, Err(Error::DuplicateMembership)));
            assert_eq!(crate::database::get_staff_by_id(&pool,bob.id).await?.unwrap().comment, "");
            second.allow_duplicate = true;
            submit(&pool, cash(6), second, &admin).await?;
            assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM payments WHERE staff = $1 AND season = 2027").bind(bob.id).fetch_one(&pool).await?, 2);
            let selected = crate::database::get_staff_by_id(&pool,bob.id).await?.unwrap();
            sqlx::query("UPDATE staff SET phone = '0622334455', updated_at = clock_timestamp() WHERE id = $1").bind(bob.id).execute(&pool).await?;
            let mut stale = request(Some(&selected), StaffFields::from(&selected)); stale.allow_duplicate = true;
            assert!(matches!(submit(&pool, cash(7), stale, &admin).await, Err(Error::StaleStaff)));
            assert_eq!(crate::database::get_staff_by_id(&pool,bob.id).await?.unwrap().phone.as_deref(), Some("0622334455"));
            assert!(matches!(submit(&pool, cash(5), request(None, StaffFields { first_name: "Other".into(), last_name: "Person".into(), ..Default::default() }), &admin).await, Err(Error::AlreadyImported)));
            let zoe = StaffFields { first_name: "Zoé".into(), last_name: "Dupont".into(), email: "zoe@example.org".into(), ..Default::default() };
            let (first, second) = tokio::join!(submit(&pool,cash(8),request(None,zoe.clone()),&admin),submit(&pool,cash(9),request(None,zoe),&admin));
            assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
            assert!(matches!(first, Err(Error::IdentityConflict { .. })) || matches!(second, Err(Error::IdentityConflict { .. })));
            let audit: String = sqlx::query_scalar("SELECT detail FROM audit WHERE detail::jsonb->>'source' = 'helloasso:3'").fetch_one(&pool).await?;
            let audit: serde_json::Value = serde_json::from_str(&audit)?;
            assert_eq!(audit["before"]["first_name"], "Anne Celine");
            assert_eq!(audit["after"]["first_name"], "Titouan");
            Ok::<_, anyhow::Error>(())
        }.await;
        sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
            .execute(&pool)
            .await?;
        pool.close().await;
        result
    }
}
