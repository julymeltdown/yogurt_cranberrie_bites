//! Actual Concert policy/resource locks for GiftAuthority's retained store.
//! Physical selection adapts BE09's nested-savepoint pattern at source base 1cbb1823.
use super::protocol::{GiftIdentity, GiftOperation, GiftRequest};
use crate::{
    adapters::out::repository::place::PgPlaceRepository,
    community::{giveaway::GiftSettings, policy::CommunityResources, CommunityAccess},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{commands::CommandError, place::SelectionOutcome};
use concert_domain::{community::giveaway::PickupMeeting, place::PlaceId, EventId, Revision};
use sqlx::{postgres::PgTransactionManager, Acquire, PgConnection, Row, TransactionManager};
use uuid::Uuid;

/// Mandatory media owner adapter. Production implementation must call actual
/// PgMedia::link_owned_in with its owning captured policy semantics; BE37 pair
/// revision is not interchangeable with uploader admission/security generations.
/// No permissive/default implementation is provided.
#[async_trait]
pub trait GiftMediaLink: Send + Sync {
    async fn link_in(
        &self,
        c: &mut PgConnection,
        identity: GiftIdentity,
        giveaway: Uuid,
        media: Uuid,
    ) -> Result<(), CommandError>;
}
pub struct PgGiftResources<M> {
    identity: GiftIdentity,
    operation: GiftOperation,
    media: M,
}
impl<M: GiftMediaLink> PgGiftResources<M> {
    pub fn new(request: &GiftRequest, media: M) -> Result<Self, CommandError> {
        let identity = request.identity();
        identity.check().map_err(|_| CommandError::Denied)?;
        Ok(Self {
            identity,
            operation: request.operation(),
            media,
        })
    }
    async fn bind(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        actor: Option<Uuid>,
    ) -> Result<(), CommandError> {
        self.identity.check().map_err(|_| CommandError::Expired)?;
        if event != self.identity.event
            || actor.is_some_and(|a| a != self.identity.actor)
            || PgTransactionManager::get_transaction_depth(c) == 0
        {
            return Err(CommandError::Denied);
        }
        // Original transport deadline is only an upper bound, never an Auth grant.
        // SharedCommunityAuthority still MUST call its canonical owning adapter.
        let until = DateTime::<Utc>::from_timestamp_micros(self.identity.deadline.expires_micros())
            .ok_or(CommandError::Unavailable)?;
        sqlx::query("select set_config('mcr.community_authority_until',least(coalesce(nullif(current_setting('mcr.community_authority_until',true),'')::timestamptz,'infinity'::timestamptz),$1::timestamptz)::text,true)")
            .bind(until).execute(c).await.map_err(db)?;
        Ok(())
    }
}
fn db(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
#[async_trait]
impl<M: GiftMediaLink> CommunityResources for PgGiftResources<M> {
    async fn feature_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        actor: Uuid,
        access: CommunityAccess,
    ) -> Result<(), CommandError> {
        self.bind(c, event, Some(actor)).await?;
        if !matches!(
            access,
            CommunityAccess::ReadGifts | CommunityAccess::WriteGifts | CommunityAccess::SafetyGifts
        ) {
            return Err(CommandError::Denied);
        }
        let row=sqlx::query("select e.phase,s.giveaways from concert_events e join concert_event_settings s using(event_id) where e.event_id=$1 for share of e,s")
            .bind(event).fetch_optional(c).await.map_err(db)?.ok_or(CommandError::Unavailable)?;
        let phase: String = row.try_get("phase").map_err(db)?;
        let enabled: bool = row.try_get("giveaways").map_err(db)?;
        // Existing reservations can still be cancelled when feature writes stop.
        if !matches!(phase.as_str(), "OPEN" | "LIVE" | "READ_ONLY")
            || (access == CommunityAccess::WriteGifts && phase == "READ_ONLY")
            || (!enabled && self.operation != GiftOperation::Cancel)
        {
            return Err(CommandError::Denied);
        }
        Ok(())
    }
    async fn place_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        place: Uuid,
    ) -> Result<(), CommandError> {
        self.bind(c, event, None).await?;
        select_place(c, event, place, None).await
    }
    async fn gift_settings_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
    ) -> Result<GiftSettings, CommandError> {
        self.bind(c, event, None).await?;
        let row=sqlx::query("select s.pickup_grace_seconds,m.event_cutoff from concert_event_settings s join concert_meeting_settings m using(event_id) where s.event_id=$1 for share of s,m")
            .bind(event).fetch_optional(c).await.map_err(db)?.ok_or(CommandError::Unavailable)?;
        let grace: i64 = row.try_get("pickup_grace_seconds").map_err(db)?;
        if !(0..=86400).contains(&grace) {
            return Err(CommandError::Unavailable);
        }
        Ok(GiftSettings {
            pickup_grace_seconds: grace as i32,
            event_cutoff: row.try_get("event_cutoff").map_err(db)?,
        })
    }
    async fn gift_media_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        actor: Uuid,
        giveaway: Uuid,
        media: Option<Uuid>,
    ) -> Result<(), CommandError> {
        self.bind(c, event, Some(actor)).await?;
        if giveaway.is_nil() || media.is_some_and(|id| id.is_nil()) {
            return Err(CommandError::InvalidInput);
        }
        if let Some(media) = media {
            self.media
                .link_in(c, self.identity, giveaway, media)
                .await?;
        }
        Ok(())
    }
    async fn pickup_meeting_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        meeting: &PickupMeeting,
    ) -> Result<(), CommandError> {
        self.bind(c, event, None).await?;
        let settings = self.gift_settings_in(c, event).await?;
        let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *c)
            .await
            .map_err(db)?;
        if meeting.expires_at <= now || meeting.expires_at > settings.event_cutoff {
            return Err(CommandError::Denied);
        }
        select_place(
            c,
            event,
            meeting.place_id,
            Some((meeting.map_revision, meeting.place_revision)),
        )
        .await?;
        // Commit-time protection covers a meeting that expires during later work.
        crate::community::policy::pin_effect_until(c, meeting.expires_at).await
    }
}
async fn select_place(
    c: &mut PgConnection,
    event: Uuid,
    place: Uuid,
    expected: Option<(i64, i64)>,
) -> Result<(), CommandError> {
    if place.is_nil() {
        return Err(CommandError::InvalidInput);
    }
    let mut savepoint = c.begin().await.map_err(db)?;
    let head=sqlx::query("select catalog_revision,map_revision from concert_events_map_releases where event_id=$1 for share")
        .bind(event).fetch_optional(&mut *savepoint).await.map_err(db)?.ok_or(CommandError::Denied)?;
    let catalog: i64 = head.try_get("catalog_revision").map_err(db)?;
    let map: i64 = head.try_get("map_revision").map_err(db)?;
    if expected.is_some_and(|(m, _)| m != map) {
        return Err(CommandError::Conflict);
    }
    let selection = PgPlaceRepository::select_physical_in_transaction(
        &mut savepoint,
        EventId::from_uuid(event),
        PlaceId::from_uuid(place).map_err(|_| CommandError::InvalidInput)?,
        Revision::new(catalog).map_err(|_| CommandError::Unavailable)?,
        map,
    )
    .await
    .map_err(|_| CommandError::Unavailable)?;
    if !matches!(selection, SelectionOutcome::Eligible(_)) {
        return Err(CommandError::Denied);
    }
    if let Some((_, revision)) = expected {
        let actual:i64=sqlx::query_scalar("select s.revision from concert_place_current_state s join concert_events_map_releases b on b.map_id=s.map_id and b.map_revision=s.map_revision where b.event_id=$1 and s.place_id=$2 for share of s")
            .bind(event).bind(place).fetch_one(&mut *savepoint).await.map_err(db)?;
        if actual != revision {
            return Err(CommandError::Conflict);
        }
    }
    // RELEASE SAVEPOINT retains owner SHARE locks through the outer transaction.
    savepoint.commit().await.map_err(db)?;
    Ok(())
}
