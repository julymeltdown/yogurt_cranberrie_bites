use super::{
    InMemoryAuditRepository, InMemoryPasswordResetStore, InMemoryRefreshTokenStore,
    InMemoryUserRepository, InMemoryVerificationRepository, RecordingResetMailer,
};
use async_trait::async_trait;
use auth_service::application::auth_transactions::AuthTransactionsPort;
use auth_service::application::onboarding::AuthOnboardingPort;
use auth_service::application::password_work::{
    PasswordWork, PasswordWorkKind, PasswordWorkPort, PasswordWorkStats,
};
use auth_service::application::ports::{
    LoginAuditRepositoryPort, OAuthIdentityRepositoryPort, PasswordResetStorePort,
    RefreshTokenStorePort, ResetMailerPort, UserRepositoryPort, VerificationRepositoryPort,
};
use auth_service::domain::session::RotationOutcome;
use auth_service::domain::user::{User, UserStatus};
use chrono::{DateTime, Utc};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct SecurityTrace(Arc<Mutex<Vec<&'static str>>>);

impl SecurityTrace {
    async fn record(&self, event: &'static str) {
        self.0.lock().await.push(event);
    }

    pub async fn entries(&self) -> Vec<&'static str> {
        self.0.lock().await.clone()
    }
}

#[derive(Clone)]
pub struct InMemoryAuthTransactions {
    pub users: InMemoryUserRepository,
    pub verifications: InMemoryVerificationRepository,
    pub audits: InMemoryAuditRepository,
    pub refresh_tokens: InMemoryRefreshTokenStore,
    pub reset_tokens: InMemoryPasswordResetStore,
    pub mailer: RecordingResetMailer,
    pub calls: Arc<Mutex<Vec<&'static str>>>,
    trace: SecurityTrace,
}

impl InMemoryAuthTransactions {
    pub fn new(users: InMemoryUserRepository) -> Self {
        Self {
            users,
            verifications: InMemoryVerificationRepository::default(),
            audits: InMemoryAuditRepository::default(),
            refresh_tokens: InMemoryRefreshTokenStore::default(),
            reset_tokens: InMemoryPasswordResetStore::default(),
            mailer: RecordingResetMailer::default(),
            calls: Arc::new(Mutex::new(Vec::new())),
            trace: SecurityTrace::default(),
        }
    }

    pub fn with_verifications(mut self, value: InMemoryVerificationRepository) -> Self {
        self.verifications = value;
        self
    }

    pub fn with_audits(mut self, value: InMemoryAuditRepository) -> Self {
        self.audits = value;
        self
    }

    pub fn with_refresh_tokens(mut self, value: InMemoryRefreshTokenStore) -> Self {
        self.refresh_tokens = value;
        self
    }

    pub fn with_reset_tokens(mut self, value: InMemoryPasswordResetStore) -> Self {
        self.reset_tokens = value;
        self
    }

    pub fn with_mailer(mut self, value: RecordingResetMailer) -> Self {
        self.mailer = value;
        self
    }

    pub fn with_trace(mut self, trace: SecurityTrace) -> Self {
        self.trace = trace;
        self
    }

    async fn record(&self, name: &'static str) {
        self.calls.lock().await.push(name);
        self.trace.record(name).await;
    }
}

#[async_trait]
impl AuthOnboardingPort for InMemoryAuthTransactions {
    async fn resolve_oauth_user(
        &self,
        proof: &auth_service::application::dto::repository_dto::AuthOAuthProofDto,
    ) -> auth_service::application::errors::AuthResult<
        auth_service::application::dto::repository_dto::AuthUserRecordDto,
    > {
        let proof = &auth_service::domain::oauth_identity::OAuthUserInfo::from(proof.clone());
        let result: anyhow::Result<_> = async {
            self.record("resolve_oauth_user").await;
            let normalized_email = if proof.email.trim().is_empty() {
                format!("{}@oauth.local", proof.subject)
            } else {
                User::normalized_email(&proof.email)
            };
            if let Some(identity) = self
                .users
                .find_by_provider_subject((proof.provider).into(), &proof.subject)
                .await?
            {
                let user = self
                    .users
                    .find_by_id(identity.user_id)
                    .await?
                    .map(auth_service::domain::user::User::from)
                    .ok_or(auth_service::application::errors::AuthError::NotFound)?;
                if user.status != UserStatus::Active {
                    return Err(
                        auth_service::application::errors::AuthError::InvalidCredentials.into(),
                    );
                }
                let updated = user.with_last_login(Utc::now());
                self.users.save(&(&updated).into()).await?;
                return Ok(updated);
            }
            let identities =
                OAuthIdentityRepositoryPort::find_by_email(&self.users, &normalized_email).await?;
            if !proof.email_verified {
                if !identities.is_empty() {
                    return Err(auth_service::application::errors::AuthError::InvalidInput(
                        "Verified provider email is required to link an existing account",
                    )
                    .into());
                }
                let user = User {
                    id: Uuid::new_v4(),
                    primary_email: normalized_email.clone(),
                    password_hash: String::new(),
                    status: UserStatus::PendingVerification,
                    last_login_at: None,
                };
                self.users.save(&(&user).into()).await?;
                self.users
                    .link_oauth_identity(
                        user.id,
                        (proof.provider).into(),
                        &proof.subject,
                        &normalized_email,
                        false,
                    )
                    .await?;
                return Err(auth_service::application::errors::AuthError::InvalidInput(
                    "Email verification required",
                )
                .into());
            }
            let user = if identities.is_empty() {
                User {
                    id: Uuid::new_v4(),
                    primary_email: normalized_email.clone(),
                    password_hash: String::new(),
                    status: UserStatus::Active,
                    last_login_at: Some(Utc::now()),
                }
            } else {
                let user_id = identities[0].user_id;
                if identities
                    .iter()
                    .any(|identity| identity.user_id != user_id)
                {
                    anyhow::bail!("Email linked to multiple users");
                }
                let existing = self
                    .users
                    .find_by_id(user_id)
                    .await?
                    .map(auth_service::domain::user::User::from)
                    .ok_or_else(|| anyhow::anyhow!("User not found for identity"))?;
                if existing.status != UserStatus::Active {
                    return Err(
                        auth_service::application::errors::AuthError::InvalidCredentials.into(),
                    );
                }
                User {
                    primary_email: if existing.primary_email.is_empty() {
                        normalized_email.clone()
                    } else {
                        existing.primary_email.clone()
                    },
                    ..existing.with_last_login(Utc::now())
                }
            };
            self.users.save(&(&user).into()).await?;
            self.users
                .link_oauth_identity(
                    user.id,
                    (proof.provider).into(),
                    &proof.subject,
                    &normalized_email,
                    true,
                )
                .await?;
            Ok(user)
        }
        .await;
        result
            .map_err(auth_service::adapters::out::errors::boundary_error)
            .map(|value| value.into())
    }

    async fn resend_verification(
        &self,
        email: &str,
        code: &str,
    ) -> auth_service::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
            self.record("resend_verification").await;
            if let Some(user) = UserRepositoryPort::find_by_email(&self.users, email)
                .await?
                .map(auth_service::domain::user::User::from)
            {
                if user.status == UserStatus::PendingVerification {
                    self.verifications.save_code(user.id, email, code).await?;
                }
            }
            Ok(())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
}

#[async_trait]
impl AuthTransactionsPort for InMemoryAuthTransactions {
    async fn register_email(
        &self,
        user: &auth_service::application::dto::repository_dto::AuthUserRecordDto,
        code: &str,
    ) -> auth_service::application::errors::AuthResult<()> {
        let user = &auth_service::domain::user::User::from(user.clone());

        self.record("register_email").await;
        self.users.save(&(user).into()).await?;
        self.users
            .link_email_identity(user.id, &user.primary_email, false)
            .await?;
        self.verifications
            .save_code(user.id, &user.primary_email, code)
            .await
    }

    async fn verify_email(
        &self,
        email: &str,
        code: &str,
    ) -> auth_service::application::errors::AuthResult<Option<Uuid>> {
        let result: anyhow::Result<_> = async {
            self.record("verify_email").await;
            let Some(user_id) = self.verifications.consume_code(email, code).await? else {
                return Ok(None);
            };
            let Some(user) = self
                .users
                .find_by_id(user_id)
                .await?
                .map(auth_service::domain::user::User::from)
            else {
                return Ok(None);
            };
            if user.status != UserStatus::PendingVerification {
                return Ok(None);
            }
            self.users
                .save(&(&user.with_status(UserStatus::Active)).into())
                .await?;
            self.users.mark_email_verified(user_id, email).await?;
            Ok(Some(user_id))
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn reset_token_exists(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
    ) -> auth_service::application::errors::AuthResult<bool> {
        self.record("reset_token_exists").await;
        self.reset_tokens.exists(token_hash, now).await
    }

    async fn queue_password_reset(
        &self,
        user: &auth_service::application::dto::repository_dto::AuthUserRecordDto,
        token: &auth_service::application::dto::repository_dto::AuthPasswordResetRecordDto,
        link: &str,
    ) -> auth_service::application::errors::AuthResult<bool> {
        let user = &auth_service::domain::user::User::from(user.clone());
        let token = &auth_service::domain::password_reset::PasswordResetToken::from(token.clone());

        let result: anyhow::Result<_> = async {
            self.record("queue_password_reset").await;
            self.reset_tokens.save(&(token).into()).await?;
            self.mailer
                .send_reset_link(&user.primary_email, link)
                .await?;
            Ok(true)
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn reset_password(
        &self,
        token_hash: &str,
        password_hash: &str,
        now: DateTime<Utc>,
    ) -> auth_service::application::errors::AuthResult<Option<Uuid>> {
        let result: anyhow::Result<_> = async {
            self.record("reset_password").await;
            let Some(token) = self.reset_tokens.consume(token_hash, now).await? else {
                return Ok(None);
            };
            self.users
                .update_password_hash(token.user_id, password_hash)
                .await?;
            self.refresh_tokens
                .revoke_all_for_user(token.user_id)
                .await?;
            Ok(Some(token.user_id))
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn roles(
        &self,
        _user_id: Uuid,
    ) -> auth_service::application::errors::AuthResult<Vec<String>> {
        let result: anyhow::Result<_> = async {
            self.record("roles").await;
            Ok(vec!["ROLE_USER".into()])
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn begin_session(
        &self,
        expected: &auth_service::application::dto::repository_dto::AuthUserRecordDto,
        refresh: &auth_service::application::dto::repository_dto::AuthRefreshRecordDto,
        _roles: &[String],
        audit: &auth_service::application::dto::repository_dto::AuthLoginAuditRecordDto,
    ) -> auth_service::application::errors::AuthResult<bool> {
        let expected = &auth_service::domain::user::User::from(expected.clone());
        let refresh = &auth_service::domain::session::RefreshTokenRecord::from(refresh.clone());
        let audit = &auth_service::domain::session::LoginAudit::from(audit.clone());

        let result: anyhow::Result<_> = async {
            self.record("begin_session").await;
            let Some(current) = self
                .users
                .find_by_id(expected.id)
                .await?
                .map(auth_service::domain::user::User::from)
            else {
                return Ok(false);
            };
            if current.primary_email != expected.primary_email
                || current.password_hash != expected.password_hash
                || current.status != expected.status
                || current.status != UserStatus::Active
            {
                return Ok(false);
            }
            self.users
                .save(&(&current.with_last_login(audit.occurred_at)).into())
                .await?;
            self.refresh_tokens.store(&(refresh).into()).await?;
            self.audits.record(&(audit).into()).await?;
            Ok(true)
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn rotate_session(
        &self,
        expected: &auth_service::application::dto::repository_dto::AuthUserRecordDto,
        current_jti: &str,
        current_hash: &str,
        replacement: &auth_service::application::dto::repository_dto::AuthRefreshRecordDto,
        _roles: &[String],
    ) -> auth_service::application::errors::AuthResult<
        auth_service::application::dto::repository_dto::AuthRotationOutcomeDto,
    > {
        let expected = &auth_service::domain::user::User::from(expected.clone());
        let replacement =
            &auth_service::domain::session::RefreshTokenRecord::from(replacement.clone());
        self.record("rotate_session").await;
        let Some(current) = self
            .users
            .find_by_id(expected.id)
            .await?
            .map(auth_service::domain::user::User::from)
        else {
            return Ok(RotationOutcome::NotRotatable.into());
        };
        if current.primary_email != expected.primary_email
            || current.password_hash != expected.password_hash
            || current.status != UserStatus::Active
        {
            return Ok(RotationOutcome::NotRotatable.into());
        }
        self.refresh_tokens
            .rotate(current_jti, current_hash, &(replacement).into())
            .await
    }
}

#[derive(Clone, Default)]
pub struct RecordingPasswordWork {
    calls: Arc<Mutex<Vec<&'static str>>>,
    busy_signup: Arc<AtomicBool>,
    busy_reset: Arc<AtomicBool>,
    busy_login: Arc<AtomicBool>,
    equalized: Arc<AtomicUsize>,
    trace: SecurityTrace,
}

impl RecordingPasswordWork {
    pub fn as_port(&self) -> PasswordWork {
        Arc::new(self.clone())
    }

    pub fn with_trace(mut self, trace: SecurityTrace) -> Self {
        self.trace = trace;
        self
    }

    pub fn set_busy_signup(&self, value: bool) {
        self.busy_signup.store(value, Ordering::SeqCst);
    }

    pub fn set_busy_reset(&self, value: bool) {
        self.busy_reset.store(value, Ordering::SeqCst);
    }

    pub fn set_busy_login(&self, value: bool) {
        self.busy_login.store(value, Ordering::SeqCst);
    }

    pub async fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().await.clone()
    }

    pub fn equalized(&self) -> usize {
        self.equalized.load(Ordering::SeqCst)
    }

    async fn record(&self, call: &'static str) {
        self.calls.lock().await.push(call);
        self.trace.record(call).await;
    }
}

#[async_trait]
impl PasswordWorkPort for RecordingPasswordWork {
    async fn verify_login(
        &self,
        password: &str,
        _encoded_hash: &str,
    ) -> auth_service::application::errors::AuthResult<bool> {
        let result: anyhow::Result<_> = async {
            self.record("verify_login").await;
            if self.busy_login.load(Ordering::SeqCst) {
                return Err(auth_service::application::errors::AuthError::Busy.into());
            }
            Ok(password == "Passw0rd!")
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn equalize_login(
        &self,
        _password: &str,
    ) -> auth_service::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
            self.record("equalize_login").await;
            self.equalized.fetch_add(1, Ordering::SeqCst);
            if self.busy_login.load(Ordering::SeqCst) {
                return Err(auth_service::application::errors::AuthError::Busy.into());
            }
            Ok(())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn hash_for_signup(
        &self,
        _password: &str,
    ) -> auth_service::application::errors::AuthResult<String> {
        let result: anyhow::Result<_> = async {
            self.record("hash_for_signup").await;
            if self.busy_signup.load(Ordering::SeqCst) {
                return Err(auth_service::application::errors::AuthError::Busy.into());
            }
            Ok("recording-signup-hash".into())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn hash_for_reset(
        &self,
        _password: &str,
    ) -> auth_service::application::errors::AuthResult<String> {
        let result: anyhow::Result<_> = async {
            self.record("hash_for_reset").await;
            if self.busy_reset.load(Ordering::SeqCst) {
                return Err(auth_service::application::errors::AuthError::Busy.into());
            }
            Ok("recording-reset-hash".into())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    fn stats(&self, _kind: PasswordWorkKind) -> PasswordWorkStats {
        PasswordWorkStats::default()
    }
}

pub fn transaction_port(value: InMemoryAuthTransactions) -> Arc<dyn AuthTransactionsPort> {
    Arc::new(value)
}
