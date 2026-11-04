//! Dedicated signed arbitrary-subject public content; SELF_PROFILE never enters.
use crate::concert_public_profile::{PublicProfileRepository, ReadBinding, ReadError};
use platform::{
    actor_delegation::{
        authority_deadline::AuthorityRpcDeadline, read_authority::VerifiedReadAuthority,
        VerifiedActor,
    },
    grpc_auth::{VerifiedCaller, Workload},
    grpc_method_policy::MethodPolicy,
};
use std::{sync::Arc, time::Duration};
use tonic::{Request, Response, Status};
use uuid::Uuid;
pub mod proto {
    tonic::include_proto!("profile.concert_public.v1");
}
pub const BUDGET: Duration = Duration::from_secs(2);
pub const METHOD: &str = "/profile.concert_public.v1.ConcertPublicProfileService/ReadCurrent";
pub static PUBLIC_PROFILE_POLICY: MethodPolicy = MethodPolicy {
    service: "profile.concert_public.v1.ConcertPublicProfileService",
    workload: Some(Workload::Profile),
    methods: &[
        ("ReadCurrent", &[Workload::Concert]),
        ("Check", &[Workload::Concert]),
        ("Complete", &[Workload::Concert]),
    ],
    actor_fields: &[
        ("ReadCurrent", &[&[1]]),
        ("Check", &[&[1]]),
        ("Complete", &[&[1]]),
    ],
};
#[derive(Clone)]
pub struct PublicProfileServer {
    repository: Arc<dyn PublicProfileRepository>,
}
impl PublicProfileServer {
    pub fn new(repository: Arc<dyn PublicProfileRepository>) -> Self {
        Self { repository }
    }
    pub fn from_pool(pool: sqlx::PgPool) -> Self {
        Self::new(Arc::new(
            crate::concert_public_profile::persistence::PgPublicProfileRepository::new(pool),
        ))
    }
}
fn canonical(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value)
        .ok()
        .filter(|v| !v.is_nil() && v.to_string() == value)
        .ok_or_else(|| Status::invalid_argument("Invalid public Profile scope"))
}
#[tonic::async_trait]
impl proto::concert_public_profile_service_server::ConcertPublicProfileService
    for PublicProfileServer
{
    async fn read_current(
        &self,
        request: Request<proto::PublicProfileReadRequest>,
    ) -> Result<Response<proto::PublicProfileObservation>, Status> {
        match request.extensions().get::<VerifiedCaller>() {
            Some(VerifiedCaller(Workload::Concert)) => {}
            Some(_) => return Err(Status::permission_denied("Public Profile caller denied")),
            None => {
                return Err(Status::unauthenticated(
                    "Verified public Profile caller required",
                ))
            }
        }
        let actor = request
            .extensions()
            .get::<VerifiedActor>()
            .ok_or_else(|| Status::unauthenticated("Verified public Profile actor required"))?
            .0;
        let authority = request
            .extensions()
            .get::<VerifiedReadAuthority>()
            .copied()
            .ok_or_else(|| Status::unauthenticated("Current public Profile authority required"))?;
        let original = authority.deadline(BUDGET)?;
        let m = request.into_inner();
        if canonical(&m.actor_id)? != actor
            || authority.actor() != actor
            || m.purpose != proto::PublicProfilePurpose::PublicProfile as i32
        {
            return Err(Status::permission_denied("Public Profile scope denied"));
        }
        let subject = canonical(&m.subject_id)?;
        let binding = ReadBinding {
            actor,
            event: canonical(&m.event_id)?,
            subject,
            family: canonical(&m.session_family_id)?,
            request: canonical(&m.request_id)?,
        };
        binding
            .validate_completion(&m.child_id, &m.completion_secret)
            .map_err(|_| Status::invalid_argument("Invalid public Profile intent"))?;
        let deadline = AuthorityRpcDeadline::within_original(
            m.expires_unix_micros.min(original.expires_micros()),
            original.stop().into_std(),
            BUDGET,
        )?;
        let result = platform::call_deadline::scope(
            deadline.stop().into_std(),
            tokio::time::timeout_at(
                deadline.stop(),
                self.repository
                    .read_public(binding, &m.child_id, &m.completion_secret),
            ),
        )
        .await;
        deadline.remaining(BUDGET, deadline.expires_micros())?;
        let content = result
            .map_err(|_| Status::deadline_exceeded("Public Profile deadline exceeded"))?
            .map_err(|e| match e {
                ReadError::Hidden => Status::not_found("Public Profile unavailable"),
                ReadError::Unavailable => Status::unavailable("Public Profile owner unavailable"),
            })?;
        let held = content;
        let content = held.content;
        content
            .validate(subject)
            .map_err(|_| Status::unavailable("Public Profile content unavailable"))?;
        let mut result = Response::new(proto::PublicProfileObservation {
            subject_id: content.subject.to_string(),
            display_name: content.display_name,
            bio: content.bio,
            observed_at_unix_micros: content.observed_at.timestamp_micros(),
            actor_id: m.actor_id,
            event_id: m.event_id,
            session_family_id: m.session_family_id,
            request_id: m.request_id,
            child_id: held.child_id,
            completion_secret: held.completion_secret,
        });
        result
            .metadata_mut()
            .insert("cache-control", "no-store".parse().unwrap());
        deadline.remaining(BUDGET, deadline.expires_micros())?;
        Ok(result)
    }
    async fn check(
        &self,
        request: Request<proto::PublicProfileGuardRequest>,
    ) -> Result<Response<proto::PublicProfileGuardReply>, Status> {
        self.control(request, false).await
    }
    async fn complete(
        &self,
        request: Request<proto::PublicProfileGuardRequest>,
    ) -> Result<Response<proto::PublicProfileGuardReply>, Status> {
        self.control(request, true).await
    }
}

impl PublicProfileServer {
    async fn control(
        &self,
        request: Request<proto::PublicProfileGuardRequest>,
        complete: bool,
    ) -> Result<Response<proto::PublicProfileGuardReply>, Status> {
        match request.extensions().get::<VerifiedCaller>() {
            Some(VerifiedCaller(Workload::Concert)) => {}
            Some(_) => {
                return Err(Status::permission_denied(
                    "Public Profile guard caller denied",
                ))
            }
            None => {
                return Err(Status::unauthenticated(
                    "Verified public Profile caller required",
                ))
            }
        }
        let actor = request
            .extensions()
            .get::<VerifiedActor>()
            .ok_or_else(|| Status::unauthenticated("Verified actor required"))?
            .0;
        let authority = request
            .extensions()
            .get::<VerifiedReadAuthority>()
            .copied()
            .ok_or_else(|| Status::unauthenticated("Verified guard authority required"))?;
        let original = authority.deadline(BUDGET)?;
        let m = request.into_inner();
        if canonical(&m.actor_id)? != actor
            || authority.actor() != actor
            || m.purpose != proto::PublicProfilePurpose::PublicProfile as i32
        {
            return Err(Status::permission_denied(
                "Public Profile guard scope denied",
            ));
        }
        let binding = ReadBinding {
            actor,
            event: canonical(&m.event_id)?,
            subject: canonical(&m.subject_id)?,
            family: canonical(&m.session_family_id)?,
            request: canonical(&m.request_id)?,
        };
        // Completion may settle an expired read, but cannot return content or
        // create/extend an admission. It still needs its own signed RPC budget.
        let expires = if complete {
            original.expires_micros()
        } else {
            m.expires_unix_micros.min(original.expires_micros())
        };
        let deadline =
            AuthorityRpcDeadline::within_original(expires, original.stop().into_std(), BUDGET)?;
        if !complete && !m.completion_secret.is_empty() {
            return Err(Status::invalid_argument(
                "Completion secret not permitted on Check",
            ));
        }
        let result = platform::call_deadline::scope(
            deadline.stop().into_std(),
            tokio::time::timeout_at(deadline.stop(), async {
                if complete {
                    self.repository
                        .complete(binding, &m.child_id, &m.completion_secret)
                        .await
                } else {
                    self.repository.check(binding, &m.child_id).await
                }
            }),
        )
        .await;
        deadline.remaining(BUDGET, deadline.expires_micros())?;
        result
            .map_err(|_| Status::deadline_exceeded("Public Profile guard deadline exceeded"))?
            .map_err(|_| Status::unavailable("Public Profile guard unavailable"))?;
        let mut r = Response::new(proto::PublicProfileGuardReply { held: !complete });
        r.metadata_mut()
            .insert("cache-control", "no-store".parse().unwrap());
        Ok(r)
    }
}
