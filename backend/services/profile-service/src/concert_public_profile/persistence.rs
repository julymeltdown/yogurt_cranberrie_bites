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
        child_id: &str,
        secret: &str,
        stop: Instant,
    ) -> Result<HeldPublicRecord, ReadError> {
        binding.validate_completion(child_id, secret)?;
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
        // Capture the actual transaction before reserving its independently
        // committed control receipt. A cancelled pre-dispatch intent wins the
        // unique insert and prevents this transaction from ever preparing.
        let identity = sqlx::query("select (pg_control_system()).system_identifier::text system_identifier,(select oid::bigint from pg_database where datname=current_database()) database_oid,(select oid::bigint from pg_roles where rolname=current_user) owner_oid,pg_current_xact_id()::text full_xid")
            .fetch_one(&mut *tx).await.map_err(unavailable)?;
        let digest: String = sqlx::query_scalar(
            "select translate(rtrim(encode(sha256($1::bytea),'base64'),'='),'+/','-_')",
        )
        .bind(completion_bytes(child_id, secret))
        .fetch_one(&mut *tx)
        .await
        .map_err(unavailable)?;
        let registered = sqlx::query("insert into public.concert_public_profile_children(child_id,completion_digest,system_identifier,database_oid,owner_oid,full_xid,state) values($1,$2,$3,$4,$5,$6,'REGISTERED') on conflict(child_id) do nothing")
            .bind(child_id).bind(&digest)
            .bind(identity.try_get::<String,_>("system_identifier").map_err(unavailable)?)
            .bind(identity.try_get::<i64,_>("database_oid").map_err(unavailable)?)
            .bind(identity.try_get::<i64,_>("owner_oid").map_err(unavailable)?)
            .bind(identity.try_get::<String,_>("full_xid").map_err(unavailable)?)
            .execute(&self.pool).await.map_err(unavailable)?.rows_affected();
        if registered != 1 {
            return Err(ReadError::Hidden);
        }
        remaining(stop)?;
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
        statement_budget(&mut tx, stop).await?;
        sqlx::query("lock table public.profile_user_deletion_tombstones in share mode")
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        remaining(stop)?;
        statement_budget(&mut tx, stop).await?;
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
        let gid = binding.prepared_id(child_id, &digest)?;
        remaining(stop)?;
        // GID uses validated compact scope, UUID nonce and SHA-256 digest (187 bytes); never
        // interpolate request strings or a caller-supplied SQL identifier here.
        statement_budget(&mut tx, stop).await?;
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
            child_id: child_id.to_owned(),
            completion_secret: secret.to_owned(),
        })
    }
    async fn held(&self, binding: ReadBinding, child_id: &str) -> Result<(), ReadError> {
        binding.validate_child(child_id)?;
        let receipt = self.receipt(child_id).await?;
        if receipt.try_get::<String, _>("state").map_err(unavailable)? != "REGISTERED" {
            return Err(ReadError::Hidden);
        }
        let digest: String = receipt.try_get("completion_digest").map_err(unavailable)?;
        let gid = binding.prepared_id(child_id, &digest)?;
        let xid: String = receipt.try_get("full_xid").map_err(unavailable)?;
        let held:bool=sqlx::query_scalar("select exists(select 1 from pg_prepared_xacts where database=current_database() and owner=current_user and gid=$1 and transaction::text::numeric=mod($2::text::numeric,4294967296))")
            .bind(gid).bind(xid).fetch_one(&self.pool).await.map_err(unavailable)?;
        if held {
            Ok(())
        } else {
            Err(ReadError::Hidden)
        }
    }
    async fn receipt(&self, child_id: &str) -> Result<sqlx::postgres::PgRow, ReadError> {
        sqlx::query("select * from public.concert_public_profile_children where child_id=$1 and system_identifier=(pg_control_system()).system_identifier::text and database_oid=(select oid::bigint from pg_database where datname=current_database()) and owner_oid=(select oid::bigint from pg_roles where rolname=current_user)")
            .bind(child_id).fetch_optional(&self.pool).await.map_err(unavailable)?.ok_or(ReadError::Unavailable)
    }
}
#[async_trait]
impl PublicProfileRepository for PgPublicProfileRepository {
    async fn read_public(
        &self,
        binding: ReadBinding,
        child_id: &str,
        secret: &str,
    ) -> Result<HeldPublicRecord, ReadError> {
        let stop = stop();
        let result = tokio::time::timeout_at(stop, self.snapshot(binding, child_id, secret, stop))
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
            // A terminal pre-dispatch cancellation reserves the exact nonce.
            // A later ReadCurrent cannot overwrite it or create a hold.
            sqlx::query("insert into public.concert_public_profile_children(child_id,completion_digest,system_identifier,database_oid,owner_oid,state,settled_at) select $1,$2,(pg_control_system()).system_identifier::text,(select oid::bigint from pg_database where datname=current_database()),(select oid::bigint from pg_roles where rolname=current_user),'CANCELLED',clock_timestamp() on conflict(child_id) do nothing")
                .bind(child_id).bind(&digest).execute(&self.pool).await.map_err(unavailable)?;
            let receipt=self.receipt(child_id).await?;
            if receipt.try_get::<String,_>("completion_digest").map_err(unavailable)? != digest {return Err(ReadError::Hidden);}
            let state:String=receipt.try_get("state").map_err(unavailable)?;
            if state == "CANCELLED" || state == "SETTLED" { return Ok(()); }
            if state != "REGISTERED" {return Err(ReadError::Unavailable);}
            let xid:String=receipt.try_get("full_xid").map_err(unavailable)?;
            let gid=binding.prepared_id(child_id,&digest)?;
            let actual:Option<String>=sqlx::query_scalar("select transaction::text from pg_prepared_xacts where database=current_database() and owner=current_user and gid=$1")
                .bind(&gid).fetch_optional(&self.pool).await.map_err(unavailable)?;
            if let Some(actual) = actual {
                let low = xid.parse::<u64>().map_err(|_|ReadError::Unavailable)? % 4_294_967_296;
                if actual != low.to_string() { return Err(ReadError::Unavailable); }
                // Concurrent completion may already have rolled this back. Only
                // the recorded actual transaction's aborted status proves success.
                let _ = sqlx::query(&format!("rollback prepared '{gid}'")).execute(&self.pool).await;
            }
            let status:Option<String>=sqlx::query_scalar("select pg_xact_status($1::text::xid8)").bind(&xid)
                .fetch_one(&self.pool).await.map_err(unavailable)?;
            if status.as_deref() != Some("aborted") {return Err(ReadError::Unavailable);}
            let updated=sqlx::query("update public.concert_public_profile_children set state='SETTLED',settled_at=clock_timestamp() where child_id=$1 and completion_digest=$2 and full_xid=$3 and state in ('REGISTERED','SETTLED')")
                .bind(child_id).bind(&digest).bind(&xid).execute(&self.pool).await.map_err(unavailable)?.rows_affected();
            if updated != 1 {return Err(ReadError::Unavailable);}
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

async fn statement_budget(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    stop: Instant,
) -> Result<(), ReadError> {
    sqlx::query("select set_config('statement_timeout',$1,true)")
        .bind(remaining(stop)?.to_string())
        .execute(&mut **tx)
        .await
        .map_err(unavailable)?;
    remaining(stop)?;
    Ok(())
}
