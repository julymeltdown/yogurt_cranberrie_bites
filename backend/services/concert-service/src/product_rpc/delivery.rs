//! Root-owned delivery capabilities span Concert serialization and the actual
//! terminal Gateway HTTP body. Neither token is ever a public product DTO.
use super::{canonical_uuid, proto, scope_ledger::PgScopeLedger, scope_lifecycle};
use crate::adapters::out::relationship_authority::guarded::{self, OwnerGuardScope, ScopeRegistry};
use platform::{
    actor_delegation::response_scope::{ResponseScope, ResponseScopeToken},
    grpc_auth::{VerifiedCaller, Workload},
    grpc_method_policy::MethodPolicy,
};
use ring::rand::{SecureRandom, SystemRandom};
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
use tonic::{Request, Response, Status};

pub const POLICY: MethodPolicy = MethodPolicy {
    service: "concert.product.v1.ProductDeliveryService",
    workload: Some(Workload::Concert),
    methods: &[("CompleteResponse", &[Workload::Gateway])],
    actor_fields: &[],
};
#[derive(Clone)]
pub struct DeliveryPublisher {
    ledger: PgScopeLedger,
    registry: Arc<ScopeRegistry>,
}
impl DeliveryPublisher {
    pub fn new(ledger: PgScopeLedger, registry: Arc<ScopeRegistry>) -> Self {
        Self { ledger, registry }
    }
    /// Call before protected work. The registry retains the actual canonical
    /// scope even if a request future, HTTP body or completion reply is lost.
    pub async fn publish(
        &self,
        scope: Arc<OwnerGuardScope>,
    ) -> Result<(proto::ResponseDelivery, ResponseScopeToken), Status> {
        let producer = scope.producer_scope();
        let registered = self.registry.lookup_for_settlement(&producer)?;
        if !Arc::ptr_eq(&registered, &scope) {
            return Err(Status::permission_denied("Product delivery scope mismatch"));
        }
        let key = scope_lifecycle::key(&producer)?;
        let mut secret = [0; 32];
        SystemRandom::new()
            .fill(&mut secret)
            .map_err(|_| Status::unavailable("Product delivery entropy unavailable"))?;
        self.ledger.bind_delivery(&key, &secret).await?;
        Ok((
            proto::ResponseDelivery {
                instance_id: producer.instance_id,
                scope_id: producer.scope_id,
                binding_digest: producer.binding_digest,
                completion_secret: secret.to_vec(),
            },
            ResponseScopeToken::new(Arc::new(Serialized(scope))),
        ))
    }
}
struct Serialized(Arc<OwnerGuardScope>);
impl ResponseScope for Serialized {
    fn finish_response(
        self: Arc<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Status>> + Send + 'static>> {
        // This is only Concert→Gateway gRPC EOF. Actual owner completion happens
        // exclusively in CompleteResponse after Gateway's outer HTTP body EOF.
        Box::pin(async move { self.0.mark_serialized().await })
    }
}
pub struct DeliveryService {
    publisher: DeliveryPublisher,
    capacity: Arc<tokio::sync::Semaphore>,
}
impl DeliveryService {
    pub fn new(publisher: DeliveryPublisher) -> Self {
        Self {
            publisher,
            capacity: Arc::new(tokio::sync::Semaphore::new(16)),
        }
    }
}
#[tonic::async_trait]
impl proto::product_delivery_service_server::ProductDeliveryService for DeliveryService {
    async fn complete_response(
        &self,
        request: Request<proto::ResponseDelivery>,
    ) -> Result<Response<proto::CompleteResponseResult>, Status> {
        if !matches!(
            request.extensions().get::<VerifiedCaller>(),
            Some(VerifiedCaller(Workload::Gateway))
        ) {
            return Err(Status::unauthenticated(
                "Verified Gateway delivery caller required",
            ));
        }
        let capability = request.into_inner();
        let scope = guarded::proto::ProducerScope {
            instance_id: canonical_uuid(&capability.instance_id)?.to_string(),
            scope_id: canonical_uuid(&capability.scope_id)?.to_string(),
            binding_digest: capability.binding_digest,
        };
        let key = scope_lifecycle::key(&scope)?;
        if capability.completion_secret.len() != 32 {
            return Err(Status::permission_denied(
                "Invalid product delivery capability",
            ));
        }
        let _permit = self
            .capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| Status::unavailable("Product delivery settlement capacity"))?;
        let work = async {
            self.publisher
                .ledger
                .verify_delivery(&key, &capability.completion_secret)
                .await?;
            // Durable ROOT_FINISHED alone does not certify owner/child completion.
            // After restart an absent actual scope stays unresolved, even when
            // the durable root finished marker allows owner-local reconciliation.
            if !self.publisher.ledger.delivery_completed(&key).await? {
                let actual = self.publisher.registry.lookup_for_settlement(&scope)?;
                actual.finish_response().await?;
                self.publisher
                    .ledger
                    .record_delivery_completed(&key)
                    .await?;
            }
            let mut response = Response::new(proto::CompleteResponseResult { completed: true });
            response.metadata_mut().insert(
                "cache-control",
                "no-store".parse().expect("static metadata"),
            );
            Ok(response)
        };
        tokio::time::timeout(Duration::from_secs(2), work)
            .await
            .map_err(|_| Status::unavailable("Product delivery settlement unresolved"))?
    }
}
