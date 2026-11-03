//! Metadata-only system invalidation, never an actor command or authorization grant.
use super::{CommandError, SafeReference};
use uuid::Uuid;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaintenanceInvalidation {
    pub id: Uuid,
    pub event: Uuid,
    pub reference: SafeReference,
}
impl MaintenanceInvalidation {
    pub fn validate(self) -> Result<Self, CommandError> {
        self.reference.validate()?;
        Ok(self)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidationKind {
    ResourceInvalidated,
    MaintenanceInvalidated,
}
impl InvalidationKind {
    pub fn parse(kind: &str, receipt: Option<Uuid>) -> Result<Self, CommandError> {
        match (kind, receipt) {
            ("RESOURCE_INVALIDATED", Some(_)) => Ok(Self::ResourceInvalidated),
            ("MAINTENANCE_INVALIDATED", None) => Ok(Self::MaintenanceInvalidated),
            _ => Err(CommandError::InvalidState),
        }
    }
}
