//! Root-owned delivery capabilities span the Profile serialization and the
//! actual terminal Gateway HTTP body. Neither token is ever a public DTO.
//!
//! This mirrors `concert_service::product_rpc::delivery` exactly: the secret is
//! published before protected work, the Gateway receives it in the response
//! message, and only the Gateway's `CompleteResponse` after its own HTTP body
//! EOF may settle the scope.
use super::{ledger::ProfileScopeLedger, proto};
use crate::adapters::out::profile_owner_guard::{ProfileOwnerScope, ProfileScopeRegistry};
use platform::{
    actor_delegation::response_scope::{ResponseScope, ResponseScopeToken},
    grpc_auth::{VerifiedCaller, Workload},
};
use ring::rand::{SecureRandom, SystemRandom};
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
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
        // This is only Profile→Gateway gRPC EOF. Actual owner completion happens
        // exclusively in CompleteResponse after the Gateway's outer HTTP body EOF.
        // The holder is consumed here, so it stops retaining the scope once the
        // response body has ended; the durable ledger still records whether the
        // scope was settled.
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
            .map_err(|_| Status::unavailable("Profile delivery settlement capacity"))?;
        let work = async {
            self.publisher
                .ledger
                .verify_delivery(&scope, &capability.completion_secret)
                .await?;
            // A durable ROOT_FINISHED alone does not certify owner completion.
            // After restart an absent actual scope stays unresolved.
            if !self.publisher.ledger.delivery_completed(&scope).await? {
                let actual = self.publisher.registry.lookup_for_settlement(&scope)?;
                // The held scope is the only object that can complete this
                // exclusion. A mismatched secret cannot reach here.
                actual.complete(&capability.completion_secret)?;
                actual.mark_serialized().await.ok();
                self.publisher.ledger.record_delivery_completed(&scope).await?;
            }
            let mut response = Response::new(proto::CompleteProfileResponseResult {
                completed: true,
            });
            response.metadata_mut().insert(
                "cache-control",
                "no-store".parse().expect("static metadata"),
            );
            Ok(response)
        };
        tokio::time::timeout(Duration::from_secs(2), work)
            .await
            .map_err(|_| Status::unavailable("Profile delivery settlement unresolved"))?
    }
}
