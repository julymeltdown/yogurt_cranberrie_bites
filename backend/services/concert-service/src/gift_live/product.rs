//! Gift producer under the one canonical Product owner and delivery capability.
use super::{
    protocol::InputError, GiftIdentity, GiftOperation, GiftReply, GiftRequest, GiftRuntime,
};
use crate::{
    adapters::out::relationship_authority::guarded::OwnerGuardScope,
    product_rpc::{
        proto,
        scoped::{ProductRepresentation, ScopedProductProducer},
        VerifiedProduct,
    },
};
use std::sync::Arc;
use tonic::Status;

pub fn handles(operation: &str) -> bool {
    GiftOperation::parse(operation).is_ok()
}

fn input_error(error: InputError) -> Status {
    match error {
        InputError::Invalid => Status::invalid_argument("Invalid Gift request"),
        InputError::Expired => Status::deadline_exceeded("Gift request expired"),
        InputError::Unavailable => Status::unavailable("Gift request unavailable"),
    }
}

#[async_trait::async_trait]
impl ScopedProductProducer for GiftRuntime {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        request: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        context.check()?;
        let reply = if let Some(query) = context.command_query() {
            if !handles(query.operation.id()) {
                return Err(Status::unimplemented("Not a Gift command"));
            }
            self.command_status(scope, query.operation, query.key)
                .await?
        } else {
            let parsed = GiftRequest::from_verified(
                GiftIdentity {
                    actor: context.actor(),
                    event: context.event(),
                    session_family: context.session(),
                    request_id: context.request(),
                    deadline: context.deadline(),
                },
                context.operation().contract_id(),
                &request.http_method,
                &request.http_target,
                &request.content_type,
                &request.idempotency_key,
                request.http_body,
            )
            .map_err(input_error)?;
            // This context is sealed by ProductRpcService's authenticated edge;
            // raw IPs and caller-provided permission booleans never enter Gift.
            self.dispatch(scope, parsed, context.trusted_pickup_context())
                .await?
        };
        let GiftReply::Json { status, body } = reply;
        Ok(ProductRepresentation {
            status: u32::from(status),
            content_type: "application/json".into(),
            body: serde_json::to_vec(&body)
                .map_err(|_| Status::internal("Gift projection unavailable"))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_thirteen_frozen_gift_operations_are_selected() {
        for id in [
            "listConcertGiveaways",
            "getConcertGiveaway",
            "createConcertGiveaway",
            "updateConcertGiveaway",
            "getConcertGiveawayManagement",
            "setConcertPickupMeeting",
            "reserveConcertGiveaway",
            "getConcertGiftActivity",
            "getConcertReservation",
            "cancelConcertReservation",
            "claimConcertReservation",
            "getConcertPickup",
            "issueConcertPickupCode",
        ] {
            assert!(handles(id), "{id}");
        }
        for id in [
            "getConcertCommandStatus",
            "listConcertParties",
            "claimConcertReservation ",
            "",
        ] {
            assert!(!handles(id));
        }
    }
}
