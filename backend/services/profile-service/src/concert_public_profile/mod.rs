//! Canonical public content only. No Auth, Friend, avatar, or portable allow cache.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;
pub mod persistence;
/// Identifiers only; neither admission nor a public-read grant.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ReadBinding {
    pub actor: Uuid,
    pub event: Uuid,
    pub subject: Uuid,
    pub family: Uuid,
    pub request: Uuid,
}
impl ReadBinding {
    fn child_prefix(&self) -> Result<String, ReadError> {
        let ids = [
            self.actor,
            self.event,
            self.subject,
            self.family,
            self.request,
        ];
        if ids.iter().any(Uuid::is_nil) {
            return Err(ReadError::Hidden);
        }
        // Lossless compact encoding of all five UUIDs, not a revision digest.
        let bytes: Vec<u8> = ids
            .iter()
            .flat_map(|v| v.as_bytes().iter().copied())
            .collect();
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut encoded = String::new();
        for chunk in bytes.chunks(3) {
            let n = ((chunk[0] as u32) << 16)
                | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
                | chunk.get(2).copied().unwrap_or(0) as u32;
            for shift in [18, 12, 6, 0].into_iter().take(chunk.len() + 1) {
                encoded.push(alphabet[((n >> shift) & 63) as usize] as char);
            }
        }
        Ok(format!("pp:{encoded}:"))
    }
    pub fn new_child_id(&self) -> Result<String, ReadError> {
        Ok(format!(
            "{}{}",
            self.child_prefix()?,
            Uuid::new_v4().simple()
        ))
    }
    pub fn validate_child(&self, child_id: &str) -> Result<(), ReadError> {
        let prefix = self.child_prefix()?;
        let nonce = child_id.strip_prefix(&prefix).ok_or(ReadError::Hidden)?;
        validate_secret(nonce)
    }
    pub fn validate_completion(&self, child_id: &str, secret: &str) -> Result<(), ReadError> {
        self.validate_child(child_id)?;
        validate_secret(secret)
    }
    pub fn prepared_id(&self, child_id: &str, digest: &str) -> Result<String, ReadError> {
        self.validate_child(child_id)?;
        if digest.len() != 43
            || !digest
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(ReadError::Hidden);
        }
        Ok(format!("{child_id}:{digest}"))
    }
}
fn validate_secret(value: &str) -> Result<(), ReadError> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || Uuid::parse_str(value).ok().is_none_or(|v| v.is_nil())
    {
        Err(ReadError::Hidden)
    } else {
        Ok(())
    }
}
pub struct HeldPublicRecord {
    pub content: PublicRecord,
    pub child_id: String,
    pub completion_secret: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadError {
    Unavailable,
    Hidden,
}
/// Internal repository record; deliberately lacks Serialize and a content Debug.
pub struct PublicRecord {
    pub subject: Uuid,
    pub display_name: String,
    pub bio: Option<String>,
    pub observed_at: DateTime<Utc>,
}
impl PublicRecord {
    pub fn validate(&self, subject: Uuid) -> Result<(), ReadError> {
        let now = Utc::now();
        if self.subject != subject
            || subject.is_nil()
            || self.display_name.trim().is_empty()
            || self.display_name.chars().count() > 40
            || self.display_name.chars().any(char::is_control)
            || self
                .bio
                .as_ref()
                .is_some_and(|v| v.chars().count() > 160 || v.chars().any(char::is_control))
            || self.observed_at.timestamp_micros() <= 0
            || self.observed_at > now
            || now.signed_duration_since(self.observed_at) > chrono::Duration::seconds(2)
        {
            return Err(ReadError::Unavailable);
        }
        Ok(())
    }
}
#[async_trait]
pub trait PublicProfileRepository: Send + Sync {
    /// Must query current owner privacy and deletion fences in the same statement.
    async fn read_public(
        &self,
        binding: ReadBinding,
        child_id: &str,
        secret: &str,
    ) -> Result<HeldPublicRecord, ReadError>;
    async fn check(&self, binding: ReadBinding, child_id: &str) -> Result<(), ReadError>;
    /// Terminal settlement only. No data returned and no authority extension.
    async fn complete(
        &self,
        binding: ReadBinding,
        child_id: &str,
        secret: &str,
    ) -> Result<(), ReadError>;
}
