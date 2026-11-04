//! Real providers are explicit dependencies. There is no positive default for
//! operator assignment, report target/media evidence, or reviewed notice sources.
use crate::adapters::out::relationship_authority::guarded::OwnerGuardScope;
use crate::adapters::out::repository::{
    event_operations::{
        FencedNoticeReview, FencedPlaceOperations, NoticeReviewLease, UnavailableReview,
    },
    report::{evidence_digest, EvidenceLease, FencedReportEvidence},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{
    admin::{notices::NoticeInput, Actor, AuthorityQuery, RoleLease},
    commands::CommandError,
};
use concert_domain::{
    relationship::{Field, PolicyQuery},
    report::{CreateReport, TargetType},
    AuthUserId, EventId,
};
use sqlx::PgConnection;
use std::sync::Arc;
use uuid::Uuid;

/// The provider receives the existing request scope, never a second acquisition.
/// It must lock current role/revocation/revision state on the caller transaction.
/// For a disclosed privileged read it must also retain its actual owning fence
/// through Root's delivery terminal, using supported durable scope settlement.
/// A detached role snapshot or a fresh scope created here is not qualified.
#[async_trait]
pub trait ScopedOperationsRoles: Send + Sync {
    async fn check_in(
        &self,
        scope: &Arc<OwnerGuardScope>,
        c: &mut PgConnection,
        query: AuthorityQuery,
        now: DateTime<Utc>,
    ) -> Result<RoleLease, CommandError>;
}
/// Typed target/READY media providers use this same canonical scope to enroll
/// actual target owners and retain every native target/media fence through the
/// caller transaction. Missing or recent-target evidence is never synthesized.
#[async_trait]
pub trait ScopedReportEvidence: Send + Sync {
    async fn check_in(
        &self,
        scope: &Arc<OwnerGuardScope>,
        c: &mut PgConnection,
        actor: Actor,
        input: &CreateReport,
        now: DateTime<Utc>,
    ) -> Result<EvidenceLease, CommandError>;
}

#[derive(Clone)]
pub struct OperationsOwners {
    /// A real owner must lock its current event/actor role and revocation state
    /// on the caller transaction, or retain an actual owning-service fence.
    /// Absence disables only privileged operations; it never implies Operator.
    pub roles: Option<Arc<dyn ScopedOperationsRoles>>,
    /// Additional target owners and READY REPORT media; the built-in no-media
    /// USER path uses actual canonical owners and the current event admission.
    pub evidence: Option<Arc<dyn ScopedReportEvidence>>,
    pub review: Arc<dyn FencedNoticeReview>,
    /// Optional explicit replacement; absence uses the retained SQL place owner
    /// with this request's canonical authority, never a successful stub.
    pub places: Option<Arc<dyn FencedPlaceOperations>>,
}
impl Default for OperationsOwners {
    fn default() -> Self {
        Self {
            roles: None,
            evidence: None,
            review: Arc::new(UnavailableReview),
            places: None,
        }
    }
}

#[derive(Clone)]
pub(super) struct Evidence {
    pub scope: Arc<OwnerGuardScope>,
    pub provider: Option<Arc<dyn ScopedReportEvidence>>,
}
#[async_trait]
impl FencedReportEvidence for Evidence {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        actor: Actor,
        input: &CreateReport,
        now: DateTime<Utc>,
    ) -> Result<EvidenceLease, CommandError> {
        let binding = self.scope.binding();
        binding.check().map_err(super::authority_error)?;
        if actor.actor != binding.actor() || actor.event != binding.event() {
            return Err(CommandError::Denied);
        }
        if let Some(provider) = &self.provider {
            let mut lease = provider.check_in(&self.scope, c, actor, input, now).await?;
            let ceiling = self
                .scope
                .disclosure_ceiling()
                .map_err(super::authority_error)?;
            lease.expires_at = lease.expires_at.min(
                DateTime::from_timestamp_micros(ceiling.expires_micros())
                    .ok_or(CommandError::Unavailable)?,
            );
            return Ok(lease);
        }
        if input.target_type != TargetType::User || !input.evidence_media_ids.is_empty() {
            return Err(CommandError::Unavailable);
        }
        let query = PolicyQuery {
            actor: AuthUserId::from_uuid(actor.actor),
            event: EventId::from_uuid(actor.event),
            subject: AuthUserId::from_uuid(input.target_id),
            field: Field::SafetyInteraction,
        };
        self.scope
            .check_subject(query)
            .await
            .map_err(super::authority_error)?;
        let target: Option<Uuid> = sqlx::query_scalar("select user_id from concert_event_admissions where event_id=$1 and user_id=$2 for share")
            .bind(actor.event).bind(input.target_id).fetch_optional(&mut *c).await.map_err(super::unavailable)?;
        if target != Some(input.target_id) {
            return Err(CommandError::Denied);
        }
        self.scope
            .check_subject(query)
            .await
            .map_err(super::authority_error)?;
        let checked_at = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *c)
            .await
            .map_err(super::unavailable)?;
        let ceiling = self
            .scope
            .disclosure_ceiling()
            .map_err(super::authority_error)?;
        Ok(EvidenceLease {
            actor,
            input_digest: evidence_digest(input)?,
            revision: self
                .scope
                .authority_revision()
                .map_err(super::authority_error)?,
            checked_at,
            expires_at: DateTime::from_timestamp_micros(ceiling.expires_micros())
                .ok_or(CommandError::Unavailable)?,
        })
    }
}
#[derive(Clone)]
pub(super) struct Review(pub Arc<dyn FencedNoticeReview>);
#[async_trait]
impl FencedNoticeReview for Review {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        actor: Actor,
        notice: Uuid,
        input: &NoticeInput,
        now: DateTime<Utc>,
    ) -> Result<NoticeReviewLease, CommandError> {
        self.0.check_in(c, actor, notice, input, now).await
    }
}
#[derive(Clone)]
pub(super) struct Places(pub Arc<dyn FencedPlaceOperations>);
#[async_trait]
impl FencedPlaceOperations for Places {
    async fn list_in(
        &self,
        c: &mut PgConnection,
        actor: Actor,
        after: Option<Uuid>,
        closed: Option<bool>,
        limit: u16,
    ) -> Result<Vec<crate::adapters::out::repository::event_operations::AdminPlace>, CommandError>
    {
        self.0.list_in(c, actor, after, closed, limit).await
    }
    async fn update_in(
        &self,
        c: &mut PgConnection,
        actor: Actor,
        change: &concert_application::admin::places::PlaceChange,
    ) -> Result<
        Result<
            crate::adapters::out::repository::event_operations::AdminPlace,
            concert_application::commands::SafeFailure,
        >,
        CommandError,
    > {
        self.0.update_in(c, actor, change).await
    }
}
