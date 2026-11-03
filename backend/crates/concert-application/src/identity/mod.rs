//! Internal identity authority ports; these values are neither wire DTOs nor stored identities.
//! Implementations must obtain owner-verified evidence, never echo untrusted request fields.
use crate::{Clock, Operation, ParticipationPolicy, PolicyError, PolicyGrant};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_domain::{AuthUserId, ParticipantKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum IdentityError {
    #[error("Authentication is required")]
    Unauthenticated,
    #[error("Participation is not authorized")]
    Denied,
    #[error("Identity authority is unavailable")]
    Unavailable,
}

macro_rules! secret_value {
    ($name:ident) => {
        #[derive(Clone, PartialEq, Eq)]
        pub struct $name(String);
        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(concat!(stringify!($name), "([REDACTED])"))
            }
        }
        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}
secret_value!(AccessCredential);
secret_value!(SessionId);
secret_value!(ClaimReference);
impl AccessCredential {
    pub fn new(value: String) -> Result<Self, IdentityError> {
        if value.is_empty()
            || value.len() > 16384
            || value.chars().any(char::is_whitespace)
            || value.chars().any(char::is_control)
        {
            return Err(IdentityError::Unauthenticated);
        }
        Ok(Self(value))
    }
}
impl SessionId {
    /// Canonical lowercase hyphenated UUID syntax, without treating a session as a user ID.
    pub fn new(value: String) -> Result<Self, IdentityError> {
        if value.len() != 36
            || !value.bytes().enumerate().all(|(i, b)| {
                if [8, 13, 18, 23].contains(&i) {
                    b == b'-'
                } else {
                    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                }
            })
        {
            return Err(IdentityError::Unavailable);
        }
        Ok(Self(value))
    }
}
impl ClaimReference {
    /// An opaque owner claim reference, never a raw age proof, email or social handle.
    pub fn new(value: String) -> Result<Self, IdentityError> {
        if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
            return Err(IdentityError::Denied);
        }
        Ok(Self(value))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Email,
    Google,
    Apple,
    X,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentityContext {
    pub key: ParticipantKey,
    pub operation: Operation,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityBinding {
    pub context: IdentityContext,
    pub session_id: SessionId,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProofWindow {
    pub checked_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
impl ProofWindow {
    fn valid(self, requested_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
        // Require a fresh evaluation for this invocation; cached eligibility is not authority.
        requested_at <= self.checked_at && self.checked_at <= now && now < self.expires_at
    }
}
#[derive(Clone, Debug)]
pub struct VerifiedSession {
    pub binding: AuthorityBinding,
    pub window: ProofWindow,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionState {
    Active,
    Ended,
    Unknown,
}
#[derive(Clone)]
pub struct SessionAuthority {
    pub binding: AuthorityBinding,
    pub window: ProofWindow,
    pub state: SessionState,
    pub roles: Vec<String>,
}
impl std::fmt::Debug for SessionAuthority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionAuthority")
            .field("binding", &self.binding)
            .field("window", &self.window)
            .field("state", &self.state)
            .field("roles", &"[REDACTED]")
            .finish()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EligibilityState {
    Eligible,
    Suspended,
    Ineligible,
    Unknown,
}
#[derive(Clone, Debug)]
pub struct EligibilityAuthority {
    pub binding: AuthorityBinding,
    pub window: ProofWindow,
    pub state: EligibilityState,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileState {
    Active,
    Deleted,
    Unknown,
}
/// Internal allowlisted content only. A complete C03 PublicProfile additionally needs the
/// independently authorized Concert avatar; this type must never be serialized as that DTO.
#[derive(Clone, PartialEq, Eq)]
pub struct BasicProfile {
    user_id: AuthUserId,
    display_name: String,
    bio: Option<String>,
}
impl std::fmt::Debug for BasicProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BasicProfile([REDACTED])")
    }
}
impl BasicProfile {
    pub fn new(
        user_id: AuthUserId,
        display_name: String,
        bio: Option<String>,
    ) -> Result<Self, IdentityError> {
        if display_name.trim().is_empty()
            || display_name.chars().count() > 40
            || display_name.chars().any(char::is_control)
            || bio
                .as_ref()
                .is_some_and(|v| v.chars().count() > 160 || v.chars().any(char::is_control))
        {
            return Err(IdentityError::Unavailable);
        }
        Ok(Self {
            user_id,
            display_name,
            bio,
        })
    }
    pub fn user_id(&self) -> AuthUserId {
        self.user_id
    }
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    pub fn bio(&self) -> Option<&str> {
        self.bio.as_deref()
    }
}
#[derive(Clone, Debug)]
pub struct ProfileAuthority {
    pub binding: AuthorityBinding,
    pub window: ProofWindow,
    pub state: ProfileState,
    pub profile: BasicProfile,
}
#[derive(Clone, Debug)]
pub struct ProviderAuthority {
    pub binding: AuthorityBinding,
    pub window: ProofWindow,
    pub provider: Provider,
    pub claim: ClaimReference,
    pub attached: bool,
}

#[async_trait]
pub trait IdentityAuthority: Send + Sync {
    /// Verify issuer/audience/signature/expiry plus delegated event/operation context. A
    /// bearer JWT alone does not carry the full context or prove current eligibility.
    async fn authenticate(
        &self,
        credential: &AccessCredential,
        context: IdentityContext,
        at: DateTime<Utc>,
    ) -> Result<VerifiedSession, IdentityError>;
    async fn session(
        &self,
        binding: &AuthorityBinding,
        at: DateTime<Utc>,
    ) -> Result<SessionAuthority, IdentityError>;
    async fn eligibility(
        &self,
        binding: &AuthorityBinding,
        at: DateTime<Utc>,
    ) -> Result<EligibilityAuthority, IdentityError>;
    async fn profile(
        &self,
        binding: &AuthorityBinding,
        at: DateTime<Utc>,
    ) -> Result<ProfileAuthority, IdentityError>;
    /// Owner-backed claim lookup; not a provider-link mutation and not self-asserted claims.
    async fn provider(
        &self,
        binding: &AuthorityBinding,
        provider: Provider,
        claim: &ClaimReference,
        at: DateTime<Utc>,
    ) -> Result<ProviderAuthority, IdentityError>;
}

pub struct IdentityResolver<A, C> {
    authority: A,
    clock: C,
}
impl<A: IdentityAuthority, C: Clock> IdentityResolver<A, C> {
    pub fn new(authority: A, clock: C) -> Self {
        Self { authority, clock }
    }
    fn check(
        &self,
        actual: &AuthorityBinding,
        expected: &AuthorityBinding,
        window: ProofWindow,
        requested_at: DateTime<Utc>,
    ) -> Result<(), IdentityError> {
        if actual != expected || !window.valid(requested_at, self.clock.now()) {
            return Err(IdentityError::Denied);
        }
        Ok(())
    }
    async fn resolve(
        &self,
        credential: &AccessCredential,
        context: IdentityContext,
        at: DateTime<Utc>,
    ) -> Result<VerifiedSession, IdentityError> {
        if self.clock.now() < at {
            return Err(IdentityError::Denied);
        }
        let access = self.authority.authenticate(credential, context, at).await?;
        if access.binding.context != context {
            return Err(IdentityError::Denied);
        }
        self.check(&access.binding, &access.binding, access.window, at)?;
        let current = self.authority.session(&access.binding, at).await?;
        self.check(&current.binding, &access.binding, current.window, at)?;
        if current.state != SessionState::Active || !current.roles.iter().any(|r| r == "ROLE_USER")
        {
            return Err(IdentityError::Denied);
        }
        let eligible = self.authority.eligibility(&access.binding, at).await?;
        self.check(&eligible.binding, &access.binding, eligible.window, at)?;
        match eligible.state {
            EligibilityState::Eligible => {}
            EligibilityState::Unknown => return Err(IdentityError::Unavailable),
            _ => return Err(IdentityError::Denied),
        }
        // An access/session proof may expire during a later dependency call.
        self.check(&access.binding, &access.binding, access.window, at)?;
        self.check(&current.binding, &access.binding, current.window, at)?;
        Ok(VerifiedSession {
            binding: access.binding,
            window: ProofWindow {
                checked_at: access
                    .window
                    .checked_at
                    .max(current.window.checked_at)
                    .max(eligible.window.checked_at),
                expires_at: access
                    .window
                    .expires_at
                    .min(current.window.expires_at)
                    .min(eligible.window.expires_at),
            },
        })
    }
    pub async fn authorize(
        &self,
        credential: &AccessCredential,
        context: IdentityContext,
        at: DateTime<Utc>,
    ) -> Result<PolicyGrant, IdentityError> {
        self.resolve(credential, context, at).await?;
        Ok(PolicyGrant {
            key: context.key,
            operation: context.operation,
        })
    }
    pub async fn basic_profile(
        &self,
        credential: &AccessCredential,
        context: IdentityContext,
    ) -> Result<BasicProfile, IdentityError> {
        let at = self.clock.now();
        let access = self.resolve(credential, context, at).await?;
        let profile = self.authority.profile(&access.binding, at).await?;
        self.check(&profile.binding, &access.binding, profile.window, at)?;
        self.check(&access.binding, &access.binding, access.window, at)?;
        if profile.profile.user_id() != context.key.user_id {
            return Err(IdentityError::Denied);
        }
        match profile.state {
            ProfileState::Active => Ok(profile.profile),
            ProfileState::Deleted => Err(IdentityError::Denied),
            ProfileState::Unknown => Err(IdentityError::Unavailable),
        }
    }
    pub async fn authorize_claim(
        &self,
        credential: &AccessCredential,
        context: IdentityContext,
        provider: Provider,
        claim: &ClaimReference,
    ) -> Result<PolicyGrant, IdentityError> {
        let at = self.clock.now();
        let access = self.resolve(credential, context, at).await?;
        let proof = self
            .authority
            .provider(&access.binding, provider, claim, at)
            .await?;
        self.check(&proof.binding, &access.binding, proof.window, at)?;
        self.check(&access.binding, &access.binding, access.window, at)?;
        if !proof.attached || proof.provider != provider || &proof.claim != claim {
            return Err(IdentityError::Denied);
        }
        Ok(PolicyGrant {
            key: context.key,
            operation: context.operation,
        })
    }
}
/// Per-request policy. Never share this object between users/events or retain it as a cache.
/// A fresh owner check runs for every Core authorization, before any repository dispatch.
pub struct IdentityPolicy<A, C> {
    resolver: IdentityResolver<A, C>,
    credential: AccessCredential,
    context: IdentityContext,
}
impl<A: IdentityAuthority, C: Clock> IdentityPolicy<A, C> {
    pub fn new(
        authority: A,
        clock: C,
        credential: AccessCredential,
        context: IdentityContext,
    ) -> Self {
        Self {
            resolver: IdentityResolver::new(authority, clock),
            credential,
            context,
        }
    }
}
#[async_trait]
impl<A: IdentityAuthority, C: Clock> ParticipationPolicy for IdentityPolicy<A, C> {
    async fn authorize(
        &self,
        key: ParticipantKey,
        operation: Operation,
        at: DateTime<Utc>,
    ) -> Result<PolicyGrant, PolicyError> {
        let context = IdentityContext { key, operation };
        if context != self.context {
            return Err(PolicyError::Denied);
        }
        self.resolver
            .authorize(&self.credential, context, at)
            .await
            .map_err(|e| match e {
                IdentityError::Unavailable => PolicyError::Unavailable,
                IdentityError::Unauthenticated | IdentityError::Denied => PolicyError::Denied,
            })
    }
}
