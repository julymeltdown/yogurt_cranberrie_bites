//! Configured-key JWT verification for strict Gateway-local Concert access.
//!
//! Shared context/rejection types live in crate::concert_access, outside the
//! adapter layer, so the HTTP entry never imports an outgoing implementation.

use super::{JwtVerifier, TokenRejection};
pub use crate::concert_access::{
    ConcertAccessContext, ConcertAccessRejection, MAX_CONCERT_ACCESS_TOKEN_BYTES,
};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use serde::{Deserialize, Deserializer};
use std::time::Duration;
use uuid::Uuid;

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
        let context = ConcertAccessContext::from_verified(
            user_id,
            session_family,
            access_expires_micros,
            not_before_micros,
            original_stop,
            jwt_stop,
            deadline,
        );
        context.check()?;
        Ok(context)
    }
}

fn seconds_to_micros(seconds: i64) -> Result<i64, ConcertAccessRejection> {
    seconds
        .checked_mul(1_000_000)
        .ok_or(ConcertAccessRejection::InvalidClaims)
}
