use crate::application::auth_runtime::AuthRuntimePort;
use crate::application::ports::{
    AuthAuditRepositoryPort, LoginAuditRepositoryPort, OAuthIdentityRepositoryPort,
    PasswordResetStorePort, UserRepositoryPort, VerificationRepositoryPort,
};
use crate::domain::oauth_identity::OAuthIdentity;
use crate::domain::password_reset::PasswordResetToken;
use crate::domain::user::{IdentityProvider, User, UserStatus};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct PgUserRepository {
    pub pool: PgPool,
    pub(super) runtime: Arc<dyn AuthRuntimePort>,
}

impl PgUserRepository {
    pub fn new(pool: PgPool, runtime: Arc<dyn AuthRuntimePort>) -> Self {
        Self { pool, runtime }
    }
}

#[async_trait]
impl UserRepositoryPort for PgUserRepository {
    async fn find_by_email(
        &self,
        email: &str,
    ) -> crate::application::errors::AuthResult<
        Option<crate::application::dto::repository_dto::AuthUserRecordDto>,
    > {
        let result: anyhow::Result<_> = async {
        let row = sqlx::query(
            "select id, primary_email, password_hash, status, last_login_at from users where primary_email = $1",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(map_user))
    }.await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }

    async fn find_by_id(
        &self,
        user_id: Uuid,
    ) -> crate::application::errors::AuthResult<
        Option<crate::application::dto::repository_dto::AuthUserRecordDto>,
    > {
        let result: anyhow::Result<_> = async {
        let row = sqlx::query(
            "select id, primary_email, password_hash, status, last_login_at from users where id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(map_user))
    }.await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }

    async fn save(
        &self,
        user: &crate::application::dto::repository_dto::AuthUserRecordDto,
    ) -> crate::application::errors::AuthResult<()> {
        let user = &crate::domain::user::User::from(user.clone());

        let result: anyhow::Result<_> = async {
            // This compatibility save never changes an existing credential. The
            // database tombstone guard also forbids re-creating physically deleted IDs.
            let affected = sqlx::query(
                r#"
            insert into users (id, primary_email, password_hash, status, last_login_at)
            values ($1, $2, $3, $4, $5)
            on conflict (id) do update
            set primary_email = case
                    when users.primary_email = '' then excluded.primary_email
                    else users.primary_email
                end,
                status = excluded.status,
                last_login_at = greatest(users.last_login_at, excluded.last_login_at),
                updated_at = now()
            where users.deleted_at is null
              and users.status <> 'DELETED'
              and users.password_hash = excluded.password_hash
              and (users.primary_email = excluded.primary_email or users.primary_email = '')
              and (
                  users.status = excluded.status
                  or (users.status = 'PENDING_VERIFICATION' and excluded.status = 'ACTIVE')
              )
            "#,
            )
            .bind(user.id)
            .bind(&user.primary_email)
            .bind(&user.password_hash)
            .bind(match user.status {
                UserStatus::PendingVerification => "PENDING_VERIFICATION",
                UserStatus::Active => "ACTIVE",
                UserStatus::Deleted => "DELETED",
            })
            .bind(user.last_login_at)
            .execute(&self.pool)
            .await?
            .rows_affected();
            if affected != 1 {
                anyhow::bail!("User state changed; retry the operation");
            }
            Ok(())
        }
        .await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn update_password_hash(
        &self,
        user_id: Uuid,
        password_hash: &str,
    ) -> crate::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
        // The credentials trigger revokes sessions in this same statement.
        let affected = sqlx::query("update users set password_hash = $2, updated_at = now() where id = $1 and status = 'ACTIVE' and deleted_at is null")
            .bind(user_id)
            .bind(password_hash)
            .execute(&self.pool)
            .await?
            .rows_affected();
        anyhow::ensure!(affected == 1, "User not active");
        Ok(())
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn link_email_identity(
        &self,
        user_id: Uuid,
        email: &str,
        verified: bool,
    ) -> crate::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
        sqlx::query(
            r#"
            insert into user_identities (id, user_id, provider_type, email, email_verified, linked_at)
            values ($1, $2, 'EMAIL', $3, $4, now())
            on conflict (user_id, provider_type, email) do update
            set email_verified = excluded.email_verified
            "#,
        )
        .bind(Uuid::new_v4()).bind(user_id).bind(email).bind(verified)
        .execute(&self.pool).await?;
        Ok(())
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn mark_email_verified(
        &self,
        user_id: Uuid,
        email: &str,
    ) -> crate::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
        sqlx::query("update user_identities set email_verified = true where user_id = $1 and provider_type = 'EMAIL' and email = $2")
            .bind(user_id).bind(email).execute(&self.pool).await?;
        Ok(())
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn linked_providers(
        &self,
        user_id: Uuid,
    ) -> crate::application::errors::AuthResult<
        Vec<crate::application::dto::repository_dto::AuthIdentityProviderDto>,
    > {
        let result: anyhow::Result<_> = async {
        let rows = sqlx::query("select distinct provider_type from user_identities where user_id = $1 and deleted_at is null")
            .bind(user_id).fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| match row.get::<String, _>("provider_type").as_str() {
                "EMAIL" => Some(IdentityProvider::Email),
                "GOOGLE" => Some(IdentityProvider::Google),
                "APPLE" => Some(IdentityProvider::Apple),
                "X" => Some(IdentityProvider::X),
                _ => None,
            })
            .collect())
    }.await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value: Vec<IdentityProvider>| value.into_iter().map(Into::into).collect())
    }

    async fn soft_delete_user(
        &self,
        user_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
    ) -> crate::application::errors::AuthResult<bool> {
        let result: anyhow::Result<_> = async {
        let mut tx = self.pool.begin().await?;
        let updated = sqlx::query(
            r#"
            update users
            set status = 'DELETED', deleted_at = now(), deleted_by = $2,
                delete_reason = $3, retention_until = $4, updated_at = now()
            where id = $1 and deleted_at is null
            "#,
        )
        .bind(user_id)
        .bind(actor_id)
        .bind(reason)
        .bind(retention_until)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if updated == 1 {
            sqlx::query("update user_identities set deleted_at = now(), retention_until = $2 where user_id = $1 and deleted_at is null")
                .bind(user_id).bind(retention_until).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(updated == 1)
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }
}

#[async_trait]
impl OAuthIdentityRepositoryPort for PgUserRepository {
    async fn find_by_provider_subject(
        &self,
        provider: crate::application::dto::repository_dto::AuthIdentityProviderDto,
        subject: &str,
    ) -> crate::application::errors::AuthResult<
        Option<crate::application::dto::repository_dto::AuthOAuthIdentityRecordDto>,
    > {
        let provider = crate::domain::user::IdentityProvider::from(provider);
        let result: anyhow::Result<_> = async {
        let row = sqlx::query("select user_id, provider_type, provider_subject, email, email_verified from user_identities where provider_type = $1 and provider_subject = $2")
            .bind(provider_name(provider)).bind(subject).fetch_optional(&self.pool).await?;
        Ok(row.map(map_oauth_identity))
    }.await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }

    async fn find_by_email(
        &self,
        email: &str,
    ) -> crate::application::errors::AuthResult<
        Vec<crate::application::dto::repository_dto::AuthOAuthIdentityRecordDto>,
    > {
        let result: anyhow::Result<_> = async {
        let rows = sqlx::query("select user_id, provider_type, provider_subject, email, email_verified from user_identities where email = $1")
            .bind(email).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(map_oauth_identity).collect())
    }.await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value: Vec<OAuthIdentity>| value.into_iter().map(Into::into).collect())
    }

    async fn link_oauth_identity(
        &self,
        user_id: Uuid,
        provider: crate::application::dto::repository_dto::AuthIdentityProviderDto,
        subject: &str,
        email: &str,
        email_verified: bool,
    ) -> crate::application::errors::AuthResult<()> {
        let provider = crate::domain::user::IdentityProvider::from(provider);

        let result: anyhow::Result<_> = async {
        let updated = sqlx::query(
            r#"
            update user_identities
            set email = $4, email_verified = $5, linked_at = now()
            where provider_type = $1 and provider_subject = $2 and user_id = $3 and deleted_at is null
            "#,
        )
        .bind(provider_name(provider)).bind(subject).bind(user_id).bind(email).bind(email_verified)
        .execute(&self.pool).await?;
        if updated.rows_affected() == 0 {
            // Never transfer an existing provider subject to another account.
            sqlx::query("insert into user_identities (id, user_id, provider_type, provider_subject, email, email_verified, linked_at) values ($1, $2, $3, $4, $5, $6, now())")
                .bind(Uuid::new_v4()).bind(user_id).bind(provider_name(provider)).bind(subject)
                .bind(email).bind(email_verified).execute(&self.pool).await?;
        }
        Ok(())
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }
}

#[derive(Clone)]
pub struct PgVerificationRepository {
    pool: PgPool,
}

impl PgVerificationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl VerificationRepositoryPort for PgVerificationRepository {
    async fn save_code(
        &self,
        user_id: Uuid,
        email: &str,
        code: &str,
    ) -> crate::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
        sqlx::query("insert into email_verifications (id, user_id, email, code, expires_at) values ($1, $2, $3, $4, now() + interval '1 hour')")
            .bind(Uuid::new_v4()).bind(user_id).bind(email).bind(code).execute(&self.pool).await?;
        Ok(())
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn consume_code(
        &self,
        email: &str,
        code: &str,
    ) -> crate::application::errors::AuthResult<Option<Uuid>> {
        let result: anyhow::Result<_> = async {
        let row = sqlx::query("delete from email_verifications where email = $1 and code = $2 and expires_at > now() returning user_id")
            .bind(email).bind(code).fetch_optional(&self.pool).await?;
        Ok(row.map(|row| row.get("user_id")))
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }
}

#[derive(Clone)]
pub struct PgLoginAuditRepository {
    pool: PgPool,
}

impl PgLoginAuditRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LoginAuditRepositoryPort for PgLoginAuditRepository {
    async fn record(
        &self,
        audit: &crate::application::dto::repository_dto::AuthLoginAuditRecordDto,
    ) -> crate::application::errors::AuthResult<()> {
        let audit = &crate::domain::session::LoginAudit::from(audit.clone());

        let result: anyhow::Result<_> = async {
        sqlx::query("insert into login_audit (id, user_id, ip_address, user_agent, success, credential_attempt, occurred_at) values ($1, $2, $3, $4, $5, $6, $7)")
            .bind(audit.id).bind(audit.user_id).bind(&audit.ip_address).bind(&audit.user_agent)
            .bind(audit.success).bind(audit.credential_attempt).bind(audit.occurred_at)
            .execute(&self.pool).await?;
        Ok(())
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn count_recent_failures(
        &self,
        user_id: Uuid,
        since: DateTime<Utc>,
    ) -> crate::application::errors::AuthResult<i64> {
        let result: anyhow::Result<_> = async {
        // Only genuine credential failures count. Refusals caused by an existing
        // lockout are excluded so the lockout cannot be extended indefinitely.
        let row = sqlx::query("select count(*)::bigint as count from login_audit where user_id = $1 and success = false and credential_attempt and occurred_at >= $2")
            .bind(user_id).bind(since).fetch_one(&self.pool).await?;
        Ok(row.get("count"))
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }
}

#[derive(Clone)]
pub struct PgAuthAuditRepository {
    pool: PgPool,
}

impl PgAuthAuditRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuthAuditRepositoryPort for PgAuthAuditRepository {
    async fn record(
        &self,
        event: &crate::application::dto::repository_dto::AuthAuditEventRecordDto,
    ) -> crate::application::errors::AuthResult<()> {
        let event = &crate::domain::session::AuthAuditEvent::from(event.clone());

        let result: anyhow::Result<_> = async {
            sqlx::query(
                r#"
            insert into auth_audit_events (
                id, actor_id, subject_user_id, action, outcome,
                request_id, ip_address, user_agent, metadata_json, occurred_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            )
            .bind(event.id)
            .bind(event.actor_id)
            .bind(event.subject_user_id)
            .bind(&event.action)
            .bind(&event.outcome)
            .bind(&event.request_id)
            .bind(&event.ip_address)
            .bind(&event.user_agent)
            .bind(&event.metadata_json)
            .bind(event.occurred_at)
            .execute(&self.pool)
            .await?;
            Ok(())
        }
        .await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }
}

#[derive(Clone)]
pub struct PgPasswordResetStore {
    pool: PgPool,
}

impl PgPasswordResetStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PasswordResetStorePort for PgPasswordResetStore {
    async fn save(
        &self,
        token: &crate::application::dto::repository_dto::AuthPasswordResetRecordDto,
    ) -> crate::application::errors::AuthResult<()> {
        let token = &crate::domain::password_reset::PasswordResetToken::from(token.clone());

        let result: anyhow::Result<_> = async {
        sqlx::query("insert into password_reset_tokens (id, user_id, token_hash, expires_at) values ($1, $2, $3, $4)")
            .bind(Uuid::new_v4()).bind(token.user_id).bind(&token.token_hash).bind(token.expires_at)
            .execute(&self.pool).await?;
        Ok(())
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn exists(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
    ) -> crate::application::errors::AuthResult<bool> {
        let result: anyhow::Result<_> = async {
        let found: Option<Uuid> = sqlx::query_scalar(
            "select id from password_reset_tokens where token_hash = $1 and expires_at > $2 limit 1",
        )
        .bind(token_hash)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        Ok(found.is_some())
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn consume(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
    ) -> crate::application::errors::AuthResult<
        Option<crate::application::dto::repository_dto::AuthPasswordResetRecordDto>,
    > {
        let result: anyhow::Result<_> = async {
        let row = sqlx::query("delete from password_reset_tokens where token_hash = $1 and expires_at > $2 returning user_id, token_hash, expires_at")
            .bind(token_hash).bind(now).fetch_optional(&self.pool).await?;
        Ok(row.map(|row| PasswordResetToken {
            user_id: row.get("user_id"),
            token_hash: row.get("token_hash"),
            expires_at: row.get("expires_at"),
        }))
    }.await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }
}

fn map_user(row: sqlx::postgres::PgRow) -> User {
    let status = match row.get::<String, _>("status").as_str() {
        "ACTIVE" => UserStatus::Active,
        "DELETED" => UserStatus::Deleted,
        _ => UserStatus::PendingVerification,
    };
    User {
        id: row.get("id"),
        primary_email: row.get("primary_email"),
        password_hash: row.get("password_hash"),
        status,
        last_login_at: row.get("last_login_at"),
    }
}

fn map_oauth_identity(row: sqlx::postgres::PgRow) -> OAuthIdentity {
    OAuthIdentity {
        user_id: row.get("user_id"),
        provider: parse_provider(row.get("provider_type")),
        subject: row
            .get::<Option<String>, _>("provider_subject")
            .unwrap_or_default(),
        email: row.get("email"),
        email_verified: row.get("email_verified"),
    }
}

fn parse_provider(value: String) -> IdentityProvider {
    match value.as_str() {
        "GOOGLE" => IdentityProvider::Google,
        "APPLE" => IdentityProvider::Apple,
        "X" => IdentityProvider::X,
        _ => IdentityProvider::Email,
    }
}

fn provider_name(provider: IdentityProvider) -> &'static str {
    match provider {
        IdentityProvider::Email => "EMAIL",
        IdentityProvider::Google => "GOOGLE",
        IdentityProvider::Apple => "APPLE",
        IdentityProvider::X => "X",
    }
}
