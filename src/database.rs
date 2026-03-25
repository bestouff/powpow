use crate::models::{
    self, Atelier, Cash, ContentBlock, ContentImage, Equipment, Membership, Need,
    PaymentHistoryEntry, Photo, PhotoMeta, Qualification, Role, Staff, StaffMatchType, StaffQualif,
    StaffWithSeason, User,
};
use anyhow::Result;
use futures_util::StreamExt;
use sqlx::PgPool;
use sqlx::Row;
use tracing::info;

/// Remove common French accents from a string for comparison
fn strip_accents(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' | 'À' | 'Â' | 'Ä' | 'Á' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'î' | 'ï' | 'í' | 'ì' | 'Î' | 'Ï' | 'Í' | 'Ì' => 'i',
            'ô' | 'ö' | 'ó' | 'ò' | 'Ô' | 'Ö' | 'Ó' | 'Ò' => 'o',
            'ù' | 'û' | 'ü' | 'ú' | 'Ù' | 'Û' | 'Ü' | 'Ú' => 'u',
            'ÿ' | 'ý' | 'Ÿ' | 'Ý' => 'y',
            'ç' | 'Ç' => 'c',
            'ñ' | 'Ñ' => 'n',
            _ => c,
        })
        .collect()
}

pub async fn setup_database(database_url: &str) -> Result<PgPool> {
    // Redact password from URL before logging (postgres://user:PASSWORD@host/db)
    let redacted = if let Some(at_pos) = database_url.find('@') {
        let prefix = &database_url[..at_pos];
        if let Some(colon_pos) = prefix.rfind(':') {
            format!(
                "{}:****{}",
                &database_url[..colon_pos],
                &database_url[at_pos..]
            )
        } else {
            database_url.to_string()
        }
    } else {
        database_url.to_string()
    };
    info!("Connecting to database: {}", redacted);
    let pool = PgPool::connect(database_url).await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &PgPool) -> Result<i64> {
    info!("Running database migrations");

    // Count existing migrations before running
    let before: i64 =
        sqlx::query_scalar(r"SELECT COUNT(*) FROM _sqlx_migrations WHERE success = true")
            .fetch_one(pool)
            .await
            .unwrap_or(0);

    sqlx::migrate!("./migrations").run(pool).await?;

    // Count after
    let after: i64 =
        sqlx::query_scalar(r"SELECT COUNT(*) FROM _sqlx_migrations WHERE success = true")
            .fetch_one(pool)
            .await
            .unwrap_or(0);

    let applied = after - before;
    if applied > 0 {
        info!("Applied {applied} new migration(s) (total: {after})");
    } else {
        info!("Database already up to date ({after} migrations)");
    }

    Ok(applied)
}

pub async fn upsert_user(pool: &PgPool, user: &User) -> Result<User> {
    let result = sqlx::query_as::<_, User>(
        r"
        INSERT INTO users (
            email, first_name, last_name, phone,
            address, city, zip_code, country, birth_date, created_at, updated_at, last_sync_at
        )
        VALUES ($1.email, $1.first_name, $1.last_name, $1.phone, $1.address, $1.city, $1.zip_code, $1.country, $1.birth_date, $1.created_at, $1.updated_at, $1.last_sync_at)
        ON CONFLICT (email)
        DO UPDATE SET
            first_name = EXCLUDED.first_name,
            last_name = EXCLUDED.last_name,
            phone = EXCLUDED.phone,
            address = EXCLUDED.address,
            city = EXCLUDED.city,
            zip_code = EXCLUDED.zip_code,
            country = EXCLUDED.country,
            birth_date = EXCLUDED.birth_date,
            updated_at = EXCLUDED.updated_at,
            last_sync_at = EXCLUDED.last_sync_at
        RETURNING *
        ",
    )
    .bind(user)
    .fetch_one(pool)
    .await?;

    Ok(result)
}

pub async fn get_users_paginated(pool: &PgPool, limit: i64, offset: i64) -> Result<Vec<User>> {
    let users = sqlx::query_as::<_, User>(
        r"
        SELECT email, first_name, last_name, phone, address, city, zip_code, country,
               birth_date, created_at, updated_at, last_sync_at
        FROM users
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        ",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(users)
}

pub async fn get_user_by_email(pool: &PgPool, email: String) -> Result<Option<User>> {
    let user = sqlx::query_as::<_, User>(
        r"
        SELECT email, first_name, last_name, phone, address, city, zip_code, country,
               birth_date, created_at, updated_at, last_sync_at
        FROM users
        WHERE email = $1
        ",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    Ok(user)
}

pub async fn count_users(pool: &PgPool) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) as count FROM users")
        .fetch_one(pool)
        .await?;

    let count: i64 = row.try_get("count")?;
    Ok(count)
}

#[allow(dead_code)]
pub async fn get_users_by_email(pool: &PgPool, email: &str) -> Result<Vec<User>> {
    let pattern = format!("%{}%", email);
    let users = sqlx::query_as::<_, User>(
        r"
        SELECT email, first_name, last_name, phone, address, city, zip_code, country,
               birth_date, created_at, updated_at, last_sync_at
        FROM users
        WHERE email ILIKE $1
        ORDER BY created_at DESC
        ",
    )
    .bind(pattern)
    .fetch_all(pool)
    .await?;

    Ok(users)
}

#[allow(dead_code)]
pub async fn delete_user(pool: &PgPool, email: &str) -> Result<bool> {
    let result = sqlx::query("DELETE FROM users WHERE email = $1")
        .bind(email)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

#[allow(dead_code)]
pub async fn get_recent_synced_users(pool: &PgPool, limit: i64) -> Result<Vec<User>> {
    let users = sqlx::query_as::<_, User>(
        r"
        SELECT * FROM users
        WHERE last_sync_at IS NOT NULL
        ORDER BY last_sync_at DESC
        LIMIT $1
        ",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(users)
}

pub async fn get_recently_synced_users(pool: &PgPool, hours: i64) -> Result<i64> {
    let row = sqlx::query(
        r"
        SELECT COUNT(*) as count FROM users
        WHERE last_sync_at >= NOW() - ($1 * INTERVAL '1 hour')
        ",
    )
    .bind(hours)
    .fetch_one(pool)
    .await?;

    let count: i64 = row.try_get("count")?;
    Ok(count)
}

// Get users with their associated memberships
#[allow(dead_code)]
pub async fn get_users_with_memberships(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<(User, Vec<Membership>)>> {
    let users: Vec<User> = sqlx::query_as::<_, User>(
        r"
        SELECT email, first_name, last_name, phone, address, city, zip_code, country,
               birth_date, created_at, updated_at, last_sync_at
        FROM users
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        ",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    // For each user, get their associated memberships
    let mut result = Vec::new();
    for user in users {
        let memberships = sqlx::query_as::<_, Membership>(
            r"
            SELECT * FROM memberships
            WHERE payer_email = $1 OR email = $1
            ORDER BY order_date DESC
            ",
        )
        .bind(&user.email)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        result.push((user, memberships));
    }

    Ok(result)
}

// Membership functions
pub async fn upsert_membership(pool: &PgPool, membership: &Membership) -> Result<Membership> {
    let result = sqlx::query_as::<_, Membership>(
        r"
        INSERT INTO memberships (
            helloasso_order_id, helloasso_item_id, payer_email,
            beneficiary_first_name, beneficiary_last_name, phone, email,
            item_name, item_type, tier_name, amount, order_date, comment,
            created_at, updated_at
        )
        VALUES ($1.helloasso_order_id, $1.helloasso_item_id, $1.payer_email, $1.beneficiary_first_name, $1.beneficiary_last_name, $1.phone, $1.email, $1.item_name, $1.item_type, $1.tier_name, $1.amount, $1.order_date, $1.comment, $1.created_at, $1.updated_at)
        ON CONFLICT (helloasso_item_id)
        DO UPDATE SET
            payer_email = EXCLUDED.payer_email,
            beneficiary_first_name = EXCLUDED.beneficiary_first_name,
            beneficiary_last_name = EXCLUDED.beneficiary_last_name,
            phone = EXCLUDED.phone,
            email = EXCLUDED.email,
            item_name = EXCLUDED.item_name,
            item_type = EXCLUDED.item_type,
            tier_name = EXCLUDED.tier_name,
            amount = EXCLUDED.amount,
            order_date = EXCLUDED.order_date,
            comment = EXCLUDED.comment,
            updated_at = EXCLUDED.updated_at
        RETURNING *
        ",
    )
    .bind(membership)
    .fetch_one(pool)
    .await?;

    Ok(result)
}

#[allow(dead_code)]
pub async fn get_memberships_paginated(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<Membership>> {
    let memberships = sqlx::query_as::<_, Membership>(
        r"
        SELECT * FROM memberships
        ORDER BY order_date DESC
        LIMIT $1 OFFSET $2
        ",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(memberships)
}

#[allow(dead_code)]
pub async fn count_memberships(pool: &PgPool) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) as count FROM memberships")
        .fetch_one(pool)
        .await?;

    let count: i64 = row.try_get("count")?;
    Ok(count)
}

// Check if a staff/payment exists for a given membership order and season
pub async fn has_staff_for_membership(
    pool: &PgPool,
    helloasso_item_id: i64,
    season: i16,
) -> Result<bool> {
    let row = sqlx::query(
        r"
        SELECT EXISTS(
            SELECT 1 FROM payments
            WHERE helloasso_item_id = $1 AND season = $2
        ) as exists
        ",
    )
    .bind(helloasso_item_id)
    .bind(season)
    .fetch_one(pool)
    .await?;

    let exists: bool = row.try_get("exists")?;
    Ok(exists)
}

/// Returns the set of all (`helloasso_item_id`, season) pairs from the payments table.
/// Used to batch-check which memberships have already been imported, avoiding N+1 queries.
pub async fn get_all_imported_item_ids(
    pool: &PgPool,
) -> Result<std::collections::HashSet<(i64, i16)>> {
    let rows = sqlx::query(
        r"
        SELECT helloasso_item_id, season FROM payments
        WHERE helloasso_item_id IS NOT NULL
        ",
    )
    .fetch_all(pool)
    .await?;

    let set = rows
        .iter()
        .map(|row| {
            let item_id: i64 = row.get("helloasso_item_id");
            let season: i16 = row.get("season");
            (item_id, season)
        })
        .collect();

    Ok(set)
}

pub async fn get_membership_by_item_id(
    pool: &PgPool,
    helloasso_item_id: i64,
) -> Result<Option<Membership>> {
    let membership = sqlx::query_as::<_, Membership>(
        r"
        SELECT * FROM memberships
        WHERE helloasso_item_id = $1
        ",
    )
    .bind(helloasso_item_id)
    .fetch_optional(pool)
    .await?;

    Ok(membership)
}

// Staff functions

/// Find staff candidates for a membership, ordered by match quality
/// Priority: exact matches first, then double subscriptions (exact name but already paid),
/// then fuzzy matches last
pub async fn find_staff_candidates(
    pool: &PgPool,
    membership_email: &str,
    payer_email: &str,
    first_name: &str,
    last_name: &str,
    season: i16,
) -> Result<Vec<StaffWithSeason>> {
    let mut candidates = Vec::new();

    // Normalize inputs: trim whitespace and lowercase
    let membership_email_norm = membership_email.trim().to_lowercase();
    let payer_email_norm = payer_email.trim().to_lowercase();
    let first_name_norm = first_name.trim().to_lowercase();
    let last_name_norm = last_name.trim().to_lowercase();

    // Use membership email if available, otherwise payer email for primary search
    let email_norm = if membership_email_norm.is_empty() {
        payer_email_norm.clone()
    } else {
        membership_email_norm.clone()
    };

    // Collect all emails to search with their source (is_payer_only)
    // is_payer_only = true means the match is via payer email when beneficiary email differs
    let mut emails_to_search: Vec<(String, bool)> = vec![(membership_email_norm.clone(), false)];
    if !payer_email_norm.is_empty() && payer_email_norm != membership_email_norm {
        // Payer email is different from membership email - mark as payer-only match
        emails_to_search.push((payer_email_norm.clone(), true));
    }

    // 1. Exact both match (highest priority - email AND name match, no payment yet)
    // Use unaccent for accent-insensitive name matching (é = e, etc.)
    for (search_email, _is_payer_only) in &emails_to_search {
        if search_email.is_empty() {
            continue;
        }
        let exact_both_matches = sqlx::query_as::<_, Staff>(
            r"
            SELECT s.* FROM staff s
            WHERE LOWER(TRIM(s.email)) = $1
            AND unaccent(LOWER(TRIM(s.first_name))) = unaccent($2)
            AND unaccent(LOWER(TRIM(s.last_name))) = unaccent($3)
            AND NOT EXISTS (
                SELECT 1 FROM payments p WHERE p.staff = s.id AND p.season = $4
            )
            ",
        )
        .bind(search_email)
        .bind(&first_name_norm)
        .bind(&last_name_norm)
        .bind(season)
        .fetch_all(pool)
        .await?;

        for staff in exact_both_matches {
            if candidates
                .iter()
                .any(|c: &StaffWithSeason| c.staff.id == staff.id)
            {
                continue;
            }
            let latest_season = get_staff_latest_season(pool, staff.id).await?;
            candidates.push(StaffWithSeason {
                staff,
                latest_season,
                match_type: StaffMatchType::ExactBoth,
            });
        }
    }

    // 2. Double subscription: exact name match but already paid for this season
    // This is high priority because exact name match is a strong signal
    // Use unaccent for accent-insensitive matching (é = e, etc.)
    let double_subscription_matches = sqlx::query_as::<_, Staff>(
        r"
        SELECT s.* FROM staff s
        WHERE unaccent(LOWER(TRIM(s.first_name))) = unaccent($1)
        AND unaccent(LOWER(TRIM(s.last_name))) = unaccent($2)
        AND EXISTS (
            SELECT 1 FROM payments p WHERE p.staff = s.id AND p.season = $3
        )
        ",
    )
    .bind(&first_name_norm)
    .bind(&last_name_norm)
    .bind(season)
    .fetch_all(pool)
    .await?;

    for staff in double_subscription_matches {
        if candidates.iter().any(|c| c.staff.id == staff.id) {
            continue;
        }
        let latest_season = get_staff_latest_season(pool, staff.id).await?;
        candidates.push(StaffWithSeason {
            staff,
            latest_season,
            match_type: StaffMatchType::DoubleSubscription,
        });
    }

    // 2b. Exact email match but already paid (potential double subscription via email)
    for (search_email, is_payer_only) in &emails_to_search {
        if search_email.is_empty() {
            continue;
        }
        let exact_email_paid_matches = sqlx::query_as::<_, Staff>(
            r"
            SELECT s.* FROM staff s
            WHERE LOWER(TRIM(s.email)) = $1
            AND EXISTS (
                SELECT 1 FROM payments p WHERE p.staff = s.id AND p.season = $2
            )
            ",
        )
        .bind(search_email)
        .bind(season)
        .fetch_all(pool)
        .await?;

        for staff in exact_email_paid_matches {
            if candidates.iter().any(|c| c.staff.id == staff.id) {
                continue;
            }
            let latest_season = get_staff_latest_season(pool, staff.id).await?;
            // Determine match type based on whether name also matches and email source
            // Use strip_accents for accent-insensitive comparison (é = e, etc.)
            let staff_first = strip_accents(&staff.first_name.trim().to_lowercase());
            let staff_last = strip_accents(&staff.last_name.trim().to_lowercase());
            let search_first = strip_accents(&first_name_norm);
            let search_last = strip_accents(&last_name_norm);
            let match_type = if staff_first == search_first && staff_last == search_last {
                StaffMatchType::DoubleSubscription // Both email and name match
            } else if *is_payer_only {
                StaffMatchType::PayerEmailMatch // Payer email matches but names differ
            } else {
                StaffMatchType::ExactEmail // Only beneficiary email matches (different person with same email who already paid)
            };
            candidates.push(StaffWithSeason {
                staff,
                latest_season,
                match_type,
            });
        }
    }

    // 3. Exact name match (name matches but email differs, no payment yet)
    // Use unaccent for accent-insensitive matching
    let exact_name_matches = sqlx::query_as::<_, Staff>(
        r"
        SELECT s.* FROM staff s
        WHERE unaccent(LOWER(TRIM(s.first_name))) = unaccent($1)
        AND unaccent(LOWER(TRIM(s.last_name))) = unaccent($2)
        AND LOWER(TRIM(s.email)) != $3
        AND NOT EXISTS (
            SELECT 1 FROM payments p WHERE p.staff = s.id AND p.season = $4
        )
        ",
    )
    .bind(&first_name_norm)
    .bind(&last_name_norm)
    .bind(&email_norm)
    .bind(season)
    .fetch_all(pool)
    .await?;

    for staff in exact_name_matches {
        if candidates.iter().any(|c| c.staff.id == staff.id) {
            continue;
        }
        let latest_season = get_staff_latest_season(pool, staff.id).await?;
        candidates.push(StaffWithSeason {
            staff,
            latest_season,
            match_type: StaffMatchType::ExactName,
        });
    }

    // 4. Exact email match (email matches but name differs, no payment yet)
    // Use unaccent for accent-insensitive name comparison
    for (search_email, is_payer_only) in &emails_to_search {
        if search_email.is_empty() {
            continue;
        }
        let exact_email_matches = sqlx::query_as::<_, Staff>(
            r"
            SELECT s.* FROM staff s
            WHERE LOWER(TRIM(s.email)) = $1
            AND NOT (unaccent(LOWER(TRIM(s.first_name))) = unaccent($2) AND unaccent(LOWER(TRIM(s.last_name))) = unaccent($3))
            AND NOT EXISTS (
                SELECT 1 FROM payments p WHERE p.staff = s.id AND p.season = $4
            )
            ",
        )
        .bind(search_email)
        .bind(&first_name_norm)
        .bind(&last_name_norm)
        .bind(season)
        .fetch_all(pool)
        .await?;

        for staff in exact_email_matches {
            if candidates.iter().any(|c| c.staff.id == staff.id) {
                continue;
            }
            let latest_season = get_staff_latest_season(pool, staff.id).await?;
            // Use PayerEmailMatch if matched via payer email, ExactEmail if matched via beneficiary email
            let match_type = if *is_payer_only {
                StaffMatchType::PayerEmailMatch
            } else {
                StaffMatchType::ExactEmail
            };
            candidates.push(StaffWithSeason {
                staff,
                latest_season,
                match_type,
            });
        }
    }

    // 5. Similar email match (email contains or is contained) - fuzzy, lower priority
    for (search_email, _is_payer_only) in &emails_to_search {
        if search_email.len() < 3 {
            continue;
        }
        let similar_email_matches = sqlx::query_as::<_, Staff>(
            r"
            SELECT s.* FROM staff s
            WHERE (LOWER(TRIM(s.email)) LIKE '%' || $1 || '%' OR $1 LIKE '%' || LOWER(TRIM(s.email)) || '%')
            AND LOWER(TRIM(s.email)) != $1
            AND NOT EXISTS (
                SELECT 1 FROM payments p WHERE p.staff = s.id AND p.season = $2
            )
            ",
        )
        .bind(search_email)
        .bind(season)
        .fetch_all(pool)
        .await?;

        for staff in similar_email_matches {
            if candidates.iter().any(|c| c.staff.id == staff.id) {
                continue;
            }
            let latest_season = get_staff_latest_season(pool, staff.id).await?;
            candidates.push(StaffWithSeason {
                staff,
                latest_season,
                match_type: StaffMatchType::SimilarEmail,
            });
        }
    }

    // 6. Similar name match (partial match) - fuzzy, lowest priority
    // Only run if names are long enough to be meaningful (avoid matching on short strings)
    // Use unaccent for accent-insensitive matching
    if first_name_norm.len() >= 3 || last_name_norm.len() >= 3 {
        let similar_name_matches = sqlx::query_as::<_, Staff>(
            r"
            SELECT s.* FROM staff s
            WHERE (
                ($1 != '' AND LENGTH($1) >= 3 AND (unaccent(LOWER(TRIM(s.first_name))) LIKE '%' || unaccent($1) || '%' OR unaccent($1) LIKE '%' || unaccent(LOWER(TRIM(s.first_name))) || '%'))
                OR ($2 != '' AND LENGTH($2) >= 3 AND (unaccent(LOWER(TRIM(s.last_name))) LIKE '%' || unaccent($2) || '%' OR unaccent($2) LIKE '%' || unaccent(LOWER(TRIM(s.last_name))) || '%'))
            )
            AND NOT (unaccent(LOWER(TRIM(s.first_name))) = unaccent($1) AND unaccent(LOWER(TRIM(s.last_name))) = unaccent($2))
            AND NOT EXISTS (
                SELECT 1 FROM payments p WHERE p.staff = s.id AND p.season = $3
            )
            ",
        )
        .bind(&first_name_norm)
        .bind(&last_name_norm)
        .bind(season)
        .fetch_all(pool)
        .await?;

        // Collect similar name matches with their similarity scores
        let mut scored_matches: Vec<(Staff, Option<i16>, i32)> = Vec::new();

        for staff in similar_name_matches {
            if candidates.iter().any(|c| c.staff.id == staff.id) {
                continue;
            }
            let latest_season = get_staff_latest_season(pool, staff.id).await?;

            // Calculate similarity score (higher is better)
            // Strip accents for better matching (é -> e, etc.)
            let staff_first = strip_accents(&staff.first_name.trim().to_lowercase());
            let staff_last = strip_accents(&staff.last_name.trim().to_lowercase());
            let search_first = strip_accents(&first_name_norm);
            let search_last = strip_accents(&last_name_norm);

            let mut score = 0i32;

            // Exact match on last name (after stripping accents)
            if staff_last == search_last {
                score += 100;
            } else if staff_last.contains(&search_last) || search_last.contains(&staff_last) {
                // Partial match on last name - score by length similarity
                let len_diff =
                    i32::try_from(staff_last.len().abs_diff(search_last.len())).unwrap_or(i32::MAX);
                score += 50 - len_diff.min(50);
            }

            // Exact match on first name (after stripping accents)
            if staff_first == search_first {
                score += 100;
            } else if staff_first.contains(&search_first) || search_first.contains(&staff_first) {
                // Partial match on first name - score by length similarity
                let len_diff = i32::try_from(staff_first.len().abs_diff(search_first.len()))
                    .unwrap_or(i32::MAX);
                score += 50 - len_diff.min(50);
            }

            scored_matches.push((staff, latest_season, score));
        }

        // Sort by score descending (best matches first)
        scored_matches.sort_by(|a, b| b.2.cmp(&a.2));

        for (staff, latest_season, _score) in scored_matches {
            candidates.push(StaffWithSeason {
                staff,
                latest_season,
                match_type: StaffMatchType::SimilarName,
            });
        }
    }

    Ok(candidates)
}

/// Get the latest season a staff member has paid for
async fn get_staff_latest_season(pool: &PgPool, staff_id: uuid::Uuid) -> Result<Option<i16>> {
    let row = sqlx::query(
        r"
        SELECT MAX(season) as latest_season FROM payments
        WHERE staff = $1
        ",
    )
    .bind(staff_id)
    .fetch_one(pool)
    .await?;

    let latest: Option<i16> = row.try_get("latest_season")?;
    Ok(latest)
}

/// Create a new staff member and link it with a payment
/// Uses a transaction to ensure atomicity and prevent race conditions
#[allow(clippy::too_many_arguments)]
pub async fn create_staff_with_payment(
    pool: &PgPool,
    first_name: &str,
    last_name: &str,
    email: &str,
    phone: Option<&str>,
    comment: &str,
    helloasso_item_id: i64,
    season: i16,
) -> Result<Staff> {
    let mut tx = pool.begin().await?;

    // Check if already imported (within transaction for consistency)
    let already_imported: bool =
        sqlx::query_scalar(r"SELECT EXISTS(SELECT 1 FROM payments WHERE helloasso_item_id = $1)")
            .bind(helloasso_item_id)
            .fetch_one(&mut *tx)
            .await?;

    if already_imported {
        return Err(anyhow::anyhow!("ALREADY_IMPORTED"));
    }

    // Create the staff (allow duplicate names - two people can have the same name)
    let staff = sqlx::query_as::<_, Staff>(
        r"
        INSERT INTO staff (first_name, last_name, email, phone, comment)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        ",
    )
    .bind(first_name)
    .bind(last_name)
    .bind(email)
    .bind(phone)
    .bind(comment)
    .fetch_one(&mut *tx)
    .await?;

    // Create the payment link
    sqlx::query(
        r"
        INSERT INTO payments (season, helloasso_item_id, cash_id, staff)
        VALUES ($1, $2, NULL, $3)
        ",
    )
    .bind(season)
    .bind(helloasso_item_id)
    .bind(staff.id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(staff)
}

/// Update an existing staff member and link it with a payment for a new season
/// Uses a transaction to ensure atomicity and prevent race conditions
#[allow(clippy::too_many_arguments)]
pub async fn update_staff_with_payment(
    pool: &PgPool,
    staff_id: uuid::Uuid,
    first_name: &str,
    last_name: &str,
    email: &str,
    phone: Option<&str>,
    comment: &str,
    helloasso_item_id: i64,
    season: i16,
) -> Result<Staff> {
    let mut tx = pool.begin().await?;

    // Check if already imported (within transaction for consistency)
    let already_imported: bool =
        sqlx::query_scalar(r"SELECT EXISTS(SELECT 1 FROM payments WHERE helloasso_item_id = $1)")
            .bind(helloasso_item_id)
            .fetch_one(&mut *tx)
            .await?;

    if already_imported {
        return Err(anyhow::anyhow!("ALREADY_IMPORTED"));
    }

    // Update the staff
    // Use COALESCE to preserve existing phone if new value is empty
    let staff = sqlx::query_as::<_, Staff>(
        r"
        UPDATE staff
        SET first_name = $2, last_name = $3, email = $4,
            phone = COALESCE(NULLIF($5, ''), phone),
            comment = COALESCE(NULLIF($6, ''), comment),
            updated_at = NOW()
        WHERE id = $1
        RETURNING *
        ",
    )
    .bind(staff_id)
    .bind(first_name)
    .bind(last_name)
    .bind(email)
    .bind(phone)
    .bind(comment)
    .fetch_one(&mut *tx)
    .await?;

    // Create the payment link
    sqlx::query(
        r"
        INSERT INTO payments (season, helloasso_item_id, cash_id, staff)
        VALUES ($1, $2, NULL, $3)
        ",
    )
    .bind(season)
    .bind(helloasso_item_id)
    .bind(staff_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(staff)
}

/// Get a staff member by ID
pub async fn get_staff_by_id(pool: &PgPool, staff_id: uuid::Uuid) -> Result<Option<Staff>> {
    let staff = sqlx::query_as::<_, Staff>(
        r"
        SELECT * FROM staff WHERE id = $1
        ",
    )
    .bind(staff_id)
    .fetch_optional(pool)
    .await?;

    Ok(staff)
}

/// Check if a staff member is chief of any atelier
pub async fn is_chief(pool: &PgPool, staff_id: uuid::Uuid) -> Result<bool> {
    let row = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM roles WHERE staff = $1 AND chief = true)",
    )
    .bind(staff_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Get email addresses of all admin staff members
pub async fn get_admin_emails(pool: &PgPool) -> Result<Vec<String>> {
    let emails = sqlx::query_scalar::<_, String>(
        "SELECT email FROM staff WHERE is_admin = true AND email != ''",
    )
    .fetch_all(pool)
    .await?;
    Ok(emails)
}

/// Get chiefs (staff + email) for a given atelier
pub async fn get_chiefs_for_atelier(pool: &PgPool, atelier_id: uuid::Uuid) -> Result<Vec<Staff>> {
    let chiefs = sqlx::query_as::<_, Staff>(
        r"
        SELECT s.* FROM staff s
        JOIN roles r ON r.staff = s.id
        WHERE r.atelier = r.atelier AND r.chief = true AND r.atelier = $1 AND s.email != ''
        ",
    )
    .bind(atelier_id)
    .fetch_all(pool)
    .await?;
    Ok(chiefs)
}

/// Get ateliers where a staff member is chief
pub async fn get_chief_ateliers(pool: &PgPool, staff_id: uuid::Uuid) -> Result<Vec<Atelier>> {
    let ateliers = sqlx::query_as::<_, Atelier>(
        r"
        SELECT a.* FROM ateliers a
        JOIN roles r ON r.atelier = a.id
        WHERE r.staff = $1 AND r.chief = true
        ORDER BY a.name
        ",
    )
    .bind(staff_id)
    .fetch_all(pool)
    .await?;
    Ok(ateliers)
}

/// Get all memberships with their associated user info, with optional search filter
pub async fn get_all_memberships_filtered(
    pool: &PgPool,
    search: Option<&str>,
) -> Result<Vec<(User, Membership)>> {
    let search_pattern = search.map(|s| format!("%{}%", s.to_lowercase()));

    // Use DISTINCT ON to avoid duplicates, then re-order by date (most recent first), donations last for same day
    let rows = if let Some(pattern) = &search_pattern {
        sqlx::query(
            r"
            SELECT * FROM (
                SELECT DISTINCT ON (m.helloasso_item_id)
                    u.email as user_email, u.first_name as user_first_name, u.last_name as user_last_name,
                    u.phone as user_phone, u.address as user_address, u.city as user_city,
                    u.zip_code as user_zip_code, u.country as user_country, u.birth_date as user_birth_date,
                    u.created_at as user_created_at, u.updated_at as user_updated_at, u.last_sync_at as user_last_sync_at,
                    m.*
                FROM memberships m
                LEFT JOIN users u ON m.payer_email = u.email OR m.email = u.email
                WHERE LOWER(COALESCE(m.email, '')) LIKE $1
                   OR LOWER(COALESCE(m.beneficiary_first_name, '')) LIKE $1
                   OR LOWER(COALESCE(m.beneficiary_last_name, '')) LIKE $1
                   OR LOWER(COALESCE(m.payer_email, '')) LIKE $1
                   OR LOWER(COALESCE(u.email, '')) LIKE $1
                   OR LOWER(COALESCE(u.first_name, '')) LIKE $1
                   OR LOWER(COALESCE(u.last_name, '')) LIKE $1
                ORDER BY m.helloasso_item_id, m.order_date DESC
            ) sub
            ORDER BY DATE(sub.order_date) DESC, (sub.item_type = 'Donation') ASC, sub.order_date DESC
            ",
        )
        .bind(pattern)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            r"
            SELECT * FROM (
                SELECT DISTINCT ON (m.helloasso_item_id)
                    u.email as user_email, u.first_name as user_first_name, u.last_name as user_last_name,
                    u.phone as user_phone, u.address as user_address, u.city as user_city,
                    u.zip_code as user_zip_code, u.country as user_country, u.birth_date as user_birth_date,
                    u.created_at as user_created_at, u.updated_at as user_updated_at, u.last_sync_at as user_last_sync_at,
                    m.*
                FROM memberships m
                LEFT JOIN users u ON m.payer_email = u.email OR m.email = u.email
                ORDER BY m.helloasso_item_id, m.order_date DESC
            ) sub
            ORDER BY DATE(sub.order_date) DESC, (sub.item_type = 'Donation') ASC, sub.order_date DESC
            ",
        )
        .fetch_all(pool)
        .await?
    };

    let mut result = Vec::new();
    for row in rows {
        let user = User {
            email: row
                .try_get::<Option<String>, _>("user_email")?
                .unwrap_or_default(),
            first_name: row.try_get("user_first_name")?,
            last_name: row.try_get("user_last_name")?,
            phone: row.try_get("user_phone")?,
            address: row.try_get("user_address")?,
            city: row.try_get("user_city")?,
            zip_code: row.try_get("user_zip_code")?,
            country: row.try_get("user_country")?,
            birth_date: row.try_get("user_birth_date")?,
            created_at: row
                .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("user_created_at")?
                .unwrap_or_else(chrono::Utc::now),
            updated_at: row
                .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("user_updated_at")?
                .unwrap_or_else(chrono::Utc::now),
            last_sync_at: row.try_get("user_last_sync_at")?,
        };

        let membership = Membership {
            helloasso_order_id: row.try_get("helloasso_order_id")?,
            helloasso_item_id: row.try_get("helloasso_item_id")?,
            payer_email: row.try_get("payer_email")?,
            beneficiary_first_name: row.try_get("beneficiary_first_name")?,
            beneficiary_last_name: row.try_get("beneficiary_last_name")?,
            phone: row.try_get("phone")?,
            email: row.try_get("email")?,
            item_name: row.try_get("item_name")?,
            item_type: row.try_get("item_type")?,
            tier_name: row.try_get("tier_name")?,
            amount: row.try_get("amount")?,
            order_date: row.try_get("order_date")?,
            comment: row.try_get("comment")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        };

        result.push((user, membership));
    }

    Ok(result)
}

/// Get all staff members with their latest paid season
/// Sorted by: chiefs first, then alphabetically by last name, first name
pub async fn get_all_staff_with_season(pool: &PgPool) -> Result<Vec<(Staff, Option<i16>)>> {
    let rows = sqlx::query(
        r"
        SELECT s.*, MAX(p.season) as latest_season, BOOL_OR(r.chief) as is_chief
        FROM staff s
        LEFT JOIN payments p ON p.staff = s.id
        LEFT JOIN roles r ON r.staff = s.id
        GROUP BY s.id
        ORDER BY is_chief DESC NULLS LAST, s.last_name, s.first_name
        ",
    )
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for row in rows {
        let staff = Staff {
            id: row.try_get("id")?,
            first_name: row.try_get("first_name")?,
            last_name: row.try_get("last_name")?,
            phone: row.try_get("phone")?,
            email: row.try_get("email")?,
            comment: row.try_get("comment")?,
            is_admin: row.try_get("is_admin")?,
            is_god: row.try_get("is_god")?,
            token: row.try_get("token")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        };
        let latest_season: Option<i16> = row.try_get("latest_season")?;
        result.push((staff, latest_season));
    }

    Ok(result)
}

/// Get all ateliers
pub async fn get_all_ateliers(pool: &PgPool) -> Result<Vec<Atelier>> {
    let ateliers = sqlx::query_as::<_, Atelier>(r"SELECT * FROM ateliers ORDER BY name")
        .fetch_all(pool)
        .await?;

    Ok(ateliers)
}

/// Get roles for a staff member
pub async fn get_staff_roles(pool: &PgPool, staff_id: uuid::Uuid) -> Result<Vec<Role>> {
    let roles = sqlx::query_as::<_, Role>(r"SELECT * FROM roles WHERE staff = $1")
        .bind(staff_id)
        .fetch_all(pool)
        .await?;

    Ok(roles)
}

/// Add a role for a staff member
pub async fn add_role(
    pool: &PgPool,
    staff_id: uuid::Uuid,
    atelier_id: uuid::Uuid,
    validated: bool,
) -> Result<()> {
    sqlx::query(
        r"
        INSERT INTO roles (staff, atelier, validated, chief)
        VALUES ($1, $2, $3, false)
        ON CONFLICT (staff, atelier) DO NOTHING
        ",
    )
    .bind(staff_id)
    .bind(atelier_id)
    .bind(validated)
    .execute(pool)
    .await?;

    Ok(())
}

/// Remove a role for a staff member
pub async fn remove_role(
    pool: &PgPool,
    staff_id: uuid::Uuid,
    atelier_id: uuid::Uuid,
) -> Result<()> {
    sqlx::query(r"DELETE FROM roles WHERE staff = $1 AND atelier = $2")
        .bind(staff_id)
        .bind(atelier_id)
        .execute(pool)
        .await?;

    Ok(())
}

/// Update role properties (validated and/or chief)
/// Note: if chief is set to true, validated is automatically set to true as well
pub async fn update_role(
    pool: &PgPool,
    staff_id: uuid::Uuid,
    atelier_id: uuid::Uuid,
    validated: Option<bool>,
    chief: Option<bool>,
) -> Result<()> {
    if let Some(v) = validated {
        sqlx::query(r"UPDATE roles SET validated = $3 WHERE staff = $1 AND atelier = $2")
            .bind(staff_id)
            .bind(atelier_id)
            .bind(v)
            .execute(pool)
            .await?;
    }

    if let Some(c) = chief {
        // If setting chief to true, also set validated to true
        if c {
            sqlx::query(
                r"UPDATE roles SET chief = $3, validated = true WHERE staff = $1 AND atelier = $2",
            )
            .bind(staff_id)
            .bind(atelier_id)
            .bind(c)
            .execute(pool)
            .await?;
        } else {
            sqlx::query(r"UPDATE roles SET chief = $3 WHERE staff = $1 AND atelier = $2")
                .bind(staff_id)
                .bind(atelier_id)
                .bind(c)
                .execute(pool)
                .await?;
        }
    }

    Ok(())
}

/// Get an atelier by ID
pub async fn get_atelier_by_id(pool: &PgPool, atelier_id: uuid::Uuid) -> Result<Option<Atelier>> {
    let atelier = sqlx::query_as::<_, Atelier>(r"SELECT * FROM ateliers WHERE id = $1")
        .bind(atelier_id)
        .fetch_optional(pool)
        .await?;

    Ok(atelier)
}

/// Get all roles
pub async fn get_all_roles(pool: &PgPool) -> Result<Vec<Role>> {
    let roles = sqlx::query_as::<_, Role>(r"SELECT * FROM roles")
        .fetch_all(pool)
        .await?;

    Ok(roles)
}

/// Get all qualifications
pub async fn get_all_qualifications(pool: &PgPool) -> Result<Vec<Qualification>> {
    let qualifications =
        sqlx::query_as::<_, Qualification>(r"SELECT * FROM qualifications ORDER BY name")
            .fetch_all(pool)
            .await?;

    Ok(qualifications)
}

/// Get all staff qualification records
pub async fn get_all_staff_qualifications(pool: &PgPool) -> Result<Vec<StaffQualif>> {
    let staff_qualifs = sqlx::query_as::<_, StaffQualif>(
        r"SELECT id, staff, qualification, obtained_date, training_proof_mime
          FROM staff_qualif ORDER BY obtained_date DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(staff_qualifs)
}

/// Get qualifications for a specific staff member (joined with qualification name/duration)
pub async fn get_staff_qualifications_for_person(
    pool: &PgPool,
    staff_id: uuid::Uuid,
) -> Result<Vec<(StaffQualif, String, Option<i16>)>> {
    let rows = sqlx::query(
        r"SELECT sq.id, sq.staff, sq.qualification, sq.obtained_date,
                sq.training_proof_mime, q.name, q.duration
          FROM staff_qualif sq
          JOIN qualifications q ON q.id = sq.qualification
          WHERE sq.staff = $1
          ORDER BY q.name, sq.obtained_date DESC",
    )
    .bind(staff_id)
    .fetch_all(pool)
    .await?;

    let mut results = Vec::new();
    for row in &rows {
        use sqlx::Row;
        let proof_mime: Option<String> = row.try_get("training_proof_mime")?;
        let sq = StaffQualif {
            id: row.try_get("id")?,
            staff: row.try_get("staff")?,
            qualification: row.try_get("qualification")?,
            obtained_date: row.try_get("obtained_date")?,
            has_training_proof: proof_mime.is_some(),
        };
        let name: String = row.try_get("name")?;
        let duration: Option<i16> = row.try_get("duration")?;
        results.push((sq, name, duration));
    }

    Ok(results)
}

/// Create a new qualification type
pub async fn create_qualification(
    pool: &PgPool,
    name: &str,
    duration: Option<i16>,
) -> Result<Qualification> {
    let row = sqlx::query_as::<_, Qualification>(
        r"INSERT INTO qualifications (name, duration) VALUES ($1, $2) RETURNING *",
    )
    .bind(name)
    .bind(duration)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Delete a qualification type (cascades to `staff_qualif`)
pub async fn delete_qualification(pool: &PgPool, id: i32) -> Result<()> {
    sqlx::query(r"DELETE FROM qualifications WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

/// Add a staff qualification record
pub async fn add_staff_qualif(
    pool: &PgPool,
    staff_id: uuid::Uuid,
    qualification_id: i32,
    obtained_date: chrono::NaiveDate,
) -> Result<StaffQualif> {
    let row = sqlx::query_as::<_, StaffQualif>(
        r"INSERT INTO staff_qualif (staff, qualification, obtained_date) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(staff_id)
    .bind(qualification_id)
    .bind(obtained_date)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Remove a staff qualification record
pub async fn delete_staff_qualif(pool: &PgPool, id: i32) -> Result<()> {
    sqlx::query(r"DELETE FROM staff_qualif WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

/// Get all staff qualifications with staff and qualification details (for admin page)
pub async fn get_all_staff_qualifications_detailed(
    pool: &PgPool,
) -> Result<Vec<(StaffQualif, String, String, Option<i16>)>> {
    let rows = sqlx::query(
        r"SELECT sq.id, sq.staff, sq.qualification, sq.obtained_date,
                sq.training_proof_mime,
                s.first_name, s.last_name, q.name AS qual_name, q.duration
         FROM staff_qualif sq
         JOIN staff s ON s.id = sq.staff
         JOIN qualifications q ON q.id = sq.qualification
         ORDER BY s.last_name, s.first_name, q.name, sq.obtained_date DESC",
    )
    .fetch_all(pool)
    .await?;

    let mut results = Vec::new();
    for row in &rows {
        use sqlx::Row;
        let proof_mime: Option<String> = row.try_get("training_proof_mime")?;
        let sq = StaffQualif {
            id: row.try_get("id")?,
            staff: row.try_get("staff")?,
            qualification: row.try_get("qualification")?,
            obtained_date: row.try_get("obtained_date")?,
            has_training_proof: proof_mime.is_some(),
        };
        let first_name: String = row.try_get("first_name")?;
        let last_name: String = row.try_get("last_name")?;
        let staff_name = format!("{first_name} {last_name}");
        let qual_name: String = row.try_get("qual_name")?;
        let duration: Option<i16> = row.try_get("duration")?;
        results.push((sq, staff_name, qual_name, duration));
    }

    Ok(results)
}

/// Set training proof image for a staff qualification
pub async fn set_training_proof(
    pool: &PgPool,
    staff_qualif_id: i32,
    data: &[u8],
    mime: &str,
) -> Result<()> {
    sqlx::query(
        r"UPDATE staff_qualif SET training_proof_data = $1, training_proof_mime = $2 WHERE id = $3",
    )
    .bind(data)
    .bind(mime)
    .bind(staff_qualif_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get training proof image for a staff qualification
pub async fn get_training_proof(
    pool: &PgPool,
    staff_qualif_id: i32,
) -> Result<Option<(Vec<u8>, String)>> {
    let row = sqlx::query(
        r"SELECT training_proof_data, training_proof_mime FROM staff_qualif WHERE id = $1",
    )
    .bind(staff_qualif_id)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = row {
        use sqlx::Row;
        let data: Option<Vec<u8>> = row.try_get("training_proof_data")?;
        let mime: Option<String> = row.try_get("training_proof_mime")?;
        if let (Some(data), Some(mime)) = (data, mime) {
            return Ok(Some((data, mime)));
        }
    }

    Ok(None)
}

/// Clear training proof image for a staff qualification
pub async fn clear_training_proof(pool: &PgPool, staff_qualif_id: i32) -> Result<()> {
    sqlx::query(
        r"UPDATE staff_qualif SET training_proof_data = NULL, training_proof_mime = NULL WHERE id = $1",
    )
    .bind(staff_qualif_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get the owner (staff UUID) of a staff qualification record
pub async fn get_staff_qualif_owner(
    pool: &PgPool,
    staff_qualif_id: i32,
) -> Result<Option<uuid::Uuid>> {
    let row = sqlx::query(r"SELECT staff FROM staff_qualif WHERE id = $1")
        .bind(staff_qualif_id)
        .fetch_optional(pool)
        .await?;

    if let Some(row) = row {
        use sqlx::Row;
        let staff: uuid::Uuid = row.try_get("staff")?;
        return Ok(Some(staff));
    }

    Ok(None)
}

/// Update a staff member's comment
pub async fn update_staff_comment(
    pool: &PgPool,
    staff_id: uuid::Uuid,
    comment: &str,
) -> Result<()> {
    sqlx::query(r"UPDATE staff SET comment = $2, updated_at = NOW() WHERE id = $1")
        .bind(staff_id)
        .bind(comment)
        .execute(pool)
        .await?;

    Ok(())
}

/// Update a staff member's email and phone
pub async fn update_staff_contact(
    pool: &PgPool,
    staff_id: uuid::Uuid,
    email: &str,
    phone: Option<&str>,
) -> Result<()> {
    sqlx::query(r"UPDATE staff SET email = $2, phone = $3, updated_at = NOW() WHERE id = $1")
        .bind(staff_id)
        .bind(email)
        .bind(phone)
        .execute(pool)
        .await?;

    Ok(())
}

/// Get payment history for a staff member (both `HelloAsso` and cash payments)
pub async fn get_staff_payment_history(
    pool: &PgPool,
    staff_id: uuid::Uuid,
) -> Result<Vec<PaymentHistoryEntry>> {
    let rows = sqlx::query(
        r"SELECT
            p.season,
            CASE WHEN p.helloasso_item_id IS NOT NULL THEN 'helloasso' ELSE c.payment_method END AS source,
            m.order_date AS ha_date,
            c.date AS cash_date,
            COALESCE(m.amount, c.amount) AS amount,
            m.item_type AS ha_item_type,
            c.is_membership AS cash_is_membership,
            m.beneficiary_first_name AS ha_first_name,
            m.beneficiary_last_name AS ha_last_name,
            c.first_name AS cash_first_name,
            c.last_name AS cash_last_name,
            COALESCE(m.email, c.email) AS email,
            COALESCE(m.phone, c.phone) AS phone,
            m.payer_email
        FROM payments p
        LEFT JOIN memberships m ON p.helloasso_item_id = m.helloasso_item_id
        LEFT JOIN cash c ON p.cash_id = c.id
        WHERE p.staff = $1
        ORDER BY COALESCE(m.order_date, c.date::timestamptz) DESC",
    )
    .bind(staff_id)
    .fetch_all(pool)
    .await?;

    let mut entries = Vec::new();
    for row in &rows {
        let source: String = row.get("source");
        let is_helloasso = source == "helloasso";

        let date = if is_helloasso {
            let d: Option<chrono::DateTime<chrono::Utc>> = row.get("ha_date");
            d.map(|d| d.format("%d/%m/%Y").to_string())
        } else {
            let d: Option<chrono::NaiveDate> = row.get("cash_date");
            d.map(|d| d.format("%d/%m/%Y").to_string())
        };

        let item_type = if is_helloasso {
            let t: Option<String> = row.get("ha_item_type");
            match t.as_deref() {
                Some("Donation") => "Don".to_string(),
                _ => "Adhésion".to_string(),
            }
        } else {
            let is_membership: Option<bool> = row.get("cash_is_membership");
            if is_membership.unwrap_or(true) {
                "Adhésion".to_string()
            } else {
                "Don".to_string()
            }
        };

        let first_name = if is_helloasso {
            let n: Option<String> = row.get("ha_first_name");
            n.unwrap_or_default()
        } else {
            let n: Option<String> = row.get("cash_first_name");
            n.unwrap_or_default()
        };

        let last_name = if is_helloasso {
            let n: Option<String> = row.get("ha_last_name");
            n.unwrap_or_default()
        } else {
            let n: Option<String> = row.get("cash_last_name");
            n.unwrap_or_default()
        };

        entries.push(PaymentHistoryEntry {
            season: row.get("season"),
            source,
            date,
            amount: row.get("amount"),
            item_type,
            first_name,
            last_name,
            email: row.get("email"),
            phone: row.get("phone"),
            payer_email: row.get("payer_email"),
        });
    }

    Ok(entries)
}

// Cash payment functions

/// Create a new cash/check payment record
#[allow(clippy::too_many_arguments)]
pub async fn create_cash_payment(
    pool: &PgPool,
    first_name: &str,
    last_name: &str,
    email: Option<&str>,
    phone: Option<&str>,
    date: chrono::NaiveDate,
    amount: i32,
    is_membership: bool,
    payment_method: &str,
) -> Result<Cash> {
    let cash = sqlx::query_as::<_, Cash>(
        r"
        INSERT INTO cash (first_name, last_name, email, phone, date, amount, is_membership, payment_method)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING *
        ",
    )
    .bind(first_name)
    .bind(last_name)
    .bind(email)
    .bind(phone)
    .bind(date)
    .bind(amount)
    .bind(is_membership)
    .bind(payment_method)
    .fetch_one(pool)
    .await?;

    Ok(cash)
}

/// Link a cash payment to a staff member by creating a payment record
#[allow(dead_code)]
pub async fn link_cash_to_staff(
    pool: &PgPool,
    cash_id: uuid::Uuid,
    staff_id: uuid::Uuid,
    season: i16,
) -> Result<()> {
    sqlx::query(
        r"
        INSERT INTO payments (season, helloasso_item_id, cash_id, staff)
        VALUES ($1, NULL, $2, $3)
        ",
    )
    .bind(season)
    .bind(cash_id)
    .bind(staff_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get all cash payments
pub async fn get_all_cash_payments(pool: &PgPool) -> Result<Vec<Cash>> {
    let payments = sqlx::query_as::<_, Cash>(r"SELECT * FROM cash ORDER BY date DESC")
        .fetch_all(pool)
        .await?;

    Ok(payments)
}

/// Check if a staff/payment exists for a given cash payment
pub async fn has_staff_for_cash(pool: &PgPool, cash_id: uuid::Uuid) -> Result<bool> {
    let row = sqlx::query(
        r"
        SELECT EXISTS(
            SELECT 1 FROM payments WHERE cash_id = $1
        ) as exists
        ",
    )
    .bind(cash_id)
    .fetch_one(pool)
    .await?;

    let exists: bool = row.try_get("exists")?;
    Ok(exists)
}

/// Create a new staff member and link it with a cash payment
#[allow(clippy::too_many_arguments)]
pub async fn create_staff_with_cash_payment(
    pool: &PgPool,
    first_name: &str,
    last_name: &str,
    email: &str,
    phone: Option<&str>,
    comment: &str,
    cash_id: uuid::Uuid,
    season: i16,
) -> Result<Staff> {
    let mut tx = pool.begin().await?;

    // Check if already imported
    let already_imported: bool =
        sqlx::query_scalar(r"SELECT EXISTS(SELECT 1 FROM payments WHERE cash_id = $1)")
            .bind(cash_id)
            .fetch_one(&mut *tx)
            .await?;

    if already_imported {
        return Err(anyhow::anyhow!("ALREADY_IMPORTED"));
    }

    let staff = sqlx::query_as::<_, Staff>(
        r"
        INSERT INTO staff (first_name, last_name, email, phone, comment)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        ",
    )
    .bind(first_name)
    .bind(last_name)
    .bind(email)
    .bind(phone)
    .bind(comment)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        r"
        INSERT INTO payments (season, helloasso_item_id, cash_id, staff)
        VALUES ($1, NULL, $2, $3)
        ",
    )
    .bind(season)
    .bind(cash_id)
    .bind(staff.id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(staff)
}

/// Update an existing staff member and link it with a cash payment
#[allow(clippy::too_many_arguments)]
pub async fn update_staff_with_cash_payment(
    pool: &PgPool,
    staff_id: uuid::Uuid,
    first_name: &str,
    last_name: &str,
    email: &str,
    phone: Option<&str>,
    comment: &str,
    cash_id: uuid::Uuid,
    season: i16,
) -> Result<Staff> {
    let mut tx = pool.begin().await?;

    // Check if already imported
    let already_imported: bool =
        sqlx::query_scalar(r"SELECT EXISTS(SELECT 1 FROM payments WHERE cash_id = $1)")
            .bind(cash_id)
            .fetch_one(&mut *tx)
            .await?;

    if already_imported {
        return Err(anyhow::anyhow!("ALREADY_IMPORTED"));
    }

    let staff = sqlx::query_as::<_, Staff>(
        r"
        UPDATE staff
        SET first_name = $2, last_name = $3, email = $4,
            phone = COALESCE(NULLIF($5, ''), phone),
            comment = COALESCE(NULLIF($6, ''), comment),
            updated_at = NOW()
        WHERE id = $1
        RETURNING *
        ",
    )
    .bind(staff_id)
    .bind(first_name)
    .bind(last_name)
    .bind(email)
    .bind(phone)
    .bind(comment)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        r"
        INSERT INTO payments (season, helloasso_item_id, cash_id, staff)
        VALUES ($1, NULL, $2, $3)
        ",
    )
    .bind(season)
    .bind(cash_id)
    .bind(staff.id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(staff)
}

/// Get a cash payment by ID
pub async fn get_cash_by_id(pool: &PgPool, cash_id: uuid::Uuid) -> Result<Option<Cash>> {
    let cash = sqlx::query_as::<_, Cash>(r"SELECT * FROM cash WHERE id = $1")
        .bind(cash_id)
        .fetch_optional(pool)
        .await?;

    Ok(cash)
}

/// Count memberships not yet imported (no payment record for their season)
pub async fn count_unimported_memberships(pool: &PgPool, current_season: i16) -> Result<i64> {
    let row = sqlx::query(
        r"
        SELECT COUNT(*) as count FROM memberships m
        WHERE (m.item_type IS NULL OR m.item_type NOT IN ('Registration', 'Donation'))
        AND NOT EXISTS (
            SELECT 1 FROM payments p
            WHERE p.helloasso_item_id = m.helloasso_item_id
            AND p.season = (
                CASE
                    WHEN m.order_date IS NULL THEN $1
                    WHEN EXTRACT(MONTH FROM m.order_date) >= 6
                    THEN EXTRACT(YEAR FROM m.order_date)::smallint + 1
                    ELSE EXTRACT(YEAR FROM m.order_date)::smallint
                END
            )
        )
        ",
    )
    .bind(current_season)
    .fetch_one(pool)
    .await?;

    let count: i64 = row.try_get("count")?;
    Ok(count)
}

/// Count cash payments not yet linked to a staff member
pub async fn count_unimported_cash(pool: &PgPool) -> Result<i64> {
    let row = sqlx::query(
        r"
        SELECT COUNT(*) as count FROM cash c
        WHERE NOT EXISTS (
            SELECT 1 FROM payments p WHERE p.cash_id = c.id
        )
        ",
    )
    .fetch_one(pool)
    .await?;

    let count: i64 = row.try_get("count")?;
    Ok(count)
}

// Calendar / presence functions

/// Get an atelier by slug
pub async fn get_atelier_by_slug(pool: &PgPool, slug: &str) -> Result<Option<Atelier>> {
    let atelier = sqlx::query_as::<_, Atelier>(r"SELECT * FROM ateliers WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await?;

    Ok(atelier)
}

/// Get all needs for an atelier, ordered by day
pub async fn get_needs_for_atelier(pool: &PgPool, atelier_id: uuid::Uuid) -> Result<Vec<Need>> {
    let needs = sqlx::query_as::<_, Need>(r"SELECT * FROM needs WHERE atelier = $1 ORDER BY day")
        .bind(atelier_id)
        .fetch_all(pool)
        .await?;

    Ok(needs)
}

/// Batch fetch presence records for a set of need IDs
/// Returns (`needs_id`, `staff_id`, `first_half`, `second_half`)
pub async fn get_presence_for_needs(
    pool: &PgPool,
    need_ids: &[uuid::Uuid],
) -> Result<Vec<(uuid::Uuid, uuid::Uuid, bool, bool)>> {
    let rows = sqlx::query(
        r"SELECT needs, staff, first_half, second_half FROM presence WHERE needs = ANY($1)",
    )
    .bind(need_ids)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for row in rows {
        result.push((
            row.try_get("needs")?,
            row.try_get("staff")?,
            row.try_get("first_half")?,
            row.try_get("second_half")?,
        ));
    }

    Ok(result)
}

/// Get staff with a role in this atelier
/// Sorted: chiefs first, then alphabetically by last name
pub async fn get_staff_for_atelier(pool: &PgPool, atelier_id: uuid::Uuid) -> Result<Vec<Staff>> {
    let staff = sqlx::query_as::<_, Staff>(
        r"
        SELECT s.* FROM staff s
        JOIN roles r ON r.staff = s.id AND r.atelier = $1
        ORDER BY r.chief DESC, s.last_name, s.first_name
        ",
    )
    .bind(atelier_id)
    .fetch_all(pool)
    .await?;

    Ok(staff)
}

/// Get upcoming needs with their deficit (quantity - filled) for the next N days.
/// Returns (day, `atelier_name`, quantity, `filled_count`) rows, ordered by day then atelier name.
pub async fn get_upcoming_needs_deficit(
    pool: &PgPool,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> Result<Vec<(chrono::NaiveDate, String, i16, i64)>> {
    let rows = sqlx::query(
        r"
        SELECT n.day, a.name AS atelier_name, n.quantity,
               COUNT(DISTINCT p.staff) AS filled
        FROM needs n
        JOIN ateliers a ON a.id = n.atelier
        LEFT JOIN presence p ON p.needs = n.id AND (p.first_half OR p.second_half)
        WHERE n.day >= $1 AND n.day <= $2
        GROUP BY n.day, a.name, n.quantity
        ORDER BY n.day, a.name
        ",
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for row in rows {
        result.push((
            row.try_get("day")?,
            row.try_get("atelier_name")?,
            row.try_get("quantity")?,
            row.try_get("filled")?,
        ));
    }

    Ok(result)
}

/// Get all distinct days that have at least one need (for calendar highlighting)
pub async fn get_all_need_days(pool: &PgPool) -> Result<Vec<chrono::NaiveDate>> {
    let rows = sqlx::query(r"SELECT DISTINCT day FROM needs ORDER BY day")
        .fetch_all(pool)
        .await?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.try_get("day")?);
    }
    Ok(result)
}

/// Get all needs for a given day (across all ateliers)
pub async fn get_needs_for_day(pool: &PgPool, day: chrono::NaiveDate) -> Result<Vec<Need>> {
    let needs = sqlx::query_as::<_, Need>(r"SELECT * FROM needs WHERE day = $1 ORDER BY atelier")
        .bind(day)
        .fetch_all(pool)
        .await?;

    Ok(needs)
}

/// Fetch all future needs together with per-half presence counts.
/// Returns (Need, `first_half_count`, `second_half_count`).
pub async fn get_all_future_needs_with_counts(
    pool: &PgPool,
    from: chrono::NaiveDate,
) -> Result<Vec<(Need, i64, i64)>> {
    let rows = sqlx::query(
        r"
        SELECT n.id, n.day, n.atelier, n.quantity, n.nightly,
               COUNT(DISTINCT CASE WHEN p.first_half  THEN p.staff END) AS h1,
               COUNT(DISTINCT CASE WHEN p.second_half THEN p.staff END) AS h2
        FROM needs n
        LEFT JOIN presence p ON p.needs = n.id
        WHERE n.day >= $1
        GROUP BY n.id
        ORDER BY n.day, n.atelier
        ",
    )
    .bind(from)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for row in rows {
        let need = Need {
            id: row.try_get("id")?,
            day: row.try_get("day")?,
            atelier: row.try_get("atelier")?,
            quantity: row.try_get("quantity")?,
            nightly: row.try_get("nightly")?,
        };
        result.push((need, row.try_get("h1")?, row.try_get("h2")?));
    }
    Ok(result)
}

/// Upsert a need (INSERT or UPDATE on conflict day+atelier)
pub async fn upsert_need(
    pool: &PgPool,
    atelier_id: uuid::Uuid,
    day: chrono::NaiveDate,
    quantity: i16,
    nightly: bool,
) -> Result<Need> {
    let need = sqlx::query_as::<_, Need>(
        r"
        INSERT INTO needs (day, atelier, quantity, nightly)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (day, atelier) DO UPDATE SET
            quantity = EXCLUDED.quantity,
            nightly = EXCLUDED.nightly
        RETURNING *
        ",
    )
    .bind(day)
    .bind(atelier_id)
    .bind(quantity)
    .bind(nightly)
    .fetch_one(pool)
    .await?;

    Ok(need)
}

/// Delete a need by (atelier, day). Returns true if a row was deleted.
/// Associated presence rows are removed via ON DELETE CASCADE.
pub async fn delete_need(
    pool: &PgPool,
    atelier_id: uuid::Uuid,
    day: chrono::NaiveDate,
) -> Result<bool> {
    let result = sqlx::query(r"DELETE FROM needs WHERE atelier = $1 AND day = $2")
        .bind(atelier_id)
        .bind(day)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

// ── Opening days ──────────────────────────────────────────────────────

/// Get all opening days (ordered by day).
#[allow(dead_code)]
pub async fn get_all_opening_days(pool: &PgPool) -> Result<Vec<models::OpeningDay>> {
    let rows = sqlx::query_as::<_, models::OpeningDay>(
        r"SELECT day, status FROM opening_days ORDER BY day",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Get opening days for a set of dates (used when rendering calendar columns).
pub async fn get_opening_days_for_dates(
    pool: &PgPool,
    dates: &[chrono::NaiveDate],
) -> Result<Vec<models::OpeningDay>> {
    if dates.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as::<_, models::OpeningDay>(
        r"SELECT day, status FROM opening_days WHERE day = ANY($1) ORDER BY day",
    )
    .bind(dates)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Create a new opening day with 'reserved' status.
pub async fn create_opening_day(
    pool: &PgPool,
    day: chrono::NaiveDate,
) -> Result<models::OpeningDay> {
    let row = sqlx::query_as::<_, models::OpeningDay>(
        r"
        INSERT INTO opening_days (day, status) VALUES ($1, 'reserved')
        ON CONFLICT (day) DO NOTHING
        RETURNING day, status
        ",
    )
    .bind(day)
    .fetch_optional(pool)
    .await?;

    row.ok_or_else(|| anyhow::anyhow!("Opening day {day} already exists"))
}

/// Update the status of an opening day.
pub async fn update_opening_day_status(
    pool: &PgPool,
    day: chrono::NaiveDate,
    status: models::OpeningDayStatus,
) -> Result<bool> {
    let result = sqlx::query(r"UPDATE opening_days SET status = $2 WHERE day = $1")
        .bind(day)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Delete all needs (and cascade presence) for a given day across all ateliers.
#[allow(dead_code)]
pub async fn delete_needs_for_day(pool: &PgPool, day: chrono::NaiveDate) -> Result<u64> {
    let result = sqlx::query(r"DELETE FROM needs WHERE day = $1")
        .bind(day)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

/// Upsert presence; delete row if both halves are false
pub async fn upsert_presence(
    pool: &PgPool,
    needs_id: uuid::Uuid,
    staff_id: uuid::Uuid,
    first_half: bool,
    second_half: bool,
) -> Result<()> {
    if !first_half && !second_half {
        // Delete the row
        sqlx::query(r"DELETE FROM presence WHERE needs = $1 AND staff = $2")
            .bind(needs_id)
            .bind(staff_id)
            .execute(pool)
            .await?;
    } else {
        sqlx::query(
            r"
            INSERT INTO presence (needs, staff, first_half, second_half)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (needs, staff) DO UPDATE SET
                first_half = EXCLUDED.first_half,
                second_half = EXCLUDED.second_half
            ",
        )
        .bind(needs_id)
        .bind(staff_id)
        .bind(first_half)
        .bind(second_half)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Get current presence for a specific need+staff
pub async fn get_presence(
    pool: &PgPool,
    needs_id: uuid::Uuid,
    staff_id: uuid::Uuid,
) -> Result<Option<(bool, bool)>> {
    let row = sqlx::query(
        r"SELECT first_half, second_half FROM presence WHERE needs = $1 AND staff = $2",
    )
    .bind(needs_id)
    .bind(staff_id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => Ok(Some((r.try_get("first_half")?, r.try_get("second_half")?))),
        None => Ok(None),
    }
}

/// Get a single need by ID
pub async fn get_need_by_id(pool: &PgPool, need_id: uuid::Uuid) -> Result<Option<Need>> {
    let need = sqlx::query_as::<_, Need>(r"SELECT * FROM needs WHERE id = $1")
        .bind(need_id)
        .fetch_optional(pool)
        .await?;
    Ok(need)
}

/// Check whether a staff member already has presence on the same day for a different atelier,
/// on the specified half-day. Returns the conflicting atelier name if found.
pub async fn check_presence_conflict(
    pool: &PgPool,
    staff_id: uuid::Uuid,
    day: chrono::NaiveDate,
    exclude_need_id: uuid::Uuid,
    half: &str,
) -> Result<Option<String>> {
    let half_filter = match half {
        "first" => "p.first_half",
        "second" => "p.second_half",
        _ => return Ok(None),
    };
    // Cannot use a bind parameter inside a column reference, so we use two separate queries.
    let query = format!(
        r"
        SELECT a.name
        FROM presence p
        JOIN needs n ON n.id = p.needs
        JOIN ateliers a ON a.id = n.atelier
        WHERE p.staff = $1
          AND n.day = $2
          AND n.id <> $3
          AND {half_filter}
        LIMIT 1
        "
    );
    let row = sqlx::query_scalar::<_, String>(&query)
        .bind(staff_id)
        .bind(day)
        .bind(exclude_need_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Get all upcoming needs (today or later) where a staff member has a role in the atelier,
/// along with their presence status. Used for the "Mon Calendrier" widget.
/// Returns tuples of (`Need`, `atelier_name`, `atelier_slug`, `atelier_icon`, `first_half`, `second_half`).
#[allow(clippy::type_complexity)]
pub async fn get_person_calendar(
    pool: &PgPool,
    staff_id: uuid::Uuid,
) -> Result<Vec<(Need, String, String, String, bool, bool)>> {
    let today = chrono::Utc::now().date_naive();
    let rows = sqlx::query(
        r"
        SELECT n.id, n.day, n.atelier, n.quantity, n.nightly,
               a.name AS atelier_name, a.slug AS atelier_slug, a.icon AS atelier_icon,
               COALESCE(p.first_half, false) AS first_half,
               COALESCE(p.second_half, false) AS second_half
        FROM needs n
        JOIN ateliers a ON a.id = n.atelier
        JOIN roles r ON r.staff = $1 AND r.atelier = n.atelier
        LEFT JOIN presence p ON p.needs = n.id AND p.staff = $1
        WHERE n.day >= $2
        ORDER BY n.day, a.name
        ",
    )
    .bind(staff_id)
    .bind(today)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for row in rows {
        let need = Need {
            id: row.try_get("id")?,
            day: row.try_get("day")?,
            atelier: row.try_get("atelier")?,
            quantity: row.try_get("quantity")?,
            nightly: row.try_get("nightly")?,
        };
        let atelier_name: String = row.try_get("atelier_name")?;
        let atelier_slug: String = row.try_get("atelier_slug")?;
        let atelier_icon: String = row.try_get("atelier_icon")?;
        let first_half: bool = row.try_get("first_half")?;
        let second_half: bool = row.try_get("second_half")?;
        result.push((
            need,
            atelier_name,
            atelier_slug,
            atelier_icon,
            first_half,
            second_half,
        ));
    }

    Ok(result)
}

/// Update admin flags for a staff member
/// Enforces: `is_god` implies `is_admin`
pub async fn update_staff_admin_flags(
    pool: &PgPool,
    staff_id: uuid::Uuid,
    is_admin: bool,
    is_god: bool,
) -> Result<Staff> {
    // Enforce constraint: is_god implies is_admin
    let is_admin = is_god || is_admin;

    let staff = sqlx::query_as::<_, Staff>(
        r"
        UPDATE staff SET is_admin = $2, is_god = $3, updated_at = NOW()
        WHERE id = $1
        RETURNING *
        ",
    )
    .bind(staff_id)
    .bind(is_admin)
    .bind(is_god)
    .fetch_one(pool)
    .await?;

    Ok(staff)
}

/// Search staff by name (`first_name` + `last_name`) using unaccent for accent-insensitive matching
pub async fn search_staff_by_name(pool: &PgPool, query: &str) -> Result<Vec<Staff>> {
    let pattern = format!("%{}%", query.trim().to_lowercase());
    let staff = sqlx::query_as::<_, Staff>(
        r"
        SELECT * FROM staff
        WHERE unaccent(LOWER(first_name || ' ' || last_name)) LIKE unaccent($1)
        ORDER BY last_name, first_name
        LIMIT 10
        ",
    )
    .bind(&pattern)
    .fetch_all(pool)
    .await?;

    Ok(staff)
}

/// Generate a UUID v4 token for a staff member and store it in the token column
pub async fn set_staff_token(pool: &PgPool, staff_id: uuid::Uuid) -> Result<uuid::Uuid> {
    let token = uuid::Uuid::new_v4();
    sqlx::query(r"UPDATE staff SET token = $2 WHERE id = $1")
        .bind(staff_id)
        .bind(token)
        .execute(pool)
        .await?;

    Ok(token)
}

/// Atomically verify a token matches and clear it. Returns the staff if valid, None if mismatch.
pub async fn verify_and_clear_token(
    pool: &PgPool,
    staff_id: uuid::Uuid,
    token: uuid::Uuid,
) -> Result<Option<Staff>> {
    let staff = sqlx::query_as::<_, Staff>(
        r"
        UPDATE staff SET token = NULL
        WHERE id = $1 AND token = $2
        RETURNING *
        ",
    )
    .bind(staff_id)
    .bind(token)
    .fetch_optional(pool)
    .await?;

    Ok(staff)
}

/// Get all staff with their atelier names (for Mailchimp export)
pub async fn get_all_staff_with_ateliers(pool: &PgPool) -> Result<Vec<(Staff, Vec<String>)>> {
    let rows = sqlx::query(
        r"
        SELECT s.*,
               COALESCE(array_agg(a.name ORDER BY a.name) FILTER (WHERE a.name IS NOT NULL), '{}') as atelier_names
        FROM staff s
        LEFT JOIN roles r ON r.staff = s.id
        LEFT JOIN ateliers a ON a.id = r.atelier
        GROUP BY s.id
        ORDER BY s.last_name, s.first_name
        ",
    )
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for row in rows {
        let staff = Staff {
            id: row.try_get("id")?,
            first_name: row.try_get("first_name")?,
            last_name: row.try_get("last_name")?,
            phone: row.try_get("phone")?,
            email: row.try_get("email")?,
            comment: row.try_get("comment")?,
            is_admin: row.try_get("is_admin")?,
            is_god: row.try_get("is_god")?,
            token: row.try_get("token")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        };
        let atelier_names: Vec<String> = row.try_get("atelier_names")?;
        result.push((staff, atelier_names));
    }

    Ok(result)
}

/// List names of unimported memberships + cash payments (for daily summary email).
/// Returns (`first_name`, `last_name`, source) where source is "`HelloAsso`" or "Espèces/Chèque".
pub async fn list_unimported_names(
    pool: &PgPool,
    current_season: i16,
) -> Result<Vec<(String, String, String)>> {
    let rows = sqlx::query(
        r"
        SELECT beneficiary_first_name AS first_name, beneficiary_last_name AS last_name, 'HelloAsso' AS source
        FROM memberships m
        WHERE (m.item_type IS NULL OR m.item_type NOT IN ('Registration', 'Donation'))
        AND NOT EXISTS (
            SELECT 1 FROM payments p
            WHERE p.helloasso_item_id = m.helloasso_item_id
            AND p.season = (
                CASE
                    WHEN m.order_date IS NULL THEN $1
                    WHEN EXTRACT(MONTH FROM m.order_date) >= 6
                    THEN EXTRACT(YEAR FROM m.order_date)::smallint + 1
                    ELSE EXTRACT(YEAR FROM m.order_date)::smallint
                END
            )
        )
        UNION ALL
        SELECT first_name, last_name, 'Espèces/Chèque' AS source
        FROM cash c
        WHERE NOT EXISTS (
            SELECT 1 FROM payments p WHERE p.cash_id = c.id
        )
        ORDER BY last_name, first_name
        ",
    )
    .bind(current_season)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for row in rows {
        result.push((
            row.try_get("first_name")?,
            row.try_get("last_name")?,
            row.try_get("source")?,
        ));
    }

    Ok(result)
}

/// Insert an audit log entry
pub async fn insert_audit(
    pool: &PgPool,
    staff_id: Option<uuid::Uuid>,
    staff_name: &str,
    operation: &str,
    detail: &str,
) -> Result<()> {
    sqlx::query(
        r"INSERT INTO audit (staff_id, staff_name, operation, detail) VALUES ($1, $2, $3, $4)",
    )
    .bind(staff_id)
    .bind(staff_name)
    .bind(operation)
    .bind(detail)
    .execute(pool)
    .await?;

    Ok(())
}

/// Audit log entry returned by queries
pub struct AuditEntry {
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub staff_name: String,
    pub operation: String,
    pub detail: String,
}

/// Get audit log entries, most recent first, with pagination
pub async fn get_audit_log_paginated(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<AuditEntry>> {
    let rows = sqlx::query(
        r"SELECT created_at, staff_name, operation, detail FROM audit ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for row in rows {
        result.push(AuditEntry {
            created_at: row.try_get("created_at")?,
            staff_name: row.try_get("staff_name")?,
            operation: row.try_get("operation")?,
            detail: row.try_get("detail")?,
        });
    }

    Ok(result)
}

/// Look up staff names by a list of UUIDs, returning a map from UUID to "first last".
pub async fn get_staff_names_by_ids(
    pool: &PgPool,
    ids: &[uuid::Uuid],
) -> Result<std::collections::HashMap<uuid::Uuid, String>> {
    use sqlx::Row;
    let rows = sqlx::query(r"SELECT id, first_name, last_name FROM staff WHERE id = ANY($1)")
        .bind(ids)
        .fetch_all(pool)
        .await?;

    let mut map = std::collections::HashMap::new();
    for row in rows {
        let id: uuid::Uuid = row.try_get("id")?;
        let first: String = row.try_get("first_name")?;
        let last: String = row.try_get("last_name")?;
        map.insert(id, format!("{first} {last}"));
    }
    Ok(map)
}

/// Count total audit log entries
pub async fn count_audit(pool: &PgPool) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) as count FROM audit")
        .fetch_one(pool)
        .await?;
    let count: i64 = row.try_get("count")?;
    Ok(count)
}

/// Count pending (unvalidated) role requests for ateliers where a given staff is chief.
/// Returns `Vec<(atelier_name, count)>`.
pub async fn count_pending_validations_for_chief(
    pool: &PgPool,
    staff_id: uuid::Uuid,
) -> Result<Vec<(String, i64)>> {
    let rows = sqlx::query(
        r"
        SELECT a.name, COUNT(*) as cnt
        FROM roles r
        JOIN ateliers a ON a.id = r.atelier
        WHERE r.validated = false
        AND r.atelier IN (SELECT atelier FROM roles WHERE staff = $1 AND chief = true)
        GROUP BY a.name
        ORDER BY a.name
        ",
    )
    .bind(staff_id)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for row in rows {
        result.push((row.try_get("name")?, row.try_get("cnt")?));
    }
    Ok(result)
}

/// Count total pending (unvalidated) role requests.
/// If `staff_id` is Some, count only for ateliers where that staff is chief.
/// If None, count all pending validations (for admins).
pub async fn count_pending_validations(pool: &PgPool, staff_id: Option<uuid::Uuid>) -> Result<i64> {
    let row = if let Some(sid) = staff_id {
        sqlx::query(
            r"
            SELECT COUNT(*) as cnt
            FROM roles r
            WHERE r.validated = false
            AND r.atelier IN (SELECT atelier FROM roles WHERE staff = $1 AND chief = true)
            ",
        )
        .bind(sid)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query("SELECT COUNT(*) as cnt FROM roles WHERE validated = false")
            .fetch_one(pool)
            .await?
    };
    Ok(row.try_get("cnt")?)
}

/// Get pending (unvalidated) role requests with staff and atelier details.
/// If `chief_of_ateliers` is Some, filter to those ateliers only (for chiefs).
/// If None, return all pending validations (for admins).
pub async fn get_pending_validations(
    pool: &PgPool,
    chief_of_ateliers: Option<&[uuid::Uuid]>,
) -> Result<Vec<(Staff, Atelier)>> {
    let rows = if let Some(atelier_ids) = chief_of_ateliers {
        sqlx::query(
            r"
            SELECT s.*, a.id AS a_id, a.name AS a_name, a.slug AS a_slug, a.icon AS a_icon, a.needs_validation AS a_needs_validation, a.default_nightly AS a_default_nightly, a.opening_day_typical_needed AS a_opening_day_typical_needed
            FROM roles r
            JOIN staff s ON s.id = r.staff
            JOIN ateliers a ON a.id = r.atelier
            WHERE r.validated = false AND r.atelier = ANY($1)
            ORDER BY a.name, s.last_name, s.first_name
            ",
        )
        .bind(atelier_ids)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            r"
            SELECT s.*, a.id AS a_id, a.name AS a_name, a.slug AS a_slug, a.icon AS a_icon, a.needs_validation AS a_needs_validation, a.default_nightly AS a_default_nightly, a.opening_day_typical_needed AS a_opening_day_typical_needed
            FROM roles r
            JOIN staff s ON s.id = r.staff
            JOIN ateliers a ON a.id = r.atelier
            WHERE r.validated = false
            ORDER BY a.name, s.last_name, s.first_name
            ",
        )
        .fetch_all(pool)
        .await?
    };

    let mut result = Vec::new();
    for row in rows {
        let staff = Staff {
            id: row.try_get("id")?,
            first_name: row.try_get("first_name")?,
            last_name: row.try_get("last_name")?,
            phone: row.try_get("phone")?,
            email: row.try_get("email")?,
            comment: row.try_get("comment")?,
            is_admin: row.try_get("is_admin")?,
            is_god: row.try_get("is_god")?,
            token: row.try_get("token")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        };
        let atelier = Atelier {
            id: row.try_get("a_id")?,
            name: row.try_get("a_name")?,
            slug: row.try_get("a_slug")?,
            needs_validation: row.try_get("a_needs_validation")?,
            default_nightly: row.try_get("a_default_nightly")?,
            icon: row.try_get("a_icon")?,
            opening_day_typical_needed: row.try_get("a_opening_day_typical_needed")?,
        };
        result.push((staff, atelier));
    }

    Ok(result)
}

/// Check whether a staff member has any presence records for upcoming needs (today or later).
pub async fn has_upcoming_presence(pool: &PgPool, staff_id: uuid::Uuid) -> Result<bool> {
    let today = chrono::Utc::now().date_naive();
    let exists: bool = sqlx::query_scalar(
        r"
        SELECT EXISTS(
            SELECT 1 FROM presence p
            JOIN needs n ON n.id = p.needs
            WHERE p.staff = $1 AND n.day >= $2 AND (p.first_half OR p.second_half)
        )
        ",
    )
    .bind(staff_id)
    .bind(today)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

/// Tables in dependency order (parents first). Used for COPY data output.
const TABLES_PARENT_FIRST: &[&str] = &[
    "users",
    "staff",
    "cash",
    "ateliers",
    "equipments",
    "opening_days",
    "memberships",
    "payments",
    "roles",
    "needs",
    "presence",
    "photos",
    "audit",
    "content_images",
    "contents",
    "qualifications",
    "staff_qualif",
];

/// Tables in reverse dependency order (children first). Used for TRUNCATE.
const TABLES_CHILD_FIRST: &[&str] = &[
    "staff_qualif",
    "qualifications",
    "contents",
    "content_images",
    "presence",
    "audit",
    "photos",
    "roles",
    "needs",
    "payments",
    "memberships",
    "opening_days",
    "equipments",
    "cash",
    "ateliers",
    "staff",
    "users",
];

/// Produce a full database backup as a SQL string using COPY protocol.
/// Output format: TRUNCATE statements (children first) + COPY FROM stdin blocks (parents first).
pub async fn backup_all_tables(pool: &PgPool) -> Result<String> {
    let mut conn = pool.acquire().await?;
    let mut sql = String::new();

    // Header
    sql.push_str("-- PowPow database backup\n");
    sql.push_str(&format!(
        "-- Generated at {}\n\n",
        chrono::Utc::now().to_rfc3339()
    ));

    // TRUNCATE statements (children first to respect FK constraints)
    for table in TABLES_CHILD_FIRST {
        sql.push_str(&format!("TRUNCATE {} CASCADE;\n", table));
    }
    sql.push('\n');

    // COPY data blocks (parents first so FKs are satisfied on restore)
    for table in TABLES_PARENT_FIRST {
        // Get column names for this table
        let columns: Vec<String> = sqlx::query_scalar::<_, String>(
            "SELECT column_name FROM information_schema.columns WHERE table_name = $1 ORDER BY ordinal_position",
        )
        .bind(table)
        .fetch_all(&mut *conn)
        .await?;

        if columns.is_empty() {
            continue;
        }

        let col_list = columns.join(", ");
        let copy_query = format!("COPY {} ({}) TO STDOUT", table, col_list);

        let mut stream = conn.copy_out_raw(&copy_query).await?;

        // Collect all data from the COPY stream
        let mut data = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            data.extend_from_slice(&chunk);
        }

        // Only emit the COPY block if there is data
        if !data.is_empty() {
            sql.push_str(&format!("COPY {} ({}) FROM stdin;\n", table, col_list));
            // The data from COPY TO STDOUT is already tab-separated with newlines
            sql.push_str(&String::from_utf8_lossy(&data));
            // Ensure we end with a newline before the terminator
            if !sql.ends_with('\n') {
                sql.push('\n');
            }
            sql.push_str("\\.\n\n");
        }
    }

    Ok(sql)
}

/// Restore database from a SQL backup string produced by `backup_all_tables`.
/// Parses TRUNCATE statements and COPY FROM stdin blocks, executes in a transaction.
pub async fn restore_from_sql(pool: &PgPool, sql: &str) -> Result<()> {
    let mut conn = pool.acquire().await?;

    // Begin transaction using raw SQL (works on the underlying PgConnection)
    sqlx::query("BEGIN").execute(&mut *conn).await?;

    let result = restore_inner(&mut conn, sql).await;

    match result {
        Ok(()) => {
            sqlx::query("COMMIT").execute(&mut *conn).await?;
            Ok(())
        }
        Err(e) => {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            Err(e)
        }
    }
}

async fn restore_inner(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
    sql: &str,
) -> Result<()> {
    let lines: Vec<&str> = sql.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with("--") {
            i += 1;
            continue;
        }

        // Handle COPY ... FROM stdin blocks
        if line.to_uppercase().starts_with("COPY") && line.to_lowercase().ends_with("from stdin;") {
            // Extract the COPY header (without the trailing semicolon for copy_in_raw)
            let copy_header = line.trim_end_matches(';');

            // Collect data lines until we hit the \. terminator
            let mut data_lines = Vec::new();
            i += 1;
            while i < lines.len() {
                if lines[i] == "\\." {
                    i += 1;
                    break;
                }
                data_lines.push(lines[i]);
                i += 1;
            }

            // Build all data into a single buffer
            let mut buf = Vec::new();
            for row in &data_lines {
                buf.extend_from_slice(row.as_bytes());
                buf.push(b'\n');
            }

            // Stream the data into the COPY command
            let mut copy_in = conn.copy_in_raw(copy_header).await?;
            copy_in.read_from(&mut &buf[..]).await?;
            copy_in.finish().await?;
            continue;
        }

        // TRUNCATE and other SQL statements — execute directly
        sqlx::query(line).execute(&mut **conn).await?;
        i += 1;
    }

    Ok(())
}

// Photo functions
pub async fn create_photo(
    pool: &PgPool,
    photo_data: Vec<u8>,
    mime_type: String,
    photographer_id: uuid::Uuid,
) -> Result<Photo> {
    let result = sqlx::query_as::<_, Photo>(
        r"
        INSERT INTO photos (photo_data, mime_type, photographer_id)
        VALUES ($1, $2, $3)
        RETURNING *
        ",
    )
    .bind(photo_data)
    .bind(mime_type)
    .bind(photographer_id)
    .fetch_one(pool)
    .await?;

    Ok(result)
}

pub async fn get_photo_by_id(pool: &PgPool, id: uuid::Uuid) -> Result<Option<Photo>> {
    let result = sqlx::query_as::<_, Photo>(
        r"
        SELECT * FROM photos WHERE id = $1
        ",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

pub async fn get_all_photos(pool: &PgPool) -> Result<Vec<(PhotoMeta, String)>> {
    let rows = sqlx::query(
        r"
        SELECT p.id, p.mime_type, p.photographer_id, p.is_frontpage, p.is_staff, p.created_at, p.updated_at,
               s.first_name AS staff_first_name, s.last_name AS staff_last_name
        FROM photos p
        JOIN staff s ON p.photographer_id = s.id
        ORDER BY p.created_at DESC
        ",
    )
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for row in rows {
        use sqlx::FromRow;
        use sqlx::Row;
        let photo = PhotoMeta::from_row(&row)?;
        let first: String = row.try_get("staff_first_name")?;
        let last: String = row.try_get("staff_last_name")?;
        result.push((photo, format!("{} {}", first, last)));
    }

    Ok(result)
}

pub async fn delete_photo(pool: &PgPool, id: uuid::Uuid) -> Result<bool> {
    let result = sqlx::query(
        r"
        DELETE FROM photos WHERE id = $1
        ",
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Toggle the `is_frontpage` flag on a photo.
pub async fn toggle_photo_frontpage(pool: &PgPool, id: uuid::Uuid) -> Result<bool> {
    let row = sqlx::query_scalar::<_, bool>(
        r"UPDATE photos SET is_frontpage = NOT is_frontpage WHERE id = $1 RETURNING is_frontpage",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Toggle the `is_staff` flag on a photo.
pub async fn toggle_photo_staff(pool: &PgPool, id: uuid::Uuid) -> Result<bool> {
    let row = sqlx::query_scalar::<_, bool>(
        r"UPDATE photos SET is_staff = NOT is_staff WHERE id = $1 RETURNING is_staff",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub async fn create_staff_minimal(
    pool: &PgPool,
    first_name: &str,
    last_name: &str,
    email: &str,
    phone: Option<&str>,
) -> Result<Staff> {
    // Check for exact name duplicate (unaccent)
    let existing = sqlx::query_scalar::<_, bool>(
        r"SELECT EXISTS(SELECT 1 FROM staff WHERE unaccent(LOWER(TRIM(first_name))) = unaccent($1) AND unaccent(LOWER(TRIM(last_name))) = unaccent($2))"
    )
    .bind(first_name.trim().to_lowercase())
    .bind(last_name.trim().to_lowercase())
    .fetch_one(pool).await?;

    if existing {
        return Err(anyhow::anyhow!("DUPLICATE_NAME"));
    }

    let staff = sqlx::query_as::<_, Staff>(
        r"INSERT INTO staff (first_name, last_name, email, phone, comment) VALUES ($1, $2, $3, $4, '') RETURNING *"
    )
    .bind(first_name.trim())
    .bind(last_name.trim())
    .bind(email)
    .bind(phone)
    .fetch_one(pool).await?;

    Ok(staff)
}

// ── Equipment functions ──────────────────────────────────────────────

/// List all equipments, ordered by type then name.
pub async fn get_all_equipments(pool: &PgPool) -> Result<Vec<Equipment>> {
    let rows = sqlx::query_as::<_, Equipment>(
        "SELECT id, name, equipment_type, status, difficulty FROM equipments ORDER BY equipment_type, name",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Set the `status` for a single equipment, returning the new value.
pub async fn set_equipment_status(
    pool: &PgPool,
    equipment_id: uuid::Uuid,
    status: models::EquipmentStatus,
) -> Result<models::EquipmentStatus> {
    let row = sqlx::query("UPDATE equipments SET status = $1 WHERE id = $2 RETURNING status")
        .bind(status)
        .bind(equipment_id)
        .fetch_optional(pool)
        .await?;
    match row {
        Some(r) => Ok(r.try_get("status")?),
        None => Err(anyhow::anyhow!("Equipment not found")),
    }
}

/// Check whether the station is open today (has a validated opening day).
pub async fn is_station_open_today(pool: &PgPool) -> Result<bool> {
    let today = chrono::Local::now().date_naive();
    let row = sqlx::query("SELECT 1 FROM opening_days WHERE day = $1 AND status = 'validated'")
        .bind(today)
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

/// Get photo IDs for the hero slideshow (`is_frontpage` only).
pub async fn get_all_photo_ids(pool: &PgPool) -> Result<Vec<uuid::Uuid>> {
    let rows = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM photos WHERE is_frontpage = TRUE ORDER BY created_at",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Get photo IDs tagged as staff photos (`is_staff` only).
pub async fn get_staff_photo_ids(pool: &PgPool) -> Result<Vec<uuid::Uuid>> {
    let rows = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM photos WHERE is_staff = TRUE ORDER BY created_at",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ── CMS content functions ────────────────────────────────────────────

/// Fetch all content blocks, keyed by slug.
pub async fn get_all_contents(
    pool: &PgPool,
) -> Result<std::collections::HashMap<String, ContentBlock>> {
    let rows = sqlx::query_as::<_, ContentBlock>("SELECT * FROM contents ORDER BY slug")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|c| (c.slug.clone(), c)).collect())
}

/// Fetch content blocks for a set of slugs, keyed by slug.
pub async fn get_contents_by_slugs(
    pool: &PgPool,
    slugs: &[&str],
) -> Result<std::collections::HashMap<String, ContentBlock>> {
    let rows = sqlx::query_as::<_, ContentBlock>("SELECT * FROM contents WHERE slug = ANY($1)")
        .bind(slugs)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|c| (c.slug.clone(), c)).collect())
}

/// Fetch a single content block by slug.
pub async fn get_content(pool: &PgPool, slug: &str) -> Result<Option<ContentBlock>> {
    let row = sqlx::query_as::<_, ContentBlock>("SELECT * FROM contents WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Update a content block (upsert on slug).
pub async fn update_content(
    pool: &PgPool,
    slug: &str,
    title: &str,
    body: &str,
    image_id: Option<uuid::Uuid>,
    link_url: Option<&str>,
    link_label: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r"
        INSERT INTO contents (slug, title, body, image_id, link_url, link_label, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, NOW())
        ON CONFLICT (slug) DO UPDATE SET
            title = EXCLUDED.title,
            body = EXCLUDED.body,
            image_id = EXCLUDED.image_id,
            link_url = EXCLUDED.link_url,
            link_label = EXCLUDED.link_label,
            updated_at = NOW()
        ",
    )
    .bind(slug)
    .bind(title)
    .bind(body)
    .bind(image_id)
    .bind(link_url)
    .bind(link_label)
    .execute(pool)
    .await?;
    Ok(())
}

/// Create a new CMS image, returning its UUID.
pub async fn create_content_image(
    pool: &PgPool,
    data: Vec<u8>,
    content_type: &str,
    filename: &str,
) -> Result<uuid::Uuid> {
    let id = sqlx::query_scalar::<_, uuid::Uuid>(
        "INSERT INTO content_images (data, content_type, filename) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(data)
    .bind(content_type)
    .bind(filename)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Fetch a CMS image by ID (binary data included).
pub async fn get_content_image(pool: &PgPool, id: uuid::Uuid) -> Result<Option<ContentImage>> {
    let row = sqlx::query_as::<_, ContentImage>("SELECT * FROM content_images WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Delete a CMS image by ID.
pub async fn delete_content_image(pool: &PgPool, id: uuid::Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM content_images WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Get the filename of a content image (without loading binary data).
pub async fn get_content_image_filename(pool: &PgPool, id: uuid::Uuid) -> Result<Option<String>> {
    let row = sqlx::query_scalar::<_, String>("SELECT filename FROM content_images WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

// ── News (RSS feed items) ───────────────────────────────────────────

/// Upsert a news item by its RSS `guid`.
///
/// If a row with the same `guid` already exists, its text, link, `pub_date`,
/// and image columns are updated.
pub async fn upsert_news_item(
    pool: &PgPool,
    guid: &str,
    text: &str,
    link: &str,
    pub_date: Option<chrono::DateTime<chrono::Utc>>,
    image_data: Option<&[u8]>,
    image_mime: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO news (guid, text, link, pub_date, image_data, image_mime)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (guid) DO UPDATE
         SET text = EXCLUDED.text,
             link = EXCLUDED.link,
             pub_date = EXCLUDED.pub_date,
             image_data = EXCLUDED.image_data,
             image_mime = EXCLUDED.image_mime",
    )
    .bind(guid)
    .bind(text)
    .bind(link)
    .bind(pub_date)
    .bind(image_data)
    .bind(image_mime)
    .execute(pool)
    .await?;
    Ok(())
}

/// Fetch the N most recent news items for display (no image binary).
/// The first news item is fake and contains the metadata
pub async fn get_recent_news(pool: &PgPool, limit: i64) -> Result<Vec<crate::models::NewsRow>> {
    let rows = sqlx::query_as::<
        _,
        (
            uuid::Uuid,
            String,
            String,
            String,
            Option<chrono::DateTime<chrono::Utc>>,
            bool,
        ),
    >(
        "SELECT id, guid, text, link, pub_date, (image_data IS NOT NULL) AS has_image
         FROM news
         ORDER BY pub_date DESC NULLS FIRST
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut items: Vec<crate::models::NewsRow> = rows
        .into_iter()
        .map(
            |(id, guid, text, link, pub_date, has_image)| crate::models::NewsRow {
                id,
                guid,
                text,
                link,
                pub_date,
                has_image,
            },
        )
        .collect();

    if !items.is_empty() {
        let mut fake_news_position = 0;
        items.iter().enumerate().for_each(|(i, item)| {
            if item.guid.is_empty() {
                fake_news_position = i;
            }
        });
        items.swap(fake_news_position, 0);

        items[1..].sort_unstable_by(|a, b| {
            b.pub_date
                .unwrap_or_default()
                .cmp(&a.pub_date.unwrap_or_default())
        });
    }
    Ok(items)
}

/// Fetch news-image binary data and MIME type by news row ID.
pub async fn get_news_image(pool: &PgPool, id: uuid::Uuid) -> Result<Option<(Vec<u8>, String)>> {
    let row = sqlx::query_as::<_, (Vec<u8>, String)>(
        "SELECT image_data, image_mime FROM news WHERE id = $1 AND image_data IS NOT NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Create a new atelier.
pub async fn create_atelier(
    pool: &PgPool,
    name: &str,
    slug: &str,
    icon: &str,
    needs_validation: bool,
    default_nightly: bool,
    opening_day_typical_needed: i16,
) -> Result<Atelier> {
    let atelier = sqlx::query_as::<_, Atelier>(
        r"INSERT INTO ateliers (id, name, slug, icon, needs_validation, default_nightly, opening_day_typical_needed)
          VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6)
          RETURNING *",
    )
    .bind(name)
    .bind(slug)
    .bind(icon)
    .bind(needs_validation)
    .bind(default_nightly)
    .bind(opening_day_typical_needed)
    .fetch_one(pool)
    .await?;

    Ok(atelier)
}

/// Update an existing atelier.
#[allow(clippy::too_many_arguments)]
pub async fn update_atelier(
    pool: &PgPool,
    id: uuid::Uuid,
    name: &str,
    slug: &str,
    icon: &str,
    needs_validation: bool,
    default_nightly: bool,
    opening_day_typical_needed: i16,
) -> Result<Atelier> {
    let atelier = sqlx::query_as::<_, Atelier>(
        r"UPDATE ateliers
          SET name = $2, slug = $3, icon = $4, needs_validation = $5,
              default_nightly = $6, opening_day_typical_needed = $7
          WHERE id = $1
          RETURNING *",
    )
    .bind(id)
    .bind(name)
    .bind(slug)
    .bind(icon)
    .bind(needs_validation)
    .bind(default_nightly)
    .bind(opening_day_typical_needed)
    .fetch_one(pool)
    .await?;

    Ok(atelier)
}

/// Delete an atelier by ID.
pub async fn delete_atelier(pool: &PgPool, id: uuid::Uuid) -> Result<()> {
    sqlx::query(r"DELETE FROM ateliers WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

/// Keep only the `keep` most recent news items, deleting the rest.
pub async fn prune_old_news(pool: &PgPool, keep: i64) -> Result<u64> {
    let result = sqlx::query(
        "DELETE FROM news
        WHERE id NOT IN (
            SELECT id FROM news
            WHERE guid <> ''
            ORDER BY pub_date DESC NULLS LAST LIMIT $1
        )
        AND guid <> ''",
    )
    .bind(keep)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
