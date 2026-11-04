//! Adapter over the actual sealed canonical owner scope. This file is staged
//! for the guard owner's qualified SDK; it is not yet exported by live/mod.rs.
//! It owns no Complete/finish_response path and never releases on Drop/error.
use super::{
    coordinator::{CoordinatorAuthority, NativeTransaction, ParticipantProfileJournal},
    owner_error,
};
use crate::{
    adapters::out::relationship_authority::guarded::{
        OwnerGuardScope, ProducerKind, ProducerTransaction,
    },
    participant::{
        context::VerifiedParticipantRequest,
        ports::{CurrentAuth, MutationAuthorityPort, MutationPermit},
        Operation,
    },
};
use concert_application::avatar::{AvatarEffectFence, AvatarError, AvatarTransactionIdentity};
use concert_domain::relationship::{Field, PolicyQuery};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct BoundParticipantAuthority {
    scope: Arc<OwnerGuardScope>,
    request: VerifiedParticipantRequest,
    journal: Arc<ParticipantProfileJournal>,
}
pub fn operation_label(operation: Operation) -> &'static str {
    match operation {
        Operation::GetParticipant => "getConcertParticipant",
        Operation::SetAvatar => "setConcertAvatar",
        Operation::UpdatePreferences => "updateConcertParticipant",
    }
}
impl BoundParticipantAuthority {
    /// Only an actual successful CanonicalOwnerFence::acquire supplies this
    /// sealed scope. A verified transport parent or owner read cannot create it.
    pub fn new(
        scope: Arc<OwnerGuardScope>,
        request: &VerifiedParticipantRequest,
        journal: Arc<ParticipantProfileJournal>,
    ) -> Result<Self, AvatarError> {
        let value = Self {
            scope,
            request: request.clone(),
            journal,
        };
        value.check_bound(request)?;
        Ok(value)
    }
    pub fn scope(&self) -> &Arc<OwnerGuardScope> {
        &self.scope
    }
    fn check_bound(&self, request: &VerifiedParticipantRequest) -> Result<(), AvatarError> {
        request.check().map_err(owner_error)?;
        self.request.check().map_err(owner_error)?;
        let parent = request.parent_authority().map_err(owner_error)?;
        let original = self.request.parent_authority().map_err(owner_error)?;
        if parent.parent_digest() != original.parent_digest()
            || request.operation() != self.request.operation()
            || request.release_digest() != self.request.release_digest()
            || request.raw_body() != self.request.raw_body()
            || request.deadline().expires_micros() != self.request.deadline().expires_micros()
            || request.deadline().stop() != self.request.deadline().stop()
        {
            return Err(AvatarError::Denied);
        }
        self.scope
            .binding()
            .assert_context(
                request.key().user_id.uuid(),
                request.key().event_id.uuid(),
                request.session_family_id(),
                request.request_id(),
                parent.parent_digest(),
                chrono::DateTime::from_timestamp_micros(request.deadline().expires_micros())
                    .ok_or(AvatarError::Unavailable)?,
            )
            .map_err(owner_error)?;
        let binding = self.scope.wire_binding();
        if self.scope.binding().parent_digest() != parent.parent_digest()
            || binding.body_digest.as_slice() != request.body_sha256()
            || binding.parent_body_digest.as_slice() != parent.request_digest()
            || binding.parent_method != parent.request().method
            || binding.http_method != request.http_method()
            || binding.http_target != request.http_target()
            || binding.operation != operation_label(request.operation())
        {
            return Err(AvatarError::Denied);
        }
        self.scope.authority_revision().map_err(owner_error)?;
        Ok(())
    }
    async fn check_live(&self, request: &VerifiedParticipantRequest) -> Result<(), AvatarError> {
        self.check_bound(request)?;
        self.scope
            .check_subject(PolicyQuery {
                actor: request.key().user_id,
                event: request.key().event_id,
                subject: request.key().user_id,
                field: Field::SafetySelf,
            })
            .await
            .map_err(owner_error)?;
        self.check_bound(request)
    }
    fn instance(&self) -> Result<Uuid, AvatarError> {
        Uuid::parse_str(&self.scope.producer_scope().instance_id)
            .ok()
            .filter(|id| !id.is_nil())
            .ok_or(AvatarError::Unavailable)
    }
    async fn track(
        &self,
        identity: AvatarTransactionIdentity,
        effect: [u8; 32],
    ) -> Result<(), AvatarError> {
        self.check_live(&self.request).await?;
        self.scope
            .track_transaction(ProducerTransaction {
                producer: ProducerKind::Concert,
                system_identifier: identity.system_identifier,
                database_oid: identity.database_oid,
                full_xid: identity.full_xid,
                backend_pid: identity.backend_pid,
                backend_started: identity.backend_started,
                effect_binding_digest: effect,
                producer_instance_id: self.instance()?,
                settlement_reference: self.request.request_id().as_bytes().to_vec(),
            })
            .await
            .map_err(owner_error)?;
        self.check_live(&self.request).await
    }
}
impl MutationPermit for BoundParticipantAuthority {
    fn check(&self, request: &VerifiedParticipantRequest) -> Result<(), AvatarError> {
        if request.operation() == Operation::GetParticipant {
            return Err(AvatarError::Denied);
        }
        self.check_bound(request)
    }
    fn effect_fence(&self) -> Option<&dyn AvatarEffectFence> {
        Some(self)
    }
}
#[async_trait::async_trait]
impl MutationAuthorityPort for BoundParticipantAuthority {
    async fn authorize(
        &self,
        request: &VerifiedParticipantRequest,
        auth: &CurrentAuth,
    ) -> Result<Box<dyn MutationPermit>, AvatarError> {
        auth.check(request)?;
        MutationPermit::check(self, request)?;
        self.check_live(request).await?;
        Ok(Box::new(self.clone()))
    }
}
#[async_trait::async_trait]
impl AvatarEffectFence for BoundParticipantAuthority {
    async fn register(&self, transaction: AvatarTransactionIdentity) -> Result<(), AvatarError> {
        MutationPermit::check(self, &self.request)?;
        let mut digest = Sha256::new();
        digest.update(b"mcr participant avatar effect v1\0");
        digest.update(self.scope.binding().digest());
        digest.update(self.request.release_digest().as_bytes());
        digest.update(self.request.body_sha256());
        self.track(transaction, digest.finalize().into()).await
    }
    async fn check(&self) -> Result<(), AvatarError> {
        self.check_live(&self.request).await?;
        self.journal.ensure_visible(&self.request, None).await
    }
}
#[async_trait::async_trait]
impl CoordinatorAuthority for BoundParticipantAuthority {
    async fn check(&self, request: &VerifiedParticipantRequest) -> Result<(), AvatarError> {
        MutationPermit::check(self, request)?;
        self.check_live(request).await
    }
    async fn track_concert_transaction(
        &self,
        request: &VerifiedParticipantRequest,
        transaction: &NativeTransaction,
        effect_digest: [u8; 32],
    ) -> Result<(), AvatarError> {
        self.check_bound(request)?;
        self.track(
            AvatarTransactionIdentity {
                system_identifier: transaction.system_identifier().into(),
                database_oid: transaction.database_oid(),
                full_xid: transaction.full_xid().into(),
                backend_pid: transaction.backend_pid(),
                backend_started: transaction.backend_started(),
            },
            effect_digest,
        )
        .await
    }
}
