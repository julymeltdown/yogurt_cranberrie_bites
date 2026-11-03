//! Independent Gateway admission refresh before projection dispatch.
use crate::adapters::r#in::grpc::fanout_projection::proto;
use platform::actor_delegation::fanout::{AdmissionFuture, FanoutAdmissionCheck, VerifiedFanout};
use platform::grpc_auth::Workload;
use tonic::Request;
#[derive(Clone)]
pub struct GrpcFanoutAdmission {
    client: proto::fanout_grant_service_client::FanoutGrantServiceClient<
        tonic::service::interceptor::InterceptedService<
            platform::grpc_client_trace::GrpcClientTraceService<tonic::transport::Channel>,
            platform::grpc_auth::ServiceIdentityClientInterceptor,
        >,
    >,
    budget: std::time::Duration,
}
impl GrpcFanoutAdmission {
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
            Workload::Gateway,
            budget,
        )?;
        Ok(Self {
            client: proto::fanout_grant_service_client::FanoutGrantServiceClient::with_interceptor(
                platform::grpc_client_trace::GrpcClientTraceService::new(channel),
                platform::grpc_auth::ServiceIdentityClientInterceptor::new(
                    Workload::Friend,
                    credential,
                )?,
            ),
            budget: budget.deadline,
        })
    }
}
impl FanoutAdmissionCheck for GrpcFanoutAdmission {
    fn check<'a>(&'a self, proof: &'a VerifiedFanout) -> AdmissionFuture<'a> {
        Box::pin(async move {
            let remaining = proof
                .expires_micros()
                .saturating_sub(chrono::Utc::now().timestamp_micros());
            if remaining <= 0 {
                return Err(platform::actor_delegation::Denial::Expired);
            }
            let budget = self
                .budget
                .min(std::time::Duration::from_micros(remaining as u64));
            let mut request = Request::new(proto::ValidateFanoutGrantRequest {
                framed_body: proof.body().to_vec(),
                token: proof.token().to_owned(),
            });
            request.set_timeout(budget);
            let reply =
                tokio::time::timeout(budget, self.client.clone().validate_fanout_grant(request))
                    .await
                    .map_err(|_| platform::actor_delegation::Denial::ReplayCacheFull)?
                    .map_err(|error| match error.code() {
                        tonic::Code::PermissionDenied | tonic::Code::Unauthenticated => {
                            platform::actor_delegation::Denial::WrongBody
                        }
                        _ => platform::actor_delegation::Denial::ReplayCacheFull,
                    })?
                    .into_inner();
            if reply.expires_micros > proof.expires_micros() {
                return Err(platform::actor_delegation::Denial::WrongBody);
            }
            if chrono::Utc::now().timestamp_micros() >= reply.expires_micros {
                return Err(platform::actor_delegation::Denial::Expired);
            }
            Ok(reply.expires_micros)
        })
    }
}
