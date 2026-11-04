use crate::auth_runtime::AuthRuntimePort;
use crate::auth_transactions::AuthTransactionsPort;
use crate::dto::auth_dto::{EmailAvailabilityResponseDto, SignupResponseDto};
use crate::onboarding::ResendVerificationResponse;
use crate::password_work::PasswordWork;
use crate::ports::UserRepositoryPort;
use crate::verification_crypto::VerificationCryptoPort;
use auth_domain::user::{User, UserStatus};
use std::sync::Arc;

#[derive(Clone)]
pub struct SignupUseCase<R> {
    user_repository: R,
    transactions: Arc<dyn AuthTransactionsPort>,
    password_work: PasswordWork,
    runtime: Arc<dyn AuthRuntimePort>,
    verification_crypto: Arc<dyn VerificationCryptoPort>,
}

impl<R> SignupUseCase<R>
where
    R: UserRepositoryPort + Clone,
{
    pub fn new(
        user_repository: R,
        transactions: Arc<dyn AuthTransactionsPort>,
        password_work: PasswordWork,
        runtime: Arc<dyn AuthRuntimePort>,
        verification_crypto: Arc<dyn VerificationCryptoPort>,
    ) -> Self {
        Self {
            user_repository,
            transactions,
            password_work,
            runtime,
            verification_crypto,
        }
    }

    /// Prepare using the same admitted password worker and identity/entropy owner.
    /// The MCR adapter commits this result together with durable consent; preparing
    /// alone creates no account, challenge, mail or acceptance.
    pub async fn prepare_email_signup(
        &self,
        email: &str,
        password: &str,
    ) -> crate::errors::AuthResult<(
        crate::dto::repository_dto::AuthUserRecordDto,
        crate::dto::verification_code_dto::VerificationCodeDto,
    )> {
        let normalized_email = User::normalized_email(email);
        validate_email(&normalized_email)?;
        validate_password(password)?;
        let password_hash = self.password_work.hash_for_signup(password).await?;
        let code = self.verification_crypto.generate()?;
        let user = User {
            id: self.runtime.new_id(),
            primary_email: normalized_email,
            password_hash,
            status: UserStatus::PendingVerification,
            last_login_at: Some(self.runtime.now()),
        };
        Ok(((&user).into(), code))
    }

    pub async fn signup_email(
        &self,
        email: &str,
        password: &str,
    ) -> crate::errors::AuthResult<SignupResponseDto> {
        let normalized_email = User::normalized_email(email);
        validate_email(&normalized_email)?;
        validate_password(password)?;
        if self
            .user_repository
            .find_by_email(&normalized_email)
            .await?
            .map(User::from)
            .is_some()
        {
            return Err(crate::errors::AuthError::Conflict(
                "Email already registered",
            ));
        }

        // Argon2 is deliberately completed before the database transaction.
        // A saturated signup class therefore changes no durable state.
        let password_hash = self.password_work.hash_for_signup(password).await?;
        let code = self.verification_crypto.generate()?;
        let user = User {
            id: self.runtime.new_id(),
            primary_email: normalized_email,
            password_hash,
            status: UserStatus::PendingVerification,
            last_login_at: Some(self.runtime.now()),
        };
        self.transactions
            .register_email(&(&user).into(), &code.code)
            .await?;
        Ok(SignupResponseDto {
            user_id: user.id,
            status: "PENDING_VERIFICATION".into(),
        })
    }

    pub async fn resend_verification(
        &self,
        email: &str,
    ) -> crate::errors::AuthResult<ResendVerificationResponse> {
        let normalized_email = User::normalized_email(email);
        validate_email(&normalized_email)?;
        let code = self.verification_crypto.generate()?;
        self.transactions
            .resend_verification(&normalized_email, &code.code)
            .await?;
        // Admission, account existence and delivery success are deliberately not
        // returned. ACCEPTED acknowledges a request, not an email delivery.
        Ok(ResendVerificationResponse {
            status: "ACCEPTED".into(),
        })
    }

    pub async fn verify_email(
        &self,
        email: &str,
        code: &str,
    ) -> crate::errors::AuthResult<SignupResponseDto> {
        let normalized_email = User::normalized_email(email);
        let user_id = self
            .transactions
            .verify_email(&normalized_email, code)
            .await?
            .ok_or(crate::errors::AuthError::InvalidInput(
                "Verification failed",
            ))?;
        Ok(SignupResponseDto {
            user_id,
            status: "VERIFIED".into(),
        })
    }

    pub async fn check_email(
        &self,
        email: &str,
    ) -> crate::errors::AuthResult<EmailAvailabilityResponseDto> {
        let normalized_email = User::normalized_email(email);
        if !is_valid_email(&normalized_email) {
            return Ok(EmailAvailabilityResponseDto {
                email: normalized_email,
                available: false,
                status: "INVALID".into(),
            });
        }
        let taken = self
            .user_repository
            .find_by_email(&normalized_email)
            .await?
            .map(User::from)
            .is_some();
        Ok(EmailAvailabilityResponseDto {
            email: normalized_email,
            available: !taken,
            status: if taken { "TAKEN" } else { "AVAILABLE" }.into(),
        })
    }
}

pub fn validate_password(password: &str) -> crate::errors::AuthResult<()> {
    if !(8..=128).contains(&password.len()) {
        return Err(crate::errors::AuthError::InvalidInput(
            "Password must be 8-128 characters",
        ));
    }
    Ok(())
}

pub fn validate_email(email: &str) -> crate::errors::AuthResult<()> {
    if !is_valid_email(email) {
        return Err(crate::errors::AuthError::InvalidInput(
            "Invalid email format or domain",
        ));
    }
    Ok(())
}

fn is_valid_email(email: &str) -> bool {
    let parts: Vec<_> = email.split('@').collect();
    email.len() <= 254
        && !email.chars().any(char::is_whitespace)
        && parts.len() == 2
        && parts[0].len() > 1
        && parts[1].contains('.')
}
