//! Opaque request-bound cursor encoding for BE27. This is an authenticated
//! framing surface; the application accepts only the injected encoder and keeps
//! raw row fields out of the token.
use super::{IntroError, Position};
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorBinding {
    pub actor: Uuid,
    pub event: Uuid,
    pub operation: &'static str,
    pub collection: &'static str,
    pub query: Option<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorWindow {
    pub upper: DateTime<Utc>,
    pub after: Option<Position>,
}
pub trait CursorCodec: Send + Sync {
    fn seal(
        &self,
        binding: &CursorBinding,
        window: CursorWindow,
        now: DateTime<Utc>,
    ) -> Result<String, IntroError>;
    fn open(
        &self,
        binding: &CursorBinding,
        token: &str,
        now: DateTime<Utc>,
    ) -> Result<CursorWindow, IntroError>;
}
/// The concrete wire form is H(c)|T20|I36|B. H is a keyed digest of the complete
/// request binding and row position, T is the fixed read-fence upper bound, and B
/// is the token lifetime. Opening rejects a changed actor/event/query, an older fence
/// beyond 15 minutes, tampering and malformed widths. A production composition must
/// inject a keyed implementation; the codec here remains an explicit port.
pub fn encode_authenticated(
    key: &[u8; 32],
    binding: &CursorBinding,
    window: CursorWindow,
    now: DateTime<Utc>,
) -> Result<String, IntroError> {
    validate_tag(binding)?;
    if window.upper > now || window.after.is_some_and(|p| p.intro_id.is_nil()) {
        return Err(IntroError::Invalid);
    }
    let deadline = checked_expiry(now)?;
    let body = format!(
        "{:020}|{}|{}|{:020}",
        window.upper.timestamp_micros(),
        position(window.after),
        encode_binding_tag(binding),
        deadline.timestamp_micros()
    );
    let mac = cursor_mac(key, &body);
    Ok(format!("{mac}|{body}"))
}
pub fn decode_authenticated(
    key: &[u8; 32],
    binding: &CursorBinding,
    token: &str,
    now: DateTime<Utc>,
) -> Result<CursorWindow, IntroError> {
    if token.is_empty() || token.len() > super::INTRO_CURSOR_MAX {
        return Err(IntroError::Invalid);
    }
    let fields = token.split('|').collect::<Vec<_>>();
    if fields.len() != 9 {
        return Err(IntroError::Invalid);
    }
    let mac = fields[0];
    let upper = fields[1];
    let position = fields[2];
    let query_tag = fields[3..8].join("|");
    validate_tag(binding)?;
    let deadline = parse_micros(fields[8])?;
    if now > deadline || query_tag != encode_binding_tag(binding) {
        return Err(IntroError::Invalid);
    }
    // The tag is authenticated as one field; only split its components after the
    // HMAC verifies, so differing UUID/key syntax cannot alter framing first.
    let expected = cursor_mac(
        key,
        &format!("{upper}|{position}|{query_tag}|{}", fields[8]),
    );
    if !constant_time_equal(mac.as_bytes(), expected.as_bytes()) {
        return Err(IntroError::Invalid);
    }
    let value = CursorWindow {
        upper: parse_micros(upper)?,
        after: decode_position(position)?,
    };
    validate_binding(binding, value, now)?;
    Ok(value)
}
fn encode_binding_tag(binding: &CursorBinding) -> String {
    let query = binding
        .query
        .map(|value| format!("{value:016x}"))
        .unwrap_or_else(|| "-".into());
    format!(
        "{}|{}|{}|{}|{}",
        binding.actor, binding.event, binding.operation, binding.collection, query
    )
}
pub fn validate_tag(binding: &CursorBinding) -> Result<(), IntroError> {
    if binding.actor.is_nil()
        || binding.event.is_nil()
        || binding.operation != "listConcertIntros"
        || binding.collection != "intros"
    {
        Err(IntroError::Invalid)
    } else {
        Ok(())
    }
}
fn position(value: Option<Position>) -> String {
    encode_position(value)
}
fn encode_position(value: Option<Position>) -> String {
    value.map_or_else(
        || "-".to_owned(),
        |p| format!("{:020}:{}", p.created_at.timestamp_micros(), p.intro_id),
    )
}
fn decode_position(value: &str) -> Result<Option<Position>, IntroError> {
    if value == "-" {
        return Ok(None);
    }
    let (time, id) = value.split_once(':').ok_or(IntroError::Invalid)?;
    Ok(Some(Position {
        created_at: parse_micros(time)?,
        intro_id: Uuid::parse_str(id).map_err(|_| IntroError::Invalid)?,
    }))
}
fn parse_micros(value: &str) -> Result<DateTime<Utc>, IntroError> {
    if value.len() != 20 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(IntroError::Invalid);
    }
    DateTime::from_timestamp_micros(value.parse().map_err(|_| IntroError::Invalid)?)
        .ok_or(IntroError::Invalid)
}
fn cursor_mac(key: &[u8; 32], body: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts the fixed key length");
    mac.update(super::INTRO_CURSOR_DOMAIN);
    mac.update(body.as_bytes());
    let bytes = mac.finalize().into_bytes();
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(64);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 15) as usize] as char);
    }
    encoded
}
fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .fold(0_u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}
pub struct UnavailableCursorCodec;
impl CursorCodec for UnavailableCursorCodec {
    fn seal(
        &self,
        _: &CursorBinding,
        _: CursorWindow,
        _: DateTime<Utc>,
    ) -> Result<String, IntroError> {
        Err(IntroError::Unavailable)
    }
    fn open(
        &self,
        _: &CursorBinding,
        _: &str,
        _: DateTime<Utc>,
    ) -> Result<CursorWindow, IntroError> {
        Err(IntroError::Unavailable)
    }
}
pub fn validate_binding(
    binding: &CursorBinding,
    window: CursorWindow,
    now: DateTime<Utc>,
) -> Result<(), IntroError> {
    validate_tag(binding)?;
    if window.upper > now
        || window
            .after
            .is_some_and(|p| p.intro_id.is_nil() || p.created_at > window.upper)
    {
        return Err(IntroError::Invalid);
    }
    Ok(())
}
pub fn checked_expiry(now: DateTime<Utc>) -> Result<DateTime<Utc>, IntroError> {
    now.checked_add_signed(Duration::minutes(15))
        .ok_or(IntroError::Invalid)
}
