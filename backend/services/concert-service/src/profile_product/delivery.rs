//! Root-owned delivery capabilities for the concert-hosted profile transport.
//!
//! The secret is published before any protected work, the Gateway receives it in
//! the response message, and only the Gateway's `CompleteResponse` after its own
//! HTTP body EOF may settle the scope. This mirrors
//! `profile_service::profile_product::delivery` exactly, for the same reason: a
//! dropped or cancelled HTTP body must never certify completion.
use super::guard::{ProfileOwnerScope, ProfileScopeRegistry};
use super::proto;
use super::ledger::ProfileScopeLedger;
use platform::{
    actor_delegation::response_scope::{ResponseScope, ResponseScopeToken},
    grpc_auth::{VerifiedCaller, Workload},
};
use ring::rand::{SecureRandom, SystemRandom};
use std::{future::Future, pin::Pin, sync::Arc};
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct ProfileDeliveryPublisher {
    ledger: ProfileScopeLedger,
    registry: Arc<ProfileScopeRegistry>,
}

impl ProfileDeliveryPublisher {
    pub fn new(ledger: ProfileScopeLedger, registry: Arc<ProfileScopeRegistry>) -> Self {
        Self { ledger, registry }
    }
    /// Call before protected work. The registry retains the actual canonical
    /// scope even if a request future, HTTP body or completion reply is lost.
    pub async fn publish(
        &self,
        scope: Arc<ProfileOwnerScope>,
    ) -> Result<(proto::ProfileResponseDelivery, ResponseScopeToken), Status> {
        let producer = scope.producer_scope();
        let registered = self.registry.lookup_for_settlement(&producer)?;
        if !Arc::ptr_eq(&registered, &scope) {
            return Err(Status::permission_denied("Profile delivery scope mismatch"));
        }
        let mut secret = [0; 32];
        SystemRandom::new()
            .fill(&mut secret)
            .map_err(|_| Status::unavailable("Profile delivery entropy unavailable"))?;
        self.ledger.bind_delivery(&producer, &secret).await?;
        scope.bind_delivery(secret)?;
        Ok((
            proto::ProfileResponseDelivery {
                instance_id: producer.instance_id.to_string(),
                scope_id: producer.scope_id.to_string(),
                binding_digest: producer.binding_digest.to_vec(),
                completion_secret: secret.to_vec(),
            },
            ResponseScopeToken::new(Arc::new(Serialized(scope))),
        ))
    }
}

/// The response-scope holder. It deliberately holds the scope for as long as the
/// response exists, so the owner exclusion survives until the Gateway's
/// `CompleteResponse` can settle it. A holder that dropped the scope at gRPC EOF
/// would make settlement impossible and silently release the exclusion.
struct Serialized(#[allow(dead_code)] Arc<ProfileOwnerScope>);

impl ResponseScope for Serialized {
    fn finish_response(
        self: Arc<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Status>> + Send + 'static>> {
        // This is only Concert→Gateway gRPC EOF. Actual owner completion happens
        // exclusively in CompleteResponse after the Gateway's outer HTTP body
        // EOF. The holder is consumed here, so it stops retaining the scope once
        // the response body has ended; the durable ledger still records whether
        // the scope was settled.
        Box::pin(async move { self.0.mark_serialized().await })
    }
}

pub struct ProfileDeliveryService {
    publisher: ProfileDeliveryPublisher,
    capacity: Arc<tokio::sync::Semaphore>,
}

impl ProfileDeliveryService {
    pub fn new(publisher: ProfileDeliveryPublisher) -> Self {
        Self {
            publisher,
            capacity: Arc::new(tokio::sync::Semaphore::new(16)),
        }
    }
}

#[tonic::async_trait]
impl proto::profile_product_delivery_service_server::ProfileProductDeliveryService
    for ProfileDeliveryService
{
    async fn complete_response(
        &self,
        request: Request<proto::ProfileResponseDelivery>,
    ) -> Result<Response<proto::CompleteProfileResponseResult>, Status> {
        if !matches!(
            request.extensions().get::<VerifiedCaller>(),
            Some(VerifiedCaller(Workload::Gateway))
        ) {
            return Err(Status::unauthenticated(
                "Verified Gateway delivery caller required",
            ));
        }
        let capability = request.into_inner();
        let scope = super::ledger::scope_key(
            &capability.instance_id,
            &capability.scope_id,
            &capability.binding_digest,
        )?;
        if capability.completion_secret.len() != 32 {
            return Err(Status::permission_denied(
                "Invalid profile delivery capability",
            ));
        }
        let _permit = self
            .capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| Status::unavailable("Profile delivery capacity unavailable"))?;
        let held = self.publisher.registry.lookup_for_settlement(&scope)?;
        // The in-memory scope must accept the exact secret before the durable
        // ledger is advanced, so a wrong secret never seals a scope.
        let completed = held.complete(&capability.completion_secret)?;
        if !completed {
            // A second settlement of the same scope is idempotent.
            return Ok(Response::new(proto::CompleteProfileResponseResult {
                completed: true,
            }));
        }
        self.publisher
            .ledger
            .verify_delivery(&scope, &capability.completion_secret)
            .await?;
        self.publisher
            .ledger
            .record_delivery_completed(&scope)
            .await?;
        Ok(Response::new(proto::CompleteProfileResponseResult {
            completed: true,
        }))
    }
}
