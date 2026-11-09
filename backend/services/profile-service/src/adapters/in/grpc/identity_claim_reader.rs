//! The internal claim read transport.
//!
//! Contract: [identity_claim_reader.proto](../../../../proto/profile/v1/identity_claim_reader.proto).
//!
//! This is the wire form of [`crate::identity::claim_read::ReadClaim`]. It is
//! registered on the profile service's own gRPC surface, not on the profile
//! product transport, because it is not a browser operation: the caller is the
//! concert service, it is not acting for a user, and the subject is named by the
//! request rather than by a session.
//!
//! The security shape is small and deliberate:
//!
//! * only the Concert workload may call it, and there is no actor delegation,
//!   because there is no actor: the concert service is asking a question about a
//!   profile, not performing an operation as a user;
//! * the request can name a profile and an instant and nothing else. It cannot
//!   ask for an arbitrary account, a claim value, or a decision;
//! * the reply carries the raw claim facts. It carries no current age, no birth
//!   date, no provider subject and no provider token.
use crate::identity::{ClaimRead, ClaimReadError, ReadClaim};
use chrono::{DateTime, Utc};
use platform::grpc_auth::Workload;
use platform::grpc_method_policy::MethodPolicy;
use tonic::{Request, Response, Status};

pub mod proto {
    tonic::include_proto!("profile.claims.v1");
}

/// Only the Concert workload may read a claim.
///
/// No actor field is declared: the request names the subject, and there is no
/// session to bind. That is safe because the reply is claim *facts*, which the
/// concert service needs in order to evaluate an audience it already owns; it is
/// not an operation performed on the subject's behalf.
pub const POLICY: MethodPolicy = MethodPolicy {
    service: "profile.claims.v1.IdentityClaimReader",
    workload: Some(Workload::Profile),
    methods: &[("ReadClaim", &[Workload::Concert])],
    actor_fields: &[],
};

#[derive(Clone)]
pub struct IdentityClaimReaderServer {
    reader: std::sync::Arc<ReadClaim>,
}

impl IdentityClaimReaderServer {
    pub fn new(reader: std::sync::Arc<ReadClaim>) -> Self {
        Self { reader }
    }
}

#[tonic::async_trait]
impl proto::identity_claim_reader_server::IdentityClaimReader
    for IdentityClaimReaderServer
{
    async fn read_claim(
        &self,
        request: Request<proto::ReadClaimRequest>,
    ) -> Result<Response<proto::ReadClaimReply>, Status> {
        let input = request.into_inner();
        // The evaluation instant is the caller's, and it is required: a read
        // without one would have to fall back to this service's clock, and then
        // two clocks could disagree about whether a claim is live.
        if input.evaluated_at_unix_micros <= 0 {
            return Err(Status::invalid_argument(
                "An evaluation instant is required",
            ));
        }
        let evaluated_at =
            DateTime::<Utc>::from_timestamp_micros(input.evaluated_at_unix_micros)
                .ok_or_else(|| Status::invalid_argument("Invalid evaluation instant"))?;
        let profile_id = input.profile_id.clone();
        let read = self
            .reader
            .read(&profile_id, evaluated_at)
            .await
            .map_err(claim_status)?;
        let claim = match read {
            ClaimRead::None => None,
            ClaimRead::Observed(observation) => Some(proto::ClaimObservation {
                status: observation.status.as_str().to_owned(),
                gender_bucket: observation.gender_bucket.as_str().to_owned(),
                age_at_verification: observation.age_at_verification.map(i32::from),
                next_birthday_at: observation
                    .next_birthday_at
                    .map(|date| date.format("%Y-%m-%d").to_string()),
                verifier: observation.verifier,
                source_revision: observation.source_revision,
                verified_at: crate::identity::claim_read::format_timestamp(
                    observation.verified_at,
                ),
                expires_at: crate::identity::claim_read::format_timestamp(
                    observation.expires_at,
                ),
                live: observation.live,
            }),
        };
        let mut response = Response::new(proto::ReadClaimReply {
            claim,
            profile_id,
        });
        // A claim read is per-subject and must never enter a shared cache.
        response.metadata_mut().insert(
            "cache-control",
            tonic::metadata::MetadataValue::from_static("no-store"),
        );
        Ok(response)
    }
}

fn claim_status(error: ClaimReadError) -> Status {
    match error {
        ClaimReadError::InvalidProfile => Status::invalid_argument("Invalid profile identity"),
        ClaimReadError::Unavailable => Status::unavailable("Identity claims are unavailable"),
    }
}
