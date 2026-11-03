use crate::application::dto::auth_dto::AuthLifecycleCleanupResult;
use crate::application::ports::AuthLifecycleCleanupPort;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Clone)]
pub struct PgAuthLifecycleCleanupRepository {
    pool: PgPool,
}

impl PgAuthLifecycleCleanupRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuthLifecycleCleanupPort for PgAuthLifecycleCleanupRepository {
    async fn cleanup_expired(
        &self,
        now: DateTime<Utc>,
    ) -> crate::application::errors::AuthResult<AuthLifecycleCleanupResult> {
        let result: anyhow::Result<_> = async {
            let mut tx = self.pool.begin().await?;
            let deleted_email_verifications =
                sqlx::query("delete from email_verifications where expires_at <= $1")
                    .bind(now)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected() as i64;
            let deleted_password_reset_tokens =
                sqlx::query("delete from password_reset_tokens where expires_at <= $1")
                    .bind(now)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected() as i64;
            let deleted_refresh_tokens =
                sqlx::query("delete from refresh_tokens where expires_at <= $1")
                    .bind(now)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected() as i64;
            let deleted_oauth_authorizations =
                sqlx::query("delete from oauth_authorizations where expires_at <= $1")
                    .bind(now)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected() as i64;
            let deleted_users = sqlx::query(
                r#"
            delete from users u
            where u.status = 'DELETED'
              and u.retention_until <= $1
              and not exists (
                  select 1
                  from user_deletion_sagas saga
                  where saga.user_id = u.id
                    and (
                        saga.status <> 'COMPLETED'
                        or exists (
                            select 1
                            from user_deletion_steps step
                            where step.saga_id = saga.id
                              and step.status <> 'SUCCEEDED'
                        )
                    )
              )
            "#,
            )
            .bind(now)
            .execute(&mut *tx)
            .await?
            .rows_affected() as i64;
            tx.commit().await?;

            Ok(AuthLifecycleCleanupResult {
                deleted_email_verifications,
                deleted_password_reset_tokens,
                deleted_refresh_tokens,
                deleted_oauth_authorizations,
                deleted_users,
            })
        }
        .await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }
}
