//! Actual canonical owner-scope adapter. No positive fixture/default constructor.
use super::{PhotoFrame, PhotoTransactionGuard};
use crate::{
    adapters::out::relationship_authority::guarded::OwnerGuardScope,
    media_runtime::{capture_owner_context_in, OwnerMediaContextMode, OwnerMediaIdentity},
};
use async_trait::async_trait;
use concert_application::{commands::CommandError, media::Context};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use std::sync::Arc;
use uuid::Uuid;
/// Closed request settlement identity. Root must retain the actual scope and
/// authenticate/resolve its durable transaction registration through HTTP terminal
/// settlement. This reference alone is not evidence of commit or authorization.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    version: u8,
    purpose: String,
    actor: Uuid,
    event: Uuid,
    beacon: Uuid,
    key: Option<Uuid>,
    operation: String,
    guard_digest: [u8; 32],
    body_sha256: [u8; 32],
    photo_binding_digest: [u8; 32],
}
pub struct CanonicalPhotoGuard {
    scope: Arc<OwnerGuardScope>,
    expected: [u8; 32],
}
impl CanonicalPhotoGuard {
    /// Root seals this frame only after copying the actual verified HTTP key,
    /// content/body/target and canonical IDs. A frame is not authentication.
    pub fn from_request(
        scope: Arc<OwnerGuardScope>,
        frame: &PhotoFrame,
    ) -> Result<Self, CommandError> {
        check_binding(&scope, frame)?;
        Ok(Self {
            scope,
            expected: frame.digest(),
        })
    }
    pub fn scope(&self) -> &Arc<OwnerGuardScope> {
        &self.scope
    }
}
#[async_trait]
impl PhotoTransactionGuard for CanonicalPhotoGuard {
    async fn register_in(
        &self,
        c: &mut PgConnection,
        f: &PhotoFrame,
    ) -> Result<Context, CommandError> {
        if f.digest() != self.expected {
            return Err(CommandError::Denied);
        }
        check_binding(&self.scope, f)?;
        let b = self.scope.binding();
        b.check().map_err(|_| CommandError::Unavailable)?;
        if b.actor() != f.actor.actor
            || b.event() != f.actor.event
            || b.operation() != f.operation
            || b.http_method() != f.method
            || b.http_target() != f.target
            || b.body_digest() != f.body_sha256
        {
            return Err(CommandError::Denied);
        }
        let reference = Reference {
            version: 1,
            purpose: "MEETING_PHOTO".into(),
            actor: f.actor.actor,
            event: f.actor.event,
            beacon: f.beacon,
            key: f.key,
            operation: f.operation.into(),
            guard_digest: b.digest(),
            body_sha256: f.body_sha256,
            photo_binding_digest: f.digest(),
        };
        self.scope
            .register_transaction_in(
                c,
                b.digest(),
                serde_json::to_vec(&reference).map_err(|_| CommandError::InvalidState)?,
            )
            .await
            .map_err(|_| CommandError::Unavailable)?;
        // Real held per-actor security generation; no BE37 pair-stamp substitution.
        let revision = self
            .scope
            .authority_revision()
            .map_err(|_| CommandError::Unavailable)?;
        capture_owner_context_in(
            c,
            OwnerMediaIdentity {
                actor: b.actor(),
                event: b.event(),
                session_family: b.session_family(),
                request_id: b.request_id(),
                deadline: b.deadline(),
            },
            revision,
            if f.method == "GET" || f.method == "DELETE" {
                OwnerMediaContextMode::OwnerReadOrRecovery
            } else {
                OwnerMediaContextMode::Mutation
            },
        )
        .await
    }
}

fn check_binding(scope: &OwnerGuardScope, f: &PhotoFrame) -> Result<(), CommandError> {
    let b = scope.binding();
    b.check().map_err(|_| CommandError::Unavailable)?;
    if b.actor() != f.actor.actor
        || b.event() != f.actor.event
        || b.operation() != f.operation
        || b.http_method() != f.method
        || b.http_target() != f.target
        || b.body_digest() != f.body_sha256
    {
        return Err(CommandError::Denied);
    }
    Ok(())
}

/// Root may inspect only an authenticated durable registration's stored reference.
/// Shape validation neither authenticates the caller nor resolves SQL outcome.
pub struct ValidatedPhotoReference {
    reference: Reference,
}
impl ValidatedPhotoReference {
    pub fn photo_binding_digest(&self) -> [u8; 32] {
        self.reference.photo_binding_digest
    }
    pub fn guard_digest(&self) -> [u8; 32] {
        self.reference.guard_digest
    }
    pub fn idempotency_key(&self) -> Option<Uuid> {
        self.reference.key
    }
    pub fn beacon(&self) -> Uuid {
        self.reference.beacon
    }
    pub fn actor(&self) -> Uuid {
        self.reference.actor
    }
    pub fn event(&self) -> Uuid {
        self.reference.event
    }
    pub fn operation(&self) -> &str {
        &self.reference.operation
    }
}
pub fn validate_settlement_reference(
    bytes: &[u8],
) -> Result<ValidatedPhotoReference, CommandError> {
    if bytes.len() > 4096 {
        return Err(CommandError::InvalidInput);
    }
    let r: Reference = serde_json::from_slice(bytes).map_err(|_| CommandError::InvalidInput)?;
    let mutation = matches!(
        r.operation.as_str(),
        "createConcertBeacon" | "updateConcertBeacon" | "revokeConcertBeacon"
    );
    let read = matches!(
        r.operation.as_str(),
        "getConcertBeacon" | "getConcertMedia" | "readConcertMedia"
    );
    if r.version != 1
        || r.purpose != "MEETING_PHOTO"
        || [r.actor, r.event, r.beacon].iter().any(Uuid::is_nil)
        || r.key.is_some_and(|v| v.is_nil())
        || r.guard_digest == [0; 32]
        || r.photo_binding_digest == [0; 32]
        || r.body_sha256 == [0; 32]
        || (!mutation && !read)
        || mutation != r.key.is_some()
    {
        return Err(CommandError::InvalidInput);
    }
    Ok(ValidatedPhotoReference { reference: r })
}
