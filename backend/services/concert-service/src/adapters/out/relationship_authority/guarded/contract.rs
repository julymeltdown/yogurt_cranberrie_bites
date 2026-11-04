//! Binding and sealed consumer API for owner-held canonical exclusion.
use super::super::{OwnerRevision, RevisionVector};
use chrono::{DateTime, Utc};
use concert_domain::relationship::{Field, PolicyQuery, RelationshipError};
use platform::actor_delegation::owner_binding::OwnerBinding;
use platform::actor_delegation::{
    authority_deadline::AuthorityRpcDeadline, exchange::VerifiedParentAuthority,
};
use std::{fmt, sync::Arc, time::Duration};
use tonic::Status;
use uuid::Uuid;

/// Closed disclosure purpose; this is metadata, never a grant or public DTO.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GiftPrivateKind {
    Pickup,
    Code,
}

pub const MAX_SUBJECTS: usize = 100;
pub const MAX_GUARD_DURATION: Duration = Duration::from_secs(5);
/// A binding is not authority. Only acquisition against both actual owners may
/// construct OwnerGuardScope. Never log parent proofs, views or completion keys.
#[derive(Clone)]
pub struct GuardBinding {
    pub(crate) actor: Uuid,
    pub(crate) event: Uuid,
    pub(crate) family: Uuid,
    pub(crate) request: Uuid,
    pub(crate) operation: Arc<str>,
    pub(crate) parent_method: Arc<str>,
    pub(crate) parent_digest: [u8; 32],
    pub(crate) parent_body_digest: [u8; 32],
    pub(crate) http_method: Arc<str>,
    pub(crate) http_target: Arc<str>,
    pub(crate) body_digest: [u8; 32],
    pub(crate) deadline: AuthorityRpcDeadline,
    digest: [u8; 32],
}
impl fmt::Debug for GuardBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GuardBinding([REDACTED])")
    }
}
impl GuardBinding {
    #[allow(clippy::too_many_arguments)]
    pub fn from_parent(
        parent: &VerifiedParentAuthority,
        actor: Uuid,
        event: Uuid,
        family: Uuid,
        request: Uuid,
        operation: &str,
        http_method: &str,
        http_target: &str,
        body_digest: [u8; 32],
        original: AuthorityRpcDeadline,
    ) -> Result<Self, Status> {
        if actor != parent.actor()
            || [actor, event, family, request].iter().any(Uuid::is_nil)
            || operation.is_empty()
            || operation.len() > 100
            || operation.chars().any(char::is_control)
            || !matches!(http_method, "GET" | "POST" | "PATCH" | "PUT" | "DELETE")
            || !http_target.starts_with(&format!("/api/concert/events/{event}/"))
            || http_target.len() > 4096
            || http_target.chars().any(char::is_control)
            || body_digest == [0; 32]
        {
            return Err(Status::permission_denied("Invalid owner guard binding"));
        }
        let expires = parent
            .expires_at()
            .checked_mul(1_000_000)
            .ok_or_else(|| Status::permission_denied("Invalid parent deadline"))?
            .min(original.expires_micros());
        let deadline = AuthorityRpcDeadline::within_original(
            expires,
            original.stop().into_std(),
            MAX_GUARD_DURATION,
        )?;
        let wire_binding = OwnerBinding {
            actor_id: actor,
            event_id: event,
            session_family_id: family,
            request_id: request,
            operation: operation.into(),
            expires_unix_micros: deadline.expires_micros(),
            parent_digest: parent.parent_digest(),
            parent_body_digest: parent.request_digest(),
            parent_method: parent.request().method.into(),
            http_method: http_method.into(),
            http_target: http_target.into(),
            body_digest,
        };
        wire_binding.matches_parent(parent)?;
        let digest = wire_binding.digest()?;
        Ok(Self {
            digest,
            actor,
            event,
            family,
            request,
            operation: operation.into(),
            parent_method: parent.request().method.into(),
            parent_digest: parent.parent_digest(),
            parent_body_digest: parent.request_digest(),
            http_method: http_method.into(),
            http_target: http_target.into(),
            body_digest,
            deadline,
        })
    }
    pub fn actor(&self) -> Uuid {
        self.actor
    }
    pub fn event(&self) -> Uuid {
        self.event
    }
    pub fn session_family(&self) -> Uuid {
        self.family
    }
    pub fn request_id(&self) -> Uuid {
        self.request
    }
    pub fn parent_digest(&self) -> [u8; 32] {
        self.parent_digest
    }
    pub fn deadline(&self) -> AuthorityRpcDeadline {
        self.deadline
    }
    pub fn operation(&self) -> &str {
        &self.operation
    }
    pub fn check(&self) -> Result<(), Status> {
        self.deadline
            .remaining(MAX_GUARD_DURATION, self.deadline.expires_micros())
            .map(|_| ())
    }
    pub fn assert_context(
        &self,
        actor: Uuid,
        event: Uuid,
        family: Uuid,
        request: Uuid,
        parent_digest: [u8; 32],
        expires: DateTime<Utc>,
    ) -> Result<(), Status> {
        self.check()?;
        if (
            actor,
            event,
            family,
            request,
            parent_digest,
            expires.timestamp_micros(),
        ) != (
            self.actor,
            self.event,
            self.family,
            self.request,
            self.parent_digest,
            self.deadline.expires_micros(),
        ) {
            return Err(Status::permission_denied("Owner guard context mismatch"));
        }
        Ok(())
    }
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
    pub fn http_method(&self) -> &str {
        &self.http_method
    }
    pub fn http_target(&self) -> &str {
        &self.http_target
    }
    pub fn body_digest(&self) -> [u8; 32] {
        self.body_digest
    }
    pub fn owner_binding(&self) -> OwnerBinding {
        OwnerBinding {
            actor_id: self.actor,
            event_id: self.event,
            session_family_id: self.family,
            request_id: self.request,
            operation: self.operation.to_string(),
            expires_unix_micros: self.deadline.expires_micros(),
            parent_digest: self.parent_digest,
            parent_body_digest: self.parent_body_digest,
            parent_method: self.parent_method.to_string(),
            http_method: self.http_method.to_string(),
            http_target: self.http_target.to_string(),
            body_digest: self.body_digest,
        }
    }

    pub fn wire(&self, subjects: impl IntoIterator<Item = Uuid>) -> super::proto::GuardBinding {
        super::proto::GuardBinding {
            actor_id: self.actor.to_string(),
            event_id: self.event.to_string(),
            session_family_id: self.family.to_string(),
            request_id: self.request.to_string(),
            operation: self.operation.to_string(),
            subject_ids: subjects.into_iter().map(|id| id.to_string()).collect(),
            expires_unix_micros: self.deadline.expires_micros(),
            parent_digest: self.parent_digest.to_vec(),
            parent_body_digest: self.parent_body_digest.to_vec(),
            parent_method: self.parent_method.to_string(),
            http_method: self.http_method.to_string(),
            http_target: self.http_target.to_string(),
            body_digest: self.body_digest.to_vec(),
        }
    }
}
/// Profile-owned child exclusion. Root retains this object until actual response
/// completion. An uncertain drop MUST retain owner exclusion for reconciliation.
#[async_trait::async_trait]
pub trait HeldChildGuard: Send + Sync {
    async fn check(&self) -> Result<(), Status>;
    async fn complete_settled(&self) -> Result<(), Status>;
    fn recovery_reference(&self) -> Result<super::ChildRecoveryReference, Status>;
}
/// A sealed, currently enrolled public-profile subject. Construction stays within
/// the real canonical owner scope; arbitrary Context/observation cannot mint it.
pub struct GuardedSubject {
    pub(super) scope: Arc<super::OwnerGuardScope>,
    pub(super) subject: Uuid,
}
impl fmt::Debug for GuardedSubject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GuardedSubject([REDACTED])")
    }
}
impl GuardedSubject {
    pub fn query(&self) -> PolicyQuery {
        PolicyQuery {
            actor: concert_domain::AuthUserId::from_uuid(self.scope.binding().actor()),
            event: concert_domain::EventId::from_uuid(self.scope.binding().event()),
            subject: concert_domain::AuthUserId::from_uuid(self.subject),
            field: Field::PublicProfile,
        }
    }
    pub fn binding(&self) -> &GuardBinding {
        self.scope.binding()
    }
    pub fn revision_vector(&self) -> RevisionVector {
        self.scope.revision_vector()
    }
    pub async fn check(&self) -> Result<(), Status> {
        self.scope.check_subject(self.query()).await
    }
    pub fn security_revisions(&self) -> Result<(i64, i64), Status> {
        self.scope.security_revisions(self.subject)
    }
    pub async fn retain_child(&self, child: Arc<dyn HeldChildGuard>) -> Result<(), Status> {
        self.scope.retain_child(child).await
    }
    pub async fn register_child_intent(
        &self,
        reference: &super::ChildRecoveryReference,
    ) -> Result<(), Status> {
        if reference.query().subject != self.subject {
            return Err(Status::permission_denied("Child subject mismatch"));
        }
        self.scope.register_child_intent(reference).await
    }
    pub async fn capture_child_identity_in(
        &self,
        connection: &mut sqlx::PgConnection,
    ) -> Result<super::ChildNativeIdentity, Status> {
        super::ChildNativeIdentity::capture_concert_in(connection, &self.scope).await
    }
}
/// Actual full transaction identity must be read by the producer on its protected
/// connection before effects, then registered durably before it proceeds.
#[derive(Clone)]
pub struct ProducerTransaction {
    pub producer: ProducerKind,
    pub system_identifier: String,
    pub database_oid: u32,
    pub full_xid: String,
    pub backend_pid: i32,
    pub backend_started: DateTime<Utc>,
    pub effect_binding_digest: [u8; 32],
    pub producer_instance_id: Uuid,
    pub settlement_reference: Vec<u8>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProducerKind {
    Concert,
    Post,
    Profile,
}
impl fmt::Debug for ProducerTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProducerTransaction([REDACTED])")
    }
}
pub(super) fn unavailable(_: impl fmt::Display) -> Status {
    Status::unavailable("Canonical owner authority unavailable")
}
pub(super) fn relationship_unavailable(_: Status) -> RelationshipError {
    RelationshipError::Unavailable
}
pub(super) fn validate_vector(vector: RevisionVector) -> Result<RevisionVector, Status> {
    for OwnerRevision { epoch, revision } in [vector.auth, vector.friend] {
        if epoch.is_nil() || !(1..=9_007_199_254_740_991).contains(&revision) {
            return Err(Status::unavailable("Invalid owner revision"));
        }
    }
    Ok(vector)
}
