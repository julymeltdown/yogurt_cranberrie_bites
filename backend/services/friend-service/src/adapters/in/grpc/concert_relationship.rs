//! Concert-only signed bounded owner read. This service never supplies a
//! revocation fence, event membership, provider consent or private profile data.
use crate::adapters::out::persistence::concert_relationship::PgCurrentRelationship;
use friend_application::concert_relationship::*;
use platform::{
    actor_delegation::{read_authority::VerifiedReadAuthority, VerifiedActor},
    grpc_auth::{VerifiedCaller, Workload},
    grpc_method_policy::MethodPolicy,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;
pub mod proto {
    tonic::include_proto!("relationship.v1");
}
pub const POLICY: MethodPolicy = MethodPolicy {
    service: "relationship.v1.FriendConcertRelationshipService",
    workload: Some(Workload::Friend),
    methods: &[("ReadCurrent", &[Workload::Concert])],
    actor_fields: &[("ReadCurrent", &[&[1]])],
};
pub struct RelationshipServer {
    owner: Arc<dyn CurrentRelationshipPort>,
}
impl RelationshipServer {
    pub fn new(owner: Arc<dyn CurrentRelationshipPort>) -> Self {
        Self { owner }
    }
    pub fn from_pool(pool: sqlx::PgPool) -> Self {
        Self::new(Arc::new(PgCurrentRelationship::new(pool)))
    }
}
fn canonical(value: &str) -> Result<Uuid, Status> {
    let id = Uuid::parse_str(value)
        .map_err(|_| Status::invalid_argument("Invalid relationship binding"))?;
    if id.is_nil() || id.to_string() != value {
        return Err(Status::invalid_argument("Invalid relationship binding"));
    }
    Ok(id)
}
fn status(error: ReadError) -> Status {
    match error {
        ReadError::InvalidInput => Status::invalid_argument("Invalid relationship input"),
        ReadError::Cancelled => Status::cancelled("Relationship read cancelled"),
        ReadError::DeadlineExceeded => {
            Status::deadline_exceeded("Relationship read deadline elapsed")
        }
        ReadError::Unavailable => Status::unavailable("Relationship owner unavailable"),
    }
}
struct CancelOnDrop(ReadCancellation);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
#[tonic::async_trait]
impl proto::friend_concert_relationship_service_server::FriendConcertRelationshipService
    for RelationshipServer
{
    async fn read_current(
        &self,
        request: Request<proto::RelationshipRead>,
    ) -> Result<Response<proto::FriendRelationshipObservation>, Status> {
        if !matches!(
            request.extensions().get::<VerifiedCaller>(),
            Some(VerifiedCaller(Workload::Concert))
        ) {
            return Err(Status::unauthenticated("Verified Concert caller required"));
        }
        let actor = request
            .extensions()
            .get::<VerifiedActor>()
            .ok_or_else(|| Status::unauthenticated("Verified relationship actor required"))?
            .0;
        let proof = request
            .extensions()
            .get::<VerifiedReadAuthority>()
            .copied()
            .ok_or_else(|| Status::unauthenticated("Bounded relationship authority required"))?;
        let message = request.get_ref();
        let actor_id = canonical(&message.actor_id)?;
        if actor_id != actor || actor_id != proof.actor() {
            return Err(Status::permission_denied("Relationship actor mismatch"));
        }
        let original = proof.deadline(ReadBudget::MAX_DURATION)?;
        let expires = message.expires_unix_micros;
        if expires > original.expires_micros() {
            return Err(Status::permission_denied(
                "Relationship expiry exceeds original authority",
            ));
        }
        let deadline =
            platform::actor_delegation::authority_deadline::AuthorityRpcDeadline::within_original(
                expires,
                original.stop().into_std(),
                ReadBudget::MAX_DURATION,
            )?;
        let expires_at = chrono::DateTime::from_timestamp_micros(expires)
            .ok_or_else(|| Status::invalid_argument("Invalid relationship expiry"))?;
        let input = RelationshipRead::new(
            canonical(&message.event_id)?,
            actor_id,
            canonical(&message.subject_id)?,
            canonical(&message.session_family_id)?,
            canonical(&message.request_id)?,
            Purpose::parse(&message.purpose).map_err(status)?,
            expires_at,
        )
        .map_err(status)?;
        input.check_at(chrono::Utc::now()).map_err(status)?;
        let cancel = ReadCancellation::default();
        let _cancel = CancelOnDrop(cancel.clone());
        let budget = ReadBudget::new(deadline.stop().into_std(), cancel).map_err(status)?;
        let result =
            tokio::time::timeout_at(deadline.stop(), self.owner.read_current(input, &budget)).await;
        deadline.remaining(ReadBudget::MAX_DURATION, expires)?;
        let facts = result
            .map_err(|_| Status::deadline_exceeded("Relationship read deadline elapsed"))?
            .map_err(status)?
            .validate(input, chrono::Utc::now())
            .map_err(status)?;
        let mut response = Response::new(proto::FriendRelationshipObservation {
            query: Some(message.clone()),
            observed_at_unix_micros: facts.observed_at.timestamp_micros(),
            valid_until_unix_micros: facts.valid_until.timestamp_micros(),
            owner_epoch: facts.owner_epoch.to_string(),
            revision: facts.revision,
            actor_blocks_subject: facts.actor_blocks_subject,
            subject_blocks_actor: facts.subject_blocks_actor,
            actor_deleted: facts.actor_deleted,
            subject_deleted: facts.subject_deleted,
            actor_follows_subject: facts.actor_follows_subject,
            subject_follows_actor: facts.subject_follows_actor,
        });
        response.metadata_mut().insert(
            "cache-control",
            tonic::metadata::MetadataValue::from_static("no-store"),
        );
        deadline.remaining(ReadBudget::MAX_DURATION, expires)?;
        Ok(response)
    }
}
