//! Transactional relationship adapter. Production composition must bind real owner authority.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::relationship::RelationshipPolicy;
use concert_domain::relationship::*;
use concert_domain::{EventPhase, Revision};
use sqlx::{PgConnection, PgPool, Row};
use std::str::FromStr;

pub mod commands;
pub mod eligibility;

/// Request-bound authenticated actor plus fresh Auth/Friend restrictions. Implementations
/// MUST fence owner revocation until the caller transaction commits (or use an equivalent
/// qualified owner revision protocol). Profile existence or participant rows are not proof.
/// Deterministic implementations belong exclusively in tests.
#[async_trait]
pub trait FencedRelationshipAuthority: Send + Sync {
    async fn check_in(
        &self,
        connection: &mut PgConnection,
        query: PolicyQuery,
        now: DateTime<Utc>,
    ) -> Result<FreshAuthority, RelationshipError>;
}
pub struct UnavailableAuthority;
#[async_trait]
impl FencedRelationshipAuthority for UnavailableAuthority {
    async fn check_in(
        &self,
        _: &mut PgConnection,
        _: PolicyQuery,
        _: DateTime<Utc>,
    ) -> Result<FreshAuthority, RelationshipError> {
        Err(RelationshipError::Unavailable)
    }
}
pub struct PgRelationshipPolicy<A> {
    pool: PgPool,
    authority: A,
}
impl<A: FencedRelationshipAuthority> PgRelationshipPolicy<A> {
    pub fn new(pool: PgPool, authority: A) -> Self {
        Self { pool, authority }
    }
    /// Current actor/session/member only; NO relationship-pair acquisition.
    /// Use before a consumer root lock, then take full pair authorization AFTER that root.
    /// Success never authorizes a target, audience, private field or later mutation.
    pub async fn authorize_actor_in(
        &self,
        c: &mut PgConnection,
        key: concert_domain::ParticipantKey,
    ) -> Result<ActorDecision, RelationshipError> {
        let event =
            sqlx::query("select phase,revision from concert_events where event_id=$1 for share")
                .bind(key.event_id.uuid())
                .fetch_optional(&mut *c)
                .await
                .map_err(unavailable)?
                .ok_or(RelationshipError::Denied)?;
        let member = sqlx::query(
            "select user_id from concert_participants where event_id=$1 and user_id=$2 for share",
        )
        .bind(key.event_id.uuid())
        .bind(key.user_id.uuid())
        .fetch_optional(&mut *c)
        .await
        .map_err(unavailable)?
        .is_some();
        let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *c)
            .await
            .map_err(unavailable)?;
        let query = PolicyQuery {
            event: key.event_id,
            actor: key.user_id,
            subject: key.user_id,
            field: Field::SafetySelf,
        };
        let authority = self.authority.check_in(c, query, now).await?;
        let decision = decide_actor(ActorSnapshot {
            key,
            phase: EventPhase::from_str(
                &event
                    .try_get::<String, _>("phase")
                    .map_err(unavailable_data)?,
            )
            .map_err(|_| RelationshipError::Unavailable)?,
            event_revision: Revision::new(event.try_get("revision").map_err(unavailable_data)?)
                .map_err(|_| RelationshipError::Unavailable)?,
            member,
            now,
            authority,
        })?;
        let finished: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *c)
            .await
            .map_err(unavailable)?;
        if finished >= decision.valid_until() {
            return Err(RelationshipError::Unavailable);
        }
        pin_deadline(c, decision.valid_until()).await?;
        Ok(decision)
    }
    /// Caller MUST supply an open transaction and retain it through the protected effect.
    /// Lock order: event, normalized pair, sorted participants, owner authority.
    pub async fn authorize_in(
        &self,
        c: &mut PgConnection,
        query: PolicyQuery,
        expected: Option<PolicyStamp>,
    ) -> Result<PolicyDecision, RelationshipError> {
        let state = lock_state(c, query).await?;
        let authority = self.authority.check_in(c, query, state.now).await?;
        let finished: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *c)
            .await
            .map_err(unavailable)?;
        if finished >= authority.valid_until {
            return Err(RelationshipError::Unavailable);
        }
        let decision = decide(PolicySnapshot { authority, ..state })?;
        pin_deadline(c, decision.valid_until()).await?;
        if expected.is_some_and(|value| value != decision.stamp()) {
            return Err(RelationshipError::Stale);
        }
        Ok(decision)
    }
    pub(crate) async fn safety_authority(
        &self,
        c: &mut PgConnection,
        query: PolicyQuery,
    ) -> Result<PolicySnapshot, RelationshipError> {
        let mut state = lock_state(c, query).await?;
        state.authority = self.authority.check_in(c, query, state.now).await?;
        let a = state.authority;
        let finished: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *c)
            .await
            .map_err(unavailable)?;
        if a.query != query || a.checked_at != state.now || a.valid_until <= finished {
            return Err(RelationshipError::Unavailable);
        }
        // Blocking/unblocking is a safety operation even if the subject is suspended/blocked.
        if !state.actor_member
            || !a.actor_allowed
            || matches!(state.phase, EventPhase::Draft | EventPhase::Archived)
        {
            return Err(RelationshipError::Denied);
        }
        pin_deadline(c, a.valid_until).await?;
        Ok(state)
    }
}
#[async_trait]
impl<A: FencedRelationshipAuthority> RelationshipPolicy for PgRelationshipPolicy<A> {
    async fn authorize(&self, query: PolicyQuery) -> Result<PolicyDecision, RelationshipError> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        let decision = self.authorize_in(&mut tx, query, None).await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(decision)
    }
}
pub(crate) async fn lock_state(
    c: &mut PgConnection,
    query: PolicyQuery,
) -> Result<PolicySnapshot, RelationshipError> {
    let event =
        sqlx::query("select phase,revision from public.concert_events where event_id=$1 for share")
            .bind(query.event.uuid())
            .fetch_optional(&mut *c)
            .await
            .map_err(unavailable)?
            .ok_or(RelationshipError::Denied)?;
    let (low, high) = ordered(query);
    sqlx::query("insert into public.concert_relationship_pairs(event_id,low_user,high_user) values($1,$2,$3) on conflict do nothing")
        .bind(query.event.uuid()).bind(low).bind(high).execute(&mut *c).await.map_err(unavailable)?;
    let pair = sqlx::query("select revision,low_blocks_high,high_blocks_low from public.concert_relationship_pairs where event_id=$1 and low_user=$2 and high_user=$3 for update")
        .bind(query.event.uuid()).bind(low).bind(high).fetch_one(&mut *c).await.map_err(unavailable)?;
    let members: Vec<uuid::Uuid> = sqlx::query_scalar("select user_id from public.concert_participants where event_id=$1 and user_id in ($2,$3) order by user_id for share")
        .bind(query.event.uuid()).bind(low).bind(high).fetch_all(&mut *c).await.map_err(unavailable)?;
    let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&mut *c)
        .await
        .map_err(unavailable)?;
    let revision = Revision::new(pair.try_get("revision").map_err(unavailable_data)?)
        .map_err(|_| RelationshipError::Unavailable)?;
    Ok(PolicySnapshot {
        query,
        phase: EventPhase::from_str(
            &event
                .try_get::<String, _>("phase")
                .map_err(unavailable_data)?,
        )
        .map_err(|_| RelationshipError::Unavailable)?,
        event_revision: Revision::new(event.try_get("revision").map_err(unavailable_data)?)
            .map_err(|_| RelationshipError::Unavailable)?,
        relationship_revision: revision,
        actor_member: members.contains(&query.actor.uuid()),
        subject_member: members.contains(&query.subject.uuid()),
        blocked: pair
            .try_get::<bool, _>("low_blocks_high")
            .map_err(unavailable_data)?
            || pair
                .try_get::<bool, _>("high_blocks_low")
                .map_err(unavailable_data)?,
        now,
        authority: FreshAuthority {
            query,
            checked_at: now,
            valid_until: now,
            revision,
            actor_allowed: false,
            subject_allowed: false,
            inherited_block: true,
        },
    })
}
pub(crate) fn ordered(query: PolicyQuery) -> (uuid::Uuid, uuid::Uuid) {
    let a = query.actor.uuid();
    let b = query.subject.uuid();
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}
pub(crate) fn unavailable(_: sqlx::Error) -> RelationshipError {
    RelationshipError::Unavailable
}
fn unavailable_data(_: sqlx::Error) -> RelationshipError {
    RelationshipError::Unavailable
}
pub mod crypto;
pub mod http;
pub mod social;

/// Install the earliest trusted deadline for deferred SQL guards at BE15 commit.
/// This setting is not authentication; only verified owner ports call this helper.
pub(crate) async fn pin_deadline(
    c: &mut PgConnection,
    until: DateTime<Utc>,
) -> Result<(), RelationshipError> {
    sqlx::query("select set_config('mcr.relationship_authority_until', least(coalesce(nullif(current_setting('mcr.relationship_authority_until',true),'')::timestamptz,'infinity'::timestamptz),$1::timestamptz)::text,true)")
        .bind(until).execute(c).await.map_err(unavailable)?;
    Ok(())
}
