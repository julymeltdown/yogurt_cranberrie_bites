//! Concrete Party adapter over the already-acquired canonical owner scope.
//! It consumes the qualified PublicProfileReader and never acquires authority.
use super::{PartyLiveBinding, PartyLiveError, PartyLiveOwnerGuard};
use crate::adapters::out::{
    public_profile::PublicProfileReader, relationship_authority::guarded::OwnerGuardScope,
};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

pub struct ScopePartyLiveGuard {
    scope: Arc<OwnerGuardScope>,
    profiles: Arc<PublicProfileReader>,
    binding: PartyLiveBinding,
}

impl ScopePartyLiveGuard {
    /// Root must call this with the exact Party binding after it acquired and
    /// published the canonical scope. No canonical authority is created here.
    pub fn new(
        scope: Arc<OwnerGuardScope>,
        profiles: Arc<PublicProfileReader>,
        binding: PartyLiveBinding,
    ) -> Result<Self, PartyLiveError> {
        if scope.binding().digest() != binding.parent_digest()? {
            return Err(PartyLiveError::DependencyUnavailable);
        }
        Ok(Self {
            scope,
            profiles,
            binding,
        })
    }

    fn check(&self, binding: &PartyLiveBinding) -> Result<(), PartyLiveError> {
        binding.check()?;
        if binding.digest() != self.binding.digest() {
            return Err(PartyLiveError::Denied);
        }
        self.scope
            .binding()
            .check()
            .map_err(|_| PartyLiveError::DependencyUnavailable)
    }

    fn subject(&self, subject: Uuid) -> Result<(), PartyLiveError> {
        if subject.is_nil() {
            return Err(PartyLiveError::InvalidInput);
        }
        Ok(())
    }
}

#[async_trait]
impl PartyLiveOwnerGuard for ScopePartyLiveGuard {
    fn binding_digest(&self) -> [u8; 32] {
        self.binding.digest()
    }

    fn authority_revision(&self) -> i64 {
        self.scope.authority_revision().unwrap_or_default()
    }

    async fn authorize_in(
        &self,
        _: &mut PgConnection,
        binding: &PartyLiveBinding,
        subject: Uuid,
    ) -> Result<(), PartyLiveError> {
        self.check(binding)?;
        self.subject(subject)?;
        let guarded = self
            .scope
            .public_profile_subject(subject)
            .await
            .map_err(status)?;
        self.profiles
            .local_subject(&guarded)
            .await
            .map_err(|_| PartyLiveError::DependencyUnavailable)?;
        guarded.check().await.map_err(status)
    }

    async fn public_profile_in(
        &self,
        _: &mut PgConnection,
        binding: &PartyLiveBinding,
        subject: Uuid,
    ) -> Result<serde_json::Value, PartyLiveError> {
        self.check(binding)?;
        self.subject(subject)?;
        let guarded = self
            .scope
            .public_profile_subject(subject)
            .await
            .map_err(status)?;
        let author = self
            .profiles
            .read(&guarded)
            .await
            .map_err(|_| PartyLiveError::DependencyUnavailable)?;
        if author.user_id.uuid() != subject || author.json.is_empty() {
            return Err(PartyLiveError::DependencyUnavailable);
        }
        let profile: serde_json::Value = serde_json::from_slice(&author.json)
            .map_err(|_| PartyLiveError::DependencyUnavailable)?;
        crate::adapters::out::public_profile::validate(&profile).map_err(status)?;
        guarded.check().await.map_err(status)?;
        Ok(profile)
    }

    async fn track_in(
        &self,
        connection: &mut PgConnection,
        binding: &PartyLiveBinding,
    ) -> Result<(), PartyLiveError> {
        self.check(binding)?;
        let mut hash = Sha256::new();
        hash.update(b"MCR party transaction v1\0");
        hash.update(self.scope.binding().digest());
        hash.update(binding.digest());
        let effect: [u8; 32] = hash.finalize().into();
        let reference = serde_json::to_vec(&serde_json::json!({
            "kind": "PARTY_REQUEST_V1",
            "actor_id": binding.actor(),
            "event_id": binding.event(),
            "request_id": binding.request_id(),
            "operation_id": binding.operation(),
            "idempotency_key": binding.idempotency_key(),
        }))
        .map_err(|_| PartyLiveError::DependencyUnavailable)?;
        self.scope
            .register_transaction_in(connection, effect, reference)
            .await
            .map_err(status)
    }
}

fn status(_: Status) -> PartyLiveError {
    PartyLiveError::DependencyUnavailable
}
