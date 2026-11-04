//! C03 transport using the original sealed access context and actual identity reads.
use super::{
    concert_identity::{ConcertIdentityReadError, ConcertIdentityReader},
    jwt::concert_access::ConcertAccessContext,
};
use platform::{
    actor_delegation::{request_for, ActorDelegationClient, ActorDelegationSigner},
    edge::{EdgeBudget, FailurePolicy, RetryClass},
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use std::{sync::Arc, time::Duration};
use tonic::{service::interceptor::InterceptedService, transport::Channel, Status};
use uuid::Uuid;
pub mod proto {
    include!(concat!(
        env!("OUT_DIR"),
        "/participant/concert.participant.v1.rs"
    ));
}
mod response;
use super::concert_product::GrpcConcertProductClient;
pub use response::{failure_status, validate_document};
pub const MAX_BODY: usize = 8192;
const BUDGET: Duration = Duration::from_secs(2);
const EDGE: EdgeBudget = EdgeBudget {
    edge: "gateway.concert.participant",
    expected_latency: Duration::from_millis(100),
    deadline: BUDGET,
    connect_deadline: Duration::from_millis(500),
    retry: RetryClass::Never,
    failure_policy: FailurePolicy::FailRequest,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Read,
    Avatar,
    Update,
}
impl Operation {
    pub fn method(self) -> &'static str {
        match self {
            Self::Read => "GET",
            Self::Avatar => "PUT",
            Self::Update => "PATCH",
        }
    }
    pub fn target(self, event: Uuid) -> String {
        format!(
            "/api/concert/events/{event}/me{}",
            if self == Self::Avatar { "/avatar" } else { "" }
        )
    }
}
pub struct ParticipantHttpRequest {
    pub event: Uuid,
    pub request_id: Uuid,
    pub operation: Operation,
    pub body: Vec<u8>,
}
#[derive(Clone)]
pub struct GrpcConcertParticipantClient {
    client: proto::concert_participant_service_client::ConcertParticipantServiceClient<
        InterceptedService<ActorDelegationClient<Channel>, ServiceIdentityClientInterceptor>,
    >,
    delivery: GrpcConcertProductClient,
    release: Arc<str>,
}
impl GrpcConcertParticipantClient {
    pub fn connect(
        endpoint: &str,
        token: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        signer: ActorDelegationSigner,
        release_digest: &str,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            release_digest.len() == 64
                && release_digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "Valid configured Participant release required"
        );
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            endpoint,
            Workload::Concert,
            EDGE.validate()?,
        )?;
        Ok(Self {client:proto::concert_participant_service_client::ConcertParticipantServiceClient::with_interceptor(ActorDelegationClient::new(channel,signer.clone(),Workload::Concert),ServiceIdentityClientInterceptor::new(Workload::Gateway,token)?).max_encoding_message_size(16384).max_decoding_message_size(32768),delivery:GrpcConcertProductClient::connect(endpoint,token,tls,signer)?,release:release_digest.into()})
    }
    pub async fn dispatch(
        &self,
        reader: &ConcertIdentityReader,
        context: &ConcertAccessContext,
        http: ParticipantHttpRequest,
    ) -> Result<proto::ParticipantResponse, Status> {
        if http.event.is_nil()
            || http.request_id.is_nil()
            || http.body.len() > MAX_BODY
            || (http.operation == Operation::Read && !http.body.is_empty())
        {
            return Err(Status::invalid_argument("Invalid Participant request"));
        }
        context
            .check()
            .map_err(|_| Status::unauthenticated("Current access required"))?;
        let original = context
            .rpc_deadline()
            .map_err(|_| Status::deadline_exceeded("Participant deadline elapsed"))?;
        let mut stop = original.stop();
        if let Some(outer) = platform::call_deadline::current() {
            stop = stop.min(outer.into());
        }
        let work = async {
            let identity = reader
                .read(context, None)
                .await
                .map_err(|error| match error {
                    ConcertIdentityReadError::NotEligible => {
                        Status::permission_denied("Current identity required")
                    }
                    ConcertIdentityReadError::AccessExpired => {
                        Status::unauthenticated("Current access required")
                    }
                    ConcertIdentityReadError::DeadlineExceeded => {
                        Status::deadline_exceeded("Participant deadline elapsed")
                    }
                    _ => Status::unavailable("Participant identity unavailable"),
                })?;
            identity
                .check()
                .map_err(|_| Status::unauthenticated("Current identity required"))?;
            let actor = identity
                .user_id()
                .map_err(|_| Status::unauthenticated("Current identity required"))?;
            let auth_at = identity
                .admission_observed_at()
                .map_err(|_| Status::unavailable("Current identity required"))?
                .timestamp_micros();
            let profile_at = identity
                .profile_observed_at()
                .map_err(|_| Status::unavailable("Current identity required"))?
                .timestamp_micros();
            let remaining = context
                .remaining(BUDGET)
                .map_err(|_| Status::deadline_exceeded("Participant deadline elapsed"))?
                .min(stop.saturating_duration_since(tokio::time::Instant::now()));
            let now = chrono::Utc::now().timestamp_micros();
            let expires = original
                .expires_micros()
                .min(context.access_expires_micros())
                .min(now.saturating_add(remaining.as_micros() as i64))
                .min(auth_at.saturating_add(2_000_000))
                .min(profile_at.saturating_add(2_000_000));
            if remaining.is_zero() || expires <= now {
                return Err(Status::deadline_exceeded("Participant deadline elapsed"));
            }
            let message = proto::ParticipantRequest {
                user_id: actor.to_string(),
                event_id: http.event.to_string(),
                session_family_id: context
                    .session_family()
                    .map_err(|_| Status::unauthenticated("Current session required"))?
                    .to_string(),
                request_id: http.request_id.to_string(),
                http_method: http.operation.method().into(),
                http_target: http.operation.target(http.event),
                request_json: http.body,
                release_digest: self.release.to_string(),
                authority_expires_unix_micros: expires,
                auth_observed_at_unix_micros: auth_at,
                profile_observed_at_unix_micros: profile_at,
            };
            let mut request = request_for(message, &actor.to_string())?;
            request.set_timeout(remaining.min(Duration::from_micros((expires - now) as u64)));
            let mut client = self.client.clone();
            let response = match http.operation {
                Operation::Read => client.get_participant(request).await?,
                Operation::Avatar => client.set_avatar(request).await?,
                Operation::Update => client.update_preferences(request).await?,
            };
            identity
                .check()
                .map_err(|_| Status::unauthenticated("Current identity required"))?;
            context
                .check()
                .map_err(|_| Status::unauthenticated("Current access required"))?;
            if chrono::Utc::now().timestamp_micros() >= expires
                || response
                    .metadata()
                    .get("cache-control")
                    .and_then(|v| v.to_str().ok())
                    != Some("no-store")
            {
                return Err(Status::unavailable("Participant response unavailable"));
            }
            let value = response.into_inner();
            let capability = value
                .delivery
                .as_ref()
                .ok_or_else(|| Status::unavailable("Participant delivery unavailable"))?;
            self.delivery_token(capability.clone())?;
            match &value.failure {
                Some(failure) if value.response_json.is_empty() => {
                    failure_status(failure)?;
                }
                None => validate_document(&value.response_json, http.operation, actor, http.event)?,
                _ => return Err(Status::unavailable("Participant response unavailable")),
            }
            Ok(value)
        };
        platform::call_deadline::scope(stop.into_std(), async {
            tokio::time::timeout_at(stop, work)
                .await
                .map_err(|_| Status::deadline_exceeded("Participant deadline elapsed"))?
        })
        .await
    }
    pub fn delivery_token(
        &self,
        capability: super::concert_product::proto::ResponseDelivery,
    ) -> Result<platform::actor_delegation::response_scope::ResponseScopeToken, Status> {
        if capability.completion_secret.len() != 32
            || capability.completion_secret.iter().all(|b| *b == 0)
        {
            return Err(Status::unavailable("Participant delivery unavailable"));
        }
        self.delivery.delivery_token(capability)
    }
}
