//! Concert-owned half of Presence reads, reusing PgRelationshipPolicy's local lock protocol.
//! No Auth/Friend tables, synthetic FreshAuthority, or packed owner revision is introduced.
use super::super::CurrentOwnedMembership;
use chrono::{DateTime, Utc};
use concert_domain::{
    relationship::{Field, PolicyQuery, RelationshipError},
    EventPhase, Revision,
};
use sqlx::{Connection, PgConnection, Row};
use std::str::FromStr;

/// Local revisions remain independent of the Auth/Friend observation vector.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct LocalRevision {
    pub event: Revision,
    pub relationship: Revision,
    pub actor_participant: Revision,
    pub subject_participant: Revision,
    pub actor_restriction: Revision,
    pub subject_restriction: Revision,
}
impl std::fmt::Debug for LocalRevision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LocalRevision([REDACTED])")
    }
}
pub(super) struct LocalRead {
    pub revision: LocalRevision,
    pub checked_at: DateTime<Utc>,
    pub valid_until: DateTime<Utc>,
    pub visible: bool,
}
fn unavailable(_: sqlx::Error) -> RelationshipError {
    RelationshipError::Unavailable
}
fn revision(v: i64) -> Result<Revision, RelationshipError> {
    Revision::new(v).map_err(|_| RelationshipError::Unavailable)
}
/// Must be called in a transaction with the original request deadline. It holds only
/// Concert locks; a remote observation cannot make those locks fence remote revocation.
pub(super) async fn read_in(
    c: &mut PgConnection,
    q: PolicyQuery,
    actor: &CurrentOwnedMembership,
    subject: &CurrentOwnedMembership,
) -> Result<LocalRead, RelationshipError> {
    if q.field != Field::Presence
        || actor.membership.key.event_id != q.event
        || actor.membership.key.user_id != q.actor
        || subject.membership.key.event_id != q.event
        || subject.membership.key.user_id != q.subject
        || actor.membership.room_id != subject.membership.room_id
        || actor.owner != subject.owner
        || actor.membership.pin != subject.membership.pin
    {
        return Err(RelationshipError::Denied);
    }
    let state = crate::relationship::lock_state(c, q).await?;
    if matches!(state.phase, EventPhase::Draft | EventPhase::Archived)
        || !state.actor_member
        || !state.subject_member
    {
        return Err(RelationshipError::Denied);
    }
    let rows=sqlx::query("select p.user_id,p.revision,a.state,a.restriction_revision from public.concert_participants p join public.concert_event_admissions a on a.event_id=p.event_id and a.user_id=p.user_id where p.event_id=$1 and p.user_id in($2,$3) order by p.user_id for share of p,a")
        .bind(q.event.uuid()).bind(q.actor.uuid()).bind(q.subject.uuid()).fetch_all(&mut *c).await.map_err(unavailable)?;
    let find = |user| {
        rows.iter()
            .find(|r| r.get::<uuid::Uuid, _>("user_id") == user)
            .ok_or(RelationshipError::Denied)
    };
    let a = find(q.actor.uuid())?;
    let s = find(q.subject.uuid())?;
    if a.try_get::<String, _>("state").map_err(unavailable)? != "ACTIVE" {
        return Err(RelationshipError::Denied);
    }
    let subject_active = s.try_get::<String, _>("state").map_err(unavailable)? == "ACTIVE";
    let stamp = LocalRevision {
        event: state.event_revision,
        relationship: state.relationship_revision,
        actor_participant: revision(a.try_get("revision").map_err(unavailable)?)?,
        subject_participant: revision(s.try_get("revision").map_err(unavailable)?)?,
        actor_restriction: revision(a.try_get("restriction_revision").map_err(unavailable)?)?,
        subject_restriction: revision(s.try_get("restriction_revision").map_err(unavailable)?)?,
    };
    let actor_until = current_membership(c, actor).await?;
    let subject_until = if actor.membership.key == subject.membership.key {
        actor_until
    } else {
        current_membership(c, subject).await?
    };
    let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&mut *c)
        .await
        .map_err(unavailable)?;
    let until = actor_until.min(subject_until);
    if until <= now {
        return Err(RelationshipError::Stale);
    }
    Ok(LocalRead {
        revision: stamp,
        checked_at: now,
        valid_until: until,
        visible: subject_active && !state.blocked,
    })
}
/// Validate exact current owner, session, connection and resource identity. Unlike a
/// user-only lookup this cannot label another room/epoch's membership as the candidate.
async fn current_membership(
    c: &mut PgConnection,
    expected: &CurrentOwnedMembership,
) -> Result<DateTime<Utc>, RelationshipError> {
    let m = &expected.membership;
    let family =
        uuid::Uuid::from_str(m.session_id.as_str()).map_err(|_| RelationshipError::Denied)?;
    if family.is_nil()
        || family.to_string() != m.session_id.as_str()
        || m.pin.event_id != m.key.event_id
        || m.pin.room_id != m.room_id
    {
        return Err(RelationshipError::Denied);
    }
    let row=sqlx::query("select m.lease_until,m.access_expires_at,l.lease_until as owner_until from public.concert_world_memberships m join public.concert_world_owner_leases l on l.event_id=m.event_id and l.room_id=m.room_id join public.concert_world_rooms r on r.event_id=m.event_id and r.room_id=m.room_id where m.event_id=$1 and m.user_id=$2 and m.room_id=$3 and m.session_id=$4 and m.connection_id=$5 and m.owner_process_id=$6 and m.owner_epoch=$7 and l.owner_process_id=m.owner_process_id and l.owner_epoch=m.owner_epoch and not m.disconnected and m.lease_until>clock_timestamp() and m.access_expires_at>clock_timestamp() and l.lease_until>clock_timestamp() and r.availability='AVAILABLE' and m.map_id=$8 and m.map_revision=$9 and m.avatar_manifest_id=$10 and m.avatar_manifest_version=$11 and r.map_id=m.map_id and r.map_revision=m.map_revision and r.avatar_manifest_id=m.avatar_manifest_id and r.avatar_manifest_version=m.avatar_manifest_version for share of m,l,r")
        .bind(m.key.event_id.uuid()).bind(m.key.user_id.uuid()).bind(m.room_id.uuid()).bind(family).bind(m.connection_id.uuid())
        .bind(&expected.owner.process_id).bind(expected.owner.epoch).bind(m.pin.map.id.as_str()).bind(m.pin.map.revision.get())
        .bind(m.pin.avatar.manifest_id.as_str()).bind(m.pin.avatar.manifest_version.get()).fetch_optional(&mut *c).await.map_err(unavailable)?
        .ok_or(RelationshipError::Stale)?;
    let lease: DateTime<Utc> = row.try_get("lease_until").map_err(unavailable)?;
    let access: DateTime<Utc> = row.try_get("access_expires_at").map_err(unavailable)?;
    let owner: DateTime<Utc> = row.try_get("owner_until").map_err(unavailable)?;
    if access != m.access_expires_at || lease > access {
        return Err(RelationshipError::Stale);
    }
    // Renewal does not extend a previously captured read's source authority.
    Ok(lease.min(m.lease_until).min(access).min(owner))
}

/// Keep an owning task alive until SQL cancellation/rollback drains. Dropping an
/// outer operation signals this task rather than abandoning SQLx's pending query.
struct CancelOnDrop(Option<tokio::sync::oneshot::Sender<()>>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if let Some(tx) = self.0.take() {
            let _ = tx.send(());
        }
    }
}
pub(super) async fn read(
    pool: &sqlx::PgPool,
    q: PolicyQuery,
    actor: &CurrentOwnedMembership,
    subject: &CurrentOwnedMembership,
    deadline: platform::actor_delegation::authority_deadline::AuthorityRpcDeadline,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> Result<LocalRead, RelationshipError> {
    // Reserve a control connection BEFORE taking any policy locks. Cancellation
    // must not wait for a pool exhausted by other blocked recipient reads.
    let mut control = pool.begin().await.map_err(unavailable)?;
    sqlx::query("select set_config('statement_timeout','250',true)")
        .execute(&mut *control)
        .await
        .map_err(unavailable)?;
    let mut tx = pool.begin().await.map_err(unavailable)?;
    let remaining = deadline
        .remaining(
            crate::adapters::out::relationship_authority::BUDGET,
            deadline.expires_micros(),
        )
        .map_err(|_| RelationshipError::Stale)?;
    let millis = remaining.as_millis().min(100);
    if millis == 0 {
        return Err(RelationshipError::Stale);
    }
    // Database-side safety net; the owning task also cancels exactly at the
    // original stop. A 100ms ceiling bounds draining even if the cancel port fails.
    let row=sqlx::query("select pg_backend_pid() as pid,set_config('statement_timeout',$1,true),set_config('lock_timeout',$1,true)")
        .bind(millis.to_string()).fetch_one(&mut *tx).await.map_err(unavailable)?;
    let pid: i32 = row.try_get("pid").map_err(unavailable)?;
    let actor = actor.clone();
    let subject = subject.clone();
    let stop = deadline.stop();
    let (cancel, mut cancelled) = tokio::sync::oneshot::channel();
    let guard = CancelOnDrop(Some(cancel));
    let task = tokio::spawn(async move {
        let _permit = permit;
        let (result, cancel_query) = tokio::select! {
            biased;
            _=&mut cancelled=>(Err(RelationshipError::Stale),true),
            _=tokio::time::sleep_until(stop)=>(Err(RelationshipError::Stale),true),
            result=read_in(&mut tx,q,&actor,&subject)=>(result,false),
        };
        let cancellation_ack = if cancel_query {
            // This exact backend remains exclusively held by tx until after the
            // cancel response and rollback. No recycled pool query can be hit.
            sqlx::query_scalar::<_, bool>("select pg_cancel_backend($1)")
                .bind(pid)
                .fetch_one(&mut *control)
                .await
                .unwrap_or(false)
        } else {
            false
        };
        // SQLx can surface the cancelled statement's ErrorResponse before its
        // ReadyForQuery. Drain that expected response before issuing ROLLBACK,
        // otherwise rollback itself can return the old cancellation error.
        let drain = match (&mut *tx).flush().await {
            Err(sqlx::Error::Database(ref error))
                if matches!(error.code().as_deref(), Some("57014" | "55P03")) =>
            {
                (&mut *tx).flush().await.map_err(unavailable)
            }
            result => result.map_err(unavailable),
        };
        // Never commit lock_state's provisional neutral-pair insertion.
        let rollback = tx.rollback().await.map_err(unavailable);
        // Transaction-local limits disappear without changing serving defaults.
        let control_rollback = control.rollback().await.map_err(unavailable);
        #[cfg(test)]
        if cancel_query {
            eprintln!("recipient SQL cleanup: cancel_ack={cancellation_ack} drain_ok={} data_rollback_ok={} control_rollback_ok={}",drain.is_ok(),rollback.is_ok(),control_rollback.is_ok());
        }
        drain?;
        rollback?;
        control_rollback?;
        #[cfg(test)]
        if cancellation_ack {
            CANCELLED_AND_DRAINED.lock().unwrap().insert(pid);
        }
        #[cfg(not(test))]
        let _ = cancellation_ack;
        result
    });
    let result = task.await.map_err(|_| RelationshipError::Unavailable)?;
    drop(guard);
    result
}

#[cfg(test)]
static CANCELLED_AND_DRAINED: std::sync::Mutex<std::collections::BTreeSet<i32>> =
    std::sync::Mutex::new(std::collections::BTreeSet::new());
#[cfg(test)]
pub(super) fn cancellation_drained(pid: i32) -> bool {
    CANCELLED_AND_DRAINED.lock().unwrap().contains(&pid)
}
