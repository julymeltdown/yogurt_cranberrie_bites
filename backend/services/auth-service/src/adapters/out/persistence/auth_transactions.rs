use super::user_repository::PgUserRepository;
use crate::application::auth_transactions::AuthTransactionsPort;
use crate::domain::session::RefreshTokenRecord;
use crate::domain::user::{User, UserStatus};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, Postgres, Row, Transaction};
use uuid::Uuid;

#[async_trait]
impl AuthTransactionsPort for PgUserRepository {
    async fn register_email(
        &self,
        user: &crate::application::dto::repository_dto::AuthUserRecordDto,
        code: &str,
    ) -> crate::application::errors::AuthResult<()> {
        let user = &crate::domain::user::User::from(user.clone());

        let result: anyhow::Result<_> = async {
        anyhow::ensure!(
            user.status == UserStatus::PendingVerification,
            "Invalid signup status"
        );
        anyhow::ensure!(
            code.len() == 6 && code.bytes().all(|byte| byte.is_ascii_digit()),
            "Invalid verification code"
        );
        let mut tx = begin(self).await?;
        sqlx::query("insert into users (id, primary_email, password_hash, status, last_login_at) values ($1, $2, $3, 'PENDING_VERIFICATION', $4)")
            .bind(user.id).bind(&user.primary_email).bind(&user.password_hash)
            .bind(user.last_login_at).execute(&mut *tx).await?;
        sqlx::query("insert into user_identities (id, user_id, provider_type, email, email_verified) values ($1, $2, 'EMAIL', $3, false)")
            .bind(Uuid::new_v4()).bind(user.id).bind(&user.primary_email)
            .execute(&mut *tx).await?;
        let expires_at = Utc::now() + chrono::Duration::hours(1);
        sqlx::query("insert into email_verifications (id, user_id, email, code, expires_at) values ($1, $2, $3, $4, $5)")
            .bind(Uuid::new_v4()).bind(user.id).bind(&user.primary_email).bind(code)
            .bind(expires_at).execute(&mut *tx).await?;
        enqueue(
            &mut tx,
            user.id,
            "VERIFY_EMAIL",
            serde_json::json!({"email": user.primary_email, "code": code}).to_string(),
            expires_at,
        )
        .await?;
        security_audit(&mut tx, user.id, "EMAIL_SIGNUP").await?;
        tx.commit().await?;
        Ok(())
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn verify_email(
        &self,
        email: &str,
        code: &str,
    ) -> crate::application::errors::AuthResult<Option<Uuid>> {
        let result: anyhow::Result<_> = async {
        let mut tx = begin(self).await?;
        let user_id: Option<Uuid> = sqlx::query_scalar("select id from users where primary_email = $1 and status = 'PENDING_VERIFICATION' and deleted_at is null for update")
            .bind(email).fetch_optional(&mut *tx).await?;
        let Some(user_id) = user_id else {
            return Ok(None);
        };
        let row = sqlx::query("select id, code, attempts from email_verifications where user_id = $1 and email = $2 and expires_at > clock_timestamp() order by expires_at desc, id limit 1 for update")
            .bind(user_id).bind(email).fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let attempts: i32 = row.try_get("attempts")?;
        if attempts >= 5 {
            return Ok(None);
        }
        let expected: String = row.try_get("code")?;
        if code != expected {
            let id: Uuid = row.try_get("id")?;
            sqlx::query("update email_verifications set attempts = attempts + 1 where id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            return Ok(None);
        }
        sqlx::query("delete from email_verifications where user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("update users set status = 'ACTIVE', updated_at = now() where id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        let changed = sqlx::query("update user_identities set email_verified = true where user_id = $1 and provider_type = 'EMAIL' and email = $2 and deleted_at is null")
            .bind(user_id).bind(email).execute(&mut *tx).await?.rows_affected();
        anyhow::ensure!(changed == 1, "Email identity missing");
        cancel_pending_mail(&mut tx, user_id, "VERIFY_EMAIL", "verified").await?;
        security_audit(&mut tx, user_id, "EMAIL_VERIFIED").await?;
        tx.commit().await?;
        Ok(Some(user_id))
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn queue_password_reset(
        &self,
        user: &crate::application::dto::repository_dto::AuthUserRecordDto,
        token: &crate::application::dto::repository_dto::AuthPasswordResetRecordDto,
        link: &str,
    ) -> crate::application::errors::AuthResult<bool> {
        let user = &crate::domain::user::User::from(user.clone());
        let token = &crate::domain::password_reset::PasswordResetToken::from(token.clone());

        let result: anyhow::Result<_> = async {
        anyhow::ensure!(
            token.user_id == user.id && link.len() <= 8192,
            "Invalid reset request"
        );
        let mut tx = begin(self).await?;
        if !lock_active_snapshot(&mut tx, user).await? {
            return Ok(false);
        }
        // A row lock serializes delivery admission across all auth replicas.
        let limited: bool = sqlx::query_scalar("select exists (select 1 from auth_mail_outbox where user_id = $1 and kind = 'PASSWORD_RESET' and created_at > now() - interval '1 minute') or (select count(*) from auth_mail_outbox where user_id = $1 and kind = 'PASSWORD_RESET' and created_at > now() - interval '1 day') >= 20")
            .bind(user.id).fetch_one(&mut *tx).await?;
        if limited {
            return Ok(false);
        }
        sqlx::query("delete from password_reset_tokens where user_id = $1")
            .bind(user.id)
            .execute(&mut *tx)
            .await?;
        cancel_pending_mail(&mut tx, user.id, "PASSWORD_RESET", "superseded").await?;
        sqlx::query("insert into password_reset_tokens (id, user_id, token_hash, expires_at) values ($1, $2, $3, $4)")
            .bind(Uuid::new_v4()).bind(user.id).bind(&token.token_hash).bind(token.expires_at)
            .execute(&mut *tx).await?;
        enqueue(
            &mut tx,
            user.id,
            "PASSWORD_RESET",
            serde_json::json!({"email": user.primary_email, "link": link}).to_string(),
            token.expires_at,
        )
        .await?;
        security_audit(&mut tx, user.id, "PASSWORD_RESET_REQUESTED").await?;
        tx.commit().await?;
        Ok(true)
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn reset_password(
        &self,
        token_hash: &str,
        password_hash: &str,
        now: DateTime<Utc>,
    ) -> crate::application::errors::AuthResult<Option<Uuid>> {
        let result: anyhow::Result<_> = async {
        let mut tx = begin(self).await?;
        // Discover the owner without locking a token first. Every security
        // transaction uses the same user -> token lock order.
        let owner: Option<Uuid> = sqlx::query_scalar("select user_id from password_reset_tokens where token_hash = $1 and expires_at > $2 limit 1")
            .bind(token_hash).bind(now).fetch_optional(&mut *tx).await?;
        let Some(user_id) = owner else {
            return Ok(None);
        };
        let active: Option<Uuid> = sqlx::query_scalar("select id from users where id = $1 and status = 'ACTIVE' and deleted_at is null for update")
            .bind(user_id).fetch_optional(&mut *tx).await?;
        if active.is_none() {
            return Ok(None);
        }
        let consumed: Option<Uuid> = sqlx::query_scalar("delete from password_reset_tokens where user_id = $1 and token_hash = $2 and expires_at > greatest($3, clock_timestamp()) returning id")
            .bind(user_id).bind(token_hash).bind(now).fetch_optional(&mut *tx).await?;
        if consumed.is_none() {
            return Ok(None);
        }
        sqlx::query("update users set password_hash = $2, updated_at = now() where id = $1")
            .bind(user_id)
            .bind(password_hash)
            .execute(&mut *tx)
            .await?;
        sqlx::query("update refresh_tokens set revoked_at = now() where user_id = $1 and revoked_at is null")
            .bind(user_id).execute(&mut *tx).await?;
        sqlx::query("delete from password_reset_tokens where user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        cancel_pending_mail(&mut tx, user_id, "PASSWORD_RESET", "consumed").await?;
        security_audit(&mut tx, user_id, "PASSWORD_RESET_COMPLETED").await?;
        tx.commit().await?;
        Ok(Some(user_id))
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn reset_token_exists(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
    ) -> crate::application::errors::AuthResult<bool> {
        let result: anyhow::Result<_> = async {
        // A read-only pre-check. The authoritative comparison still happens in
        // `reset_password`, which re-tests expiry and state under the user lock.
        let found: Option<Uuid> = sqlx::query_scalar(
            "select user_id from password_reset_tokens where token_hash = $1 and expires_at > greatest($2, clock_timestamp()) limit 1",
        )
        .bind(token_hash)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        Ok(found.is_some())
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn roles(&self, user_id: Uuid) -> crate::application::errors::AuthResult<Vec<String>> {
        let result: anyhow::Result<_> = async {
            let mut connection = self.pool.acquire().await?;
            roles_in_connection(&mut connection, user_id).await
        }
        .await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn begin_session(
        &self,
        expected: &crate::application::dto::repository_dto::AuthUserRecordDto,
        refresh: &crate::application::dto::repository_dto::AuthRefreshRecordDto,
        roles: &[String],
        audit: &crate::application::dto::repository_dto::AuthLoginAuditRecordDto,
    ) -> crate::application::errors::AuthResult<bool> {
        let expected = &crate::domain::user::User::from(expected.clone());
        let refresh = &crate::domain::session::RefreshTokenRecord::from(refresh.clone());
        let audit = &crate::domain::session::LoginAudit::from(audit.clone());

        let result: anyhow::Result<_> = async {
        anyhow::ensure!(
            refresh.user_id == expected.id && audit.user_id == expected.id && audit.success,
            "Invalid session owner"
        );
        let mut tx = begin(self).await?;
        if !lock_active_snapshot(&mut tx, expected).await?
            || !roles_match(&mut tx, expected.id, roles).await?
        {
            return Ok(false);
        }
        insert_refresh(&mut tx, refresh, self.runtime.now()).await?;
        sqlx::query("update users set last_login_at = greatest(last_login_at, $2), updated_at = now() where id = $1")
            .bind(expected.id).bind(audit.occurred_at).execute(&mut *tx).await?;
        sqlx::query("insert into login_audit (id, user_id, ip_address, user_agent, success, occurred_at) values ($1, $2, $3, $4, true, $5)")
            .bind(audit.id).bind(audit.user_id).bind(&audit.ip_address).bind(&audit.user_agent)
            .bind(audit.occurred_at).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn rotate_session(
        &self,
        expected: &crate::application::dto::repository_dto::AuthUserRecordDto,
        current_jti: &str,
        current_hash: &str,
        replacement: &crate::application::dto::repository_dto::AuthRefreshRecordDto,
        roles: &[String],
    ) -> crate::application::errors::AuthResult<
        crate::application::dto::repository_dto::AuthRotationOutcomeDto,
    > {
        let expected = &crate::domain::user::User::from(expected.clone());
        let replacement = &crate::domain::session::RefreshTokenRecord::from(replacement.clone());
        let result: anyhow::Result<_> = async {
        use crate::domain::session::RotationOutcome;
        anyhow::ensure!(replacement.user_id == expected.id, "Invalid session owner");
        let mut tx = begin(self).await?;
        if !lock_active_snapshot(&mut tx, expected).await?
            || !roles_match(&mut tx, expected.id, roles).await?
        {
            return Ok(RotationOutcome::NotRotatable);
        }
        // Read the current row under the same transaction that updates it, so a
        // concurrent rotation cannot also observe "active" and both proceed.
        let current = sqlx::query(
            "select token_hash, family_id, rotated_at, successor_jti, revoked_at, expires_at
             from refresh_tokens where jti = $1 for update",
        )
        .bind(current_jti)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(current) = current else {
            return Ok(RotationOutcome::NotRotatable);
        };
        let stored_hash: String = current.get("token_hash");
        let expires_at: chrono::DateTime<chrono::Utc> = current.get("expires_at");
        let revoked_at: Option<chrono::DateTime<chrono::Utc>> = current.get("revoked_at");
        // Admission and rotation time are sampled after the user/token locks.
        let decision_at = self.runtime.now();
        if stored_hash != current_hash || expires_at <= decision_at {
            return Ok(RotationOutcome::NotRotatable);
        }
        if revoked_at.is_some() {
            let rotated_at: Option<chrono::DateTime<chrono::Utc>> = current.get("rotated_at");
            let successor_jti: Option<String> = current.get("successor_jti");
            return match (rotated_at, successor_jti) {
                (Some(rotated_at), Some(successor_jti)) => {
                    tx.commit().await?;
                    Ok(RotationOutcome::AlreadyRotated {
                        successor_jti,
                        rotated_at,
                    })
                }
                _ => Ok(RotationOutcome::NotRotatable),
            };
        }
        let family_id: uuid::Uuid = current.get("family_id");
        anyhow::ensure!(
            family_id == replacement.family_id,
            "replacement token must continue its own family"
        );
        let changed = sqlx::query(
            "update refresh_tokens
             set revoked_at = $3, rotated_at = $3, successor_jti = $2,
                 revoked_reason = 'ROTATED'
             where jti = $1 and revoked_at is null",
        )
        .bind(current_jti)
        .bind(&replacement.jti)
        .bind(decision_at)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if changed != 1 {
            return Ok(RotationOutcome::NotRotatable);
        }
        insert_refresh(&mut tx, replacement, self.runtime.now()).await?;
        sqlx::query("insert into auth_audit_events (id, actor_id, subject_user_id, action, outcome, occurred_at) values ($1, $2, $2, 'SESSION_ROTATED', 'SUCCESS', $3)")
            .bind(self.runtime.new_id()).bind(expected.id).bind(self.runtime.now()).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(RotationOutcome::Rotated)
    }.await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.into())
    }
}

async fn begin(repository: &PgUserRepository) -> anyhow::Result<Transaction<'_, Postgres>> {
    let mut tx = repository.pool.begin().await?;
    sqlx::query("select set_config('lock_timeout', '2s', true), set_config('statement_timeout', '5s', true)")
        .execute(&mut *tx).await?;
    Ok(tx)
}

async fn lock_active_snapshot(
    connection: &mut PgConnection,
    expected: &User,
) -> anyhow::Result<bool> {
    if expected.status != UserStatus::Active {
        return Ok(false);
    }
    let row = sqlx::query("select primary_email, password_hash from users where id = $1 and status = 'ACTIVE' and deleted_at is null for update")
        .bind(expected.id).fetch_optional(&mut *connection).await?;
    match row {
        Some(row) => Ok(
            row.try_get::<String, _>("password_hash")? == expected.password_hash
                && row.try_get::<String, _>("primary_email")? == expected.primary_email,
        ),
        None => Ok(false),
    }
}

async fn roles_in_connection(
    connection: &mut PgConnection,
    user_id: Uuid,
) -> anyhow::Result<Vec<String>> {
    let mut roles: Vec<String> =
        sqlx::query_scalar("select role from auth_user_roles where user_id = $1 order by role")
            .bind(user_id)
            .fetch_all(&mut *connection)
            .await?;
    roles.push("ROLE_USER".into());
    roles.sort();
    roles.dedup();
    Ok(roles)
}

async fn roles_match(
    connection: &mut PgConnection,
    user_id: Uuid,
    expected: &[String],
) -> anyhow::Result<bool> {
    let mut expected = expected.to_vec();
    expected.sort();
    expected.dedup();
    Ok(roles_in_connection(connection, user_id).await? == expected)
}

async fn insert_refresh(
    connection: &mut PgConnection,
    record: &RefreshTokenRecord,
    now: DateTime<Utc>,
) -> anyhow::Result<()> {
    anyhow::ensure!(record.expires_at > now, "Invalid session expiry");
    sqlx::query("insert into refresh_tokens (jti, token_hash, user_id, family_id, expires_at, revoked_at) values ($1, $2, $3, $4, $5, null)")
        .bind(&record.jti).bind(&record.token_hash).bind(record.user_id)
        .bind(record.family_id).bind(record.expires_at)
        .execute(&mut *connection).await?;
    Ok(())
}

async fn enqueue(
    connection: &mut PgConnection,
    user_id: Uuid,
    kind: &str,
    payload: String,
    expires_at: DateTime<Utc>,
) -> anyhow::Result<()> {
    let origin_traceparent =
        platform::trace_context::current().map(|context| context.traceparent());
    sqlx::query(
        "insert into auth_mail_outbox (id, user_id, kind, payload_json, expires_at, origin_traceparent) values ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(kind)
    .bind(payload)
    .bind(expires_at)
    .bind(origin_traceparent)
    .execute(&mut *connection)
    .await?;
    Ok(())
}

async fn cancel_pending_mail(
    connection: &mut PgConnection,
    user_id: Uuid,
    kind: &str,
    reason: &str,
) -> anyhow::Result<()> {
    sqlx::query("update auth_mail_outbox set dead_lettered_at = now(), payload_json = null, origin_traceparent = null, last_error_code = $3 where user_id = $1 and kind = $2 and delivered_at is null and dead_lettered_at is null")
        .bind(user_id).bind(kind).bind(reason).execute(&mut *connection).await?;
    Ok(())
}

async fn security_audit(
    connection: &mut PgConnection,
    user_id: Uuid,
    action: &str,
) -> anyhow::Result<()> {
    sqlx::query("insert into auth_audit_events (id, actor_id, subject_user_id, action, outcome) values ($1, $2, $2, $3, 'SUCCESS')")
        .bind(Uuid::new_v4()).bind(user_id).bind(action).execute(&mut *connection).await?;
    Ok(())
}
