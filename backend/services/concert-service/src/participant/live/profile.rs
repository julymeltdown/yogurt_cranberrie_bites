//! Actual typed canonical SELF client. Staged until the SELF producer and root
//! generation checkpoint are qualified; this file is not exported yet.
use super::{
    coordinator::*,
    guarded::BoundParticipantAuthority,
    owner_error, owner_reply, owner_window,
    update::{Field, ParticipantUpdate},
};
use crate::participant::{
    context::VerifiedParticipantRequest,
    ports::{CanonicalProfilePort, CurrentProfile},
};
use concert_application::avatar::AvatarError;
use concert_domain::Revision;
use platform::{
    actor_delegation::{
        concert_self_profile::ConcertSelfProfileSigner, request_for, ActorDelegationClient,
    },
    edge::{EdgeBudget, FailurePolicy, RetryClass},
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use std::{future::Future, sync::Arc, time::Duration};
use tonic::{service::interceptor::InterceptedService, transport::Channel, Response, Status};
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("profile.self_.v1");
}
type Transport =
    InterceptedService<ActorDelegationClient<Channel>, ServiceIdentityClientInterceptor>;
const EDGE: EdgeBudget = EdgeBudget {
    edge: "concert.participant.profile_self",
    expected_latency: Duration::from_millis(100),
    deadline: Duration::from_secs(2),
    connect_deadline: Duration::from_millis(500),
    retry: RetryClass::Never,
    failure_policy: FailurePolicy::FailRequest,
};
#[derive(Clone)]
pub struct GrpcCanonicalSelfProfile {
    client: proto::concert_self_profile_service_client::ConcertSelfProfileServiceClient<Transport>,
}
pub struct ObservedSelfProfile {
    pub current: CurrentProfile,
    pub version: CanonicalProfileVersion,
}
pub struct PreparedSelfProfile {
    pub(crate) content: proto::SelfContent,
    pub(crate) native: NativeTransaction,
    pub(crate) effect: [u8; 32],
    pub(crate) instance: Uuid,
}
#[derive(Clone)]
pub struct BoundCanonicalSelfProfile {
    client: GrpcCanonicalSelfProfile,
    authority: Arc<BoundParticipantAuthority>,
}
impl GrpcCanonicalSelfProfile {
    pub fn connect(
        endpoint: &str,
        token: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        signer: ConcertSelfProfileSigner,
    ) -> anyhow::Result<Self> {
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            endpoint,
            Workload::Profile,
            EDGE.validate()?,
        )?;
        Ok(Self { client:proto::concert_self_profile_service_client::ConcertSelfProfileServiceClient::with_interceptor(signer.client(channel),ServiceIdentityClientInterceptor::new(Workload::Concert,token)?).max_decoding_message_size(32768).max_encoding_message_size(131072) })
    }
    pub fn bind(&self, authority: Arc<BoundParticipantAuthority>) -> BoundCanonicalSelfProfile {
        BoundCanonicalSelfProfile {
            client: self.clone(),
            authority,
        }
    }

    /// Fresh bounded recovery can ONLY inspect/settle a persisted original
    /// operation. Missing owner views cannot authorize Read or PrepareUpdate.
    pub(crate) async fn recover(
        &self,
        operation: &RecoveryOperation,
    ) -> Result<ProfileTerminalReceipt, AvatarError> {
        let stop = tokio::time::Instant::now() + Duration::from_secs(2);
        let message = proto::SelfOperationRequest {
            scope: Some(proto::SelfScope {
                binding: Some(operation.binding.clone()),
                auth_view: None,
                friend_view: None,
            }),
            operation_id: operation.operation.to_string(),
        };
        let mut request =
            request_for(message, &operation.actor.to_string()).map_err(owner_error)?;
        request.set_timeout(Duration::from_secs(2));
        let mut client = self.client.clone();
        let result = platform::call_deadline::scope(
            stop.into_std(),
            tokio::time::timeout_at(stop, Box::pin(client.reconcile(request))),
        )
        .await
        .map_err(|_| AvatarError::Unavailable)?
        .map_err(owner_error)?;
        let reply = owner_reply(result)?;
        if tokio::time::Instant::now() >= stop {
            return Err(AvatarError::Unavailable);
        }
        terminal_reply(
            reply,
            operation.operation,
            &operation.expected_profile,
            None,
        )
    }
}
impl BoundCanonicalSelfProfile {
    fn scope(&self) -> proto::SelfScope {
        proto::SelfScope {
            binding: Some(self.authority.scope().wire_binding()),
            auth_view: Some(self.authority.scope().auth_view()),
            friend_view: Some(self.authority.scope().friend_view()),
        }
    }
    async fn call<T>(
        &self,
        context: &VerifiedParticipantRequest,
        future: impl Future<Output = Result<Response<T>, Status>>,
    ) -> Result<T, AvatarError> {
        let (stop, _, _) = owner_window(context)?;
        let response =
            platform::call_deadline::scope(stop.into_std(), tokio::time::timeout_at(stop, future))
                .await
                .map_err(|_| AvatarError::Unavailable)?
                .map_err(owner_error)?;
        context.check().map_err(owner_error)?;
        let result = owner_reply(response)?;
        self.authority.check_live(context).await?;
        context.check().map_err(owner_error)?;
        Ok(result)
    }
    pub async fn read_current(
        &self,
        context: &VerifiedParticipantRequest,
    ) -> Result<ObservedSelfProfile, AvatarError> {
        self.authority.check_live(context).await?;
        let (_, _, remaining) = owner_window(context)?;
        let message = proto::ReadSelfRequest {
            scope: Some(self.scope()),
            subject_id: context.key().user_id.uuid().to_string(),
        };
        let mut request =
            request_for(message, &context.key().user_id.uuid().to_string()).map_err(owner_error)?;
        request.set_timeout(remaining);
        let mut client = self.client.client.clone();
        let started = chrono::Utc::now().timestamp_micros();
        let reply = self.call(context, Box::pin(client.read(request))).await?;
        if reply.observed_at_unix_micros < started
            || reply.expires_unix_micros > context.deadline().expires_micros()
        {
            return Err(AvatarError::Unavailable);
        }
        let content = reply.content.ok_or(AvatarError::Unavailable)?;
        let version = version(content.version.as_ref().ok_or(AvatarError::Unavailable)?)?;
        let current = CurrentProfile {
            user_id: canonical(&content.user_id)?,
            display_name: content.display_name,
            bio: content.bio,
            observed_at_micros: reply.observed_at_unix_micros,
            expires_micros: reply.expires_unix_micros,
        };
        current.check(context)?;
        Ok(ObservedSelfProfile { current, version })
    }
    pub(crate) async fn prepare(
        &self,
        context: &VerifiedParticipantRequest,
        transaction: &ParticipantProfileTransaction,
        baseline: &ObservedSelfProfile,
    ) -> Result<PreparedSelfProfile, AvatarError> {
        self.authority.check_live(context).await?;
        let expected = transaction.expected_profile();
        if baseline.version != expected.version() {
            return Err(AvatarError::Conflict);
        }
        baseline.current.check(context)?;
        let expected_again = ExpectedProfileEffect::for_request(
            context,
            self.authority.scope().binding().digest(),
            baseline.version,
        )?;
        if expected_again.gid() != expected.gid() || expected_again.digest() != expected.digest() {
            return Err(AvatarError::Denied);
        }
        transaction
            .mark_prepare_attempt(context, self.authority.as_ref())
            .await?;
        let (_, _, remaining) = owner_window(context)?;
        let mut request = request_for(
            proto::PrepareSelfUpdateRequest {
                scope: Some(self.scope()),
                subject_id: context.key().user_id.uuid().to_string(),
                operation_id: context.request_id().to_string(),
                expected_version: Some(proto::ProfileVersion {
                    epoch: baseline.version.epoch.to_string(),
                    revision: baseline.version.revision.get(),
                }),
                raw_body: context.raw_body().to_vec(),
            },
            &context.key().user_id.uuid().to_string(),
        )
        .map_err(owner_error)?;
        request.set_timeout(remaining);
        let mut client = self.client.client.clone();
        let reply = self
            .call(context, Box::pin(client.prepare_update(request)))
            .await?;
        if reply.operation_id != context.request_id().to_string()
            || reply.prepared_gid != expected.gid()
            || reply.effect_binding_digest.as_slice() != expected.digest()
        {
            return Err(AvatarError::Denied);
        }
        let (native, instance) = native_profile(
            reply.transaction.as_ref().ok_or(AvatarError::Unavailable)?,
            expected.digest(),
        )?;
        let content = reply.proposed_content.ok_or(AvatarError::Unavailable)?;
        let update = ParticipantUpdate::parse(context.raw_body())?;
        let expected_name = match &update.display_name {
            Field::Unchanged => baseline.current.display_name.as_str(),
            Field::Set(name) => name.as_str(),
        };
        let expected_bio = match &update.bio {
            Field::Unchanged => baseline.current.bio.as_deref(),
            Field::Set(bio) => bio.as_deref().filter(|text| !text.is_empty()),
        };
        let changed = expected_name != baseline.current.display_name
            || expected_bio != baseline.current.bio.as_deref();
        let expected_version = CanonicalProfileVersion {
            epoch: baseline.version.epoch,
            revision: if changed {
                baseline
                    .version
                    .revision
                    .next()
                    .map_err(|_| AvatarError::Conflict)?
            } else {
                baseline.version.revision
            },
        };
        if canonical(&content.user_id)? != context.key().user_id.uuid()
            || content.display_name != expected_name
            || content.bio.as_deref() != expected_bio
            || version(content.version.as_ref().ok_or(AvatarError::Unavailable)?)?
                != expected_version
        {
            return Err(AvatarError::InvalidState);
        }
        Ok(PreparedSelfProfile {
            content,
            native,
            effect: expected.digest(),
            instance,
        })
    }
    pub(crate) async fn reconcile(
        &self,
        context: &VerifiedParticipantRequest,
        expected: &ExpectedProfileEffect,
        prepared: &PreparedSelfProfile,
    ) -> Result<ProfileTerminalReceipt, AvatarError> {
        self.authority.check_live(context).await?;
        let (_, _, remaining) = owner_window(context)?;
        let mut request = request_for(
            proto::SelfOperationRequest {
                scope: Some(self.scope()),
                operation_id: context.request_id().to_string(),
            },
            &context.key().user_id.uuid().to_string(),
        )
        .map_err(owner_error)?;
        request.set_timeout(remaining);
        let mut client = self.client.client.clone();
        let reply = self
            .call(context, Box::pin(client.reconcile(request)))
            .await?;
        terminal_reply(reply, context.request_id(), expected, Some(prepared))
    }
}
#[async_trait::async_trait]
impl CanonicalProfilePort for BoundCanonicalSelfProfile {
    async fn read(
        &self,
        context: &VerifiedParticipantRequest,
    ) -> Result<CurrentProfile, AvatarError> {
        self.read_current(context).await.map(|value| value.current)
    }
}
fn version(value: &proto::ProfileVersion) -> Result<CanonicalProfileVersion, AvatarError> {
    Ok(CanonicalProfileVersion {
        epoch: canonical(&value.epoch)?,
        revision: Revision::new(value.revision).map_err(|_| AvatarError::Unavailable)?,
    })
}
fn canonical(value: &str) -> Result<Uuid, AvatarError> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or(AvatarError::Unavailable)
}
fn native_profile(
    value: &crate::adapters::out::relationship_authority::guarded::proto::TransactionIdentity,
    effect: [u8; 32],
) -> Result<(NativeTransaction, Uuid), AvatarError> {
    let instance = canonical(&value.producer_instance_id)?;
    let decimal = |text: &str| text.parse::<u64>().ok().is_some_and(|number| number > 0);
    if value.producer != 3
        || value.effect_binding_digest.as_slice() != effect
        || !decimal(&value.system_identifier)
        || !decimal(&value.full_xid)
        || value.database_oid == 0
        || value.backend_pid <= 0
        || value.settlement_reference.is_empty()
        || value.settlement_reference.len() > 256
    {
        return Err(AvatarError::Unavailable);
    }
    Ok((
        NativeTransaction {
            system_identifier: value.system_identifier.clone(),
            database_oid: value.database_oid,
            full_xid: value.full_xid.clone(),
            backend_pid: value.backend_pid,
            backend_started: chrono::DateTime::from_timestamp_micros(
                value.backend_started_unix_micros,
            )
            .ok_or(AvatarError::Unavailable)?,
        },
        instance,
    ))
}
fn terminal_reply(
    reply: proto::SelfOperationResponse,
    operation: Uuid,
    expected: &ExpectedProfileEffect,
    prepared: Option<&PreparedSelfProfile>,
) -> Result<ProfileTerminalReceipt, AvatarError> {
    if reply.operation_id != operation.to_string()
        || reply.effect_binding_digest.as_slice() != expected.digest()
    {
        return Err(AvatarError::Denied);
    }
    let (native, instance) = native_profile(
        reply.transaction.as_ref().ok_or(AvatarError::Unavailable)?,
        expected.digest(),
    )?;
    if let Some(prepared) = prepared {
        if native != prepared.native
            || instance != prepared.instance
            || expected.digest() != prepared.effect
        {
            return Err(AvatarError::Denied);
        }
    }
    let outcome = match proto::SelfOperationStatus::try_from(reply.status)
        .map_err(|_| AvatarError::Unavailable)?
    {
        proto::SelfOperationStatus::Committed => ProfileTerminalOutcome::Committed,
        proto::SelfOperationStatus::Aborted => ProfileTerminalOutcome::Aborted,
        _ => return Err(AvatarError::Unavailable),
    };
    ProfileTerminalReceipt::from_owner_response(
        operation,
        expected,
        expected.gid(),
        expected.digest(),
        native,
        outcome,
    )
}
