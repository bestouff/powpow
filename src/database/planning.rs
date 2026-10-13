use sqlx::PgPool;
use uuid::Uuid;

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
