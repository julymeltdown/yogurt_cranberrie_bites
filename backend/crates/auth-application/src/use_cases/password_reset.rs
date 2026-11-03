use crate::auth_runtime::AuthRuntimePort;
use crate::auth_transactions::AuthTransactionsPort;
use crate::dto::auth_dto::PasswordResetResponseDto;
use crate::errors::AuthError;
use crate::password_reset_crypto::PasswordResetCryptoPort;
use crate::password_work::PasswordWork;
use crate::ports::UserRepositoryPort;
use crate::use_cases::signup::validate_password;
use auth_domain::password_reset::PasswordResetToken;
use auth_domain::user::User;
use chrono::Duration;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct PasswordResetUseCase<U> {
    user_repository: U,
    transactions: Arc<dyn AuthTransactionsPort>,
    reset_link_base_url: PasswordResetLinkBaseUrl,
    password_work: PasswordWork,
    in_flight: InFlightResets,
    runtime: Arc<dyn AuthRuntimePort>,
    crypto: Arc<dyn PasswordResetCryptoPort>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasswordResetLinkBaseUrl(String);

impl PasswordResetLinkBaseUrl {
    pub fn new(value: impl Into<String>) -> crate::errors::AuthResult<Self> {
        let value = value.into().trim().trim_end_matches('/').to_owned();
        if value.is_empty() {
            return Err(crate::errors::AuthError::Invariant(
                "AUTH_PASSWORD_RESET_BASE_URL is required",
            ));
        }
        if value.contains('?') || value.contains('#') {
            return Err(crate::errors::AuthError::Invariant(
                "AUTH_PASSWORD_RESET_BASE_URL must not include query or fragment",
            ));
        }
        Ok(Self(value))
    }

    pub fn build_link(&self, token: &str) -> String {
        format!("{}#token={token}", self.0)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<U> PasswordResetUseCase<U>
where
    U: UserRepositoryPort + Clone,
{
    pub fn new(
        user_repository: U,
        transactions: Arc<dyn AuthTransactionsPort>,
        password_work: PasswordWork,
        reset_link_base_url: PasswordResetLinkBaseUrl,
        runtime: Arc<dyn AuthRuntimePort>,
        crypto: Arc<dyn PasswordResetCryptoPort>,
    ) -> Self {
        Self {
            user_repository,
            transactions,
            reset_link_base_url,
            password_work,
            in_flight: InFlightResets::default(),
            runtime,
            crypto,
        }
    }

    pub async fn request_reset(
        &self,
        email: &str,
    ) -> crate::errors::AuthResult<PasswordResetResponseDto> {
        let normalized_email = User::normalized_email(email);
        if let Some(user) = self
            .user_repository
            .find_by_email(&normalized_email)
            .await?
            .map(User::from)
        {
            let material = self.crypto.generate()?;
            let record = PasswordResetToken {
                user_id: user.id,
                token_hash: material.fingerprint,
                expires_at: self.runtime.now() + Duration::hours(1),
            };
            let link = self.reset_link_base_url.build_link(&material.token);
            self.transactions
                .queue_password_reset(&(&user).into(), &(&record).into(), &link)
                .await?;
        }
        // Preserve the public non-enumerating response. Delivery is asynchronous.
        Ok(PasswordResetResponseDto {
            status: "SENT".into(),
        })
    }

    pub async fn confirm_reset(
        &self,
        token: &str,
        new_password: &str,
    ) -> crate::errors::AuthResult<PasswordResetResponseDto> {
        validate_password(new_password)?;
        let token_hash = self.crypto.fingerprint(token);
        let now = self.runtime.now();

        // Random or expired tokens are rejected before spending Argon2 capacity.
        // The authoritative token check still occurs in `reset_password`.
        if !self
            .transactions
            .reset_token_exists(&token_hash, now)
            .await?
        {
            return Err(AuthError::InvalidInput("Invalid or expired reset token"));
        }

        // One confirmation per token per process can hash or commit at a time.
        let _in_flight = self.in_flight.claim(&token_hash).ok_or(AuthError::Busy)?;

        // Hash before opening the atomic persistence transaction. Busy or failed
        // password admission cannot consume the one-time reset token.
        let password_hash = self.password_work.hash_for_reset(new_password).await?;
        self.transactions
            .reset_password(&token_hash, &password_hash, self.runtime.now())
            .await?
            .ok_or(AuthError::InvalidInput("Invalid or expired reset token"))?;
        Ok(PasswordResetResponseDto {
            status: "RESET".into(),
        })
    }
}

#[derive(Clone, Default)]
struct InFlightResets(Arc<Mutex<HashSet<String>>>);

impl InFlightResets {
    fn claim(&self, token_hash: &str) -> Option<InFlightReset> {
        let mut claimed = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        claimed
            .insert(token_hash.to_owned())
            .then(|| InFlightReset {
                resets: self.clone(),
                token_hash: token_hash.to_owned(),
            })
    }
}

struct InFlightReset {
    resets: InFlightResets,
    token_hash: String,
}

impl Drop for InFlightReset {
    fn drop(&mut self) {
        self.resets
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&self.token_hash);
    }
}
