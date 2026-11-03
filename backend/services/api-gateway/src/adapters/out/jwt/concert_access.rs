//! Strict, Gateway-local identity/time input for future Concert orchestration.
//!
//! This is neither an owner result nor admission authority. Trusted request
//! entry must capture its original monotonic stop before verification, refresh,
//! channel readiness or owner waits, and carry that same stop here. No HTTP
//! entry wiring or key-withdrawal/revocation protocol is supplied by this module.

use super::{JwtVerifier, TokenRejection};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use serde::{Deserialize, Deserializer};
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
/// use api_gateway::adapters::out::jwt::concert_access::ConcertAccessContext;
/// let context = ConcertAccessContext {};
/// ```
/// It cannot be serialized or reconstructed as a portable capability:
/// ```compile_fail
/// use api_gateway::adapters::out::jwt::concert_access::ConcertAccessContext;
/// fn export(context: &ConcertAccessContext) { let _ = serde_json::to_string(context); }
/// ```
/// ```compile_fail
/// use api_gateway::adapters::out::jwt::concert_access::ConcertAccessContext;
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

#[derive(Deserialize)]
struct StrictClaims {
    // Auth's issuer uses i64 seconds. Unlike the library's permissive numeric
    // visitor, typed deserialization rejects fractional/string/overflow values.
    exp: i64,
    iss: String,
    #[serde(default)]
    nbf: NotBefore,
}

#[derive(Default)]
enum NotBefore {
    #[default]
    Absent,
    Seconds(i64),
}
impl<'de> Deserialize<'de> for NotBefore {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        i64::deserialize(deserializer).map(Self::Seconds)
    }
}

impl JwtVerifier {
    /// Verify once through the canonical installed keys, then retain strict
    /// identity and time bounds. `original_stop` must come from trusted entry,
    /// never from a browser claim or a fresh duration after an earlier await.
    pub fn verify_concert_access(
        &self,
        token: &str,
        original_stop: Option<std::time::Instant>,
    ) -> Result<ConcertAccessContext, ConcertAccessRejection> {
        let original_stop = original_stop.ok_or(ConcertAccessRejection::MissingDeadline)?;
        // Sample monotonic first: verification time and sampling latency must
        // consume authority, not extend the captured JWT expiry.
        let captured_mono = tokio::time::Instant::now().into_std();
        let captured_wall = chrono::Utc::now().timestamp_micros();
        if original_stop <= captured_mono {
            return Err(ConcertAccessRejection::DeadlineExceeded);
        }
        if token.len() > MAX_CONCERT_ACCESS_TOKEN_BYTES {
            return Err(ConcertAccessRejection::Token(TokenRejection::Malformed));
        }
        let claims = self
            .verified_access_claims::<StrictClaims>(token)
            .map_err(ConcertAccessRejection::Token)?;
        let canonical = |text: &str| {
            Uuid::parse_str(text)
                .ok()
                .filter(|id| !id.is_nil() && id.to_string() == text)
                .ok_or(ConcertAccessRejection::InvalidClaims)
        };
        let user_id = canonical(&claims.sub)?;
        let session_family = canonical(
            claims
                .sid
                .as_deref()
                .ok_or(ConcertAccessRejection::InvalidClaims)?,
        )?;
        if claims.extra.iss != self.issuer {
            return Err(ConcertAccessRejection::InvalidClaims);
        }
        let access_expires_micros = seconds_to_micros(claims.extra.exp)?;
        let now_wall = chrono::Utc::now().timestamp_micros();
        let not_before_micros = match claims.extra.nbf {
            NotBefore::Absent => None,
            NotBefore::Seconds(seconds) => Some(seconds_to_micros(seconds)?),
        };
        if not_before_micros.is_some_and(|nbf| nbf > now_wall) {
            return Err(ConcertAccessRejection::InvalidClaims);
        }
        // A live future exp is normal; no iat requirement or lifetime cap is
        // imposed. Arithmetic/platform representation failures reject outright.
        let live_micros = access_expires_micros
            .checked_sub(captured_wall)
            .filter(|micros| *micros > 0)
            .ok_or(ConcertAccessRejection::DeadlineExceeded)?;
        let jwt_stop = captured_mono
            .checked_add(Duration::from_micros(live_micros as u64))
            .ok_or(ConcertAccessRejection::InvalidClaims)?;
        let deadline = AuthorityRpcDeadline::within_original(
            access_expires_micros,
            original_stop.min(jwt_stop),
            original_stop.duration_since(captured_mono),
        )
        .map_err(|_| ConcertAccessRejection::DeadlineExceeded)?;
        let context = ConcertAccessContext {
            user_id,
            session_family,
            access_expires_micros,
            not_before_micros,
            original_stop,
            jwt_stop,
            deadline,
        };
        context.check()?;
        Ok(context)
    }
}

fn seconds_to_micros(seconds: i64) -> Result<i64, ConcertAccessRejection> {
    seconds
        .checked_mul(1_000_000)
        .ok_or(ConcertAccessRejection::InvalidClaims)
}
