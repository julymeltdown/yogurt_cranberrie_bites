//! Independent mTLS Post-owner/Friend-policy clients. No caller-supplied
//! observations or fallback, and one deadline survives all orchestration hops.
use crate::adapters::r#in::grpc::{
    authority_exchange::proto,
    notification_authority::{NotificationOwnerLookup, NotificationPolicyLookup},
};
use platform::{
    actor_delegation::notification::{
        NotificationRpcDeadline, PreparedNotification, ReadyNotificationPolicy, MAX_BYTES,
    },
    edge::EdgeBudget,
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
    grpc_client_trace::GrpcClientTraceService,
    grpc_tls::TlsMaterial,
};
use std::time::Duration;
use tonic::{service::interceptor::InterceptedService, transport::Channel, Request, Status};
type Transport =
    InterceptedService<GrpcClientTraceService<Channel>, ServiceIdentityClientInterceptor>;

#[derive(Clone)]
pub struct GrpcNotificationAuthorityLookup {
    owner:
        proto::notification_owner_authority_service_client::NotificationOwnerAuthorityServiceClient<
            Transport,
        >,
    policy: proto::notification_policy_service_client::NotificationPolicyServiceClient<Transport>,
    owner_budget: Duration,
    policy_budget: Duration,
}
impl GrpcNotificationAuthorityLookup {
    pub fn connect(
        post: &str,
        friend: &str,
        credential: &str,
        tls: &TlsMaterial,
        budgets: [EdgeBudget; 2],
    ) -> anyhow::Result<Self> {
        let owner = budgets[0].validate()?;
        let policy = budgets[1].validate()?;
        let transport = |endpoint, peer, budget| -> anyhow::Result<Transport> {
            Ok(InterceptedService::new(
                GrpcClientTraceService::new(
                    platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
                        Some(tls),
                        endpoint,
                        peer,
                        budget,
                    )?,
                ),
                ServiceIdentityClientInterceptor::new(Workload::Gateway, credential)?,
            ))
        };
        Ok(Self {
            owner: proto::notification_owner_authority_service_client::NotificationOwnerAuthorityServiceClient::new(transport(post, Workload::Post, owner)?),
            policy: proto::notification_policy_service_client::NotificationPolicyServiceClient::new(transport(friend, Workload::Friend, policy)?),
            owner_budget: owner.deadline, policy_budget: policy.deadline,
        })
    }
}
const DENIED: &str = "Notification authority denied";

/// Preserve the upstream guard's documented stable denial code, and keep the
/// bare constant for every other shape so no arbitrary upstream text crosses
/// the boundary. Collapsing every reason into the constant alone made an
/// observed gate failure unattributable between an expired grant, a replay and
/// a wrong audience.
fn denied_message(upstream: &str) -> String {
    match upstream
        .strip_prefix(DENIED)
        .and_then(|rest| rest.strip_prefix(" ("))
        .and_then(|rest| rest.strip_suffix(')'))
        .filter(|code| {
            code.starts_with("DELEGATION_")
                && code
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte == b'_')
        }) {
        Some(code) => format!("{DENIED} ({code})"),
        None => DENIED.to_owned(),
    }
}

fn failure(error: Status) -> Status {
    match error.code() {
        tonic::Code::PermissionDenied
        | tonic::Code::Unauthenticated
        | tonic::Code::InvalidArgument => {
            Status::permission_denied(denied_message(error.message()))
        }
        tonic::Code::DeadlineExceeded => {
            Status::deadline_exceeded("Notification authority expired")
        }
        _ => Status::unavailable("Notification authority unavailable"),
    }
}
#[allow(clippy::result_large_err)]
fn evidence(e: String) -> Result<String, Status> {
    if e.is_empty() || e.len() > MAX_BYTES {
        return Err(Status::permission_denied("Invalid notification evidence"));
    }
    Ok(e)
}
#[async_trait::async_trait]
impl NotificationOwnerLookup for GrpcNotificationAuthorityLookup {
    async fn resolve(
        &self,
        p: &PreparedNotification,
        deadline: NotificationRpcDeadline,
    ) -> Result<String, Status> {
        let budget = deadline.remaining(self.owner_budget, p.expires_micros())?;
        let mut request = Request::new(proto::ResolveNotificationSourceRequest {
            binding: p
                .binding()
                .encode()
                .map_err(|_| Status::permission_denied("Invalid notification binding"))?,
            framed_body: p.body().to_vec(),
            challenge: p.challenge().to_string(),
        });
        request.set_timeout(budget);
        let reply = tokio::time::timeout(
            budget,
            self.owner.clone().resolve_notification_source(request),
        )
        .await
        .map_err(|_| Status::deadline_exceeded("Notification owner expired"))?
        .map_err(failure)?
        .into_inner();
        deadline.remaining(self.owner_budget, p.expires_micros())?;
        evidence(reply.evidence)
    }
}
#[async_trait::async_trait]
impl NotificationPolicyLookup for GrpcNotificationAuthorityLookup {
    async fn check(
        &self,
        ready: &ReadyNotificationPolicy,
        deadline: NotificationRpcDeadline,
    ) -> Result<String, Status> {
        let budget = deadline.remaining(self.policy_budget, ready.expires_micros())?;
        let mut request = Request::new(proto::CheckNotificationRecipientRequest {
            query: ready.token().to_owned(),
        });
        request.set_timeout(budget);
        let reply = tokio::time::timeout(
            budget,
            self.policy.clone().check_notification_recipient(request),
        )
        .await
        .map_err(|_| Status::deadline_exceeded("Notification policy expired"))?
        .map_err(failure)?
        .into_inner();
        deadline.remaining(self.policy_budget, ready.expires_micros())?;
        evidence(reply.evidence)
    }
}

#[cfg(test)]
mod tests {
    use super::{denied_message, DENIED};

    #[test]
    fn keeps_the_guard_stable_code_and_drops_other_upstream_text() {
        assert_eq!(
            denied_message("Notification authority denied (DELEGATION_EXPIRED)"),
            "Notification authority denied (DELEGATION_EXPIRED)"
        );
        assert_eq!(
            denied_message("Notification authority denied (DELEGATION_ACTOR_MISMATCH)"),
            "Notification authority denied (DELEGATION_ACTOR_MISMATCH)"
        );
    }

    #[test]
    fn refuses_to_forward_arbitrary_upstream_detail() {
        assert_eq!(denied_message("Notification authority denied"), DENIED);
        // Any other code, prose, or injected text stays behind the constant.
        assert_eq!(denied_message("internal claim token abc123"), DENIED);
        assert_eq!(
            denied_message("Notification authority denied (leaked)"),
            DENIED
        );
        assert_eq!(
            denied_message("Notification authority denied (DELEGATION_expired)"),
            DENIED
        );
        assert_eq!(denied_message(""), DENIED);
    }
}
