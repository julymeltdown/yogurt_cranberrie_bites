//! Atomic persistence boundaries for security-sensitive authentication changes.
//! Implementations must lock the user before token rows, and commit audit and
//! durable delivery intents in the same transaction as the state transition.

use crate::onboarding::AuthOnboardingPort;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[async_trait]
pub trait AuthTransactionsPort: AuthOnboardingPort + Send + Sync {
    async fn register_email(
        &self,
        user: &crate::dto::repository_dto::AuthUserRecordDto,
        code: &str,
    ) -> crate::errors::AuthResult<()>;
    async fn verify_email(
        &self,
        email: &str,
        code: &str,
    ) -> crate::errors::AuthResult<Option<Uuid>>;
    /// Report whether an unexpired reset token exists, without consuming it.
    ///
    /// The caller checks this before hashing a candidate password, so random
    /// tokens cannot spend the shared password-hashing budget.
    async fn reset_token_exists(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
    ) -> crate::errors::AuthResult<bool>;
    async fn queue_password_reset(
        &self,
        user: &crate::dto::repository_dto::AuthUserRecordDto,
        token: &crate::dto::repository_dto::AuthPasswordResetRecordDto,
        link: &str,
    ) -> crate::errors::AuthResult<bool>;
    async fn reset_password(
        &self,
        token_hash: &str,
        password_hash: &str,
        now: DateTime<Utc>,
    ) -> crate::errors::AuthResult<Option<Uuid>>;
    async fn roles(&self, user_id: Uuid) -> crate::errors::AuthResult<Vec<String>>;
    async fn begin_session(
        &self,
        expected: &crate::dto::repository_dto::AuthUserRecordDto,
        refresh: &crate::dto::repository_dto::AuthRefreshRecordDto,
        roles: &[String],
        audit: &crate::dto::repository_dto::AuthLoginAuditRecordDto,
    ) -> crate::errors::AuthResult<bool>;
    async fn rotate_session(
        &self,
        expected: &crate::dto::repository_dto::AuthUserRecordDto,
        current_jti: &str,
        current_hash: &str,
        replacement: &crate::dto::repository_dto::AuthRefreshRecordDto,
        roles: &[String],
    ) -> crate::errors::AuthResult<crate::dto::repository_dto::AuthRotationOutcomeDto>;
}
