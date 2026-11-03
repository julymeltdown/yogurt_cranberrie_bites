//! Independent notification issuer/admission orchestration. All trust material,
//! owning clients and budgets are explicit; this does not activate a listener.
use super::authority_exchange::proto;
use platform::actor_delegation::notification::{self, *};
use platform::grpc_auth::{VerifiedCaller, Workload};
use platform::grpc_method_policy::MethodPolicy;
use std::{sync::Arc, time::Duration};
use tonic::{Request, Response, Status};

pub static NOTIFICATION_GRANT_POLICY: MethodPolicy = MethodPolicy {
    service: "authority.v1.NotificationGrantService",
    workload: Some(Workload::Gateway),
    methods: &[
        ("IssueNotificationGrant", &[Workload::Post]),
        (
            "ValidateNotificationGrant",
            &[Workload::Friend, Workload::Notification],
        ),
    ],
    actor_fields: &[],
};
#[async_trait::async_trait]
pub trait NotificationOwnerLookup: Send + Sync {
    async fn resolve(
        &self,
        prepared: &PreparedNotification,
        deadline: NotificationRpcDeadline,
    ) -> Result<String, Status>;
}
#[async_trait::async_trait]
pub trait NotificationPolicyLookup: Send + Sync {
    async fn check(
        &self,
        ready: &ReadyNotificationPolicy,
        deadline: NotificationRpcDeadline,
    ) -> Result<String, Status>;
}

#[derive(Clone)]
pub struct NotificationGrantGrpcServer {
    issuer: NotificationIssuer,
    verifier: NotificationVerifier,
    owner: Arc<dyn NotificationOwnerLookup>,
    policy: Arc<dyn NotificationPolicyLookup>,
    operation_budget: Duration,
}
impl NotificationGrantGrpcServer {
    pub fn new(
        issuer: NotificationIssuer,
        verifier: NotificationVerifier,
        owner: Arc<dyn NotificationOwnerLookup>,
        policy: Arc<dyn NotificationPolicyLookup>,
        operation_budget: Duration,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !operation_budget.is_zero() && operation_budget.as_micros() <= i64::MAX as u128,
            "Invalid notification operation budget"
        );
        Ok(Self {
            issuer,
            verifier,
            owner,
            policy,
            operation_budget,
        })
    }
    async fn final_policy(
        &self,
        p: PreparedNotification,
        owner: &str,
        deadline: NotificationRpcDeadline,
    ) -> Result<(FinalNotificationPolicy, String), Status> {
        let ready = self
            .issuer
            .prepare_policy(p, owner, now())
            .map_err(notification::status)?;
        let evidence = self.policy.check(&ready, deadline).await?;
        let final_policy = self
            .issuer
            .prepare_final_owner(ready, &evidence, now())
            .map_err(notification::status)?;
        let fresh_owner = self
            .owner
            .resolve(final_policy.owner_request(), deadline)
            .await?;
        Ok((final_policy, fresh_owner))
    }
    async fn issue(
        &self,
        binding: NotificationBinding,
        body: Vec<u8>,
        deadline: NotificationRpcDeadline,
    ) -> Result<NotificationIssue, Status> {
        let prepared = self
            .issuer
            .prepare_until(
                Workload::Post,
                binding,
                body,
                deadline.expires_micros(),
                now(),
            )
            .map_err(notification::status)?;
        let evidence = self.owner.resolve(&prepared, deadline).await?;
        if let Some(suppressed) = self
            .issuer
            .source_suppression(&prepared, &evidence, now())
            .map_err(notification::status)?
        {
            return Ok(NotificationIssue::Suppressed {
                expires_micros: suppressed.expires_micros,
            });
        }
        if matches!(prepared.binding(), NotificationBinding::FollowerPage(_)) {
            return self
                .issuer
                .issue_page(prepared, &evidence, now())
                .map(NotificationIssue::Granted)
                .map_err(notification::status);
        }
        let (final_policy, owner) = self.final_policy(prepared, &evidence, deadline).await?;
        self.issuer
            .issue_delivery(final_policy, &owner, now())
            .map_err(notification::status)
    }
    async fn refresh(
        &self,
        proof: VerifiedNotification,
        deadline: NotificationRpcDeadline,
    ) -> Result<NotificationAdmission, Status> {
        let prepared = self
            .issuer
            .prepare_until(
                Workload::Post,
                proof.binding().clone(),
                proof.body().to_vec(),
                deadline.expires_micros(),
                now(),
            )
            .map_err(notification::status)?;
        let evidence = self.owner.resolve(&prepared, deadline).await?;
        if let Some(suppressed) = self
            .issuer
            .refresh_source_suppression(&proof, &prepared, &evidence, now())
            .map_err(notification::status)?
        {
            return Ok(suppressed);
        }
        if matches!(prepared.binding(), NotificationBinding::FollowerPage(_)) {
            return self
                .issuer
                .refresh_page(&proof, prepared, &evidence, now())
                .map_err(notification::status);
        }
        let (final_policy, owner) = self.final_policy(prepared, &evidence, deadline).await?;
        self.issuer
            .refresh_delivery(&proof, final_policy, &owner, now())
            .map_err(notification::status)
    }
}
fn now() -> i64 {
    chrono::Utc::now().timestamp_micros()
}
#[allow(clippy::result_large_err)]
fn binding(bytes: &[u8], body: &[u8]) -> Result<NotificationBinding, Status> {
    let b = NotificationBinding::decode(bytes).map_err(notification::status)?;
    if b.frame().map_err(notification::status)? != body {
        return Err(Status::invalid_argument("Invalid notification frame"));
    }
    Ok(b)
}
#[allow(clippy::result_large_err)]
fn caller<T>(request: &Request<T>) -> Result<Workload, Status> {
    request
        .extensions()
        .get::<VerifiedCaller>()
        .map(|v| v.0)
        .ok_or_else(|| Status::unauthenticated("Verified workload required"))
}
#[tonic::async_trait]
impl proto::notification_grant_service_server::NotificationGrantService
    for NotificationGrantGrpcServer
{
    async fn issue_notification_grant(
        &self,
        request: Request<proto::IssueNotificationGrantRequest>,
    ) -> Result<Response<proto::IssueNotificationGrantResponse>, Status> {
        if caller(&request)? != Workload::Post {
            return Err(Status::permission_denied("Post presenter required"));
        }
        let r = request.into_inner();
        let binding = binding(&r.binding, &r.framed_body)?;
        if r.deadline_micros > binding.claim().lease_expires_micros {
            return Err(Status::invalid_argument("Invalid notification deadline"));
        }
        let deadline = NotificationRpcDeadline::new(r.deadline_micros, self.operation_budget)?;
        let budget =
            deadline.remaining(self.operation_budget, binding.claim().lease_expires_micros)?;
        let result = tokio::time::timeout(budget, self.issue(binding, r.framed_body, deadline))
            .await
            .map_err(|_| Status::deadline_exceeded("Notification issuance expired"))??;
        deadline.remaining(self.operation_budget, deadline.expires_micros())?;
        use proto::issue_notification_grant_response::Result as Wire;
        let result = match result {
            NotificationIssue::Granted(g) => Wire::Granted(proto::NotificationGranted {
                token: g.token,
                expires_micros: g.expires_micros,
            }),
            NotificationIssue::Suppressed { expires_micros } => {
                Wire::Suppressed(proto::NotificationSuppressed { expires_micros })
            }
        };
        Ok(Response::new(proto::IssueNotificationGrantResponse {
            result: Some(result),
        }))
    }
    async fn validate_notification_grant(
        &self,
        request: Request<proto::ValidateNotificationGrantRequest>,
    ) -> Result<Response<proto::ValidateNotificationGrantResponse>, Status> {
        let presenter = caller(&request)?;
        let r = request.into_inner();
        let binding = binding(&r.binding, &r.framed_body)?;
        if presenter != binding.audience() {
            return Err(Status::permission_denied("Notification audience required"));
        }
        let proof = self
            .verifier
            .verify(
                &r.token,
                Workload::Post,
                binding.method(),
                &r.framed_body,
                now(),
            )
            .map_err(notification::status)?;
        if proof.binding() != &binding || r.deadline_micros > proof.expires_micros() {
            return Err(Status::permission_denied("Notification proof mismatch"));
        }
        let deadline = NotificationRpcDeadline::new(r.deadline_micros, self.operation_budget)?;
        let budget = deadline.remaining(self.operation_budget, proof.expires_micros())?;
        let admitted = tokio::time::timeout(budget, self.refresh(proof, deadline))
            .await
            .map_err(|_| Status::deadline_exceeded("Notification admission expired"))??;
        deadline.remaining(self.operation_budget, admitted.expires_micros)?;
        Ok(Response::new(admission_message(admitted)))
    }
}
pub fn admission_message(a: NotificationAdmission) -> proto::ValidateNotificationGrantResponse {
    use proto::validate_notification_grant_response::Result as Wire;
    let result = match a.decision {
        NotificationDecision::Suppressed => Wire::Suppressed(proto::NotificationSuppressed {
            expires_micros: a.expires_micros,
        }),
        NotificationDecision::Allowed { visibility } => Wire::Allowed(proto::NotificationAllowed {
            visibility: match visibility {
                None => 0,
                Some(SourceVisibility::Public) => 1,
                Some(SourceVisibility::Friends) => 2,
            },
            expires_micros: a.expires_micros,
        }),
    };
    proto::ValidateNotificationGrantResponse {
        result: Some(result),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grant_method_policy_matches_schema_and_exact_presentation_roles() {
        let section = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proto/authority/v1/authority.proto"
        ))
        .split("service NotificationGrantService")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
        assert_eq!(
            platform::grpc_method_policy::proto_rpc_names(section),
            NOTIFICATION_GRANT_POLICY.method_names().collect::<Vec<_>>()
        );
        for caller in Workload::ALL {
            assert_eq!(
                NOTIFICATION_GRANT_POLICY.allows(
                    "/authority.v1.NotificationGrantService/IssueNotificationGrant",
                    caller
                ),
                caller == Workload::Post
            );
            assert_eq!(
                NOTIFICATION_GRANT_POLICY.allows(
                    "/authority.v1.NotificationGrantService/ValidateNotificationGrant",
                    caller
                ),
                matches!(caller, Workload::Friend | Workload::Notification)
            );
        }
    }
}
