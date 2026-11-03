//! Independently authenticated lookup of an already published owner stage.
//! No root admission/use-case reentry, witness inserts, or caller-supplied rows.
use platform::actor_delegation::{
    exchange::{ParentRequest, PostAuthor, ResourceEvidenceSigner, ResourceParentScope},
    ActorDelegationVerifier,
};
use platform::grpc_auth::{authorize_call, RpcAction, Workload};
use platform::grpc_method_policy::MethodPolicy;
use post_application::authority_witness::{
    AuthorityWitnessRepositoryPort, ResolveWitnessDto, WitnessErrorDto,
};
use post_application::use_cases::resolve_resource_witness::ResolveResourceWitnessUseCase;
use std::collections::HashSet;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("authority.v1");
}

pub static POST_RESOURCE_AUTHORITY_POLICY: MethodPolicy = MethodPolicy {
    service: "authority.v1.PostResourceAuthorityService",
    workload: Some(Workload::Post),
    methods: &[("ResolvePostResources", &[Workload::Gateway])],
    actor_fields: &[],
};
#[derive(Clone)]
pub struct PostResourceAuthorityGrpcServer {
    lookup: ResolveResourceWitnessUseCase,
    parents: ActorDelegationVerifier,
    signer: ResourceEvidenceSigner,
}
impl PostResourceAuthorityGrpcServer {
    /// Served on Post's internal listener to the Gateway only. The signer comes
    /// from RESOURCE_EVIDENCE_SIGNING_SEED and is distinct from user authority.
    pub fn new(
        repository: Arc<dyn AuthorityWitnessRepositoryPort>,
        parents: ActorDelegationVerifier,
        signer: ResourceEvidenceSigner,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            parents.audience() == Workload::Post,
            "Post parent verifier required"
        );
        Ok(Self {
            lookup: ResolveResourceWitnessUseCase::new(repository),
            parents,
            signer,
        })
    }
}
#[allow(clippy::result_large_err)]
fn uuid(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or_else(|| Status::invalid_argument("Invalid resource lookup binding"))
}
fn lookup_error(error: WitnessErrorDto) -> Status {
    match error {
        WitnessErrorDto::Unavailable | WitnessErrorDto::StageConflict => {
            Status::unavailable("Owner evidence unavailable")
        }
        _ => Status::permission_denied("Owner evidence denied"),
    }
}
#[tonic::async_trait]
impl proto::post_resource_authority_service_server::PostResourceAuthorityService
    for PostResourceAuthorityGrpcServer
{
    async fn resolve_post_resources(
        &self,
        request: Request<proto::ResolvePostResourcesRequest>,
    ) -> Result<Response<proto::ResolvePostResourcesResponse>, Status> {
        if authorize_call(&request, RpcAction::Read)? != Workload::Gateway {
            return Err(Status::permission_denied(
                "Independent Gateway lookup required",
            ));
        }
        let request = request.into_inner();
        if request.target_author_ids.is_empty() || request.target_author_ids.len() > 100 {
            return Err(Status::invalid_argument("Resource target bound exceeded"));
        }
        let authors = request
            .target_author_ids
            .iter()
            .map(|id| uuid(id))
            .collect::<Result<Vec<_>, _>>()?;
        if authors.iter().copied().collect::<HashSet<_>>().len() != authors.len() {
            return Err(Status::invalid_argument("Duplicate resource targets"));
        }
        let stage = uuid(&request.witness_id)?;
        let parent = request
            .parent
            .ok_or_else(|| Status::invalid_argument("Parent required"))?;
        let operation = parent.operation_id.as_deref().map(uuid).transpose()?;
        let proof = self
            .parents
            .verify_resource_parent(
                ParentRequest {
                    token: &parent.token,
                    method: &parent.method,
                    body: &parent.framed_body,
                    operation,
                },
                chrono::Utc::now().timestamp(),
            )
            .map_err(|denial| denial.status())?;
        let query = match proof.scope() {
            ResourceParentScope::Posts(ids) => {
                ResolveWitnessDto::posts(stage, *proof.digest(), ids.to_vec())
            }
            ResourceParentScope::Discovery => {
                ResolveWitnessDto::authors(stage, *proof.digest(), authors.clone())
            }
        }
        .map_err(lookup_error)?;
        let subset = self.lookup.execute(&query).await.map_err(lookup_error)?;
        if subset.expires_at().timestamp() != proof.expires_at()
            || !authors
                .iter()
                .all(|id| subset.members().iter().any(|row| row.author_id() == *id))
        {
            return Err(Status::permission_denied("Owner evidence binding denied"));
        }
        let rows: Vec<_> = subset
            .members()
            .iter()
            .map(|row| PostAuthor {
                post_id: row.post_id(),
                author_id: row.author_id(),
            })
            .collect();
        let evidence = self
            .signer
            .attest_resource_parent(
                &proof,
                stage,
                &rows,
                subset.missing(),
                chrono::Utc::now().timestamp(),
            )
            .map_err(|denial| denial.status())?;
        Ok(Response::new(proto::ResolvePostResourcesResponse {
            evidence,
        }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resource_policy_is_complete_and_gateway_only() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proto/authority/v1/authority.proto"
        ));
        let section = source
            .split("service PostResourceAuthorityService")
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        let declared = platform::grpc_method_policy::proto_rpc_names(section);
        assert_eq!(declared, vec!["ResolvePostResources"]);
        assert_eq!(
            POST_RESOURCE_AUTHORITY_POLICY
                .method_names()
                .collect::<Vec<_>>(),
            vec!["ResolvePostResources"]
        );
        for caller in Workload::ALL {
            assert_eq!(
                POST_RESOURCE_AUTHORITY_POLICY.allows(
                    "/authority.v1.PostResourceAuthorityService/ResolvePostResources",
                    caller
                ),
                caller == Workload::Gateway
            );
        }
    }
}
