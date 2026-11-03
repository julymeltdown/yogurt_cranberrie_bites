use crate::auth_runtime::AuthRuntimePort;
use crate::auth_transactions::AuthTransactionsPort;
use crate::dto::auth_dto::AuthResponseDto;
use crate::errors::AuthError;
use crate::password_work::PasswordWork;
use crate::ports::{
    LoginAuditRepositoryPort, RefreshTokenStorePort, TokenServicePort, UserRepositoryPort,
};
use auth_domain::session::{
    AuthSession, AuthTokenPair, DecodedRefreshToken, LoginAudit, RefreshTokenRecord,
    RefreshTokenState, RotationOutcome, SessionStatus,
};
use auth_domain::user::{IdentityProvider, User, UserStatus};
use base64ct::{Base64, Encoding};
use chrono::Duration;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

const DEFAULT_LOCKOUT_MAX_FAILURES: u32 = 5;
const DEFAULT_LOCKOUT_WINDOW_SECONDS: i64 = 900;
const DEFAULT_REFRESH_GRACE_SECONDS: i64 = 30;

#[derive(Clone)]
pub struct LoginUseCase<R, A, S, T> {
    user_repository: R,
    login_audit_repository: A,
    refresh_token_store: S,
    token_service: T,
    transactions: Arc<dyn AuthTransactionsPort>,
    password_work: PasswordWork,
    runtime: Arc<dyn AuthRuntimePort>,
    lockout_policy: LoginLockoutPolicy,
    refresh_grace_window: Duration,
}

#[derive(Clone)]
pub struct LoginUseCaseDeps<R, A, S, T> {
    pub user_repository: R,
    pub login_audit_repository: A,
    pub refresh_token_store: S,
    pub token_service: T,
    pub transactions: Arc<dyn AuthTransactionsPort>,
    pub password_work: PasswordWork,
    pub runtime: Arc<dyn AuthRuntimePort>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoginLockoutPolicy {
    pub max_failures: u32,
    pub window: Duration,
}

impl Default for LoginLockoutPolicy {
    fn default() -> Self {
        Self {
            max_failures: DEFAULT_LOCKOUT_MAX_FAILURES,
            window: Duration::seconds(DEFAULT_LOCKOUT_WINDOW_SECONDS),
        }
    }
}

impl<R, A, S, T> LoginUseCase<R, A, S, T>
where
    R: UserRepositoryPort + Clone,
    A: LoginAuditRepositoryPort + Clone,
    S: RefreshTokenStorePort + Clone,
    T: TokenServicePort + Clone,
{
    pub fn new(deps: LoginUseCaseDeps<R, A, S, T>) -> Self {
        Self::new_with_lockout_policy(deps, LoginLockoutPolicy::default())
    }

    pub fn new_with_lockout_policy(
        deps: LoginUseCaseDeps<R, A, S, T>,
        lockout_policy: LoginLockoutPolicy,
    ) -> Self {
        Self {
            user_repository: deps.user_repository,
            login_audit_repository: deps.login_audit_repository,
            refresh_token_store: deps.refresh_token_store,
            token_service: deps.token_service,
            transactions: deps.transactions,
            password_work: deps.password_work,
            runtime: deps.runtime,
            lockout_policy,
            refresh_grace_window: Duration::seconds(DEFAULT_REFRESH_GRACE_SECONDS),
        }
    }

    #[must_use]
    pub fn with_refresh_grace_window(mut self, grace_window: Duration) -> Self {
        self.refresh_grace_window = grace_window;
        self
    }

    pub async fn login(
        &self,
        email: &str,
        password: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> crate::errors::AuthResult<AuthResponseDto> {
        let normalized_email = User::normalized_email(email);
        let user = self
            .user_repository
            .find_by_email(&normalized_email)
            .await?
            .map(User::from);
        let Some(user) = user else {
            self.password_work.equalize_login(password).await?;
            return Err(AuthError::InvalidCredentials);
        };
        if user.status != UserStatus::Active {
            self.password_work.equalize_login(password).await?;
            self.record_audit(LoginAudit::failure(
                self.runtime.new_id(),
                user.id,
                ip_address,
                user_agent,
                self.runtime.now(),
            ))
            .await?;
            return Err(AuthError::InvalidCredentials);
        }
        if self.is_locked_out(user.id).await? {
            self.password_work.equalize_login(password).await?;
            self.record_audit(LoginAudit::lockout_refusal(
                self.runtime.new_id(),
                user.id,
                ip_address,
                user_agent,
                self.runtime.now(),
            ))
            .await?;
            return Err(AuthError::InvalidCredentials);
        }
        if !self
            .password_work
            .verify_login(password, &user.password_hash)
            .await?
        {
            self.record_audit(LoginAudit::failure(
                self.runtime.new_id(),
                user.id,
                ip_address,
                user_agent,
                self.runtime.now(),
            ))
            .await?;
            return Err(AuthError::InvalidCredentials);
        }

        // Password verification finishes before the database transaction opens.
        // The transaction then rechecks the credential/status/role snapshot.
        let roles = self.transactions.roles(user.id).await?;
        let linked_providers = self
            .user_repository
            .linked_providers(user.id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
        let tokens = self
            .token_service
            .issue_token_pair(user.id, roles.clone())
            .map(AuthTokenPair::from)?;
        let refresh_record = RefreshTokenRecord::new_family(
            uuid::Uuid::parse_str(&tokens.refresh_jti).unwrap_or_else(|_| self.runtime.new_id()),
            tokens.refresh_jti.clone(),
            hash_token(&tokens.refresh_token),
            user.id,
            tokens.refresh_expires_at,
        );
        let audit = LoginAudit::success(
            self.runtime.new_id(),
            user.id,
            ip_address,
            user_agent,
            self.runtime.now(),
        );
        if !self
            .transactions
            .begin_session(
                &(&user).into(),
                &(&refresh_record).into(),
                &roles,
                &(&audit).into(),
            )
            .await?
        {
            return Err(AuthError::Invariant(
                "User state changed; retry authentication",
            ));
        }
        Ok(to_response(
            AuthSession {
                user_id: user.id,
                tokens,
            },
            linked_providers,
        ))
    }

    pub async fn refresh(&self, refresh_token: &str) -> crate::errors::AuthResult<AuthResponseDto> {
        let decoded: DecodedRefreshToken = self
            .token_service
            .decode_refresh_token(refresh_token)
            .map(DecodedRefreshToken::from)?;
        let presented_hash = hash_token(refresh_token);
        let existing = self
            .refresh_token_store
            .find(&decoded.jti)
            .await?
            .map(RefreshTokenRecord::from);
        let Some(existing) = existing else {
            return self.classify_inactive_token(&decoded).await;
        };
        if existing.token_hash != presented_hash || existing.user_id != decoded.subject {
            self.refresh_token_store
                .revoke_family(existing.family_id)
                .await?;
            return Err(AuthError::CredentialRejected);
        }
        let user = match self
            .user_repository
            .find_by_id(decoded.subject)
            .await?
            .map(User::from)
        {
            Some(user) if user.status == UserStatus::Active => user,
            _ => {
                self.refresh_token_store
                    .revoke_all_for_user(decoded.subject)
                    .await?;
                return Err(AuthError::CredentialRejected);
            }
        };
        let roles = self.transactions.roles(user.id).await?;
        let linked_providers = self
            .user_repository
            .linked_providers(user.id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
        let tokens = self
            .token_service
            .continue_session(user.id, roles.clone(), existing.family_id)
            .map(AuthTokenPair::from)?;
        let replacement = RefreshTokenRecord::successor_of(
            existing.family_id,
            tokens.refresh_jti.clone(),
            hash_token(&tokens.refresh_token),
            user.id,
            tokens.refresh_expires_at,
        );
        let outcome = self
            .transactions
            .rotate_session(
                &(&user).into(),
                &decoded.jti,
                &presented_hash,
                &(&replacement).into(),
                &roles,
            )
            .await?;
        match RotationOutcome::from(outcome) {
            RotationOutcome::Rotated => {}
            RotationOutcome::AlreadyRotated {
                rotated_at,
                successor_jti,
            } => {
                if self.runtime.now().signed_duration_since(rotated_at) <= self.refresh_grace_window
                {
                    tracing::info!(
                        family_id = %existing.family_id,
                        successor_jti = %successor_jti,
                        "concurrent refresh lost the rotation race; caller should retry"
                    );
                    return Err(AuthError::RefreshConflict);
                }
                tracing::warn!(
                    family_id = %existing.family_id,
                    subject = %decoded.subject,
                    "superseded refresh token was reused; ending its session family"
                );
                self.refresh_token_store
                    .revoke_family(existing.family_id)
                    .await?;
                return Err(AuthError::CredentialRejected);
            }
            RotationOutcome::NotRotatable => return Err(AuthError::CredentialRejected),
        }
        Ok(to_response(
            AuthSession {
                user_id: user.id,
                tokens,
            },
            linked_providers,
        ))
    }

    async fn classify_inactive_token(
        &self,
        decoded: &DecodedRefreshToken,
    ) -> crate::errors::AuthResult<AuthResponseDto> {
        let state = self
            .refresh_token_store
            .find_state(&decoded.jti, self.runtime.now())
            .await?
            .map(RefreshTokenState::from);
        match state {
            Some(RefreshTokenState::Rotated {
                successor_jti,
                rotated_at,
                family_id,
                user_id,
            }) => {
                if self.runtime.now().signed_duration_since(rotated_at) <= self.refresh_grace_window
                {
                    tracing::info!(
                        successor_jti = %successor_jti,
                        "concurrent refresh lost the rotation race; caller should retry"
                    );
                    return Err(AuthError::RefreshConflict);
                }
                tracing::warn!(
                    family_id = %family_id,
                    user_id = %user_id,
                    "superseded refresh token was reused; ending its session family"
                );
                self.refresh_token_store.revoke_family(family_id).await?;
                Err(AuthError::CredentialRejected)
            }
            Some(RefreshTokenState::RevokedByLogout) => {
                tracing::debug!("refresh token was ended by logout; caller must sign in again");
                Err(AuthError::CredentialRejected)
            }
            Some(RefreshTokenState::Revoked) | None => Err(AuthError::CredentialRejected),
        }
    }

    pub async fn logout(&self, refresh_token: &str) -> crate::errors::AuthResult<()> {
        let decoded = self
            .token_service
            .decode_refresh_token(refresh_token)
            .map(DecodedRefreshToken::from)?;
        self.refresh_token_store.revoke(&decoded.jti).await
    }

    pub fn jwk_set(&self) -> crate::dto::token_dto::AuthJwkSetDto {
        self.token_service.jwk_set()
    }

    pub async fn session_status(
        &self,
        user_id: Uuid,
        session_id: Uuid,
    ) -> crate::errors::AuthResult<SessionStatus> {
        match self
            .user_repository
            .find_by_id(user_id)
            .await?
            .map(User::from)
        {
            Some(user) if user.status == UserStatus::Active => {}
            _ => return Ok(SessionStatus::Ended),
        }
        if !self
            .refresh_token_store
            .session_is_live(user_id, session_id, self.runtime.now())
            .await?
        {
            return Ok(SessionStatus::Ended);
        }
        Ok(SessionStatus::Active {
            roles: self.transactions.roles(user_id).await?,
        })
    }

    async fn is_locked_out(&self, user_id: Uuid) -> crate::errors::AuthResult<bool> {
        if self.lockout_policy.max_failures == 0 {
            return Ok(false);
        }
        let recent_failures = self
            .login_audit_repository
            .count_recent_failures(user_id, self.runtime.now() - self.lockout_policy.window)
            .await?;
        Ok(recent_failures >= i64::from(self.lockout_policy.max_failures))
    }

    async fn record_audit(&self, audit: LoginAudit) -> crate::errors::AuthResult<()> {
        self.login_audit_repository.record(&(&audit).into()).await
    }
}

fn hash_token(token: &str) -> String {
    Base64::encode_string(Sha256::digest(token.as_bytes()).as_slice())
}

fn to_response(session: AuthSession, linked_providers: Vec<IdentityProvider>) -> AuthResponseDto {
    AuthResponseDto {
        user_id: session.user_id,
        access_token: session.tokens.access_token,
        refresh_token: session.tokens.refresh_token,
        token_type: session.tokens.token_type,
        expires_in_seconds: session.tokens.expires_in_seconds,
        linked_providers,
    }
}
