//! Idempotent participant/default bootstrap over a sealed current admission.
//!
//! The public C03 response is supplied by the service after strict contract
//! validation. This layer owns identity/default consistency and persistence
//! decisions, never public serialization or Gateway verification.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_domain::{
    AuthUserId, AvatarSelection, EventId, EventPhase, ParticipantKey, Preferences,
};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootstrapError {
    InvalidInput,
    Unauthenticated,
    Denied,
    EventNotFound,
    EventNotOpen,
    RestrictionUnavailable,
    ReleaseUnavailable,
    DependencyUnavailable,
    InvalidResponse,
}

/// An exact current admission. Construction is intentionally restricted to the
/// service's verified RPC boundary; this module never parses transport data.
pub struct BootstrapAdmission {
    user_id: AuthUserId,
    event_id: EventId,
    session_family_id: uuid::Uuid,
    request_id: uuid::Uuid,
    display_name: String,
    bio: Option<String>,
    received_at: DateTime<Utc>,
}
impl fmt::Debug for BootstrapAdmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BootstrapAdmission([REDACTED])")
    }
}
impl BootstrapAdmission {
    #[allow(clippy::too_many_arguments)]
    pub fn from_verified(
        user_id: AuthUserId,
        event_id: EventId,
        session_family_id: uuid::Uuid,
        request_id: uuid::Uuid,
        display_name: String,
        bio: Option<String>,
        received_at: DateTime<Utc>,
    ) -> Result<Self, BootstrapError> {
        if session_family_id.is_nil() || request_id.is_nil() {
            return Err(BootstrapError::Unauthenticated);
        }
        if !valid_display_name(&display_name) || bio.as_deref().is_some_and(|v| !valid_bio(v)) {
            return Err(BootstrapError::InvalidInput);
        }
        Ok(Self {
            user_id,
            event_id,
            session_family_id,
            request_id,
            display_name,
            bio,
            received_at,
        })
    }
    pub fn key(&self) -> ParticipantKey {
        ParticipantKey {
            event_id: self.event_id,
            user_id: self.user_id,
        }
    }
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    pub fn bio(&self) -> Option<&str> {
        self.bio.as_deref()
    }
    pub fn session_family_id(&self) -> uuid::Uuid {
        self.session_family_id
    }
    pub fn request_id(&self) -> uuid::Uuid {
        self.request_id
    }
    pub fn received_at(&self) -> DateTime<Utc> {
        self.received_at
    }
}

fn valid_display_name(value: &str) -> bool {
    !value.trim().is_empty() && value.chars().count() <= 40 && !value.chars().any(char::is_control)
}
fn valid_bio(value: &str) -> bool {
    value.chars().count() <= 160 && !value.chars().any(char::is_control)
}

/// Configuration that can affect the persisted participant/default result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootstrapDefaults {
    pub theme: concert_domain::Theme,
    pub avatar: AvatarSelection,
}
/// Configuration that can affect only the frozen C03 result.
#[derive(Clone)]
pub struct BootstrapRelease {
    defaults: BootstrapDefaults,
    bootstrap_json: Vec<u8>,
}
impl std::fmt::Debug for BootstrapRelease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BootstrapRelease")
            .field("bootstrap_json", &"[C03 JSON]")
            .finish_non_exhaustive()
    }
}
impl BootstrapRelease {
    pub fn new(
        defaults: BootstrapDefaults,
        bootstrap_json: Vec<u8>,
    ) -> Result<Self, BootstrapError> {
        if bootstrap_json.is_empty() || bootstrap_json.len() > 2 * 1024 * 1024 {
            return Err(BootstrapError::ReleaseUnavailable);
        }
        Ok(Self {
            defaults,
            bootstrap_json,
        })
    }
    pub fn defaults(&self) -> &BootstrapDefaults {
        &self.defaults
    }
    pub fn bootstrap_json(&self) -> &[u8] {
        &self.bootstrap_json
    }
}

/// A validated immutable product release. The service validator owns the
/// complete C03/C06 checks; application callers must not construct this from
/// unchecked JSON or treat the embedded fixture bundle as product art.
pub struct ValidatedProductRelease {
    release: BootstrapRelease,
    source_digest: String,
}
impl fmt::Debug for ValidatedProductRelease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValidatedProductRelease")
            .field("source_digest", &self.source_digest)
            .finish_non_exhaustive()
    }
}
impl ValidatedProductRelease {
    pub fn from_validated(
        release: BootstrapRelease,
        source_digest: String,
    ) -> Result<Self, BootstrapError> {
        if source_digest.len() != 64
            || !source_digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(BootstrapError::ReleaseUnavailable);
        }
        Ok(Self {
            release,
            source_digest,
        })
    }
    pub fn release(&self) -> &BootstrapRelease {
        &self.release
    }
    pub fn source_digest(&self) -> &str {
        &self.source_digest
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootstrapRecord {
    pub event_id: EventId,
    pub event_revision: i64,
    pub event_slug: String,
    pub event_title: String,
    pub event_timezone: String,
    pub phase: EventPhase,
    pub restriction_revision: i64,
    pub active: bool,
    pub participant_revision: i64,
    pub preferences: Preferences,
    pub avatar_revision: i64,
    pub avatar: AvatarSelection,
    pub completed_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct BootstrapResult {
    pub record: BootstrapRecord,
    pub release: BootstrapRelease,
}

/// Persistence must hold event and admission locks through all writes and
/// return only committed rows. A suspended membership is never reactivated.
#[async_trait]
pub trait BootstrapRepository: Send + Sync {
    async fn bootstrap(
        &self,
        admission: &BootstrapAdmission,
        defaults: &BootstrapDefaults,
    ) -> Result<BootstrapRecord, BootstrapError>;
}

pub struct BootstrapUseCase<R> {
    repository: R,
    release: BootstrapRelease,
}
impl<R: BootstrapRepository> BootstrapUseCase<R> {
    pub fn new(repository: R, release: BootstrapRelease) -> Self {
        Self {
            repository,
            release,
        }
    }
    pub fn release(&self) -> &BootstrapRelease {
        &self.release
    }
    pub async fn execute(
        &self,
        admission: &BootstrapAdmission,
    ) -> Result<BootstrapResult, BootstrapError> {
        let record = self
            .repository
            .bootstrap(admission, self.release.defaults())
            .await?;
        if record.event_id != admission.key().event_id
            || !record.active
            || record.event_revision < 1
            || record.restriction_revision < 1
            || !matches!(
                record.phase,
                EventPhase::Open | EventPhase::Live | EventPhase::ReadOnly | EventPhase::Archived
            )
            || record.participant_revision < 1
            || record.avatar_revision < 1
        {
            return Err(BootstrapError::InvalidResponse);
        }
        Ok(BootstrapResult {
            record,
            release: self.release.clone(),
        })
    }
}

#[async_trait]
impl<T: BootstrapRepository + ?Sized> BootstrapRepository for std::sync::Arc<T> {
    async fn bootstrap(
        &self,
        admission: &BootstrapAdmission,
        defaults: &BootstrapDefaults,
    ) -> Result<BootstrapRecord, BootstrapError> {
        T::bootstrap(self.as_ref(), admission, defaults).await
    }
}

/// Explicit test-only release input. The service's synthetic parser is gated by
/// an explicit test configuration and never used by production composition.
pub fn synthetic_test_release(
    bootstrap_json: Vec<u8>,
    defaults: BootstrapDefaults,
) -> Result<BootstrapRelease, BootstrapError> {
    BootstrapRelease::new(defaults, bootstrap_json)
}
