//! Explicit owner RPC; no listener or worker activation.
use super::resource_authority::proto;
use chrono::{DateTime, Utc};
use platform::actor_delegation::fanout::{self, FanoutOwnerSigner, FanoutScope};
use platform::grpc_auth::{authorize_call, RpcAction, Workload};
use platform::grpc_method_policy::MethodPolicy;
use post_application::dto::timeline_fanout_dto::TimelineFanoutClaimDto;
use post_application::fanout_authority::{FanoutAuthorityRepositoryPort, FanoutPageLookupDto};
use post_domain::recommendation::TimelineFanoutJob;
use prost::Message;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub static FANOUT_OWNER_POLICY: MethodPolicy = MethodPolicy {
    service: "authority.v1.FanoutOwnerAuthorityService",
    workload: Some(Workload::Post),
    methods: &[("ResolveFanoutPage", &[Workload::Gateway])],
    actor_fields: &[],
};
#[derive(Clone)]
pub struct FanoutOwnerGrpcServer {
    repository: Arc<dyn FanoutAuthorityRepositoryPort>,
    signer: FanoutOwnerSigner,
}
impl FanoutOwnerGrpcServer {
    pub fn new(
        repository: Arc<dyn FanoutAuthorityRepositoryPort>,
        signer: FanoutOwnerSigner,
    ) -> Self {
        Self { repository, signer }
    }
}
pub fn message(request: &FanoutPageLookupDto) -> proto::ListFanoutFollowersRequest {
    let c = request.claim();
    let j = &c.job;
    proto::ListFanoutFollowersRequest {
        claim: Some(proto::FanoutClaim {
            job_id: j.job_id.to_string(),
            event_id: j.event_id.to_string(),
            post_id: j.post_id.to_string(),
            author_id: j.author_id.to_string(),
            lease_token: c.lease_token.to_string(),
            post_created_micros: j.post_created_at.timestamp_micros(),
            lease_expires_micros: c.lease_expires_at.timestamp_micros(),
            attempt: j.attempt_count,
            page_attempt: c.page_attempt_count,
            max_page_attempts: c.max_page_attempts,
            cursor: j.cursor.clone(),
        }),
        limit: request.page_limit(),
    }
}
pub fn frame(message: &impl Message) -> Vec<u8> {
    let body = message.encode_to_vec();
    [vec![0], (body.len() as u32).to_be_bytes().to_vec(), body].concat()
}
#[allow(clippy::result_large_err)]
pub fn lookup(body: &[u8]) -> Result<FanoutPageLookupDto, Status> {
    let invalid = || Status::invalid_argument("Invalid fanout binding");
    if body.len() < 5 || body.len() > 16 * 1024 || body[0] != 0 {
        return Err(invalid());
    }
    let m = proto::ListFanoutFollowersRequest::decode(&body[5..]).map_err(|_| invalid())?;
    if frame(&m) != body {
        return Err(invalid());
    }
    let c = m.claim.ok_or_else(invalid)?;
    let uuid = |s: &str| Uuid::parse_str(s).map_err(|_| invalid());
    let claim = TimelineFanoutClaimDto {
        job: TimelineFanoutJob {
            job_id: uuid(&c.job_id)?,
            event_id: uuid(&c.event_id)?,
            post_id: uuid(&c.post_id)?,
            author_id: uuid(&c.author_id)?,
            post_created_at: DateTime::from_timestamp_micros(c.post_created_micros)
                .ok_or_else(invalid)?,
            cursor: c.cursor,
            attempt_count: c.attempt,
        },
        lease_token: uuid(&c.lease_token)?,
        lease_expires_at: DateTime::from_timestamp_micros(c.lease_expires_micros)
            .ok_or_else(invalid)?,
        page_attempt_count: c.page_attempt,
        max_page_attempts: c.max_page_attempts,
    };
    FanoutPageLookupDto::new(claim, m.limit).map_err(|_| invalid())
}
#[allow(clippy::result_large_err)]
pub fn scope(request: &FanoutPageLookupDto) -> Result<FanoutScope, Status> {
    let c = request.claim();
    let j = &c.job;
    Ok(FanoutScope {
        job_id: j.job_id,
        event_id: j.event_id,
        post_id: j.post_id,
        author_id: j.author_id,
        lease_digest: FanoutScope::lease_digest(c.lease_token).map_err(fanout::status)?,
        post_created_micros: j.post_created_at.timestamp_micros(),
        lease_expires_micros: c.lease_expires_at.timestamp_micros(),
        attempt: j.attempt_count,
        page_attempt: c.page_attempt_count,
        max_page_attempts: c.max_page_attempts,
        cursor: j.cursor.clone(),
        limit: request.page_limit(),
    })
}
#[tonic::async_trait]
impl proto::fanout_owner_authority_service_server::FanoutOwnerAuthorityService
    for FanoutOwnerGrpcServer
{
    async fn resolve_fanout_page(
        &self,
        request: Request<proto::ResolveFanoutPageRequest>,
    ) -> Result<Response<proto::ResolveFanoutPageResponse>, Status> {
        if authorize_call(&request, RpcAction::Read)? != Workload::Gateway {
            return Err(Status::permission_denied(
                "Independent Gateway lookup required",
            ));
        }
        let request = request.into_inner();
        let challenge = Uuid::parse_str(&request.challenge)
            .ok()
            .filter(|id| !id.is_nil())
            .ok_or_else(|| Status::invalid_argument("Invalid owner challenge"))?;
        let query = lookup(&request.framed_body)?;
        let owned =
            self.repository
                .resolve_claimed_page(&query)
                .await
                .map_err(|error| match error {
                    post_application::fanout_authority::FanoutAuthorityErrorDto::Unavailable => {
                        Status::unavailable("Fanout owner unavailable")
                    }
                    _ => Status::permission_denied("Fanout owner denied"),
                })?;
        let evidence = self
            .signer
            .attest(
                scope(owned.page())?,
                &request.framed_body,
                challenge,
                owned.checked_at().timestamp_micros(),
                Utc::now().timestamp_micros(),
            )
            .map_err(fanout::status)?;
        Ok(Response::new(proto::ResolveFanoutPageResponse { evidence }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_table_matches_exact_proto_service_and_denies_other_workloads() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proto/authority/v1/authority.proto"
        ));
        let section = source
            .split("service FanoutOwnerAuthorityService")
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        assert_eq!(
            platform::grpc_method_policy::proto_rpc_names(section),
            vec!["ResolveFanoutPage"]
        );
        assert_eq!(
            FANOUT_OWNER_POLICY.method_names().collect::<Vec<_>>(),
            vec!["ResolveFanoutPage"]
        );
        for caller in Workload::ALL {
            assert_eq!(
                FANOUT_OWNER_POLICY.allows(
                    "/authority.v1.FanoutOwnerAuthorityService/ResolveFanoutPage",
                    caller
                ),
                caller == Workload::Gateway
            );
        }
    }
}
