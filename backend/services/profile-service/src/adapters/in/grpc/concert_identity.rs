//! Guarded current self content; no portable admission or account/session assertion.
use super::server::proto;
use crate::config::AppState;
use platform::actor_delegation::{read_authority::VerifiedReadAuthority, VerifiedActor};
use platform::edge::Edge;
use platform::grpc_auth::{VerifiedCaller, Workload};
use profile_application::concert_identity::{ConcertProfileError, ConcertProfileQuery};
use tonic::{Request, Response, Status};

pub(super) async fn read(
    state: &AppState,
    request: Request<proto::ReadConcertSelfProfileRequest>,
) -> Result<Response<proto::ReadConcertSelfProfileResponse>, Status> {
    match request.extensions().get::<VerifiedCaller>() {
        Some(VerifiedCaller(Workload::Gateway)) => {}
        Some(_) => return Err(Status::permission_denied("Profile caller denied")),
        None => return Err(Status::unauthenticated("Verified Profile caller required")),
    }
    let actor = request
        .extensions()
        .get::<VerifiedActor>()
        .ok_or_else(|| Status::unauthenticated("Verified Profile actor required"))?
        .0;
    let authority = request
        .extensions()
        .get::<VerifiedReadAuthority>()
        .copied()
        .ok_or_else(|| Status::unauthenticated("Current read authority required"))?;
    // This retained helper subtracts LOCAL_TIMEOUT_MARGIN from GatewayProfile's
    // edge budget. The verified context preserves time already spent in the guard.
    let ceiling = platform::grpc_deadline::local_deadline(Edge::GatewayProfile);
    let deadline = authority.deadline(ceiling)?;
    let query = ConcertProfileQuery::from_canonical(&request.get_ref().user_id)
        .map_err(|_| Status::invalid_argument("Invalid Profile subject"))?;
    if actor != query.user_id() || authority.actor() != query.user_id() {
        return Err(Status::permission_denied("Profile actor mismatch"));
    }
    deadline.remaining(ceiling, deadline.expires_micros())?;
    let result =
        tokio::time::timeout_at(deadline.stop(), state.concert_self_profile.execute(query)).await;
    // Tokio can finish polling a noncooperative owner after the timeout. Recheck
    // both clocks before inspecting even an owner error or building the response.
    deadline.remaining(ceiling, deadline.expires_micros())?;
    let profile = result
        .map_err(|_| Status::deadline_exceeded("Profile read deadline exceeded"))?
        .map_err(owner_status)?;
    let mut response = Response::new(proto::ReadConcertSelfProfileResponse {
        profile: Some(proto::ConcertSelfProfile {
            user_id: profile.user_id().to_string(),
            display_name: profile.display_name().to_owned(),
            bio: profile.bio().map(str::to_owned),
            observed_at_unix_micros: profile.observed_at().timestamp_micros(),
        }),
    });
    response.metadata_mut().insert(
        "cache-control",
        tonic::metadata::MetadataValue::from_static("no-store"),
    );
    deadline.remaining(ceiling, deadline.expires_micros())?;
    Ok(response)
}

fn owner_status(error: ConcertProfileError) -> Status {
    match error {
        ConcertProfileError::InvalidSubject => Status::invalid_argument("Invalid Profile subject"),
        ConcertProfileError::ContentUnavailable => Status::not_found("Profile content unavailable"),
        ConcertProfileError::DependencyUnavailable => {
            Status::unavailable("Profile owner unavailable")
        }
    }
}
