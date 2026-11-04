//! Public non-draft giveaway and owner/recipient reservation status policy.
use super::local::{self, db, revision};
use concert_application::notification::{Candidate, Context};
use concert_domain::{
    community::giveaway::{GiftState, Giveaway},
    notification::{NotificationError, Result, TargetType},
};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

async fn load(c: &mut PgConnection, context: Context, id: Uuid) -> Result<Giveaway> {
    let row = sqlx::query("select owner_id,revision,state,quantity_total,reserved_unclaimed,quantity_claimed,aggregate from public.concert_community_giveaways where event_id=$1 and giveaway_id=$2 for share")
        .bind(context.event_id).bind(id).fetch_optional(c).await.map_err(db)?.ok_or(NotificationError::Unavailable)?;
    let value: Giveaway =
        serde_json::from_value(row.try_get("aggregate").map_err(db)?).map_err(db)?;
    value.validate().map_err(db)?;
    if value.event != context.event_id
        || value.id != id
        || value.owner.is_nil()
        || value.owner != row.try_get::<Uuid, _>("owner_id").map_err(db)?
        || value.revision != row.try_get::<i64, _>("revision").map_err(db)?
        || serde_json::to_value(value.state).map_err(db)?
            != row.try_get::<String, _>("state").map_err(db)?
        || value.fields.quantity_total != row.try_get::<i32, _>("quantity_total").map_err(db)?
        || value.reserved_unclaimed != row.try_get::<i32, _>("reserved_unclaimed").map_err(db)?
        || value.quantity_claimed != row.try_get::<i32, _>("quantity_claimed").map_err(db)?
    {
        return Err(NotificationError::Unavailable);
    }
    Ok(value)
}
pub(super) async fn giveaway(
    c: &mut PgConnection,
    context: Context,
    candidate: Candidate,
) -> Result<bool> {
    let value = load(c, context, local::id(candidate)?).await?;
    if value.state == GiftState::Draft || !local::feature(c, context, TargetType::Giveaway).await? {
        return Ok(false);
    }
    // The owner exposes paused/closed/cancelled public status, not pickup access.
    local::audience(c, context, candidate, vec![value.owner]).await
}
pub(super) async fn reservation(
    c: &mut PgConnection,
    context: Context,
    candidate: Candidate,
) -> Result<bool> {
    let id = local::id(candidate)?;
    let parent: Option<Uuid> = sqlx::query_scalar("select giveaway_id from public.concert_community_reservations where event_id=$1 and reservation_id=$2")
        .bind(context.event_id).bind(id).fetch_optional(&mut *c).await.map_err(db)?;
    let value = load(c, context, parent.ok_or(NotificationError::Unavailable)?).await?;
    let row = sqlx::query("select giveaway_id,recipient_id,state,revision,expires_at from public.concert_community_reservations where event_id=$1 and reservation_id=$2 for share")
        .bind(context.event_id).bind(id).fetch_optional(&mut *c).await.map_err(db)?.ok_or(NotificationError::Unavailable)?;
    let child = value
        .reservations
        .iter()
        .find(|r| r.id == id)
        .ok_or(NotificationError::Unavailable)?;
    revision(row.try_get("revision").map_err(db)?)?;
    if value.id != row.try_get::<Uuid, _>("giveaway_id").map_err(db)?
        || child.recipient != row.try_get::<Uuid, _>("recipient_id").map_err(db)?
        || child.revision != row.try_get::<i64, _>("revision").map_err(db)?
        || child.expires_at
            != row
                .try_get::<chrono::DateTime<chrono::Utc>, _>("expires_at")
                .map_err(db)?
        || child.expires_at <= child.created_at
        || serde_json::to_value(child.state).map_err(db)?
            != row.try_get::<String, _>("state").map_err(db)?
    {
        return Err(NotificationError::Unavailable);
    }
    if context.actor_id != value.owner && context.actor_id != child.recipient {
        return Ok(false);
    }
    if !local::feature(c, context, TargetType::Reservation).await? {
        return Ok(false);
    }
    // Expired/claimed/cancelled status remains readable to the same audience.
    // We neither lazily advance state nor grant pickup, meeting or code access.
    local::audience(c, context, candidate, vec![value.owner, child.recipient]).await
}
