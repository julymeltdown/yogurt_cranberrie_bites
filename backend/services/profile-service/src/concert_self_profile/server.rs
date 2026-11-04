//! Canonical Profile SELF producer. Wire signatures route calls; the actual
//! owning guards and database transactions grant and fence protected effects.
use super::{
    coordinator::Coordinator,
    fingerprint::effect_digest,
    guard::{canonical, wire_identity, Owners},
    identity::{prepared_gid, EffectConnection, NativeStatus},
    model::{Content, Error, Version},
    operation::Operation,
    patch::ParticipantPatch,
    read_reference::ReadReference,
    wire::{lease, proto},
};
use platform::{
    actor_delegation::{
        authority_deadline::AuthorityRpcDeadline, concert_self_profile::ConcertSelfProfileVerifier,
        participant_decision::ParticipantDecisionSigner, read_authority::VerifiedReadAuthority,
        ActorDelegationVerifier, VerifiedActor,
    },
    grpc_auth::{VerifiedCaller, Workload},
    grpc_method_policy::MethodPolicy,
};
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::time::Duration;
use tonic::{Request, Response, Status};
use uuid::Uuid;
pub const BUDGET: Duration = Duration::from_secs(5);
pub static SELF_POLICY: MethodPolicy = MethodPolicy {
    service: "profile.self.v1.ConcertSelfProfileService",
    workload: Some(Workload::Profile),
    methods: &[
        ("Read", &[Workload::Concert]),
        ("PrepareUpdate", &[Workload::Concert]),
        ("ReadOperation", &[Workload::Concert]),
        ("Reconcile", &[Workload::Concert]),
    ],
    actor_fields: &[
        ("Read", &[&[1, 1, 1], &[2]]),
        ("PrepareUpdate", &[&[1, 1, 1], &[2]]),
        ("ReadOperation", &[&[1, 1, 1]]),
        ("Reconcile", &[&[1, 1, 1]]),
    ],
};
#[derive(Clone)]
pub struct SelfProfileService {
    pub(super) pool: PgPool,
    owners: Owners,
    coordinator: Coordinator,
    instance: Uuid,
}
impl SelfProfileService {
    /// No guessed endpoints, parity decision seed or fallback producer. Missing
    /// dedicated SELF keys leaves this additive service unregistered.
    pub fn configured(
        pool: PgPool,
        gateway: &ActorDelegationVerifier,
        production: bool,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
    ) -> anyhow::Result<Option<(Self, ActorDelegationVerifier)>> {
        let Some(verifier) = ConcertSelfProfileVerifier::configured_from_env(gateway)? else {
            return Ok(None);
        };
        let required = |name: &str| {
            std::env::var(name)
                .ok()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow::anyhow!("{name} required for SELF producer"))
        };
        let auth = required("PROFILE_SELF_AUTH_GRPC_ENDPOINT")?;
        let friend = required("PROFILE_SELF_FRIEND_GRPC_ENDPOINT")?;
        let concert = required("PROFILE_SELF_CONCERT_GRPC_ENDPOINT")?;
        let token = platform::config::resolve_outbound_identity(production, Workload::Profile)?;
        let owners = Owners::connect(&auth, &friend, &token, tls)?;
        let coordinator = Coordinator::connect(
            &concert,
            &token,
            tls,
            ParticipantDecisionSigner::from_env(gateway)?,
        )?;
        Ok(Some((
            Self {
                pool,
                owners,
                coordinator,
                instance: Uuid::new_v4(),
            },
            verifier.into_inner(),
        )))
    }
    pub fn server(
        self,
    ) -> proto::concert_self_profile_service_server::ConcertSelfProfileServiceServer<Self> {
        proto::concert_self_profile_service_server::ConcertSelfProfileServiceServer::new(self)
            .max_decoding_message_size(131072)
            .max_encoding_message_size(32768)
    }
    async fn operation(
        &self,
        query: &proto::SelfOperationRequest,
        actor: Uuid,
        deadline: AuthorityRpcDeadline,
    ) -> Result<Operation, Status> {
        let id = canonical(&query.operation_id)?;
        let binding = query
            .scope
            .as_ref()
            .and_then(|s| s.binding.as_ref())
            .ok_or_else(denied)?;
        if canonical(&binding.actor_id)? != actor || canonical(&binding.request_id)? != id {
            return Err(denied());
        }
        let operation = Operation::load(&self.pool, id, deadline)
            .await
            .map_err(status)?
            .ok_or_else(|| Status::not_found("SELF operation unavailable"))?;
        // Recovery intentionally accepts an expired original binding, but it
        // cannot relabel it, create a new operation, or change its transaction.
        if operation.actor != actor
            || operation.binding_proto != binding.encode_to_vec()
            || operation.binding_digest != super::guard::binding_digest(binding)?
        {
            return Err(denied());
        }
        Ok(operation)
    }
    pub(super) async fn settle(
        &self,
        operation: &Operation,
        deadline: AuthorityRpcDeadline,
    ) -> Result<NativeStatus, Error> {
        super::reconcile::reconcile(&self.pool, &self.coordinator, operation, deadline).await
    }
}
#[tonic::async_trait]
impl proto::concert_self_profile_service_server::ConcertSelfProfileService for SelfProfileService {
    async fn read(
        &self,
        request: Request<proto::ReadSelfRequest>,
    ) -> Result<Response<proto::ReadSelfResponse>, Status> {
        let (actor, verified) = authenticate(&request)?;
        let message = request.into_inner();
        if canonical(&message.subject_id)? != actor {
            return Err(denied());
        }
        let scope = self
            .owners
            .inspect(
                message.scope.ok_or_else(denied)?,
                &message.subject_id,
                verified,
            )
            .await?;
        let binding = scope.binding();
        if !allowed_read(binding, scope.event()) {
            return Err(denied());
        }
        let deadline = scope.deadline();
        let work = async {
            let reference = Uuid::new_v4();
            let mut hash = Sha256::new();
            hash.update(b"mcr profile self read v1\0");
            hash.update(scope.digest());
            hash.update(reference.as_bytes());
            let mut effect =
                EffectConnection::open(&self.pool, self.instance, hash.finalize().into(), deadline)
                    .await
                    .map_err(status)?;
            ReadReference {
                id: reference,
                binding_proto: scope.binding().encode_to_vec(),
                binding_digest: scope.digest(),
                identity: effect.identity().clone(),
            }
            .register(&self.pool)
            .await
            .map_err(status)?;
            self.owners
                .track(&scope, effect.identity(), reference)
                .await?;
            let (content, observed) = super::store::read_in(&mut effect, actor, false)
                .await
                .map_err(status)?;
            self.owners.recheck(&scope).await?;
            effect.rollback().await.map_err(status)?;
            scope.check()?;
            Ok::<_, Status>(proto::ReadSelfResponse {
                content: Some(wire_content(content)),
                observed_at_unix_micros: observed.timestamp_micros(),
                expires_unix_micros: deadline.expires_micros(),
            })
        };
        let result = tokio::time::timeout_at(deadline.stop(), work)
            .await
            .map_err(|_| Status::deadline_exceeded("SELF read deadline exceeded"))??;
        scope.check()?;
        Ok(no_store(result))
    }
    async fn prepare_update(
        &self,
        request: Request<proto::PrepareSelfUpdateRequest>,
    ) -> Result<Response<proto::PreparedSelfUpdate>, Status> {
        let (actor, verified) = authenticate(&request)?;
        let message = request.into_inner();
        if canonical(&message.subject_id)? != actor {
            return Err(denied());
        }
        let scope = self
            .owners
            .inspect(
                message.scope.ok_or_else(denied)?,
                &message.subject_id,
                verified,
            )
            .await?;
        scope.assert_patch(&message.raw_body)?;
        let id = canonical(&message.operation_id)?;
        if id != scope.request() {
            return Err(denied());
        }
        let expected = message
            .expected_version
            .ok_or_else(|| Status::invalid_argument("SELF Profile version required"))?;
        let expected =
            Version::new(canonical(&expected.epoch)?, expected.revision).map_err(status)?;
        let parsed = ParticipantPatch::parse(&message.raw_body).map_err(status)?;
        let patch = parsed
            .profile()
            .ok_or_else(|| Status::invalid_argument("No selected SELF Profile fields"))?;
        let digest =
            effect_digest(scope.digest(), id, expected, &message.raw_body).map_err(status)?;
        let command_digest: [u8; 32] = Sha256::digest(&message.raw_body).into();
        let deadline = scope.deadline();
        let work = async {
            let mut effect = EffectConnection::open(&self.pool, self.instance, digest, deadline)
                .await
                .map_err(status)?;
            let operation = Operation {
                id,
                actor,
                event: scope.event(),
                family: scope.family(),
                binding_digest: scope.digest(),
                binding_proto: scope.binding().encode_to_vec(),
                command_digest,
                expected,
                identity: effect.identity().clone(),
            };
            operation
                .register(&self.pool, deadline)
                .await
                .map_err(status)?;
            // The actual transaction identity is durably registered with BOTH
            // canonical owners before any protected Profile content is read/written.
            let transaction = self.owners.track(&scope, effect.identity(), id).await?;
            let content = super::store::update_in(&mut effect, actor, expected, patch)
                .await
                .map_err(status)?;
            super::store::receipt_in(
                &mut effect,
                id,
                actor,
                scope.event(),
                command_digest,
                &content,
            )
            .await
            .map_err(status)?;
            self.owners.recheck(&scope).await?;
            effect.prepare(id).await.map_err(status)?;
            scope.check()?;
            Ok::<_, Status>(proto::PreparedSelfUpdate {
                operation_id: id.to_string(),
                prepared_gid: prepared_gid(id).map_err(status)?,
                transaction: Some(transaction),
                effect_binding_digest: digest.to_vec(),
                proposed_content: Some(wire_content(content)),
            })
        };
        let result = tokio::time::timeout_at(deadline.stop(), work)
            .await
            .map_err(|_| {
                Status::deadline_exceeded("SELF prepare outcome requires reconciliation")
            })??;
        scope.check()?;
        Ok(no_store(result))
    }
    async fn read_operation(
        &self,
        request: Request<proto::SelfOperationRequest>,
    ) -> Result<Response<proto::SelfOperationResponse>, Status> {
        let (actor, deadline) = authenticate(&request)?;
        let operation = self.operation(request.get_ref(), actor, deadline).await?;
        let state = operation
            .status(&self.pool, deadline)
            .await
            .map_err(status)?;
        Ok(no_store(operation_response(&operation, state)))
    }
    async fn reconcile(
        &self,
        request: Request<proto::SelfOperationRequest>,
    ) -> Result<Response<proto::SelfOperationResponse>, Status> {
        let (actor, deadline) = authenticate(&request)?;
        let operation = self.operation(request.get_ref(), actor, deadline).await?;
        let state = tokio::time::timeout_at(deadline.stop(), self.settle(&operation, deadline))
            .await
            .map_err(|_| Status::deadline_exceeded("SELF settlement unresolved"))?
            .map_err(status)?;
        Ok(no_store(operation_response(&operation, state)))
    }
}
pub(super) fn authenticate<T>(
    request: &Request<T>,
) -> Result<(Uuid, AuthorityRpcDeadline), Status> {
    if !matches!(
        request.extensions().get::<VerifiedCaller>(),
        Some(VerifiedCaller(Workload::Concert))
    ) {
        return Err(denied());
    }
    let actor = request
        .extensions()
        .get::<VerifiedActor>()
        .ok_or_else(denied)?
        .0;
    let authority = request
        .extensions()
        .get::<VerifiedReadAuthority>()
        .copied()
        .ok_or_else(denied)?;
    if actor.is_nil() || authority.actor() != actor {
        return Err(denied());
    }
    Ok((actor, authority.deadline(BUDGET)?))
}
pub(super) fn no_store<T>(message: T) -> Response<T> {
    let mut response = Response::new(message);
    response.metadata_mut().insert(
        "cache-control",
        "no-store".parse().expect("static cache policy"),
    );
    response
}
pub(super) fn operation_response(
    operation: &Operation,
    state: NativeStatus,
) -> proto::SelfOperationResponse {
    let state = match state {
        NativeStatus::InProgress => proto::SelfOperationStatus::InProgress,
        NativeStatus::Prepared => proto::SelfOperationStatus::Prepared,
        NativeStatus::Committed => proto::SelfOperationStatus::Committed,
        NativeStatus::Aborted => proto::SelfOperationStatus::Aborted,
        NativeStatus::Unknown => proto::SelfOperationStatus::Unknown,
    };
    proto::SelfOperationResponse {
        operation_id: operation.id.to_string(),
        status: state as i32,
        transaction: Some(wire_identity(&operation.identity, operation.id)),
        effect_binding_digest: operation.identity.effect_digest.to_vec(),
    }
}
fn wire_content(content: Content) -> proto::SelfContent {
    proto::SelfContent {
        user_id: content.user_id().to_string(),
        display_name: content.display_name().into(),
        bio: content.bio().map(str::to_owned),
        version: Some(proto::ProfileVersion {
            epoch: content.version().epoch().to_string(),
            revision: content.version().revision(),
        }),
    }
}
pub(super) fn status(error: Error) -> Status {
    match error {
        Error::Invalid => Status::invalid_argument("Invalid SELF Profile command"),
        Error::Missing => Status::not_found("Canonical SELF Profile content unavailable"),
        Error::Conflict => Status::aborted("SELF Profile version or operation conflict"),
        Error::Unavailable => Status::unavailable("SELF Profile owner unavailable"),
    }
}
fn denied() -> Status {
    Status::permission_denied("Canonical SELF Profile scope denied")
}
fn allowed_read(binding: &lease::GuardBinding, event: Uuid) -> bool {
    let base = format!("/api/concert/events/{event}/me");
    match (binding.operation.as_str(), binding.http_method.as_str()) {
        ("getConcertParticipant", "GET") | ("updateConcertParticipant", "PATCH") => {
            binding.http_target == base
        }
        ("setConcertAvatar", "PUT") => binding.http_target == format!("{base}/avatar"),
        _ => false,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_exact_c03_read_parent_triples_are_admitted() {
        let event = Uuid::new_v4();
        for (operation, method, suffix) in [
            ("getConcertParticipant", "GET", ""),
            ("updateConcertParticipant", "PATCH", ""),
            ("setConcertAvatar", "PUT", "/avatar"),
        ] {
            let mut binding = lease::GuardBinding {
                operation: operation.into(),
                http_method: method.into(),
                http_target: format!("/api/concert/events/{event}/me{suffix}"),
                ..Default::default()
            };
            assert!(allowed_read(&binding, event));
            assert!(!allowed_read(&binding, Uuid::new_v4()));
            for wrong in ["POST", "DELETE", "GET", "PATCH", "PUT"]
                .into_iter()
                .filter(|v| *v != method)
            {
                binding.http_method = wrong.into();
                assert!(!allowed_read(&binding, event));
            }
            binding.http_method = method.into();
            binding.http_target.push_str("?other=1");
            assert!(!allowed_read(&binding, event));
        }
    }
    #[test]
    fn self_policy_matches_owned_idl_and_denies_other_workloads() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proto/profile/v1/concert_self_profile.proto"
        ));
        let body = source
            .split_once("service ConcertSelfProfileService {")
            .unwrap()
            .1;
        let mut actual = platform::grpc_method_policy::proto_rpc_names(body);
        let mut expected = SELF_POLICY
            .method_names()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
        for method in expected {
            for caller in Workload::ALL {
                assert_eq!(
                    SELF_POLICY.allows(
                        &format!("/profile.self.v1.ConcertSelfProfileService/{method}"),
                        caller
                    ),
                    caller == Workload::Concert
                );
            }
        }
        assert!(!SELF_POLICY.allows(
            "/profile.self.v1.ConcertSelfProfileService/Commit",
            Workload::Concert
        ));
    }
}
