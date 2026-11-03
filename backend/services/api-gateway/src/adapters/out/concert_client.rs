//! Signed bootstrap dispatch after fresh canonical owner reads; no retries/cache.
use super::{
    concert_identity::{ConcertIdentityReadError, ConcertIdentityReader},
    jwt::concert_access::ConcertAccessContext,
};
use platform::{
    actor_delegation::{request_for, ActorDelegationClient, ActorDelegationSigner},
    edge::{EdgeBudget, FailurePolicy, RetryClass},
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use std::time::Duration;
use tonic::{service::interceptor::InterceptedService, transport::Channel, Code};
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("concert.v1");
}
const BUDGET: Duration = Duration::from_secs(2);
const MAX_RESPONSE: usize = 4 * 1024 * 1024;
const EDGE: EdgeBudget = EdgeBudget {
    edge: "gateway.concert.bootstrap",
    expected_latency: Duration::from_millis(100),
    deadline: BUDGET,
    connect_deadline: Duration::from_millis(500),
    retry: RetryClass::Never,
    failure_policy: FailurePolicy::FailRequest,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BootstrapError {
    #[error("Bootstrap identity is unavailable")]
    Identity,
    #[error("Bootstrap is not permitted")]
    Denied,
    #[error("Bootstrap resource is unavailable")]
    NotFound,
    #[error("Bootstrap authority expired")]
    Expired,
    #[error("Bootstrap dependency is unavailable")]
    Unavailable,
    #[error("Bootstrap dependency response is invalid")]
    InvalidResponse,
}
impl From<ConcertIdentityReadError> for BootstrapError {
    fn from(error: ConcertIdentityReadError) -> Self {
        match error {
            ConcertIdentityReadError::AccessExpired
            | ConcertIdentityReadError::DeadlineExceeded => Self::Expired,
            ConcertIdentityReadError::NotEligible => Self::Denied,
            _ => Self::Identity,
        }
    }
}
#[derive(Clone)]
pub struct GrpcConcertClient {
    client: proto::concert_service_client::ConcertServiceClient<
        InterceptedService<ActorDelegationClient<Channel>, ServiceIdentityClientInterceptor>,
    >,
}
impl GrpcConcertClient {
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
            client: proto::concert_service_client::ConcertServiceClient::with_interceptor(
                ActorDelegationClient::new(channel, signer, Workload::Concert),
                ServiceIdentityClientInterceptor::new(Workload::Gateway, service_token)?,
            )
            .max_decoding_message_size(MAX_RESPONSE),
        })
    }
    pub async fn bootstrap(
        &self,
        reader: &ConcertIdentityReader,
        context: &ConcertAccessContext,
        event: Uuid,
        request_id: Uuid,
    ) -> Result<serde_json::Value, BootstrapError> {
        if event.is_nil() || request_id.is_nil() {
            return Err(BootstrapError::Denied);
        }
        context.check().map_err(|_| BootstrapError::Expired)?;
        let original = context
            .rpc_deadline()
            .map_err(|_| BootstrapError::Expired)?;
        let mut stop = original.stop();
        if let Some(outer) = platform::call_deadline::current() {
            stop = stop.min(outer.into());
        }
        let work = async {
            // Keep nested generated clients off the caller's async state stack.
            let identity = Box::pin(reader.read(context, None)).await?;
            identity.check()?;
            let actor = identity.user_id()?;
            let auth_at = identity.admission_observed_at()?.timestamp_micros();
            let profile_at = identity.profile_observed_at()?.timestamp_micros();
            let now = chrono::Utc::now().timestamp_micros();
            let remaining = context
                .remaining(BUDGET)
                .map_err(|_| BootstrapError::Expired)?
                .min(stop.saturating_duration_since(tokio::time::Instant::now()));
            let expires = original
                .expires_micros()
                .min(context.access_expires_micros())
                .min(now.saturating_add(remaining.as_micros() as i64))
                .min(auth_at.saturating_add(BUDGET.as_micros() as i64))
                .min(profile_at.saturating_add(BUDGET.as_micros() as i64));
            if remaining.is_zero() || expires <= now {
                return Err(BootstrapError::Expired);
            }
            let message = proto::BootstrapRequest {
                user_id: actor.to_string(),
                event_id: event.to_string(),
                session_family_id: context
                    .session_family()
                    .map_err(|_| BootstrapError::Expired)?
                    .to_string(),
                authority_expires_unix_micros: expires,
                auth_observed_at_unix_micros: auth_at,
                profile_observed_at_unix_micros: profile_at,
                display_name: identity.display_name()?.into(),
                bio: identity.bio()?.map(str::to_owned),
                request_id: request_id.to_string(),
                http_method: "GET".into(),
                http_target: format!("/api/concert/events/{event}/bootstrap"),
            };
            let mut request =
                request_for(message, &actor.to_string()).map_err(|_| BootstrapError::Denied)?;
            let dispatch_stop = stop.min(tokio::time::Instant::now() + remaining);
            let timeout = remaining.min(Duration::from_micros((expires - now) as u64));
            request.set_timeout(timeout);
            identity.check()?;
            let mut client = self.client.clone();
            let response = platform::call_deadline::scope(dispatch_stop.into_std(), async {
                tokio::time::timeout_at(dispatch_stop, Box::pin(client.bootstrap(request))).await
            })
            .await
            .map_err(|_| BootstrapError::Expired)?;
            identity.check()?;
            if chrono::Utc::now().timestamp_micros() >= expires
                || tokio::time::Instant::now() >= dispatch_stop
            {
                return Err(BootstrapError::Expired);
            }
            let response = response.map_err(|status| match status.code() {
                Code::PermissionDenied | Code::Unauthenticated => BootstrapError::Denied,
                Code::NotFound => BootstrapError::NotFound,
                Code::DeadlineExceeded => BootstrapError::Expired,
                _ => BootstrapError::Unavailable,
            })?;
            if response
                .metadata()
                .get("cache-control")
                .and_then(|v| v.to_str().ok())
                != Some("no-store")
            {
                return Err(BootstrapError::InvalidResponse);
            }
            let body = response.into_inner().bootstrap_json;
            if body.is_empty() || body.len() > MAX_RESPONSE {
                return Err(BootstrapError::InvalidResponse);
            }
            let value: serde_json::Value =
                serde_json::from_slice(&body).map_err(|_| BootstrapError::InvalidResponse)?;
            if value["contract_version"] != "0.1.0"
                || value["event"]["event_id"] != event.to_string()
                || value["participant"]["event_id"] != event.to_string()
                || value["participant"]["user_id"] != actor.to_string()
                || value["participant"]["profile"]["user_id"] != actor.to_string()
            {
                return Err(BootstrapError::InvalidResponse);
            }
            identity.check()?;
            if chrono::Utc::now().timestamp_micros() >= expires {
                return Err(BootstrapError::Expired);
            }
            Ok(value)
        };
        platform::call_deadline::scope(stop.into_std(), async {
            tokio::time::timeout_at(stop, Box::pin(work))
                .await
                .map_err(|_| BootstrapError::Expired)?
        })
        .await
    }
}
