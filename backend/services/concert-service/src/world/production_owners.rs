//! Qualified owning reads and a separate Concert-local publication fence.
//! Remote observations are bounded reads, never a distributed revocation lease.
use super::{
    admission_bridge::LiveOwnerView,
    interest::{Facing, PublicIdentity},
    recipient_owners::{RecipientObservation, WorldRecipientReader},
    CurrentOwnedMembership,
};
use crate::adapters::out::{
    relationship_authority::world_projection::{WorldPresenceProjection, WorldProjectionParent},
    world_profile::GrpcWorldProfileReader,
};
use chrono::{DateTime, Utc};
use concert_application::world_ticket::WorldError;
use concert_domain::{avatar::AvatarCatalog, AvatarSelection, Revision};
use futures::{stream, FutureExt, StreamExt, TryStreamExt};
use sqlx::{postgres::PgRow, PgPool, Postgres, Row, Transaction};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::{oneshot, Notify};
use uuid::Uuid;

fn unavailable(_: impl std::fmt::Debug) -> WorldError {
    WorldError::Unavailable
}
fn avatar(row: &PgRow) -> Result<(Revision, AvatarSelection), WorldError> {
    Ok((
        Revision::new(row.try_get("revision").map_err(unavailable)?).map_err(unavailable)?,
        crate::adapters::out::repository::avatar::selection_from_row(row).map_err(unavailable)?,
    ))
}
// Bound concurrent remote candidates without changing their source order, owner
// attempt gates or original parent deadline. No detached tasks survive a failure.
const RECIPIENT_READ_CONCURRENCY: usize = 4;
async fn observe_in_order<T, E, F>(reads: impl IntoIterator<Item = F>) -> Result<Vec<T>, E>
where
    F: std::future::Future<Output = Result<T, E>>,
{
    stream::iter(reads)
        .buffered(RECIPIENT_READ_CONCURRENCY)
        .try_collect()
        .await
}

const AVATAR:&str="select revision,contract_version,representation,preset_id,manifest_id,manifest_version,body_id,eyes_id,hair_id,outfit_id,accessory_id,skin_tone_id from public.concert_avatar_settings where event_id=$1 and user_id=$2";
struct Candidate {
    context: WorldPresenceProjection,
    observation: RecipientObservation,
}
struct Plan {
    parent: WorldProjectionParent,
    identity: PublicIdentity,
    candidates: Vec<Candidate>,
    until: DateTime<Utc>,
}
struct Lease {
    enabled: AtomicBool,
    requests: BTreeSet<Uuid>,
    until: DateTime<Utc>,
    stop: Mutex<Option<oneshot::Sender<()>>>,
    finished: AtomicBool,
    rollback_ok: AtomicBool,
    done: Notify,
}
impl Lease {
    fn stop(&self) {
        self.enabled.store(false, Ordering::Release);
        if let Ok(mut stop) = self.stop.lock() {
            if let Some(stop) = stop.take() {
                let _ = stop.send(());
            }
        }
    }
    async fn drained(&self) -> Result<(), WorldError> {
        loop {
            let notified = self.done.notified();
            if self.finished.load(Ordering::Acquire) {
                return if self.rollback_ok.load(Ordering::Acquire) {
                    Ok(())
                } else {
                    Err(WorldError::Unavailable)
                };
            }
            notified.await;
        }
    }
}
/// One bridge owns this state. Async operation serialization lives in AdmissionBridge.
/// Every local publication permit owns a live SQL transaction, not just revisions.
pub(crate) struct ProductionOwners {
    pool: PgPool,
    profile: GrpcWorldProfileReader,
    recipient: WorldRecipientReader,
    catalog: AvatarCatalog,
    fence: Arc<Mutex<()>>,
    generation: Uuid,
    plans: Mutex<BTreeMap<(Uuid, Uuid), Arc<Plan>>>,
    lease: Mutex<Option<Arc<Lease>>>,
}
impl ProductionOwners {
    pub(crate) fn new(
        pool: PgPool,
        profile: GrpcWorldProfileReader,
        recipient: WorldRecipientReader,
        catalog: AvatarCatalog,
    ) -> Arc<Self> {
        Arc::new(Self {
            pool,
            profile,
            recipient,
            catalog,
            fence: Arc::new(Mutex::new(())),
            generation: Uuid::new_v4(),
            plans: Mutex::new(BTreeMap::new()),
            lease: Mutex::new(None),
        })
    }
    // Caller holds the bridge native-handoff fence. Unrelated parent authority
    // is retained; its candidate cache is refreshed without revoking its session.
    pub(crate) fn forget_invalidated(&self, users: &BTreeSet<Uuid>) -> Result<(), WorldError> {
        self.plans.lock().map_err(unavailable)?.retain(|_, p| {
            if users.contains(&p.parent.actor()) {
                p.parent.cancel();
                return false;
            }
            !p.candidates
                .iter()
                .any(|c| users.contains(&c.context.query().subject.uuid()))
        });
        Ok(())
    }
    pub(crate) fn fence(&self) -> Arc<Mutex<()>> {
        self.fence.clone()
    }
    pub(crate) fn generation(&self) -> Uuid {
        self.generation
    }
    pub(crate) fn ready(&self, parent: &WorldProjectionParent) -> bool {
        parent.check().is_ok()
            && self.lease.lock().is_ok_and(|lease| {
                lease.as_ref().is_some_and(|lease| {
                    lease.enabled.load(Ordering::Acquire)
                        && Utc::now() < lease.until
                        && lease.requests.contains(&parent.request_id())
                })
            })
    }
    /// Disable under the exact native-send fence BEFORE releasing SQL locks.
    /// Existing bounded membership is not fabricated/extended during refresh.
    pub(crate) async fn pause(&self) -> Result<(), WorldError> {
        let lease = {
            let _f = self.fence.lock().map_err(unavailable)?;
            let lease = self.lease.lock().map_err(unavailable)?.take();
            if let Some(lease) = &lease {
                lease.stop();
            }
            lease
        };
        if let Some(lease) = lease {
            lease.drained().await?;
        }
        Ok(())
    }
    pub(crate) async fn current(
        &self,
        parent: &WorldProjectionParent,
    ) -> Result<LiveOwnerView, WorldError> {
        parent.check()?;
        let context = parent.profile_projection()?;
        let profile = Box::pin(self.profile.observe(&context)).await?;
        profile.check(&context)?;
        let row = tokio::time::timeout_at(
            parent.deadline().stop(),
            sqlx::query(AVATAR)
                .bind(parent.event_id())
                .bind(parent.actor())
                .fetch_optional(&self.pool),
        )
        .await
        .map_err(unavailable)?
        .map_err(unavailable)?
        .ok_or(WorldError::Denied)?;
        parent.check()?;
        let (revision, selection) = avatar(&row)?;
        self.catalog.validate(&selection).map_err(unavailable)?;
        let now = Utc::now();
        Ok(LiveOwnerView {
            subject: parent.subject().clone(),
            checked_at: now,
            expires_at: DateTime::from_timestamp_micros(parent.deadline().expires_micros())
                .ok_or(WorldError::Unavailable)?,
            identity: PublicIdentity {
                display_name: profile.display_name().to_owned(),
                avatar_revision: revision,
                avatar: selection,
                facing: Facing::Down,
                moving: false,
            },
        })
    }
    pub(crate) async fn visible(
        &self,
        parent: &WorldProjectionParent,
        subjects: &[CurrentOwnedMembership],
        identity: &PublicIdentity,
    ) -> Result<(BTreeSet<Uuid>, DateTime<Utc>), WorldError> {
        parent.check()?;
        let actor = parent
            .current_membership()
            .ok_or(WorldError::MembershipMissing)?;
        // Each candidate still calls the qualified reader, which reserves and
        // runs both real owner attempts and enforces the clipped original stop.
        // Ordered buffering preserves the later SQL plan/lock order while a
        // slow first candidate no longer serializes all following owner reads.
        let mut reads = Vec::with_capacity(subjects.len());
        for subject in subjects {
            let expected = concert_application::world_ticket::WorldSubject {
                key: subject.membership.key,
                event_id: subject.membership.key.event_id,
                session_id: subject.membership.session_id.clone(),
            };
            let context = parent.presence_projection(
                subject,
                &actor.owner,
                &expected,
                subject.membership.connection_id,
            )?;
            let recipient = &self.recipient;
            reads.push(
                async move {
                    let observation = Box::pin(recipient.read(&context)).await?;
                    observation.check_scope(&context)?;
                    Ok::<_, WorldError>(Candidate {
                        context,
                        observation,
                    })
                }
                .boxed(),
            );
        }
        let candidates = observe_in_order(reads).await?;
        let mut visible = BTreeSet::new();
        let mut until = DateTime::from_timestamp_micros(parent.deadline().expires_micros())
            .ok_or(WorldError::Unavailable)?;
        for candidate in &candidates {
            // A completed earlier observation may have expired while another
            // candidate was pending. Never turn concurrency into a fresh lease.
            candidate.observation.check_scope(&candidate.context)?;
            until = until.min(candidate.observation.valid_until());
            if candidate.observation.visible() {
                visible.insert(candidate.context.candidate().membership.key.user_id.uuid());
            }
        }
        if !visible.contains(&parent.actor()) {
            return Err(WorldError::Denied);
        }
        parent.check()?;
        let key = (parent.event_id(), parent.actor());
        let mut plans = self.plans.lock().map_err(unavailable)?;
        if plans.len() >= 800 && !plans.contains_key(&key) {
            return Err(WorldError::Unavailable);
        }
        plans.insert(
            key,
            Arc::new(Plan {
                parent: parent.clone(),
                identity: identity.clone(),
                candidates,
                until,
            }),
        );
        Ok((visible, until))
    }
    /// Re-observe local scope/revisions under locks and retain the locks while
    /// the room owner may send. No remote I/O occurs in this transaction.
    pub(crate) async fn activate(&self) -> Result<(), WorldError> {
        self.pause().await?;
        let plans: Vec<_> = {
            let mut plans = self.plans.lock().map_err(unavailable)?;
            plans.retain(|_, p| p.parent.check().is_ok() && p.until > Utc::now());
            plans.values().cloned().collect()
        };
        if plans.is_empty() {
            return Ok(());
        }
        let stop = plans
            .iter()
            .map(|p| p.parent.deadline().stop())
            .min()
            .ok_or(WorldError::Unavailable)?;
        let mut tx = tokio::time::timeout_at(stop, self.pool.begin())
            .await
            .map_err(unavailable)?
            .map_err(unavailable)?;
        sqlx::query("set local lock_timeout='200ms'")
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        sqlx::query("set local statement_timeout='250ms'")
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        let mut requests = BTreeSet::new();
        let mut until: Option<DateTime<Utc>> = None;
        for plan in &plans {
            let valid = tokio::time::timeout_at(stop, Box::pin(self.hold_plan(&mut tx, plan)))
                .await
                .map_err(|_| WorldError::Unavailable)??;
            if valid {
                requests.insert(plan.parent.request_id());
                until = Some(until.map_or(plan.until, |u| u.min(plan.until)));
            }
        }
        let Some(until) = until else {
            tx.rollback().await.map_err(unavailable)?;
            return Ok(());
        };
        let remaining = (until - Utc::now()).to_std().map_err(unavailable)?;
        let stop = stop.min(tokio::time::Instant::now() + remaining);
        let (sender, receiver) = oneshot::channel();
        let lease = Arc::new(Lease {
            enabled: AtomicBool::new(true),
            requests,
            until,
            stop: Mutex::new(Some(sender)),
            finished: AtomicBool::new(false),
            rollback_ok: AtomicBool::new(false),
            done: Notify::new(),
        });
        {
            let _f = self.fence.lock().map_err(unavailable)?;
            if until <= Utc::now() {
                return Err(WorldError::Unavailable);
            }
            *self.lease.lock().map_err(unavailable)? = Some(lease.clone());
        }
        let fence = self.fence.clone();
        tokio::spawn(async move {
            // The original monotonic stop also applies if the wall clock moves.
            let cancelled = async {
                loop {
                    if plans.iter().any(|p| p.parent.check().is_err()) {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            };
            tokio::select! {_ = receiver=>{},_ = tokio::time::sleep_until(stop)=>{}, _ = cancelled=>{}}
            if let Ok(_f) = fence.lock() {
                lease.enabled.store(false, Ordering::Release);
            } else {
                lease.enabled.store(false, Ordering::Release);
            }
            let rollback = tx.rollback().await;
            lease.rollback_ok.store(rollback.is_ok(), Ordering::Release);
            lease.finished.store(true, Ordering::Release);
            lease.done.notify_waiters();
        });
        Ok(())
    }
    async fn hold_plan(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        plan: &Plan,
    ) -> Result<bool, WorldError> {
        if plan.parent.check().is_err() {
            return Ok(false);
        }
        for candidate in &plan.candidates {
            if candidate
                .observation
                .check_scope(&candidate.context)
                .is_err()
            {
                return Ok(false);
            }
            let q = candidate.context.query();
            let stamp = candidate.observation.local_revisions();
            let event: Option<(String, i64)> = sqlx::query_as(
                "select phase,revision from concert_events where event_id=$1 for share",
            )
            .bind(q.event.uuid())
            .fetch_optional(&mut **tx)
            .await
            .map_err(unavailable)?;
            if !event.is_some_and(|(phase, rev)| {
                matches!(phase.as_str(), "OPEN" | "LIVE") && rev == stamp.event.get()
            }) {
                return Ok(false);
            }
            let (low, high) = if q.actor.uuid() < q.subject.uuid() {
                (q.actor.uuid(), q.subject.uuid())
            } else {
                (q.subject.uuid(), q.actor.uuid())
            };
            // The reader rolls back neutral-pair creation. Retain a provisional
            // row here so the absence-to-insert race is fenced too; always rollback.
            sqlx::query("insert into concert_relationship_pairs(event_id,low_user,high_user) values($1,$2,$3) on conflict do nothing")
                .bind(q.event.uuid()).bind(low).bind(high).execute(&mut **tx).await.map_err(unavailable)?;
            let pair:Option<(i64,bool,bool)>=sqlx::query_as("select revision,low_blocks_high,high_blocks_low from concert_relationship_pairs where event_id=$1 and low_user=$2 and high_user=$3 for share").bind(q.event.uuid()).bind(low).bind(high).fetch_optional(&mut **tx).await.map_err(unavailable)?;
            if !pair.is_some_and(|(rev, _, _)| rev == stamp.relationship.get()) {
                return Ok(false);
            }
            let rows=sqlx::query("select p.user_id,p.revision,a.state,a.restriction_revision from concert_participants p join concert_event_admissions a using(event_id,user_id) where p.event_id=$1 and p.user_id in($2,$3) order by p.user_id for share of p,a").bind(q.event.uuid()).bind(low).bind(high).fetch_all(&mut **tx).await.map_err(unavailable)?;
            for (user, revision, restriction, actor) in [
                (
                    q.actor.uuid(),
                    stamp.actor_participant,
                    stamp.actor_restriction,
                    true,
                ),
                (
                    q.subject.uuid(),
                    stamp.subject_participant,
                    stamp.subject_restriction,
                    false,
                ),
            ] {
                let Some(row) = rows.iter().find(|r| r.get::<Uuid, _>("user_id") == user) else {
                    return Ok(false);
                };
                if row.get::<i64, _>("revision") != revision.get()
                    || row.get::<i64, _>("restriction_revision") != restriction.get()
                    || (actor && row.get::<String, _>("state") != "ACTIVE")
                {
                    return Ok(false);
                }
            }
            if !held_membership(
                tx,
                candidate
                    .context
                    .parent()
                    .current_membership()
                    .ok_or(WorldError::MembershipMissing)?,
            )
            .await?
                || !held_membership(tx, candidate.context.candidate()).await?
            {
                return Ok(false);
            }
        }
        let row = sqlx::query(&format!("{AVATAR} for share"))
            .bind(plan.parent.event_id())
            .bind(plan.parent.actor())
            .fetch_optional(&mut **tx)
            .await
            .map_err(unavailable)?;
        let Some(row) = row else {
            return Ok(false);
        };
        let (revision, selection) = avatar(&row)?;
        Ok(revision == plan.identity.avatar_revision
            && selection == plan.identity.avatar
            && plan.parent.check().is_ok()
            && plan.until > Utc::now())
    }
}
impl Drop for ProductionOwners {
    fn drop(&mut self) {
        if let Ok(lease) = self.lease.lock() {
            if let Some(lease) = lease.as_ref() {
                lease.stop();
            }
        }
    }
}
// Same exact canonical predicates as the qualified recipient local producer.
async fn held_membership(
    tx: &mut Transaction<'_, Postgres>,
    current: &CurrentOwnedMembership,
) -> Result<bool, WorldError> {
    let m = &current.membership;
    let access:Option<DateTime<Utc>>=sqlx::query_scalar("select m.access_expires_at from concert_world_memberships m join concert_world_owner_leases l using(event_id,room_id) join concert_world_rooms r using(event_id,room_id) where m.event_id=$1 and m.user_id=$2 and m.room_id=$3 and m.session_id=$4 and m.connection_id=$5 and m.owner_process_id=$6 and m.owner_epoch=$7 and l.owner_process_id=m.owner_process_id and l.owner_epoch=m.owner_epoch and not m.disconnected and m.lease_until>clock_timestamp() and m.access_expires_at>clock_timestamp() and m.lease_until<=m.access_expires_at and l.lease_until>clock_timestamp() and r.availability='AVAILABLE' and m.map_id=$8 and m.map_revision=$9 and m.avatar_manifest_id=$10 and m.avatar_manifest_version=$11 and r.map_id=m.map_id and r.map_revision=m.map_revision and r.avatar_manifest_id=m.avatar_manifest_id and r.avatar_manifest_version=m.avatar_manifest_version for share of m,l,r")
        .bind(m.key.event_id.uuid()).bind(m.key.user_id.uuid()).bind(m.room_id.uuid()).bind(Uuid::parse_str(m.session_id.as_str()).map_err(unavailable)?).bind(m.connection_id.uuid()).bind(&current.owner.process_id).bind(current.owner.epoch).bind(m.pin.map.id.as_str()).bind(m.pin.map.revision.get()).bind(m.pin.avatar.manifest_id.as_str()).bind(m.pin.avatar.manifest_version.get()).fetch_optional(&mut **tx).await.map_err(unavailable)?;
    Ok(access == Some(m.access_expires_at))
}

#[cfg(test)]
mod visibility_pipeline_tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    struct Active(Arc<AtomicUsize>);
    impl Drop for Active {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn overlaps_at_most_four_and_retains_input_lock_order() {
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let values = observe_in_order((0..12).map(|n| {
            let active = active.clone();
            let peak = peak.clone();
            async move {
                let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                let _guard = Active(active);
                peak.fetch_max(count, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis((4 - n % 4) * 2)).await;
                Ok::<_, WorldError>(n)
            }
        }))
        .await
        .unwrap();
        assert_eq!(values, (0..12).collect::<Vec<_>>());
        assert_eq!(peak.load(Ordering::SeqCst), RECIPIENT_READ_CONCURRENCY);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn failure_drops_all_inflight_reads_without_a_partial_plan() {
        let active = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(AtomicUsize::new(0));
        let result = observe_in_order((0..20).map(|n| {
            let active = active.clone();
            let started = started.clone();
            async move {
                active.fetch_add(1, Ordering::SeqCst);
                started.fetch_add(1, Ordering::SeqCst);
                let _guard = Active(active);
                if n == 0 {
                    tokio::task::yield_now().await;
                    Err::<usize, _>(WorldError::Unavailable)
                } else {
                    std::future::pending().await
                }
            }
        }))
        .await;
        assert!(matches!(result, Err(WorldError::Unavailable)));
        assert!(started.load(Ordering::SeqCst) <= RECIPIENT_READ_CONCURRENCY);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn original_deadline_cancels_the_whole_batch_and_releases_reads() {
        let active = Arc::new(AtomicUsize::new(0));
        let original = tokio::time::Instant::now() + Duration::from_millis(10);
        let reads = observe_in_order((0..20).map(|_| {
            let active = active.clone();
            async move {
                active.fetch_add(1, Ordering::SeqCst);
                let _guard = Active(active);
                std::future::pending::<Result<(), WorldError>>().await
            }
        }));
        assert!(tokio::time::timeout_at(original, reads).await.is_err());
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }
}
