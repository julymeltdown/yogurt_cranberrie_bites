//! Actual Concert event/feature/place locks for the Party producer.
//! The resource owner grants no authority itself: it only checks the current
//! event phase, the `parties` feature switch and a reviewed physical place.
use crate::{
    community::{giveaway::GiftSettings, policy::CommunityResources, CommunityAccess},
    gift_live::resources::select_place,
};
use async_trait::async_trait;
use concert_application::commands::CommandError;
use concert_domain::community::giveaway::PickupMeeting;
use sqlx::{PgConnection, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgPartyResources;

fn db(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}

#[async_trait]
impl CommunityResources for PgPartyResources {
    async fn feature_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        _actor: Uuid,
        access: CommunityAccess,
    ) -> Result<(), CommandError> {
        let row = sqlx::query(
            "select e.phase,s.parties from concert_events e join concert_event_settings s using(event_id) where e.event_id=$1 for share of e,s",
        )
        .bind(event)
        .fetch_optional(c)
        .await
        .map_err(db)?
        .ok_or(CommandError::Unavailable)?;
        let phase: String = row.try_get("phase").map_err(db)?;
        let enabled: bool = row.try_get("parties").map_err(db)?;
        // Existing safety commands remain available after the feature stops.
        if !matches!(phase.as_str(), "OPEN" | "LIVE" | "READ_ONLY")
            || (access == CommunityAccess::WriteParties && phase == "READ_ONLY")
            || (!enabled && access != CommunityAccess::SafetyParties)
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
        select_place(c, event, place, None).await
    }

    async fn gift_settings_in(
        &self,
        _: &mut PgConnection,
        _: Uuid,
    ) -> Result<GiftSettings, CommandError> {
        Err(CommandError::Denied)
    }

    async fn gift_media_in(
        &self,
        _: &mut PgConnection,
        _: Uuid,
        _: Uuid,
        _: Uuid,
        _: Option<Uuid>,
    ) -> Result<(), CommandError> {
        Err(CommandError::Denied)
    }

    async fn pickup_meeting_in(
        &self,
        _: &mut PgConnection,
        _: Uuid,
        _: &PickupMeeting,
    ) -> Result<(), CommandError> {
        Err(CommandError::Denied)
    }
}
