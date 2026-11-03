use super::server::proto;
use crate::application::use_cases::signup::validate_email;
use crate::config::AppState;
use crate::domain::user::User;
use tonic::metadata::MetadataValue;
use tonic::{Response, Status};

pub(super) async fn resend(
    state: &AppState,
    request: proto::ResendVerificationRequest,
) -> Result<Response<proto::ResendVerificationResponse>, Status> {
    let email = normalized_email(&request.email)?;
    // Do not use the legacy to_status mapper: database errors are not bad input
    // and their messages can contain SQL, constraints or account information.
    let accepted = state
        .signup
        .resend_verification(&email)
        .await
        .map_err(|_| {
            tracing::error!(
                operation = "resend_verification",
                "verification persistence failed"
            );
            Status::internal("Verification request failed")
        })?;
    if accepted.status != "ACCEPTED" {
        tracing::error!(
            operation = "resend_verification",
            "unexpected verification result"
        );
        return Err(Status::internal("Verification request failed"));
    }
    let mut response = Response::new(proto::ResendVerificationResponse {
        status: "ACCEPTED".into(),
    });
    response
        .metadata_mut()
        .insert("cache-control", MetadataValue::from_static("no-store"));
    Ok(response)
}

#[allow(clippy::result_large_err)]
fn normalized_email(value: &str) -> Result<String, Status> {
    let trimmed = value.trim();
    if trimmed.len() > 254 {
        return Err(Status::invalid_argument("Invalid email"));
    }
    let email = User::normalized_email(trimmed);
    validate_email(&email).map_err(|_| Status::invalid_argument("Invalid email"))?;
    Ok(email)
}

#[cfg(test)]
mod tests {
    use super::normalized_email;
    use tonic::Code;

    #[test]
    fn normalizes_like_the_http_and_application_paths() {
        assert_eq!(
            normalized_email("  MEMBER@EXAMPLE.INVALID  ").unwrap(),
            "member@example.invalid"
        );
    }

    #[test]
    fn validation_failures_do_not_echo_the_input() {
        for email in [
            "",
            "no-at-sign",
            "member@invalid",
            "member @example.invalid",
        ] {
            let error = normalized_email(email).unwrap_err();
            assert_eq!(error.code(), Code::InvalidArgument);
            assert_eq!(error.message(), "Invalid email");
        }
    }

    #[test]
    fn oversized_email_is_rejected_before_normalized_allocation() {
        let error = normalized_email(&format!("{}@example.invalid", "x".repeat(255))).unwrap_err();
        assert_eq!(error.code(), Code::InvalidArgument);
        assert_eq!(error.message(), "Invalid email");
    }
}
