//! Opaque AEAD cursors: actor/event/operation/collection/canonical filters are AAD.
//! No identity, row UUID, timestamp or private field is exposed in plaintext.
use chrono::{DateTime, Duration, Utc};
use concert_domain::feed::{FeedError, Filter, Window};
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CursorBinding {
    pub actor: Uuid,
    pub event: Uuid,
    pub operation: String,
    pub collection: String,
    pub filter: Filter,
    pub before: Option<DateTime<Utc>>,
    pub limit: u16,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    version: u8,
    window: Window,
    expires: DateTime<Utc>,
}
/// Inject a dedicated 256-bit cursor key; no demo/default/env key or plaintext fallback.
pub struct CursorCodec {
    key: aead::LessSafeKey,
}
impl CursorCodec {
    pub fn new(secret: &[u8; 32]) -> Result<Self, FeedError> {
        let key = aead::UnboundKey::new(&aead::AES_256_GCM, secret)
            .map_err(|_| FeedError::DependencyUnavailable)?;
        Ok(Self {
            key: aead::LessSafeKey::new(key),
        })
    }
    pub fn seal(
        &self,
        binding: &CursorBinding,
        window: Window,
        now: DateTime<Utc>,
    ) -> Result<String, FeedError> {
        validate(binding, window, now)?;
        let mut nonce = [0; 12];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| FeedError::DependencyUnavailable)?;
        let aad = serde_json::to_vec(binding).map_err(|_| FeedError::InvalidInput)?;
        let mut bytes = serde_json::to_vec(&Payload {
            version: 1,
            window,
            expires: now + Duration::minutes(15),
        })
        .map_err(|_| FeedError::InvalidInput)?;
        self.key
            .seal_in_place_append_tag(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(aad),
                &mut bytes,
            )
            .map_err(|_| FeedError::DependencyUnavailable)?;
        let mut value = nonce.to_vec();
        value.extend(bytes);
        let token = encode(&value);
        if token.len() > 1024 {
            return Err(FeedError::InvalidInput);
        }
        Ok(token)
    }
    pub fn open(
        &self,
        binding: &CursorBinding,
        token: &str,
        now: DateTime<Utc>,
    ) -> Result<Window, FeedError> {
        let mut bytes = decode(token)?;
        if bytes.len() < 29 {
            return Err(FeedError::InvalidInput);
        }
        let nonce: [u8; 12] = bytes[..12]
            .try_into()
            .map_err(|_| FeedError::InvalidInput)?;
        let aad = serde_json::to_vec(binding).map_err(|_| FeedError::InvalidInput)?;
        let plaintext = self
            .key
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(aad),
                &mut bytes[12..],
            )
            .map_err(|_| FeedError::InvalidInput)?;
        let payload: Payload =
            serde_json::from_slice(plaintext).map_err(|_| FeedError::InvalidInput)?;
        if payload.version != 1
            || now >= payload.expires
            || payload.expires > now + Duration::minutes(15)
        {
            return Err(FeedError::InvalidInput);
        }
        validate(binding, payload.window, now)?;
        Ok(payload.window)
    }
}
fn validate(b: &CursorBinding, w: Window, now: DateTime<Utc>) -> Result<(), FeedError> {
    if b.actor.is_nil()
        || b.event.is_nil()
        || !(1..=100).contains(&b.limit)
        || !matches!(
            b.operation.as_str(),
            "listConcertFeed" | "listConcertComments"
        )
        || b.collection.is_empty()
        || b.collection.len() > 80
        || w.upper > now
        || b.before.is_some_and(|v| w.upper > v)
        || w.after
            .is_some_and(|p| p.created_at > w.upper || p.id.is_nil())
    {
        return Err(FeedError::InvalidInput);
    }
    Ok(())
}
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
fn encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for shift in (0..4).take(chunk.len() + 1) {
            out.push(ALPHABET[((value >> (18 - shift * 6)) & 63) as usize] as char);
        }
    }
    out
}
fn decode(value: &str) -> Result<Vec<u8>, FeedError> {
    if value.is_empty() || value.len() > 1024 || value.len() % 4 == 1 {
        return Err(FeedError::InvalidInput);
    }
    let mut out = Vec::new();
    let mut accumulator = 0u32;
    let mut bits = 0;
    for byte in value.bytes() {
        let n = ALPHABET
            .iter()
            .position(|b| *b == byte)
            .ok_or(FeedError::InvalidInput)? as u32;
        accumulator = (accumulator << 6) | n;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((accumulator >> bits) as u8);
            accumulator &= (1 << bits) - 1;
        }
    }
    if accumulator != 0 || encode(&out) != value {
        return Err(FeedError::InvalidInput);
    }
    Ok(out)
}
