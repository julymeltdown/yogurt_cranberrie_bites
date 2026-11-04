//! Internal Auth-owned current-state read. These values are not signed admission
//! capabilities, public wire DTOs, event roles, suspension or provider consent.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    #[error("invalid identity read input")]
    InvalidInput,
    #[error("identity read cancelled")]
    Cancelled,
    #[error("identity read deadline exceeded")]
    DeadlineExceeded,
    #[error("global service consent required")]
    ConsentRequired,
    #[error("global enrollment policy unavailable")]
    PolicyUnavailable,
    #[error("identity owner unavailable")]
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingProvider {
    Email,
    Google,
    Apple,
    X,
}
impl BindingProvider {
    pub fn parse(value: &str) -> Result<Self, ReadError> {
        match value {
            "EMAIL" => Ok(Self::Email),
            "GOOGLE" => Ok(Self::Google),
            "APPLE" => Ok(Self::Apple),
            "X" => Ok(Self::X),
            _ => Err(ReadError::InvalidInput),
        }
    }
    pub fn owner_name(self) -> &'static str {
        match self {
            Self::Email => "EMAIL",
            Self::Google => "GOOGLE",
            Self::Apple => "APPLE",
            Self::X => "X",
        }
    }
}

/// Exact owner row generation. Epochs are assigned by Auth's database trigger,
/// never inferred from linked_at or a caller's clock. No subject is carried.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ProviderBinding {
    identity_id: Uuid,
    provider: BindingProvider,
    epoch: i64,
}
impl ProviderBinding {
    pub const MAX_EPOCH: i64 = 9_007_199_254_740_991;
    pub fn new(
        identity_id: Uuid,
        provider: BindingProvider,
        epoch: i64,
    ) -> Result<Self, ReadError> {
        if identity_id.is_nil() || !(1..=Self::MAX_EPOCH).contains(&epoch) {
            return Err(ReadError::InvalidInput);
        }
        Ok(Self {
            identity_id,
            provider,
            epoch,
        })
    }
    pub fn parse(identity_id: &str, provider: &str, epoch: i64) -> Result<Self, ReadError> {
        let id = Uuid::parse_str(identity_id).map_err(|_| ReadError::InvalidInput)?;
        if id.hyphenated().to_string() != identity_id {
            return Err(ReadError::InvalidInput);
        }
        Self::new(id, BindingProvider::parse(provider)?, epoch)
    }
    pub fn identity_id(self) -> Uuid {
        self.identity_id
    }
    pub fn provider(self) -> BindingProvider {
        self.provider
    }
    pub fn epoch(self) -> i64 {
        self.epoch
    }
}
impl fmt::Debug for ProviderBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProviderBinding([REDACTED])")
    }
}

#[derive(Clone, Copy)]
pub struct AdmissionRead {
    user_id: Uuid,
    family_id: Uuid,
    binding: Option<ProviderBinding>,
}
impl AdmissionRead {
    /// The caller must authenticate these canonical IDs. Construction proves
    /// only shape, not authority to assert them.
    pub fn new(
        user_id: Uuid,
        family_id: Uuid,
        binding: Option<ProviderBinding>,
    ) -> Result<Self, ReadError> {
        if user_id.is_nil() || family_id.is_nil() {
            return Err(ReadError::InvalidInput);
        }
        Ok(Self {
            user_id,
            family_id,
            binding,
        })
    }
    pub fn user_id(self) -> Uuid {
        self.user_id
    }
    pub fn family_id(self) -> Uuid {
        self.family_id
    }
    pub fn binding(self) -> Option<ProviderBinding> {
        self.binding
    }
}
impl fmt::Debug for AdmissionRead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AdmissionRead([REDACTED])")
    }
}

#[derive(Clone, Default)]
pub struct ReadCancellation(Arc<AtomicBool>);
impl ReadCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
impl fmt::Debug for ReadCancellation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReadCancellation([REDACTED])")
    }
}

/// One caller deadline, shared across acquisition, setup, snapshot and cleanup.
/// Adapter cancellation observation is bounded to 5ms while awaiting I/O.
#[derive(Clone)]
pub struct ReadBudget {
    deadline: Instant,
    cancellation: ReadCancellation,
}
impl ReadBudget {
    pub const MAX_DURATION: Duration = Duration::from_secs(2);
    pub fn new(deadline: Instant, cancellation: ReadCancellation) -> Result<Self, ReadError> {
        if deadline.saturating_duration_since(Instant::now()) > Self::MAX_DURATION {
            return Err(ReadError::InvalidInput);
        }
        Ok(Self {
            deadline,
            cancellation,
        })
    }
    pub fn deadline(&self) -> Instant {
        self.deadline
    }
    pub fn check(&self) -> Result<(), ReadError> {
        if self.cancellation.is_cancelled() {
            Err(ReadError::Cancelled)
        } else if Instant::now() >= self.deadline {
            Err(ReadError::DeadlineExceeded)
        } else {
            Ok(())
        }
    }
    pub fn remaining(&self) -> Result<Duration, ReadError> {
        self.check()?;
        Ok(self.deadline.saturating_duration_since(Instant::now()))
    }
}
impl fmt::Debug for ReadBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReadBudget([REDACTED])")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountEligibility {
    Active,
    Ineligible,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionEligibility {
    Live,
    Ended,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingEligibility {
    NotRequested,
    Bound,
    NotBound,
}

/// Facts observed in one database statement snapshot, not a lease. A concurrent
/// revocation committed after that snapshot requires a fresh read to observe it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CurrentAdmission {
    pub observed_at: DateTime<Utc>,
    pub account: AccountEligibility,
    pub session: SessionEligibility,
    pub binding: BindingEligibility,
}
impl CurrentAdmission {
    pub fn eligible(self) -> bool {
        self.account == AccountEligibility::Active
            && self.session == SessionEligibility::Live
            && self.binding != BindingEligibility::NotBound
    }
}
impl fmt::Debug for CurrentAdmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CurrentAdmission([REDACTED])")
    }
}

#[async_trait]
pub trait CurrentAdmissionPort: Send + Sync {
    async fn read_current(
        &self,
        request: AdmissionRead,
        budget: &ReadBudget,
    ) -> Result<CurrentAdmission, ReadError>;
}
