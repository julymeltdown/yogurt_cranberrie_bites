//! The internal callback transport.
//!
//! The browser-facing callback is anonymous, so it cannot use the profile
//! product transport (whose every message is bound to a verified actor). This
//! module is the narrow profile-owned counterpart of exactly one operation.
//!
//! The security shape is deliberate and small:
//!
//! * the method policy admits only the Gateway workload, and declares no actor
//!   fields, because there is no actor: the user is read from the stored state;
//! * the request carries only the provider's own query parameters. It cannot
//!   name a user, a claim, an age or a gender, so a compromised Gateway cannot
//!   mint a claim for an arbitrary user beyond what a real provider callback
//!   could;
//! * the reply carries only a result marker and the stored return path. Neither
//!   is a claim value.
use crate::identity::{CallbackError, CallbackResult, IdentityRuntime};
use platform::grpc_auth::{authorize_call, RpcAction};
use platform::grpc_method_policy::MethodPolicy;
use tonic::{Request, Response, Status};

pub mod proto {
    tonic::include_proto!("profile.identity.v1");
}

/// Only the Gateway may complete a verification, and the method acts for no
/// declared actor: the state row names the user.
pub const POLICY: MethodPolicy = MethodPolicy {
    service: "profile.identity.v1.IdentityVerificationService",
    workload: Some(platform::grpc_auth::Workload::Profile),
    methods: &[("Complete", &[platform::grpc_auth::Workload::Gateway])],
    actor_fields: &[],
};

#[derive(Clone)]
pub struct IdentityVerificationServer {
    runtime: std::sync::Arc<IdentityRuntime>,
}

impl IdentityVerificationServer {
    pub fn new(runtime: std::sync::Arc<IdentityRuntime>) -> Self {
        Self { runtime }
    }
}

#[tonic::async_trait]
impl proto::identity_verification_service_server::IdentityVerificationService
    for IdentityVerificationServer
{
    async fn complete(
        &self,
        request: Request<proto::CompleteIdentityVerificationRequest>,
    ) -> Result<Response<proto::CompleteIdentityVerificationReply>, Status> {
        authorize_call(&request, RpcAction::VerificationCallback)?;
        let command = request.into_inner();
        // The provider query is bounded before it reaches the store or the
        // provider client. A state longer than the contract admits cannot be a
        // state this service issued.
        if command.state.is_empty()
            || command.state.len() > 512
            || command.code.len() > 512
            || command.provider_error.len() > 128
        {
            return Err(Status::invalid_argument(
                "Invalid identity verification state",
            ));
        }
        let outcome = self
            .runtime
            .complete()
            .complete(crate::profile_product::targets::callback_command(
                &command.state,
                Some(command.code.as_str()).filter(|code| !code.is_empty()),
                Some(command.provider_error.as_str()).filter(|error| !error.is_empty()),
                chrono::Utc::now(),
            ))
            .await
            .map_err(callback_status)?;
        Ok(Response::new(proto::CompleteIdentityVerificationReply {
            return_path: outcome.return_path,
            result: outcome.result.as_str().to_owned(),
        }))
    }
}

/// Map a callback refusal to a closed gRPC status.
///
/// `InvalidState` is deliberately the same status and the same message for an
/// unknown state, an expired state and a replayed state that cannot be resolved
/// to a return path: the gateway answers `400` and the browser learns nothing
/// about which case it was.
fn callback_status(error: CallbackError) -> Status {
    match error {
        CallbackError::InvalidState => {
            Status::invalid_argument("Invalid identity verification state")
        }
        CallbackError::Store => Status::unavailable("Identity verification is unavailable"),
    }
}

/// Whether a result marker is one this service can produce.
///
/// Used by the gateway to validate a reply before it becomes a `Location`, so a
/// compromised or buggy producer cannot inject an arbitrary string into a
/// redirect.
pub fn is_known_result(value: &str) -> bool {
    [
        CallbackResult::Success,
        CallbackResult::Duplicate,
        CallbackResult::ProfileIncomplete,
        CallbackResult::AgeNotSupported,
        CallbackResult::Failed,
        CallbackResult::Cancelled,
        CallbackResult::Expired,
        CallbackResult::Replayed,
    ]
    .into_iter()
    .any(|result| result.as_str() == value)
}
