//! Minimal current self content for a future authenticated Concert identity RPC.
//! A query is a subject selector, not authentication or admission. No Auth/session,
//! provider, role, avatar or event-policy assertions belong to this owner read.
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Utc};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConcertProfileError {
    InvalidSubject,
    ContentUnavailable,
    DependencyUnavailable,
}
impl std::fmt::Display for ConcertProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidSubject => "Profile subject is invalid",
            Self::ContentUnavailable => "Profile content is unavailable",
            Self::DependencyUnavailable => "Profile owner is unavailable",
        })
    }
}
impl std::error::Error for ConcertProfileError {}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ConcertProfileQuery(Uuid);
impl ConcertProfileQuery {
    pub fn new(user_id: Uuid) -> Result<Self, ConcertProfileError> {
        if user_id.is_nil() {
            return Err(ConcertProfileError::InvalidSubject);
        }
        Ok(Self(user_id))
    }
    pub fn from_canonical(value: &str) -> Result<Self, ConcertProfileError> {
        let user_id = Uuid::parse_str(value).map_err(|_| ConcertProfileError::InvalidSubject)?;
        if user_id.to_string() != value {
            return Err(ConcertProfileError::InvalidSubject);
        }
        Self::new(user_id)
    }
    pub fn user_id(self) -> Uuid {
        self.0
    }
}
impl std::fmt::Debug for ConcertProfileQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ConcertProfileQuery([REDACTED])")
    }
}

/// Repository boundary values, deliberately excluding every broad Profile DTO field.
/// Observed time is a database statement observation, never a TTL, revision or proof.
#[derive(Clone)]
pub struct ConcertProfileRecord {
    pub user_id: Uuid,
    pub nickname: String,
    pub bio: String,
    pub observed_at: DateTime<Utc>,
}
impl std::fmt::Debug for ConcertProfileRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ConcertProfileRecord([REDACTED])")
    }
}

/// Private, nonserialized content for the future self-only producer. It is not a
/// C03 PublicProfile: an independently authorized Concert avatar is still required.
/// ```compile_fail
/// use profile_application::concert_identity::ConcertSelfProfile;
/// fn cannot_serialize_private_content(value: ConcertSelfProfile) {
///     let _ = serde_json::to_value(value);
/// }
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct ConcertSelfProfile {
    user_id: Uuid,
    display_name: String,
    bio: Option<String>,
    observed_at: DateTime<Utc>,
}
impl ConcertSelfProfile {
    pub fn user_id(&self) -> Uuid {
        self.user_id
    }
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    pub fn bio(&self) -> Option<&str> {
        self.bio.as_deref()
    }
    pub fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }
}
impl std::fmt::Debug for ConcertSelfProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ConcertSelfProfile([REDACTED])")
    }
}

#[async_trait]
pub trait ConcertProfileRepository: Send + Sync {
    /// Exactly one uncached owner observation. Absent, deleted and fenced are all None.
    async fn find_self_content(
        &self,
        query: ConcertProfileQuery,
    ) -> Result<Option<ConcertProfileRecord>, ConcertProfileError>;
}
#[derive(Clone)]
pub struct ReadConcertSelfProfile(Arc<dyn ConcertProfileRepository>);
impl ReadConcertSelfProfile {
    pub fn new(repository: Arc<dyn ConcertProfileRepository>) -> Self {
        Self(repository)
    }
    pub async fn execute(
        &self,
        query: ConcertProfileQuery,
    ) -> Result<ConcertSelfProfile, ConcertProfileError> {
        let row = self
            .0
            .find_self_content(query)
            .await
            .map_err(|_| ConcertProfileError::DependencyUnavailable)?
            .ok_or(ConcertProfileError::ContentUnavailable)?;
        // Fail closed on a defective/misbound repository without rendering its contents.
        // Portable finite timestamp range only; this does not claim clock freshness.
        if row.user_id != query.user_id()
            || row.user_id.is_nil()
            || !(1..=9999).contains(&row.observed_at.year())
            || row.nickname.trim().is_empty()
            || row.nickname.chars().count() > 40
            || row.nickname.chars().any(char::is_control)
            || row.bio.chars().count() > 160
            || row.bio.chars().any(char::is_control)
        {
            return Err(ConcertProfileError::ContentUnavailable);
        }
        Ok(ConcertSelfProfile {
            user_id: row.user_id,
            display_name: row.nickname,
            bio: if row.bio.is_empty() {
                None
            } else {
                Some(row.bio)
            },
            observed_at: row.observed_at,
        })
    }
}
