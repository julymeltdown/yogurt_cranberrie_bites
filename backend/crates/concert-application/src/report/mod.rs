//! Bounded read selectors; opaque transport cursor encoding belongs to the HTTP owner.
use crate::commands::CommandError;
use concert_domain::report::{ReportState, TargetType};
use uuid::Uuid;
#[derive(Clone, Copy, Debug)]
pub struct ReportFilter {
    pub state: Option<ReportState>,
    pub target_type: Option<TargetType>,
    pub after: Option<Uuid>,
    pub limit: u16,
}
impl ReportFilter {
    pub fn validate(self) -> Result<Self, CommandError> {
        if !(1..=100).contains(&self.limit) {
            Err(CommandError::InvalidInput)
        } else {
            Ok(self)
        }
    }
}

impl Default for ReportFilter {
    fn default() -> Self {
        Self {
            state: None,
            target_type: None,
            after: None,
            limit: 20,
        }
    }
}
