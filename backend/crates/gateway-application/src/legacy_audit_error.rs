//! Controlled failure; database diagnostics remain in the outgoing adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyAuditStorageError;

impl std::fmt::Display for LegacyAuditStorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Legacy audit storage failed")
    }
}

impl std::error::Error for LegacyAuditStorageError {}
