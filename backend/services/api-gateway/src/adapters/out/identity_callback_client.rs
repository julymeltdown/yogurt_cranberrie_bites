//! Signed Gateway client for the anonymous NAVER verification callback.
//!
//! The browser-facing callback (`GET /api/identity/naver/callback`) arrives from
//! the provider with no session, so there is no actor to delegate. The profile
//! service therefore serves it through `profile.identity.v1` — a service with no
//! actor fields, callable only by the Gateway workload — and reads the user from
//! the stored one-use state row.
//!
//! What this client deliberately does not do:
//!
//! * it never sends a user identity. The request message has no field for one,
//!   so a compromised gateway cannot name a victim;
//! * it never sends a claim value. The only inputs are the provider's own query
//!   parameters;
//! * it never trusts the reply's `return_path` as a URL. The reply is validated
//!   here against the same closed allowlist the profile service used at start
//!   time, so a compromised producer cannot turn the redirect into an open
//!   redirect.
use crate::domain::identity_verification::{CallbackReply, CallbackReplyError};
use platform::{
    edge::{EdgeBudget, FailurePolicy, RetryClass},
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use std::time::Duration;
use tonic::{service::interceptor::InterceptedService, transport::Channel, Status};

pub mod proto {
    tonic::include_proto!("profile.identity.v1");
}

const BUDGET: Duration = Duration::from_secs(5);
const MAX_REPLY: usize = 4096;
const EDGE: EdgeBudget = EdgeBudget {
    edge: "gateway.identity.callback",
    expected_latency: Duration::from_millis(200),
    deadline: BUDGET,
    connect_deadline: Duration::from_millis(500),
    retry: RetryClass::Never,
    failure_policy: FailurePolicy::FailRequest,
};

#[derive(Clone)]
pub struct GrpcIdentityCallbackClient {
    client: proto::identity_verification_service_client::IdentityVerificationServiceClient<
        InterceptedService<Channel, ServiceIdentityClientInterceptor>,
    >,
}

impl GrpcIdentityCallbackClient {
    pub fn connect(
        endpoint: &str,
        service_token: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
    ) -> anyhow::Result<Self> {
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            endpoint,
            Workload::Profile,
            EDGE.validate()?,
        )?;
        Ok(Self {
            client: proto::identity_verification_service_client::IdentityVerificationServiceClient::with_interceptor(
                channel,
                ServiceIdentityClientInterceptor::new(Workload::Gateway, service_token)?,
            )
            .max_decoding_message_size(MAX_REPLY)
            .max_encoding_message_size(MAX_REPLY),
        })
    }

    /// Complete one verification.
    ///
    /// `state` is the provider's query value. `code` and `provider_error` are the
    /// provider's own parameters; at most one of them is meaningful.
    pub async fn complete(
        &self,
        state: &str,
        code: Option<&str>,
        provider_error: Option<&str>,
    ) -> Result<CallbackReply, CallbackReplyError> {
        if state.is_empty() || state.len() > 512 {
            return Err(CallbackReplyError::InvalidState);
        }
        if code.is_some_and(|code| code.len() > 512)
            || provider_error.is_some_and(|error| error.len() > 128)
        {
            return Err(CallbackReplyError::InvalidState);
        }
        let mut request = tonic::Request::new(proto::CompleteIdentityVerificationRequest {
            state: state.to_owned(),
            code: code.unwrap_or_default().to_owned(),
            provider_error: provider_error.unwrap_or_default().to_owned(),
        });
        request.set_timeout(BUDGET);
        let reply = self
            .client
            .clone()
            .complete(request)
            .await
            .map_err(classify)?;
        let reply = reply.into_inner();
        CallbackReply::validate(reply.return_path, reply.result)
    }
}

/// Map a profile-service status to the gateway's closed callback vocabulary.
fn classify(status: Status) -> CallbackReplyError {
    match status.code() {
        tonic::Code::InvalidArgument => CallbackReplyError::InvalidState,
        tonic::Code::DeadlineExceeded => CallbackReplyError::Unavailable,
        _ => CallbackReplyError::Unavailable,
    }
}
