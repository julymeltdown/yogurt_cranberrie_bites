use super::*;
use concert_application::commands::maintenance::InvalidationKind;
use sqlx::{postgres::PgRow, PgConnection, PgPool, Row};
fn unavailable(_: impl std::fmt::Debug) -> ApplyError {
    ApplyError::Unavailable
}
pub(super) struct Signal {
    id: Uuid,
    pub scope: Scope,
    owner_epoch: Uuid,
    receipt: Uuid,
    operation: String,
    resource: Uuid,
    revision: i64,
    pub superseded: bool,
}
pub(super) struct Applied {
    pub victims: Vec<Victim>,
    pub duplicate: bool,
    pub superseded: bool,
}
fn decode(r: &PgRow, s: &Signal) -> Result<Applied, ApplyError> {
    let (kind, a, b) = s.scope.parts();
    if r.get::<Uuid, _>("owner_epoch") != s.owner_epoch
        || r.get::<Uuid, _>("receipt_id") != s.receipt
        || r.get::<String, _>("operation_id") != s.operation
        || r.get::<Uuid, _>("resource_id") != s.resource
        || r.get::<i64, _>("revision") != s.revision
        || r.get::<String, _>("scope_kind") != kind
        || r.get::<Uuid, _>("scope_a") != a
        || r.get::<Uuid, _>("scope_b") != b
    {
        return Err(ApplyError::Unbound);
    }
    let victims: Vec<Victim> = serde_json::from_value(r.get("victims")).map_err(unavailable)?;
    if victims.len() > 800
        || victims
            .iter()
            .any(|v| v.user.is_nil() || v.connection.is_nil() || v.epoch.is_nil())
    {
        return Err(ApplyError::Unavailable);
    }
    Ok(Applied {
        victims,
        duplicate: true,
        superseded: r.get("superseded"),
    })
}
pub(super) async fn previous(
    pool: &PgPool,
    consumer: Uuid,
    s: &Signal,
) -> Result<Option<Applied>, ApplyError> {
    let row=sqlx::query("select * from concert_world_invalidation_receipts where consumer_id=$1 and event_id=$2 and signal_id=$3")
        .bind(consumer).bind(s.scope.event()).bind(s.id).fetch_optional(pool).await.map_err(unavailable)?;
    row.map(|r| decode(&r, s)).transpose()
}
pub(super) async fn resolve(
    pool: &PgPool,
    d: &Delivery,
    consumer: Uuid,
    epoch: Uuid,
    event: Uuid,
) -> Result<Signal, ApplyError> {
    let mut c = pool.acquire().await.map_err(unavailable)?;
    resolve_in(&mut c, d, consumer, epoch, event).await
}
async fn resolve_in(
    c: &mut PgConnection,
    d: &Delivery,
    consumer: Uuid,
    epoch: Uuid,
    event: Uuid,
) -> Result<Signal, ApplyError> {
    if d.event != event || d.kind != InvalidationKind::ResourceInvalidated || d.receipt.is_none() {
        return Err(ApplyError::Unbound);
    }
    let row=sqlx::query("select o.*,r.actor_id,r.target_id,r.operation_id,r.state as receipt_state,r.resource_id as receipt_resource,r.resource_revision as receipt_revision,r.result_status,(o.lease_until>clock_timestamp()) as live,s.owner_epoch from concert_outbox o join concert_command_receipts r on r.receipt_id=o.receipt_id cross join concert_world_invalidation_source s where o.outbox_id=$1 and s.singleton")
        .bind(d.id).fetch_optional(&mut *c).await.map_err(unavailable)?.ok_or(ApplyError::Unbound)?;
    if row.get::<String, _>("state") != "LEASED"
        || !row.get::<bool, _>("live")
        || row.get::<Option<Uuid>, _>("lease_token") != Some(d.lease)
        || row.get::<i32, _>("attempts") != d.attempt
    {
        return Err(ApplyError::StaleLease);
    }
    if row.get::<Option<Uuid>, _>("receipt_id") != d.receipt
        || row.get::<Uuid, _>("event_id") != d.event
        || row.get::<Uuid, _>("resource_id") != d.reference.resource
        || row.get::<i64, _>("resource_revision") != d.reference.revision
        || row.get::<String, _>("kind") != "RESOURCE_INVALIDATED"
        || row.get::<Uuid, _>("target_id") != d.reference.resource
    {
        return Err(ApplyError::Unbound);
    }
    let already:bool=sqlx::query_scalar("select exists(select 1 from concert_world_invalidation_receipts where consumer_id=$1 and event_id=$2 and signal_id=$3)")
        .bind(consumer).bind(event).bind(d.id).fetch_one(&mut *c).await.map_err(unavailable)?;
    if !(row.get::<String, _>("receipt_state") == "SUCCEEDED"
        && row.get::<Option<Uuid>, _>("receipt_resource") == Some(d.reference.resource)
        && row.get::<Option<i64>, _>("receipt_revision") == Some(d.reference.revision)
        && row.get::<Option<i16>, _>("result_status") == Some(200))
        && !(already && row.get::<String, _>("receipt_state") == "RETIRED")
    {
        return Err(ApplyError::Unbound);
    }
    let active:bool=sqlx::query_scalar("select exists(select 1 from concert_world_owner_leases where event_id=$1 and owner_process_id=$2 and owner_epoch=$3 and lease_until>clock_timestamp())")
        .bind(event).bind(consumer.to_string()).bind(epoch).fetch_one(&mut *c).await.map_err(unavailable)?;
    if !active {
        return Err(ApplyError::Unbound);
    }
    let actor: Uuid = row.get("actor_id");
    let operation: String = row.get("operation_id");
    let scope = match operation.as_str() {
        "blockConcertUser" | "unblockConcertUser"
            if actor != d.reference.resource && !actor.is_nil() =>
        {
            let revision:Option<i64>=sqlx::query_scalar("select revision from concert_relationship_pairs where event_id=$1 and low_user=$2 and high_user=$3")
                .bind(event).bind(actor.min(d.reference.resource)).bind(actor.max(d.reference.resource)).fetch_optional(&mut *c).await.map_err(unavailable)?;
            if !revision.is_some_and(|r| r >= d.reference.revision) {
                return Err(ApplyError::Unbound);
            }
            Scope::Pair {
                event,
                a: actor,
                b: d.reference.resource,
            }
        }
        "closeConcertEvent" => {
            let valid:bool=sqlx::query_scalar("select exists(select 1 from concert_closure_jobs j join concert_events e using(event_id) where j.event_id=$1 and j.job_id=$2 and j.revision>=$3 and e.phase in ('READ_ONLY','ARCHIVED') and exists(select 1 from concert_closure_effects f where f.event_id=j.event_id and f.job_id=j.job_id and f.step='READ_ONLY'))")
                .bind(event).bind(d.reference.resource).bind(d.reference.revision).fetch_one(&mut *c).await.map_err(unavailable)?;
            if !valid {
                return Err(ApplyError::Unbound);
            }
            Scope::Event(event)
        }
        _ => return Err(ApplyError::Unbound),
    };
    let (kind, a, b) = scope.parts();
    let owner_epoch: Uuid = row.get("owner_epoch");
    let cursor:Option<i64>=sqlx::query_scalar("select revision from concert_world_invalidation_cursors where consumer_id=$1 and event_id=$2 and owner_epoch=$3 and scope_kind=$4 and scope_a=$5 and scope_b=$6")
        .bind(consumer).bind(event).bind(owner_epoch).bind(kind).bind(a).bind(b).fetch_optional(&mut *c).await.map_err(unavailable)?;
    Ok(Signal {
        id: d.id,
        scope,
        owner_epoch,
        receipt: d.receipt.unwrap(),
        operation,
        resource: d.reference.resource,
        revision: d.reference.revision,
        superseded: cursor.is_some_and(|r| r >= d.reference.revision),
    })
}
pub(super) async fn apply(
    pool: &PgPool,
    d: &Delivery,
    consumer: Uuid,
    epoch: Uuid,
    event: Uuid,
    expected: &Signal,
) -> Result<Applied, ApplyError> {
    let mut tx = pool.begin().await.map_err(unavailable)?;
    sqlx::query("set local statement_timeout='2000ms'")
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
    sqlx::query("set local lock_timeout='500ms'")
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
    sqlx::query("select outbox_id from concert_outbox where outbox_id=$1 for update")
        .bind(d.id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?;
    let s = resolve_in(&mut tx, d, consumer, epoch, event).await?;
    if s.owner_epoch != expected.owner_epoch || s.scope != expected.scope {
        return Err(ApplyError::Unbound);
    }
    let prior=sqlx::query("select * from concert_world_invalidation_receipts where consumer_id=$1 and event_id=$2 and signal_id=$3")
        .bind(consumer).bind(event).bind(d.id).fetch_optional(&mut *tx).await.map_err(unavailable)?;
    if let Some(prior) = prior {
        let result = decode(&prior, &s)?;
        tx.commit().await.map_err(unavailable)?;
        return Ok(result);
    }
    let (kind, a, b) = s.scope.parts();
    let mut victims = vec![];
    if !s.superseded {
        // SQL effects are scoped to this bound World owner. Foreign active owners
        // require their actual consumer, not an assumed fanout acknowledgment.
        let foreign:bool=sqlx::query_scalar("select exists(select 1 from concert_world_memberships where event_id=$1 and ($2='EVENT' or user_id in($3,$4)) and not disconnected and lease_until>clock_timestamp() and (owner_process_id<>$5 or owner_epoch<>$6))")
            .bind(event).bind(kind).bind(a).bind(b).bind(consumer.to_string()).bind(epoch).fetch_one(&mut *tx).await.map_err(unavailable)?;
        if foreign {
            return Err(ApplyError::Unbound);
        }
        let rows=sqlx::query("update concert_world_memberships set disconnected=true where event_id=$1 and ($2='EVENT' or user_id in($3,$4)) and owner_process_id=$5 and owner_epoch=$6 and not disconnected returning user_id,room_id,session_id,connection_id,owner_process_id,owner_epoch")
            .bind(event).bind(kind).bind(a).bind(b).bind(consumer.to_string()).bind(epoch).fetch_all(&mut *tx).await.map_err(unavailable)?;
        if rows.len() > 800 {
            return Err(ApplyError::Unavailable);
        }
        victims = rows
            .into_iter()
            .map(|r| Victim {
                user: r.get("user_id"),
                room: r.get("room_id"),
                family: r.get("session_id"),
                connection: r.get("connection_id"),
                owner: r.get("owner_process_id"),
                epoch: r.get("owner_epoch"),
            })
            .collect();
        sqlx::query("insert into concert_world_invalidation_cursors(consumer_id,event_id,owner_epoch,scope_kind,scope_a,scope_b,revision) values($1,$2,$3,$4,$5,$6,$7) on conflict(consumer_id,event_id,owner_epoch,scope_kind,scope_a,scope_b) do update set revision=greatest(concert_world_invalidation_cursors.revision,excluded.revision)")
            .bind(consumer).bind(event).bind(s.owner_epoch).bind(kind).bind(a).bind(b).bind(s.revision).execute(&mut *tx).await.map_err(unavailable)?;
    }
    sqlx::query("insert into concert_world_invalidation_receipts(consumer_id,event_id,signal_id,owner_epoch,receipt_id,operation_id,resource_id,revision,scope_kind,scope_a,scope_b,victims,superseded) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)")
        .bind(consumer).bind(event).bind(d.id).bind(s.owner_epoch).bind(s.receipt).bind(&s.operation).bind(s.resource).bind(s.revision).bind(kind).bind(a).bind(b).bind(serde_json::to_value(&victims).map_err(unavailable)?).bind(s.superseded).execute(&mut *tx).await.map_err(unavailable)?;
    // Delayed effects must still be bound to the same live delivery/owner at
    // the commit boundary; a timed-out lease never becomes an application ack.
    resolve_in(&mut tx, d, consumer, epoch, event).await?;
    tx.commit().await.map_err(unavailable)?;
    Ok(Applied {
        victims,
        duplicate: false,
        superseded: s.superseded,
    })
}
