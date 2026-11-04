//! Current canonical account/session observations from the actual Auth owner.
//! This implements no MutationAuthorityPort and no write/reveal grant conversion.
use super::{current_auth, owner_error, owner_reply, owner_window};
use crate::{
    adapters::out::relationship_authority::{grpc::proto, OwnerRevision},
    participant::{
        context::VerifiedParticipantRequest,
        ports::{CanonicalAuthPort, CurrentAuth},
    },
};
use concert_application::avatar::AvatarError;
use platform::{
    actor_delegation::{
        relationship_read::RelationshipReadSigner, request_for, ActorDelegationClient,
    },
    edge::{EdgeBudget, FailurePolicy, RetryClass},
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use std::time::Duration;
use tonic::{service::interceptor::InterceptedService, transport::Channel};
use uuid::Uuid;

type Transport =
    InterceptedService<ActorDelegationClient<Channel>, ServiceIdentityClientInterceptor>;
const EDGE: EdgeBudget = EdgeBudget {
    edge: "concert.participant.auth_read",
    expected_latency: Duration::from_millis(100),
    deadline: Duration::from_secs(2),
    connect_deadline: Duration::from_millis(500),
    retry: RetryClass::Never,
    failure_policy: FailurePolicy::FailRequest,
};

#[derive(Clone)]
pub struct GrpcCanonicalAuth {
    client: proto::auth_concert_relationship_service_client::AuthConcertRelationshipServiceClient<
        Transport,
    >,
}
impl GrpcCanonicalAuth {
    /// Runtime owns TLS-mode policy and explicit credentials. The read signer
    /// enforces a distinct Concert key, never a Gateway/parity signing fallback.
    pub fn connect(
        endpoint: &str,
        service_token: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        signer: RelationshipReadSigner,
    ) -> anyhow::Result<Self> {
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            endpoint,
            Workload::Auth,
            EDGE.validate()?,
        )?;
        let client = proto::auth_concert_relationship_service_client::AuthConcertRelationshipServiceClient::with_interceptor(
            signer.client(channel, Workload::Auth)?,
            ServiceIdentityClientInterceptor::new(Workload::Concert, service_token)?,
        )
        .max_decoding_message_size(4096)
        .max_encoding_message_size(4096);
        Ok(Self { client })
    }
}

#[async_trait::async_trait]
impl CanonicalAuthPort for GrpcCanonicalAuth {
    async fn read(&self, context: &VerifiedParticipantRequest) -> Result<CurrentAuth, AvatarError> {
        let (stop, expires, remaining) = owner_window(context)?;
        let query = proto::RelationshipRead {
            actor_id: context.key().user_id.uuid().to_string(),
            event_id: context.key().event_id.uuid().to_string(),
            subject_id: context.key().user_id.uuid().to_string(),
            session_family_id: context.session_family_id().to_string(),
            request_id: context.request_id().to_string(),
            purpose: "SAFETY_SELF".into(),
            expires_unix_micros: expires,
        };
        let mut request =
            request_for(query.clone(), &query.actor_id).map_err(|_| AvatarError::Denied)?;
        request.set_timeout(remaining);
        let started = chrono::Utc::now().timestamp_micros();
        let mut client = self.client.clone();
        let result = platform::call_deadline::scope(
            stop.into_std(),
            tokio::time::timeout_at(stop, Box::pin(client.read_current(request))),
        )
        .await;
        context.check().map_err(|_| AvatarError::Unauthenticated)?;
        if tokio::time::Instant::now() >= stop {
            return Err(AvatarError::Unauthenticated);
        }
        let body = owner_reply(
            result
                .map_err(|_| AvatarError::Unavailable)?
                .map_err(owner_error)?,
        )?;
        let epoch = Uuid::parse_str(&body.owner_epoch)
            .ok()
            .filter(|id| !id.is_nil() && id.to_string() == body.owner_epoch)
            .ok_or(AvatarError::Unavailable)?;
        OwnerRevision {
            epoch,
            revision: body.revision,
        }
        .validate()
        .map_err(|_| AvatarError::Unavailable)?;
        if body.query.as_ref() != Some(&query)
            || body.observed_at_unix_micros < started
            || body.valid_until_unix_micros > expires
        {
            return Err(AvatarError::Unavailable);
        }
        if !body.actor_active || !body.subject_active || !body.session_live {
            return Err(AvatarError::Denied);
        }
        current_auth(
            context,
            body.observed_at_unix_micros,
            body.valid_until_unix_micros,
        )
    }
}
