//! Social metadata observation, never reciprocal handle/reveal authorization.
use super::local::{self, db, revision};
use concert_application::notification::{Candidate, Context};
use concert_domain::notification::{NotificationError, Result, TargetType};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

pub(super) async fn request(
    c: &mut PgConnection,
    context: Context,
    candidate: Candidate,
) -> Result<bool> {
    let row = sqlx::query("select requester_id,recipient_id,intro_id,intro_revision,state,revision,expires_at,created_at from public.concert_social_requests where event_id=$1 and request_id=$2 for share")
        .bind(context.event_id).bind(local::id(candidate)?).fetch_optional(&mut *c).await.map_err(db)?.ok_or(NotificationError::Unavailable)?;
    revision(row.try_get("revision").map_err(db)?)?;
    revision(row.try_get("intro_revision").map_err(db)?)?;
    let requester: Uuid = row.try_get("requester_id").map_err(db)?;
    let recipient: Uuid = row.try_get("recipient_id").map_err(db)?;
    if requester.is_nil()
        || recipient.is_nil()
        || requester == recipient
        || row.try_get::<Uuid, _>("intro_id").map_err(db)?.is_nil()
        || row
            .try_get::<chrono::DateTime<chrono::Utc>, _>("expires_at")
            .map_err(db)?
            <= row
                .try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
                .map_err(db)?
        || !matches!(
            row.try_get::<String, _>("state").map_err(db)?.as_str(),
            "PENDING" | "APPROVED" | "REJECTED" | "CANCELLED" | "REVOKED" | "EXPIRED"
        )
    {
        return Err(NotificationError::Unavailable);
    }
    if context.actor_id != requester && context.actor_id != recipient {
        return Ok(false);
    }
    let bindings = sqlx::query("select platform,requester_version,recipient_version,requester_consents,approved from public.concert_social_bindings where event_id=$1 and request_id=$2 order by platform for share")
        .bind(context.event_id).bind(local::id(candidate)?).fetch_all(&mut *c).await.map_err(db)?;
    if !(1..=2).contains(&bindings.len()) {
        return Err(NotificationError::Unavailable);
    }
    for binding in bindings {
        if !matches!(
            binding
                .try_get::<String, _>("platform")
                .map_err(db)?
                .as_str(),
            "INSTAGRAM" | "X"
        ) || !binding
            .try_get::<bool, _>("requester_consents")
            .map_err(db)?
        {
            return Err(NotificationError::Unavailable);
        }
        revision(binding.try_get("requester_version").map_err(db)?)?;
        revision(binding.try_get("recipient_version").map_err(db)?)?;
    }
    if !local::feature(c, context, TargetType::SocialRequest).await? {
        return Ok(false);
    }
    // Actual getConcertSocialRequest retains metadata even after expiry/revoke;
    // expiry never extends any sharing grant. No handle, grant or Intro content
    // is selected. Root must resolve the current exact operation separately.
    local::audience(c, context, candidate, vec![requester, recipient]).await
}
