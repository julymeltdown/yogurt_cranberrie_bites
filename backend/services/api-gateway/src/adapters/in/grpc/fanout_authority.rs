//! Explicitly composed system issuer and independent pre-dispatch refresh.
use super::authority_exchange::proto;
use platform::actor_delegation::fanout::{self, FanoutIssuer, FanoutScope, FanoutVerifier, METHOD};
use platform::grpc_auth::{authorize_call, RpcAction, Workload};
use platform::grpc_method_policy::MethodPolicy;
use prost::Message;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub static FANOUT_GRANT_POLICY: MethodPolicy = MethodPolicy {
    service: "authority.v1.FanoutGrantService",
    workload: Some(Workload::Gateway),
    methods: &[
        ("IssueFanoutGrant", &[Workload::Post]),
        ("ValidateFanoutGrant", &[Workload::Friend]),
    ],
    actor_fields: &[],
};
#[async_trait::async_trait]
pub trait FanoutOwnerLookup: Send + Sync {
    async fn resolve(
        &self,
        body: &[u8],
        challenge: Uuid,
        deadline: platform::actor_delegation::authority_deadline::AuthorityRpcDeadline,
    ) -> Result<String, Status>;
}
#[derive(Clone)]
pub struct FanoutGrantGrpcServer {
    issuer: FanoutIssuer,
    verifier: FanoutVerifier,
    owner: Arc<dyn FanoutOwnerLookup>,
}
impl FanoutGrantGrpcServer {
    pub fn new(
        issuer: FanoutIssuer,
        verifier: FanoutVerifier,
        owner: Arc<dyn FanoutOwnerLookup>,
    ) -> Self {
        Self {
            issuer,
            verifier,
            owner,
        }
    }
}
#[allow(clippy::result_large_err)]
fn scope(body: &[u8]) -> Result<FanoutScope, Status> {
    let invalid = || Status::invalid_argument("Invalid fanout request");
    if body.len() < 5 || body.len() > 16 * 1024 || body[0] != 0 {
        return Err(invalid());
    }
    let m = proto::ListFanoutFollowersRequest::decode(&body[5..]).map_err(|_| invalid())?;
    let encoded = m.encode_to_vec();
    if body[1..5] != (encoded.len() as u32).to_be_bytes() || encoded != body[5..] {
        return Err(invalid());
    }
    let c = m.claim.ok_or_else(invalid)?;
    let uuid = |s: &str| Uuid::parse_str(s).map_err(|_| invalid());
    let scope = FanoutScope {
        job_id: uuid(&c.job_id)?,
        event_id: uuid(&c.event_id)?,
        post_id: uuid(&c.post_id)?,
        author_id: uuid(&c.author_id)?,
        lease_digest: FanoutScope::lease_digest(uuid(&c.lease_token)?).map_err(fanout::status)?,
        post_created_micros: c.post_created_micros,
        lease_expires_micros: c.lease_expires_micros,
        attempt: c.attempt,
        page_attempt: c.page_attempt,
        max_page_attempts: c.max_page_attempts,
        cursor: c.cursor,
        limit: m.limit,
    };
    scope.validate().map_err(fanout::status)?;
    if chrono::DateTime::from_timestamp_micros(scope.post_created_micros).is_none()
        || chrono::DateTime::from_timestamp_micros(scope.lease_expires_micros).is_none()
    {
        return Err(invalid());
    }
    Ok(scope)
}
#[tonic::async_trait]
impl proto::fanout_grant_service_server::FanoutGrantService for FanoutGrantGrpcServer {
    async fn issue_fanout_grant(
        &self,
        request: Request<proto::IssueFanoutGrantRequest>,
    ) -> Result<Response<proto::IssueFanoutGrantResponse>, Status> {
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
            .map_err(fanout::status)?;
        let deadline = prepared.deadline().map_err(fanout::status)?;
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
            .map_err(fanout::status)?;
        Ok(Response::new(proto::IssueFanoutGrantResponse {
            token: issued.token,
            expires_micros: issued.expires_micros,
        }))
    }
    async fn validate_fanout_grant(
        &self,
        request: Request<proto::ValidateFanoutGrantRequest>,
    ) -> Result<Response<proto::ValidateFanoutGrantResponse>, Status> {
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
            .map_err(fanout::status)?;
        let decoded = scope(&request.framed_body)?;
        if proof.scope() != &decoded {
            return Err(Status::permission_denied("Fanout scope mismatch"));
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
            .map_err(fanout::status)?;
        let deadline = prepared.deadline().map_err(fanout::status)?;
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
            .map_err(fanout::status)?;
        if now >= proof.expires_micros() {
            return Err(fanout::status(platform::actor_delegation::Denial::Expired));
        }
        Ok(Response::new(proto::ValidateFanoutGrantResponse {
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
        let request = proto::ListFanoutFollowersRequest {
            claim: Some(proto::FanoutClaim {
                job_id: Uuid::new_v4().to_string(),
                event_id: Uuid::new_v4().to_string(),
                post_id: Uuid::new_v4().to_string(),
                author_id: author.to_string(),
                lease_token: Uuid::new_v4().to_string(),
                post_created_micros: 10,
                lease_expires_micros: 1_000_000,
                attempt: 3,
                page_attempt: 2,
                max_page_attempts: 2,
                cursor: None,
            }),
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
        duplicate.extend([0x10, 10]);
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
            .split("service FanoutGrantService")
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        assert_eq!(
            platform::grpc_method_policy::proto_rpc_names(section),
            vec!["IssueFanoutGrant", "ValidateFanoutGrant"]
        );
        assert_eq!(
            FANOUT_GRANT_POLICY.method_names().collect::<Vec<_>>(),
            vec!["IssueFanoutGrant", "ValidateFanoutGrant"]
        );
        for caller in Workload::ALL {
            assert_eq!(
                FANOUT_GRANT_POLICY
                    .allows("/authority.v1.FanoutGrantService/IssueFanoutGrant", caller),
                caller == Workload::Post
            );
        }
        for caller in Workload::ALL {
            assert_eq!(
                FANOUT_GRANT_POLICY.allows(
                    "/authority.v1.FanoutGrantService/ValidateFanoutGrant",
                    caller
                ),
                caller == Workload::Friend
            );
        }
    }
}
