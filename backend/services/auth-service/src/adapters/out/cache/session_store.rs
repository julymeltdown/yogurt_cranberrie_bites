use crate::application::auth_runtime::AuthRuntimePort;
use crate::application::ports::RefreshTokenStorePort;
use crate::domain::session::{RefreshTokenRecord, RefreshTokenState, RotationOutcome};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use std::sync::Arc;

#[derive(Clone)]
pub struct PgRefreshTokenStore {
    pool: PgPool,
    runtime: Arc<dyn AuthRuntimePort>,
}

impl PgRefreshTokenStore {
    pub fn new(pool: PgPool, runtime: Arc<dyn AuthRuntimePort>) -> Self {
        Self { pool, runtime }
    }

    async fn begin(&self) -> anyhow::Result<sqlx::Transaction<'static, sqlx::Postgres>> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("select set_config('lock_timeout', '2s', true), set_config('statement_timeout', '5s', true)").execute(&mut *tx).await?;
        Ok(tx)
    }
}

#[async_trait]
impl RefreshTokenStorePort for PgRefreshTokenStore {
    async fn store(
        &self,
        record: &crate::application::dto::repository_dto::AuthRefreshRecordDto,
    ) -> crate::application::errors::AuthResult<()> {
        let record = &crate::domain::session::RefreshTokenRecord::from(record.clone());

        let result: anyhow::Result<_> = async {
        sqlx::query("insert into refresh_tokens (jti, token_hash, user_id, family_id, expires_at, revoked_at) values ($1, $2, $3, $4, $5, null)")
            .bind(&record.jti).bind(&record.token_hash).bind(record.user_id)
            .bind(record.family_id).bind(record.expires_at)
            .execute(&self.pool).await?;
        Ok(())
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn find(
        &self,
        jti: &str,
    ) -> crate::application::errors::AuthResult<
        Option<crate::application::dto::repository_dto::AuthRefreshRecordDto>,
    > {
        let result: anyhow::Result<_> = async {
        let row = sqlx::query("select jti, token_hash, user_id, family_id, expires_at from refresh_tokens where jti = $1 and revoked_at is null and expires_at > $2")
            .bind(jti).bind(self.runtime.now()).fetch_optional(&self.pool).await?;
        Ok(row.map(|row| RefreshTokenRecord {
            jti: row.get("jti"),
            token_hash: row.get("token_hash"),
            user_id: row.get("user_id"),
            family_id: row.get("family_id"),
            expires_at: row.get("expires_at"),
        }))
    }.await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }

    async fn rotate(
        &self,
        current_jti: &str,
        current_hash: &str,
        next: &crate::application::dto::repository_dto::AuthRefreshRecordDto,
    ) -> crate::application::errors::AuthResult<
        crate::application::dto::repository_dto::AuthRotationOutcomeDto,
    > {
        let next = &crate::domain::session::RefreshTokenRecord::from(next.clone());

        let result: anyhow::Result<_> = async {
        let mut tx = self.begin().await?;
        let active: Option<uuid::Uuid> = sqlx::query_scalar("select id from users where id = $1 and status = 'ACTIVE' and deleted_at is null for update")
            .bind(next.user_id).fetch_optional(&mut *tx).await?;
        if active.is_none() {
            return Ok(RotationOutcome::NotRotatable);
        }

        // Read the current row under the same transaction that will update it, so
        // two concurrent refreshes cannot both observe "active" and both rotate.
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
        let expires_at: DateTime<Utc> = current.get("expires_at");
        // Sample only after both ownership locks are acquired.
        let decision_at = self.runtime.now();
        let expired = expires_at <= decision_at;
        let revoked: Option<DateTime<Utc>> = current.get("revoked_at");
        let revoked = revoked.is_some();
        // A different token presented under this jti is a mismatch, never a race.
        if stored_hash != current_hash || expired {
            return Ok(RotationOutcome::NotRotatable);
        }
        if revoked {
            let rotated_at: Option<DateTime<Utc>> = current.get("rotated_at");
            let successor_jti: Option<String> = current.get("successor_jti");
            if let (Some(rotated_at), Some(successor_jti)) = (rotated_at, successor_jti) {
                tx.commit().await?;
                return Ok(RotationOutcome::AlreadyRotated {
                    successor_jti,
                    rotated_at,
                });
            }
            return Ok(RotationOutcome::NotRotatable);
        }

        let family_id: uuid::Uuid = current.get("family_id");
        anyhow::ensure!(
            family_id == next.family_id,
            "replacement token must continue its own family"
        );

        // Mark the predecessor superseded rather than merely revoked. The reason
        // column is what lets a later presentation be classified as a benign race
        // instead of reuse.
        let updated = sqlx::query(
            "update refresh_tokens
             set revoked_at = $3, rotated_at = $3, successor_jti = $2,
                 revoked_reason = 'ROTATED'
             where jti = $1 and revoked_at is null",
        )
        .bind(current_jti)
        .bind(&next.jti)
        .bind(decision_at)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if updated != 1 {
            // Another transaction rotated it between our read and write.
            return Ok(RotationOutcome::NotRotatable);
        }
        // A failed replacement insert rolls the predecessor update back, so the
        // original token is never revoked without a usable successor.
        anyhow::ensure!(
            next.expires_at > self.runtime.now(),
            "Invalid session expiry"
        );
        sqlx::query("insert into refresh_tokens (jti, token_hash, user_id, family_id, expires_at, revoked_at) values ($1, $2, $3, $4, $5, null)")
            .bind(&next.jti).bind(&next.token_hash).bind(next.user_id)
            .bind(next.family_id).bind(next.expires_at)
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(RotationOutcome::Rotated)
    }.await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.into())
    }

    async fn find_state(
        &self,
        jti: &str,
        now: DateTime<Utc>,
    ) -> crate::application::errors::AuthResult<
        Option<crate::application::dto::repository_dto::AuthRefreshStateDto>,
    > {
        let result: anyhow::Result<_> = async {
            let row = sqlx::query(
                "select user_id, family_id, rotated_at, successor_jti, revoked_reason, expires_at
             from refresh_tokens where jti = $1",
            )
            .bind(jti)
            .fetch_optional(&self.pool)
            .await?;
            let Some(row) = row else {
                return Ok(None);
            };
            // Expiry is reported as no state: an expired token is simply not usable,
            // and a client refreshing one is expected to log in again.
            let expires_at: DateTime<Utc> = row.get("expires_at");
            if expires_at <= now {
                return Ok(None);
            }
            if let (Some(rotated_at), Some(successor_jti)) = (
                row.get::<Option<DateTime<Utc>>, _>("rotated_at"),
                row.get::<Option<String>, _>("successor_jti"),
            ) {
                return Ok(Some(RefreshTokenState::Rotated {
                    successor_jti,
                    rotated_at,
                    family_id: row.get("family_id"),
                    user_id: row.get("user_id"),
                }));
            }
            let reason: Option<String> = row.get("revoked_reason");
            match reason.as_deref() {
                Some("LOGOUT") => Ok(Some(RefreshTokenState::RevokedByLogout)),
                Some(_) => Ok(Some(RefreshTokenState::Revoked)),
                // Revoked with no recorded reason: treat it as revoked, which is the
                // conservative reading, rather than as a benign stale logout.
                None => Ok(Some(RefreshTokenState::Revoked)),
            }
        }
        .await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }

    async fn revoke(&self, jti: &str) -> crate::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
        let mut tx = self.begin().await?;
        let owner: Option<(uuid::Uuid, uuid::Uuid)> =
            sqlx::query_as("select user_id, family_id from refresh_tokens where jti=$1")
                .bind(jti)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some((user, family)) = owner {
            // Rotation and logout serialize on the same user before token rows.
            // An ancestor identifies the session even if rotation wins first.
            lock_session_owner(&mut tx, user).await?;
            sqlx::query("update refresh_tokens set revoked_at=$2, revoked_reason='LOGOUT' where family_id=$1 and revoked_at is null")
                .bind(family).bind(self.runtime.now()).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn revoke_family(
        &self,
        family_id: uuid::Uuid,
    ) -> crate::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
        let mut tx = self.begin().await?;
        let owner: Option<uuid::Uuid> =
            sqlx::query_scalar("select user_id from refresh_tokens where family_id=$1 limit 1")
                .bind(family_id)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some(user) = owner {
            lock_session_owner(&mut tx, user).await?;
            sqlx::query("update refresh_tokens set revoked_at=$2, revoked_reason='FAMILY_REUSE' where family_id=$1 and revoked_at is null")
                .bind(family_id).bind(self.runtime.now()).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn revoke_all_for_user(
        &self,
        user_id: uuid::Uuid,
    ) -> crate::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
        let mut tx = self.begin().await?;
        lock_session_owner(&mut tx, user_id).await?;
        sqlx::query("update refresh_tokens set revoked_at=$2, revoked_reason='USER_REVOKED' where user_id=$1 and revoked_at is null")
            .bind(user_id).bind(self.runtime.now()).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn session_is_live(
        &self,
        user_id: uuid::Uuid,
        family_id: uuid::Uuid,
        now: DateTime<Utc>,
    ) -> crate::application::errors::AuthResult<bool> {
        let result: anyhow::Result<_> = async {
            // A rotation revokes the predecessor and inserts the successor in one
            // transaction, so a live family always has exactly one live row.
            let live: bool = sqlx::query_scalar(
                "select exists (
                 select 1 from refresh_tokens
                 where family_id = $1 and user_id = $2
                   and revoked_at is null and expires_at > $3
             )",
            )
            .bind(family_id)
            .bind(user_id)
            .bind(now)
            .fetch_one(&self.pool)
            .await?;
            Ok(live)
        }
        .await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }
}

async fn lock_session_owner(
    connection: &mut sqlx::PgConnection,
    user: uuid::Uuid,
) -> anyhow::Result<()> {
    sqlx::query("select id from users where id=$1 for update")
        .bind(user)
        .execute(connection)
        .await?;
    Ok(())
}
