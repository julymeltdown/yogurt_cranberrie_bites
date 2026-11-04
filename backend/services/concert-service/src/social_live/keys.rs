//! Explicit managed secret injection. No environment, random/default key or
//! Debug/Serialize implementation; Root owns configuration and key retention.
use crate::relationship::crypto::{HandleKeys, ManagedHandleKey};
use concert_application::commands::{FingerprintKeys, SigningKey};
use ring::hmac;
use tonic::Status;
pub struct KeyMaterial {
    id: String,
    secret: Vec<u8>,
}
impl KeyMaterial {
    pub fn new(id: String, secret: Vec<u8>) -> Self {
        Self { id, secret }
    }
}
pub struct ManagedSocialKeys {
    handle: KeyMaterial,
    old_handles: Vec<KeyMaterial>,
    command: KeyMaterial,
    old_commands: Vec<KeyMaterial>,
    cursor: hmac::Key,
}
impl ManagedSocialKeys {
    pub fn new(
        handle: KeyMaterial,
        old_handles: Vec<KeyMaterial>,
        command: KeyMaterial,
        old_commands: Vec<KeyMaterial>,
        cursor_secret: &[u8],
    ) -> Result<Self, Status> {
        if cursor_secret.len() < 32 || cursor_secret.len() > 4096 {
            return Err(unavailable());
        }
        let value = Self {
            handle,
            old_handles,
            command,
            old_commands,
            cursor: hmac::Key::new(hmac::HMAC_SHA256, cursor_secret),
        };
        value.handles()?;
        value.commands()?;
        Ok(value)
    }
    pub(crate) fn handles(&self) -> Result<HandleKeys, Status> {
        let key =
            |m: &KeyMaterial| ManagedHandleKey::new(&m.id, &m.secret).map_err(|_| unavailable());
        HandleKeys::new(
            key(&self.handle)?,
            self.old_handles.iter().map(key).collect::<Result<_, _>>()?,
        )
        .map_err(|_| unavailable())
    }
    pub(crate) fn commands(&self) -> Result<FingerprintKeys, Status> {
        let key = |m: &KeyMaterial| SigningKey::new(&m.id, &m.secret).map_err(|_| unavailable());
        FingerprintKeys::new(
            key(&self.command)?,
            self.old_commands
                .iter()
                .map(key)
                .collect::<Result<_, _>>()?,
        )
        .map_err(|_| unavailable())
    }
    pub(crate) fn cursor_sign(&self, payload: &[u8]) -> Vec<u8> {
        hmac::sign(&self.cursor, payload).as_ref().to_vec()
    }
    pub(crate) fn cursor_check(&self, payload: &[u8], signature: &[u8]) -> Result<(), Status> {
        hmac::verify(&self.cursor, payload, signature)
            .map_err(|_| Status::invalid_argument("Invalid social cursor"))
    }
}
fn unavailable() -> Status {
    Status::unavailable("Managed social keys unavailable")
}
