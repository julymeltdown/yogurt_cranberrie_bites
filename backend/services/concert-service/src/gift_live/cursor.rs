//! Gift-specific opaque cursors. Adapted from feed/cursor.rs at 1cbb1823.
//! Actor/event/collection/filter/limit are authenticated, never exposed plaintext.
use chrono::{DateTime, Duration, Utc};
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Collection {
    Public,
    Giving,
    Receiving,
    Management,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CursorBinding {
    pub actor: Uuid,
    pub event: Uuid,
    pub collection: Collection,
    pub operation: String,
    /// Only management binds a specific giveaway.
    pub giveaway: Option<Uuid>,
    /// Canonical parsed filters; excludes cursor and limit keys.
    pub filters: BTreeMap<String, String>,
    pub limit: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Position {
    Gift { created_at: DateTime<Utc>, id: Uuid },
    Reservation { created_at: DateTime<Utc>, id: Uuid },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Window {
    pub upper: DateTime<Utc>,
    pub position: Position,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    version: u8,
    window: Window,
    issued: DateTime<Utc>,
    expires: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorError {
    Invalid,
    Unavailable,
}

/// Requires a dedicated injected deployment key; no plaintext/default fallback.
pub struct CursorCodec {
    key: aead::LessSafeKey,
}
impl CursorCodec {
    pub fn new(secret: &[u8; 32]) -> Result<Self, CursorError> {
        if secret.iter().all(|v| *v == 0) {
            return Err(CursorError::Unavailable);
        }
        let key = aead::UnboundKey::new(&aead::AES_256_GCM, secret)
            .map_err(|_| CursorError::Unavailable)?;
        Ok(Self {
            key: aead::LessSafeKey::new(key),
        })
    }
    pub fn seal(
        &self,
        binding: &CursorBinding,
        window: Window,
        now: DateTime<Utc>,
    ) -> Result<String, CursorError> {
        validate(binding, window, now)?;
        let mut nonce = [0; 12];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| CursorError::Unavailable)?;
        let aad = aad(binding)?;
        let mut bytes = serde_json::to_vec(&Payload {
            version: 2,
            window,
            issued: now,
            expires: now + Duration::minutes(15),
        })
        .map_err(|_| CursorError::Invalid)?;
        self.key
            .seal_in_place_append_tag(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(aad),
                &mut bytes,
            )
            .map_err(|_| CursorError::Unavailable)?;
        let mut value = nonce.to_vec();
        value.extend(bytes);
        let token = encode(&value);
        if token.len() > 1024 {
            return Err(CursorError::Invalid);
        }
        Ok(token)
    }
    pub fn open(
        &self,
        binding: &CursorBinding,
        token: &str,
        now: DateTime<Utc>,
    ) -> Result<Window, CursorError> {
        let mut bytes = decode(token)?;
        if bytes.len() < 29 {
            return Err(CursorError::Invalid);
        }
        let nonce: [u8; 12] = bytes[..12].try_into().map_err(|_| CursorError::Invalid)?;
        let aad = aad(binding)?;
        let plaintext = self
            .key
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(aad),
                &mut bytes[12..],
            )
            .map_err(|_| CursorError::Invalid)?;
        let payload: Payload =
            serde_json::from_slice(plaintext).map_err(|_| CursorError::Invalid)?;
        if payload.version != 2
            || payload.issued > now
            || payload.expires != payload.issued + Duration::minutes(15)
            || now >= payload.expires
        {
            return Err(CursorError::Invalid);
        }
        validate(binding, payload.window, now)?;
        Ok(payload.window)
    }
}

fn aad(binding: &CursorBinding) -> Result<Vec<u8>, CursorError> {
    let mut out = b"mcr.gift-live.cursor.v2\0".to_vec();
    out.extend(serde_json::to_vec(binding).map_err(|_| CursorError::Invalid)?);
    Ok(out)
}

fn validate(b: &CursorBinding, w: Window, now: DateTime<Utc>) -> Result<(), CursorError> {
    let allowed: &[&str] = match b.collection {
        Collection::Public => &[
            "state",
            "in_stock",
            "pickup_after",
            "pickup_before",
            "public_place_id",
        ],
        Collection::Management => &["state"],
        Collection::Giving | Collection::Receiving => &[],
    };
    let operation = match b.collection {
        Collection::Public => "listConcertGiveaways",
        Collection::Management => "getConcertGiveawayManagement",
        Collection::Giving | Collection::Receiving => "getConcertGiftActivity",
    };
    if b.operation != operation
        || w.upper > now
        || b.actor.is_nil()
        || b.event.is_nil()
        || !(1..=100).contains(&b.limit)
        || (b.collection == Collection::Management) != b.giveaway.is_some()
        || b.giveaway.is_some_and(|id| id.is_nil())
        || b.filters.iter().any(|(k, v)| {
            !allowed.contains(&k.as_str())
                || v.is_empty()
                || v.len() > 128
                || v.chars().any(char::is_control)
        })
    {
        return Err(CursorError::Invalid);
    }
    match (b.collection, w.position) {
        (Collection::Public | Collection::Giving, Position::Gift { created_at, id })
            if !id.is_nil() && created_at <= w.upper =>
        {
            Ok(())
        }
        (
            Collection::Receiving | Collection::Management,
            Position::Reservation { created_at, id },
        ) if !id.is_nil() && created_at <= w.upper => Ok(()),
        _ => Err(CursorError::Invalid),
    }
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
fn decode(value: &str) -> Result<Vec<u8>, CursorError> {
    if value.is_empty() || value.len() > 1024 || value.len() % 4 == 1 {
        return Err(CursorError::Invalid);
    }
    let mut out = Vec::new();
    let mut accumulator = 0u32;
    let mut bits = 0;
    for byte in value.bytes() {
        let n = ALPHABET
            .iter()
            .position(|b| *b == byte)
            .ok_or(CursorError::Invalid)? as u32;
        accumulator = (accumulator << 6) | n;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((accumulator >> bits) as u8);
            accumulator &= (1 << bits) - 1;
        }
    }
    if accumulator != 0 || encode(&out) != value {
        return Err(CursorError::Invalid);
    }
    Ok(out)
}
