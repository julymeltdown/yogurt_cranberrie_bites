use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResendVerificationRequest {
    pub email: String,
}

impl std::fmt::Debug for ResendVerificationRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResendVerificationRequest")
            .field("email", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerificationRequestStatus {
    Accepted,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResendVerificationResponse {
    pub status: VerificationRequestStatus,
}

impl ResendVerificationResponse {
    pub fn accepted() -> Self {
        Self {
            status: VerificationRequestStatus::Accepted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resend_verification_request_debug_redacts_email() {
        let request = ResendVerificationRequest {
            email: "private-verification-email@example.test".into(),
        };

        let rendered = format!("{request:?}");

        assert!(
            !rendered.contains("private-verification-email@example.test"),
            "{rendered}"
        );
        assert!(rendered.contains("[REDACTED]"));
    }

    #[test]
    fn accepted_serializes_without_account_or_delivery_information() {
        assert_eq!(
            serde_json::to_value(ResendVerificationResponse::accepted()).unwrap(),
            serde_json::json!({"status": "ACCEPTED"})
        );
    }

    #[test]
    fn unknown_status_is_not_a_success_response() {
        for status in ["SENT", "DELIVERED", "UNKNOWN", ""] {
            assert!(serde_json::from_value::<ResendVerificationResponse>(
                serde_json::json!({"status": status})
            )
            .is_err());
        }
    }

    #[test]
    fn request_rejects_unexpected_fields() {
        assert!(serde_json::from_value::<ResendVerificationRequest>(
            serde_json::json!({"email": "user@example.com", "userId": "other-user"})
        )
        .is_err());
    }
}
