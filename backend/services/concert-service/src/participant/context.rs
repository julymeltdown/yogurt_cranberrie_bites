//! Verified signed transport context. This proves no live account or capability.
use super::{proto, Operation};
use concert_domain::{avatar::SetAvatar, AuthUserId, EventId, ParticipantKey};
use platform::{
    actor_delegation::{
        authority_deadline::AuthorityRpcDeadline, read_authority::VerifiedReadAuthority,
        VerifiedActor,
    },
    grpc_auth::{VerifiedCaller, Workload},
};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tonic::{Request, Status};
use uuid::Uuid;

pub const BUDGET: Duration = Duration::from_secs(2);
pub struct VerifiedParticipantRequest {
    key: ParticipantKey,
    session: Uuid,
    request_id: Uuid,
    operation: Operation,
    method: String,
    target: String,
    body: Vec<u8>,
    digest: String,
    deadline: AuthorityRpcDeadline,
}
impl std::fmt::Debug for VerifiedParticipantRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VerifiedParticipantRequest([REDACTED])")
    }
}
impl VerifiedParticipantRequest {
    pub fn key(&self) -> ParticipantKey {
        self.key
    }
    pub fn session_family_id(&self) -> Uuid {
        self.session
    }
    pub fn request_id(&self) -> Uuid {
        self.request_id
    }
    pub fn operation(&self) -> Operation {
        self.operation
    }
    pub fn http_method(&self) -> &str {
        &self.method
    }
    pub fn http_target(&self) -> &str {
        &self.target
    }
    pub fn raw_body(&self) -> &[u8] {
        &self.body
    }
    pub fn body_sha256(&self) -> [u8; 32] {
        Sha256::digest(&self.body).into()
    }
    pub fn release_digest(&self) -> &str {
        &self.digest
    }
    pub fn deadline(&self) -> AuthorityRpcDeadline {
        self.deadline
    }
    pub fn check(&self) -> Result<(), Status> {
        self.deadline
            .remaining(BUDGET, self.deadline.expires_micros())
            .map(|_| ())
    }
    pub(crate) fn verify(
        request: &Request<proto::ParticipantRequest>,
        operation: Operation,
    ) -> Result<Self, Status> {
        if !matches!(
            request.extensions().get::<VerifiedCaller>(),
            Some(VerifiedCaller(Workload::Gateway))
        ) {
            return Err(Status::unauthenticated("Verified Gateway required"));
        }
        let actor = request
            .extensions()
            .get::<VerifiedActor>()
            .ok_or_else(|| Status::unauthenticated("Verified actor required"))?
            .0;
        let proof = request
            .extensions()
            .get::<VerifiedReadAuthority>()
            .ok_or_else(|| {
                Status::unauthenticated("Original signed transport authority required")
            })?;
        let m = request.get_ref();
        let id = |s: &str| {
            Uuid::parse_str(s)
                .ok()
                .filter(|v| !v.is_nil() && v.to_string() == s)
                .ok_or_else(|| Status::invalid_argument("Invalid participant identity"))
        };
        let user = id(&m.user_id)?;
        let event = id(&m.event_id)?;
        let session = id(&m.session_family_id)?;
        let request_id = id(&m.request_id)?;
        if actor != user || proof.actor() != user {
            return Err(Status::permission_denied("Participant actor mismatch"));
        }
        let (method, suffix) = match operation {
            Operation::GetParticipant => ("GET", "me"),
            Operation::SetAvatar => ("PUT", "me/avatar"),
            Operation::UpdatePreferences => ("PATCH", "me"),
        };
        if m.http_method != method
            || m.http_target != format!("/api/concert/events/{event}/{suffix}")
        {
            return Err(Status::permission_denied("Participant target mismatch"));
        }
        if m.release_digest.len() != 64
            || !m
                .release_digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Status::invalid_argument("Invalid release identity"));
        }
        match operation {
            Operation::GetParticipant if !m.request_json.is_empty() => {
                return Err(Status::invalid_argument("GET body must be empty"))
            }
            Operation::SetAvatar => {
                SetAvatar::parse(&m.request_json)
                    .map_err(|_| Status::invalid_argument("Invalid avatar request"))?;
            }
            Operation::UpdatePreferences => {
                crate::adapters::out::repository::avatar::parse_preferences(&m.request_json)
                    .map_err(|_| {
                        Status::invalid_argument("Only preference updates are supported")
                    })?;
            }
            _ => {}
        }
        let now = chrono::Utc::now().timestamp_micros();
        for observed in [
            m.auth_observed_at_unix_micros,
            m.profile_observed_at_unix_micros,
        ] {
            if observed <= 0
                || observed > now
                || m.authority_expires_unix_micros <= observed
                || m.authority_expires_unix_micros.saturating_sub(observed)
                    > BUDGET.as_micros() as i64
            {
                return Err(Status::permission_denied(
                    "Invalid owner observation bounds",
                ));
            }
        }
        let original = proof.deadline(BUDGET)?;
        let deadline = AuthorityRpcDeadline::within_original(
            m.authority_expires_unix_micros
                .min(original.expires_micros()),
            original.stop().into_std(),
            BUDGET,
        )?;
        let value = Self {
            key: ParticipantKey {
                event_id: EventId::from_uuid(event),
                user_id: AuthUserId::from_uuid(user),
            },
            session,
            request_id,
            operation,
            method: m.http_method.clone(),
            target: m.http_target.clone(),
            body: m.request_json.clone(),
            digest: m.release_digest.clone(),
            deadline,
        };
        value.check()?;
        Ok(value)
    }
}
