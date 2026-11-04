//! Live request orchestration. Staged until coherent root/SELF generated types
//! are consumed; no default producer or implicit startup registration.
use super::{
    auth::GrpcCanonicalAuth,
    coordinator::{ParticipantProfileJournal, ProfileTerminalOutcome},
    guarded::{operation_label, BoundParticipantAuthority},
    profile::{BoundCanonicalSelfProfile, GrpcCanonicalSelfProfile},
    update::ParticipantUpdate,
};
use crate::{
    adapters::out::relationship_authority::guarded::{
        CanonicalOwnerFence, GuardBinding, OwnerGuardScope,
    },
    participant::{
        context::VerifiedParticipantRequest,
        ports::{CanonicalAuthPort, CurrentAuth, CurrentProfile, ParticipantStore, StoreResult},
        proto, status, validate_response, Operation,
    },
    product_rpc::proto::ResponseDelivery,
};
use concert_application::avatar::{
    AvatarAuthority, AvatarEffectFence, AvatarError, AvatarGrant, AvatarOperation, AvatarSnapshot,
};
use platform::actor_delegation::response_scope::ResponseScopeToken;
use serde_json::{json, Value};
use std::sync::Arc;
use tonic::{Request, Response, Status};

/// The root owner implements this using its durable delivery registry. Publish
/// before protected work; only actual response-body EOF marks serialization and
/// Gateway completion can finish the held owner response. Drop is no proof.
#[async_trait::async_trait]
pub trait ParticipantDeliveryPublisher: Send + Sync {
    async fn publish(
        &self,
        scope: Arc<OwnerGuardScope>,
    ) -> Result<(ResponseDelivery, ResponseScopeToken), Status>;
}

#[derive(Clone)]
pub struct GuardedParticipantService {
    store: Arc<dyn ParticipantStore>,
    auth: GrpcCanonicalAuth,
    profile: GrpcCanonicalSelfProfile,
    fence: Arc<CanonicalOwnerFence>,
    journal: Arc<ParticipantProfileJournal>,
    delivery: Arc<dyn ParticipantDeliveryPublisher>,
}
impl GuardedParticipantService {
    pub fn new(
        store: Arc<dyn ParticipantStore>,
        auth: GrpcCanonicalAuth,
        profile: GrpcCanonicalSelfProfile,
        fence: Arc<CanonicalOwnerFence>,
        journal: Arc<ParticipantProfileJournal>,
        delivery: Arc<dyn ParticipantDeliveryPublisher>,
    ) -> Self {
        Self {
            store,
            auth,
            profile,
            fence,
            journal,
            delivery,
        }
    }
    pub fn server(
        self,
    ) -> proto::concert_participant_service_server::ConcertParticipantServiceServer<Self> {
        proto::concert_participant_service_server::ConcertParticipantServiceServer::new(self)
            .max_decoding_message_size(16 * 1024)
            .max_encoding_message_size(16 * 1024)
    }
    async fn handle(
        &self,
        request: Request<proto::ParticipantRequest>,
        operation: Operation,
    ) -> Result<Response<proto::ParticipantResponse>, Status> {
        let context = VerifiedParticipantRequest::verify(&request, operation)?;
        if context.release_digest() != self.store.release_digest() {
            return Err(status(AvatarError::Unavailable));
        }
        let stop = context.deadline().stop();
        platform::call_deadline::scope(stop.into_std(), async {
            tokio::time::timeout_at(stop, self.respond(&context))
                .await
                .map_err(|_| Status::deadline_exceeded("Participant deadline elapsed"))?
        })
        .await
    }
    async fn respond(
        &self,
        context: &VerifiedParticipantRequest,
    ) -> Result<Response<proto::ParticipantResponse>, Status> {
        let auth = self.auth.read(context).await.map_err(status)?;
        auth.check(context).map_err(status)?;
        let binding = GuardBinding::from_parent(
            context.parent_authority()?,
            context.key().user_id.uuid(),
            context.key().event_id.uuid(),
            context.session_family_id(),
            context.request_id(),
            operation_label(context.operation()),
            context.http_method(),
            context.http_target(),
            context.body_sha256(),
            context.deadline(),
        )?;
        let scope = self.fence.acquire(binding).await?;
        context.check()?;
        let authority = Arc::new(
            BoundParticipantAuthority::new(scope.clone(), context, self.journal.clone())
                .map_err(status)?,
        );
        // An error/cancellation after acquisition retains the registered scope
        // for independent native settlement; no handler cleanup releases it.
        let producer = scope.producer_scope();
        let binding_digest = scope.binding().digest();
        let (delivery, token) = self.delivery.publish(scope).await?;
        if delivery.instance_id != producer.instance_id
            || delivery.scope_id != producer.scope_id
            || delivery.binding_digest.as_slice() != binding_digest
            || delivery.completion_secret.len() != 32
            || delivery.completion_secret.iter().all(|byte| *byte == 0)
        {
            return Err(status(AvatarError::Unavailable));
        }
        context.check()?;
        let profile = self.profile.bind(authority.clone());
        let document = self
            .produce(context, &auth, authority.as_ref(), &profile)
            .await
            .map_err(status)?;
        validate_response(&document, context.operation())?;
        authority.check_live(context).await.map_err(status)?;
        auth.check(context).map_err(status)?;
        let bytes = serde_json::to_vec(&document).map_err(|_| status(AvatarError::InvalidState))?;
        context.check()?;
        let mut response = Response::new(proto::ParticipantResponse {
            response_json: bytes,
            delivery: Some(delivery),
        });
        response.extensions_mut().insert(token);
        response
            .metadata_mut()
            .insert("cache-control", "no-store".parse().unwrap());
        Ok(response)
    }
    async fn produce(
        &self,
        context: &VerifiedParticipantRequest,
        auth: &CurrentAuth,
        authority: &BoundParticipantAuthority,
        profile: &BoundCanonicalSelfProfile,
    ) -> Result<Value, AvatarError> {
        auth.check(context)?;
        authority.check_live(context).await?;
        self.journal.ensure_visible(context, None).await?;
        match context.operation() {
            Operation::GetParticipant => {
                self.current_document(context, auth, authority, profile)
                    .await
            }
            Operation::SetAvatar => {
                let baseline = profile.read_current(context).await?;
                let local = LocalAuthority {
                    context,
                    auth,
                    authority,
                    operation: AvatarOperation::SetAvatar,
                };
                baseline.current.check(context)?;
                let result = self
                    .store
                    .execute(
                        context.key(),
                        context.operation(),
                        context.raw_body(),
                        &local,
                    )
                    .await?;
                auth.check(context)?;
                authority.check_live(context).await?;
                let fresh = profile.read_current(context).await?;
                fresh.current.check(context)?;
                self.journal.ensure_visible(context, None).await?;
                let StoreResult::Avatar(value) = result else {
                    return Err(AvatarError::InvalidState);
                };
                let command = concert_domain::avatar::SetAvatar::parse(context.raw_body())
                    .map_err(|_| AvatarError::InvalidInput)?;
                if value["config"] != concert_domain::avatar::config_value(&command.selection)
                    || value["revision"].as_i64()
                        != command.expected.next().ok().map(|revision| revision.get())
                {
                    return Err(AvatarError::InvalidState);
                }
                Ok(value)
            }
            Operation::UpdatePreferences => {
                let update = ParticipantUpdate::parse(context.raw_body())?;
                let baseline = profile.read_current(context).await?;
                if update.changes_profile() {
                    let mut transaction = self
                        .journal
                        .begin(
                            context,
                            authority.scope().binding().digest(),
                            baseline.version,
                            authority,
                        )
                        .await?;
                    transaction.lock_participant(context, authority).await?;
                    let expected = transaction.expected_profile().clone();
                    let prepared = profile.prepare(context, &transaction, &baseline).await?;
                    let next = transaction
                        .commit_after_prepared(
                            context,
                            authority,
                            expected.gid(),
                            expected.digest(),
                        )
                        .await?;
                    let terminal = profile.reconcile(context, &expected, &prepared).await?;
                    if terminal.outcome() != ProfileTerminalOutcome::Committed {
                        return Err(AvatarError::Unavailable);
                    }
                    self.journal.record_terminal(&terminal).await?;
                    let document = self
                        .current_document(context, auth, authority, profile)
                        .await?;
                    if document["revision"].as_i64() != Some(next.get()) {
                        return Err(AvatarError::Unavailable);
                    }
                    Ok(document)
                } else {
                    let local = LocalAuthority {
                        context,
                        auth,
                        authority,
                        operation: AvatarOperation::SetPreferences,
                    };
                    baseline.current.check(context)?;
                    let result = self
                        .store
                        .execute(
                            context.key(),
                            context.operation(),
                            context.raw_body(),
                            &local,
                        )
                        .await?;
                    let StoreResult::Snapshot(snapshot) = result else {
                        return Err(AvatarError::InvalidState);
                    };
                    let (expected, preferences) =
                        crate::adapters::out::repository::avatar::parse_preferences(
                            context.raw_body(),
                        )?;
                    if Some(snapshot.participant.revision) != expected.next().ok()
                        || snapshot.participant.preferences != preferences
                    {
                        return Err(AvatarError::InvalidState);
                    }
                    self.join_snapshot(context, auth, authority, profile, snapshot)
                        .await
                }
            }
        }
    }
    async fn current_document(
        &self,
        context: &VerifiedParticipantRequest,
        auth: &CurrentAuth,
        authority: &BoundParticipantAuthority,
        profile: &BoundCanonicalSelfProfile,
    ) -> Result<Value, AvatarError> {
        // Local waits precede the canonical content read. A response cannot join
        // an old local revision with a newer coordinated Profile commit.
        let local = LocalAuthority {
            context,
            auth,
            authority,
            operation: AvatarOperation::Read,
        };
        let result = self
            .store
            .execute(context.key(), Operation::GetParticipant, &[], &local)
            .await?;
        let StoreResult::Snapshot(snapshot) = result else {
            return Err(AvatarError::InvalidState);
        };
        self.join_snapshot(context, auth, authority, profile, snapshot)
            .await
    }
    async fn join_snapshot(
        &self,
        context: &VerifiedParticipantRequest,
        auth: &CurrentAuth,
        authority: &BoundParticipantAuthority,
        profile: &BoundCanonicalSelfProfile,
        snapshot: AvatarSnapshot,
    ) -> Result<Value, AvatarError> {
        if snapshot.participant.key != context.key() || snapshot.avatar.key != context.key() {
            return Err(AvatarError::InvalidState);
        }
        auth.check(context)?;
        authority.check_live(context).await?;
        self.journal
            .ensure_visible(context, Some(snapshot.participant.revision))
            .await?;
        let fresh = profile.read_current(context).await?;
        fresh.current.check(context)?;
        self.journal
            .ensure_visible(context, Some(snapshot.participant.revision))
            .await?;
        authority.check_live(context).await?;
        auth.check(context)?;
        Ok(document(snapshot, fresh.current))
    }
}
struct LocalAuthority<'a> {
    context: &'a VerifiedParticipantRequest,
    auth: &'a CurrentAuth,
    authority: &'a BoundParticipantAuthority,
    operation: AvatarOperation,
}
impl AvatarAuthority for LocalAuthority<'_> {
    fn check(&self) -> Result<AvatarGrant, AvatarError> {
        self.auth.check(self.context)?;
        self.authority.check_bound(self.context)?;
        if self.operation != AvatarOperation::Read
            && self.operation != self.context.operation().avatar_operation()
        {
            return Err(AvatarError::Denied);
        }
        Ok(AvatarGrant {
            key: self.context.key(),
            operation: self.operation,
            release_digest: self.context.release_digest().into(),
        })
    }
    fn effect_fence(&self) -> Option<&dyn AvatarEffectFence> {
        Some(self.authority)
    }
}
fn document(snapshot: AvatarSnapshot, profile: CurrentProfile) -> Value {
    let participant = snapshot.participant;
    let avatar = crate::adapters::out::repository::avatar::avatar_state(&snapshot.avatar);
    json!({"event_id":participant.key.event_id.uuid().to_string(),"user_id":participant.key.user_id.uuid().to_string(),"revision":participant.revision.get(),
        "profile":{"user_id":profile.user_id.to_string(),"display_name":profile.display_name,"bio":profile.bio,"avatar":avatar},
        "preferences":{"theme":participant.preferences.theme.as_str(),"low_power":participant.preferences.low_power,"reduced_motion":participant.preferences.reduced_motion,"sound_enabled":participant.preferences.sound_enabled},"age_status":"UNKNOWN"})
}
#[tonic::async_trait]
impl proto::concert_participant_service_server::ConcertParticipantService
    for GuardedParticipantService
{
    async fn get_participant(
        &self,
        request: Request<proto::ParticipantRequest>,
    ) -> Result<Response<proto::ParticipantResponse>, Status> {
        self.handle(request, Operation::GetParticipant)
            .await
            .map_err(crate::participant::normalize_status)
    }
    async fn set_avatar(
        &self,
        request: Request<proto::ParticipantRequest>,
    ) -> Result<Response<proto::ParticipantResponse>, Status> {
        self.handle(request, Operation::SetAvatar)
            .await
            .map_err(crate::participant::normalize_status)
    }
    async fn update_preferences(
        &self,
        request: Request<proto::ParticipantRequest>,
    ) -> Result<Response<proto::ParticipantResponse>, Status> {
        self.handle(request, Operation::UpdatePreferences)
            .await
            .map_err(crate::participant::normalize_status)
    }
}
