//! Party-only resources for the existing SharedCommunityAuthority.
//! Adapted from gift_live/resources.rs at published 7d5d91bd. No Gift grants.
//! These local locks supplement the canonical Auth/Friend scope; they never
//! replace actor/subject authorization performed by SharedCommunityAuthority.
use crate::{
    adapters::out::repository::place::PgPlaceRepository,
    community::{giveaway::GiftSettings, policy::CommunityResources, CommunityAccess},
};
use async_trait::async_trait;
use concert_application::{commands::CommandError, place::SelectionOutcome};
use concert_domain::{community::giveaway::PickupMeeting, place::PlaceId, EventId, Revision};
use sqlx::{postgres::PgTransactionManager, Acquire, PgConnection, Row, TransactionManager};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub(super) struct PgPartyResources;

fn active_transaction(c: &mut PgConnection, event: Uuid) -> Result<(), CommandError> {
    if event.is_nil() || PgTransactionManager::get_transaction_depth(c) == 0 {
        return Err(CommandError::Denied);
    }
    Ok(())
}

fn feature_allowed(phase: &str, enabled: bool, access: CommunityAccess) -> bool {
    match access {
        CommunityAccess::ReadParties => enabled && matches!(phase, "OPEN" | "LIVE" | "READ_ONLY"),
        CommunityAccess::WriteParties => enabled && matches!(phase, "OPEN" | "LIVE"),
        // Existing participation may be cancelled/removed after feature writes
        // stop. The command still enforces exact actor/target ownership.
        CommunityAccess::SafetyParties => matches!(phase, "OPEN" | "LIVE" | "READ_ONLY"),
        _ => false,
    }
}

#[async_trait]
impl CommunityResources for PgPartyResources {
    async fn feature_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        actor: Uuid,
        access: CommunityAccess,
    ) -> Result<(), CommandError> {
        active_transaction(c, event)?;
        if actor.is_nil() {
            return Err(CommandError::Denied);
        }
        let row = sqlx::query("select e.phase,s.parties from public.concert_events e join public.concert_event_settings s using(event_id) where e.event_id=$1 for share of e,s")
            .bind(event).fetch_optional(c).await.map_err(db)?.ok_or(CommandError::Denied)?;
        let phase: String = row.try_get("phase").map_err(db)?;
        let enabled: bool = row.try_get("parties").map_err(db)?;
        if !feature_allowed(&phase, enabled, access) {
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
        active_transaction(c, event)?;
        if place.is_nil() {
            return Err(CommandError::InvalidInput);
        }
        // BE09 nested savepoint: successful RELEASE retains reviewed physical
        // place and release/state SHARE locks until this outer Party COMMIT.
        let mut savepoint = c.begin().await.map_err(db)?;
        let head = sqlx::query("select catalog_revision,map_revision from public.concert_events_map_releases where event_id=$1 for share")
            .bind(event).fetch_optional(&mut *savepoint).await.map_err(db)?.ok_or(CommandError::Denied)?;
        let catalog: i64 = head.try_get("catalog_revision").map_err(db)?;
        let map: i64 = head.try_get("map_revision").map_err(db)?;
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
        savepoint.commit().await.map_err(db)?;
        Ok(())
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
fn db(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn party_feature_disabled_and_read_only_preserve_only_safety_actions() {
        for phase in ["OPEN", "LIVE", "READ_ONLY"] {
            assert!(feature_allowed(
                phase,
                false,
                CommunityAccess::SafetyParties
            ));
            assert!(!feature_allowed(
                phase,
                false,
                CommunityAccess::WriteParties
            ));
            assert!(!feature_allowed(phase, false, CommunityAccess::ReadParties));
        }
        assert!(feature_allowed(
            "READ_ONLY",
            true,
            CommunityAccess::ReadParties
        ));
        assert!(!feature_allowed(
            "READ_ONLY",
            true,
            CommunityAccess::WriteParties
        ));
        for phase in ["CLOSED", "DRAFT", "FROZEN", "", "open"] {
            for access in [
                CommunityAccess::ReadParties,
                CommunityAccess::WriteParties,
                CommunityAccess::SafetyParties,
            ] {
                assert!(!feature_allowed(phase, true, access));
            }
        }
    }
    #[test]
    fn party_resources_never_enable_gift_access() {
        for phase in ["OPEN", "LIVE", "READ_ONLY"] {
            for access in [
                CommunityAccess::ReadGifts,
                CommunityAccess::WriteGifts,
                CommunityAccess::SafetyGifts,
            ] {
                assert!(!feature_allowed(phase, true, access));
            }
        }
    }
}
