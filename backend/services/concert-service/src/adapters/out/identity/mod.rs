//! Typed owner-boundary adapter. No transport, credentials, RPC policy, or production
//! registration is implied here. Owner records below are internal inputs, not a new wire API.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::identity::*;
use concert_application::Operation;
use concert_domain::{AuthUserId, EventId, ParticipantKey};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerFailure {
    CredentialRejected,
    CallerRejected,
    PermissionDenied,
    Timeout,
    Unavailable,
    Malformed,
}
impl From<OwnerFailure> for IdentityError {
    fn from(value: OwnerFailure) -> Self {
        match value {
            OwnerFailure::CredentialRejected => Self::Unauthenticated,
            OwnerFailure::PermissionDenied => Self::Denied,
            OwnerFailure::CallerRejected
            | OwnerFailure::Timeout
            | OwnerFailure::Unavailable
            | OwnerFailure::Malformed => Self::Unavailable,
        }
    }
}
/// Metadata must come from a verified owner reply/delegation, never from copying request
/// values. Existing CheckSession/GetProfile responses do not provide this whole proof.
#[derive(Clone)]
pub struct OwnerProof {
    pub user_id: String,
    pub event_id: String,
    pub session_id: String,
    pub operation: Operation,
    pub checked_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
impl OwnerProof {
    fn decode(self) -> Result<(AuthorityBinding, ProofWindow), IdentityError> {
        let user_id =
            AuthUserId::from_str(&self.user_id).map_err(|_| IdentityError::Unavailable)?;
        let event_id = EventId::from_str(&self.event_id).map_err(|_| IdentityError::Unavailable)?;
        let session_id = SessionId::new(self.session_id)?;
        if self.expires_at <= self.checked_at {
            return Err(IdentityError::Unavailable);
        }
        Ok((
            AuthorityBinding {
                context: IdentityContext {
                    key: ParticipantKey { user_id, event_id },
                    operation: self.operation,
                },
                session_id,
            },
            ProofWindow {
                checked_at: self.checked_at,
                expires_at: self.expires_at,
            },
        ))
    }
}
#[derive(Clone)]
pub struct OwnerSession {
    pub proof: OwnerProof,
    pub active: bool,
    pub roles: Vec<String>,
}
#[derive(Clone)]
pub struct OwnerEligibility {
    pub proof: OwnerProof,
    pub state: EligibilityState,
}
/// Profile fields selected at the owner-client boundary. Legacy avatar URL, username,
/// counts and privacy metadata have no destination field and are never accepted here.
#[derive(Clone)]
pub struct OwnerProfile {
    pub proof: OwnerProof,
    pub user_id: String,
    pub nickname: String,
    pub bio: String,
    pub lifecycle_status: String,
}
#[derive(Clone)]
pub struct OwnerProvider {
    pub proof: OwnerProof,
    pub provider: Provider,
    pub claim: ClaimReference,
    pub attached: bool,
}

#[async_trait]
pub trait OwnerIdentityClient: Send + Sync {
    /// Requires verified JWT AND actor/event/operation delegation. No bearer-only fallback.
    async fn authenticate(
        &self,
        credential: &AccessCredential,
        context: IdentityContext,
        at: DateTime<Utc>,
    ) -> Result<OwnerProof, OwnerFailure>;
    /// CheckSession active/current roles plus qualified request-bound response evidence.
    async fn check_session(
        &self,
        binding: &AuthorityBinding,
        at: DateTime<Utc>,
    ) -> Result<OwnerSession, OwnerFailure>;
    /// Separate real suspension authority; Profile lifecycle and Auth roles cannot supply it.
    async fn check_eligibility(
        &self,
        binding: &AuthorityBinding,
        at: DateTime<Utc>,
    ) -> Result<OwnerEligibility, OwnerFailure>;
    async fn get_profile(
        &self,
        binding: &AuthorityBinding,
        at: DateTime<Utc>,
    ) -> Result<OwnerProfile, OwnerFailure>;
    /// The Auth/provider owner must prove this exact claim reference belongs to this actor.
    async fn check_provider(
        &self,
        binding: &AuthorityBinding,
        provider: Provider,
        claim: &ClaimReference,
        at: DateTime<Utc>,
    ) -> Result<OwnerProvider, OwnerFailure>;
}

fn dependency_error(error: OwnerFailure) -> IdentityError {
    match error {
        OwnerFailure::CredentialRejected => IdentityError::Unavailable,
        other => other.into(),
    }
}

pub struct OwnerIdentityAdapter<T> {
    client: T,
}
impl<T: OwnerIdentityClient> OwnerIdentityAdapter<T> {
    pub fn new(client: T) -> Self {
        Self { client }
    }
}
#[async_trait]
impl<T: OwnerIdentityClient> IdentityAuthority for OwnerIdentityAdapter<T> {
    async fn authenticate(
        &self,
        credential: &AccessCredential,
        context: IdentityContext,
        at: DateTime<Utc>,
    ) -> Result<VerifiedSession, IdentityError> {
        let (binding, window) = self
            .client
            .authenticate(credential, context, at)
            .await?
            .decode()?;
        Ok(VerifiedSession { binding, window })
    }
    async fn session(
        &self,
        binding: &AuthorityBinding,
        at: DateTime<Utc>,
    ) -> Result<SessionAuthority, IdentityError> {
        let reply = self
            .client
            .check_session(binding, at)
            .await
            .map_err(dependency_error)?;
        let (binding, window) = reply.proof.decode()?;
        Ok(SessionAuthority {
            binding,
            window,
            state: if reply.active {
                SessionState::Active
            } else {
                SessionState::Ended
            },
            roles: reply.roles,
        })
    }
    async fn eligibility(
        &self,
        binding: &AuthorityBinding,
        at: DateTime<Utc>,
    ) -> Result<EligibilityAuthority, IdentityError> {
        let reply = self
            .client
            .check_eligibility(binding, at)
            .await
            .map_err(dependency_error)?;
        let (binding, window) = reply.proof.decode()?;
        Ok(EligibilityAuthority {
            binding,
            window,
            state: reply.state,
        })
    }
    async fn profile(
        &self,
        binding: &AuthorityBinding,
        at: DateTime<Utc>,
    ) -> Result<ProfileAuthority, IdentityError> {
        let reply = self
            .client
            .get_profile(binding, at)
            .await
            .map_err(dependency_error)?;
        let user_id =
            AuthUserId::from_str(&reply.user_id).map_err(|_| IdentityError::Unavailable)?;
        let profile = BasicProfile::new(
            user_id,
            reply.nickname,
            if reply.bio.is_empty() {
                None
            } else {
                Some(reply.bio)
            },
        )?;
        let (binding, window) = reply.proof.decode()?;
        let state = match reply.lifecycle_status.as_str() {
            "ACTIVE" => ProfileState::Active,
            "DELETED" => ProfileState::Deleted,
            _ => ProfileState::Unknown,
        };
        Ok(ProfileAuthority {
            binding,
            window,
            state,
            profile,
        })
    }
    async fn provider(
        &self,
        binding: &AuthorityBinding,
        provider: Provider,
        claim: &ClaimReference,
        at: DateTime<Utc>,
    ) -> Result<ProviderAuthority, IdentityError> {
        let reply = self
            .client
            .check_provider(binding, provider, claim, at)
            .await
            .map_err(dependency_error)?;
        let (binding, window) = reply.proof.decode()?;
        Ok(ProviderAuthority {
            binding,
            window,
            provider: reply.provider,
            claim: reply.claim,
            attached: reply.attached,
        })
    }
}
/// Useful only as an explicit fail-closed placeholder in tests/future composition. The
/// current production application continues using BE05's original UnavailablePolicy.
pub struct UnavailableOwner;
#[async_trait]
impl OwnerIdentityClient for UnavailableOwner {
    async fn authenticate(
        &self,
        _: &AccessCredential,
        _: IdentityContext,
        _: DateTime<Utc>,
    ) -> Result<OwnerProof, OwnerFailure> {
        Err(OwnerFailure::Unavailable)
    }
    async fn check_session(
        &self,
        _: &AuthorityBinding,
        _: DateTime<Utc>,
    ) -> Result<OwnerSession, OwnerFailure> {
        Err(OwnerFailure::Unavailable)
    }
    async fn check_eligibility(
        &self,
        _: &AuthorityBinding,
        _: DateTime<Utc>,
    ) -> Result<OwnerEligibility, OwnerFailure> {
        Err(OwnerFailure::Unavailable)
    }
    async fn get_profile(
        &self,
        _: &AuthorityBinding,
        _: DateTime<Utc>,
    ) -> Result<OwnerProfile, OwnerFailure> {
        Err(OwnerFailure::Unavailable)
    }
    async fn check_provider(
        &self,
        _: &AuthorityBinding,
        _: Provider,
        _: &ClaimReference,
        _: DateTime<Utc>,
    ) -> Result<OwnerProvider, OwnerFailure> {
        Err(OwnerFailure::Unavailable)
    }
}
