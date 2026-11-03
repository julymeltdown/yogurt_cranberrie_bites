use crate::dto::operation_view_dto::{
    ManualResolutionDispositionDto, ManualResolutionEvidenceKindDto,
};
use serde::Deserialize;
use uuid::Uuid;

/// An administrator's evidence-backed disposition for a Gateway operation whose
/// result is still unknown. This records a decision; it does not replay the write.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManualOperationResolutionRequest {
    pub disposition: ManualResolutionDispositionDto,
    pub evidence_kind: ManualResolutionEvidenceKindDto,
    pub evidence_reference: String,
    pub reason: String,
    /// Required when amending an existing decision. The value must identify the
    /// current decision, preventing concurrent or stale resolutions from winning.
    #[serde(default)]
    pub supersedes_resolution_id: Option<Uuid>,
}

impl ManualOperationResolutionRequest {
    pub fn validate(&self) -> Result<(), &'static str> {
        let reason = self.reason.trim();
        if !(20..=1024).contains(&reason.chars().count())
            || reason.len() > 1024
            || reason.chars().any(char::is_control)
        {
            return Err("reason must contain 20 to 1024 non-control characters and at most 1024 UTF-8 bytes");
        }

        let reference = self.evidence_reference.as_str();
        if !(3..=256).contains(&reference.len())
            || !reference.is_ascii()
            || !reference
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._:/#-".contains(&byte))
            || !reference
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
        {
            return Err(
                "evidenceReference must be a 3 to 256 character protected-record identifier",
            );
        }

        Ok(())
    }
}

/// Committed work from one journal-maintenance pass. Retirement may be deferred
/// without undoing recovery or sensitive-body pruning already committed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReconciliationOutcome {
    pub abandoned_operations: u64,
    pub journal_bodies_pruned: u64,
    pub report_receipt_bodies_pruned: u64,
    pub journal_rows_retired: u64,
    pub completed_queue_rows_retired: u64,
    pub retirement_deferred: bool,
}

impl ReconciliationOutcome {
    pub fn total(&self) -> u64 {
        self.abandoned_operations
            + self.journal_bodies_pruned
            + self.report_receipt_bodies_pruned
            + self.journal_rows_retired
            + self.completed_queue_rows_retired
            + u64::from(self.retirement_deferred)
    }
}
