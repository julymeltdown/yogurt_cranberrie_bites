//! Owner receipt evidence for Gateway operations whose outcome is UNKNOWN.
//!
//! An owner records a receipt in the same transaction as the effect, so a
//! receipt that matches the operation's id, actor and versioned scope proves
//! the write committed. The reconciler stores that conclusion, so the caller's
//! own status lookup reports COMMITTED instead of UNKNOWN. A missing,
//! mismatched or incomplete receipt proves nothing: nothing fences a late
//! commit, so those operations stay UNKNOWN and are checked again. Lookup claims
//! are short database transactions; no database connection spans an owner RPC.

use crate::application::dto::journal_repository_dto::OperationJournalLookupDto;
use crate::application::dto::journal_write_dto::OwnerReceiptClaimDto as ReceiptClaim;
use crate::application::dto::journal_write_dto::*;
use crate::application::journal_runtime_ports::JournalRuntimePorts;
use crate::config::AppState;
use crate::domain::operation::OwnerReceiptLookupStatus;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// Operations examined per reconciliation pass.
pub const RECONCILE_BATCH: i64 = 50;
/// Interval between reconciliation passes.
pub const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);
/// Bounds lookup frequency without retiring an uncertain operation.
const MAX_RETRY_DELAY: Duration = Duration::from_secs(300);
/// An individual owner lookup cannot consume the entire pass.
const OWNER_LOOKUP_TIMEOUT: Duration = Duration::from_secs(2);
/// Includes connection acquisition, queries, and all lookups in a pass.
const RECONCILE_PASS_TIMEOUT: Duration = Duration::from_secs(10);

/// What one reconciliation pass did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OwnerReceiptReconciliation {
    /// A matching receipt proved the commit; the outcome is now COMMITTED.
    pub committed: u64,
    /// The lookup answered, but the answer proves nothing; retried later.
    pub unresolved: u64,
    /// The owner could not be asked; retried after bounded backoff.
    pub unavailable: u64,
    /// Unsupported routes or invalid actors need identity/policy repair.
    /// Their outcome remains UNKNOWN; this is not proof of non-commit.
    pub suspended: u64,
}

/// Run reconciliation passes until shutdown.
pub async fn run(state: Arc<AppState>, journal: JournalRuntimePorts) -> anyhow::Result<()> {
    platform::shutdown::periodic(
        "gateway-owner-receipt-reconciler",
        RECONCILE_INTERVAL,
        || {
            let state = state.clone();
            let journal = journal.clone();
            async move {
                let pass = reconcile_from_owner_receipts(&state, &journal, RECONCILE_BATCH).await?;
                if pass.committed + pass.unresolved + pass.unavailable + pass.suspended > 0 {
                    tracing::info!(
                        committed = pass.committed,
                        unresolved = pass.unresolved,
                        unavailable = pass.unavailable,
                        suspended = pass.suspended,
                        "owner receipt reconciliation pass"
                    );
                }
                Ok(pass)
            }
        },
    )
    .await
}

/// Examine a bounded batch of UNKNOWN operations without an operator decision.
/// Claim one row at a time so an unavailable owner cannot starve newer rows or
/// mark unexamined rows as checked. Replicas share the retry lease. A process
/// crash releases a claim after one interval; it never redispatches a mutation.
pub async fn reconcile_from_owner_receipts(
    state: &AppState,
    journal: &JournalRuntimePorts,
    batch: i64,
) -> anyhow::Result<OwnerReceiptReconciliation> {
    anyhow::ensure!(
        (1..=RECONCILE_BATCH).contains(&batch),
        "invalid owner receipt batch"
    );
    tokio::time::timeout(
        RECONCILE_PASS_TIMEOUT,
        reconcile_batch(state, journal, batch),
    )
    .await
    .map_err(|_| anyhow::anyhow!("owner receipt reconciliation pass timed out"))?
}

async fn claim_next(journal: &JournalRuntimePorts) -> anyhow::Result<Option<ReceiptClaim>> {
    Ok(journal
        .owner_receipts
        .claim_next(OwnerReceiptClaimRequestDto {
            lease_seconds: RECONCILE_INTERVAL.as_secs_f64(),
        })
        .await?)
}

fn retry_delay(attempt: i32) -> Duration {
    let multiplier = 1_u64 << attempt.saturating_sub(1).clamp(0, 4);
    Duration::from_secs((RECONCILE_INTERVAL.as_secs() * multiplier).min(MAX_RETRY_DELAY.as_secs()))
}

async fn finish_lookup(
    journal: &JournalRuntimePorts,
    claim: &ReceiptClaim,
    operation: &crate::domain::operation::OperationStatus,
    suspended_reason: Option<&str>,
) -> anyhow::Result<bool> {
    Ok(journal
        .owner_receipts
        .finish_lookup(OwnerReceiptFinishLookupDto {
            claim,
            identity: OwnerReceiptIdentityDto {
                actor: operation.actor_identity(),
                action: &operation.action,
                resource_path: &operation.resource_path,
            },
            suspended_reason,
            retry_seconds: retry_delay(claim.attempt).as_secs_f64(),
        })
        .await?)
}

async fn reconcile_batch(
    state: &AppState,
    journal: &JournalRuntimePorts,
    batch: i64,
) -> anyhow::Result<OwnerReceiptReconciliation> {
    let mut pass = OwnerReceiptReconciliation::default();
    for _ in 0..batch {
        let Some(claim) = claim_next(journal).await? else {
            break;
        };
        let operation_id = claim.operation_id;
        let Some(operation) =
            super::request_journal::operation_status(journal, operation_id).await?
        else {
            continue;
        };
        let suspended_reason = if operation.owner_mutation_scope().is_none() {
            Some("NOT_APPLICABLE")
        } else if operation
            .actor_identity()
            .and_then(|actor| Uuid::parse_str(actor).ok())
            .filter(|actor| !actor.is_nil())
            .is_none()
        {
            Some("INVALID_ACTOR")
        } else {
            None
        };
        if suspended_reason.is_some() {
            if finish_lookup(journal, &claim, &operation, suspended_reason).await? {
                pass.suspended += 1;
            }
            continue;
        }
        let receipt = match tokio::time::timeout(
            OWNER_LOOKUP_TIMEOUT,
            owner_receipt_status(state, &operation, journal),
        )
        .await
        {
            Ok(receipt) => receipt,
            Err(_) => {
                pass.unavailable += 1;
                finish_lookup(journal, &claim, &operation, None).await?;
                continue;
            }
        };
        match receipt.lookup_status {
            OwnerReceiptLookupStatus::Found
                if receipt.receipt_state.as_deref() != Some("INCOMPLETE") =>
            {
                let recorded = journal
                    .owner_receipts
                    .record_commit(OwnerReceiptCommitDto {
                        claim: &claim,
                        identity: OwnerReceiptIdentityDto {
                            actor: operation.actor_identity(),
                            action: &operation.action,
                            resource_path: &operation.resource_path,
                        },
                        scope: receipt.scope.as_deref(),
                        receipt_state: receipt.receipt_state.as_deref(),
                        committed_at: receipt.committed_at,
                    })
                    .await?;
                if recorded {
                    pass.committed += 1;
                    state.metrics.inc_admin_audit_event(
                        "OPERATION_RECONCILED_FROM_OWNER_RECEIPT",
                        "COMMITTED",
                    );
                }
            }
            OwnerReceiptLookupStatus::Unavailable => {
                pass.unavailable += 1;
                finish_lookup(journal, &claim, &operation, None).await?;
            }
            status => {
                if status == OwnerReceiptLookupStatus::Mismatch {
                    tracing::warn!(
                        %operation_id,
                        code = "OWNER_RECEIPT_MISMATCH",
                        "owner receipt does not match the Gateway operation; left UNKNOWN"
                    );
                }
                pass.unresolved += 1;
                finish_lookup(journal, &claim, &operation, None).await?;
            }
        }
    }
    Ok(pass)
}

/// Owner receipt evidence for one operation, checked against its actor and
/// versioned scope.
pub(crate) async fn owner_receipt_status(
    state: &AppState,
    operation: &crate::domain::operation::OperationStatus,
    local_receipts: &JournalRuntimePorts,
) -> crate::domain::operation::OwnerReceiptStatus {
    use crate::application::dto::post_dto::MutationReceiptStateDto;
    use crate::domain::operation::{OwnerReceiptLookupStatus as LookupStatus, OwnerReceiptStatus};

    let Some(expected_scope) = operation.owner_mutation_scope() else {
        return OwnerReceiptStatus {
            lookup_status: LookupStatus::NotApplicable,
            ..OwnerReceiptStatus::default()
        };
    };
    let Some(expected_actor) = operation
        .actor_identity()
        .and_then(|actor| Uuid::parse_str(actor).ok())
    else {
        return OwnerReceiptStatus {
            lookup_status: LookupStatus::Unavailable,
            ..OwnerReceiptStatus::default()
        };
    };

    if expected_scope == crate::domain::moderation::CREATE_REPORT_OWNER_SCOPE {
        let row = match local_receipts
            .owner_receipts
            .local_receipt(OperationJournalLookupDto {
                operation_id: operation.operation_id,
            })
            .await
        {
            Ok(Some(row)) => row,
            Ok(None) => {
                return OwnerReceiptStatus {
                    lookup_status: LookupStatus::NotFound,
                    ..OwnerReceiptStatus::default()
                }
            }
            Err(error) => {
                tracing::warn!(
                    operation_id = %operation.operation_id,
                    error = ?error,
                    "Gateway moderation report receipt lookup unavailable"
                );
                return OwnerReceiptStatus {
                    lookup_status: LookupStatus::Unavailable,
                    ..OwnerReceiptStatus::default()
                };
            }
        };
        if row.actor_id != expected_actor || row.scope != expected_scope {
            return OwnerReceiptStatus {
                lookup_status: LookupStatus::Mismatch,
                ..OwnerReceiptStatus::default()
            };
        }
        let replayable = row.replayable && row.state == "REPLAYABLE";
        return OwnerReceiptStatus {
            lookup_status: LookupStatus::Found,
            receipt_state: Some(if replayable { "REPLAYABLE" } else { "EXPIRED" }.into()),
            scope: Some(expected_scope.into()),
            committed_at: Some(row.committed_at),
            replay_until: Some(row.replay_until),
        };
    }

    let receipt = match if expected_scope.starts_with("post.v1.") {
        state
            .post_service
            .get_mutation_receipt_status(operation.operation_id)
            .await
    } else if expected_scope.starts_with("friend.v1.") {
        state
            .friend_service
            .get_mutation_receipt_status(operation.operation_id)
            .await
    } else {
        state
            .profile_service
            .get_mutation_receipt_status(operation.operation_id)
            .await
    } {
        Ok(Some(receipt)) => receipt,
        Ok(None) => {
            return OwnerReceiptStatus {
                lookup_status: LookupStatus::NotFound,
                ..OwnerReceiptStatus::default()
            }
        }
        Err(_) => {
            tracing::warn!(
                operation_id = %operation.operation_id,
                "owner mutation receipt lookup unavailable"
            );
            return OwnerReceiptStatus {
                lookup_status: LookupStatus::Unavailable,
                ..OwnerReceiptStatus::default()
            };
        }
    };
    if receipt.operation_id != operation.operation_id
        || receipt.actor_id != expected_actor
        || receipt.scope != expected_scope
    {
        return OwnerReceiptStatus {
            lookup_status: LookupStatus::Mismatch,
            ..OwnerReceiptStatus::default()
        };
    }

    let receipt_state = match receipt.state {
        MutationReceiptStateDto::Replayable => "REPLAYABLE",
        MutationReceiptStateDto::Expired => "EXPIRED",
        MutationReceiptStateDto::Incomplete => "INCOMPLETE",
    };
    OwnerReceiptStatus {
        lookup_status: LookupStatus::Found,
        receipt_state: Some(receipt_state.into()),
        scope: Some(receipt.scope),
        committed_at: Some(receipt.committed_at),
        replay_until: Some(receipt.replay_until),
    }
}
