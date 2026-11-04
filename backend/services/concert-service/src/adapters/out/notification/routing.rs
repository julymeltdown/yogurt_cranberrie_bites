//! Closed projection of committed Social commands. No handle, consent payload,
//! introduction text, location or user-supplied summary crosses this boundary.
use super::source::{ProjectionRouting, Source};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_domain::notification::*;
use sqlx::{PgConnection, Row};
use uuid::Uuid;

pub struct SocialProjectionRouting;
#[async_trait]
impl ProjectionRouting for SocialProjectionRouting {
    async fn route_in(
        &self,
        connection: &mut PgConnection,
        source: &Source,
        now: DateTime<Utc>,
    ) -> Result<Vec<Recipient>> {
        source.envelope.validate()?;
        if source.actor_id.is_nil() {
            return Err(NotificationError::Unavailable);
        }
        // Unknown families must retry until their real projection is supplied.
        // An empty result is reserved for an explicit current suppression below.
        if !matches!(
            source.operation.as_str(),
            "createConcertSocialRequest"
                | "decideConcertSocialRequest"
                | "cancelConcertSocialRequest"
                | "revokeConcertSocialRequest"
        ) {
            return Err(NotificationError::Unavailable);
        }
        let event = source.envelope.event_id;
        let event_row = sqlx::query(
            "select phase,revision from public.concert_events where event_id=$1 for share",
        )
        .bind(event)
        .fetch_optional(&mut *connection)
        .await
        .map_err(db)?
        .ok_or(NotificationError::Unavailable)?;
        revision(event_row.try_get("revision").map_err(db)?)?;
        let enabled = match event_row
            .try_get::<String, _>("phase")
            .map_err(db)?
            .as_str()
        {
            "OPEN" | "LIVE" | "READ_ONLY" => true,
            "DRAFT" | "ARCHIVED" => false,
            _ => return Err(NotificationError::Unavailable),
        };
        let social: bool = sqlx::query_scalar(
            "select social from public.concert_event_settings where event_id=$1 for share",
        )
        .bind(event)
        .fetch_optional(&mut *connection)
        .await
        .map_err(db)?
        .ok_or(NotificationError::Unavailable)?;
        let row = sqlx::query("select requester_id,recipient_id,state,revision,created_at,expires_at from public.concert_social_requests where event_id=$1 and request_id=$2 for share")
            .bind(event).bind(source.envelope.resource_id).fetch_optional(&mut *connection).await.map_err(db)?.ok_or(NotificationError::Unavailable)?;
        let current: i64 = row.try_get("revision").map_err(db)?;
        revision(current)?;
        let requester: Uuid = row.try_get("requester_id").map_err(db)?;
        let recipient: Uuid = row.try_get("recipient_id").map_err(db)?;
        let created: DateTime<Utc> = row.try_get("created_at").map_err(db)?;
        let expires: DateTime<Utc> = row.try_get("expires_at").map_err(db)?;
        if requester.is_nil()
            || recipient.is_nil()
            || requester == recipient
            || current < source.envelope.resource_revision
            || created > now
            || expires <= created
            || !matches!(
                row.try_get::<String, _>("state").map_err(db)?.as_str(),
                "PENDING" | "APPROVED" | "REJECTED" | "CANCELLED" | "REVOKED" | "EXPIRED"
            )
        {
            return Err(NotificationError::Unavailable);
        }
        // A later transition does not invalidate an earlier immutable outbox.
        // Current state never grants access to a handle or extends its expiry.
        let user_id = match source.operation.as_str() {
            "createConcertSocialRequest"
                if source.actor_id == requester && source.envelope.resource_revision == 1 =>
            {
                recipient
            }
            "decideConcertSocialRequest"
                if source.actor_id == recipient && source.envelope.resource_revision > 1 =>
            {
                requester
            }
            "cancelConcertSocialRequest"
                if source.actor_id == requester && source.envelope.resource_revision > 1 =>
            {
                recipient
            }
            "revokeConcertSocialRequest"
                if source.envelope.resource_revision > 1 && source.actor_id == requester =>
            {
                recipient
            }
            "revokeConcertSocialRequest"
                if source.envelope.resource_revision > 1 && source.actor_id == recipient =>
            {
                requester
            }
            _ => return Err(NotificationError::Unavailable),
        };
        let mut suppressed = !enabled || !social;
        let mut subjects = [requester, recipient];
        subjects.sort();
        for subject in subjects {
            let admission = sqlx::query("select state,restriction_revision from public.concert_event_admissions where event_id=$1 and user_id=$2 for share")
                .bind(event).bind(subject).fetch_optional(&mut *connection).await.map_err(db)?.ok_or(NotificationError::Unavailable)?;
            revision(admission.try_get("restriction_revision").map_err(db)?)?;
            match admission
                .try_get::<String, _>("state")
                .map_err(db)?
                .as_str()
            {
                "ACTIVE" => {}
                "SUSPENDED" => suppressed = true,
                _ => return Err(NotificationError::Unavailable),
            }
            let member: i64 = sqlx::query_scalar("select revision from public.concert_participants where event_id=$1 and user_id=$2 for share")
                .bind(event).bind(subject).fetch_optional(&mut *connection).await.map_err(db)?.ok_or(NotificationError::Unavailable)?;
            revision(member)?;
        }
        let pair = sqlx::query("select low_blocks_high,high_blocks_low,revision from public.concert_relationship_pairs where event_id=$1 and low_user=$2 and high_user=$3 for share")
            .bind(event).bind(subjects[0]).bind(subjects[1]).fetch_optional(&mut *connection).await.map_err(db)?.ok_or(NotificationError::Unavailable)?;
        revision(pair.try_get("revision").map_err(db)?)?;
        suppressed |= pair.try_get::<bool, _>("low_blocks_high").map_err(db)?
            || pair.try_get::<bool, _>("high_blocks_low").map_err(db)?;
        if suppressed {
            return Ok(vec![]);
        }
        Ok(vec![Recipient {
            user_id,
            source_actor_id: Some(source.actor_id),
            kind: Kind::Social,
            target: Some(Target {
                target_type: TargetType::SocialRequest,
                target_id: source.envelope.resource_id,
            }),
        }])
    }
}
fn revision(value: i64) -> Result<()> {
    if (1..=concert_domain::MAX_REVISION).contains(&value) {
        Ok(())
    } else {
        Err(NotificationError::Unavailable)
    }
}
fn db(_: sqlx::Error) -> NotificationError {
    NotificationError::Unavailable
}
