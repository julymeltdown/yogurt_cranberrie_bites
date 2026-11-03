//! Durable metadata-only at-least-once outbox. Leases fence acknowledgements, not
//! external effects; every downstream MUST durably deduplicate the outbox UUID.
use concert_application::commands::{CommandError, SafeReference};
use sqlx::{PgPool, Row};
use uuid::Uuid;
#[derive(Clone, Debug)]
pub struct Delivery {
    pub id: Uuid,
    pub receipt: Uuid,
    pub event: Uuid,
    pub reference: SafeReference,
    pub attempt: i32,
    pub lease: Uuid,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryFailure {
    Retryable,
    Rejected,
}
#[derive(Clone)]
pub struct PgOutbox {
    pool: PgPool,
}
impl PgOutbox {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    /// DB time, fixed 30-second lease, batch <=100, total attempts <=8. A crash
    /// consumes an attempt; expired attempt eight becomes DEAD, never success.
    pub async fn claim(&self, limit: i64) -> Result<Vec<Delivery>, CommandError> {
        if !(1..=100).contains(&limit) {
            return Err(CommandError::InvalidInput);
        }
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        sqlx::query("with exhausted as (select outbox_id from public.concert_outbox where state='LEASED' and lease_until<=clock_timestamp() and attempts>=8 order by lease_until,outbox_id for update skip locked limit $1) update public.concert_outbox o set state='DEAD',lease_token=null,lease_until=null,terminal_at=clock_timestamp(),last_error='ATTEMPTS_EXHAUSTED' from exhausted e where o.outbox_id=e.outbox_id").bind(limit).execute(&mut *tx).await.map_err(unavailable)?;
        let rows=sqlx::query("with eligible as (select outbox_id from public.concert_outbox where attempts<8 and ((state='READY' and available_at<=clock_timestamp()) or (state='LEASED' and lease_until<=clock_timestamp())) order by available_at,created_at,outbox_id for update skip locked limit $1) update public.concert_outbox o set state='LEASED',attempts=attempts+1,lease_token=$2,lease_until=clock_timestamp()+interval '30 seconds' from eligible e where o.outbox_id=e.outbox_id returning o.*")
            .bind(limit).bind(Uuid::new_v4()).fetch_all(&mut *tx).await.map_err(unavailable)?;
        let result = rows
            .into_iter()
            .map(|r| {
                Ok(Delivery {
                    id: r.try_get("outbox_id").map_err(unavailable)?,
                    receipt: r.try_get("receipt_id").map_err(unavailable)?,
                    event: r.try_get("event_id").map_err(unavailable)?,
                    reference: SafeReference {
                        resource: r.try_get("resource_id").map_err(unavailable)?,
                        revision: r.try_get("resource_revision").map_err(unavailable)?,
                    }
                    .validate()?,
                    attempt: r.try_get("attempts").map_err(unavailable)?,
                    lease: r.try_get("lease_token").map_err(unavailable)?,
                })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        tx.commit().await.map_err(unavailable)?;
        Ok(result)
    }
    pub async fn acknowledge(&self, delivery: &Delivery) -> Result<bool, CommandError> {
        let result=sqlx::query("update public.concert_outbox set state='DELIVERED',lease_token=null,lease_until=null,terminal_at=clock_timestamp(),last_error=null where outbox_id=$1 and state='LEASED' and lease_token=$2 and attempts=$3 and lease_until>clock_timestamp()")
            .bind(delivery.id).bind(delivery.lease).bind(delivery.attempt).execute(&self.pool).await.map_err(unavailable)?;
        Ok(result.rows_affected() == 1)
    }
    pub async fn fail(
        &self,
        delivery: &Delivery,
        failure: DeliveryFailure,
    ) -> Result<bool, CommandError> {
        let terminal = delivery.attempt >= 8 || failure == DeliveryFailure::Rejected;
        let error = if failure == DeliveryFailure::Rejected {
            "REJECTED"
        } else if terminal {
            "ATTEMPTS_EXHAUSTED"
        } else {
            "RETRYABLE"
        };
        // 2,4,8,...128 seconds, never unbounded or caller-controlled text.
        let backoff = 2_i32.pow(delivery.attempt.clamp(1, 7) as u32);
        let result=sqlx::query("update public.concert_outbox set state=case when $4 then 'DEAD' else 'READY' end,lease_token=null,lease_until=null,terminal_at=case when $4 then clock_timestamp() else null end,last_error=$5,available_at=clock_timestamp()+make_interval(secs=>$6) where outbox_id=$1 and state='LEASED' and lease_token=$2 and attempts=$3 and lease_until>clock_timestamp()")
            .bind(delivery.id).bind(delivery.lease).bind(delivery.attempt).bind(terminal).bind(error).bind(f64::from(backoff)).execute(&self.pool).await.map_err(unavailable)?;
        Ok(result.rows_affected() == 1)
    }
    /// Seven-day terminal diagnostics retention, at most 1000 per call. Receipt
    /// tombstones remain; pruning must never make a command executable again.
    pub async fn prune(&self, limit: i64) -> Result<u64, CommandError> {
        if !(1..=1000).contains(&limit) {
            return Err(CommandError::InvalidInput);
        }
        let result=sqlx::query("with old as (select outbox_id from public.concert_outbox where state in ('DELIVERED','DEAD') and terminal_at<clock_timestamp()-interval '7 days' order by terminal_at,outbox_id for update skip locked limit $1) delete from public.concert_outbox o using old where o.outbox_id=old.outbox_id").bind(limit).execute(&self.pool).await.map_err(unavailable)?;
        Ok(result.rows_affected())
    }
}
fn unavailable(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
