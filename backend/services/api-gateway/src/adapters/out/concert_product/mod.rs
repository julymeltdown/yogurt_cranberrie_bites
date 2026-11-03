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
    client: proto::product_service_client::ProductServiceClient<
        InterceptedService<ActorDelegationClient<Channel>, ServiceIdentityClientInterceptor>,
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
            client: proto::product_service_client::ProductServiceClient::with_interceptor(
                ActorDelegationClient::new(channel, signer, Workload::Concert),
                ServiceIdentityClientInterceptor::new(Workload::Gateway, token)?,
            )
            .max_decoding_message_size(MAX_BODY + 16 * 1024)
            .max_encoding_message_size(MAX_BODY + 16 * 1024),
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
}
