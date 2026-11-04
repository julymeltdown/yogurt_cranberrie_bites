//! Request-bound opaque party cursors. Actor, event, canonical filters, limit and
//! immutable first-page upper bound are authenticated as AEAD associated data.
use super::PartyLiveError;
use chrono::{DateTime, Duration, Utc};
use concert_domain::community::party::PartyState;
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartyCursorBinding {
    pub actor: Uuid,
    pub event: Uuid,
    pub operation: String,
    pub collection: String,
    pub ticket_section: Option<String>,
    pub state: Option<PartyState>,
    pub has_space: Option<bool>,
    pub starts_after: Option<DateTime<Utc>>,
    pub starts_before: Option<DateTime<Utc>>,
    pub limit: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartyPosition {
    pub created_at: DateTime<Utc>,
    pub party_id: Uuid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartyWindow {
    pub upper: DateTime<Utc>,
    pub after: Option<PartyPosition>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    version: u8,
    window: PartyWindow,
    expires: DateTime<Utc>,
}

#[derive(Clone)]
pub struct PartyCursorCodec {
    key: aead::LessSafeKey,
}

impl PartyCursorCodec {
    pub fn new(secret: &[u8; 32]) -> Result<Self, PartyLiveError> {
        let key = aead::UnboundKey::new(&aead::AES_256_GCM, secret)
            .map_err(|_| PartyLiveError::DependencyUnavailable)?;
        Ok(Self {
            key: aead::LessSafeKey::new(key),
        })
    }

    pub fn seal(
        &self,
        binding: &PartyCursorBinding,
        window: PartyWindow,
        now: DateTime<Utc>,
    ) -> Result<String, PartyLiveError> {
        validate(binding, window, now)?;
        let mut nonce = [0; 12];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| PartyLiveError::DependencyUnavailable)?;
        let aad = serde_json::to_vec(binding).map_err(|_| PartyLiveError::InvalidInput)?;
        let mut bytes = serde_json::to_vec(&Payload {
            version: 1,
            window,
            expires: now + Duration::minutes(15),
        })
        .map_err(|_| PartyLiveError::InvalidInput)?;
        self.key
            .seal_in_place_append_tag(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(aad),
                &mut bytes,
            )
            .map_err(|_| PartyLiveError::DependencyUnavailable)?;
        let mut value = nonce.to_vec();
        value.extend(bytes);
        let token = encode(&value);
        if token.len() > 1024 {
            return Err(PartyLiveError::InvalidInput);
        }
        Ok(token)
    }

    pub fn open(
        &self,
        binding: &PartyCursorBinding,
        token: &str,
        now: DateTime<Utc>,
    ) -> Result<PartyWindow, PartyLiveError> {
        let mut bytes = decode(token)?;
        if bytes.len() < 29 {
            return Err(PartyLiveError::InvalidInput);
        }
        let nonce: [u8; 12] = bytes[..12]
            .try_into()
            .map_err(|_| PartyLiveError::InvalidInput)?;
        let aad = serde_json::to_vec(binding).map_err(|_| PartyLiveError::InvalidInput)?;
        let plaintext = self
            .key
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(aad),
                &mut bytes[12..],
            )
            .map_err(|_| PartyLiveError::InvalidInput)?;
        let payload: Payload =
            serde_json::from_slice(plaintext).map_err(|_| PartyLiveError::InvalidInput)?;
        if payload.version != 1
            || now >= payload.expires
            || payload.expires > now + Duration::minutes(15)
        {
            return Err(PartyLiveError::InvalidInput);
        }
        validate(binding, payload.window, now)?;
        Ok(payload.window)
    }
}

fn validate(
    binding: &PartyCursorBinding,
    window: PartyWindow,
    now: DateTime<Utc>,
) -> Result<(), PartyLiveError> {
    if binding.actor.is_nil()
        || binding.event.is_nil()
        || binding.operation != "listConcertParties"
        || binding.collection != "parties"
        || !(1..=100).contains(&binding.limit)
        || binding.ticket_section.as_ref().is_some_and(|v| {
            v.is_empty() || v.chars().count() > 120 || v.chars().any(char::is_control)
        })
        || window.upper > now
        || window.after.is_some_and(|position| {
            position.party_id.is_nil() || position.created_at > window.upper
        })
    {
        return Err(PartyLiveError::InvalidInput);
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

fn decode(value: &str) -> Result<Vec<u8>, PartyLiveError> {
    if value.is_empty() || value.len() > 1024 || value.len() % 4 == 1 {
        return Err(PartyLiveError::InvalidInput);
    }
    let mut out = Vec::new();
    let mut accumulator = 0u32;
    let mut bits = 0;
    for byte in value.bytes() {
        let n = ALPHABET
            .iter()
            .position(|b| *b == byte)
            .ok_or(PartyLiveError::InvalidInput)? as u32;
        accumulator = (accumulator << 6) | n;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((accumulator >> bits) as u8);
            accumulator &= (1 << bits) - 1;
        }
    }
    if accumulator != 0 || encode(&out) != value {
        return Err(PartyLiveError::InvalidInput);
    }
    Ok(out)
}
