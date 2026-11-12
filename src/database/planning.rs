use chrono::NaiveDate;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::models::{AtelierDayPlanning, PlannedStaff};

pub async fn has_activity(pool: &PgPool, day: NaiveDate) -> anyhow::Result<bool> {
    Ok(
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM needs WHERE day = $1)")
            .bind(day)
            .fetch_one(pool)
            .await?,
    )
}

/// Load every scheduled atelier and its assigned staff for one local calendar day.
pub async fn for_day(pool: &PgPool, day: NaiveDate) -> anyhow::Result<Vec<AtelierDayPlanning>> {
    let rows = sqlx::query(
        "SELECT a.id AS atelier_id, a.name, a.slug, a.icon, n.nightly,
                s.id AS staff_id, s.first_name, s.last_name, p.first_half, p.second_half
         FROM needs n
         JOIN ateliers a ON a.id = n.atelier
         LEFT JOIN presence p ON p.needs = n.id AND (p.first_half OR p.second_half)
         LEFT JOIN staff s ON s.id = p.staff
         WHERE n.day = $1
         ORDER BY a.name, a.id, s.last_name, s.first_name, s.id",
    )
    .bind(day)
    .fetch_all(pool)
    .await?;

    let mut planning: Vec<AtelierDayPlanning> = Vec::new();
    for row in rows {
        let atelier_id = row.try_get("atelier_id")?;
        if planning
            .last()
            .is_none_or(|entry| entry.atelier_id != atelier_id)
        {
            planning.push(AtelierDayPlanning {
                atelier_id,
                atelier_name: row.try_get("name")?,
                atelier_slug: row.try_get("slug")?,
                atelier_icon: row.try_get("icon")?,
                nightly: row.try_get("nightly")?,
                staff: Vec::new(),
            });
        }
        if let Some(id) = row.try_get::<Option<Uuid>, _>("staff_id")? {
            planning
                .last_mut()
                .expect("atelier was added")
                .staff
                .push(PlannedStaff {
                    id,
                    first_name: row.try_get("first_name")?,
                    last_name: row.try_get("last_name")?,
                    first_half: row.try_get("first_half")?,
                    second_half: row.try_get("second_half")?,
                });
        }
    }
    Ok(planning)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("half must be 'first' or 'second'")]
    InvalidHalf,
    #[error("Bénévole ou besoin introuvable")]
    NotFound,
    #[error("Une inscription validée à cet atelier est nécessaire.")]
    Enrollment,
    #[error("Vous êtes déjà inscrit(e) sur ce créneau pour l'atelier « {0} »")]
    Conflict(String),
    #[error(transparent)]
    Database(#[from] anyhow::Error),
}

/// Serialize a member's read/check/write cycle across all ateliers. A lock on
/// an existing staff row also works when no presence rows exist yet.
pub async fn toggle(
    pool: &PgPool,
    staff: Uuid,
    need_id: Uuid,
    half: &str,
    value: bool,
) -> Result<(bool, bool), Error> {
    if !matches!(half, "first" | "second") {
        return Err(Error::InvalidHalf);
    }
    let mut tx = pool.begin().await.map_err(anyhow::Error::from)?;
    let member: Option<Uuid> = sqlx::query_scalar("SELECT id FROM staff WHERE id = $1 FOR UPDATE")
        .bind(staff)
        .fetch_optional(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
    if member.is_none() {
        return Err(Error::NotFound);
    }
    let need = super::get_need_by_id(&mut *tx, need_id)
        .await?
        .ok_or(Error::NotFound)?;
    if value {
        if !super::can_register_for_need(&mut *tx, staff, need_id).await? {
            return Err(Error::Enrollment);
        }
        if let Some(atelier) =
            super::check_presence_conflict(&mut *tx, staff, need.day, need_id, half).await?
        {
            return Err(Error::Conflict(atelier));
        }
    }
    let (mut first, mut second) = super::get_presence(&mut *tx, need_id, staff)
        .await?
        .unwrap_or_default();
    if half == "first" {
        first = value;
    } else {
        second = value;
    }
    super::upsert_presence(&mut *tx, need_id, staff, first, second).await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok((first, second))
}
