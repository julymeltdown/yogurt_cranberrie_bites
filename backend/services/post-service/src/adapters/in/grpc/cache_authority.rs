//! Explicit CACHE owner RPC and exact wire mapping. No listener activation.
use super::{fanout_authority::frame, resource_authority::proto};
use platform::{
    actor_delegation::cache::{self, CacheOwnerSigner, CacheScope},
    grpc_auth::{authorize_call, RpcAction, Workload},
    grpc_method_policy::MethodPolicy,
};
use post_application::cache_authority::*;
use prost::Message;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;
pub static CACHE_OWNER_POLICY: MethodPolicy = MethodPolicy {
    service: "authority.v1.CacheOwnerAuthorityService",
    workload: Some(Workload::Post),
    methods: &[("ResolveCachePage", &[Workload::Gateway])],
    actor_fields: &[],
};
#[derive(Clone)]
pub struct CacheOwnerGrpcServer {
    repository: Arc<dyn CacheAuthorityRepositoryPort>,
    signer: CacheOwnerSigner,
}
impl CacheOwnerGrpcServer {
    pub fn new(
        repository: Arc<dyn CacheAuthorityRepositoryPort>,
        signer: CacheOwnerSigner,
    ) -> Self {
        Self { repository, signer }
    }
}
pub fn reference(request: &CachePageLookupDto) -> anyhow::Result<CachePageReferenceDto> {
    let c = request.claim().claim();
    anyhow::ensure!(
        c.lease_expires_at.timestamp_subsec_nanos() % 1000 == 0,
        "Invalid cache timestamp precision"
    );
    Ok(CachePageReferenceDto {
        event_id: c.event.event_id,
        post_id: c.event.aggregate_id,
        author_id: request.author_id(),
        event_digest: CacheScope::digest(&cache_event_identity(&c.event)?),
        attempt_digest: CacheScope::digest(&cache_attempt_identity(request.owner())?),
        lease_token: c.lease_token,
        attempt_id: c.attempt_id,
        attempt_number: c.attempt_number,
        lease_expires_at: c.lease_expires_at,
        cursor: request.cursor().map(str::to_owned),
        limit: request.limit(),
    })
}
pub fn message(r: &CachePageReferenceDto) -> proto::ListCacheFollowersRequest {
    proto::ListCacheFollowersRequest {
        claim: Some(proto::CacheClaim {
            event_id: r.event_id.to_string(),
            post_id: r.post_id.to_string(),
            event_digest: r.event_digest.to_vec(),
            attempt_digest: r.attempt_digest.to_vec(),
            lease_token: r.lease_token.to_string(),
            attempt_id: r.attempt_id.to_string(),
            attempt: r.attempt_number,
            lease_expires_micros: r.lease_expires_at.timestamp_micros(),
        }),
        author_id: r.author_id.to_string(),
        cursor: r.cursor.clone(),
        limit: r.limit,
    }
}
#[allow(clippy::result_large_err)]
pub fn lookup(body: &[u8]) -> Result<CachePageReferenceDto, Status> {
    let invalid = || Status::invalid_argument("Invalid cache binding");
    if body.len() < 5 || body.len() > 16 * 1024 || body[0] != 0 {
        return Err(invalid());
    }
    let m = proto::ListCacheFollowersRequest::decode(&body[5..]).map_err(|_| invalid())?;
    if frame(&m) != body {
        return Err(invalid());
    }
    let c = m.claim.ok_or_else(invalid)?;
    let id = |s: &str| Uuid::parse_str(s).map_err(|_| invalid());
    let r = CachePageReferenceDto {
        event_id: id(&c.event_id)?,
        post_id: id(&c.post_id)?,
        author_id: id(&m.author_id)?,
        event_digest: c.event_digest.try_into().map_err(|_| invalid())?,
        attempt_digest: c.attempt_digest.try_into().map_err(|_| invalid())?,
        lease_token: id(&c.lease_token)?,
        attempt_id: id(&c.attempt_id)?,
        attempt_number: c.attempt,
        lease_expires_at: chrono::DateTime::from_timestamp_micros(c.lease_expires_micros)
            .ok_or_else(invalid)?,
        cursor: m.cursor,
        limit: m.limit,
    };
    r.validate().map_err(|_| invalid())?;
    Ok(r)
}
#[allow(clippy::result_large_err)]
pub fn scope(r: &CachePageReferenceDto) -> Result<CacheScope, Status> {
    Ok(CacheScope {
        event_id: r.event_id,
        post_id: r.post_id,
        author_id: r.author_id,
        attempt_id: r.attempt_id,
        event_digest: CacheScope::encode_digest(&r.event_digest),
        attempt_digest: CacheScope::encode_digest(&r.attempt_digest),
        lease_digest: CacheScope::lease_digest(r.lease_token).map_err(cache::status)?,
        lease_expires_micros: r.lease_expires_at.timestamp_micros(),
        attempt: r.attempt_number,
        cursor: r.cursor.clone(),
        limit: r.limit,
    })
}
#[tonic::async_trait]
impl proto::cache_owner_authority_service_server::CacheOwnerAuthorityService
    for CacheOwnerGrpcServer
{
    async fn resolve_cache_page(
        &self,
        request: Request<proto::ResolveCachePageRequest>,
    ) -> Result<Response<proto::ResolveCachePageResponse>, Status> {
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
        let owned = self
            .repository
            .resolve_cache_page(&lookup(&request.framed_body)?)
            .await
            .map_err(|e| match e {
                post_application::delivery_authority::DeliveryAuthorityErrorDto::Unavailable => {
                    Status::unavailable("Cache owner unavailable")
                }
                _ => Status::permission_denied("Cache owner denied"),
            })?;
        let evidence = self
            .signer
            .attest(
                scope(owned.reference())?,
                &request.framed_body,
                challenge,
                owned.checked_at().timestamp_micros(),
                chrono::Utc::now().timestamp_micros(),
            )
            .map_err(cache::status)?;
        Ok(Response::new(proto::ResolveCachePageResponse { evidence }))
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
            .split("service CacheOwnerAuthorityService")
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        assert_eq!(
            platform::grpc_method_policy::proto_rpc_names(section),
            vec!["ResolveCachePage"]
        );
        assert_eq!(
            CACHE_OWNER_POLICY.method_names().collect::<Vec<_>>(),
            vec!["ResolveCachePage"]
        );
        for caller in Workload::ALL {
            assert_eq!(
                CACHE_OWNER_POLICY.allows(
                    "/authority.v1.CacheOwnerAuthorityService/ResolveCachePage",
                    caller
                ),
                caller == Workload::Gateway
            );
        }
    }
}
