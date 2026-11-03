//! Sealed runtime-owned self observations. No event admission or publication fence.
use super::{grpc, OwnerRevision, BUDGET};
use crate::world::CurrentOwnedMembership;
use chrono::{DateTime, Utc};
use concert_application::world_ticket::{
    ConnectionId, OwnerIdentity, TicketClaim, WorldError, WorldSubject, TICKET_TTL_SECONDS,
};
use platform::{
    actor_delegation::{
        authority_deadline::AuthorityRpcDeadline, relationship_read::RelationshipReadSigner,
        request_for,
    },
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use std::{
    fmt,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use uuid::Uuid;

/// Only the server runtime constructs this after a real consume/current-owner read.
/// Neither public WorldSubject nor browser identity strings can construct one.
pub(crate) struct WorldOwnerRequest {
    subject: WorldSubject,
    family: Uuid,
    request_id: Uuid,
    credential_expires_at: DateTime<Utc>,
    source_expires_at: DateTime<Utc>,
    created_at_micros: i64,
    deadline: AuthorityRpcDeadline,
    attempted: AtomicBool,
    cancelled: AtomicBool,
}
impl fmt::Debug for WorldOwnerRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WorldOwnerRequest([REDACTED])")
    }
}
impl WorldOwnerRequest {
    /// Caller has consumed exactly once and compared the requested resource pin.
    /// No membership is required before initial admission and no ticket is consumed here.
    pub(crate) fn from_consumed(
        claim: &TicketClaim,
        request_id: Uuid,
        deadline: AuthorityRpcDeadline,
    ) -> Result<Self, WorldError> {
        let r = &claim.record;
        let now = Utc::now();
        if r.pin.event_id != r.subject.event_id
            || r.pin.room_id.uuid().is_nil()
            || claim.connection_id.uuid().is_nil()
            || r.issued_at.timestamp_micros() <= 0
            || r.issued_at > now
            || r.expires_at <= r.issued_at
            || r.expires_at > r.issued_at + chrono::Duration::seconds(TICKET_TTL_SECONDS)
            || r.expires_at > r.access_expires_at
        {
            return Err(WorldError::Invalid);
        }
        Self::bind(
            &r.subject,
            request_id,
            r.access_expires_at,
            r.expires_at,
            deadline,
        )
    }

    /// Caller read current_owned_membership and supplies its expected exact binding.
    /// The owner epoch, connection, canonical family and subject may not drift.
    pub(crate) fn from_current(
        current: &CurrentOwnedMembership,
        expected_owner: &OwnerIdentity,
        expected_subject: &WorldSubject,
        expected_connection: ConnectionId,
        request_id: Uuid,
        deadline: AuthorityRpcDeadline,
    ) -> Result<Self, WorldError> {
        let m = &current.membership;
        if &current.owner != expected_owner
            || OwnerIdentity::new(current.owner.process_id.clone(), current.owner.epoch).is_err()
        {
            return Err(WorldError::OwnerFenced);
        }
        if m.key != expected_subject.key
            || m.key.event_id != expected_subject.event_id
            || m.session_id != expected_subject.session_id
            || m.connection_id != expected_connection
            || m.connection_id.uuid().is_nil()
            || m.pin.event_id != m.key.event_id
            || m.pin.room_id != m.room_id
            || m.room_id.uuid().is_nil()
            || m.lease_until > m.access_expires_at
        {
            return Err(WorldError::Conflict);
        }
        Self::bind(
            expected_subject,
            request_id,
            m.access_expires_at,
            m.lease_until,
            deadline,
        )
    }

    fn bind(
        subject: &WorldSubject,
        request_id: Uuid,
        credential_expires_at: DateTime<Utc>,
        source_expires_at: DateTime<Utc>,
        original: AuthorityRpcDeadline,
    ) -> Result<Self, WorldError> {
        if request_id.is_nil()
            || subject.key.user_id.uuid().is_nil()
            || subject.event_id.uuid().is_nil()
            || subject.key.event_id != subject.event_id
        {
            return Err(WorldError::Invalid);
        }
        let family = Uuid::parse_str(subject.session_id.as_str())
            .ok()
            .filter(|id| !id.is_nil() && id.to_string() == subject.session_id.as_str())
            .ok_or(WorldError::Invalid)?;
        let now = Utc::now();
        if credential_expires_at <= now || source_expires_at <= now {
            return Err(WorldError::CredentialExpired);
        }
        let deadline = AuthorityRpcDeadline::within_original(
            original
                .expires_micros()
                .min(credential_expires_at.timestamp_micros())
                .min(source_expires_at.timestamp_micros()),
            original.stop().into_std(),
            BUDGET,
        )
        .map_err(|_| WorldError::Unavailable)?;
        let result = Self {
            subject: subject.clone(),
            family,
            request_id,
            credential_expires_at,
            source_expires_at,
            created_at_micros: now.timestamp_micros(),
            deadline,
            attempted: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
        };
        result.check()?;
        Ok(result)
    }
    pub(crate) fn subject(&self) -> &WorldSubject {
        &self.subject
    }
    pub(crate) fn credential_expires_at(&self) -> DateTime<Utc> {
        self.credential_expires_at
    }
    pub(crate) fn request_id(&self) -> Uuid {
        self.request_id
    }
    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub(crate) fn check(&self) -> Result<(), WorldError> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(WorldError::Unavailable);
        }
        let now = Utc::now();
        if now >= self.credential_expires_at || now >= self.source_expires_at {
            return Err(WorldError::CredentialExpired);
        }
        self.deadline
            .remaining(BUDGET, self.credential_expires_at.timestamp_micros())
            .map(|_| ())
            .map_err(|_| WorldError::Unavailable)
    }
    fn attempt(&self) -> Result<(), WorldError> {
        self.check()?;
        self.attempted
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|_| WorldError::CredentialConsumed)
    }
}

/// A successful bounded account/session read, never an event or mutation grant.
pub(crate) struct WorldAuthObservation {
    subject: WorldSubject,
    request_id: Uuid,
    credential_expires_at: DateTime<Utc>,
    observed_at: DateTime<Utc>,
    valid_until: DateTime<Utc>,
    revision: OwnerRevision,
}
impl fmt::Debug for WorldAuthObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WorldAuthObservation([REDACTED])")
    }
}
impl WorldAuthObservation {
    pub(crate) fn subject(&self) -> &WorldSubject {
        &self.subject
    }
    pub(crate) fn credential_expires_at(&self) -> DateTime<Utc> {
        self.credential_expires_at
    }
    pub(crate) fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }
    pub(crate) fn valid_until(&self) -> DateTime<Utc> {
        self.valid_until
    }
    pub(crate) fn revision(&self) -> OwnerRevision {
        self.revision
    }
    /// Checks local scope/expiry only; does not freshly query or fence revocation.
    pub(crate) fn check(&self, request: &WorldOwnerRequest) -> Result<(), WorldError> {
        request.check()?;
        let now = Utc::now();
        if self.subject != request.subject
            || self.request_id != request.request_id
            || self.credential_expires_at != request.credential_expires_at
            || self.observed_at.timestamp_micros() < request.created_at_micros
            || self.observed_at > now
            || self.valid_until <= now
            || self.valid_until <= self.observed_at
            || self.valid_until.timestamp_micros() > request.deadline.expires_micros()
            || self.valid_until > self.credential_expires_at
            || self.revision.validate().is_err()
        {
            return Err(WorldError::Unavailable);
        }
        Ok(())
    }
}

#[async_trait::async_trait]
pub(crate) trait WorldAuthRead: Send + Sync {
    async fn observe(
        &self,
        request: &WorldOwnerRequest,
    ) -> Result<WorldAuthObservation, WorldError>;
}
#[derive(Clone)]
pub(crate) struct GrpcWorldAuthReader {
    auth:
        grpc::proto::auth_concert_relationship_service_client::AuthConcertRelationshipServiceClient<
            grpc::Transport,
        >,
}
impl GrpcWorldAuthReader {
    pub(crate) fn connect(
        auth_endpoint: &str,
        service_token: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        signer: RelationshipReadSigner,
    ) -> anyhow::Result<Self> {
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            auth_endpoint,
            Workload::Auth,
            grpc::EDGE.validate()?,
        )?;
        Ok(Self {
            auth: grpc::proto::auth_concert_relationship_service_client::AuthConcertRelationshipServiceClient::with_interceptor(
                signer.client(channel, Workload::Auth)?,
                ServiceIdentityClientInterceptor::new(Workload::Concert, service_token)?,
            ).max_decoding_message_size(4096).max_encoding_message_size(4096),
        })
    }
}
#[async_trait::async_trait]
impl WorldAuthRead for GrpcWorldAuthReader {
    async fn observe(
        &self,
        binding: &WorldOwnerRequest,
    ) -> Result<WorldAuthObservation, WorldError> {
        binding.attempt()?;
        let mut stop = binding.deadline.stop();
        if let Some(outer) = platform::call_deadline::current() {
            stop = stop.min(outer.into());
        }
        let remaining = binding
            .deadline
            .remaining(BUDGET, binding.credential_expires_at.timestamp_micros())
            .map_err(|_| WorldError::Unavailable)?
            .min(stop.saturating_duration_since(tokio::time::Instant::now()));
        if remaining.is_zero() {
            return Err(WorldError::Unavailable);
        }
        let expires = binding.deadline.expires_micros().min(
            Utc::now()
                .timestamp_micros()
                .saturating_add(remaining.as_micros() as i64),
        );
        let message = grpc::proto::RelationshipRead {
            actor_id: binding.subject.key.user_id.uuid().to_string(),
            subject_id: binding.subject.key.user_id.uuid().to_string(),
            event_id: binding.subject.event_id.uuid().to_string(),
            session_family_id: binding.family.to_string(),
            request_id: binding.request_id.to_string(),
            purpose: "SAFETY_SELF".into(),
            expires_unix_micros: expires,
        };
        let dispatch = async {
            let mut request = request_for(message.clone(), &message.actor_id)
                .map_err(|_| WorldError::Unavailable)?;
            request.set_timeout(stop.saturating_duration_since(tokio::time::Instant::now()));
            let mut client = self.auth.clone();
            let auth = grpc::reply(
                Box::pin(client.read_current(request))
                    .await
                    .map_err(|_| WorldError::Unavailable)?,
            )
            .map_err(|_| WorldError::Unavailable)?;
            binding.check()?;
            let (observed_at, valid_until) = grpc::window(
                &auth.query,
                &message,
                auth.observed_at_unix_micros,
                auth.valid_until_unix_micros,
            )
            .map_err(|_| WorldError::Unavailable)?;
            let revision = grpc::revision(&auth.owner_epoch, auth.revision)
                .map_err(|_| WorldError::Unavailable)?;
            if !auth.actor_active || !auth.subject_active || !auth.session_live {
                return Err(WorldError::Denied);
            }
            let observation = WorldAuthObservation {
                subject: binding.subject.clone(),
                request_id: binding.request_id,
                credential_expires_at: binding.credential_expires_at,
                observed_at,
                valid_until,
                revision,
            };
            observation.check(binding)?;
            Ok(observation)
        };
        let cancellation = async {
            loop {
                binding.check()?;
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            #[allow(unreachable_code)]
            Ok::<(), WorldError>(())
        };
        let result = platform::call_deadline::scope(stop.into_std(), async {
            tokio::select! { biased;
                _ = tokio::time::sleep_until(stop) => Err(WorldError::Unavailable),
                cancelled = cancellation => Err(cancelled.err().unwrap_or(WorldError::Unavailable)),
                result = dispatch => result,
            }
        })
        .await;
        // Reject a noncooperative poll that returned after the original timer.
        if tokio::time::Instant::now() >= stop {
            return Err(WorldError::Unavailable);
        }
        binding.check()?;
        result
    }
}

#[cfg(test)]
mod tests;
