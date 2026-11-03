//! Request-local current self content from its owning service. Avatar stays local.
use super::relationship_authority::{world_projection::WorldProfileProjection, BUDGET};
use chrono::{DateTime, Utc};
use concert_application::world_ticket::WorldError;
use platform::{
    actor_delegation::{
        authority_deadline::AuthorityRpcDeadline, relationship_read::RelationshipReadSigner,
        request_for, ActorDelegationClient,
    },
    edge::{EdgeBudget, FailurePolicy, RetryClass},
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use tonic::{service::interceptor::InterceptedService, transport::Channel};
use uuid::Uuid;
pub(super) mod proto {
    tonic::include_proto!("profile.v1");
}
type Transport =
    InterceptedService<ActorDelegationClient<Channel>, ServiceIdentityClientInterceptor>;
const EDGE: EdgeBudget = EdgeBudget {
    edge: "concert.world.profile_read",
    expected_latency: std::time::Duration::from_millis(100),
    deadline: BUDGET,
    connect_deadline: std::time::Duration::from_millis(500),
    retry: RetryClass::Never,
    failure_policy: FailurePolicy::FailRequest,
};
/// Current content only; it conveys no event admission or publication permission.
pub(crate) struct WorldProfileObservation {
    subject: Uuid,
    display_name: String,
    observed_at: DateTime<Utc>,
    event: Uuid,
    family: Uuid,
    request: Uuid,
    deadline: AuthorityRpcDeadline,
}
impl std::fmt::Debug for WorldProfileObservation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WorldProfileObservation([REDACTED])")
    }
}
impl WorldProfileObservation {
    pub(crate) fn subject(&self) -> Uuid {
        self.subject
    }
    pub(crate) fn display_name(&self) -> &str {
        &self.display_name
    }
    pub(crate) fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }
    pub(crate) fn check(&self, context: &WorldProfileProjection) -> Result<(), WorldError> {
        context.check()?;
        self.deadline
            .remaining(BUDGET, self.deadline.expires_micros())
            .map_err(|_| WorldError::Unavailable)?;
        let now = Utc::now();
        if self.subject != context.actor()
            || self.subject != context.subject()
            || self.event != context.event_id()
            || self.family != context.session_family_id()
            || self.request != context.request_id()
            || self.observed_at > now
            || now.signed_duration_since(self.observed_at) > chrono::Duration::seconds(2)
        {
            return Err(WorldError::Unavailable);
        }
        Ok(())
    }
}
#[derive(Clone)]
pub(crate) struct GrpcWorldProfileReader {
    client:
        proto::concert_world_profile_service_client::ConcertWorldProfileServiceClient<Transport>,
}
impl GrpcWorldProfileReader {
    pub(crate) fn connect(
        endpoint: &str,
        service_token: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        signer: RelationshipReadSigner,
    ) -> anyhow::Result<Self> {
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            endpoint,
            Workload::Profile,
            EDGE.validate()?,
        )?;
        let client = proto::concert_world_profile_service_client::ConcertWorldProfileServiceClient::with_interceptor(
            signer.client(channel, Workload::Profile)?,
            ServiceIdentityClientInterceptor::new(Workload::Concert, service_token)?,
        )
        .max_decoding_message_size(4096)
        .max_encoding_message_size(4096);
        Ok(Self { client })
    }
    pub(crate) async fn observe(
        &self,
        context: &WorldProfileProjection,
    ) -> Result<WorldProfileObservation, WorldError> {
        let attempt = context.begin_read()?;
        attempt
            .run(async {
                context.check()?;
                let original = context.deadline();
                let deadline = AuthorityRpcDeadline::within_original(
                    original.expires_micros(),
                    platform::call_deadline::current()
                        .unwrap_or(original.stop().into_std())
                        .min(original.stop().into_std()),
                    BUDGET,
                )
                .map_err(|_| WorldError::Unavailable)?;
                let mut remaining = deadline
                    .remaining(BUDGET, deadline.expires_micros())
                    .map_err(|_| WorldError::Unavailable)?;
                if let Some(stop) = platform::call_deadline::current() {
                    remaining =
                        remaining.min(stop.saturating_duration_since(std::time::Instant::now()));
                }
                if remaining.is_zero() {
                    return Err(WorldError::Unavailable);
                }
                let expires = deadline.expires_micros().min(
                    Utc::now()
                        .timestamp_micros()
                        .saturating_add(remaining.as_micros() as i64),
                );
                let message = proto::WorldProfileReadRequest {
                    actor_id: context.actor().to_string(),
                    event_id: context.event_id().to_string(),
                    subject_id: context.subject().to_string(),
                    session_family_id: context.session_family_id().to_string(),
                    request_id: context.request_id().to_string(),
                    purpose: proto::WorldProfilePurpose::SelfProfile as i32,
                    expires_unix_micros: expires,
                };
                let mut request = request_for(message.clone(), &message.actor_id)
                    .map_err(|_| WorldError::Unavailable)?;
                request.set_timeout(remaining);
                let response = self
                    .client
                    .clone()
                    .read_current(request)
                    .await
                    .map_err(|_| WorldError::Unavailable)?;
                context.check()?;
                if response
                    .metadata()
                    .get("cache-control")
                    .and_then(|v| v.to_str().ok())
                    != Some("no-store")
                {
                    return Err(WorldError::Unavailable);
                }
                let body = response.into_inner();
                let subject = Uuid::parse_str(&body.subject_id)
                    .ok()
                    .filter(|id| !id.is_nil() && id.to_string() == body.subject_id)
                    .ok_or(WorldError::Unavailable)?;
                let observed_at = DateTime::from_timestamp_micros(body.observed_at_unix_micros)
                    .filter(|v| v.timestamp_micros() > 0)
                    .ok_or(WorldError::Unavailable)?;
                if body.display_name.trim().is_empty()
                    || body.display_name.chars().count() > 40
                    || body.display_name.chars().any(char::is_control)
                {
                    return Err(WorldError::Unavailable);
                }
                let result = WorldProfileObservation {
                    subject,
                    display_name: body.display_name,
                    observed_at,
                    event: context.event_id(),
                    family: context.session_family_id(),
                    request: context.request_id(),
                    deadline,
                };
                result.check(context)?;
                Ok(result)
            })
            .await
    }
}
#[cfg(test)]
mod tests;
