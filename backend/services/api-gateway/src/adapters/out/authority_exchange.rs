//! Issuer implementation: independently resolve owner evidence, then recheck
//! parent expiry and exact child scope before signing. No presenter evidence.
use async_trait::async_trait;
use gateway_application::authority_exchange::*;
use platform::actor_delegation::{
    exchange::{ChildRequest, DelegationExchange, ParentRequest, ResourceLookup},
    Denial,
};
use platform::grpc_auth::Workload;
use std::sync::Arc;

#[derive(Clone)]
pub struct DelegationExchangeRuntime {
    issuer: DelegationExchange,
    owner: Arc<dyn OwnerResourceAuthorityPort>,
}
impl DelegationExchangeRuntime {
    pub fn new(issuer: DelegationExchange, owner: Arc<dyn OwnerResourceAuthorityPort>) -> Self {
        Self { issuer, owner }
    }
}
fn denial(error: Denial) -> AuthorityExchangeErrorDto {
    if matches!(error, Denial::ReplayCacheFull) {
        AuthorityExchangeErrorDto::Unavailable
    } else {
        AuthorityExchangeErrorDto::Denied
    }
}
#[async_trait]
impl AuthorityExchangePort for DelegationExchangeRuntime {
    async fn exchange(
        &self,
        request: AuthorityExchangeRequestDto,
    ) -> Result<IssuedAuthorityDto, AuthorityExchangeErrorDto> {
        let audience =
            Workload::parse(request.audience()).ok_or(AuthorityExchangeErrorDto::Invalid)?;
        let parent = request.parent();
        let parent_request = ParentRequest {
            token: parent.token(),
            method: parent.method(),
            body: parent.body(),
            operation: parent.operation_id(),
        };
        let child = ChildRequest {
            audience,
            method: request.method(),
            body: request.body(),
            operation: request.operation_id(),
        };
        let now = chrono::Utc::now().timestamp();
        let prepared = match request.witness_id() {
            Some(stage) => {
                self.issuer
                    .prepare_with_witness(Workload::Post, parent_request, child, stage, now)
            }
            None => self
                .issuer
                .prepare(Workload::Post, parent_request, child, now),
        }
        .map_err(denial)?;
        let evidence = match prepared.resource_lookup() {
            ResourceLookup::None => None,
            _ => {
                let lookup = OwnerResourceLookupDto::new(
                    parent.clone(),
                    prepared
                        .witness_id()
                        .ok_or(AuthorityExchangeErrorDto::Denied)?,
                    prepared.targets().to_vec(),
                )?;
                Some(self.owner.resolve(&lookup).await?)
            }
        };
        let issued = self
            .issuer
            .finish(
                prepared,
                evidence.as_ref().map(OwnerResourceEvidenceDto::token),
                chrono::Utc::now().timestamp(),
            )
            .map_err(denial)?;
        IssuedAuthorityDto::from_issuer(issued.token, issued.expires_at)
    }
}
