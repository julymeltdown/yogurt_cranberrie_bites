use super::{contract::*, proto};
use platform::{
    actor_delegation::{
        authority_deadline::AuthorityRpcDeadline, relationship_read::RelationshipReadSigner,
        request_for, ActorDelegationClient,
    },
    edge::{EdgeBudget, FailurePolicy, RetryClass},
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use std::{future::Future, time::Duration};
use tonic::{service::interceptor::InterceptedService, transport::Channel, Status};
type Transport =
    InterceptedService<ActorDelegationClient<Channel>, ServiceIdentityClientInterceptor>;
const EDGE: EdgeBudget = EdgeBudget {
    edge: "concert.canonical_owner_guard",
    expected_latency: Duration::from_millis(100),
    deadline: MAX_GUARD_DURATION,
    connect_deadline: Duration::from_millis(500),
    retry: RetryClass::Never,
    failure_policy: FailurePolicy::FailRequest,
};
#[derive(Clone)]
pub(super) struct OwnerClients {
    auth: proto::auth_owner_authority_service_client::AuthOwnerAuthorityServiceClient<Transport>,
    friend:
        proto::friend_owner_authority_service_client::FriendOwnerAuthorityServiceClient<Transport>,
}
#[derive(Clone, Copy)]
pub(super) enum Owner {
    Auth,
    Friend,
}
impl OwnerClients {
    pub fn connect(
        auth_endpoint: &str,
        friend_endpoint: &str,
        credential: &str,
        tls: &platform::grpc_tls::TlsMaterial,
        signer: RelationshipReadSigner,
    ) -> anyhow::Result<Self> {
        let channel = |endpoint, audience| {
            platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
                Some(tls),
                endpoint,
                audience,
                EDGE.validate()?,
            )
        };
        Ok(Self{auth:proto::auth_owner_authority_service_client::AuthOwnerAuthorityServiceClient::with_interceptor(signer.client(channel(auth_endpoint,Workload::Auth)?,Workload::Auth)?,ServiceIdentityClientInterceptor::new(Workload::Concert,credential)?).max_encoding_message_size(65536).max_decoding_message_size(65536),friend:proto::friend_owner_authority_service_client::FriendOwnerAuthorityServiceClient::with_interceptor(signer.client(channel(friend_endpoint,Workload::Friend)?,Workload::Friend)?,ServiceIdentityClientInterceptor::new(Workload::Concert,credential)?).max_encoding_message_size(65536).max_decoding_message_size(65536)})
    }
    pub async fn acquire(
        &self,
        owner: Owner,
        message: proto::AcquireRequest,
        budget: AuthorityRpcDeadline,
    ) -> Result<proto::AcquireResponse, Status> {
        let actor = message
            .binding
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing binding"))?
            .actor_id
            .clone();
        let mut request = request_for(message, &actor)?;
        request.set_timeout(budget.remaining(MAX_GUARD_DURATION, budget.expires_micros())?);
        match owner {
            Owner::Auth => bounded(budget, self.auth.clone().acquire(request)).await,
            Owner::Friend => bounded(budget, self.friend.clone().acquire(request)).await,
        }
    }
}
macro_rules! method {
    ($method:ident,$input:ident,$output:ident) => {
        impl OwnerClients {
            pub async fn $method(
                &self,
                owner: Owner,
                message: proto::$input,
                budget: AuthorityRpcDeadline,
            ) -> Result<proto::$output, Status> {
                let mut request = tonic::Request::new(message);
                request.set_timeout(budget.remaining(MAX_GUARD_DURATION, budget.expires_micros())?);
                match owner {
                    Owner::Auth => bounded(budget, self.auth.clone().$method(request)).await,
                    Owner::Friend => bounded(budget, self.friend.clone().$method(request)).await,
                }
            }
        }
    };
}
method!(check, CheckRequest, CheckResponse);
method!(enroll, EnrollRequest, CheckResponse);
method!(track, TrackRequest, TrackResponse);
method!(complete, CompleteRequest, CompleteResponse);
async fn bounded<T>(
    budget: AuthorityRpcDeadline,
    future: impl Future<Output = Result<tonic::Response<T>, Status>>,
) -> Result<T, Status> {
    let response = platform::call_deadline::scope(
        budget.stop().into_std(),
        tokio::time::timeout_at(budget.stop(), future),
    )
    .await
    .map_err(|_| Status::deadline_exceeded("Canonical authority deadline"))??;
    if response
        .metadata()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        != Some("no-store")
    {
        return Err(Status::unavailable("Missing owner response cache policy"));
    }
    Ok(response.into_inner())
}
