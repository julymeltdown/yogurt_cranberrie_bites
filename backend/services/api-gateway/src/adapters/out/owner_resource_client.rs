//! Independent Gateway -> Post owner evidence transport, without forwarding
//! presenter-supplied rows/evidence or installing root actor context.
use super::super::r#in::grpc::authority_exchange::proto;
use async_trait::async_trait;
use gateway_application::authority_exchange::*;
use platform::edge::Edge;
use platform::edge_metrics::EdgeDeadlineMetrics;
use platform::grpc_auth::{ServiceIdentityClientInterceptor, Workload};
use platform::grpc_client_trace::GrpcClientTraceService;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::Channel;

#[derive(Clone)]
pub struct GrpcPostResourceAuthorityClient {
    client: proto::post_resource_authority_service_client::PostResourceAuthorityServiceClient<
        InterceptedService<GrpcClientTraceService<Channel>, ServiceIdentityClientInterceptor>,
    >,
    deadlines: EdgeDeadlineMetrics,
}
impl GrpcPostResourceAuthorityClient {
    pub fn connect(
        endpoint: &str,
        credential: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        deadlines: EdgeDeadlineMetrics,
    ) -> anyhow::Result<Self> {
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            endpoint,
            Workload::Post,
            Edge::GatewayPost.budget(),
        )?;
        Ok(Self {
            client:
                proto::post_resource_authority_service_client::PostResourceAuthorityServiceClient::with_interceptor(
                    GrpcClientTraceService::new(channel),
                    ServiceIdentityClientInterceptor::new(Workload::Gateway, credential)?,
                ),
            deadlines,
        })
    }
}
#[async_trait]
impl OwnerResourceAuthorityPort for GrpcPostResourceAuthorityClient {
    async fn resolve(
        &self,
        request: &OwnerResourceLookupDto,
    ) -> Result<OwnerResourceEvidenceDto, AuthorityExchangeErrorDto> {
        let parent = request.parent();
        // Post asks for this exchange while it serves a Gateway to Post call
        // that already holds a GatewayPost permit, so the lookup uses the
        // separate callback pool rather than competing with its own caller.
        let response = platform::grpc_deadline::with_callback_edge_deadline(
            Edge::GatewayPost,
            self.deadlines.clone(),
            self.client
                .clone()
                .resolve_post_resources(proto::ResolvePostResourcesRequest {
                    parent: Some(proto::ParentAuthority {
                        token: parent.token().to_owned(),
                        method: parent.method().to_owned(),
                        framed_body: parent.body().to_vec(),
                        operation_id: parent.operation_id().map(|id| id.to_string()),
                    }),
                    witness_id: request.witness_id().to_string(),
                    target_author_ids: request.targets().iter().map(ToString::to_string).collect(),
                }),
        )
        .await
        .map_err(|status| match status.code() {
            tonic::Code::PermissionDenied | tonic::Code::Unauthenticated => {
                AuthorityExchangeErrorDto::Denied
            }
            tonic::Code::InvalidArgument => AuthorityExchangeErrorDto::Invalid,
            _ => AuthorityExchangeErrorDto::Unavailable,
        })?
        .into_inner();
        OwnerResourceEvidenceDto::from_owner_response(response.evidence)
    }
}
