//! Real signed owner clients. No retries, cache, fallback or cross-owner DB access.
use super::*;
use platform::{
    actor_delegation::{
        relationship_read::RelationshipReadSigner, request_for, ActorDelegationClient,
    },
    edge::{EdgeBudget, FailurePolicy, RetryClass},
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use tonic::{service::interceptor::InterceptedService, transport::Channel};
pub mod proto {
    tonic::include_proto!("relationship.v1");
}
pub(super) type Transport =
    InterceptedService<ActorDelegationClient<Channel>, ServiceIdentityClientInterceptor>;
pub(super) const EDGE: EdgeBudget = EdgeBudget {
    edge: "concert.relationship.owner_read",
    expected_latency: Duration::from_millis(100),
    deadline: BUDGET,
    connect_deadline: Duration::from_millis(500),
    retry: RetryClass::Never,
    failure_policy: FailurePolicy::FailRequest,
};
#[derive(Clone)]
pub struct GrpcRelationshipAuthority {
    auth: proto::auth_concert_relationship_service_client::AuthConcertRelationshipServiceClient<
        Transport,
    >,
    friend:
        proto::friend_concert_relationship_service_client::FriendConcertRelationshipServiceClient<
            Transport,
        >,
}
impl GrpcRelationshipAuthority {
    pub fn connect(
        auth_endpoint: &str,
        friend_endpoint: &str,
        service_token: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        signer: RelationshipReadSigner,
    ) -> anyhow::Result<Self> {
        let auth = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            auth_endpoint,
            Workload::Auth,
            EDGE.validate()?,
        )?;
        let friend = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            friend_endpoint,
            Workload::Friend,
            EDGE.validate()?,
        )?;
        Ok(Self{
            auth:proto::auth_concert_relationship_service_client::AuthConcertRelationshipServiceClient::with_interceptor(signer.client(auth,Workload::Auth)?,ServiceIdentityClientInterceptor::new(Workload::Concert,service_token)?).max_decoding_message_size(4096).max_encoding_message_size(4096),
            friend:proto::friend_concert_relationship_service_client::FriendConcertRelationshipServiceClient::with_interceptor(signer.client(friend,Workload::Friend)?,ServiceIdentityClientInterceptor::new(Workload::Concert,service_token)?).max_decoding_message_size(4096).max_encoding_message_size(4096),
        })
    }
}
fn purpose(field: Field) -> &'static str {
    match field {
        Field::PublicProfile => "PUBLIC_PROFILE",
        Field::Presence => "PRESENCE",
        Field::Interaction => "INTERACTION",
        Field::SafetySelf => "SAFETY_SELF",
        Field::SafetyInteraction => "SAFETY_INTERACTION",
        Field::PrivateHandle => "PRIVATE_HANDLE",
    }
}
fn timestamp(micros: i64) -> Result<DateTime<Utc>, RelationshipError> {
    DateTime::from_timestamp_micros(micros)
        .filter(|v| v.timestamp_micros() > 0)
        .ok_or(RelationshipError::Unavailable)
}
pub(super) fn revision(epoch: &str, revision: i64) -> Result<OwnerRevision, RelationshipError> {
    let id = Uuid::parse_str(epoch).map_err(|_| RelationshipError::Unavailable)?;
    if id.to_string() != epoch {
        return Err(RelationshipError::Unavailable);
    }
    OwnerRevision {
        epoch: id,
        revision,
    }
    .validate()
}
pub(super) fn window(
    query: &Option<proto::RelationshipRead>,
    expected: &proto::RelationshipRead,
    observed: i64,
    until: i64,
) -> Result<(DateTime<Utc>, DateTime<Utc>), RelationshipError> {
    let at = timestamp(observed)?;
    let expires = timestamp(until)?;
    let now = Utc::now();
    if query.as_ref() != Some(expected)
        || at > now
        || expires <= now
        || expires <= at
        || expires.timestamp_micros() > expected.expires_unix_micros
        || expires - at > chrono::Duration::seconds(2)
    {
        return Err(RelationshipError::Unavailable);
    }
    Ok((at, expires))
}
pub(super) fn reply<T>(response: tonic::Response<T>) -> Result<T, RelationshipError> {
    if response
        .metadata()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        != Some("no-store")
    {
        return Err(RelationshipError::Unavailable);
    }
    Ok(response.into_inner())
}
#[async_trait::async_trait]
impl BoundedRelationshipAuthority for GrpcRelationshipAuthority {
    async fn observe(
        &self,
        binding: &RequestBinding,
    ) -> Result<BoundedObservation, RelationshipError> {
        binding.check()?;
        let mut stop = binding.deadline().stop();
        if let Some(original) = platform::call_deadline::current() {
            stop = stop.min(original.into());
        }
        let remaining = binding
            .deadline()
            .remaining(BUDGET, binding.deadline().expires_micros())
            .map_err(|_| RelationshipError::Unavailable)?
            .min(stop.saturating_duration_since(tokio::time::Instant::now()));
        if remaining.is_zero() {
            return Err(RelationshipError::Unavailable);
        }
        let expires = binding.deadline().expires_micros().min(
            Utc::now()
                .timestamp_micros()
                .saturating_add(remaining.as_micros() as i64),
        );
        let q = binding.query;
        let message = proto::RelationshipRead {
            actor_id: q.actor.uuid().to_string(),
            event_id: q.event.uuid().to_string(),
            subject_id: q.subject.uuid().to_string(),
            session_family_id: binding.family.to_string(),
            request_id: binding.request.to_string(),
            purpose: purpose(q.field).into(),
            expires_unix_micros: expires,
        };
        let dispatch = async {
            let mut auth_request = request_for(message.clone(), &message.actor_id)
                .map_err(|_| RelationshipError::Unavailable)?;
            auth_request.set_timeout(stop.saturating_duration_since(tokio::time::Instant::now()));
            let mut auth_client = self.auth.clone();
            let auth = reply(
                Box::pin(auth_client.read_current(auth_request))
                    .await
                    .map_err(|_| RelationshipError::Unavailable)?,
            )?;
            binding.check()?;
            let (auth_at, auth_until) = window(
                &auth.query,
                &message,
                auth.observed_at_unix_micros,
                auth.valid_until_unix_micros,
            )?;
            let auth_revision = revision(&auth.owner_epoch, auth.revision)?;
            let mut friend_request = request_for(message.clone(), &message.actor_id)
                .map_err(|_| RelationshipError::Unavailable)?;
            friend_request.set_timeout(stop.saturating_duration_since(tokio::time::Instant::now()));
            let mut friend_client = self.friend.clone();
            let friend = reply(
                Box::pin(friend_client.read_current(friend_request))
                    .await
                    .map_err(|_| RelationshipError::Unavailable)?,
            )?;
            binding.check()?;
            let (friend_at, friend_until) = window(
                &friend.query,
                &message,
                friend.observed_at_unix_micros,
                friend.valid_until_unix_micros,
            )?;
            let friend_revision = revision(&friend.owner_epoch, friend.revision)?;
            let observation = BoundedObservation {
                query: q,
                family: binding.family,
                request: binding.request,
                revision: RevisionVector {
                    auth: auth_revision,
                    friend: friend_revision,
                },
                auth_observed_at: auth_at,
                friend_observed_at: friend_at,
                valid_until: auth_until.min(friend_until),
                actor_allowed: auth.actor_active && auth.session_live && !friend.actor_deleted,
                subject_allowed: auth.subject_active && !friend.subject_deleted,
                inherited_block: friend.actor_blocks_subject || friend.subject_blocks_actor,
            };
            observation.ensure_current(binding, observation.revision, Utc::now())?;
            Ok(observation)
        };
        let result = platform::call_deadline::scope(
            stop.into_std(),
            tokio::time::timeout_at(stop, dispatch),
        )
        .await
        .map_err(|_| RelationshipError::Unavailable)?;
        binding.check()?;
        result
    }
}
