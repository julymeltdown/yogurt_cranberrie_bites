//! Post-only deferred subject admission against the actual retained root scope.
use super::canonical_uuid;
use crate::adapters::out::{
    public_profile::PublicProfileReader,
    relationship_authority::guarded::{proto, ScopeRegistry},
};
use concert_domain::feed::FeedError;
use platform::{
    actor_delegation::{
        exchange::VerifiedParentAuthority, read_authority::VerifiedReadAuthority, VerifiedActor,
    },
    grpc_auth::{VerifiedCaller, Workload},
    grpc_method_policy::MethodPolicy,
};
use prost::Message;
use std::{sync::Arc, time::Duration};
use tonic::{Request, Response, Status};

pub const POLICY: MethodPolicy = MethodPolicy {
    service: "authority.lease.v1.ConcertRootReadService",
    workload: Some(Workload::Concert),
    methods: &[("CheckSubject", &[Workload::Post])],
    actor_fields: &[("CheckSubject", &[&[1, 3, 1]])],
};
pub struct ConcertRootReadService {
    registry: Arc<ScopeRegistry>,
    profiles: Arc<PublicProfileReader>,
    capacity: Arc<tokio::sync::Semaphore>,
}
impl ConcertRootReadService {
    pub fn new(registry: Arc<ScopeRegistry>, profiles: Arc<PublicProfileReader>) -> Self {
        Self {
            registry,
            profiles,
            capacity: Arc::new(tokio::sync::Semaphore::new(16)),
        }
    }
    /// Mount only inside the dedicated PostConcertRootReadVerifier actor guard
    /// and the actual mTLS/workload policy. Construction alone registers nothing.
    pub fn server(
        self,
    ) -> proto::concert_root_read_service_server::ConcertRootReadServiceServer<Self> {
        proto::concert_root_read_service_server::ConcertRootReadServiceServer::new(self)
            .max_decoding_message_size(16 * 1024)
            .max_encoding_message_size(16 * 1024)
    }
}
#[tonic::async_trait]
impl proto::concert_root_read_service_server::ConcertRootReadService for ConcertRootReadService {
    async fn check_subject(
        &self,
        request: Request<proto::CheckSubjectRequest>,
    ) -> Result<Response<proto::CheckSubjectResponse>, Status> {
        if !matches!(
            request.extensions().get::<VerifiedCaller>(),
            Some(VerifiedCaller(Workload::Post))
        ) {
            return Err(Status::unauthenticated("Verified Post caller required"));
        }
        let actor = request
            .extensions()
            .get::<VerifiedActor>()
            .ok_or_else(|| Status::unauthenticated("Verified callback actor required"))?
            .0;
        let proof = request
            .extensions()
            .get::<VerifiedReadAuthority>()
            .ok_or_else(|| Status::unauthenticated("Original callback deadline required"))?;
        let deadline = proof.deadline(Duration::from_secs(2))?;
        let parent = request
            .extensions()
            .get::<VerifiedParentAuthority>()
            .ok_or_else(|| Status::unauthenticated("Verified callback frame required"))?;
        if parent.actor() != actor
            || proof.actor() != actor
            || parent.request().method != "/authority.lease.v1.ConcertRootReadService/CheckSubject"
            || proto::CheckSubjectRequest::decode(
                parent
                    .message()
                    .map_err(|_| Status::permission_denied("Invalid callback frame"))?,
            )
            .map_err(|_| Status::permission_denied("Invalid callback message"))?
                != *request.get_ref()
        {
            return Err(Status::permission_denied("Callback actor/frame mismatch"));
        }
        let input = request.into_inner();
        let root = input
            .root
            .as_ref()
            .ok_or_else(|| Status::permission_denied("Root read capability required"))?;
        let subject_id = canonical_uuid(&input.subject_id)?;
        // This permit prevents root finish from racing an admitted callback.
        // Its lifetime covers BOTH owner enrollment and actual local PREPARE.
        let permit = self.registry.lookup_root_read(root)?;
        let scope = permit.scope();
        if scope.binding().actor() != actor {
            return Err(Status::permission_denied("Callback root actor mismatch"));
        }
        let _capacity = self
            .capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| Status::unavailable("Root read callback capacity"))?;
        let work = async {
            // Actor admission is never a filterable subject absence. A missing
            // actor/event invalidates this entire producer operation.
            let actor_subject = scope
                .public_profile_subject(actor)
                .await
                .map_err(actor_failure)?;
            self.profiles
                .local_subject(&actor_subject)
                .await
                .map_err(|_| Status::unavailable("Root actor local eligibility unavailable"))?;
            let subject = scope.public_profile_subject(subject_id).await?;
            let local =
                self.profiles
                    .local_subject(&subject)
                    .await
                    .map_err(|error| match error {
                        FeedError::ResourceUnavailable if subject_id != actor => {
                            Status::not_found("Subject unavailable")
                        }
                        _ => Status::unavailable("Subject policy unavailable"),
                    })?;
            subject.check().await?;
            let vector = subject.revision_vector();
            Ok::<_, Status>(proto::CheckSubjectResponse {
                scope: Some(scope.producer_scope()),
                subject_id: subject_id.to_string(),
                event_revision: local.event_revision,
                pair_revision: local.pair_revision,
                expires_unix_micros: local
                    .valid_until
                    .timestamp_micros()
                    .min(scope.binding().deadline().expires_micros()),
                auth_revision: Some(proto::OwnerRevision {
                    epoch: vector.auth.epoch.to_string(),
                    revision: vector.auth.revision,
                }),
                friend_revision: Some(proto::OwnerRevision {
                    epoch: vector.friend.epoch.to_string(),
                    revision: vector.friend.revision,
                }),
            })
        };
        let body = platform::call_deadline::scope(
            deadline.stop().into_std(),
            tokio::time::timeout_at(deadline.stop(), scope.run(work)),
        )
        .await
        .map_err(|_| Status::deadline_exceeded("Root read callback deadline"))??;
        drop(permit);
        let mut response = Response::new(body);
        response.metadata_mut().insert(
            "cache-control",
            "no-store".parse().expect("static metadata"),
        );
        Ok(response)
    }
}
fn actor_failure(status: Status) -> Status {
    if status.code() == tonic::Code::NotFound {
        Status::permission_denied("Root actor unavailable")
    } else {
        status
    }
}
