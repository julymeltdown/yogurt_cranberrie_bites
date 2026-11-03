//! Validated social commands. All identity/intro/claim authority remains server-owned.
use concert_domain::relationship::{social::*, RelationshipError};
use concert_domain::Revision;

#[derive(Clone, Debug)]
pub struct CreateSharing {
    pub context: SharingContext,
    pub consents: Vec<ConsentBinding>,
}
impl CreateSharing {
    pub fn validate(&self) -> Result<(), RelationshipError> {
        if self.context.requester == self.context.recipient
            || self.consents.is_empty()
            || self.consents.len() > 2
            || self.consents.iter().any(|c| !c.requester_consents)
            || self
                .consents
                .iter()
                .enumerate()
                .any(|(i, c)| self.consents[..i].iter().any(|p| p.platform == c.platform))
        {
            return Err(RelationshipError::Denied);
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct SharingDecision {
    pub expected: Revision,
    pub approve: bool,
    pub approved_platforms: Vec<Platform>,
    pub recipient_consents: bool,
}
impl SharingDecision {
    pub fn validate(&self) -> Result<(), RelationshipError> {
        if self.approved_platforms.len() > 2
            || self
                .approved_platforms
                .iter()
                .enumerate()
                .any(|(i, p)| self.approved_platforms[..i].contains(p))
            || (self.approve && (!self.recipient_consents || self.approved_platforms.is_empty()))
            || (!self.approve && (self.recipient_consents || !self.approved_platforms.is_empty()))
        {
            return Err(RelationshipError::Denied);
        }
        Ok(())
    }
}
