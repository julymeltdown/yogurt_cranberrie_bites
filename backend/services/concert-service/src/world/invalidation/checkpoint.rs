//! Atomic local effects plus applied owner position. No producer acknowledgment.
use super::{
    remote::{owner_name, RemoteVictim, Signal, MAX_VICTIMS},
    ApplyError, Victim,
};
use crate::adapters::out::relationship_authority::invalidation::{
    OwnerCursor, VerifiedOwnerHead, VerifiedOwnerPage,
};
use sqlx::{postgres::PgRow, PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

fn unavailable(_: impl std::fmt::Debug) -> ApplyError {
    ApplyError::Unavailable
}
async fn settings(tx: &mut Transaction<'_, Postgres>) -> Result<(), ApplyError> {
    sqlx::query("set local statement_timeout='900ms'")
        .execute(&mut **tx)
        .await
        .map_err(unavailable)?;
    sqlx::query("set local lock_timeout='100ms'")
        .execute(&mut **tx)
        .await
        .map_err(unavailable)?;
    Ok(())
}
async fn lock_run(
    tx: &mut Transaction<'_, Postgres>,
    consumer: Uuid,
    run: Uuid,
) -> Result<(), ApplyError> {
    let n=sqlx::query("update concert_world_remote_runs set lease_until=clock_timestamp()+interval '2 seconds',updated_at=clock_timestamp() where consumer_id=$1 and run_epoch=$2 and lease_until>clock_timestamp()")
        .bind(consumer).bind(run).execute(&mut **tx).await.map_err(unavailable)?.rows_affected();
    if n != 1 {
        return Err(ApplyError::Unbound);
    }
    Ok(())
}

/// The caller has already fenced every old native view, pending publication and
/// parent operation. This transaction invalidates this stable consumer's old
/// persistent generations before any ReadHead is permitted.
pub(super) async fn boot(
    pool: &PgPool,
    consumer: Uuid,
    run: Uuid,
    events: &[Uuid],
) -> Result<(), ApplyError> {
    let mut tx = pool.begin().await.map_err(unavailable)?;
    settings(&mut tx).await?;
    let n=sqlx::query("insert into concert_world_remote_runs(consumer_id,run_epoch,lease_until) values($1,$2,clock_timestamp()+interval '2 seconds') on conflict(consumer_id) do update set run_epoch=excluded.run_epoch,lease_until=excluded.lease_until,updated_at=clock_timestamp() where concert_world_remote_runs.lease_until<=clock_timestamp() or concert_world_remote_runs.run_epoch=$2")
        .bind(consumer).bind(run).execute(&mut *tx).await.map_err(unavailable)?.rows_affected();
    if n != 1 {
        return Err(ApplyError::Unbound);
    }
    let rows=sqlx::query("update concert_world_memberships set disconnected=true where event_id=any($1) and owner_process_id=$2 and not disconnected returning user_id")
        .bind(events).bind(consumer.to_string()).fetch_all(&mut *tx).await.map_err(unavailable)?;
    if rows.len() > MAX_VICTIMS {
        return Err(ApplyError::Unavailable);
    }
    tx.commit().await.map_err(unavailable)
}
pub(super) async fn baseline(
    pool: &PgPool,
    consumer: Uuid,
    run: Uuid,
    head: &VerifiedOwnerHead,
) -> Result<OwnerCursor, ApplyError> {
    if head.consumer_id() != consumer
        || head.query().after().is_some()
        || head.query().limit().is_some()
    {
        return Err(ApplyError::UnqualifiedSource);
    }
    let mut tx = pool.begin().await.map_err(unavailable)?;
    settings(&mut tx).await?;
    lock_run(&mut tx, consumer, run).await?;
    let r=sqlx::query("select source_epoch,sequence from concert_world_remote_checkpoints where consumer_id=$1 and source_owner=$2 for update")
        .bind(consumer).bind(owner_name(head.owner())).fetch_optional(&mut *tx).await.map_err(unavailable)?;
    let cursor = if let Some(r) = r {
        let epoch: Uuid = r.try_get("source_epoch").map_err(unavailable)?;
        let sequence: i64 = r.try_get("sequence").map_err(unavailable)?;
        if epoch == head.cursor().epoch() {
            if sequence > head.cursor().sequence() {
                return Err(ApplyError::Unbound);
            }
            OwnerCursor::new(head.owner(), epoch, sequence).map_err(unavailable)?
        } else {
            // Only this full invalidating bootstrap may establish a new epoch.
            head.cursor()
        }
    } else {
        head.cursor()
    };
    sqlx::query("insert into concert_world_remote_checkpoints(consumer_id,source_owner,source_epoch,sequence) values($1,$2,$3,$4) on conflict(consumer_id,source_owner) do update set source_epoch=excluded.source_epoch,sequence=excluded.sequence,updated_at=clock_timestamp()")
        .bind(consumer).bind(owner_name(cursor.owner())).bind(cursor.epoch()).bind(cursor.sequence()).execute(&mut *tx).await.map_err(unavailable)?;
    tx.commit().await.map_err(unavailable)?;
    Ok(cursor)
}
pub(super) async fn stop(pool: &PgPool, consumer: Uuid, run: Uuid) -> Result<(), ApplyError> {
    sqlx::query("update concert_world_remote_runs set lease_until=clock_timestamp(),updated_at=clock_timestamp() where consumer_id=$1 and run_epoch=$2")
        .bind(consumer).bind(run).execute(pool).await.map_err(unavailable)?;
    Ok(())
}

pub(super) struct Prepared {
    tx: Transaction<'static, Postgres>,
    consumer: Uuid,
    after: OwnerCursor,
    next: OwnerCursor,
    pub signals: Vec<Signal>,
    pub previous: Option<Vec<Vec<RemoteVictim>>>,
}
fn exact(row: &PgRow, s: &Signal) -> Result<Vec<RemoteVictim>, ApplyError> {
    if row
        .try_get::<Uuid, _>("source_epoch")
        .map_err(unavailable)?
        != s.cursor.epoch()
        || row.try_get::<i64, _>("sequence").map_err(unavailable)? != s.cursor.sequence()
        || row.try_get::<Uuid, _>("signal_id").map_err(unavailable)? != s.id
        || row.try_get::<String, _>("reason").map_err(unavailable)? != s.reason
        || row
            .try_get::<String, _>("scope_kind")
            .map_err(unavailable)?
            != s.kind
        || row.try_get::<Uuid, _>("scope_a").map_err(unavailable)? != s.a
        || row.try_get::<Uuid, _>("scope_b").map_err(unavailable)? != s.b
    {
        return Err(ApplyError::Unbound);
    }
    let victims: Vec<RemoteVictim> =
        serde_json::from_value(row.try_get("victims").map_err(unavailable)?)
            .map_err(unavailable)?;
    if victims.len() > MAX_VICTIMS
        || victims.iter().any(|v| {
            v.event.is_nil()
                || v.victim.user.is_nil()
                || v.victim.room.is_nil()
                || v.victim.family.is_nil()
                || v.victim.connection.is_nil()
                || v.victim.epoch.is_nil()
                || Uuid::parse_str(&v.victim.owner).is_err()
        })
    {
        return Err(ApplyError::Unbound);
    }
    Ok(victims)
}
pub(super) async fn prepare(
    pool: &PgPool,
    consumer: Uuid,
    run: Uuid,
    expected: OwnerCursor,
    page: &VerifiedOwnerPage,
) -> Result<Prepared, ApplyError> {
    if page.consumer_id() != consumer
        || page.owner() != expected.owner()
        || page.query().after() != Some(expected)
        || page.head().epoch() != expected.epoch()
        || page.next().epoch() != expected.epoch()
        || !page.query().limit().is_some_and(|n| (1..=256).contains(&n))
        || page.signals().len() > 256
    {
        return Err(ApplyError::UnqualifiedSource);
    }
    let signals = page
        .signals()
        .iter()
        .map(Signal::from_verified)
        .collect::<Result<Vec<_>, _>>()?;
    let mut sequence = expected.sequence();
    for s in &signals {
        sequence = sequence.checked_add(1).ok_or(ApplyError::Unbound)?;
        if s.cursor.owner() != expected.owner()
            || s.cursor.epoch() != expected.epoch()
            || s.cursor.sequence() != sequence
        {
            return Err(ApplyError::Unbound);
        }
    }
    if sequence != page.next().sequence() || page.next().sequence() > page.head().sequence() {
        return Err(ApplyError::Unbound);
    }
    let mut tx = pool.begin().await.map_err(unavailable)?;
    settings(&mut tx).await?;
    lock_run(&mut tx, consumer, run).await?;
    let row=sqlx::query("select source_epoch,sequence from concert_world_remote_checkpoints where consumer_id=$1 and source_owner=$2 for update")
        .bind(consumer).bind(owner_name(expected.owner())).fetch_optional(&mut *tx).await.map_err(unavailable)?.ok_or(ApplyError::Unbound)?;
    if row
        .try_get::<Uuid, _>("source_epoch")
        .map_err(unavailable)?
        != expected.epoch()
    {
        return Err(ApplyError::Unbound);
    }
    let current: i64 = row.try_get("sequence").map_err(unavailable)?;
    let ids: Vec<_> = signals.iter().map(|s| s.id).collect();
    let rows=sqlx::query("select * from concert_world_remote_effects where consumer_id=$1 and source_owner=$2 and ((source_epoch=$3 and sequence>$4 and sequence<=$5) or signal_id=any($6))")
        .bind(consumer).bind(owner_name(expected.owner())).bind(expected.epoch()).bind(expected.sequence()).bind(page.next().sequence()).bind(&ids).fetch_all(&mut *tx).await.map_err(unavailable)?;
    let previous = if current == expected.sequence() {
        if !rows.is_empty() {
            return Err(ApplyError::Unbound);
        }
        None
    } else if current == page.next().sequence() && !signals.is_empty() {
        if rows.len() != signals.len() {
            return Err(ApplyError::Unbound);
        }
        Some(
            signals
                .iter()
                .map(|s| {
                    let r = rows
                        .iter()
                        .find(|r| r.try_get::<Uuid, _>("signal_id").ok() == Some(s.id))
                        .ok_or(ApplyError::Unbound)?;
                    exact(r, s)
                })
                .collect::<Result<Vec<_>, _>>()?,
        )
    } else {
        return Err(ApplyError::Unbound);
    };
    Ok(Prepared {
        tx,
        consumer,
        after: expected,
        next: page.next(),
        signals,
        previous,
    })
}
impl Prepared {
    /// Called only after all native removals and production permit drains.
    /// Receipt rows and checkpoint advance are committed in this same transaction.
    pub async fn commit(mut self, bindings: serde_json::Value) -> Result<OwnerCursor, ApplyError> {
        if self.previous.is_none() {
            for s in &self.signals {
                let rows=sqlx::query("update concert_world_memberships m set disconnected=true where m.owner_process_id=$1 and not m.disconnected and exists(select 1 from jsonb_to_recordset($2) as b(event uuid,epoch uuid) where b.event=m.event_id and b.epoch=m.owner_epoch) and (($3='USER' and m.user_id=$4) or ($3='FAMILY' and m.user_id=$4 and m.session_id=$5) or ($3='PAIR' and m.user_id in($4,$5))) returning event_id,user_id,room_id,session_id,connection_id,owner_process_id,owner_epoch")
                    .bind(self.consumer.to_string()).bind(&bindings).bind(s.kind).bind(s.a).bind(s.b).fetch_all(&mut *self.tx).await.map_err(unavailable)?;
                if rows.len() > MAX_VICTIMS {
                    return Err(ApplyError::Unavailable);
                }
                let victims = rows
                    .into_iter()
                    .map(|r| {
                        Ok(RemoteVictim {
                            event: r.try_get("event_id").map_err(unavailable)?,
                            victim: Victim {
                                user: r.try_get("user_id").map_err(unavailable)?,
                                room: r.try_get("room_id").map_err(unavailable)?,
                                family: r.try_get("session_id").map_err(unavailable)?,
                                connection: r.try_get("connection_id").map_err(unavailable)?,
                                owner: r.try_get("owner_process_id").map_err(unavailable)?,
                                epoch: r.try_get("owner_epoch").map_err(unavailable)?,
                            },
                        })
                    })
                    .collect::<Result<Vec<_>, ApplyError>>()?;
                sqlx::query("insert into concert_world_remote_effects(consumer_id,source_owner,source_epoch,sequence,signal_id,reason,scope_kind,scope_a,scope_b,victims) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
                    .bind(self.consumer).bind(owner_name(s.cursor.owner())).bind(s.cursor.epoch()).bind(s.cursor.sequence()).bind(s.id).bind(s.reason).bind(s.kind).bind(s.a).bind(s.b).bind(serde_json::to_value(victims).map_err(unavailable)?).execute(&mut *self.tx).await.map_err(unavailable)?;
            }
            let n=sqlx::query("update concert_world_remote_checkpoints set sequence=$5,updated_at=clock_timestamp() where consumer_id=$1 and source_owner=$2 and source_epoch=$3 and sequence=$4")
                .bind(self.consumer).bind(owner_name(self.after.owner())).bind(self.after.epoch()).bind(self.after.sequence()).bind(self.next.sequence()).execute(&mut *self.tx).await.map_err(unavailable)?.rows_affected();
            if n != 1 {
                return Err(ApplyError::Unbound);
            }
        }
        self.tx.commit().await.map_err(unavailable)?;
        Ok(self.next)
    }
}
