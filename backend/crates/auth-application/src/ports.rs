use crate::dto::auth_dto::AuthLifecycleCleanupResult;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait UserRepositoryPort: Send + Sync {
    async fn find_by_email(
        &self,
        email: &str,
    ) -> crate::errors::AuthResult<Option<crate::dto::repository_dto::AuthUserRecordDto>>;
    async fn find_by_id(
        &self,
        user_id: Uuid,
    ) -> crate::errors::AuthResult<Option<crate::dto::repository_dto::AuthUserRecordDto>>;
    async fn save(
        &self,
        user: &crate::dto::repository_dto::AuthUserRecordDto,
    ) -> crate::errors::AuthResult<()>;
    async fn update_password_hash(
        &self,
        user_id: Uuid,
        password_hash: &str,
    ) -> crate::errors::AuthResult<()>;
    async fn link_email_identity(
        &self,
        user_id: Uuid,
        email: &str,
        verified: bool,
    ) -> crate::errors::AuthResult<()>;
    async fn mark_email_verified(
        &self,
        user_id: Uuid,
        email: &str,
    ) -> crate::errors::AuthResult<()>;
    async fn linked_providers(
        &self,
        user_id: Uuid,
    ) -> crate::errors::AuthResult<Vec<crate::dto::repository_dto::AuthIdentityProviderDto>>;
    async fn soft_delete_user(
        &self,
        user_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
    ) -> crate::errors::AuthResult<bool>;
}

#[async_trait]
pub trait OAuthIdentityRepositoryPort: Send + Sync {
    async fn find_by_provider_subject(
        &self,
        provider: crate::dto::repository_dto::AuthIdentityProviderDto,
        subject: &str,
    ) -> crate::errors::AuthResult<Option<crate::dto::repository_dto::AuthOAuthIdentityRecordDto>>;
    async fn find_by_email(
        &self,
        email: &str,
    ) -> crate::errors::AuthResult<Vec<crate::dto::repository_dto::AuthOAuthIdentityRecordDto>>;
    async fn link_oauth_identity(
        &self,
        user_id: Uuid,
        provider: crate::dto::repository_dto::AuthIdentityProviderDto,
        subject: &str,
        email: &str,
        email_verified: bool,
    ) -> crate::errors::AuthResult<()>;
}

#[async_trait]
pub trait OAuthAuthorizationStorePort: Send + Sync {
    async fn save(
        &self,
        transaction: &crate::dto::repository_dto::AuthOAuthAuthorizationRecordDto,
    ) -> crate::errors::AuthResult<()>;
    async fn consume(
        &self,
        provider: crate::dto::repository_dto::AuthIdentityProviderDto,
        state: &str,
        now: DateTime<Utc>,
    ) -> crate::errors::AuthResult<
        Option<crate::dto::repository_dto::AuthConsumedOAuthAuthorizationDto>,
    >;
}

#[async_trait]
pub trait VerificationRepositoryPort: Send + Sync {
    async fn save_code(
        &self,
        user_id: Uuid,
        email: &str,
        code: &str,
    ) -> crate::errors::AuthResult<()>;
    async fn consume_code(
        &self,
        email: &str,
        code: &str,
    ) -> crate::errors::AuthResult<Option<Uuid>>;
}

#[async_trait]
pub trait RefreshTokenStorePort: Send + Sync {
    async fn store(
        &self,
        record: &crate::dto::repository_dto::AuthRefreshRecordDto,
    ) -> crate::errors::AuthResult<()>;
    async fn find(
        &self,
        jti: &str,
    ) -> crate::errors::AuthResult<Option<crate::dto::repository_dto::AuthRefreshRecordDto>>;
    async fn rotate(
        &self,
        current_jti: &str,
        current_hash: &str,
        next: &crate::dto::repository_dto::AuthRefreshRecordDto,
    ) -> crate::errors::AuthResult<crate::dto::repository_dto::AuthRotationOutcomeDto>;
    /// Look up a token that is no longer active, to learn *why* it stopped.
    ///
    /// A missing token and a superseded one must be handled differently: the
    /// first is unknown, the second may be a benign concurrent refresh. Without
    /// this distinction the refresh path had to treat every unusual presentation
    /// as an attack and revoke every session the user owned.
    async fn find_state(
        &self,
        jti: &str,
        now: DateTime<Utc>,
    ) -> crate::errors::AuthResult<Option<crate::dto::repository_dto::AuthRefreshStateDto>>;
    /// End the session family identified by this token, including a successor
    /// committed by an overlapping refresh. Other devices remain active.
    async fn revoke(&self, jti: &str) -> crate::errors::AuthResult<()>;
    /// End one session family, leaving other logins of the same user intact.
    async fn revoke_family(&self, family_id: Uuid) -> crate::errors::AuthResult<()>;
    async fn revoke_all_for_user(&self, user_id: Uuid) -> crate::errors::AuthResult<()>;
    /// Whether `user_id` still holds an unrevoked, unexpired refresh token in
    /// `family_id`. Logout, reuse detection, and credential or role changes all
    /// revoke the live token, so this is false once a session has ended.
    async fn session_is_live(
        &self,
        user_id: Uuid,
        family_id: Uuid,
        now: DateTime<Utc>,
    ) -> crate::errors::AuthResult<bool>;
}

#[async_trait]
pub trait LoginAuditRepositoryPort: Send + Sync {
    async fn record(
        &self,
        audit: &crate::dto::repository_dto::AuthLoginAuditRecordDto,
    ) -> crate::errors::AuthResult<()>;
    async fn count_recent_failures(
        &self,
        user_id: Uuid,
        since: DateTime<Utc>,
    ) -> crate::errors::AuthResult<i64>;
}

#[async_trait]
pub trait AuthAuditRepositoryPort: Send + Sync {
    async fn record(
        &self,
        event: &crate::dto::repository_dto::AuthAuditEventRecordDto,
    ) -> crate::errors::AuthResult<()>;
}

#[async_trait]
pub trait PasswordResetStorePort: Send + Sync {
    async fn save(
        &self,
        token: &crate::dto::repository_dto::AuthPasswordResetRecordDto,
    ) -> crate::errors::AuthResult<()>;
    /// Report whether an unexpired token exists, without consuming it.
    ///
    /// Password hashing is expensive, so it must happen only after the token is
    /// known to be valid. An unauthenticated caller sending random tokens must
    /// not be able to spend the password-hashing budget.
    async fn exists(&self, token_hash: &str, now: DateTime<Utc>)
        -> crate::errors::AuthResult<bool>;
    async fn consume(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
    ) -> crate::errors::AuthResult<Option<crate::dto::repository_dto::AuthPasswordResetRecordDto>>;
}

#[async_trait]
pub trait ResetMailerPort: Send + Sync {
    async fn send_reset_link(&self, email: &str, link: &str) -> crate::errors::AuthResult<()>;
}

pub trait TokenServicePort: Send + Sync {
    /// Issue the tokens of a new session family.
    fn issue_token_pair(
        &self,
        subject: Uuid,
        roles: Vec<String>,
    ) -> crate::errors::AuthResult<crate::dto::token_dto::AuthTokenPairDto>;
    /// Issue replacement tokens that stay in the session family `session_id`.
    fn continue_session(
        &self,
        subject: Uuid,
        roles: Vec<String>,
        session_id: Uuid,
    ) -> crate::errors::AuthResult<crate::dto::token_dto::AuthTokenPairDto>;
    fn decode_access_token(
        &self,
        token: &str,
    ) -> crate::errors::AuthResult<crate::dto::token_dto::AuthDecodedAccessTokenDto>;
    fn decode_refresh_token(
        &self,
        token: &str,
    ) -> crate::errors::AuthResult<crate::dto::token_dto::AuthDecodedRefreshTokenDto>;
    fn jwk_set(&self) -> crate::dto::token_dto::AuthJwkSetDto;
}

#[async_trait]
pub trait OAuthProviderPort: Send + Sync {
    fn build_authorization_uri(
        &self,
        provider: crate::dto::repository_dto::AuthIdentityProviderDto,
        redirect_uri: &str,
        state: &str,
        code_challenge: &str,
        nonce: &str,
    ) -> crate::errors::AuthResult<String>;
    async fn fetch_user_info(
        &self,
        provider: crate::dto::repository_dto::AuthIdentityProviderDto,
        code: &str,
        redirect_uri: &str,
        code_verifier: Option<&str>,
        expected_nonce: &str,
    ) -> crate::errors::AuthResult<crate::dto::repository_dto::AuthOAuthProofDto>;
}

pub trait OAuthProviderMetricsPort: Send + Sync {
    fn record_provider_exchange(
        &self,
        provider: crate::dto::repository_dto::AuthIdentityProviderDto,
        outcome: &str,
    );
}

#[async_trait]
pub trait UserDeletionSagaPort: Send + Sync {
    async fn begin(
        &self,
        request: crate::dto::deletion_dto::AuthBeginUserDeletionDto,
    ) -> crate::errors::AuthResult<Option<crate::dto::deletion_dto::AuthUserDeletionSagaRecordDto>>;
    async fn find_by_user_id(
        &self,
        user_id: Uuid,
    ) -> crate::errors::AuthResult<Option<crate::dto::deletion_dto::AuthUserDeletionSagaRecordDto>>;
    async fn find_by_saga_id(
        &self,
        saga_id: Uuid,
    ) -> crate::errors::AuthResult<Option<crate::dto::deletion_dto::AuthUserDeletionSagaRecordDto>>;
    async fn list_dead_lettered(
        &self,
        after: Option<(DateTime<Utc>, Uuid)>,
        limit: i32,
    ) -> crate::errors::AuthResult<Vec<crate::dto::deletion_dto::AuthUserDeletionSagaRecordDto>>;
    async fn retry_dead_lettered(
        &self,
        request: crate::dto::deletion_dto::AuthRetryUserDeletionDto,
    ) -> crate::errors::AuthResult<Option<crate::dto::deletion_dto::AuthUserDeletionSagaRecordDto>>;
    async fn claim_due(
        &self,
        now: DateTime<Utc>,
        lease_timeout: chrono::Duration,
        limit: i32,
    ) -> crate::errors::AuthResult<Vec<crate::dto::deletion_dto::AuthUserDeletionStepRecordDto>>;
    async fn mark_step_succeeded(
        &self,
        step: &crate::dto::deletion_dto::AuthUserDeletionStepRecordDto,
        now: DateTime<Utc>,
    ) -> crate::errors::AuthResult<crate::dto::deletion_dto::AuthUserDeletionSagaRecordDto>;
    async fn mark_step_failed(
        &self,
        step: &crate::dto::deletion_dto::AuthUserDeletionStepRecordDto,
        error: &str,
        next_attempt_at: DateTime<Utc>,
        dead_lettered: bool,
        now: DateTime<Utc>,
    ) -> crate::errors::AuthResult<crate::dto::deletion_dto::AuthUserDeletionSagaRecordDto>;
}

#[async_trait]
pub trait UserDeletionDispatcherPort: Send + Sync {
    async fn apply(
        &self,
        target: crate::dto::deletion_dto::AuthUserDeletionTargetDto,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> crate::errors::AuthResult<()>;
}

#[async_trait]
pub trait AuthLifecycleCleanupPort: Send + Sync {
    async fn cleanup_expired(
        &self,
        now: DateTime<Utc>,
    ) -> crate::errors::AuthResult<AuthLifecycleCleanupResult>;
}

#[async_trait]
impl<T> UserRepositoryPort for Arc<T>
where
    T: UserRepositoryPort + ?Sized,
{
    async fn find_by_email(
        &self,
        email: &str,
    ) -> crate::errors::AuthResult<Option<crate::dto::repository_dto::AuthUserRecordDto>> {
        (**self).find_by_email(email).await
    }

    async fn find_by_id(
        &self,
        user_id: Uuid,
    ) -> crate::errors::AuthResult<Option<crate::dto::repository_dto::AuthUserRecordDto>> {
        (**self).find_by_id(user_id).await
    }

    async fn save(
        &self,
        user: &crate::dto::repository_dto::AuthUserRecordDto,
    ) -> crate::errors::AuthResult<()> {
        (**self).save(user).await
    }

    async fn update_password_hash(
        &self,
        user_id: Uuid,
        password_hash: &str,
    ) -> crate::errors::AuthResult<()> {
        (**self).update_password_hash(user_id, password_hash).await
    }

    async fn link_email_identity(
        &self,
        user_id: Uuid,
        email: &str,
        verified: bool,
    ) -> crate::errors::AuthResult<()> {
        (**self).link_email_identity(user_id, email, verified).await
    }

    async fn mark_email_verified(
        &self,
        user_id: Uuid,
        email: &str,
    ) -> crate::errors::AuthResult<()> {
        (**self).mark_email_verified(user_id, email).await
    }

    async fn linked_providers(
        &self,
        user_id: Uuid,
    ) -> crate::errors::AuthResult<Vec<crate::dto::repository_dto::AuthIdentityProviderDto>> {
        (**self).linked_providers(user_id).await
    }

    async fn soft_delete_user(
        &self,
        user_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
    ) -> crate::errors::AuthResult<bool> {
        (**self)
            .soft_delete_user(user_id, actor_id, reason, retention_until)
            .await
    }
}

#[async_trait]
impl<T> VerificationRepositoryPort for Arc<T>
where
    T: VerificationRepositoryPort + ?Sized,
{
    async fn save_code(
        &self,
        user_id: Uuid,
        email: &str,
        code: &str,
    ) -> crate::errors::AuthResult<()> {
        (**self).save_code(user_id, email, code).await
    }

    async fn consume_code(
        &self,
        email: &str,
        code: &str,
    ) -> crate::errors::AuthResult<Option<Uuid>> {
        (**self).consume_code(email, code).await
    }
}

#[async_trait]
impl<T> OAuthIdentityRepositoryPort for Arc<T>
where
    T: OAuthIdentityRepositoryPort + ?Sized,
{
    async fn find_by_provider_subject(
        &self,
        provider: crate::dto::repository_dto::AuthIdentityProviderDto,
        subject: &str,
    ) -> crate::errors::AuthResult<Option<crate::dto::repository_dto::AuthOAuthIdentityRecordDto>>
    {
        (**self).find_by_provider_subject(provider, subject).await
    }

    async fn find_by_email(
        &self,
        email: &str,
    ) -> crate::errors::AuthResult<Vec<crate::dto::repository_dto::AuthOAuthIdentityRecordDto>>
    {
        (**self).find_by_email(email).await
    }

    async fn link_oauth_identity(
        &self,
        user_id: Uuid,
        provider: crate::dto::repository_dto::AuthIdentityProviderDto,
        subject: &str,
        email: &str,
        email_verified: bool,
    ) -> crate::errors::AuthResult<()> {
        (**self)
            .link_oauth_identity(user_id, provider, subject, email, email_verified)
            .await
    }
}

#[async_trait]
impl<T> OAuthAuthorizationStorePort for Arc<T>
where
    T: OAuthAuthorizationStorePort + ?Sized,
{
    async fn save(
        &self,
        transaction: &crate::dto::repository_dto::AuthOAuthAuthorizationRecordDto,
    ) -> crate::errors::AuthResult<()> {
        (**self).save(transaction).await
    }

    async fn consume(
        &self,
        provider: crate::dto::repository_dto::AuthIdentityProviderDto,
        state: &str,
        now: DateTime<Utc>,
    ) -> crate::errors::AuthResult<
        Option<crate::dto::repository_dto::AuthConsumedOAuthAuthorizationDto>,
    > {
        (**self).consume(provider, state, now).await
    }
}

#[async_trait]
impl<T> RefreshTokenStorePort for Arc<T>
where
    T: RefreshTokenStorePort + ?Sized,
{
    async fn store(
        &self,
        record: &crate::dto::repository_dto::AuthRefreshRecordDto,
    ) -> crate::errors::AuthResult<()> {
        (**self).store(record).await
    }

    async fn find(
        &self,
        jti: &str,
    ) -> crate::errors::AuthResult<Option<crate::dto::repository_dto::AuthRefreshRecordDto>> {
        (**self).find(jti).await
    }

    async fn rotate(
        &self,
        current_jti: &str,
        current_hash: &str,
        next: &crate::dto::repository_dto::AuthRefreshRecordDto,
    ) -> crate::errors::AuthResult<crate::dto::repository_dto::AuthRotationOutcomeDto> {
        (**self).rotate(current_jti, current_hash, next).await
    }

    async fn find_state(
        &self,
        jti: &str,
        now: DateTime<Utc>,
    ) -> crate::errors::AuthResult<Option<crate::dto::repository_dto::AuthRefreshStateDto>> {
        (**self).find_state(jti, now).await
    }

    async fn revoke(&self, jti: &str) -> crate::errors::AuthResult<()> {
        (**self).revoke(jti).await
    }

    async fn revoke_family(&self, family_id: Uuid) -> crate::errors::AuthResult<()> {
        (**self).revoke_family(family_id).await
    }

    async fn revoke_all_for_user(&self, user_id: Uuid) -> crate::errors::AuthResult<()> {
        (**self).revoke_all_for_user(user_id).await
    }

    async fn session_is_live(
        &self,
        user_id: Uuid,
        family_id: Uuid,
        now: DateTime<Utc>,
    ) -> crate::errors::AuthResult<bool> {
        (**self).session_is_live(user_id, family_id, now).await
    }
}

#[async_trait]
impl<T> LoginAuditRepositoryPort for Arc<T>
where
    T: LoginAuditRepositoryPort + ?Sized,
{
    async fn record(
        &self,
        audit: &crate::dto::repository_dto::AuthLoginAuditRecordDto,
    ) -> crate::errors::AuthResult<()> {
        (**self).record(audit).await
    }

    async fn count_recent_failures(
        &self,
        user_id: Uuid,
        since: DateTime<Utc>,
    ) -> crate::errors::AuthResult<i64> {
        (**self).count_recent_failures(user_id, since).await
    }
}

#[async_trait]
impl<T> AuthAuditRepositoryPort for Arc<T>
where
    T: AuthAuditRepositoryPort + ?Sized,
{
    async fn record(
        &self,
        event: &crate::dto::repository_dto::AuthAuditEventRecordDto,
    ) -> crate::errors::AuthResult<()> {
        (**self).record(event).await
    }
}

#[async_trait]
impl<T> PasswordResetStorePort for Arc<T>
where
    T: PasswordResetStorePort + ?Sized,
{
    async fn save(
        &self,
        token: &crate::dto::repository_dto::AuthPasswordResetRecordDto,
    ) -> crate::errors::AuthResult<()> {
        (**self).save(token).await
    }

    async fn exists(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
    ) -> crate::errors::AuthResult<bool> {
        (**self).exists(token_hash, now).await
    }

    async fn consume(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
    ) -> crate::errors::AuthResult<Option<crate::dto::repository_dto::AuthPasswordResetRecordDto>>
    {
        (**self).consume(token_hash, now).await
    }
}

#[async_trait]
impl<T> ResetMailerPort for Arc<T>
where
    T: ResetMailerPort + ?Sized,
{
    async fn send_reset_link(&self, email: &str, link: &str) -> crate::errors::AuthResult<()> {
        (**self).send_reset_link(email, link).await
    }
}

impl<T> TokenServicePort for Arc<T>
where
    T: TokenServicePort + ?Sized,
{
    fn issue_token_pair(
        &self,
        subject: Uuid,
        roles: Vec<String>,
    ) -> crate::errors::AuthResult<crate::dto::token_dto::AuthTokenPairDto> {
        (**self).issue_token_pair(subject, roles)
    }

    fn continue_session(
        &self,
        subject: Uuid,
        roles: Vec<String>,
        session_id: Uuid,
    ) -> crate::errors::AuthResult<crate::dto::token_dto::AuthTokenPairDto> {
        (**self).continue_session(subject, roles, session_id)
    }

    fn decode_access_token(
        &self,
        token: &str,
    ) -> crate::errors::AuthResult<crate::dto::token_dto::AuthDecodedAccessTokenDto> {
        (**self).decode_access_token(token)
    }

    fn decode_refresh_token(
        &self,
        token: &str,
    ) -> crate::errors::AuthResult<crate::dto::token_dto::AuthDecodedRefreshTokenDto> {
        (**self).decode_refresh_token(token)
    }

    fn jwk_set(&self) -> crate::dto::token_dto::AuthJwkSetDto {
        (**self).jwk_set()
    }
}

#[async_trait]
impl<T> OAuthProviderPort for Arc<T>
where
    T: OAuthProviderPort + ?Sized,
{
    fn build_authorization_uri(
        &self,
        provider: crate::dto::repository_dto::AuthIdentityProviderDto,
        redirect_uri: &str,
        state: &str,
        code_challenge: &str,
        nonce: &str,
    ) -> crate::errors::AuthResult<String> {
        (**self).build_authorization_uri(provider, redirect_uri, state, code_challenge, nonce)
    }

    async fn fetch_user_info(
        &self,
        provider: crate::dto::repository_dto::AuthIdentityProviderDto,
        code: &str,
        redirect_uri: &str,
        code_verifier: Option<&str>,
        expected_nonce: &str,
    ) -> crate::errors::AuthResult<crate::dto::repository_dto::AuthOAuthProofDto> {
        (**self)
            .fetch_user_info(provider, code, redirect_uri, code_verifier, expected_nonce)
            .await
    }
}

impl<T> OAuthProviderMetricsPort for Arc<T>
where
    T: OAuthProviderMetricsPort + ?Sized,
{
    fn record_provider_exchange(
        &self,
        provider: crate::dto::repository_dto::AuthIdentityProviderDto,
        outcome: &str,
    ) {
        (**self).record_provider_exchange(provider, outcome);
    }
}

#[async_trait]
impl<T> AuthLifecycleCleanupPort for Arc<T>
where
    T: AuthLifecycleCleanupPort + ?Sized,
{
    async fn cleanup_expired(
        &self,
        now: DateTime<Utc>,
    ) -> crate::errors::AuthResult<AuthLifecycleCleanupResult> {
        (**self).cleanup_expired(now).await
    }
}

#[async_trait]
impl<T> UserDeletionSagaPort for Arc<T>
where
    T: UserDeletionSagaPort + ?Sized,
{
    async fn begin(
        &self,
        request: crate::dto::deletion_dto::AuthBeginUserDeletionDto,
    ) -> crate::errors::AuthResult<Option<crate::dto::deletion_dto::AuthUserDeletionSagaRecordDto>>
    {
        (**self).begin(request).await
    }
    async fn find_by_user_id(
        &self,
        user_id: Uuid,
    ) -> crate::errors::AuthResult<Option<crate::dto::deletion_dto::AuthUserDeletionSagaRecordDto>>
    {
        (**self).find_by_user_id(user_id).await
    }
    async fn find_by_saga_id(
        &self,
        saga_id: Uuid,
    ) -> crate::errors::AuthResult<Option<crate::dto::deletion_dto::AuthUserDeletionSagaRecordDto>>
    {
        (**self).find_by_saga_id(saga_id).await
    }
    async fn list_dead_lettered(
        &self,
        after: Option<(DateTime<Utc>, Uuid)>,
        limit: i32,
    ) -> crate::errors::AuthResult<Vec<crate::dto::deletion_dto::AuthUserDeletionSagaRecordDto>>
    {
        (**self).list_dead_lettered(after, limit).await
    }
    async fn retry_dead_lettered(
        &self,
        request: crate::dto::deletion_dto::AuthRetryUserDeletionDto,
    ) -> crate::errors::AuthResult<Option<crate::dto::deletion_dto::AuthUserDeletionSagaRecordDto>>
    {
        (**self).retry_dead_lettered(request).await
    }
    async fn claim_due(
        &self,
        now: DateTime<Utc>,
        lease_timeout: chrono::Duration,
        limit: i32,
    ) -> crate::errors::AuthResult<Vec<crate::dto::deletion_dto::AuthUserDeletionStepRecordDto>>
    {
        (**self).claim_due(now, lease_timeout, limit).await
    }
    async fn mark_step_succeeded(
        &self,
        step: &crate::dto::deletion_dto::AuthUserDeletionStepRecordDto,
        now: DateTime<Utc>,
    ) -> crate::errors::AuthResult<crate::dto::deletion_dto::AuthUserDeletionSagaRecordDto> {
        (**self).mark_step_succeeded(step, now).await
    }
    async fn mark_step_failed(
        &self,
        step: &crate::dto::deletion_dto::AuthUserDeletionStepRecordDto,
        error: &str,
        next_attempt_at: DateTime<Utc>,
        dead_lettered: bool,
        now: DateTime<Utc>,
    ) -> crate::errors::AuthResult<crate::dto::deletion_dto::AuthUserDeletionSagaRecordDto> {
        (**self)
            .mark_step_failed(step, error, next_attempt_at, dead_lettered, now)
            .await
    }
}

#[async_trait]
impl<T> UserDeletionDispatcherPort for Arc<T>
where
    T: UserDeletionDispatcherPort + ?Sized,
{
    async fn apply(
        &self,
        target: crate::dto::deletion_dto::AuthUserDeletionTargetDto,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> crate::errors::AuthResult<()> {
        (**self)
            .apply(target, saga_id, user_id, actor_id, retention_until)
            .await
    }
}
