//! Dedicated signed Concert self content. No session/account or avatar assertion.
pub use super::server::proto;
use platform::{
    actor_delegation::{
        authority_deadline::AuthorityRpcDeadline, read_authority::VerifiedReadAuthority,
        VerifiedActor,
    },
    grpc_auth::{VerifiedCaller, Workload},
    grpc_method_policy::MethodPolicy,
};
use profile_application::concert_identity::{
    ConcertProfileError, ConcertProfileQuery, ConcertProfileRepository, ReadConcertSelfProfile,
};
use std::{sync::Arc, time::Duration};
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub const BUDGET: Duration = Duration::from_secs(2);
pub const METHOD: &str = "/profile.v1.ConcertWorldProfileService/ReadCurrent";
pub static WORLD_PROFILE_POLICY: MethodPolicy = MethodPolicy {
    service: "profile.v1.ConcertWorldProfileService",
    workload: Some(Workload::Profile),
    methods: &[("ReadCurrent", &[Workload::Concert])],
    actor_fields: &[("ReadCurrent", &[&[1]])],
};

#[derive(Clone)]
pub struct WorldProfileServer {
    reader: ReadConcertSelfProfile,
}
impl WorldProfileServer {
    pub fn new(repository: Arc<dyn ConcertProfileRepository>) -> Self {
        Self {
            reader: ReadConcertSelfProfile::new(repository),
        }
    }
    pub fn from_pool(pool: sqlx::PgPool) -> Self {
        Self::new(Arc::new(
            crate::adapters::out::persistence::concert_identity::PgConcertProfileRepository::new(
                pool,
            ),
        ))
    }
}
fn canonical(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or_else(|| Status::invalid_argument("Invalid world Profile scope"))
}
#[tonic::async_trait]
impl proto::concert_world_profile_service_server::ConcertWorldProfileService
    for WorldProfileServer
{
    async fn read_current(
        &self,
        request: Request<proto::WorldProfileReadRequest>,
    ) -> Result<Response<proto::WorldProfileObservation>, Status> {
        match request.extensions().get::<VerifiedCaller>() {
            Some(VerifiedCaller(Workload::Concert)) => {}
            Some(_) => return Err(Status::permission_denied("World Profile caller denied")),
            None => {
                return Err(Status::unauthenticated(
                    "Verified world Profile caller required",
                ))
            }
        }
        let actor = request
            .extensions()
            .get::<VerifiedActor>()
            .ok_or_else(|| Status::unauthenticated("Verified world Profile actor required"))?
            .0;
        let authority = request
            .extensions()
            .get::<VerifiedReadAuthority>()
            .copied()
            .ok_or_else(|| Status::unauthenticated("Current read authority required"))?;
        let original = authority.deadline(BUDGET)?;
        let m = request.into_inner();
        let bound_actor = canonical(&m.actor_id)?;
        let subject = canonical(&m.subject_id)?;
        canonical(&m.event_id)?;
        canonical(&m.session_family_id)?;
        canonical(&m.request_id)?;
        if actor != bound_actor || authority.actor() != actor || subject != actor {
            return Err(Status::permission_denied(
                "World Profile self scope required",
            ));
        }
        if m.purpose != proto::WorldProfilePurpose::SelfProfile as i32 {
            return Err(Status::permission_denied("World Profile purpose denied"));
        }
        let deadline = AuthorityRpcDeadline::within_original(
            m.expires_unix_micros.min(original.expires_micros()),
            original.stop().into_std(),
            BUDGET,
        )?;
        let query = ConcertProfileQuery::new(subject)
            .map_err(|_| Status::invalid_argument("Invalid world Profile subject"))?;
        let result = platform::call_deadline::scope(
            deadline.stop().into_std(),
            tokio::time::timeout_at(deadline.stop(), self.reader.execute(query)),
        )
        .await;
        deadline.remaining(BUDGET, deadline.expires_micros())?;
        let content = result
            .map_err(|_| Status::deadline_exceeded("World Profile deadline exceeded"))?
            .map_err(|error| match error {
                ConcertProfileError::InvalidSubject => {
                    Status::invalid_argument("Invalid world Profile subject")
                }
                ConcertProfileError::ContentUnavailable => {
                    Status::not_found("World Profile content unavailable")
                }
                ConcertProfileError::DependencyUnavailable => {
                    Status::unavailable("World Profile owner unavailable")
                }
            })?;
        let observed = content.observed_at().timestamp_micros();
        let now = chrono::Utc::now().timestamp_micros();
        if observed <= 0
            || observed > now
            || now.saturating_sub(observed) > BUDGET.as_micros() as i64
        {
            return Err(Status::unavailable("World Profile observation unavailable"));
        }
        let mut response = Response::new(proto::WorldProfileObservation {
            subject_id: content.user_id().to_string(),
            display_name: content.display_name().to_owned(),
            observed_at_unix_micros: observed,
        });
        response
            .metadata_mut()
            .insert("cache-control", "no-store".parse().unwrap());
        deadline.remaining(BUDGET, deadline.expires_micros())?;
        Ok(response)
    }
}
