//! Safe, fixed failures shared by the HTTP entry and the signed owner adapters.

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AdmissionReadError {
    #[error("Admission access context expired or invalid")]
    AccessExpired,
    #[error("Invalid admission binding reference")]
    InvalidBinding,
    #[error("Admission read deadline exceeded")]
    DeadlineExceeded,
    #[error("Admission read denied")]
    Denied,
    #[error("Admission read capacity unavailable")]
    CapacityExhausted,
    #[error("Global service consent required")]
    ConsentRequired,
    #[error("Global enrollment policy unavailable")]
    PolicyUnavailable,
    #[error("Admission owner unavailable")]
    Unavailable,
    #[error("Invalid admission owner response")]
    InvalidResponse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConcertProfileReadError {
    #[error("Concert Profile access context expired or invalid")]
    AccessExpired,
    #[error("Concert Profile content unavailable")]
    NotFound,
    #[error("Concert Profile read denied")]
    Denied,
    #[error("Concert Profile read deadline exceeded")]
    DeadlineExceeded,
    #[error("Concert Profile capacity unavailable")]
    CapacityExhausted,
    #[error("Concert Profile owner unavailable")]
    Unavailable,
    #[error("Invalid Concert Profile owner response")]
    InvalidResponse,
}

/// Safe composition failures. Child classes remain explicit and fixed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConcertIdentityReadError {
    #[error("Concert identity access context expired or invalid")]
    AccessExpired,
    #[error("Concert identity read deadline exceeded")]
    DeadlineExceeded,
    #[error("Concert identity is not currently eligible")]
    NotEligible,
    #[error("Canonical Auth read failed: {0}")]
    Auth(AdmissionReadError),
    #[error("Canonical Profile read failed: {0}")]
    Profile(ConcertProfileReadError),
}

impl From<AdmissionReadError> for ConcertIdentityReadError {
    fn from(error: AdmissionReadError) -> Self {
        match error {
            AdmissionReadError::AccessExpired => Self::AccessExpired,
            AdmissionReadError::DeadlineExceeded => Self::DeadlineExceeded,
            other => Self::Auth(other),
        }
    }
}

impl From<ConcertProfileReadError> for ConcertIdentityReadError {
    fn from(error: ConcertProfileReadError) -> Self {
        match error {
            ConcertProfileReadError::AccessExpired => Self::AccessExpired,
            ConcertProfileReadError::DeadlineExceeded => Self::DeadlineExceeded,
            other => Self::Profile(other),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WorldClientError {
    #[error("World access expired")]
    Expired,
    #[error("World request denied")]
    Denied,
    #[error("World room is unavailable")]
    NotFound,
    #[error("World room is full")]
    Full,
    #[error("World release does not match")]
    VersionMismatch,
    #[error("World dependency is unavailable")]
    Unavailable,
    #[error("World response is invalid")]
    InvalidResponse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BootstrapError {
    #[error("Bootstrap identity is unavailable")]
    Identity,
    #[error("Bootstrap is not permitted")]
    Denied,
    #[error("Bootstrap resource is unavailable")]
    NotFound,
    #[error("Bootstrap authority expired")]
    Expired,
    #[error("Bootstrap dependency is unavailable")]
    Unavailable,
    #[error("Bootstrap dependency response is invalid")]
    InvalidResponse,
}

impl From<ConcertIdentityReadError> for BootstrapError {
    fn from(error: ConcertIdentityReadError) -> Self {
        match error {
            ConcertIdentityReadError::AccessExpired
            | ConcertIdentityReadError::DeadlineExceeded => Self::Expired,
            ConcertIdentityReadError::NotEligible => Self::Denied,
            _ => Self::Identity,
        }
    }
}
