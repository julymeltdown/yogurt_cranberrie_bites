//! Verification start: one-use state, ten-minute expiry, validated return path.
//!
//! Adapted from Bigyeting `5eecdcaf` `application/use_cases/oauth_start.rs`
//! (`begin_naver_verification`, `BeginNaverVerification`, `begin_oauth`).
//!
//! What is preserved:
//!
//! * an authenticated start creates exactly one state row and returns the
//!   provider authorization URL plus the state's expiry;
//! * the state is a random value whose digest is what gets stored;
//! * the return path is validated before anything is written.
//!
//! What changed:
//!
//! * the donor bound the state to an event participant, a browser nonce and an
//!   account-context version. There is no event here, and this foundation has no
//!   `account_context_version` (recorded as deviation 4 in
//!   `docs/mcr/backend/wp02-schema.md`). The binding that remains is the
//!   authenticated `user_id`, which the callback re-reads from the stored row
//!   rather than from the request.
//! * the donor's `return_path` came from the composer's draft key. Here it is a
//!   closed same-origin path from a fixed allowlist, so a caller cannot use the
//!   flow as an open redirect.
//! * a bounded number of in-flight states per user replaces the donor's
//!   single-session assumption.
use crate::identity::naver_config::NaverOAuthConfig;
use crate::identity::repository::{IdentityRepository, STATE_LIFETIME_MINUTES};
use chrono::{DateTime, Utc};
use ring::rand::{SecureRandom, SystemRandom};
use sha2::{Digest, Sha256};

/// The closed set of return paths the flow may redirect to.
///
/// Every entry is a same-origin absolute path in this product. A path outside
/// the set is refused, which is what makes the callback's `Location` safe: it
/// can only ever be one of these strings, and each of them is a constant in this
/// file rather than a value from a request.
pub const RETURN_PATHS: &[&str] = &["/me/identity", "/me/settings", "/me/profile"];

/// The default return path when the caller asks for none.
pub const DEFAULT_RETURN_PATH: &str = "/me/identity";

/// Why a start was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartError {
    /// The requested return path is not in the allowlist.
    ReturnPathNotAllowed,
    /// The user already has the maximum number of in-flight verifications.
    TooManyInFlight,
    /// The state could not be generated from the system entropy source.
    Entropy,
    /// The store failed.
    Store,
}

impl StartError {
    /// A stable, non-sensitive classification for metrics.
    pub const fn classification(self) -> &'static str {
        match self {
            Self::ReturnPathNotAllowed => "return_path_not_allowed",
            Self::TooManyInFlight => "too_many_in_flight",
            Self::Entropy => "entropy",
            Self::Store => "store",
        }
    }
}

/// A started verification.
pub struct StartedVerification {
    /// The provider authorization URL. The state is embedded in it and is not
    /// returned separately: the browser only ever needs the URL.
    pub authorization_url: String,
    pub expires_at: DateTime<Utc>,
}

/// Validate a caller-supplied return path.
///
/// Two independent rules, both required:
///
/// 1. the value must be in [`RETURN_PATHS`]. This is the allowlist the task
///    document calls for, and it is what makes the redirect target a constant.
/// 2. the value must satisfy the schema's own `identity_verifications_return_path`
///    grammar (`^/(?!/)[A-Za-z0-9._~!$&'()*+,;=:@%/-]*$`, at most 512 chars).
///    The database would refuse a value that breaks it, but refusing here means
///    a caller gets a clear error instead of a constraint violation.
pub fn validate_return_path(value: &str) -> Result<&str, StartError> {
    if !RETURN_PATHS.contains(&value) {
        return Err(StartError::ReturnPathNotAllowed);
    }
    if value.len() > 512 || !value.starts_with('/') || value.starts_with("//") {
        return Err(StartError::ReturnPathNotAllowed);
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"._~!$&'()*+,;=:@%/-".contains(&byte))
    {
        return Err(StartError::ReturnPathNotAllowed);
    }
    Ok(value)
}

/// The start use case.
pub struct StartVerification {
    repository: IdentityRepository,
    config: NaverOAuthConfig,
}

impl StartVerification {
    pub fn new(repository: IdentityRepository, config: NaverOAuthConfig) -> Self {
        Self { repository, config }
    }

    /// Begin one verification for `user_id`.
    ///
    /// The raw state is generated here, hashed, stored as a digest, and placed
    /// only in the returned authorization URL. It is never logged and never
    /// persisted in raw form.
    pub async fn begin(
        &self,
        user_id: uuid::Uuid,
        return_path: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<StartedVerification, StartError> {
        let return_path = validate_return_path(return_path.unwrap_or(DEFAULT_RETURN_PATH))?;
        let state = random_state().map_err(|_| StartError::Entropy)?;
        let digest = state_digest(&state);
        let created = self
            .repository
            .begin_verification(user_id, digest, return_path, now)
            .await
            .map_err(|_| StartError::Store)?
            .ok_or(StartError::TooManyInFlight)?;
        debug_assert!(
            created.expires_at <= now + chrono::Duration::minutes(STATE_LIFETIME_MINUTES),
            "a state never outlives its configured lifetime"
        );
        Ok(StartedVerification {
            authorization_url: self.config.authorization_url(&state),
            expires_at: created.expires_at,
        })
    }
}

/// 32 bytes from the system entropy source, rendered as unpadded base64url.
///
/// 256 bits of entropy in a 43-character token: the state is the only thing
/// binding a provider callback to a browser, so it must be unguessable, not
/// merely unique.
pub fn random_state() -> Result<String, ()> {
    let mut bytes = [0u8; 32];
    SystemRandom::new().fill(&mut bytes).map_err(|_| ())?;
    Ok(base64url(&bytes))
}

/// The stored digest of a state token.
///
/// Domain-separated from the subject hash so the two 32-byte values can never be
/// confused for one another in a query, a log or a future migration.
pub fn state_digest(state: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"mcr identity verification state v1\0");
    hasher.update(state.as_bytes());
    hasher.finalize().into()
}

/// Unpadded base64url, the encoding the contract's `IdentityCallbackQuery.state`
/// grammar (`[A-Za-z0-9._~-]`) admits.
fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let mut buffer = [0u8; 4];
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        buffer[0] = ALPHABET[((value >> 18) & 0x3f) as usize];
        buffer[1] = ALPHABET[((value >> 12) & 0x3f) as usize];
        buffer[2] = ALPHABET[((value >> 6) & 0x3f) as usize];
        buffer[3] = ALPHABET[(value & 0x3f) as usize];
        let keep = chunk.len() + 1;
        encoded.push_str(std::str::from_utf8(&buffer[..keep]).expect("base64url alphabet is ASCII"));
    }
    encoded
}
