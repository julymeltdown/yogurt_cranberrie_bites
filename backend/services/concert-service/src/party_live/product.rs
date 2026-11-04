//! Root-scoped Product producer for the bounded Party routes.
use super::{
    PartyLiveError, PartyLiveOwnerGuard, PartyLiveReply, PartyLiveService, ScopePartyLiveGuard,
};
use crate::{
    adapters::out::{
        public_profile::PublicProfileReader, relationship_authority::guarded::OwnerGuardScope,
    },
    product_rpc::{
        proto, scoped::ProductRepresentation, scoped::ScopedProductProducer, VerifiedProduct,
    },
};
use async_trait::async_trait;
use std::sync::Arc;
use tonic::Status;

pub struct PartyProductProducer<A, F> {
    pub service: Arc<PartyLiveService<A, F>>,
    pub profiles: Arc<PublicProfileReader>,
}

#[async_trait]
impl<A, F> ScopedProductProducer for PartyProductProducer<A, F>
where
    A: crate::community::CommunityAuthority + 'static,
    F: super::PartyLiveOwnerFence + 'static,
{
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        request: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        if !matches!(
            context.operation(),
            crate::product_rpc::Operation::ListParties
                | crate::product_rpc::Operation::GetParty(_)
                | crate::product_rpc::Operation::CreateParty
        ) {
            return Err(Status::unimplemented("Party producer route mismatch"));
        }
        let parsed = super::PartyLiveRequest::from_owner_scope(scope.binding(), &request)
            .map_err(party_status)?;
        let binding = parsed.binding;
        let guard: Arc<dyn PartyLiveOwnerGuard> = Arc::new(
            ScopePartyLiveGuard::new(scope.clone(), self.profiles.clone(), binding)
                .map_err(party_status)?,
        );
        let reply: PartyLiveReply = self
            .service
            .dispatch_for_scope(scope, guard, request)
            .await
            .map_err(party_status)?;
        Ok(ProductRepresentation {
            status: reply.status.into(),
            content_type: "application/json".into(),
            body: serde_json::to_vec(&reply.body)
                .map_err(|_| Status::internal("Invalid Party representation"))?,
        })
    }
}

fn party_status(error: PartyLiveError) -> Status {
    match error {
        PartyLiveError::InvalidInput => Status::invalid_argument("Invalid Party request"),
        PartyLiveError::Denied => Status::permission_denied("Party authorization denied"),
        PartyLiveError::ResourceUnavailable => Status::not_found("Party unavailable"),
        PartyLiveError::VersionConflict => Status::aborted("Party revision conflict"),
        PartyLiveError::InvalidTransition | PartyLiveError::EventReadOnly => {
            Status::failed_precondition("Party transition unavailable")
        }
        PartyLiveError::DependencyUnavailable | PartyLiveError::Internal => {
            Status::unavailable("Party dependency unavailable")
        }
    }
}
