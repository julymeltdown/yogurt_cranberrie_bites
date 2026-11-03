//! Independent mTLS Gateway admission; carries the minimum refreshed deadline.
pub mod proto {
    tonic::include_proto!("authority.v1");
}
use platform::{
    actor_delegation::{notification::*, Denial},
    edge::EdgeBudget,
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
    grpc_client_trace::GrpcClientTraceService,
    grpc_tls::TlsMaterial,
};
use std::time::Duration;
use tonic::{service::interceptor::InterceptedService, transport::Channel, Request};
type Transport =
    InterceptedService<GrpcClientTraceService<Channel>, ServiceIdentityClientInterceptor>;
#[derive(Clone)]
pub struct GrpcNotificationAdmission {
    client: proto::notification_grant_service_client::NotificationGrantServiceClient<Transport>,
    budget: Duration,
}
impl GrpcNotificationAdmission {
    pub fn connect(
        endpoint: &str,
        credential: &str,
        tls: &TlsMaterial,
        budget: EdgeBudget,
    ) -> anyhow::Result<Self> {
        let budget = budget.validate()?;
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            Some(tls),
            endpoint,
            Workload::Gateway,
            budget,
        )?;
        Ok(Self { client: proto::notification_grant_service_client::NotificationGrantServiceClient::with_interceptor(
            GrpcClientTraceService::new(channel), ServiceIdentityClientInterceptor::new(Workload::Notification, credential)?), budget: budget.deadline })
    }
}
impl NotificationAdmissionCheck for GrpcNotificationAdmission {
    fn check<'a>(&'a self, proof: &'a VerifiedNotification) -> NotificationAdmissionFuture<'a> {
        Box::pin(async move {
            let deadline = NotificationRpcDeadline::new(proof.expires_micros(), self.budget)
                .map_err(|_| Denial::Expired)?;
            let budget = deadline
                .remaining(self.budget, proof.expires_micros())
                .map_err(|_| Denial::Expired)?;
            let mut request = Request::new(proto::ValidateNotificationGrantRequest {
                binding: proof.binding().encode()?,
                framed_body: proof.body().to_vec(),
                token: proof.token().to_owned(),
                deadline_micros: deadline.expires_micros(),
            });
            request.set_timeout(budget);
            let reply = tokio::time::timeout(
                budget,
                self.client.clone().validate_notification_grant(request),
            )
            .await
            .map_err(|_| Denial::ReplayCacheFull)?
            .map_err(|e| match e.code() {
                tonic::Code::PermissionDenied
                | tonic::Code::Unauthenticated
                | tonic::Code::InvalidArgument => Denial::WrongBody,
                _ => Denial::ReplayCacheFull,
            })?
            .into_inner();
            use proto::validate_notification_grant_response::Result as Wire;
            let (decision, expires_micros) = match reply.result.ok_or(Denial::Malformed)? {
                Wire::Suppressed(s) => (NotificationDecision::Suppressed, s.expires_micros),
                Wire::Allowed(a) => {
                    let visibility = match a.visibility {
                        0 => None,
                        1 => Some(SourceVisibility::Public),
                        2 => Some(SourceVisibility::Friends),
                        _ => return Err(Denial::WrongBody),
                    };
                    (
                        NotificationDecision::Allowed { visibility },
                        a.expires_micros,
                    )
                }
            };
            let admission = NotificationAdmission {
                decision,
                expires_micros,
            }
            .validate_for(
                proof.binding(),
                deadline.expires_micros(),
                chrono::Utc::now().timestamp_micros(),
            )?;
            deadline
                .remaining(self.budget, expires_micros)
                .map_err(|_| Denial::Expired)?;
            Ok(admission)
        })
    }
}
