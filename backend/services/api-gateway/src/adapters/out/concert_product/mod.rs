//! Signed C04 dispatch using the retained workload TLS/actor boundary.
use super::{concert_identity::ConcertIdentityReader, jwt::concert_access::ConcertAccessContext};
use platform::{
    actor_delegation::{request_for, ActorDelegationClient, ActorDelegationSigner},
    edge::{EdgeBudget, FailurePolicy, RetryClass},
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use std::time::Duration;
use tonic::{service::interceptor::InterceptedService, transport::Channel, Status};
use uuid::Uuid;
mod network;
pub mod proto {
    tonic::include_proto!("concert.product.v1");
}
pub const MAX_BODY: usize = 10 * 1024 * 1024;
const BUDGET: Duration = Duration::from_secs(2);
const EDGE: EdgeBudget = EdgeBudget {
    edge: "gateway.concert.product",
    expected_latency: Duration::from_millis(100),
    deadline: BUDGET,
    connect_deadline: Duration::from_millis(500),
    retry: RetryClass::Never,
    failure_policy: FailurePolicy::FailRequest,
};
/// Captured by the HTTP route. Raw uploads and idempotency keys are not logged.
pub struct ProductHttpRequest {
    pub client_address: Option<crate::adapters::r#in::http::client_identity::VerifiedClientAddress>,
    pub event: Uuid,
    pub request_id: Uuid,
    pub method: String,
    pub target: String,
    pub body: Vec<u8>,
    pub content_type: String,
    pub idempotency_key: String,
}
#[derive(Clone)]
pub struct GrpcConcertProductClient {
    network_key: Option<std::sync::Arc<network::NetworkBucketKey>>,
    client: proto::product_service_client::ProductServiceClient<
        InterceptedService<ActorDelegationClient<Channel>, ServiceIdentityClientInterceptor>,
    >,
    delivery: proto::product_delivery_service_client::ProductDeliveryServiceClient<
        InterceptedService<Channel, ServiceIdentityClientInterceptor>,
    >,
}
impl GrpcConcertProductClient {
    pub fn connect(
        endpoint: &str,
        token: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        signer: ActorDelegationSigner,
    ) -> anyhow::Result<Self> {
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            endpoint,
            Workload::Concert,
            EDGE.validate()?,
        )?;
        Ok(Self {
            network_key: network::NetworkBucketKey::load()?.map(std::sync::Arc::new),
            client: proto::product_service_client::ProductServiceClient::with_interceptor(
                ActorDelegationClient::new(channel.clone(), signer, Workload::Concert),
                ServiceIdentityClientInterceptor::new(Workload::Gateway, token)?,
            )
            .max_decoding_message_size(MAX_BODY + 16 * 1024)
            .max_encoding_message_size(MAX_BODY + 16 * 1024),
            delivery: proto::product_delivery_service_client::ProductDeliveryServiceClient::with_interceptor(
                channel,
                ServiceIdentityClientInterceptor::new(Workload::Gateway, token)?,
            )
            .max_decoding_message_size(4096)
            .max_encoding_message_size(4096),
        })
    }
    pub async fn dispatch(
        &self,
        reader: &ConcertIdentityReader,
        context: &ConcertAccessContext,
        http: ProductHttpRequest,
    ) -> Result<proto::ProductResponse, Status> {
        if http.event.is_nil()
            || http.request_id.is_nil()
            || http.body.len() > MAX_BODY
            || http.target.len() > 4096
        {
            return Err(Status::invalid_argument("Invalid product request"));
        }
        context
            .check()
            .map_err(|_| Status::unauthenticated("Invalid product access"))?;
        let original = context
            .rpc_deadline()
            .map_err(|_| Status::deadline_exceeded("Product deadline expired"))?;
        let mut stop = original.stop();
        if let Some(outer) = platform::call_deadline::current() {
            stop = stop.min(outer.into());
        }
        let work = async {
            let identity = Box::pin(reader.read(context, None))
                .await
                .map_err(|_| Status::unavailable("Product identity unavailable"))?;
            identity
                .check()
                .map_err(|_| Status::unauthenticated("Product identity expired"))?;
            let actor = identity
                .user_id()
                .map_err(|_| Status::unauthenticated("Invalid product identity"))?;
            let auth_at = identity
                .admission_observed_at()
                .map_err(|_| Status::unavailable("Product identity unavailable"))?
                .timestamp_micros();
            let profile_at = identity
                .profile_observed_at()
                .map_err(|_| Status::unavailable("Product profile unavailable"))?
                .timestamp_micros();
            let now = chrono::Utc::now().timestamp_micros();
            let remaining = context
                .remaining(BUDGET)
                .map_err(|_| Status::deadline_exceeded("Product deadline expired"))?
                .min(stop.saturating_duration_since(tokio::time::Instant::now()));
            let expires = original
                .expires_micros()
                .min(context.access_expires_micros())
                .min(now.saturating_add(remaining.as_micros() as i64))
                .min(auth_at.saturating_add(BUDGET.as_micros() as i64))
                .min(profile_at.saturating_add(BUDGET.as_micros() as i64));
            if remaining.is_zero() || expires <= now {
                return Err(Status::deadline_exceeded("Product deadline expired"));
            }
            let message = proto::ProductRequest {
                trusted_network_bucket: self.network_key.as_ref()
                    .zip(http.client_address).map(|(key, address)| key.bucket(address)).unwrap_or_default(),
                user_id: actor.to_string(),
                event_id: http.event.to_string(),
                session_family_id: context
                    .session_family()
                    .map_err(|_| Status::unauthenticated("Invalid product session"))?
                    .to_string(),
                request_id: http.request_id.to_string(),
                http_method: http.method,
                http_target: http.target,
                http_body: http.body,
                content_type: http.content_type,
                idempotency_key: http.idempotency_key,
                authority_expires_unix_micros: expires,
                auth_observed_at_unix_micros: auth_at,
                profile_observed_at_unix_micros: profile_at,
                access_expires_unix_micros: context.access_expires_micros(),
            };
            let mut request = request_for(message, &actor.to_string())?;
            request.set_timeout(remaining.min(Duration::from_micros((expires - now) as u64)));
            identity
                .check()
                .map_err(|_| Status::unauthenticated("Product identity expired"))?;
            let mut client = self.client.clone();
            let response = Box::pin(client.dispatch(request)).await?;
            identity
                .check()
                .map_err(|_| Status::unauthenticated("Product identity expired"))?;
            if chrono::Utc::now().timestamp_micros() >= expires {
                return Err(Status::deadline_exceeded("Product deadline expired"));
            }
            if response
                .metadata()
                .get("cache-control")
                .and_then(|v| v.to_str().ok())
                != Some("no-store")
            {
                return Err(Status::internal("Invalid product response"));
            }
            let response = response.into_inner();
            if response.body.is_empty() || response.body.len() > MAX_BODY {
                return Err(Status::internal("Invalid product response"));
            }
            Ok(response)
        };
        platform::call_deadline::scope(stop.into_std(), async {
            tokio::time::timeout_at(stop, Box::pin(work))
                .await
                .map_err(|_| Status::deadline_exceeded("Product deadline expired"))?
        })
        .await
    }

    /// Internal capability only. It never enters browser JSON, headers, caches
    /// or logs. The outermost HTTP body layer invokes it only after body EOF.
    pub fn delivery_token(
        &self,
        capability: proto::ResponseDelivery,
    ) -> Result<platform::actor_delegation::response_scope::ResponseScopeToken, Status> {
        validate_delivery(&capability)?;
        Ok(
            platform::actor_delegation::response_scope::ResponseScopeToken::new(
                std::sync::Arc::new(HttpDelivery {
                    client: self.clone(),
                    capability,
                }),
            ),
        )
    }
}

fn validate_delivery(value: &proto::ResponseDelivery) -> Result<(), Status> {
    for text in [&value.instance_id, &value.scope_id] {
        if !Uuid::parse_str(text)
            .ok()
            .is_some_and(|id| !id.is_nil() && id.to_string() == *text)
        {
            return Err(Status::internal("Invalid product delivery capability"));
        }
    }
    if value.binding_digest.len() != 32
        || value.binding_digest.iter().all(|byte| *byte == 0)
        || value.completion_secret.len() != 32
    {
        return Err(Status::internal("Invalid product delivery capability"));
    }
    Ok(())
}
struct HttpDelivery {
    client: GrpcConcertProductClient,
    capability: proto::ResponseDelivery,
}
impl platform::actor_delegation::response_scope::ResponseScope for HttpDelivery {
    fn finish_response(
        self: std::sync::Arc<Self>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Status>> + Send + 'static>>
    {
        Box::pin(async move {
            // A separate system operation has a fresh settlement-only budget.
            // Spawning avoids inheriting/renewing an expired content deadline.
            // No content access, actor delegation or retry is performed here.
            tokio::spawn(async move {
                let stop = tokio::time::Instant::now() + BUDGET;
                platform::call_deadline::scope(stop.into_std(), async move {
                    let mut request = tonic::Request::new(self.capability.clone());
                    request.set_timeout(BUDGET);
                    let mut client = self.client.delivery.clone();
                    let result = tokio::time::timeout_at(stop, client.complete_response(request))
                        .await
                        .map_err(|_| {
                            Status::deadline_exceeded("Product delivery completion unknown")
                        })??;
                    if !result.into_inner().completed {
                        return Err(Status::unavailable("Product delivery completion unknown"));
                    }
                    Ok(())
                })
                .await
            })
            .await
            .map_err(|_| Status::unavailable("Product delivery completion interrupted"))?
        })
    }
}
