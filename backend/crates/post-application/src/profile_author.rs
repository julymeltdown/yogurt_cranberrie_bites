//! Existence result only. No nickname, avatar, biography or visibility crosses.
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct ExistingProfileAuthorDto(Uuid);
impl ExistingProfileAuthorDto {
    pub fn new(user_id: Uuid) -> anyhow::Result<Self> {
        anyhow::ensure!(!user_id.is_nil(), "Author ID must be non-nil");
        Ok(Self(user_id))
    }
    pub fn user_id(&self) -> Uuid {
        self.0
    }
}
