//! Strict, Gateway-local identity/time input shared by the HTTP extractor and
//! the configured-key JWT adapter.
//!
//! This is neither an owner result nor admission authority. Trusted request
//! entry must capture its original monotonic stop before verification, refresh,
//! channel readiness or owner waits, and carry that same stop here. No HTTP
//! entry wiring or key-withdrawal/revocation protocol is supplied by this module.

use crate::domain::access_token::TokenRejection;
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use std::{fmt, time::Duration};
use uuid::Uuid;

/// A parse/work bound, not a JWT lifetime or product revocation promise.
pub const MAX_CONCERT_ACCESS_TOKEN_BYTES: usize = 16 * 1024;

/// Fixed safe failures; no bearer, claim or library diagnostic is retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConcertAccessRejection {
    #[error("Access token rejected: {0}")]
    Token(TokenRejection),
    #[error("Invalid access identity or time claims")]
    InvalidClaims,
    #[error("Original request deadline required")]
    MissingDeadline,
    #[error("Access context deadline expired")]
    DeadlineExceeded,
}

/// A nonserialized local input created only by configured-key JWT verification.
///
/// Recheck after every await, including the final dispatch/signing boundary.
/// Identity accessors and RPC-deadline access recheck both clocks themselves.
/// There are deliberately no roles, provider assertions, raw claims or bearer.
///
/// External callers cannot construct an authority context from plain claims:
/// ```compile_fail
/// use api_gateway::concert_access::ConcertAccessContext;
/// let context = ConcertAccessContext {};
/// ```
/// It cannot be serialized or reconstructed as a portable capability:
/// ```compile_fail
/// use api_gateway::concert_access::ConcertAccessContext;
/// fn export(context: &ConcertAccessContext) { let _ = serde_json::to_string(context); }
/// ```
/// ```compile_fail
/// use api_gateway::concert_access::ConcertAccessContext;
/// let _ = serde_json::from_str::<ConcertAccessContext>("{}");
/// ```
pub struct ConcertAccessContext {
    user_id: Uuid,
    session_family: Uuid,
    access_expires_micros: i64,
    not_before_micros: Option<i64>,
    original_stop: std::time::Instant,
    jwt_stop: std::time::Instant,
    deadline: AuthorityRpcDeadline,
}

impl fmt::Debug for ConcertAccessContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ConcertAccessContext { [REDACTED] }")
    }
}

impl ConcertAccessContext {
    /// Assemble a verified context. Only the configured-key JWT adapter may call
    /// this; every value is derived by that verification and its captured clocks.
    pub(crate) fn from_verified(
        user_id: Uuid,
        session_family: Uuid,
        access_expires_micros: i64,
        not_before_micros: Option<i64>,
        original_stop: std::time::Instant,
        jwt_stop: std::time::Instant,
        deadline: AuthorityRpcDeadline,
    ) -> Self {
        Self {
            user_id,
            session_family,
            access_expires_micros,
            not_before_micros,
            original_stop,
            jwt_stop,
            deadline,
        }
    }

    pub fn check(&self) -> Result<(), ConcertAccessRejection> {
        self.remaining(Duration::MAX).map(|_| ())
    }

    pub fn user_id(&self) -> Result<Uuid, ConcertAccessRejection> {
        self.check()?;
        Ok(self.user_id)
    }

    pub fn session_family(&self) -> Result<Uuid, ConcertAccessRejection> {
        self.check()?;
        Ok(self.session_family)
    }

    /// The authenticated signed expiry, not a newly calculated lifetime.
    pub fn access_expires_micros(&self) -> i64 {
        self.access_expires_micros
    }

    /// A later edge ceiling can only shorten the original captured budget.
    pub fn remaining(&self, ceiling: Duration) -> Result<Duration, ConcertAccessRejection> {
        let now = tokio::time::Instant::now().into_std();
        if self
            .not_before_micros
            .is_some_and(|nbf| nbf > chrono::Utc::now().timestamp_micros())
        {
            return Err(ConcertAccessRejection::InvalidClaims);
        }
        if now >= self.original_stop || now >= self.jwt_stop {
            return Err(ConcertAccessRejection::DeadlineExceeded);
        }
        self.deadline
            .remaining(ceiling, self.access_expires_micros)
            .map_err(|_| ConcertAccessRejection::DeadlineExceeded)
    }

    /// The same captured deadline; never start a fresh request budget after a wait.
    pub fn rpc_deadline(&self) -> Result<AuthorityRpcDeadline, ConcertAccessRejection> {
        self.check()?;
        Ok(self.deadline)
    }
}
