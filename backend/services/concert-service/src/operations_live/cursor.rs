//! Opaque authenticated pagination. A cursor is a position, never an access grant.
use super::unavailable;
use chrono::{DateTime, Duration, Utc};
use concert_application::commands::CommandError;
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CursorBinding {
    pub actor: Uuid,
    pub event: Uuid,
    pub operation: String,
    pub filters: BTreeMap<String, String>,
    pub limit: u16,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Position {
    after: Uuid,
    expires: DateTime<Utc>,
}

pub struct OperationsCursor(aead::LessSafeKey);
impl OperationsCursor {
    /// A dedicated configured key, not a hard-coded default or access token.
    pub fn new(key: &[u8; 32]) -> Result<Self, CommandError> {
        if *key == [0; 32] {
            return Err(CommandError::InvalidInput);
        }
        Ok(Self(aead::LessSafeKey::new(
            aead::UnboundKey::new(&aead::AES_256_GCM, key)
                .map_err(|_| CommandError::Unavailable)?,
        )))
    }
    pub fn seal(
        &self,
        binding: &CursorBinding,
        after: Uuid,
        now: DateTime<Utc>,
    ) -> Result<String, CommandError> {
        validate(binding)?;
        if after.is_nil() {
            return Err(CommandError::InvalidInput);
        }
        let mut nonce = [0; 12];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| CommandError::Unavailable)?;
        let mut payload = serde_json::to_vec(&Position {
            after,
            expires: now + Duration::minutes(15),
        })
        .map_err(unavailable)?;
        self.0
            .seal_in_place_append_tag(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(serde_json::to_vec(binding).map_err(unavailable)?),
                &mut payload,
            )
            .map_err(|_| CommandError::Unavailable)?;
        let mut bytes = nonce.to_vec();
        bytes.extend(payload);
        Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
    }
    pub fn open(
        &self,
        binding: &CursorBinding,
        token: &str,
        now: DateTime<Utc>,
    ) -> Result<Uuid, CommandError> {
        validate(binding)?;
        if token.len() < 58
            || token.len() > 1024
            || token.len() % 2 != 0
            || !token
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(CommandError::InvalidInput);
        }
        let mut bytes = (0..token.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&token[i..i + 2], 16).map_err(|_| CommandError::InvalidInput)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let nonce: [u8; 12] = bytes[..12]
            .try_into()
            .map_err(|_| CommandError::InvalidInput)?;
        let data = self
            .0
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(serde_json::to_vec(binding).map_err(unavailable)?),
                &mut bytes[12..],
            )
            .map_err(|_| CommandError::InvalidInput)?;
        let p: Position = serde_json::from_slice(data).map_err(|_| CommandError::InvalidInput)?;
        if p.after.is_nil() || p.expires <= now || p.expires > now + Duration::minutes(15) {
            return Err(CommandError::InvalidInput);
        }
        Ok(p.after)
    }
}
fn validate(binding: &CursorBinding) -> Result<(), CommandError> {
    if binding.actor.is_nil()
        || binding.event.is_nil()
        || binding.operation.is_empty()
        || !(1..=100).contains(&binding.limit)
        || binding.filters.len() > 4
    {
        Err(CommandError::InvalidInput)
    } else {
        Ok(())
    }
}
