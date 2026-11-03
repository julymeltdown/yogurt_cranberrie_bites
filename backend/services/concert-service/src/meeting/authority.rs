//! Composition over owning BE37/community seams, never a second policy engine.
use super::{rally::RallyAuthority, MeetingAuthority};
use crate::{
    community::audience::{party_audience_in, party_meeting_in},
    relationship::{FencedRelationshipAuthority, PgRelationshipPolicy},
};
use async_trait::async_trait;
use concert_application::{beacon::*, rally::*};
use concert_domain::{
    beacon::Audience,
    relationship::{Field, PolicyQuery, RelationshipError},
    AuthUserId, EventId, ParticipantKey,
};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

/// Construct per authenticated request/session. Bound actor is resolved by the
/// canonical transport, and the injected owning authority must fence revocation.
pub struct PgMeetingAuthority<A> {
    policy: PgRelationshipPolicy<A>,
    actor: MeetingActor,
}
impl<A: FencedRelationshipAuthority> PgMeetingAuthority<A> {
    pub fn new(pool: PgPool, actor: MeetingActor, authority: A) -> Self {
        Self {
            policy: PgRelationshipPolicy::new(pool, authority),
            actor,
        }
    }
    fn bind(&self, actor: MeetingActor) -> Result<(), MeetingError> {
        if actor == self.actor {
            Ok(())
        } else {
            Err(MeetingError::ResourceUnavailable)
        }
    }
    async fn pair(
        &self,
        c: &mut PgConnection,
        actor: MeetingActor,
        subject: Uuid,
        field: Field,
    ) -> Result<(), MeetingError> {
        self.bind(actor)?;
        self.policy
            .authorize_in(
                c,
                PolicyQuery {
                    event: EventId::from_uuid(actor.event),
                    actor: AuthUserId::from_uuid(actor.actor),
                    subject: AuthUserId::from_uuid(subject),
                    field,
                },
                None,
            )
            .await
            .map_err(policy)?;
        Ok(())
    }
}
fn policy(error: RelationshipError) -> MeetingError {
    match error {
        RelationshipError::Denied | RelationshipError::Stale => MeetingError::ResourceUnavailable,
        RelationshipError::Unavailable => MeetingError::PolicyUnavailable,
    }
}
fn membership(error: concert_application::commands::CommandError) -> MeetingError {
    match error {
        concert_application::commands::CommandError::Denied => MeetingError::ResourceUnavailable,
        _ => MeetingError::PolicyUnavailable,
    }
}
#[async_trait]
impl<A: FencedRelationshipAuthority> MeetingAuthority for PgMeetingAuthority<A> {
    async fn actor_in(
        &self,
        c: &mut PgConnection,
        actor: MeetingActor,
    ) -> Result<(), MeetingError> {
        self.bind(actor)?;
        // This narrow owning check deliberately does NOT lock the self pair.
        // Full pair policy comes only after the current party root below.
        self.policy
            .authorize_actor_in(
                c,
                ParticipantKey {
                    event_id: EventId::from_uuid(actor.event),
                    user_id: AuthUserId::from_uuid(actor.actor),
                },
            )
            .await
            .map_err(policy)?;
        Ok(())
    }
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        q: AudienceQuery,
    ) -> Result<AudienceGrant, MeetingError> {
        self.actor_in(c, q.actor).await?;
        if q.operation == MeetingOperation::Revoke {
            if q.actor.actor != q.owner {
                return Err(MeetingError::ResourceUnavailable);
            }
            return Ok(AudienceGrant {
                query: q,
                party: None,
            });
        }
        let writing = matches!(
            q.operation,
            MeetingOperation::Create | MeetingOperation::Update
        );
        if writing && q.owner != q.actor.actor {
            return Err(MeetingError::ResourceUnavailable);
        }
        let field = if writing {
            Field::Interaction
        } else {
            Field::SafetyInteraction
        };
        let mut recipients = vec![q.owner];
        let fence = match &q.audience {
            Audience::PartyMembers { party_id } => {
                let snapshot = party_meeting_in(
                    c,
                    q.actor.event,
                    *party_id,
                    q.actor.actor,
                    q.expected_party.map(|f| f.audience_revision),
                )
                .await
                .map_err(membership)?;
                // Owner membership is current too; an owner's removed beacon cannot survive.
                party_audience_in(
                    c,
                    q.actor.event,
                    *party_id,
                    q.owner,
                    Some(snapshot.audience.audience_revision),
                )
                .await
                .map_err(membership)?;
                if writing {
                    recipients.extend(snapshot.members.iter().map(|m| m.user));
                }
                Some(PartyFence {
                    party: *party_id,
                    meeting_revision: snapshot.audience.meeting_revision,
                    audience_revision: snapshot.audience.audience_revision,
                })
            }
            Audience::ExplicitUsers { user_ids } => {
                if writing {
                    recipients.extend(user_ids.iter().copied());
                } else if q.actor.actor != q.owner && !user_ids.contains(&q.actor.actor) {
                    return Err(MeetingError::ResourceUnavailable);
                }
                None
            }
        };
        if q.expected_party
            .is_some_and(|expected| Some(expected) != fence)
        {
            return Err(MeetingError::ResourceUnavailable);
        }
        // Normalize all actor/recipient pairs in a stable order, after party root.
        recipients.sort_unstable();
        recipients.dedup();
        for peer in recipients {
            self.pair(c, q.actor, peer, field).await?;
        }
        Ok(AudienceGrant {
            query: q,
            party: fence,
        })
    }
}
#[async_trait]
impl<A: FencedRelationshipAuthority> RallyAuthority for PgMeetingAuthority<A> {
    async fn party_in(
        &self,
        c: &mut PgConnection,
        q: RallyQuery,
    ) -> Result<PartyMeeting, MeetingError> {
        self.bind(q.actor)?;
        let value = party_meeting_in(c, q.actor.event, q.party, q.actor.actor, None)
            .await
            .map_err(membership)?;
        let a = value.audience;
        Ok(PartyMeeting {
            query: q,
            owner: a.owner,
            meeting_revision: a.meeting_revision,
            audience_revision: a.audience_revision,
            public_place_id: a.public_place_id,
            ends_at: a.meeting_ends_at,
            members: value
                .members
                .into_iter()
                .map(|m| RallyMember {
                    user: m.user,
                    membership_revision: m.revision,
                })
                .collect(),
        })
    }
    async fn peer_in(
        &self,
        c: &mut PgConnection,
        q: RallyQuery,
        peer: Uuid,
    ) -> Result<(), MeetingError> {
        self.bind(q.actor)?;
        party_audience_in(c, q.actor.event, q.party, peer, None)
            .await
            .map_err(membership)?;
        self.pair(c, q.actor, peer, Field::SafetyInteraction).await
    }
}
