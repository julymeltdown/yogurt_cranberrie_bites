use crate::adapters::out::grpc_call::BoundedGrpcCallExt;
use crate::application::enrollment_policy::*;
use crate::config::GatewayMetrics;
use async_trait::async_trait;
use platform::actor_delegation::{ActorDelegationClient, ActorDelegationSigner};
use platform::grpc_auth::{ServiceIdentityClientInterceptor, Workload};
use platform::grpc_client_trace::GrpcClientTraceService;
use serde_json::Value;
use tonic::{service::interceptor::InterceptedService, transport::Channel, Code, Request, Status};
use uuid::Uuid;
pub mod proto {
    tonic::include_proto!("auth.enrollment.v1");
}
#[derive(Clone)]
pub struct GrpcEnrollmentClient {
    client: proto::enrollment_policy_service_client::EnrollmentPolicyServiceClient<
        InterceptedService<
            GrpcClientTraceService<ActorDelegationClient<Channel>>,
            ServiceIdentityClientInterceptor,
        >,
    >,
    metrics: GatewayMetrics,
}
impl GrpcEnrollmentClient {
    pub fn connect(
        endpoint: &str,
        token: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        signer: ActorDelegationSigner,
        metrics: GatewayMetrics,
    ) -> anyhow::Result<Self> {
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            endpoint,
            Workload::Auth,
            platform::edge::Edge::GatewayAuth.budget(),
        )?;
        let client=proto::enrollment_policy_service_client::EnrollmentPolicyServiceClient::with_interceptor(
            GrpcClientTraceService::new(ActorDelegationClient::new(channel,signer,Workload::Auth)),
            ServiceIdentityClientInterceptor::new(Workload::Gateway,token)?)
            .max_decoding_message_size(32768).max_encoding_message_size(16384);
        Ok(Self { client, metrics })
    }
}
#[async_trait]
impl EnrollmentPolicyPort for GrpcEnrollmentClient {
    async fn execute(
        &self,
        operation: EnrollmentOperation,
        binding: Option<&str>,
        actor: Option<Uuid>,
        body: Value,
    ) -> Result<Value, EnrollmentFailure> {
        let mut client = self.client.clone();
        let edge = platform::edge::Edge::GatewayAuth;
        let anonymous = || {
            let mut request = Request::new(proto::AnonymousCommand {
                client_binding: binding.unwrap_or_default().into(),
                payload_json: body.to_string(),
            });
            request.set_timeout(edge.budget().deadline);
            request
        };
        let authenticated = || -> Result<Request<proto::ActorCommand>, EnrollmentFailure> {
            let actor = actor.ok_or(EnrollmentFailure {
                status: 401,
                reason: "AUTHENTICATION_REQUIRED",
            })?;
            let mut request = platform::actor_delegation::request_for(
                proto::ActorCommand {
                    user_id: actor.to_string(),
                    payload_json: if operation == EnrollmentOperation::Admission {
                        String::new()
                    } else {
                        body.to_string()
                    },
                },
                &actor.to_string(),
            )
            .map_err(|_| EnrollmentFailure::UNAVAILABLE)?;
            request.set_timeout(edge.budget().deadline);
            Ok(request)
        };
        let response = match operation {
            EnrollmentOperation::Policy => {
                let mut request = Request::new(proto::ReadPolicyRequest {});
                request.set_timeout(edge.budget().deadline);
                client
                    .read_policy(request)
                    .with_edge_deadline(edge, self.metrics.clone())
                    .await
            }
            EnrollmentOperation::Begin => {
                client
                    .begin_attempt(anonymous())
                    .with_edge_deadline(edge, self.metrics.clone())
                    .await
            }
            EnrollmentOperation::Signup => {
                client
                    .signup(anonymous())
                    .with_edge_deadline(edge, self.metrics.clone())
                    .await
            }
            EnrollmentOperation::Verify => {
                client
                    .verify(anonymous())
                    .with_edge_deadline(edge, self.metrics.clone())
                    .await
            }
            EnrollmentOperation::Resend => {
                client
                    .resend(anonymous())
                    .with_edge_deadline(edge, self.metrics.clone())
                    .await
            }
            EnrollmentOperation::Admission => {
                client
                    .read_admission(authenticated()?)
                    .with_edge_deadline(edge, self.metrics.clone())
                    .await
            }
            EnrollmentOperation::Consent => {
                client
                    .accept_consent(authenticated()?)
                    .with_edge_deadline(edge, self.metrics.clone())
                    .await
            }
        }
        .map_err(classify)?
        .into_inner();
        let value: Value = serde_json::from_str(&response.body_json)
            .map_err(|_| EnrollmentFailure::INVALID_RESPONSE)?;
        validate_response(operation, &value)?;
        Ok(value)
    }
}
fn classify(status: Status) -> EnrollmentFailure {
    let exact = match (status.code(), status.message()) {
        (Code::Unavailable, "POLICY_UNAVAILABLE") => (503, "POLICY_UNAVAILABLE"),
        (Code::AlreadyExists, "POLICY_STALE") => (409, "POLICY_STALE"),
        (Code::AlreadyExists, "IDEMPOTENCY_CONFLICT") => (409, "IDEMPOTENCY_CONFLICT"),
        (Code::OutOfRange, "ATTEMPT_EXPIRED") => (410, "ATTEMPT_EXPIRED"),
        (Code::NotFound, "ATTEMPT_INVALID") => (404, "ATTEMPT_INVALID"),
        (Code::InvalidArgument, "ACKNOWLEDGEMENTS_REQUIRED") => (422, "ACKNOWLEDGEMENTS_REQUIRED"),
        (Code::InvalidArgument, "VERIFICATION_FAILED") => (422, "VERIFICATION_FAILED"),
        (Code::InvalidArgument, "INVALID_INPUT") => (422, "INVALID_INPUT"),
        (Code::PermissionDenied, "ACCOUNT_INACTIVE") => (403, "ACCOUNT_INACTIVE"),
        (Code::Unauthenticated, _) => (401, "AUTHENTICATION_REQUIRED"),
        (Code::PermissionDenied, _) => (403, "PERMISSION_DENIED"),
        (Code::DeadlineExceeded, _) => (504, "DEADLINE_EXCEEDED"),
        (Code::ResourceExhausted, _) => (429, "RATE_LIMITED"),
        _ => (503, "DEPENDENCY_UNAVAILABLE"),
    };
    EnrollmentFailure {
        status: exact.0,
        reason: exact.1,
    }
}
fn validate_response(
    operation: EnrollmentOperation,
    value: &Value,
) -> Result<(), EnrollmentFailure> {
    // Full closed schema is source-pinned; reject unexpected top-level identity
    // material before any anonymous reply can reach the browser.
    let expected: &[&str] = match operation {
        EnrollmentOperation::Policy => &[
            "contractVersion",
            "revision",
            "documents",
            "publishedAt",
            "expiresAt",
            "serverTime",
        ],
        EnrollmentOperation::Begin => &["attemptId", "policyRevision", "expiresAt", "serverTime"],
        EnrollmentOperation::Signup | EnrollmentOperation::Resend => &[
            "status",
            "attemptId",
            "expiresAt",
            "resendAvailableAt",
            "serverTime",
        ],
        EnrollmentOperation::Verify => &["status", "attemptId", "serverTime"],
        EnrollmentOperation::Admission | EnrollmentOperation::Consent => &[
            "decision",
            "policyRevision",
            "acceptedRevision",
            "serverTime",
        ],
    };
    let object = value
        .as_object()
        .ok_or(EnrollmentFailure::INVALID_RESPONSE)?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(EnrollmentFailure::INVALID_RESPONSE);
    }
    Ok(())
}
