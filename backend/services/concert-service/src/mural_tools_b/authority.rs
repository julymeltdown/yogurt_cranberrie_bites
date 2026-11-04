//! Uses Root's single original Auth/Friend owner scope, never an allow fixture.
use crate::adapters::out::{
    relationship_authority::guarded::{GuardedAuthority, OwnerGuardScope},
    repository::mural::MuralAuthority,
};
use chrono::{DateTime, Utc};
use concert_application::mural::{MuralActor, MuralError};
use concert_domain::{relationship::RelationshipError, AuthUserId, EventId, ParticipantKey};
use sqlx::{PgConnection, PgPool};
use std::sync::Arc;

pub struct ScopedMuralAuthority {
    pool: PgPool,
    scope: Arc<OwnerGuardScope>,
}
impl ScopedMuralAuthority {
    pub fn new(pool: PgPool, scope: Arc<OwnerGuardScope>) -> Self {
        Self { pool, scope }
    }
}
pub(super) fn guard_error(error: tonic::Status) -> MuralError {
    match error.code() {
        tonic::Code::PermissionDenied | tonic::Code::Unauthenticated => MuralError::Denied,
        _ => MuralError::PolicyUnavailable,
    }
}
#[async_trait::async_trait]
impl MuralAuthority for ScopedMuralAuthority {
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        actor: MuralActor,
    ) -> Result<(), MuralError> {
        let b = self.scope.binding();
        b.check().map_err(guard_error)?;
        if !actor.valid() || b.actor() != actor.actor || b.event() != actor.event {
            return Err(MuralError::Denied);
        }
        super::witness::register_wall_transaction(c, &self.scope).await?;
        let policy = crate::relationship::PgRelationshipPolicy::new(
            self.pool.clone(),
            GuardedAuthority::new(self.scope.clone()),
        );
        let decision = policy
            .authorize_actor_in(
                c,
                ParticipantKey {
                    event_id: EventId::from_uuid(actor.event),
                    user_id: AuthUserId::from_uuid(actor.actor),
                },
            )
            .await
            .map_err(|e| match e {
                RelationshipError::Denied => MuralError::Denied,
                _ => MuralError::PolicyUnavailable,
            })?;
        let active:bool=sqlx::query_scalar("select exists(select 1 from public.concert_event_admissions where event_id=$1 and user_id=$2 and state='ACTIVE' for share)")
            .bind(actor.event).bind(actor.actor).fetch_one(&mut *c).await.map_err(|_|MuralError::PolicyUnavailable)?;
        if !active {
            return Err(MuralError::Denied);
        }
        let bound = DateTime::<Utc>::from_timestamp_micros(b.deadline().expires_micros())
            .ok_or(MuralError::PolicyUnavailable)?;
        let until = bound.min(decision.valid_until());
        self.scope
            .limit_disclosure_until(until)
            .map_err(guard_error)?;
        sqlx::query("select set_config('mcr.mural_tools_b_authority_until',least(coalesce(nullif(current_setting('mcr.mural_tools_b_authority_until',true),'')::timestamptz,'infinity'::timestamptz),$1::timestamptz)::text,true)")
            .bind(until).execute(c).await.map_err(|_|MuralError::PolicyUnavailable)?;
        self.scope.binding().check().map_err(guard_error)?;
        Ok(())
    }
}
