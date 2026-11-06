//! Single report capability over the retained runtime and Root-owned scope.
use super::OperationsRuntime;
use crate::{
    adapters::out::relationship_authority::guarded::{GuardBinding, OwnerGuardScope},
    product_rpc::{
        proto::ProductRequest,
        scoped::{ProductRepresentation, ScopedProductProducer},
        Operation, VerifiedProduct,
    },
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tonic::Status;

pub struct ReportProduct {
    runtime: Arc<OperationsRuntime>,
}
impl ReportProduct {
    /// Root supplies its existing Operations runtime and canonical composition.
    /// This leaf constructs no owner, guard, registry, ledger or delivery token.
    pub fn new(runtime: Arc<OperationsRuntime>) -> Self {
        Self { runtime }
    }
}
#[async_trait::async_trait]
impl ScopedProductProducer for ReportProduct {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        frame: ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        context.check()?;
        let operation = match context.operation() {
            Operation::Report(operation) => {
                operation.validate(&frame)?;
                operation.id()
            }
            Operation::Operations(operation) => {
                operation.validate(&frame)?;
                operation.id()
            }
            _ => {
                return Err(Status::unavailable(
                    "Report producer unavailable for operation",
                ));
            }
        };
        let expected = GuardBinding::from_parent(
            context.parent_authority(),
            context.actor(),
            context.event(),
            context.session(),
            context.request(),
            operation,
            &frame.http_method,
            &frame.http_target,
            Sha256::digest(&frame.http_body).into(),
            context.deadline(),
        )
        .map_err(|error| {
            if error.code() == tonic::Code::DeadlineExceeded {
                error
            } else {
                Status::permission_denied("Report parent binding mismatch")
            }
        })?;
        if expected.digest() != scope.binding().digest() {
            return Err(Status::permission_denied("Report canonical scope mismatch"));
        }
        // The retained runtime checks the complete original unary parent frame,
        // actor/session/event/request/body/operation and original deadline again.
        // It alone owns transaction/receipt/outbox semantics and safe responses.
        ScopedProductProducer::dispatch(&*self.runtime, context, scope, frame).await
    }
}
