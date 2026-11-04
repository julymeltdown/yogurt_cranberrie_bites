//! Encrypted canonical base64url cursor; retained authenticated framing stays internal.
use chrono::{DateTime, Utc};
use concert_application::intro::{self, cursor};
use std::sync::Arc;

#[derive(Clone)]
pub struct IntroCursor {
    framing_key: [u8; 32],
    cipher: Arc<ring::aead::LessSafeKey>,
}
impl IntroCursor {
    pub fn new(key: [u8; 32]) -> Result<Self, intro::IntroError> {
        if key.iter().all(|v| *v == 0) {
            return Err(intro::IntroError::Unavailable);
        }
        let salt = ring::hkdf::Salt::new(ring::hkdf::HKDF_SHA256, b"mcr-intro-cursor-v1");
        let prk = salt.extract(&key);
        let mut derived = [0_u8; 32];
        prk.expand(&[b"encryption"], ring::hkdf::HKDF_SHA256)
            .map_err(|_| intro::IntroError::Unavailable)?
            .fill(&mut derived)
            .map_err(|_| intro::IntroError::Unavailable)?;
        let cipher = ring::aead::UnboundKey::new(&ring::aead::AES_256_GCM, &derived)
            .map_err(|_| intro::IntroError::Unavailable)?;
        Ok(Self {
            framing_key: key,
            cipher: Arc::new(ring::aead::LessSafeKey::new(cipher)),
        })
    }
}
impl cursor::CursorCodec for IntroCursor {
    fn seal(
        &self,
        binding: &cursor::CursorBinding,
        window: cursor::CursorWindow,
        now: DateTime<Utc>,
    ) -> Result<String, intro::IntroError> {
        use ring::rand::SecureRandom;
        cursor::validate_binding(binding, window, now)?;
        if now >= window.upper + chrono::Duration::minutes(15) {
            return Err(intro::IntroError::Invalid);
        }
        let mut bytes =
            cursor::encode_authenticated(&self.framing_key, binding, window, window.upper)?
                .into_bytes();
        let mut nonce = [0_u8; 12];
        ring::rand::SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| intro::IntroError::Unavailable)?;
        self.cipher
            .seal_in_place_append_tag(
                ring::aead::Nonce::assume_unique_for_key(nonce),
                ring::aead::Aad::from(cursor_envelope::PURPOSE),
                &mut bytes,
            )
            .map_err(|_| intro::IntroError::Unavailable)?;
        let mut envelope = nonce.to_vec();
        envelope.extend(bytes);
        let token = cursor_envelope::encode(&envelope);
        if token.len() > intro::INTRO_CURSOR_MAX {
            return Err(intro::IntroError::Invalid);
        }
        Ok(token)
    }
    fn open(
        &self,
        binding: &cursor::CursorBinding,
        token: &str,
        now: DateTime<Utc>,
    ) -> Result<cursor::CursorWindow, intro::IntroError> {
        let mut bytes = cursor_envelope::decode(token)?;
        if bytes.len() < 29 {
            return Err(intro::IntroError::Invalid);
        }
        let nonce = bytes[..12]
            .try_into()
            .map_err(|_| intro::IntroError::Invalid)?;
        let plaintext = self
            .cipher
            .open_in_place(
                ring::aead::Nonce::assume_unique_for_key(nonce),
                ring::aead::Aad::from(cursor_envelope::PURPOSE),
                &mut bytes[12..],
            )
            .map_err(|_| intro::IntroError::Invalid)?;
        let frame = std::str::from_utf8(plaintext).map_err(|_| intro::IntroError::Invalid)?;
        let window = cursor::decode_authenticated(&self.framing_key, binding, frame, now)?;
        if now >= window.upper + chrono::Duration::minutes(15) {
            return Err(intro::IntroError::Invalid);
        }
        Ok(window)
    }
}

mod cursor_envelope {
    use super::intro::IntroError;
    pub(super) const PURPOSE: &[u8] = b"mcr-intro-cursor-aead-v1";
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    // Canonical unpadded base64url, matching the existing audited Feed codec.
    pub(super) fn encode(bytes: &[u8]) -> String {
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
    pub(super) fn decode(value: &str) -> Result<Vec<u8>, IntroError> {
        if value.is_empty() || value.len() > super::intro::INTRO_CURSOR_MAX || value.len() % 4 == 1
        {
            return Err(IntroError::Invalid);
        }
        let mut out = Vec::new();
        let mut accumulator = 0_u32;
        let mut bits = 0;
        for byte in value.bytes() {
            let n = ALPHABET
                .iter()
                .position(|b| *b == byte)
                .ok_or(IntroError::Invalid)? as u32;
            accumulator = (accumulator << 6) | n;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((accumulator >> bits) as u8);
                accumulator &= (1 << bits) - 1;
            }
        }
        if accumulator != 0 || encode(&out) != value {
            return Err(IntroError::Invalid);
        }
        Ok(out)
    }
}
