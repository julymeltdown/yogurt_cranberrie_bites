//! Shared BE37 adapter plus explicit feature/place/media owner ports. No fallback grants.
use super::{
    giveaway::{GiftAuthority, GiftSettings},
    CommunityAccess, CommunityAuthority,
};
use crate::relationship::{FencedRelationshipAuthority, PgRelationshipPolicy};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::commands::CommandError;
use concert_domain::community::giveaway::PickupMeeting;
use concert_domain::{
    relationship::{Field, PolicyQuery, RelationshipError},
    AuthUserId, EventId,
};
use sqlx::PgConnection;
use uuid::Uuid;
#[async_trait]
pub trait CommunityResources: Send + Sync {
    async fn feature_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        actor: Uuid,
        access: CommunityAccess,
    ) -> Result<(), CommandError>;
    async fn place_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        place: Uuid,
    ) -> Result<(), CommandError>;
    async fn gift_settings_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
    ) -> Result<GiftSettings, CommandError>;
    async fn gift_media_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        actor: Uuid,
        giveaway: Uuid,
        media: Option<Uuid>,
    ) -> Result<(), CommandError>;
    async fn pickup_meeting_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        meeting: &PickupMeeting,
    ) -> Result<(), CommandError>;
}
pub struct SharedCommunityAuthority<A, R> {
    relationship: PgRelationshipPolicy<A>,
    resources: R,
}
impl<A, R> SharedCommunityAuthority<A, R> {
    pub fn new(relationship: PgRelationshipPolicy<A>, resources: R) -> Self {
        Self {
            relationship,
            resources,
        }
    }
}
#[async_trait]
impl<A: FencedRelationshipAuthority, R: CommunityResources> CommunityAuthority
    for SharedCommunityAuthority<A, R>
{
    async fn actor_in(
        &self,
        c: &mut PgConnection,
        actor: Uuid,
        event: Uuid,
        access: CommunityAccess,
    ) -> Result<(), CommandError> {
        self.resources.feature_in(c, event, actor, access).await?;
        let key = concert_domain::ParticipantKey {
            event_id: EventId::from_uuid(event),
            user_id: AuthUserId::from_uuid(actor),
        };
        let decision = self
            .relationship
            .authorize_actor_in(c, key)
            .await
            .map_err(|e| match e {
                RelationshipError::Denied => CommandError::Denied,
                _ => CommandError::Unavailable,
            })?;
        if decision.key() != key {
            return Err(CommandError::Denied);
        }
        pin_authority_until(c, decision.valid_until()).await
    }
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        actor: Uuid,
        event: Uuid,
        subject: Uuid,
        access: CommunityAccess,
    ) -> Result<(), CommandError> {
        self.resources.feature_in(c, event, actor, access).await?;
        let field = match access {
            CommunityAccess::ReadParties | CommunityAccess::ReadGifts => Field::PublicProfile,
            CommunityAccess::WriteParties | CommunityAccess::WriteGifts => Field::Interaction,
            _ => {
                if actor == subject {
                    Field::SafetySelf
                } else {
                    Field::SafetyInteraction
                }
            }
        };
        let query = PolicyQuery {
            event: EventId::from_uuid(event),
            actor: AuthUserId::from_uuid(actor),
            subject: AuthUserId::from_uuid(subject),
            field,
        };
        let decision = self
            .relationship
            .authorize_in(c, query, None)
            .await
            .map_err(|e| match e {
                RelationshipError::Denied => CommandError::Denied,
                RelationshipError::Stale | RelationshipError::Unavailable => {
                    CommandError::Unavailable
                }
            })?;
        if decision.query() != query {
            return Err(CommandError::Denied);
        }
        pin_authority_until(c, decision.valid_until()).await
    }
    async fn place_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        place: Uuid,
    ) -> Result<(), CommandError> {
        self.resources.place_in(c, event, place).await
    }
}
#[async_trait]
impl<A: FencedRelationshipAuthority, R: CommunityResources> GiftAuthority
    for SharedCommunityAuthority<A, R>
{
    async fn settings_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
    ) -> Result<GiftSettings, CommandError> {
        self.resources.gift_settings_in(c, event).await
    }
    async fn media_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        actor: Uuid,
        giveaway: Uuid,
        media: Option<Uuid>,
    ) -> Result<(), CommandError> {
        self.resources
            .gift_media_in(c, event, actor, giveaway, media)
            .await
    }
    async fn meeting_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        meeting: &PickupMeeting,
    ) -> Result<(), CommandError> {
        self.resources.pickup_meeting_in(c, event, meeting).await
    }
}
async fn pin(c: &mut PgConnection, name: &str, until: DateTime<Utc>) -> Result<(), CommandError> {
    sqlx::query("select set_config($1,least(coalesce(nullif(current_setting($1,true),'')::timestamptz,'infinity'::timestamptz),$2::timestamptz)::text,true)").bind(name).bind(until).execute(c).await.map_err(super::db)?;
    Ok(())
}
pub(crate) async fn pin_authority_until(
    c: &mut PgConnection,
    until: DateTime<Utc>,
) -> Result<(), CommandError> {
    pin(c, "mcr.community_authority_until", until).await
}
pub(crate) async fn pin_effect_until(
    c: &mut PgConnection,
    until: DateTime<Utc>,
) -> Result<(), CommandError> {
    pin(c, "mcr.community_effect_until", until).await
}
