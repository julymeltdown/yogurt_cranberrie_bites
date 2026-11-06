//! C03 transport using the original sealed access context and actual identity reads.
use crate::{
    adapters::out::concert_identity::{ConcertIdentityReadError, ConcertIdentityReader},
    concert_access::ConcertAccessContext,
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
use crate::adapters::out::concert_product::GrpcConcertProductClient;
pub use response::{failure_status, validate_document};
pub use crate::concert_participant_http::{MAX_BODY, Operation, ParticipantHttpRequest};
const BUDGET: Duration = Duration::from_secs(2);
const EDGE: EdgeBudget = EdgeBudget {
    edge: "gateway.concert.participant",
    expected_latency: Duration::from_millis(100),
    deadline: BUDGET,
    connect_deadline: Duration::from_millis(500),
    retry: RetryClass::Never,
    failure_policy: FailurePolicy::FailRequest,
};
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
            let identity =
                Box::pin(reader.read(context, None))
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
                Operation::Read => Box::pin(client.get_participant(request)).await?,
                Operation::Avatar => Box::pin(client.set_avatar(request)).await?,
                Operation::Update => Box::pin(client.update_preferences(request)).await?,
            };
            // A received capability is cleanup authority, not content authority.
            // Validate the entire authenticated reply before any late freshness
            // refusal. Invalid/mismatched replies must remain quarantined.
            if response
                .metadata()
                .get("cache-control")
                .and_then(|v| v.to_str().ok())
                != Some("no-store")
            {
                return Err(Status::unavailable("Participant response unavailable"));
            }
            let mut value = response.into_inner();
            if value.content_expires_unix_micros <= 0 || value.content_expires_unix_micros > expires
            {
                return Err(Status::unavailable("Participant disclosure unavailable"));
            }
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
            // Preserve the exact validated opaque capability and original
            // ceiling. Only the outer HTTP EOF layer may use it; no background
            // completion, content renewal or producer work is introduced here.
            let late_failure = if identity.check().is_err() || context.check().is_err() {
                Some((tonic::Code::Unauthenticated, "AUTHENTICATION_REQUIRED"))
            } else if chrono::Utc::now().timestamp_micros() >= expires
                || value.content_expires_unix_micros <= chrono::Utc::now().timestamp_micros()
            {
                Some((tonic::Code::Unavailable, "DEPENDENCY_UNAVAILABLE"))
            } else {
                None
            };
            if let Some((code, reason)) = late_failure {
                value.response_json.clear();
                value.failure = Some(proto::ParticipantFailure {
                    grpc_code: code as u32,
                    core_reason: reason.into(),
                });
            }
            Ok(value)
        };
        platform::call_deadline::scope(stop.into_std(), async {
            tokio::time::timeout_at(stop, Box::pin(work))
                .await
                .map_err(|_| Status::deadline_exceeded("Participant deadline elapsed"))?
        })
        .await
    }
    pub fn delivery_token(
        &self,
        capability: crate::adapters::out::concert_product::proto::ResponseDelivery,
    ) -> Result<platform::actor_delegation::response_scope::ResponseScopeToken, Status> {
        if capability.completion_secret.len() != 32
            || capability.completion_secret.iter().all(|b| *b == 0)
        {
            return Err(Status::unavailable("Participant delivery unavailable"));
        }
        self.delivery.delivery_token(capability)
    }
}
