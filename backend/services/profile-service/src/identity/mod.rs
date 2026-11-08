//! NAVER identity verification, event-free.
//!
//! This module owns the whole verification flow: the provider profile decoding
//! (`naver_profile`), the provider client (`naver_provider`), the provider
//! configuration (`naver_config`), the claim store (`repository`), and the four
//! use cases the contract names (`start`, `callback`, `status`, `disconnect`).
//!
//! Three properties are structural rather than conventional:
//!
//! * **There is no event.** No type in this module has an `event_id` field, no
//!   SQL statement names an event column, and the claim store is keyed by the
//!   Auth `user_id` alone. The donor's event-scoped claim store is exactly what
//!   this module exists to avoid.
//! * **The claim store is the only authority.** Age and gender come from a
//!   stored claim written by a validated provider profile. No caller can supply
//!   either value: the start operation takes a return path, the callback takes
//!   the provider's own query parameters, and the read and disconnect take
//!   nothing but the authenticated user.
//! * **Nothing sensitive is formatted.** The provider token, the provider
//!   subject, the birth date and the derived age have no `Debug`, no `Display`
//!   and no `Serialize` anywhere in this module, and the types that do render
//!   redact them by hand.
pub mod callback;
pub mod disconnect;
pub mod naver_config;
pub mod naver_profile;
pub mod naver_provider;
pub mod repository;
pub mod start;
pub mod status;

pub use callback::{CallbackError, CallbackOutcome, CallbackResult, CallbackCommand, CompleteVerification};
pub use disconnect::DisconnectVerification;
pub use naver_config::NaverOAuthConfig;
pub use naver_profile::{NaverGender, NaverProfile};
pub use naver_provider::{
    NaverAccessToken, NaverProviderClient, NaverProviderEndpoints, NaverProviderError,
    NaverProviderProfile,
};
pub use repository::IdentityRepository;
pub use start::{StartError, StartVerification, StartedVerification};
pub use status::{IdentityVerification, ReadVerification, VerificationState};

/// The whole flow, assembled once per process.
///
/// The provider client holds one HTTP client; the repository holds one pool.
/// Nothing here is per-request state.
#[derive(Clone)]
pub struct IdentityRuntime {
    pub repository: IdentityRepository,
    pub config: NaverOAuthConfig,
    pub provider: NaverProviderClient,
}

impl IdentityRuntime {
    /// Build the runtime from a pool and the resolved configuration.
    ///
    /// `subject_salt` must be a process-stable secret: the subject hash is what
    /// stops one provider account from backing two profiles, and a salt that
    /// changes per restart would let the same account back a second profile
    /// after a deploy.
    pub fn new(
        pool: sqlx::PgPool,
        config: NaverOAuthConfig,
        subject_salt: Vec<u8>,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            subject_salt.len() >= 32,
            "the identity subject salt must be at least 32 bytes"
        );
        let provider = NaverProviderClient::new(config.endpoints())?;
        Ok(Self {
            repository: IdentityRepository::new(pool, subject_salt),
            config,
            provider,
        })
    }

    pub fn start(&self) -> StartVerification {
        StartVerification::new(self.repository.clone(), self.config.clone())
    }

    pub fn complete(&self) -> CompleteVerification {
        CompleteVerification::new(self.repository.clone(), self.provider.clone())
    }

    pub fn read(&self) -> ReadVerification {
        ReadVerification::new(self.repository.clone())
    }

    pub fn disconnect(&self) -> DisconnectVerification {
        DisconnectVerification::new(self.repository.clone())
    }
}
