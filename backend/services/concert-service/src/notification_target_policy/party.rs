//! Same public-party/private-application audience as the owning repositories.
use super::local::{self, db, revision};
use concert_application::notification::{Candidate, Context};
use concert_domain::{
    community::party::Party,
    notification::{NotificationError, Result, TargetType},
};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

async fn load(c: &mut PgConnection, context: Context, id: Uuid) -> Result<Party> {
    let row = sqlx::query("select owner_id,revision,state,member_count,capacity,meeting_revision,audience_revision,aggregate from public.concert_community_parties where event_id=$1 and party_id=$2 for share")
        .bind(context.event_id).bind(id).fetch_optional(c).await.map_err(db)?.ok_or(NotificationError::Unavailable)?;
    let value: Party = serde_json::from_value(row.try_get("aggregate").map_err(db)?).map_err(db)?;
    value.validate().map_err(db)?;
    if value.event != context.event_id
        || value.id != id
        || value.owner.is_nil()
        || value.owner != row.try_get::<Uuid, _>("owner_id").map_err(db)?
        || value.revision != row.try_get::<i64, _>("revision").map_err(db)?
        || serde_json::to_value(value.state).map_err(db)?
            != row.try_get::<String, _>("state").map_err(db)?
        || value.member_count() != row.try_get::<i32, _>("member_count").map_err(db)?
        || value.fields.capacity != row.try_get::<i32, _>("capacity").map_err(db)?
        || value.meeting_revision != row.try_get::<i64, _>("meeting_revision").map_err(db)?
        || value.audience_revision != row.try_get::<i64, _>("audience_revision").map_err(db)?
    {
        return Err(NotificationError::Unavailable);
    }
    Ok(value)
}
pub(super) async fn party(
    c: &mut PgConnection,
    context: Context,
    candidate: Candidate,
) -> Result<bool> {
    let value = load(c, context, local::id(candidate)?).await?;
    // CLOSED/CANCELLED/COMPLETED retain the owner's public status projection;
    // active membership/meeting access is a separate policy, never granted here.
    if !local::feature(c, context, TargetType::Party).await? {
        return Ok(false);
    }
    local::audience(c, context, candidate, vec![value.owner]).await
}
pub(super) async fn application(
    c: &mut PgConnection,
    context: Context,
    candidate: Candidate,
) -> Result<bool> {
    let id = local::id(candidate)?;
    // Hint only: after the root lock, re-read and SHARE-lock the exact child.
    let parent: Option<Uuid> = sqlx::query_scalar("select party_id from public.concert_community_applications where event_id=$1 and application_id=$2")
        .bind(context.event_id).bind(id).fetch_optional(&mut *c).await.map_err(db)?;
    let value = load(c, context, parent.ok_or(NotificationError::Unavailable)?).await?;
    let row = sqlx::query("select party_id,user_id,state,revision from public.concert_community_applications where event_id=$1 and application_id=$2 for share")
        .bind(context.event_id).bind(id).fetch_optional(&mut *c).await.map_err(db)?.ok_or(NotificationError::Unavailable)?;
    let child = value
        .applications
        .iter()
        .find(|a| a.id == id)
        .ok_or(NotificationError::Unavailable)?;
    revision(row.try_get("revision").map_err(db)?)?;
    if value.id != row.try_get::<Uuid, _>("party_id").map_err(db)?
        || child.user != row.try_get::<Uuid, _>("user_id").map_err(db)?
        || child.revision != row.try_get::<i64, _>("revision").map_err(db)?
        || serde_json::to_value(child.state).map_err(db)?
            != row.try_get::<String, _>("state").map_err(db)?
    {
        return Err(NotificationError::Unavailable);
    }
    if context.actor_id != value.owner && context.actor_id != child.user {
        return Ok(false);
    }
    // All four states remain readable status records to the current owner or
    // applicant, as application_from_receipt/catalog::applications specify.
    if !local::feature(c, context, TargetType::Application).await? {
        return Ok(false);
    }
    local::audience(c, context, candidate, vec![value.owner, child.user]).await
}
