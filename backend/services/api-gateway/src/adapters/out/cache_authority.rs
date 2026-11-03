//! Independently authenticated owning-service lookup.
use crate::adapters::r#in::grpc::{authority_exchange::proto, cache_authority::CacheOwnerLookup};
use platform::grpc_auth::Workload;
use tonic::{Request, Status};
use uuid::Uuid;
/// Independent Gateway-to-Post client. The presenter cannot inject this reply.
#[derive(Clone)]
pub struct GrpcCacheOwnerLookup {
    client: proto::cache_owner_authority_service_client::CacheOwnerAuthorityServiceClient<
        tonic::service::interceptor::InterceptedService<
            platform::grpc_client_trace::GrpcClientTraceService<tonic::transport::Channel>,
            platform::grpc_auth::ServiceIdentityClientInterceptor,
        >,
    >,
    budget: std::time::Duration,
}
impl GrpcCacheOwnerLookup {
    pub fn connect(
        endpoint: &str,
        credential: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        budget: platform::edge::EdgeBudget,
    ) -> anyhow::Result<Self> {
        let budget = budget.validate()?;
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            endpoint,
            Workload::Post,
            budget,
        )?;
        Ok(Self { client: proto::cache_owner_authority_service_client::CacheOwnerAuthorityServiceClient::with_interceptor(platform::grpc_client_trace::GrpcClientTraceService::new(channel), platform::grpc_auth::ServiceIdentityClientInterceptor::new(Workload::Gateway, credential)?), budget: budget.deadline })
    }
}
#[async_trait::async_trait]
impl CacheOwnerLookup for GrpcCacheOwnerLookup {
    async fn resolve(
        &self,
        body: &[u8],
        challenge: Uuid,
        deadline: platform::actor_delegation::authority_deadline::AuthorityRpcDeadline,
    ) -> Result<String, Status> {
        let mut request = Request::new(proto::ResolveCachePageRequest {
            framed_body: body.to_vec(),
            challenge: challenge.to_string(),
        });
        let budget = deadline.remaining(self.budget, deadline.expires_micros())?;
        request.set_timeout(budget);
        let reply = tokio::time::timeout(budget, self.client.clone().resolve_cache_page(request))
            .await
            .map_err(|_| Status::unavailable("Cache owner unavailable"))?
            .map_err(|error| match error.code() {
                tonic::Code::PermissionDenied | tonic::Code::Unauthenticated => {
                    Status::permission_denied("Cache owner denied")
                }
                _ => Status::unavailable("Cache owner unavailable"),
            })?
            .into_inner();
        deadline.remaining(self.budget, deadline.expires_micros())?;
        if reply.evidence.len() > 16 * 1024 {
            return Err(Status::permission_denied("Invalid cache evidence"));
        }
        Ok(reply.evidence)
    }
}
