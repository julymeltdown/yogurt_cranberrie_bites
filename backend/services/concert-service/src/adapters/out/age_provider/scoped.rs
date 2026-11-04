//! Canonical scope adapters. No cached observation can construct a positive grant.
use super::trust::ClaimTrust;
use super::{
    claim_reader::{AuthorityDecision, ProjectionAuthority},
    Access, Authority,
};
use crate::{
    adapters::out::relationship_authority::guarded::{GuardedAuthority, OwnerGuardScope},
    relationship::FencedRelationshipAuthority,
};
use async_trait::async_trait;
use chrono::Utc;
use concert_application::{
    age_policy::{ClaimQuery, Context, PolicyError},
    commands::CommandError,
};
use concert_domain::relationship::{Field, PolicyQuery};
use sqlx::PgConnection;
use std::sync::Arc;

/// Reuses the consumer's already registered native transaction. In particular,
/// Intro keeps its own settlement reference and holds the complete projection.
pub struct ScopedProjectionAuthority {
    scope: Arc<OwnerGuardScope>,
    verification_available: bool,
    trust: ClaimTrust,
}
impl ScopedProjectionAuthority {
    pub fn new(scope: Arc<OwnerGuardScope>, verification_available: bool) -> Self {
        Self {
            scope,
            verification_available,
            trust: ClaimTrust::none(),
        }
    }
    pub fn with_claim_trust(
        scope: Arc<OwnerGuardScope>,
        verification_available: bool,
        trust: ClaimTrust,
    ) -> Self {
        Self {
            scope,
            verification_available,
            trust,
        }
    }
}
#[async_trait]
impl ProjectionAuthority for ScopedProjectionAuthority {
    fn disclose_until(&self, until: chrono::DateTime<Utc>) -> Result<(), PolicyError> {
        self.scope
            .limit_disclosure_until(until)
            .map_err(|_| PolicyError::Unavailable)
    }
    fn accepts_claim(&self, verifier: &str, revision: &str) -> bool {
        self.trust.accepts(verifier, revision)
    }

    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        query: ClaimQuery,
    ) -> Result<AuthorityDecision, PolicyError> {
        if query.subject != query.context.actor {
            return Err(PolicyError::Denied);
        }
        check_in(c, &self.scope, query.context, true).await?;
        Ok(AuthorityDecision {
            query,
            verification_available: self.verification_available,
        })
    }
}

/// Own age operations register their real native transaction BEFORE private
/// effects. Erasure needs canonical owner/session authority, not participation.
pub struct ScopedAgeAuthority {
    scope: Arc<OwnerGuardScope>,
}
impl ScopedAgeAuthority {
    pub fn new(scope: Arc<OwnerGuardScope>) -> Self {
        Self { scope }
    }
}
#[async_trait]
impl Authority for ScopedAgeAuthority {
    fn disclose_until(&self, until: chrono::DateTime<Utc>) -> Result<(), CommandError> {
        self.scope
            .limit_disclosure_until(until)
            .map_err(|_| CommandError::Unavailable)
    }
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        context: Context,
        access: Access,
    ) -> Result<(), CommandError> {
        let operation = self.scope.binding().operation();
        let allowed = match access {
            Access::Read => matches!(
                operation,
                "getConcertAgeStatus"
                    | "startConcertAgeVerification"
                    | "completeConcertAgeVerification"
            ),
            Access::Start => operation == "startConcertAgeVerification",
            Access::Callback => operation == "completeConcertAgeVerification",
            Access::Erase => operation == "eraseConcertAgePrivateData",
        };
        if !allowed {
            return Err(CommandError::Denied);
        }
        assert_context(&self.scope, context).map_err(command)?;
        super::settlement::register_in(c, &self.scope)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        check_in(c, &self.scope, context, access != Access::Erase)
            .await
            .map_err(command)
    }
}
fn command(e: PolicyError) -> CommandError {
    match e {
        PolicyError::Denied => CommandError::Denied,
        PolicyError::Unavailable => CommandError::Unavailable,
    }
}
fn assert_context(scope: &OwnerGuardScope, context: Context) -> Result<(), PolicyError> {
    context.check(Utc::now())?;
    let b = scope.binding();
    b.assert_context(
        context.actor.uuid(),
        context.event.uuid(),
        context.session,
        context.request,
        b.parent_digest(),
        context.expires_at,
    )
    .map_err(|_| PolicyError::Unavailable)
}
async fn check_in(
    c: &mut PgConnection,
    scope: &Arc<OwnerGuardScope>,
    context: Context,
    participant: bool,
) -> Result<(), PolicyError> {
    assert_context(scope, context)?;
    let q = PolicyQuery {
        actor: context.actor,
        subject: context.actor,
        event: context.event,
        field: Field::SafetySelf,
    };
    let now = Utc::now();
    let a = GuardedAuthority::new(scope.clone())
        .check_in(c, q, now)
        .await
        .map_err(|_| PolicyError::Unavailable)?;
    if a.query != q || a.checked_at != now || a.valid_until <= Utc::now() {
        return Err(PolicyError::Unavailable);
    }
    if !a.actor_allowed {
        return Err(PolicyError::Denied);
    }
    if participant {
        // Event before participant, retained through the caller's same COMMIT.
        let exists: Option<uuid::Uuid> =
            sqlx::query_scalar("select event_id from concert_events where event_id=$1 for share")
                .bind(context.event.uuid())
                .fetch_optional(&mut *c)
                .await
                .map_err(|_| PolicyError::Unavailable)?;
        if exists.is_none() {
            return Err(PolicyError::Denied);
        }
        let member: Option<uuid::Uuid> = sqlx::query_scalar(
            "select user_id from concert_participants where event_id=$1 and user_id=$2 for share",
        )
        .bind(context.event.uuid())
        .bind(context.actor.uuid())
        .fetch_optional(&mut *c)
        .await
        .map_err(|_| PolicyError::Unavailable)?;
        if member.is_none() {
            return Err(PolicyError::Denied);
        }
    }
    Ok(())
}
