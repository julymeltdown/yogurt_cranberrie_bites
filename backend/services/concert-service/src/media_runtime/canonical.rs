//! Adapter for an actual root-retained canonical Auth/Friend owner scope.
//! Root owns acquisition, durable delivery retention and HTTP terminal settlement.
use super::{
    settlement::settlement_reference, OwnerMediaBinding, OwnerMediaFence, OwnerMediaGuard,
    OwnerMediaReply, OwnerMediaWork,
};
use crate::adapters::out::relationship_authority::guarded::OwnerGuardScope;
use async_trait::async_trait;
use concert_application::commands::CommandError;
use concert_domain::{
    relationship::{Field, PolicyQuery},
    AuthUserId, EventId,
};
use sqlx::PgConnection;
use std::sync::Arc;
use tonic::{Code, Status};

/// One verified media frame and its actual canonical owner scope. Construct only
/// after root has durably retained the scope and bound its delivery capability.
/// This adapter never acquires, completes or releases canonical authority.
pub struct CanonicalOwnerMediaFence {
    scope: Arc<OwnerGuardScope>,
    expected_digest: [u8; 32],
}
impl CanonicalOwnerMediaFence {
    /// Root derives `expected` from the already verified exact Product frame,
    /// including content type and idempotency key. Copy the identity's deadline
    /// directly from `scope.binding().deadline()`; constructing a fresh budget,
    /// even with the same wall expiry, is rejected.
    pub fn from_request(
        scope: Arc<OwnerGuardScope>,
        expected: &OwnerMediaBinding,
    ) -> Result<Self, CommandError> {
        check_scope(&scope, expected)?;
        security_revision(&scope)?;
        Ok(Self {
            scope,
            expected_digest: expected.digest(),
        })
    }
}

#[async_trait]
impl OwnerMediaFence for CanonicalOwnerMediaFence {
    async fn run(
        &self,
        binding: Arc<OwnerMediaBinding>,
        work: OwnerMediaWork,
    ) -> Result<OwnerMediaReply, CommandError> {
        if binding.digest() != self.expected_digest {
            return Err(CommandError::Denied);
        }
        check_scope(&self.scope, &binding)?;
        let guard = Arc::new(CanonicalOwnerMediaGuard {
            scope: self.scope.clone(),
            expected_digest: self.expected_digest,
            revision: security_revision(&self.scope)?,
        });
        // Preserve producer errors without translating them through transport
        // status. Scope.run enforces the original canonical stop; returning here
        // does not mark serialization, finish delivery, seal or release guards.
        self.scope
            .run(async move { Ok(work(guard).await) })
            .await
            .map_err(authority_error)?
    }
}

struct CanonicalOwnerMediaGuard {
    scope: Arc<OwnerGuardScope>,
    expected_digest: [u8; 32],
    revision: i64,
}
#[async_trait]
impl OwnerMediaGuard for CanonicalOwnerMediaGuard {
    fn binding_digest(&self) -> [u8; 32] {
        self.expected_digest
    }
    fn authority_revision(&self) -> i64 {
        self.revision
    }
    async fn authorize_in(
        &self,
        connection: &mut PgConnection,
        binding: &OwnerMediaBinding,
    ) -> Result<(), CommandError> {
        if binding.digest() != self.expected_digest {
            return Err(CommandError::Denied);
        }
        check_scope(&self.scope, binding)?;
        if security_revision(&self.scope)? != self.revision {
            return Err(CommandError::Denied);
        }
        let canonical_digest = self.scope.binding().digest();
        // Capture the ACTUAL effects transaction. The canonical scope persists
        // registration and both Auth/Friend Track receipts before this returns.
        // Failed/cancelled registration remains root-owned quarantine.
        self.scope
            .register_transaction_in(
                connection,
                canonical_digest,
                settlement_reference(binding, canonical_digest)?,
            )
            .await
            .map_err(authority_error)?;
        // Uploader-only self authority. Local media/event/admission/ownership
        // checks remain in the producer; this grants no linked target/audience.
        self.scope
            .check_subject(PolicyQuery {
                event: EventId::from_uuid(binding.identity.event),
                actor: AuthUserId::from_uuid(binding.identity.actor),
                subject: AuthUserId::from_uuid(binding.identity.actor),
                field: Field::SafetySelf,
            })
            .await
            .map_err(authority_error)?;
        check_scope(&self.scope, binding)?;
        if security_revision(&self.scope)? != self.revision {
            return Err(CommandError::Denied);
        }
        Ok(())
    }
}

fn check_scope(scope: &OwnerGuardScope, expected: &OwnerMediaBinding) -> Result<(), CommandError> {
    expected.check_current()?;
    let actual = scope.binding();
    actual.check().map_err(authority_error)?;
    let identity = expected.identity;
    if actual.actor() != identity.actor
        || actual.event() != identity.event
        || actual.session_family() != identity.session_family
        || actual.request_id() != identity.request_id
        || actual.operation() != expected.operation
        || actual.http_method() != expected.method
        || actual.http_target() != expected.target
        || actual.body_digest() != expected.body_sha256
        || actual.deadline().expires_micros() != identity.deadline.expires_micros()
        || actual.deadline().stop() != identity.deadline.stop()
    {
        return Err(CommandError::Denied);
    }
    Ok(())
}

fn security_revision(scope: &OwnerGuardScope) -> Result<i64, CommandError> {
    let revision = scope.authority_revision().map_err(authority_error)?;
    if !(1..=9_007_199_254_740_991).contains(&revision) {
        return Err(CommandError::Unavailable);
    }
    Ok(revision)
}

fn authority_error(status: Status) -> CommandError {
    match status.code() {
        Code::PermissionDenied | Code::Unauthenticated => CommandError::Denied,
        _ => CommandError::Unavailable,
    }
}
