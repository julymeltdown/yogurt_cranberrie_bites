//! Signed public search cursors bind a service keyset position to the query,
//! viewer scope, ordering, and bounded live-paging policy that issued it.

use crate::ports::GatewayRuntimePort;
use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

const V1_PREFIX: &str = "v1.";
const HMAC_DOMAIN: &[u8] = b"port-rust:api-gateway:search-cursor:v1\0";
const QUERY_BINDING_DOMAIN: &[u8] = b"port-rust:api-gateway:search-cursor:query:v1\0";
const CALLER_BINDING_DOMAIN: &[u8] = b"port-rust:api-gateway:search-cursor:caller:v1\0";
const MAX_CURSOR_LENGTH: usize = 512;
const MAX_DECODED_CURSOR_LENGTH: usize = 384;
const MAX_POSITION_LENGTH: usize = 100;
const MAX_CURSOR_TTL_SECONDS: u64 = 3_600;
const MAX_LEGACY_COMPATIBILITY_DAYS: i64 = 7;
const MAX_ISSUED_AT_CLOCK_SKEW_SECONDS: i64 = 30;
const CURSOR_VERSION: u8 = 1;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchSort {
    PostsCreatedAtDescIdDescV1,
    ProfilesUsernameAscV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "live-v1")]
enum SearchConsistency {
    LiveV1,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchCursorPayload {
    #[serde(rename = "v")]
    version: u8,
    #[serde(rename = "s")]
    sort: SearchSort,
    #[serde(rename = "c")]
    consistency: SearchConsistency,
    #[serde(rename = "i")]
    issued_at: i64,
    #[serde(rename = "e")]
    expires_at: i64,
    #[serde(rename = "q")]
    query_fingerprint: String,
    #[serde(rename = "u")]
    caller_fingerprint: String,
    #[serde(rename = "p")]
    position: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchCursorEnvelope {
    #[serde(rename = "p")]
    payload: SearchCursorPayload,
    #[serde(rename = "h")]
    signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacySearchCursorPayload {
    version: u8,
    sort: SearchSort,
    consistency: SearchConsistency,
    query_fingerprint: String,
    caller_fingerprint: String,
    position: String,
}

#[derive(Clone)]
pub struct SearchCursorCodec {
    hmac_key: Arc<[u8]>,
    runtime: Arc<dyn GatewayRuntimePort>,
    ttl_seconds: u64,
    legacy_accept_until: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for SearchCursorCodec {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SearchCursorCodec")
            .field("hmac_key", &"[REDACTED]")
            .field("ttl_seconds", &self.ttl_seconds)
            .field("legacy_accept_until", &self.legacy_accept_until)
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid search cursor")]
pub struct InvalidSearchCursor;

/// Encoding failures describe the invariant, never the position, key or library cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchCursorEncodingError {
    Position,
    SigningKey,
    Serialization,
    PublicBound,
}
impl std::fmt::Display for SearchCursorEncodingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Position => "search service returned an invalid cursor position",
            Self::SigningKey => "search cursor signing key is invalid",
            Self::Serialization => "search cursor encoding failed",
            Self::PublicBound => "search cursor exceeds the public contract limit",
        })
    }
}
impl std::error::Error for SearchCursorEncodingError {}

impl SearchCursorCodec {
    pub fn new(
        hmac_key: Arc<[u8]>,
        ttl_seconds: u64,
        legacy_accept_until: Option<DateTime<Utc>>,
        runtime: Arc<dyn GatewayRuntimePort>,
    ) -> anyhow::Result<Self> {
        if !(32..=4096).contains(&hmac_key.len()) {
            anyhow::bail!("GATEWAY_SEARCH_CURSOR_HMAC_KEY must contain 32 to 4096 bytes");
        }
        if !(60..=MAX_CURSOR_TTL_SECONDS).contains(&ttl_seconds) {
            anyhow::bail!("GATEWAY_SEARCH_CURSOR_TTL_SECONDS must be between 60 and 3600");
        }
        if legacy_accept_until.is_some_and(|deadline| {
            deadline > runtime.utc_now() + ChronoDuration::days(MAX_LEGACY_COMPATIBILITY_DAYS)
        }) {
            anyhow::bail!(
                "GATEWAY_SEARCH_CURSOR_LEGACY_ACCEPT_UNTIL must be within the next seven days"
            );
        }
        Ok(Self {
            hmac_key,
            runtime,
            ttl_seconds,
            legacy_accept_until,
        })
    }

    /// Decode a signed cursor, or a structurally valid pre-v1 position during
    /// the explicitly configured compatibility window.
    pub fn decode(
        &self,
        cursor: Option<&str>,
        query: &str,
        caller_id: Option<&str>,
        sort: SearchSort,
    ) -> Result<Option<String>, InvalidSearchCursor> {
        self.decode_at(cursor, query, caller_id, sort, self.runtime.utc_now())
    }

    /// Wrap a service-owned position in a signed v1 cursor. An empty service
    /// position represents the end of the result set.
    pub fn encode(
        &self,
        position: Option<&str>,
        query: &str,
        caller_id: Option<&str>,
        sort: SearchSort,
    ) -> Result<Option<String>, SearchCursorEncodingError> {
        self.encode_at(position, query, caller_id, sort, self.runtime.utc_now())
    }

    fn decode_at(
        &self,
        cursor: Option<&str>,
        query: &str,
        caller_id: Option<&str>,
        sort: SearchSort,
        now: DateTime<Utc>,
    ) -> Result<Option<String>, InvalidSearchCursor> {
        let Some(cursor) = cursor else {
            return Ok(None);
        };
        if cursor.len() > MAX_CURSOR_LENGTH {
            return Err(InvalidSearchCursor);
        }

        let Some(encoded) = cursor.strip_prefix(V1_PREFIX) else {
            return self.decode_legacy(cursor, query, caller_id, sort, now);
        };
        let decoded = Base64UrlUnpadded::decode_vec(encoded).map_err(|_| InvalidSearchCursor)?;
        if decoded.len() > MAX_DECODED_CURSOR_LENGTH {
            return Err(InvalidSearchCursor);
        }
        let envelope: SearchCursorEnvelope =
            serde_json::from_slice(&decoded).map_err(|_| InvalidSearchCursor)?;
        let canonical = serde_json::to_vec(&envelope).map_err(|_| InvalidSearchCursor)?;
        if canonical != decoded || Base64UrlUnpadded::encode_string(&decoded) != encoded {
            return Err(InvalidSearchCursor);
        }

        let signature =
            Base64UrlUnpadded::decode_vec(&envelope.signature).map_err(|_| InvalidSearchCursor)?;
        if Base64UrlUnpadded::encode_string(&signature) != envelope.signature {
            return Err(InvalidSearchCursor);
        }
        let payload_bytes =
            serde_json::to_vec(&envelope.payload).map_err(|_| InvalidSearchCursor)?;
        let mut mac =
            HmacSha256::new_from_slice(&self.hmac_key).map_err(|_| InvalidSearchCursor)?;
        mac.update(HMAC_DOMAIN);
        mac.update(&payload_bytes);
        mac.verify_slice(&signature)
            .map_err(|_| InvalidSearchCursor)?;

        let payload = envelope.payload;
        let now_timestamp = now.timestamp();
        if payload.version != CURSOR_VERSION
            || payload.sort != sort
            || payload.consistency != SearchConsistency::LiveV1
            || payload.issued_at > now_timestamp + MAX_ISSUED_AT_CLOCK_SKEW_SECONDS
            || payload.expires_at <= now_timestamp
            || payload.expires_at <= payload.issued_at
            || payload
                .expires_at
                .checked_sub(payload.issued_at)
                .is_none_or(|lifetime| lifetime > self.ttl_seconds as i64)
            || payload.query_fingerprint
                != context_fingerprint(
                    &self.hmac_key,
                    QUERY_BINDING_DOMAIN,
                    &canonical_query(query),
                )
                .map_err(|_| InvalidSearchCursor)?
            || payload.caller_fingerprint
                != context_fingerprint(
                    &self.hmac_key,
                    CALLER_BINDING_DOMAIN,
                    &caller_scope(caller_id),
                )
                .map_err(|_| InvalidSearchCursor)?
            || !valid_position(&payload.position, sort)
        {
            return Err(InvalidSearchCursor);
        }
        Ok(Some(payload.position))
    }

    fn decode_legacy(
        &self,
        cursor: &str,
        query: &str,
        caller_id: Option<&str>,
        sort: SearchSort,
        now: DateTime<Utc>,
    ) -> Result<Option<String>, InvalidSearchCursor> {
        let accepted = self
            .legacy_accept_until
            .is_some_and(|deadline| now < deadline);
        if !accepted {
            return Err(InvalidSearchCursor);
        }
        if valid_position(cursor, sort) {
            // Older callers may still hold a raw service-owned position.
            return Ok(Some(cursor.to_owned()));
        }

        // A prior Gateway release wrapped raw service positions in an
        // unsigned, context-bound base64url JSON payload. Accept it only during
        // the same explicitly configured compatibility window.
        let decoded = Base64UrlUnpadded::decode_vec(cursor).map_err(|_| InvalidSearchCursor)?;
        if decoded.len() > MAX_DECODED_CURSOR_LENGTH
            || Base64UrlUnpadded::encode_string(&decoded) != cursor
        {
            return Err(InvalidSearchCursor);
        }
        let payload: LegacySearchCursorPayload =
            serde_json::from_slice(&decoded).map_err(|_| InvalidSearchCursor)?;
        let canonical = serde_json::to_vec(&payload).map_err(|_| InvalidSearchCursor)?;
        if canonical != decoded
            || payload.version != CURSOR_VERSION
            || payload.sort != sort
            || payload.consistency != SearchConsistency::LiveV1
            || payload.query_fingerprint != legacy_fingerprint(&canonical_query(query))
            || payload.caller_fingerprint != legacy_caller_fingerprint(caller_id)
            || !valid_position(&payload.position, sort)
        {
            return Err(InvalidSearchCursor);
        }
        Ok(Some(payload.position))
    }

    fn encode_at(
        &self,
        position: Option<&str>,
        query: &str,
        caller_id: Option<&str>,
        sort: SearchSort,
        now: DateTime<Utc>,
    ) -> Result<Option<String>, SearchCursorEncodingError> {
        let Some(position) = position.filter(|position| !position.is_empty()) else {
            return Ok(None);
        };
        if !valid_position(position, sort) {
            return Err(SearchCursorEncodingError::Position);
        }

        let issued_at = now.timestamp();
        let payload = SearchCursorPayload {
            version: CURSOR_VERSION,
            sort,
            consistency: SearchConsistency::LiveV1,
            issued_at,
            expires_at: issued_at + self.ttl_seconds as i64,
            query_fingerprint: context_fingerprint(
                &self.hmac_key,
                QUERY_BINDING_DOMAIN,
                &canonical_query(query),
            )
            .map_err(|_| SearchCursorEncodingError::SigningKey)?,
            caller_fingerprint: context_fingerprint(
                &self.hmac_key,
                CALLER_BINDING_DOMAIN,
                &caller_scope(caller_id),
            )
            .map_err(|_| SearchCursorEncodingError::SigningKey)?,
            position: position.to_owned(),
        };
        let payload_bytes =
            serde_json::to_vec(&payload).map_err(|_| SearchCursorEncodingError::Serialization)?;
        let mut mac = HmacSha256::new_from_slice(&self.hmac_key)
            .map_err(|_| SearchCursorEncodingError::SigningKey)?;
        mac.update(HMAC_DOMAIN);
        mac.update(&payload_bytes);
        let signature = Base64UrlUnpadded::encode_string(&mac.finalize().into_bytes());
        let envelope = SearchCursorEnvelope { payload, signature };
        let encoded = format!(
            "{V1_PREFIX}{}",
            Base64UrlUnpadded::encode_string(
                &serde_json::to_vec(&envelope)
                    .map_err(|_| SearchCursorEncodingError::Serialization)?
            )
        );
        if encoded.len() > MAX_CURSOR_LENGTH {
            return Err(SearchCursorEncodingError::PublicBound);
        }
        Ok(Some(encoded))
    }
}

fn canonical_query(query: &str) -> String {
    search_policy::normalize_search_term(query)
        .map(|term| term.to_lowercase())
        .unwrap_or_else(|_| query.trim().to_lowercase())
}

fn legacy_fingerprint(value: &str) -> String {
    hex_digest(Sha256::digest(value.as_bytes()).as_slice())
}

fn legacy_caller_fingerprint(caller_id: Option<&str>) -> String {
    legacy_fingerprint(&caller_scope(caller_id))
}

fn caller_scope(caller_id: Option<&str>) -> String {
    caller_id
        .map(|caller| format!("authenticated:{caller}"))
        .unwrap_or_else(|| "anonymous".to_owned())
}

fn context_fingerprint(
    key: &[u8],
    domain: &[u8],
    value: &str,
) -> Result<String, hmac::digest::InvalidLength> {
    let mut mac = HmacSha256::new_from_slice(key)?;
    mac.update(domain);
    mac.update(value.as_bytes());
    Ok(Base64UrlUnpadded::encode_string(
        &mac.finalize().into_bytes(),
    ))
}

fn hex_digest(digest: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn valid_position(position: &str, sort: SearchSort) -> bool {
    if position.is_empty()
        || position.len() > MAX_POSITION_LENGTH
        || position.trim() != position
        || position.chars().any(char::is_control)
    {
        return false;
    }

    match sort {
        SearchSort::PostsCreatedAtDescIdDescV1 => {
            let mut parts = position.split('|');
            let Some(created_at) = parts.next() else {
                return false;
            };
            let Some(post_id) = parts.next() else {
                return false;
            };
            parts.next().is_none()
                && DateTime::parse_from_rfc3339(created_at).is_ok()
                && Uuid::parse_str(post_id).is_ok()
        }
        SearchSort::ProfilesUsernameAscV1 => {
            (3..=20).contains(&position.len())
                && position
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        }
    }
}

#[cfg(test)]
fn signature(
    key: &[u8],
    payload: &SearchCursorPayload,
) -> Result<String, hmac::digest::InvalidLength> {
    let mut mac = HmacSha256::new_from_slice(key)?;
    mac.update(HMAC_DOMAIN);
    mac.update(&serde_json::to_vec(payload).expect("serialize signed test payload"));
    Ok(Base64UrlUnpadded::encode_string(
        &mac.finalize().into_bytes(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct TestSystemRuntime;
    impl GatewayRuntimePort for TestSystemRuntime {
        fn utc_now(&self) -> DateTime<Utc> {
            Utc::now()
        }
        fn monotonic_now(&self) -> std::time::Instant {
            std::time::Instant::now()
        }
        fn new_id(&self) -> Uuid {
            Uuid::new_v4()
        }
    }

    const KEY: &[u8] = b"gateway-search-cursor-test-key-32-bytes-minimum";
    const POST_POSITION: &str = "2026-10-04T12:30:45.123456Z|11111111-1111-1111-1111-111111111111";
    const PROFILE_POSITION: &str = "hana_ani";

    fn codec(legacy_accept_until: Option<DateTime<Utc>>) -> SearchCursorCodec {
        SearchCursorCodec::new(
            Arc::from(KEY),
            900,
            legacy_accept_until,
            Arc::new(TestSystemRuntime),
        )
        .expect("valid test cursor codec")
    }

    #[test]
    fn search_cursor_codec_debug_redacts_the_signing_key() {
        let rendered = format!("{:?}", codec(None));

        assert!(rendered.contains("SearchCursorCodec"));
        assert!(rendered.contains("[REDACTED]"));
        assert!(
            !rendered.contains(&format!("{KEY:?}")),
            "Debug output must not contain the signing-key bytes"
        );
    }

    #[test]
    fn signed_cursor_round_trips_query_caller_sort_and_live_policy() {
        let codec = codec(None);
        let cursor = codec
            .encode(
                Some(POST_POSITION),
                "  HANA  ",
                Some("viewer-1"),
                SearchSort::PostsCreatedAtDescIdDescV1,
            )
            .expect("encode")
            .expect("next cursor");

        assert!(cursor.starts_with(V1_PREFIX));
        assert!(cursor.len() <= MAX_CURSOR_LENGTH);
        assert_eq!(
            codec
                .decode(
                    Some(&cursor),
                    "hana",
                    Some("viewer-1"),
                    SearchSort::PostsCreatedAtDescIdDescV1,
                )
                .expect("decode"),
            Some(POST_POSITION.to_owned())
        );
    }

    #[test]
    fn rejects_query_caller_sort_key_and_payload_tampering() {
        let codec = codec(None);
        let cursor = codec
            .encode(
                Some(POST_POSITION),
                "hana",
                Some("viewer-1"),
                SearchSort::PostsCreatedAtDescIdDescV1,
            )
            .expect("encode")
            .expect("next cursor");

        assert!(codec
            .decode(
                Some(&cursor),
                "hannah",
                Some("viewer-1"),
                SearchSort::PostsCreatedAtDescIdDescV1,
            )
            .is_err());
        assert!(codec
            .decode(
                Some(&cursor),
                "hana",
                Some("viewer-2"),
                SearchSort::PostsCreatedAtDescIdDescV1,
            )
            .is_err());
        assert!(codec
            .decode(
                Some(&cursor),
                "hana",
                Some("viewer-1"),
                SearchSort::ProfilesUsernameAscV1,
            )
            .is_err());

        let other_key = SearchCursorCodec::new(
            Arc::from(b"a-different-search-cursor-hmac-key-32-bytes".as_slice()),
            900,
            None,
            Arc::new(TestSystemRuntime),
        )
        .expect("valid other key");
        assert!(other_key
            .decode(
                Some(&cursor),
                "hana",
                Some("viewer-1"),
                SearchSort::PostsCreatedAtDescIdDescV1,
            )
            .is_err());

        let decoded =
            Base64UrlUnpadded::decode_vec(cursor.strip_prefix(V1_PREFIX).expect("v1 prefix"))
                .expect("decode envelope");
        let mut envelope: SearchCursorEnvelope =
            serde_json::from_slice(&decoded).expect("parse envelope");
        envelope.payload.position =
            "2026-10-05T12:30:45.123456Z|22222222-2222-2222-2222-222222222222".into();
        let forged = format!(
            "{V1_PREFIX}{}",
            Base64UrlUnpadded::encode_string(&serde_json::to_vec(&envelope).expect("serialize"))
        );
        assert!(codec
            .decode(
                Some(&forged),
                "hana",
                Some("viewer-1"),
                SearchSort::PostsCreatedAtDescIdDescV1,
            )
            .is_err());
    }

    #[test]
    fn enforces_cursor_expiry_and_rejects_future_versions() {
        let codec = codec(None);
        let issued_at = Utc::now();
        let cursor = codec
            .encode_at(
                Some(POST_POSITION),
                "hana",
                Some("viewer-1"),
                SearchSort::PostsCreatedAtDescIdDescV1,
                issued_at,
            )
            .expect("encode")
            .expect("cursor");
        assert!(codec
            .decode_at(
                Some(&cursor),
                "hana",
                Some("viewer-1"),
                SearchSort::PostsCreatedAtDescIdDescV1,
                issued_at + ChronoDuration::seconds(901),
            )
            .is_err());

        let decoded =
            Base64UrlUnpadded::decode_vec(cursor.strip_prefix(V1_PREFIX).expect("v1 prefix"))
                .expect("decode envelope");
        let mut envelope: SearchCursorEnvelope =
            serde_json::from_slice(&decoded).expect("parse envelope");
        envelope.payload.version = CURSOR_VERSION + 1;
        envelope.signature = signature(&codec.hmac_key, &envelope.payload)
            .expect("sign future-version test payload");
        let future = format!(
            "{V1_PREFIX}{}",
            Base64UrlUnpadded::encode_string(&serde_json::to_vec(&envelope).expect("serialize"))
        );
        assert!(codec
            .decode(
                Some(&future),
                "hana",
                Some("viewer-1"),
                SearchSort::PostsCreatedAtDescIdDescV1,
            )
            .is_err());
    }

    #[test]
    fn accepts_valid_legacy_positions_only_before_the_configured_deadline() {
        let now = Utc::now();
        let deadline = now + ChronoDuration::hours(1);
        let codec = codec(Some(deadline));

        assert_eq!(
            codec
                .decode_at(
                    Some(PROFILE_POSITION),
                    "hana",
                    None,
                    SearchSort::ProfilesUsernameAscV1,
                    now,
                )
                .expect("legacy cursor within window"),
            Some(PROFILE_POSITION.to_owned())
        );
        assert_eq!(
            codec
                .decode_at(
                    Some(PROFILE_POSITION),
                    "different-query",
                    Some("different-viewer"),
                    SearchSort::ProfilesUsernameAscV1,
                    now,
                )
                .expect("raw legacy positions are deliberately not context-bound"),
            Some(PROFILE_POSITION.to_owned())
        );
        assert!(codec
            .decode_at(
                Some(PROFILE_POSITION),
                "hana",
                None,
                SearchSort::ProfilesUsernameAscV1,
                deadline,
            )
            .is_err());
        assert!(codec
            .decode_at(
                Some("bad-username"),
                "hana",
                None,
                SearchSort::ProfilesUsernameAscV1,
                now,
            )
            .is_err());
    }

    #[test]
    fn migrates_previous_unsigned_envelopes_only_within_the_context_bound_window() {
        let now = Utc::now();
        let disabled_codec = codec(None);
        let codec = codec(Some(now + ChronoDuration::hours(1)));
        let payload = LegacySearchCursorPayload {
            version: CURSOR_VERSION,
            sort: SearchSort::ProfilesUsernameAscV1,
            consistency: SearchConsistency::LiveV1,
            query_fingerprint: legacy_fingerprint(&canonical_query("hana")),
            caller_fingerprint: legacy_caller_fingerprint(Some("viewer-1")),
            position: PROFILE_POSITION.to_owned(),
        };
        let cursor = Base64UrlUnpadded::encode_string(
            &serde_json::to_vec(&payload).expect("serialize legacy payload"),
        );

        assert_eq!(
            codec
                .decode_at(
                    Some(&cursor),
                    "hana",
                    Some("viewer-1"),
                    SearchSort::ProfilesUsernameAscV1,
                    now,
                )
                .expect("decode compatible unsigned envelope"),
            Some(PROFILE_POSITION.to_owned())
        );
        assert!(codec
            .decode_at(
                Some(&cursor),
                "other-query",
                Some("viewer-1"),
                SearchSort::ProfilesUsernameAscV1,
                now,
            )
            .is_err());
        assert!(codec
            .decode_at(
                Some(&cursor),
                "hana",
                Some("viewer-1"),
                SearchSort::ProfilesUsernameAscV1,
                now + ChronoDuration::hours(1),
            )
            .is_err());
        assert!(disabled_codec
            .decode_at(
                Some(&cursor),
                "hana",
                Some("viewer-1"),
                SearchSort::ProfilesUsernameAscV1,
                now,
            )
            .is_err());
        assert!(codec
            .decode_at(
                Some(&cursor),
                "hana",
                Some("viewer-2"),
                SearchSort::ProfilesUsernameAscV1,
                now,
            )
            .is_err());
    }

    #[test]
    fn rejects_legacy_malformed_blank_oversized_and_noncanonical_tokens() {
        let codec = codec(None);
        for invalid in [POST_POSITION, "not-a-cursor!", "", "   "] {
            assert!(codec
                .decode(
                    Some(invalid),
                    "hana",
                    Some("viewer-1"),
                    SearchSort::PostsCreatedAtDescIdDescV1,
                )
                .is_err());
        }
        let oversized = "a".repeat(MAX_CURSOR_LENGTH + 1);
        assert!(codec
            .decode(
                Some(&oversized),
                "hana",
                Some("viewer-1"),
                SearchSort::PostsCreatedAtDescIdDescV1,
            )
            .is_err());

        let cursor = codec
            .encode(
                Some(POST_POSITION),
                "hana",
                Some("viewer-1"),
                SearchSort::PostsCreatedAtDescIdDescV1,
            )
            .expect("encode")
            .expect("cursor");
        let noncanonical = format!("{cursor}=");
        assert!(codec
            .decode(
                Some(&noncanonical),
                "hana",
                Some("viewer-1"),
                SearchSort::PostsCreatedAtDescIdDescV1,
            )
            .is_err());
    }

    #[test]
    fn empty_positions_end_paging_and_invalid_positions_fail_closed() {
        let codec = codec(None);
        assert_eq!(
            codec
                .encode(
                    None,
                    "hana",
                    Some("viewer-1"),
                    SearchSort::PostsCreatedAtDescIdDescV1,
                )
                .expect("no cursor"),
            None
        );
        assert!(codec
            .encode(
                Some("not-a-post-cursor"),
                "hana",
                Some("viewer-1"),
                SearchSort::PostsCreatedAtDescIdDescV1,
            )
            .is_err());
        assert!(codec
            .encode(
                Some(&"x".repeat(MAX_POSITION_LENGTH + 1)),
                "hana",
                Some("viewer-1"),
                SearchSort::ProfilesUsernameAscV1,
            )
            .is_err());
    }
}
