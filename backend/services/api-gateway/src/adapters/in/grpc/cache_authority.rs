//! Explicitly composed system issuer and independent pre-dispatch refresh.
use super::authority_exchange::proto;
use platform::actor_delegation::cache::{self, CacheIssuer, CacheScope, CacheVerifier, METHOD};
use platform::grpc_auth::{authorize_call, RpcAction, Workload};
use platform::grpc_method_policy::MethodPolicy;
use prost::Message;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub static CACHE_GRANT_POLICY: MethodPolicy = MethodPolicy {
    service: "authority.v1.CacheGrantService",
    workload: Some(Workload::Gateway),
    methods: &[
        ("IssueCacheGrant", &[Workload::Post]),
        ("ValidateCacheGrant", &[Workload::Friend]),
    ],
    actor_fields: &[],
};
#[async_trait::async_trait]
pub trait CacheOwnerLookup: Send + Sync {
    async fn resolve(
        &self,
        body: &[u8],
        challenge: Uuid,
        deadline: platform::actor_delegation::authority_deadline::AuthorityRpcDeadline,
    ) -> Result<String, Status>;
}
#[derive(Clone)]
pub struct CacheGrantGrpcServer {
    issuer: CacheIssuer,
    verifier: CacheVerifier,
    owner: Arc<dyn CacheOwnerLookup>,
}
impl CacheGrantGrpcServer {
    pub fn new(
        issuer: CacheIssuer,
        verifier: CacheVerifier,
        owner: Arc<dyn CacheOwnerLookup>,
    ) -> Self {
        Self {
            issuer,
            verifier,
            owner,
        }
    }
}
#[allow(clippy::result_large_err)]
fn scope(body: &[u8]) -> Result<CacheScope, Status> {
    let invalid = || Status::invalid_argument("Invalid cache request");
    if body.len() < 5 || body.len() > 16 * 1024 || body[0] != 0 {
        return Err(invalid());
    }
    let m = proto::ListCacheFollowersRequest::decode(&body[5..]).map_err(|_| invalid())?;
    let encoded = m.encode_to_vec();
    if body[1..5] != (encoded.len() as u32).to_be_bytes() || encoded != body[5..] {
        return Err(invalid());
    }
    let c = m.claim.ok_or_else(invalid)?;
    let uuid = |s: &str| Uuid::parse_str(s).map_err(|_| invalid());
    let scope = CacheScope {
        event_id: uuid(&c.event_id)?,
        post_id: uuid(&c.post_id)?,
        author_id: uuid(&m.author_id)?,
        attempt_id: uuid(&c.attempt_id)?,
        event_digest: CacheScope::encode_digest(&c.event_digest.try_into().map_err(|_| invalid())?),
        attempt_digest: CacheScope::encode_digest(
            &c.attempt_digest.try_into().map_err(|_| invalid())?,
        ),
        lease_digest: CacheScope::lease_digest(uuid(&c.lease_token)?).map_err(cache::status)?,
        lease_expires_micros: c.lease_expires_micros,
        attempt: c.attempt,
        cursor: m.cursor,
        limit: m.limit,
    };
    scope.validate().map_err(cache::status)?;
    if chrono::DateTime::from_timestamp_micros(scope.lease_expires_micros).is_none() {
        return Err(invalid());
    }
    Ok(scope)
}
#[tonic::async_trait]
impl proto::cache_grant_service_server::CacheGrantService for CacheGrantGrpcServer {
    async fn issue_cache_grant(
        &self,
        request: Request<proto::IssueCacheGrantRequest>,
    ) -> Result<Response<proto::IssueCacheGrantResponse>, Status> {
        let caller = authorize_call(&request, RpcAction::Read)?;
        if caller != Workload::Post {
            return Err(Status::permission_denied("Post presenter required"));
        }
        let request = request.into_inner();
        let deadline_micros = request.deadline_micros;
        let body = request.framed_body;
        let prepared = self
            .issuer
            .prepare_until(
                caller,
                scope(&body)?,
                body,
                chrono::Utc::now().timestamp_micros(),
                deadline_micros,
            )
            .map_err(cache::status)?;
        let deadline = prepared.deadline().map_err(cache::status)?;
        let evidence = tokio::time::timeout_at(
            deadline.stop(),
            self.owner
                .resolve(prepared.body(), prepared.challenge(), deadline),
        )
        .await
        .map_err(|_| Status::deadline_exceeded("Authority deadline expired"))??;
        deadline.remaining(
            std::time::Duration::from_secs(60),
            deadline.expires_micros(),
        )?;
        let issued = self
            .issuer
            .finish(prepared, &evidence, chrono::Utc::now().timestamp_micros())
            .map_err(cache::status)?;
        Ok(Response::new(proto::IssueCacheGrantResponse {
            token: issued.token,
            expires_micros: issued.expires_micros,
        }))
    }
    async fn validate_cache_grant(
        &self,
        request: Request<proto::ValidateCacheGrantRequest>,
    ) -> Result<Response<proto::ValidateCacheGrantResponse>, Status> {
        if authorize_call(&request, RpcAction::Read)? != Workload::Friend {
            return Err(Status::permission_denied(
                "Friend admission refresh required",
            ));
        }
        let request = request.into_inner();
        let now = chrono::Utc::now().timestamp_micros();
        let proof = self
            .verifier
            .verify(
                &request.token,
                Workload::Post,
                METHOD,
                &request.framed_body,
                now,
            )
            .map_err(cache::status)?;
        let decoded = scope(&request.framed_body)?;
        if proof.scope() != &decoded {
            return Err(Status::permission_denied("Cache scope mismatch"));
        }
        let prepared = self
            .issuer
            .prepare_until(
                Workload::Post,
                decoded,
                request.framed_body,
                now,
                proof.expires_micros(),
            )
            .map_err(cache::status)?;
        let deadline = prepared.deadline().map_err(cache::status)?;
        let evidence = tokio::time::timeout_at(
            deadline.stop(),
            self.owner
                .resolve(prepared.body(), prepared.challenge(), deadline),
        )
        .await
        .map_err(|_| Status::deadline_exceeded("Authority deadline expired"))??;
        deadline.remaining(
            std::time::Duration::from_secs(60),
            deadline.expires_micros(),
        )?;
        let now = chrono::Utc::now().timestamp_micros();
        let refreshed_expiry = self
            .issuer
            .check_fresh_owner(prepared, &evidence, now)
            .map_err(cache::status)?;
        if now >= proof.expires_micros() {
            return Err(cache::status(platform::actor_delegation::Denial::Expired));
        }
        Ok(Response::new(proto::ValidateCacheGrantResponse {
            expires_micros: refreshed_expiry.min(proof.expires_micros()),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_a_complete_canonical_uncompressed_child_frame_can_be_issued() {
        let author = Uuid::new_v4();
        let request = proto::ListCacheFollowersRequest {
            claim: Some(proto::CacheClaim {
                event_id: Uuid::new_v4().to_string(),
                post_id: Uuid::new_v4().to_string(),
                event_digest: vec![1; 32],
                attempt_digest: vec![2; 32],
                lease_token: Uuid::new_v4().to_string(),
                attempt_id: Uuid::new_v4().to_string(),
                attempt: 1,
                lease_expires_micros: 1_000_000,
            }),
            author_id: author.to_string(),
            cursor: None,
            limit: 10,
        };
        let framed = |message: Vec<u8>| {
            [
                vec![0],
                (message.len() as u32).to_be_bytes().to_vec(),
                message,
            ]
            .concat()
        };
        let valid = framed(request.encode_to_vec());
        assert_eq!(scope(&valid).unwrap().author_id, author);
        let mut compressed = valid.clone();
        compressed[0] = 1;
        let mut incorrect_length = valid.clone();
        incorrect_length[4] ^= 1;
        let mut unknown = request.encode_to_vec();
        unknown.extend([0x78, 1]);
        let mut duplicate = request.encode_to_vec();
        duplicate.extend([0x20, 10]);
        for invalid in [
            vec![],
            compressed,
            incorrect_length,
            valid[..valid.len() - 1].to_vec(),
            framed(unknown),
            framed(duplicate),
        ] {
            assert!(scope(&invalid).is_err());
        }
    }
    #[test]
    fn method_table_matches_exact_proto_service_and_denies_other_workloads() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proto/authority/v1/authority.proto"
        ));
        let section = source
            .split("service CacheGrantService")
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        assert_eq!(
            platform::grpc_method_policy::proto_rpc_names(section),
            vec!["IssueCacheGrant", "ValidateCacheGrant"]
        );
        assert_eq!(
            CACHE_GRANT_POLICY.method_names().collect::<Vec<_>>(),
            vec!["IssueCacheGrant", "ValidateCacheGrant"]
        );
        for caller in Workload::ALL {
            assert_eq!(
                CACHE_GRANT_POLICY
                    .allows("/authority.v1.CacheGrantService/IssueCacheGrant", caller),
                caller == Workload::Post
            );
        }
        for caller in Workload::ALL {
            assert_eq!(
                CACHE_GRANT_POLICY
                    .allows("/authority.v1.CacheGrantService/ValidateCacheGrant", caller),
                caller == Workload::Friend
            );
        }
    }
}
