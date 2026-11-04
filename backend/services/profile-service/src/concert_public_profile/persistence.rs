//! Durable Profile-owner visibility hold. No cross-service tables or cached allows.
use super::{HeldPublicRecord, PublicProfileRepository, PublicRecord, ReadBinding, ReadError};
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use std::time::Duration;
use tokio::time::Instant;
#[derive(Clone)]
pub struct PgPublicProfileRepository {
    pool: PgPool,
}
impl PgPublicProfileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    async fn snapshot(
        &self,
        binding: ReadBinding,
        stop: Instant,
    ) -> Result<HeldPublicRecord, ReadError> {
        let child_id = binding.new_child_id()?;
        let secret = uuid::Uuid::new_v4().simple().to_string();
        remaining(stop)?;
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        sqlx::query("select set_config('statement_timeout', $1, true)")
            .bind(remaining(stop)?.to_string())
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        let slots: i32 =
            sqlx::query_scalar("select current_setting('max_prepared_transactions')::int")
                .fetch_one(&mut *tx)
                .await
                .map_err(unavailable)?;
        if slots < 256 {
            return Err(ReadError::Unavailable);
        }
        // A distinct transactional advisory slot stays in the prepared xact.
        // Fixed two-int namespace does not overlap retained one-bigint subject locks.
        let mut enrolled = false;
        for slot in 0i32..101 {
            remaining(stop)?;
            let acquired: bool =
                sqlx::query_scalar("select pg_try_advisory_xact_lock(1296257616::int,$1::int)")
                    .bind(slot)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(unavailable)?;
            if acquired {
                enrolled = true;
                break;
            }
        }
        if !enrolled {
            return Err(ReadError::Unavailable);
        }
        // Tombstone table before profile row matches the retained deletion saga's
        // insert→profile-update order. SHARE also fences direct tombstone inserts.
        sqlx::query("lock table public.profile_user_deletion_tombstones in share mode")
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        remaining(stop)?;
        let row=sqlx::query("select p.user_id,p.nickname,p.bio,statement_timestamp() as observed_at
            from public.profiles p where p.user_id=$1 and p.deleted_at is null
            and p.account_visibility='PUBLIC'
            and char_length(p.nickname) between 1 and 40 and octet_length(p.nickname)<=160
            and char_length(p.bio)<=160 and octet_length(p.bio)<=640
            and not exists(select 1 from public.profile_user_deletion_tombstones d where d.user_id=p.user_id)
            for share of p")
            .bind(binding.subject).fetch_optional(&mut *tx).await.map_err(unavailable)?.ok_or(ReadError::Hidden)?;
        remaining(stop)?;
        let bio: String = row.try_get("bio").map_err(unavailable)?;
        let content = PublicRecord {
            subject: row.try_get("user_id").map_err(unavailable)?,
            display_name: row.try_get("nickname").map_err(unavailable)?,
            bio: if bio.is_empty() { None } else { Some(bio) },
            observed_at: row.try_get("observed_at").map_err(unavailable)?,
        };
        content.validate(binding.subject)?;
        remaining(stop)?;
        let digest: String = sqlx::query_scalar(
            "select translate(rtrim(encode(sha256($1::bytea),'base64'),'='),'+/','-_')",
        )
        .bind(completion_bytes(&child_id, &secret))
        .fetch_one(&mut *tx)
        .await
        .map_err(unavailable)?;
        let gid = binding.prepared_id(&child_id, &digest)?;
        remaining(stop)?;
        // GID uses validated compact scope, UUID nonce and SHA-256 digest (187 bytes); never
        // interpolate request strings or a caller-supplied SQL identifier here.
        sqlx::query(&format!("prepare transaction '{gid}'"))
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        // SQLx transaction bookkeeping is cleared after PREPARE; this COMMIT has
        // no active transaction and cannot release the independent prepared xact.
        tx.commit().await.map_err(unavailable)?;
        // A failure after PREPARE intentionally leaves quarantine. No Drop/TTL release.
        remaining(stop)?;
        Ok(HeldPublicRecord {
            content,
            child_id,
            completion_secret: secret,
        })
    }
    async fn held(&self, binding: ReadBinding, child_id: &str) -> Result<(), ReadError> {
        binding.validate_child(child_id)?;
        let held:bool=sqlx::query_scalar("select exists(select 1 from pg_prepared_xacts where database=current_database() and owner=current_user and starts_with(gid,$1))")
            .bind(format!("{child_id}:")).fetch_one(&self.pool).await.map_err(unavailable)?;
        if held {
            Ok(())
        } else {
            Err(ReadError::Hidden)
        }
    }
}
#[async_trait]
impl PublicProfileRepository for PgPublicProfileRepository {
    async fn read_public(&self, binding: ReadBinding) -> Result<HeldPublicRecord, ReadError> {
        let stop = stop();
        let result = tokio::time::timeout_at(stop, self.snapshot(binding, stop))
            .await
            .map_err(|_| ReadError::Unavailable)?;
        remaining(stop)?;
        result
    }
    async fn check(&self, binding: ReadBinding, child_id: &str) -> Result<(), ReadError> {
        let stop = stop();
        let result = tokio::time::timeout_at(stop, self.held(binding, child_id))
            .await
            .map_err(|_| ReadError::Unavailable)?;
        remaining(stop)?;
        result
    }
    async fn complete(
        &self,
        binding: ReadBinding,
        child_id: &str,
        secret: &str,
    ) -> Result<(), ReadError> {
        binding.validate_completion(child_id, secret)?;
        let stop = stop();
        let result=tokio::time::timeout_at(stop,async {
            let digest:String=sqlx::query_scalar("select translate(rtrim(encode(sha256($1::bytea),'base64'),'='),'+/','-_')")
                .bind(completion_bytes(child_id,secret)).fetch_one(&self.pool).await.map_err(unavailable)?;
            let gid=binding.prepared_id(child_id,&digest)?;
            // Exact matching ID+completion digest, caller owner only. ROLLBACK
            // is the terminal action for a read-only held transaction.
            let exists:bool=sqlx::query_scalar("select exists(select 1 from pg_prepared_xacts where database=current_database() and owner=current_user and gid=$1)")
                .bind(&gid).fetch_one(&self.pool).await.map_err(unavailable)?;
            if !exists{return Err(ReadError::Hidden);}
            sqlx::query(&format!("rollback prepared '{gid}'")).execute(&self.pool).await.map_err(unavailable)?;
            Ok(())
        }).await.map_err(|_|ReadError::Unavailable)?;
        remaining(stop)?;
        result
    }
}
fn stop() -> Instant {
    let ceiling = Instant::now() + Duration::from_secs(2);
    platform::call_deadline::current()
        .map(Instant::from_std)
        .map_or(ceiling, |v| v.min(ceiling))
}
fn unavailable(_: sqlx::Error) -> ReadError {
    ReadError::Unavailable
}
fn remaining(stop: Instant) -> Result<u128, ReadError> {
    let millis = stop.saturating_duration_since(Instant::now()).as_millis();
    if millis == 0 {
        Err(ReadError::Unavailable)
    } else {
        Ok(millis)
    }
}

fn completion_bytes(child: &str, secret: &str) -> Vec<u8> {
    format!("mcr public Profile completion v1|{child}|{secret}").into_bytes()
}
