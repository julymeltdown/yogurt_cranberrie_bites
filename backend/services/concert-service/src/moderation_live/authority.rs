//! Canonical same-transaction adapter; no role acquisition or second Root scope.
use super::{ports::*, wire::ModerationRequest};
use crate::{
    adapters::out::relationship_authority::guarded::{GuardedAuthority, OwnerGuardScope},
    relationship::FencedRelationshipAuthority,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{
    admin::{Action, Actor, AuthorityQuery, RoleLease},
    commands::CommandError,
};
use concert_domain::{
    relationship::{Field, PolicyQuery},
    AuthUserId, EventId,
};
use sqlx::{PgConnection, Row};
use std::sync::Arc;
use uuid::Uuid;
#[derive(Clone)]
pub struct CanonicalModerationAuthority {
    scope: Arc<OwnerGuardScope>,
    roles: Arc<dyn EventRoleFence>,
    action: Action,
    target: Uuid,
    digest: [u8; 32],
}
impl CanonicalModerationAuthority {
    pub(crate) fn new(
        scope: Arc<OwnerGuardScope>,
        roles: Arc<dyn EventRoleFence>,
        request: &ModerationRequest,
    ) -> Result<Self, CommandError> {
        scope.binding().check().map_err(status)?;
        if roles.root_binding_digest() != scope.binding().digest()
            || request.action.operation() != scope.binding().operation()
        {
            return Err(CommandError::Denied);
        }
        let target = request
            .change
            .as_ref()
            .map(|c| c.target())
            .unwrap_or(scope.binding().event());
        Ok(Self {
            scope,
            roles,
            action: request.action,
            target,
            digest: request.digest,
        })
    }
}
#[async_trait]
impl ModerationAuthority for CanonicalModerationAuthority {
    fn canonical_scope(&self) -> Option<Arc<OwnerGuardScope>> {
        Some(self.scope.clone())
    }
    async fn check_in(
        &self,
        c: &mut PgConnection,
        q: AuthorityQuery,
        _: DateTime<Utc>,
    ) -> Result<RoleLease, CommandError> {
        let b = self.scope.binding();
        b.check().map_err(status)?;
        if q.actor.actor != b.actor()
            || q.actor.event != b.event()
            || q.action != self.action
            || q.target != self.target
            || self.roles.root_binding_digest() != b.digest()
        {
            return Err(CommandError::Denied);
        }
        self.roles.check_disclosure().await?;
        let until = self.scope.disclosure_ceiling().map_err(status)?;
        let remaining = until
            .remaining(std::time::Duration::from_secs(5), until.expires_micros())
            .map_err(status)?
            .as_millis();
        if remaining == 0 {
            return Err(CommandError::Unavailable);
        }
        sqlx::query("select set_config('statement_timeout',(extract(epoch from least(coalesce(nullif(current_setting('statement_timeout')::interval,interval '0'),$1::text::interval),$1::text::interval))*1000)::bigint::text||'ms',true)").bind(format!("{remaining}ms")).execute(&mut *c).await.map_err(db)?;
        self.scope.register_transaction_in(c,self.digest,serde_json::to_vec(&serde_json::json!({"kind":"MODERATION_V1","request_id":b.request_id(),"operation":b.operation()})).map_err(|_|CommandError::Unavailable)?).await.map_err(status)?;
        GuardedAuthority::new(self.scope.clone())
            .check_in(
                c,
                PolicyQuery {
                    actor: AuthUserId::from_uuid(b.actor()),
                    event: EventId::from_uuid(b.event()),
                    subject: AuthUserId::from_uuid(b.actor()),
                    field: Field::SafetySelf,
                },
                Utc::now(),
            )
            .await
            .map_err(|_| CommandError::Unavailable)?;
        let admission=sqlx::query("select state from public.concert_event_admissions where event_id=$1 and user_id=$2 for share").bind(b.event()).bind(b.actor()).fetch_optional(&mut *c).await.map_err(db)?.ok_or(CommandError::Denied)?;
        if admission.try_get::<String, _>("state").map_err(db)? != "ACTIVE" {
            return Err(CommandError::Denied);
        }
        let started = crate::adapters::out::repository::event_operations::now(c).await?;
        let mut role = self.roles.check_in(c, q, started).await?;
        role.expires_at = role.expires_at.min(
            DateTime::from_timestamp_micros(until.expires_micros())
                .ok_or(CommandError::Unavailable)?,
        );
        role.validate(
            q,
            started,
            crate::adapters::out::repository::event_operations::now(c).await?,
        )?;
        self.roles.check_disclosure().await?;
        Ok(role)
    }
    async fn target_role_in(
        &self,
        c: &mut PgConnection,
        actor: Actor,
        target: Uuid,
        now: DateTime<Utc>,
    ) -> Result<TargetRoleLease, CommandError> {
        if self.action != Action::ModerateUser
            || actor.actor != self.scope.binding().actor()
            || actor.event != self.scope.binding().event()
            || target != self.target
        {
            return Err(CommandError::Denied);
        }
        self.roles.target_role_in(c, actor, target, now).await
    }
    async fn check_disclosure(&self) -> Result<(), CommandError> {
        self.scope.disclosure_ceiling().map_err(status)?;
        self.roles.check_disclosure().await
    }
}
fn db(_: impl std::fmt::Display) -> CommandError {
    CommandError::Unavailable
}
fn status(e: tonic::Status) -> CommandError {
    match e.code() {
        tonic::Code::PermissionDenied | tonic::Code::Unauthenticated | tonic::Code::NotFound => {
            CommandError::Denied
        }
        _ => CommandError::Unavailable,
    }
}
