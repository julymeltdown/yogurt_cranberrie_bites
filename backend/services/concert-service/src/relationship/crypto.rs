//! Managed-key injection only. No generated/default production keys or secret logging.
use concert_domain::relationship::{social::*, RelationshipError};
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};

pub struct ManagedHandleKey {
    id: String,
    key: aead::LessSafeKey,
}
impl ManagedHandleKey {
    pub fn new(id: &str, secret: &[u8]) -> Result<Self, RelationshipError> {
        if id.is_empty()
            || id.len() > 32
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(RelationshipError::Unavailable);
        }
        let key = aead::UnboundKey::new(&aead::AES_256_GCM, secret)
            .map_err(|_| RelationshipError::Unavailable)?;
        Ok(Self {
            id: id.to_owned(),
            key: aead::LessSafeKey::new(key),
        })
    }
}
pub struct HandleKeys {
    active: ManagedHandleKey,
    retained: Vec<ManagedHandleKey>,
}
pub(crate) struct EncryptedHandle {
    pub key_id: String,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}
impl HandleKeys {
    pub fn new(
        active: ManagedHandleKey,
        retained: Vec<ManagedHandleKey>,
    ) -> Result<Self, RelationshipError> {
        if retained.len() > 31
            || retained.iter().any(|k| k.id == active.id)
            || retained
                .iter()
                .enumerate()
                .any(|(i, k)| retained[..i].iter().any(|other| other.id == k.id))
        {
            return Err(RelationshipError::Unavailable);
        }
        Ok(Self { active, retained })
    }
    pub(crate) fn encrypt(
        &self,
        c: HandleContext,
        handle: &PrivateHandle,
    ) -> Result<EncryptedHandle, RelationshipError> {
        let mut nonce = [0; 12];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| RelationshipError::Unavailable)?;
        let mut ciphertext = handle.expose_for_reveal().as_bytes().to_vec();
        self.active
            .key
            .seal_in_place_append_tag(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(aad(c, &self.active.id)),
                &mut ciphertext,
            )
            .map_err(|_| RelationshipError::Unavailable)?;
        Ok(EncryptedHandle {
            key_id: self.active.id.clone(),
            nonce: nonce.to_vec(),
            ciphertext,
        })
    }
    pub(crate) fn decrypt(
        &self,
        c: HandleContext,
        encrypted: EncryptedHandle,
    ) -> Result<PrivateHandle, RelationshipError> {
        let key = std::iter::once(&self.active)
            .chain(self.retained.iter())
            .find(|k| k.id == encrypted.key_id)
            .ok_or(RelationshipError::Unavailable)?;
        let nonce: [u8; 12] = encrypted
            .nonce
            .try_into()
            .map_err(|_| RelationshipError::Unavailable)?;
        let mut bytes = encrypted.ciphertext;
        let plaintext = key
            .key
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(aad(c, &key.id)),
                &mut bytes,
            )
            .map_err(|_| RelationshipError::Unavailable)?;
        let raw = std::str::from_utf8(plaintext).map_err(|_| RelationshipError::Unavailable)?;
        PrivateHandle::normalize(c.platform, raw).map_err(|_| RelationshipError::Unavailable)
    }
}
fn aad(c: HandleContext, key_id: &str) -> Vec<u8> {
    let mut out = b"MCR-social-handle-AES256GCM-v1".to_vec();
    for part in [
        c.event.uuid().as_bytes().as_slice(),
        c.owner.uuid().as_bytes().as_slice(),
        c.platform.as_str().as_bytes(),
        &c.version.get().to_be_bytes(),
        key_id.as_bytes(),
    ] {
        out.extend((part.len() as u64).to_be_bytes());
        out.extend(part)
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use concert_domain::{AuthUserId, EventId, Revision};
    use uuid::Uuid;
    fn context() -> HandleContext {
        HandleContext {
            event: EventId::from_uuid(Uuid::from_u128(1)),
            owner: AuthUserId::from_uuid(Uuid::from_u128(2)),
            platform: Platform::Instagram,
            version: Revision::initial(),
        }
    }
    fn keys() -> HandleKeys {
        HandleKeys::new(ManagedHandleKey::new("test", &[7; 32]).unwrap(), vec![]).unwrap()
    }
    #[test]
    fn aad_binds_every_context_field_and_key_id() {
        let keys = keys();
        let c = context();
        let h = PrivateHandle::normalize(c.platform, "private.handle").unwrap();
        for index in 0..5 {
            let mut encrypted = keys.encrypt(c, &h).unwrap();
            let mut changed = c;
            match index {
                0 => changed.event = EventId::from_uuid(Uuid::from_u128(9)),
                1 => changed.owner = AuthUserId::from_uuid(Uuid::from_u128(9)),
                2 => changed.platform = Platform::X,
                3 => changed.version = Revision::new(2).unwrap(),
                _ => encrypted.key_id = "other".to_owned(),
            };
            assert!(keys.decrypt(changed, encrypted).is_err());
        }
    }
    #[test]
    fn nonce_randomization_and_tamper_denial() {
        let keys = keys();
        let c = context();
        let h = PrivateHandle::normalize(c.platform, "private.handle").unwrap();
        let first = keys.encrypt(c, &h).unwrap();
        let mut second = keys.encrypt(c, &h).unwrap();
        assert_ne!(first.nonce, second.nonce);
        assert_ne!(first.ciphertext, second.ciphertext);
        assert_eq!(
            keys.decrypt(c, first).unwrap().expose_for_reveal(),
            "private.handle"
        );
        second.ciphertext[0] ^= 1;
        assert!(keys.decrypt(c, second).is_err());
    }
}
