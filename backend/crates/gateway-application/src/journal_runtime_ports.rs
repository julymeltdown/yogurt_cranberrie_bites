//! Journal, receipt and queue capabilities bound to one connection pool.
//!
//! The composition root builds one bundle per pool role, so the request path,
//! the receipt reconciler, maintenance and the queue worker receive only
//! neutral ports. No persistence connection type crosses this boundary; which
//! pool backs a bundle is decided where it is composed.
use crate::journal_write_ports::{
    JournalDispatchRepositoryPort, JournalRetentionRepositoryPort,
    OwnerReceiptJournalRepositoryPort,
};
use crate::queue_operation_ports::{
    AsyncOperationQueueRepositoryPort, JournalMaintenanceRepositoryPort,
};
use crate::service::operation_journal_service::OperationJournalService;
use std::sync::Arc;

#[derive(Clone)]
pub struct JournalRuntimePorts {
    /// Replay identity, response replay and owner replay; acquisitions are not
    /// recorded in pool metrics.
    pub dispatch: Arc<dyn JournalDispatchRepositoryPort>,
    /// Request completions, whose connection acquisitions are recorded.
    pub observed_dispatch: Arc<dyn JournalDispatchRepositoryPort>,
    pub retention: Arc<dyn JournalRetentionRepositoryPort>,
    pub owner_receipts: Arc<dyn OwnerReceiptJournalRepositoryPort>,
    pub maintenance: Arc<dyn JournalMaintenanceRepositoryPort>,
    /// Queue lease updates and single claims; acquisitions are not recorded.
    pub queue: Arc<dyn AsyncOperationQueueRepositoryPort>,
    /// Long-lived worker claim sessions, whose acquisitions are recorded.
    pub observed_queue: Arc<dyn AsyncOperationQueueRepositoryPort>,
    pub operations: OperationJournalService,
}
