//! The retained producer calls this port on its actual effects/audit connection.
//! No second transaction, synthetic scope or LocalPublic prepared event lock.
use super::{authority_error, unavailable, ScopedOperationsRoles};
use crate::adapters::out::{
    relationship_authority::guarded::OwnerGuardScope,
    repository::event_operations::FencedOperationsAuthority,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{
    admin::{Action, AuthorityQuery, Role, RoleLease},
    commands::CommandError,
};
use concert_domain::{
    relationship::{Field, PolicyQuery},
    AuthUserId, EventId, EventPhase,
};
use sqlx::{PgConnection, Row};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct CanonicalAuthority {
    pub scope: Arc<OwnerGuardScope>,
    pub roles: Option<Arc<dyn ScopedOperationsRoles>>,
    pub request_digest: [u8; 32],
    pub request_key: Option<uuid::Uuid>,
    pub operation: &'static str,
}
impl CanonicalAuthority {
    pub(super) async fn check_scope(&self) -> Result<(), CommandError> {
        self.scope.binding().check().map_err(authority_error)?;
        self.scope
            .check_subject(PolicyQuery {
                actor: AuthUserId::from_uuid(self.scope.binding().actor()),
                event: EventId::from_uuid(self.scope.binding().event()),
                subject: AuthUserId::from_uuid(self.scope.binding().actor()),
                field: Field::SafetySelf,
            })
            .await
            .map_err(authority_error)
    }
}
#[async_trait]
impl FencedOperationsAuthority for CanonicalAuthority {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        query: AuthorityQuery,
        _: DateTime<Utc>,
    ) -> Result<RoleLease, CommandError> {
        let binding = self.scope.binding();
        binding.check().map_err(authority_error)?;
        // Place owner repeats a read authorization on the same write transaction.
        let allowed_place_read =
            self.operation == Action::PlaceWrite.operation() && query.action == Action::PlacesRead;
        if query.actor.actor != binding.actor()
            || query.actor.event != binding.event()
            || (query.action.operation() != self.operation && !allowed_place_read)
        {
            return Err(CommandError::Denied);
        }
        let stop = self.scope.disclosure_ceiling().map_err(authority_error)?;
        let remaining = stop
            .remaining(std::time::Duration::from_secs(5), stop.expires_micros())
            .map_err(authority_error)?;
        let millis = remaining.as_millis();
        if millis == 0 {
            return Err(CommandError::Unavailable);
        }
        sqlx::query("select set_config('statement_timeout',(extract(epoch from least(coalesce(nullif(current_setting('statement_timeout')::interval,interval '0'),$1::text::interval),$1::text::interval))*1000)::bigint::text||'ms',true)")
            .bind(format!("{millis}ms"))
            .execute(&mut *c)
            .await
            .map_err(unavailable)?;
        // Registration captures this connection's actual full XID and both owner
        // Track receipts before admission/role checks and all receipt effects.
        let native =
            super::settlement::register_in(&self.scope, c, self.request_digest, self.request_key)
                .await?;
        self.check_scope().await?;
        let event =
            sqlx::query("select phase,revision from concert_events where event_id=$1 for share")
                .bind(binding.event())
                .fetch_optional(&mut *c)
                .await
                .map_err(unavailable)?
                .ok_or(CommandError::Denied)?;
        let phase: EventPhase = event
            .try_get::<String, _>("phase")
            .map_err(unavailable)?
            .parse()
            .map_err(unavailable)?;
        if phase == EventPhase::Archived {
            return Err(CommandError::Denied);
        }
        // Admission is current local state, not operator authority. Its SHARE
        // lock is retained by the existing repository through the same commit.
        let admission = sqlx::query("select state,restriction_revision from concert_event_admissions where event_id=$1 and user_id=$2 for share")
            .bind(binding.event()).bind(binding.actor()).fetch_optional(&mut *c).await.map_err(unavailable)?.ok_or(CommandError::Denied)?;
        if admission
            .try_get::<String, _>("state")
            .map_err(unavailable)?
            != "ACTIVE"
        {
            return Err(CommandError::Denied);
        }
        let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *c)
            .await
            .map_err(unavailable)?;
        let expiry = DateTime::from_timestamp_micros(stop.expires_micros())
            .ok_or(CommandError::Unavailable)?;
        let mut lease = if query.action.permits(Role::Member) {
            RoleLease {
                query,
                role: Role::Member,
                revision: admission
                    .try_get("restriction_revision")
                    .map_err(unavailable)?,
                checked_at: now,
                expires_at: expiry,
            }
        } else {
            // A membership row, JWT or bootstrap never promotes to Operator.
            self.roles
                .as_ref()
                .ok_or(CommandError::Unavailable)?
                .check_in(&self.scope, c, query, now)
                .await?
        };
        let finished: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *c)
            .await
            .map_err(unavailable)?;
        lease.expires_at = lease.expires_at.min(expiry);
        lease.validate(query, now, finished)?;
        self.check_scope().await?;
        super::settlement::witness_in(&self.scope, c, &native, self.request_key).await?;
        Ok(lease)
    }
}
