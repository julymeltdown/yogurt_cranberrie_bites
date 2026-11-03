//! Signed Gateway client for the internal world producer. No cache or replay.
use super::{
    concert_identity::ConcertIdentityObservation, jwt::concert_access::ConcertAccessContext,
};
use platform::{
    actor_delegation::{request_for, ActorDelegationClient, ActorDelegationSigner},
    edge::{EdgeBudget, FailurePolicy, RetryClass},
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use serde_json::Value;
use std::time::Duration;
use tonic::{service::interceptor::InterceptedService, transport::Channel, Code};
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("concert.world.v1");
}

const BUDGET: Duration = Duration::from_secs(2);
const MAX_RESPONSE: usize = 65_536;
const EDGE: EdgeBudget = EdgeBudget {
    edge: "gateway.concert.world",
    expected_latency: Duration::from_millis(100),
    deadline: BUDGET,
    connect_deadline: Duration::from_millis(500),
    retry: RetryClass::Never,
    failure_policy: FailurePolicy::FailRequest,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WorldClientError {
    #[error("World access expired")]
    Expired,
    #[error("World request denied")]
    Denied,
    #[error("World room is unavailable")]
    NotFound,
    #[error("World room is full")]
    Full,
    #[error("World release does not match")]
    VersionMismatch,
    #[error("World dependency is unavailable")]
    Unavailable,
    #[error("World response is invalid")]
    InvalidResponse,
}

#[derive(Clone)]
pub struct GrpcConcertWorldClient {
    client: proto::world_service_client::WorldServiceClient<
        InterceptedService<ActorDelegationClient<Channel>, ServiceIdentityClientInterceptor>,
    >,
}
impl GrpcConcertWorldClient {
    pub fn connect(
        endpoint: &str,
        service_token: &str,
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
            client: proto::world_service_client::WorldServiceClient::with_interceptor(
                ActorDelegationClient::new(channel, signer, Workload::Concert),
                ServiceIdentityClientInterceptor::new(Workload::Gateway, service_token)?,
            )
            .max_decoding_message_size(MAX_RESPONSE),
        })
    }

    pub async fn rooms(
        &self,
        identity: &ConcertIdentityObservation<'_>,
        context: &ConcertAccessContext,
        event: Uuid,
    ) -> Result<Value, WorldClientError> {
        self.dispatch(identity, context, event, "GET", Vec::new(), "ListRooms")
            .await
    }

    pub async fn ticket(
        &self,
        identity: &ConcertIdentityObservation<'_>,
        context: &ConcertAccessContext,
        event: Uuid,
        body: Vec<u8>,
    ) -> Result<Value, WorldClientError> {
        if body.is_empty() || body.len() > 4096 {
            return Err(WorldClientError::Denied);
        }
        self.dispatch(identity, context, event, "POST", body, "IssueTicket")
            .await
    }

    async fn dispatch(
        &self,
        identity: &ConcertIdentityObservation<'_>,
        context: &ConcertAccessContext,
        event: Uuid,
        method: &str,
        body: Vec<u8>,
        rpc: &str,
    ) -> Result<Value, WorldClientError> {
        if event.is_nil() {
            return Err(WorldClientError::Denied);
        }
        context.check().map_err(|_| WorldClientError::Expired)?;
        identity.check().map_err(|_| WorldClientError::Expired)?;
        let original = context
            .rpc_deadline()
            .map_err(|_| WorldClientError::Expired)?;
        let mut stop = original.stop();
        if let Some(outer) = platform::call_deadline::current() {
            stop = stop.min(outer.into());
        }
        let work = async {
            let actor = identity.user_id().map_err(|_| WorldClientError::Expired)?;
            let auth_at = identity
                .admission_observed_at()
                .map_err(|_| WorldClientError::Expired)?
                .timestamp_micros();
            let profile_at = identity
                .profile_observed_at()
                .map_err(|_| WorldClientError::Expired)?
                .timestamp_micros();
            let now = chrono::Utc::now().timestamp_micros();
            let remaining = context
                .remaining(BUDGET)
                .map_err(|_| WorldClientError::Expired)?
                .min(stop.saturating_duration_since(tokio::time::Instant::now()));
            let expires = original
                .expires_micros()
                .min(context.access_expires_micros())
                .min(now.saturating_add(remaining.as_micros() as i64))
                .min(auth_at.saturating_add(BUDGET.as_micros() as i64))
                .min(profile_at.saturating_add(BUDGET.as_micros() as i64));
            if remaining.is_zero() || expires <= now {
                return Err(WorldClientError::Expired);
            }
            let target = format!(
                "/api/concert/events/{event}/{}",
                if method == "GET" {
                    "rooms"
                } else {
                    "world-tickets"
                }
            );
            let message = proto::WorldRequest {
                user_id: actor.to_string(),
                event_id: event.to_string(),
                session_family_id: context
                    .session_family()
                    .map_err(|_| WorldClientError::Expired)?
                    .to_string(),
                request_id: Uuid::new_v4().to_string(),
                http_method: method.into(),
                http_target: target,
                http_body: body,
                authority_expires_unix_micros: expires,
                auth_observed_at_unix_micros: auth_at,
                profile_observed_at_unix_micros: profile_at,
            };
            let mut request =
                request_for(message, &actor.to_string()).map_err(|_| WorldClientError::Denied)?;
            let dispatch_stop = stop.min(tokio::time::Instant::now() + remaining);
            request.set_timeout(remaining.min(Duration::from_micros((expires - now) as u64)));
            identity.check().map_err(|_| WorldClientError::Expired)?;
            let mut client = self.client.clone();
            let response = platform::call_deadline::scope(dispatch_stop.into_std(), async {
                match rpc {
                    "ListRooms" => {
                        tokio::time::timeout_at(dispatch_stop, client.list_rooms(request))
                            .await
                            .map_err(|_| WorldClientError::Expired)
                    }
                    "IssueTicket" => {
                        tokio::time::timeout_at(dispatch_stop, client.issue_ticket(request))
                            .await
                            .map_err(|_| WorldClientError::Expired)
                    }
                    _ => return Err(WorldClientError::Denied),
                }
            })
            .await
            .map_err(|_| WorldClientError::Expired)??;
            identity.check().map_err(|_| WorldClientError::Expired)?;
            let response = response.map_err(classify)?;
            if response
                .metadata()
                .get("cache-control")
                .and_then(|v| v.to_str().ok())
                != Some("no-store")
            {
                return Err(WorldClientError::InvalidResponse);
            }
            let bytes = response.into_inner().world_json;
            if bytes.is_empty() || bytes.len() > MAX_RESPONSE {
                return Err(WorldClientError::InvalidResponse);
            }
            let value: Value =
                serde_json::from_slice(&bytes).map_err(|_| WorldClientError::InvalidResponse)?;
            identity.check().map_err(|_| WorldClientError::Expired)?;
            Ok(value)
        };
        platform::call_deadline::scope(stop.into_std(), async {
            tokio::time::timeout_at(stop, Box::pin(work))
                .await
                .map_err(|_| WorldClientError::Expired)?
        })
        .await
    }
}

fn classify(status: tonic::Status) -> WorldClientError {
    match status.code() {
        Code::PermissionDenied | Code::Unauthenticated => WorldClientError::Denied,
        Code::NotFound => WorldClientError::NotFound,
        Code::ResourceExhausted => WorldClientError::Full,
        Code::Aborted => WorldClientError::VersionMismatch,
        Code::DeadlineExceeded => WorldClientError::Expired,
        _ => WorldClientError::Unavailable,
    }
}
