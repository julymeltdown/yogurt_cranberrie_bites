//! Authenticated encrypted opaque cursor. No raw UUID/secret cursor payloads.
use super::wire::Query;
use chrono::{DateTime, Duration, Utc};
use concert_application::{admin::Actor, commands::CommandError};
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub after: String,
    pub snapshot: DateTime<Utc>,
    expires: DateTime<Utc>,
}
pub struct ModerationCursor(aead::LessSafeKey);
impl ModerationCursor {
    pub fn new(key: &[u8; 32]) -> Result<Self, CommandError> {
        if *key == [0; 32] {
            return Err(CommandError::InvalidInput);
        };
        Ok(Self(aead::LessSafeKey::new(
            aead::UnboundKey::new(&aead::AES_256_GCM, key)
                .map_err(|_| CommandError::Unavailable)?,
        )))
    }
    pub fn seal(
        &self,
        actor: Actor,
        op: &str,
        q: &Query,
        after: String,
        snapshot: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<String, CommandError> {
        if after.len() > 160 || snapshot > now {
            return Err(CommandError::InvalidInput);
        }
        let mut nonce = [0; 12];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| CommandError::Unavailable)?;
        let mut value = serde_json::to_vec(&Position {
            after,
            snapshot,
            expires: now + Duration::minutes(15),
        })
        .map_err(|_| CommandError::Unavailable)?;
        self.0
            .seal_in_place_append_tag(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(binding(actor, op, q)?),
                &mut value,
            )
            .map_err(|_| CommandError::Unavailable)?;
        let mut bytes = nonce.to_vec();
        bytes.extend(value);
        let token = encode(&bytes);
        if token.len() > 1024 {
            return Err(CommandError::InvalidInput);
        }
        Ok(token)
    }
    pub fn open(
        &self,
        actor: Actor,
        op: &str,
        q: &Query,
        token: &str,
        now: DateTime<Utc>,
    ) -> Result<Position, CommandError> {
        let mut bytes = decode(token)?;
        if bytes.len() < 29 {
            return Err(CommandError::InvalidInput);
        }
        let nonce: [u8; 12] = bytes[..12]
            .try_into()
            .map_err(|_| CommandError::InvalidInput)?;
        let opened = self
            .0
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(binding(actor, op, q)?),
                &mut bytes[12..],
            )
            .map_err(|_| CommandError::InvalidInput)?;
        let p: Position = serde_json::from_slice(opened).map_err(|_| CommandError::InvalidInput)?;
        if p.expires <= now || p.snapshot > now || p.after.len() > 160 {
            return Err(CommandError::InvalidInput);
        }
        Ok(p)
    }
}
fn binding(a: Actor, op: &str, q: &Query) -> Result<Vec<u8>, CommandError> {
    serde_json::to_vec(&serde_json::json!({"actor":a.actor,"event":a.event,"operation":op,"filters":q.filters,"limit":q.limit,"collection":"moderation-v1"})).map_err(|_|CommandError::InvalidInput)
}
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";
fn encode(v: &[u8]) -> String {
    let mut out = String::new();
    let mut acc = 0u32;
    let mut bits = 0;
    for &b in v {
        acc = (acc << 8) | u32::from(b);
        bits += 8;
        while bits >= 6 {
            bits -= 6;
            out.push(ALPHABET[((acc >> bits) & 63) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((acc << (6 - bits)) & 63) as usize] as char)
    }
    out
}
fn decode(v: &str) -> Result<Vec<u8>, CommandError> {
    if v.is_empty() || v.len() > 1024 {
        return Err(CommandError::InvalidInput);
    }
    let mut out = Vec::new();
    let mut acc = 0u32;
    let mut bits = 0;
    for b in v.bytes() {
        let n = ALPHABET
            .iter()
            .position(|a| *a == b)
            .ok_or(CommandError::InvalidInput)?;
        acc = (acc << 6) | n as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8)
        }
    }
    if encode(&out) != v {
        return Err(CommandError::InvalidInput);
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    #[test]
    fn encrypted_cursor_binds_actor_event_filters_and_snapshot() {
        let c = ModerationCursor::new(&[17; 32]).unwrap();
        let now = Utc::now();
        let a = Actor {
            actor: Uuid::new_v4(),
            event: Uuid::new_v4(),
        };
        let q = Query {
            limit: 20,
            ..Default::default()
        };
        let t = c
            .seal(a, "listConcertAudit", &q, "position".into(), now, now)
            .unwrap();
        assert_eq!(
            c.open(a, "listConcertAudit", &q, &t, now).unwrap().snapshot,
            now
        );
        assert!(c
            .open(
                Actor {
                    event: Uuid::new_v4(),
                    ..a
                },
                "listConcertAudit",
                &q,
                &t,
                now
            )
            .is_err());
        let mut changed = q.clone();
        changed.filters.insert("outcome".into(), "FAILED".into());
        assert!(c.open(a, "listConcertAudit", &changed, &t, now).is_err());
        assert!(c
            .open(a, "listConcertAudit", &q, &t, now + Duration::minutes(16))
            .is_err());
    }
    #[test]
    fn cursor_rejects_noncanonical_encoding() {
        for v in ["A", "AB", "%%%", "AA="] {
            assert!(decode(v).is_err())
        }
        assert_eq!(decode(&encode(b"\0abc\xff")).unwrap(), b"\0abc\xff");
    }
}
