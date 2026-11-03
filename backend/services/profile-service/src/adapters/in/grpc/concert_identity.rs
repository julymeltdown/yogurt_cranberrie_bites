//! Integrator-owned interface prerequisite; replaced by the assigned RPC worker.
//! This candidate must not be published before the guarded implementation passes review.
use super::server::proto;
use crate::config::AppState;
use tonic::{Request, Response, Status};

pub(super) async fn read(
    _state: &AppState,
    _request: Request<proto::ReadConcertSelfProfileRequest>,
) -> Result<Response<proto::ReadConcertSelfProfileResponse>, Status> {
    Err(Status::unavailable("Profile owner unavailable"))
}
