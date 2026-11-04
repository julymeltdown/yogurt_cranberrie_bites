use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

pub const CONTRACT_VERSION: &str = "1.0";
pub const ATTEMPT_SECONDS: i64 = 1800;
pub const RESEND_SECONDS: i64 = 60;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyDocument {
    pub id: String,
    pub title: String,
    pub url: String,
    pub sha256: String,
    pub required_acknowledgement_id: String,
}

/// Operator input, never synthesized from the application's build or defaults.
/// approval_reference identifies the separately approved publication record; this
/// type and its validation do not claim that the referenced legal review occurred.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovedPublication {
    pub revision: Uuid,
    pub documents: Vec<PolicyDocument>,
    pub approval_reference: String,
    pub published_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl ApprovedPublication {
    pub fn validate(&self, now: DateTime<Utc>) -> Result<(), EnrollmentError> {
        if self.revision.is_nil()
            || self.documents.is_empty()
            || self.documents.len() > 8
            || self.approval_reference.trim().is_empty()
            || self.approval_reference.len() > 512
            || self.published_at > now
            || self.expires_at <= now
            || self.expires_at <= self.published_at
        {
            return Err(EnrollmentError::InvalidInput);
        }
        let mut ids = BTreeSet::new();
        let mut acknowledgements = BTreeSet::new();
        for doc in &self.documents {
            let url = reqwest::Url::parse(&doc.url).map_err(|_| EnrollmentError::InvalidInput)?;
            if !identifier(&doc.id)
                || !identifier(&doc.required_acknowledgement_id)
                || doc.title.trim().is_empty()
                || doc.title.len() > 240
                || url.scheme() != "https"
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || doc.url.len() > 2048
                || doc.sha256.len() != 64
                || !doc
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || !ids.insert(&doc.id)
                || !acknowledgements.insert(&doc.required_acknowledgement_id)
            {
                return Err(EnrollmentError::InvalidInput);
            }
        }
        Ok(())
    }

    pub fn check_acknowledgements(&self, supplied: &[String]) -> Result<(), EnrollmentError> {
        let expected: BTreeSet<_> = self
            .documents
            .iter()
            .map(|d| &d.required_acknowledgement_id)
            .collect();
        let actual: BTreeSet<_> = supplied.iter().collect();
        if actual.len() != supplied.len() || actual != expected {
            return Err(EnrollmentError::AcknowledgementsRequired);
        }
        Ok(())
    }
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CurrentPolicy {
    pub contract_version: String,
    pub revision: Uuid,
    pub documents: Vec<PolicyDocument>,
    pub published_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub server_time: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BeginRequest {
    pub policy_revision: Uuid,
}

// Credential-bearing inputs deliberately do not implement Debug.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignupRequest {
    pub attempt_id: Uuid,
    pub policy_revision: Uuid,
    pub acknowledgement_ids: Vec<String>,
    pub idempotency_key: Uuid,
    pub email: String,
    pub password: String,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifyRequest {
    pub attempt_id: Uuid,
    pub code: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResendRequest {
    pub attempt_id: Uuid,
    pub idempotency_key: Uuid,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConsentRequest {
    pub policy_revision: Uuid,
    pub acknowledgement_ids: Vec<String>,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttemptReceipt {
    pub attempt_id: Uuid,
    pub policy_revision: Uuid,
    pub expires_at: DateTime<Utc>,
    pub server_time: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptedReceipt {
    pub status: String,
    pub attempt_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub resend_available_at: DateTime<Utc>,
    pub server_time: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifiedReceipt {
    pub status: String,
    pub attempt_id: Uuid,
    pub server_time: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AdmissionDecision {
    Admitted,
    ConsentRequired,
    PolicyUnavailable,
    AccountInactive,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdmissionReceipt {
    pub decision: AdmissionDecision,
    pub policy_revision: Option<Uuid>,
    pub accepted_revision: Option<Uuid>,
    pub server_time: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EnrollmentError {
    #[error("POLICY_UNAVAILABLE")]
    PolicyUnavailable,
    #[error("POLICY_STALE")]
    PolicyStale,
    #[error("ATTEMPT_EXPIRED")]
    AttemptExpired,
    #[error("ATTEMPT_INVALID")]
    AttemptInvalid,
    #[error("ACKNOWLEDGEMENTS_REQUIRED")]
    AcknowledgementsRequired,
    #[error("IDEMPOTENCY_CONFLICT")]
    IdempotencyConflict,
    #[error("VERIFICATION_FAILED")]
    VerificationFailed,
    #[error("INVALID_INPUT")]
    InvalidInput,
    #[error("ACCOUNT_INACTIVE")]
    AccountInactive,
    #[error("DEPENDENCY_UNAVAILABLE")]
    Unavailable,
}
impl EnrollmentError {
    pub fn http_status(self) -> u16 {
        match self {
            Self::PolicyUnavailable | Self::Unavailable => 503,
            Self::PolicyStale | Self::IdempotencyConflict => 409,
            Self::AttemptExpired => 410,
            Self::AttemptInvalid => 404,
            Self::AccountInactive => 403,
            Self::AcknowledgementsRequired | Self::VerificationFailed | Self::InvalidInput => 422,
        }
    }
}
