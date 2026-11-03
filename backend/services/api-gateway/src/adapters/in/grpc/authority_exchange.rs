//! Internal issuer RPC served on the Gateway's GATEWAY_GRPC_ADDR listener to Post.
use gateway_application::authority_exchange::*;
use platform::grpc_auth::{authorize_call, RpcAction, Workload};
use platform::grpc_method_policy::MethodPolicy;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("authority.v1");
}
pub static AUTHORIZATION_EXCHANGE_POLICY: MethodPolicy = MethodPolicy {
    service: "authority.v1.AuthorizationExchangeService",
    workload: Some(Workload::Gateway),
    methods: &[("ExchangeActor", &[Workload::Post])],
    actor_fields: &[],
};
#[derive(Clone)]
pub struct GatewayAuthorizationExchangeGrpcServer {
    service: ExchangeAuthorityUseCase,
}
impl GatewayAuthorizationExchangeGrpcServer {
    pub fn new(issuer: Arc<dyn AuthorityExchangePort>) -> Self {
        Self {
            service: ExchangeAuthorityUseCase::new(issuer),
        }
    }
}
fn error(error: AuthorityExchangeErrorDto) -> Status {
    match error {
        AuthorityExchangeErrorDto::Invalid => {
            Status::invalid_argument("Invalid authority exchange")
        }
        AuthorityExchangeErrorDto::Denied => Status::permission_denied("Authority exchange denied"),
        AuthorityExchangeErrorDto::Unavailable => {
            Status::unavailable("Authority exchange unavailable")
        }
    }
}
#[allow(clippy::result_large_err)]
fn uuid(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or_else(|| Status::invalid_argument("Invalid authority binding"))
}
#[tonic::async_trait]
impl proto::authorization_exchange_service_server::AuthorizationExchangeService
    for GatewayAuthorizationExchangeGrpcServer
{
    async fn exchange_actor(
        &self,
        request: Request<proto::ExchangeActorRequest>,
    ) -> Result<Response<proto::ExchangeActorResponse>, Status> {
        if authorize_call(&request, RpcAction::Read)? != Workload::Post {
            return Err(Status::permission_denied("Post presenter required"));
        }
        let request = request.into_inner();
        let parent = request
            .parent
            .ok_or_else(|| Status::invalid_argument("Parent required"))?;
        let parent = ParentAuthorityDto::new(
            parent.token,
            parent.method,
            parent.framed_body,
            parent.operation_id.as_deref().map(uuid).transpose()?,
        )
        .map_err(error)?;
        let mapped = AuthorityExchangeRequestDto::new(
            parent,
            request.audience,
            request.method,
            request.framed_body,
            request.operation_id.as_deref().map(uuid).transpose()?,
            request.witness_id.as_deref().map(uuid).transpose()?,
        )
        .map_err(error)?;
        let (token, expires_at) = self
            .service
            .exchange_authority(mapped)
            .await
            .map_err(error)?
            .into_parts();
        Ok(Response::new(proto::ExchangeActorResponse {
            token,
            expires_at,
        }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn issuer_policy_is_complete_and_post_only() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proto/authority/v1/authority.proto"
        ));
        let section = source
            .split("service AuthorizationExchangeService")
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        assert_eq!(
            platform::grpc_method_policy::proto_rpc_names(section),
            vec!["ExchangeActor"]
        );
        assert_eq!(
            AUTHORIZATION_EXCHANGE_POLICY
                .method_names()
                .collect::<Vec<_>>(),
            vec!["ExchangeActor"]
        );
        for caller in Workload::ALL {
            assert_eq!(
                AUTHORIZATION_EXCHANGE_POLICY.allows(
                    "/authority.v1.AuthorizationExchangeService/ExchangeActor",
                    caller
                ),
                caller == Workload::Post
            );
        }
    }
}
