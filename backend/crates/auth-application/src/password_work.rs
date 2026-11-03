//! Application-owned contract for bounded password work.
//!
//! The application decides which semantic class of work is required, while an
//! outbound security adapter owns Argon2, randomness, blocking workers and
//! admission. Every request path receives this capability explicitly; there is
//! no process-default fallback.

use async_trait::async_trait;
use std::sync::Arc;

const MAX_WORKERS: usize = 64;
const MAX_QUEUE: usize = 1024;

/// Which admission class a piece of password work belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordWorkKind {
    /// Credential verification, including equalizing dummy verification.
    Login,
    /// Hashing a new account's password.
    Signup,
    /// Hashing a replacement password after reset-token admission.
    Reset,
}

impl PasswordWorkKind {
    pub const ALL: [Self; 3] = [Self::Login, Self::Signup, Self::Reset];

    /// Stable label used in metrics.
    pub fn label(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::Signup => "signup",
            Self::Reset => "reset",
        }
    }
}

/// Worker and queue bounds for the admitted password adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasswordWorkLimits {
    pub workers: usize,
    pub login_queue: usize,
    pub signup_workers: usize,
    pub signup_queue: usize,
    pub reset_workers: usize,
    pub reset_queue: usize,
}

impl Default for PasswordWorkLimits {
    fn default() -> Self {
        // Signup and reset can occupy one worker each, leaving two workers that
        // remain available to login under the default four-worker budget.
        Self {
            workers: 4,
            login_queue: 64,
            signup_workers: 1,
            signup_queue: 16,
            reset_workers: 1,
            reset_queue: 16,
        }
    }
}

impl PasswordWorkLimits {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            (1..=MAX_WORKERS).contains(&self.workers),
            "AUTH_PASSWORD_WORKERS must be 1-{MAX_WORKERS}"
        );
        anyhow::ensure!(
            self.signup_workers >= 1,
            "AUTH_PASSWORD_SIGNUP_WORKERS must be at least 1"
        );
        anyhow::ensure!(
            self.reset_workers >= 1,
            "AUTH_PASSWORD_RESET_WORKERS must be at least 1"
        );
        anyhow::ensure!(
            self.signup_workers + self.reset_workers < self.workers,
            "AUTH_PASSWORD_SIGNUP_WORKERS plus AUTH_PASSWORD_RESET_WORKERS must be less than \
             AUTH_PASSWORD_WORKERS so login keeps at least one reserved worker"
        );
        for (name, queue) in [
            ("AUTH_PASSWORD_LOGIN_QUEUE", self.login_queue),
            ("AUTH_PASSWORD_SIGNUP_QUEUE", self.signup_queue),
            ("AUTH_PASSWORD_RESET_QUEUE", self.reset_queue),
        ] {
            anyhow::ensure!(queue <= MAX_QUEUE, "{name} must not exceed {MAX_QUEUE}");
        }
        Ok(())
    }

    pub fn reserved_login_workers(&self) -> usize {
        self.workers
            .saturating_sub(self.signup_workers + self.reset_workers)
    }
}

/// Operational counters exposed without leaking the worker-pool implementation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PasswordWorkStats {
    pub admitted: u64,
    pub rejected: u64,
}

/// Complete password-work capability required by authentication use cases.
///
/// Equalization is intentionally distinct from ordinary verification: callers
/// must be able to request one login-cost operation when no real credential may
/// be inspected. Signup and reset are separate so the adapter can retain their
/// bounded classes and login reservation.
#[async_trait]
pub trait PasswordWorkPort: Send + Sync {
    async fn verify_login(
        &self,
        password: &str,
        encoded_hash: &str,
    ) -> crate::errors::AuthResult<bool>;
    async fn equalize_login(&self, password: &str) -> crate::errors::AuthResult<()>;
    async fn hash_for_signup(&self, password: &str) -> crate::errors::AuthResult<String>;
    async fn hash_for_reset(&self, password: &str) -> crate::errors::AuthResult<String>;
    fn stats(&self, kind: PasswordWorkKind) -> PasswordWorkStats;
}

pub type PasswordWork = Arc<dyn PasswordWorkPort>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_limits_reserve_login_workers() {
        let limits = PasswordWorkLimits::default();
        limits.validate().expect("defaults are valid");
        assert_eq!(limits.reserved_login_workers(), 2);
    }

    #[test]
    fn invalid_limits_are_rejected() {
        let limits = PasswordWorkLimits {
            workers: 2,
            signup_workers: 1,
            reset_workers: 1,
            ..PasswordWorkLimits::default()
        };
        let error = limits.validate().expect_err("no worker is left for login");
        assert!(error.to_string().contains("reserved worker"));
        assert!(PasswordWorkLimits {
            login_queue: 1025,
            ..PasswordWorkLimits::default()
        }
        .validate()
        .is_err());
        assert!(PasswordWorkLimits {
            workers: 0,
            ..PasswordWorkLimits::default()
        }
        .validate()
        .is_err());
    }
}
