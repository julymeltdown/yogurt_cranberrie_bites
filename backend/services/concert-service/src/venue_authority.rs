//! Production VENUE_FRIEND_V1 composition.
//!
//! The Friend owner supplies both live follow directions plus live block and
//! deletion facts through the existing signed ReadCurrent observation. Auth
//! supplies account/session eligibility through the same bounded path. This
//! module adds only local event scope (phase and both participants); it never
//! reconstructs friendship from a directional flag or cached state.

use crate::adapters::out::relationship_authority::{
    BoundedObservation, BoundedRelationshipAuthority, RequestBinding,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::venue_meeting::{LandmarkReview, PairFacts, VenueMeetingAuthority};
use concert_domain::relationship::{Field, PolicyQuery, RelationshipError};
use concert_domain::venue_location::VenueLocationError;
use concert_domain::venue_meeting::{LandmarkSelection, MeetingError, MeetingPolicy};
use concert_domain::{AuthUserId, EventId};
use sqlx::{PgConnection, PgPool};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PairAuthority {
    pub allowed: bool,
    pub relationship_revision: i64,
}

impl PairAuthority {
    fn denied() -> Self {
        Self {
            allowed: false,
            relationship_revision: 1,
        }
    }
}

/// Request-bound owner reader. The binding is built only from verified product
/// state, so a caller cannot substitute another actor, event, session or
/// deadline. A missing binding or an unavailable owner fails closed.
#[derive(Clone)]
pub struct VenueFriendAuthority {
    pool: PgPool,
    relationships: Arc<dyn BoundedRelationshipAuthority>,
    binding: RequestBinding,
}

impl VenueFriendAuthority {
    pub fn new(
        pool: PgPool,
        relationships: Arc<dyn BoundedRelationshipAuthority>,
        binding: RequestBinding,
    ) -> Self {
        Self {
            pool,
            relationships,
            binding,
        }
    }

    /// Compose from an already verified product request. The subject is the
    /// pair peer; pair checks replace it per viewer while retaining the same
    /// authenticated actor, session family, request id and deadline.
    pub fn from_product(
        pool: PgPool,
        relationships: Arc<dyn BoundedRelationshipAuthority>,
        product: &crate::product_rpc::VerifiedProduct,
        subject: AuthUserId,
    ) -> Result<Self, VenueLocationError> {
        product
            .check()
            .map_err(|_| VenueLocationError::PolicyUnavailable)?;
        let binding = RequestBinding::from_verified_parts(
            PolicyQuery {
                event: EventId::from_uuid(product.event()),
                actor: AuthUserId::from_uuid(product.actor()),
                subject,
                field: Field::Interaction,
            },
            product.session(),
            product.request(),
            product.deadline(),
        )
        .map_err(|_| VenueLocationError::PolicyUnavailable)?;
        Ok(Self::new(pool, relationships, binding))
    }

    fn binding_for(
        &self,
        event: EventId,
        actor: AuthUserId,
        subject: AuthUserId,
        field: Field,
    ) -> Result<RequestBinding, VenueLocationError> {
        if self.binding.query().event != event || self.binding.query().actor != actor {
            return Err(VenueLocationError::PolicyUnavailable);
        }
        RequestBinding::from_verified_parts(
            PolicyQuery {
                event,
                actor,
                subject,
                field,
            },
            self.binding.session_family(),
            self.binding.request_id(),
            self.binding.deadline(),
        )
        .map_err(|_| VenueLocationError::PolicyUnavailable)
    }

    /// Current actor eligibility. SafetySelf checks this actor's live Auth
    /// session/account and the Friend deletion tombstone without inventing a
    /// pair or a peer relationship.
    pub async fn actor(&self, event: EventId, actor: AuthUserId) -> Result<(), VenueLocationError> {
        let binding = self.binding_for(event, actor, actor, Field::SafetySelf)?;
        let observation = self.observe(&binding).await?;
        if !observation.actor_allowed() || !observation.subject_allowed() {
            return Err(VenueLocationError::Denied);
        }
        Ok(())
    }

    /// VENUE_FRIEND_V1 for a pair: both live follow directions, no live block,
    /// both accounts current, both participants of an OPEN or LIVE event.
    pub async fn pair(
        &self,
        event: EventId,
        actor: AuthUserId,
        peer: AuthUserId,
        now: DateTime<Utc>,
    ) -> Result<PairAuthority, VenueLocationError> {
        self.pair_impl(event, actor, peer, now, None).await
    }

    /// Use the caller's effect transaction for local event scope, so the owner
    /// read sees the same Concert state the effect will commit against.
    pub async fn pair_in(
        &self,
        connection: &mut PgConnection,
        event: EventId,
        actor: AuthUserId,
        peer: AuthUserId,
        now: DateTime<Utc>,
    ) -> Result<PairAuthority, VenueLocationError> {
        self.pair_impl(event, actor, peer, now, Some(connection))
            .await
    }

    async fn pair_impl(
        &self,
        event: EventId,
        actor: AuthUserId,
        peer: AuthUserId,
        now: DateTime<Utc>,
        connection: Option<&mut PgConnection>,
    ) -> Result<PairAuthority, VenueLocationError> {
        if actor == peer {
            return Ok(PairAuthority::denied());
        }
        let binding = self.binding_for(event, actor, peer, Field::Interaction)?;
        let in_scope = match connection {
            Some(connection) => event_pair_in_scope(connection, event, actor, peer).await?,
            None => {
                let mut connection = self
                    .pool
                    .acquire()
                    .await
                    .map_err(|_| VenueLocationError::DependencyUnavailable)?;
                event_pair_in_scope(&mut connection, event, actor, peer).await?
            }
        };
        if !in_scope {
            return Ok(PairAuthority::denied());
        }
        let observation = match self.observe(&binding).await {
            Ok(value) => value,
            Err(VenueLocationError::Denied) => return Ok(PairAuthority::denied()),
            Err(error) => return Err(error),
        };
        if now >= observation.valid_until() {
            return Err(VenueLocationError::PolicyUnavailable);
        }
        observation
            .ensure_current(&binding, observation.revision(), Utc::now())
            .map_err(|_| VenueLocationError::PolicyUnavailable)?;
        Ok(PairAuthority {
            allowed: observation.actor_allowed()
                && observation.subject_allowed()
                && !observation.inherited_block()
                && observation.friend_mutual_follow(),
            relationship_revision: observation.revision().friend.revision,
        })
    }

    async fn observe(
        &self,
        binding: &RequestBinding,
    ) -> Result<BoundedObservation, VenueLocationError> {
        self.relationships
            .observe(binding)
            .await
            .map_err(|error| match error {
                RelationshipError::Denied => VenueLocationError::Denied,
                RelationshipError::Unavailable | RelationshipError::Stale => {
                    VenueLocationError::PolicyUnavailable
                }
            })
    }
}

async fn event_pair_in_scope(
    connection: &mut PgConnection,
    event: EventId,
    actor: AuthUserId,
    peer: AuthUserId,
) -> Result<bool, VenueLocationError> {
    let phase: Option<String> =
        sqlx::query_scalar("select phase from public.concert_events where event_id=$1")
            .bind(event.uuid())
            .fetch_optional(&mut *connection)
            .await
            .map_err(|_| VenueLocationError::DependencyUnavailable)?;
    if !matches!(phase.as_deref(), Some("OPEN") | Some("LIVE")) {
        return Ok(false);
    }
    let members: i64 = sqlx::query_scalar(
        "select count(*) from public.concert_participants
          where event_id=$1 and user_id = any($2)",
    )
    .bind(event.uuid())
    .bind(vec![actor.uuid(), peer.uuid()])
    .fetch_one(&mut *connection)
    .await
    .map_err(|_| VenueLocationError::DependencyUnavailable)?;
    Ok(members == 2)
}

#[async_trait]
impl VenueMeetingAuthority for VenueFriendAuthority {
    async fn friend(
        &self,
        event: Uuid,
        actor: Uuid,
        peer: Uuid,
    ) -> Result<PairFacts, MeetingError> {
        if event.is_nil() || actor.is_nil() || peer.is_nil() || actor == peer {
            return Err(MeetingError::InvalidInput);
        }
        match self
            .pair(
                EventId::from_uuid(event),
                AuthUserId::from_uuid(actor),
                AuthUserId::from_uuid(peer),
                Utc::now(),
            )
            .await
        {
            Ok(pair) if pair.allowed => Ok(PairFacts {
                relationship_revision: pair.relationship_revision,
            }),
            Ok(_) => Err(MeetingError::ResourceUnavailable),
            Err(
                VenueLocationError::PolicyUnavailable | VenueLocationError::DependencyUnavailable,
            ) => Err(MeetingError::PolicyUnavailable),
            Err(_) => Err(MeetingError::ResourceUnavailable),
        }
    }

    /// Meeting policy and reviewed-landmark providers are separate production
    /// inputs. This authority does not fabricate them; an absent provider stays
    /// fail-closed while the friend gate remains real.
    async fn reviewed_landmark(
        &self,
        _: Uuid,
        _: &LandmarkSelection,
    ) -> Result<LandmarkReview, MeetingError> {
        Err(MeetingError::PolicyUnavailable)
    }

    async fn policy(&self, _: Uuid) -> Result<MeetingPolicy, MeetingError> {
        Err(MeetingError::PolicyUnavailable)
    }
}
