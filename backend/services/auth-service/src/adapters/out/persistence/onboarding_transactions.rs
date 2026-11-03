use super::user_repository::PgUserRepository;
use crate::application::onboarding::AuthOnboardingPort;
use crate::application::use_cases::signup::validate_email;
use crate::domain::user::{IdentityProvider, User, UserStatus};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use sqlx::{PgConnection, Postgres, Row, Transaction};
use uuid::Uuid;

#[async_trait]
impl AuthOnboardingPort for PgUserRepository {
    async fn resolve_oauth_user(
        &self,
        proof: &crate::application::dto::repository_dto::AuthOAuthProofDto,
    ) -> crate::application::errors::AuthResult<
        crate::application::dto::repository_dto::AuthUserRecordDto,
    > {
        let proof = &crate::domain::oauth_identity::OAuthUserInfo::from(proof.clone());
        let result: anyhow::Result<_> = async {
        let provider = oauth_provider(proof.provider)?;
        validate_provider_subject(&proof.subject)?;
        let mut tx = begin(self).await?;
        // Separate namespaces prevent subject/email lock-order inversions. Hash
        // collisions only serialize unrelated requests; identity remains exact.
        sqlx::query("select pg_advisory_xact_lock(7011, hashtext($1))")
            .bind(format!("{provider}:{}", proof.subject))
            .execute(&mut *tx)
            .await?;
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select user_id from user_identities where provider_type = $1 and provider_subject = $2",
        )
        .bind(provider)
        .bind(&proof.subject)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(owner) = owner {
            let user = lock_active_user(&mut tx, owner).await?;
            // Lock user first, then inspect identity again after possible deletion.
            let live: bool = sqlx::query_scalar(
                "select exists(select 1 from user_identities where provider_type = $1 and provider_subject = $2 and user_id = $3 and deleted_at is null)",
            )
            .bind(provider)
            .bind(&proof.subject)
            .bind(owner)
            .fetch_one(&mut *tx)
            .await?;
            anyhow::ensure!(live, "OAuth identity not active");
            tx.commit().await?;
            return Ok(user);
        }

        // Do not leave a pending OAuth-only account which cannot complete the
        // email/password verification flow. No synthetic oauth.local addresses.
        anyhow::ensure!(proof.email_verified, "Verified provider email is required");
        let email = User::normalized_email(&proof.email);
        validate_email(&email)?;
        sqlx::query("select pg_advisory_xact_lock(7012, hashtext($1))")
            .bind(&email)
            .execute(&mut *tx)
            .await?;

        // Preserve verified-email linking only for a single live, verified local
        // owner. A primary-email collision alone is NOT proof of account ownership.
        let owners: Vec<Uuid> = sqlx::query_scalar(
            "select id from users where primary_email = $1 union select user_id from user_identities where email = $1",
        )
        .bind(&email)
        .fetch_all(&mut *tx)
        .await?;
        anyhow::ensure!(owners.len() <= 1, "Authenticated account linking required");
        let (user, created) = if let Some(owner) = owners.first() {
            let user = lock_active_user(&mut tx, *owner).await?;
            let verified: bool = sqlx::query_scalar(
                "select exists(select 1 from user_identities where user_id = $1 and email = $2 and email_verified = true and deleted_at is null)",
            )
            .bind(owner)
            .bind(&email)
            .fetch_one(&mut *tx)
            .await?;
            anyhow::ensure!(verified, "Authenticated account linking required");
            (user, false)
        } else {
            let user = User {
                id: Uuid::new_v4(),
                primary_email: email.clone(),
                password_hash: String::new(),
                status: UserStatus::Active,
                last_login_at: None,
            };
            sqlx::query(
                "insert into users (id, primary_email, password_hash, status) values ($1, $2, '', 'ACTIVE')",
            )
            .bind(user.id)
            .bind(&email)
            .execute(&mut *tx)
            .await?;
            (user, true)
        };
        // Unique (provider_type, provider_subject) is the final arbiter when an
        // explicit account-link request races this flow. Roll back, never reassign.
        sqlx::query(
            "insert into user_identities (id, user_id, provider_type, provider_subject, email, email_verified) values ($1, $2, $3, $4, $5, true)",
        )
        .bind(Uuid::new_v4())
        .bind(user.id)
        .bind(provider)
        .bind(&proof.subject)
        .bind(&email)
        .execute(&mut *tx)
        .await?;
        let action = if created {
            "OAUTH_ACCOUNT_CREATED"
        } else {
            "OAUTH_IDENTITY_LINKED"
        };
        audit(&mut tx, user.id, action).await?;
        tx.commit().await?;
        Ok(user)
    }.await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.into())
    }

    async fn resend_verification(
        &self,
        email: &str,
        code: &str,
    ) -> crate::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
        validate_email(email)?;
        anyhow::ensure!(
            code.len() == 6 && code.bytes().all(|byte| byte.is_ascii_digit()),
            "Invalid verification code"
        );
        let mut tx = begin(self).await?;
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select id from users where primary_email = $1 and status = 'PENDING_VERIFICATION' and deleted_at is null for update",
        )
        .bind(email)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(user_id) = owner else {
            return Ok(());
        };
        let eligible: bool = sqlx::query_scalar(
            "select exists(select 1 from user_identities where user_id = $1 and provider_type = 'EMAIL' and email = $2 and email_verified = false and deleted_at is null)",
        )
        .bind(user_id)
        .bind(email)
        .fetch_one(&mut *tx)
        .await?;
        if !eligible {
            return Ok(());
        }
        let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *tx)
            .await?;
        let limited: bool = sqlx::query_scalar(
            "select exists(select 1 from auth_mail_outbox where user_id = $1 and kind = 'VERIFY_EMAIL' and created_at > $2 - interval '1 minute') or (select count(*) from auth_mail_outbox where user_id = $1 and kind = 'VERIFY_EMAIL' and created_at > $2 - interval '1 day') >= 20",
        )
        .bind(user_id)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        if limited {
            return Ok(());
        }
        sqlx::query("delete from email_verifications where user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "update auth_mail_outbox set dead_lettered_at = $2, payload_json = null, origin_traceparent = null, last_error_code = 'superseded' where user_id = $1 and kind = 'VERIFY_EMAIL' and delivered_at is null and dead_lettered_at is null",
        )
        .bind(user_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        let expires_at = now + Duration::hours(1);
        sqlx::query(
            "insert into email_verifications (id, user_id, email, code, expires_at) values ($1, $2, $3, $4, $5)",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(email)
        .bind(code)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
        let payload = serde_json::json!({"email": email, "code": code}).to_string();
        let origin_traceparent =
            platform::trace_context::current().map(|context| context.traceparent());
        sqlx::query(
            "insert into auth_mail_outbox (id, user_id, kind, payload_json, expires_at, created_at, origin_traceparent) values ($1, $2, 'VERIFY_EMAIL', $3, $4, $5, $6)",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(payload)
        .bind(expires_at)
        .bind(now)
        .bind(origin_traceparent)
        .execute(&mut *tx)
        .await?;
        audit(&mut tx, user_id, "EMAIL_VERIFICATION_RESENT").await?;
        tx.commit().await?;
        Ok(())
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }
}

fn validate_provider_subject(subject: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        subject.len() <= 512
            && !subject.trim().is_empty()
            && !subject.chars().any(char::is_control),
        "Invalid provider subject"
    );
    Ok(())
}

async fn begin(repository: &PgUserRepository) -> anyhow::Result<Transaction<'_, Postgres>> {
    let mut tx = repository.pool.begin().await?;
    sqlx::query(
        "select set_config('lock_timeout', '2s', true), set_config('statement_timeout', '5s', true)",
    )
    .execute(&mut *tx)
    .await?;
    Ok(tx)
}

async fn lock_active_user(connection: &mut PgConnection, user_id: Uuid) -> anyhow::Result<User> {
    let row = sqlx::query(
        "select id, primary_email, password_hash, last_login_at from users where id = $1 and status = 'ACTIVE' and deleted_at is null for update",
    )
    .bind(user_id)
    .fetch_optional(connection)
    .await?
    .ok_or_else(|| anyhow::anyhow!("User not active"))?;
    Ok(User {
        id: row.try_get("id")?,
        primary_email: row.try_get("primary_email")?,
        password_hash: row.try_get("password_hash")?,
        status: UserStatus::Active,
        last_login_at: row.try_get("last_login_at")?,
    })
}

async fn audit(connection: &mut PgConnection, user_id: Uuid, action: &str) -> anyhow::Result<()> {
    sqlx::query(
        "insert into auth_audit_events (id, actor_id, subject_user_id, action, outcome) values ($1, $2, $2, $3, 'SUCCESS')",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(action)
    .execute(connection)
    .await?;
    Ok(())
}

fn oauth_provider(provider: IdentityProvider) -> anyhow::Result<&'static str> {
    match provider {
        IdentityProvider::Google => Ok("GOOGLE"),
        IdentityProvider::Apple => Ok("APPLE"),
        IdentityProvider::X => Ok("X"),
        IdentityProvider::Email => anyhow::bail!("OAuth provider required"),
    }
}

#[cfg(test)]
mod tests {
    use super::validate_provider_subject;

    #[test]
    fn provider_subject_is_bounded_before_whitespace_normalization() {
        assert!(validate_provider_subject(&" ".repeat(513)).is_err());
        assert!(validate_provider_subject("subject-1").is_ok());
        assert!(validate_provider_subject(" \n ").is_err());
    }
}
