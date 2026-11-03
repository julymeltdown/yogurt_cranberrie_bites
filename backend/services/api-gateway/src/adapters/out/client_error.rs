/// Retain controlled categories and deliberate bounded client messages only.
/// Library causes, arbitrary status diagnostics and metadata do not cross the port.
pub fn classify_client_error(
    error: anyhow::Error,
) -> crate::application::client_error::GatewayClientError {
    use crate::application::client_error::{GatewayClientError as Failure, GatewayClientMessage};
    for cause in error.chain() {
        if let Some(failure) = cause.downcast_ref::<Failure>() {
            return failure.clone();
        }
        if let Some(status) = cause.downcast_ref::<tonic::Status>() {
            return match status.code() {
                tonic::Code::InvalidArgument => {
                    Failure::InvalidArgument(GatewayClientMessage::new(status.message()))
                }
                tonic::Code::Unauthenticated => Failure::Unauthenticated,
                tonic::Code::PermissionDenied => Failure::PermissionDenied,
                tonic::Code::NotFound => Failure::NotFound,
                tonic::Code::AlreadyExists => match status
                    .metadata()
                    .get(platform::mutation_context::ERROR_CODE)
                    .and_then(|value| value.to_str().ok())
                {
                    Some("unknown") => Failure::OperationOutcomeUnknown,
                    Some("expired") => Failure::OperationReplayExpired,
                    Some("conflict") => Failure::OperationConflict,
                    _ => Failure::AlreadyExists,
                },
                tonic::Code::Aborted => Failure::Aborted,
                tonic::Code::ResourceExhausted => Failure::RateLimited,
                tonic::Code::FailedPrecondition => Failure::FailedPrecondition,
                // Existing exact machine identifier; never guess from diagnostic substrings.
                tonic::Code::Unavailable if status.message() == "AUTH_BUSY" => Failure::Busy,
                tonic::Code::Unavailable => Failure::Unavailable,
                tonic::Code::DeadlineExceeded => Failure::DeadlineExceeded,
                _ => Failure::Unexpected,
            };
        }
    }
    Failure::Unexpected
}
