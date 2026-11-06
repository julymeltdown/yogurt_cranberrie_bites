//! Real READ_ONLY composition and pretransaction native-publication barrier.
//! World owns its SQL implementation/schema; no healthy or boolean owner fallback.
use super::*;
use crate::{
    community::closure,
    world::{
        admission_bridge::AdmissionBridge,
        invalidation::{Removal, Scope as RemovalScope},
    },
};
use concert_application::commands::FingerprintKeys;
use sqlx::{postgres::PgTransactionManager, TransactionManager};
use std::sync::{Arc, Mutex};

/// Actual owner fences must already be retained by this connection's caller.
/// Lock event BEFORE job, never an async native/admission gate after either SQL lock.
pub(crate) async fn require_scope_in(
    c: &mut PgConnection,
    key: EffectKey,
) -> Result<(), CommandError> {
    if key.step != Step::ReadOnly
        || key.event.is_nil()
        || key.job.is_nil()
        || PgTransactionManager::get_transaction_depth(c) == 0
    {
        return Err(CommandError::InvalidInput);
    }
    let phase: Option<String> =
        sqlx::query_scalar("select phase from concert_events where event_id=$1 for update")
            .bind(key.event)
            .fetch_optional(&mut *c)
            .await
            .map_err(unavailable)?;
    if !matches!(phase.as_deref(), Some("READ_ONLY" | "ARCHIVED")) {
        return Err(CommandError::InvalidState);
    }
    let job: Option<Uuid> = sqlx::query_scalar(
        "select job_id from concert_closure_jobs where event_id=$1 and job_id=$2 for update",
    )
    .bind(key.event)
    .bind(key.job)
    .fetch_optional(&mut *c)
    .await
    .map_err(unavailable)?;
    if job != Some(key.job) {
        return Err(CommandError::Denied);
    }
    finish(c).await
}
pub(crate) async fn require_new_barrier_in(
    c: &mut PgConnection,
    key: EffectKey,
) -> Result<(), CommandError> {
    let initial: bool = sqlx::query_scalar("select exists(select 1 from concert_closure_jobs j join concert_events e using(event_id) where j.event_id=$1 and j.job_id=$2 and e.phase='READ_ONLY' and j.revision=1 and j.progress->>'state'='PENDING')")
        .bind(key.event).bind(key.job).fetch_one(c).await.map_err(unavailable)?;
    if initial {
        Ok(())
    } else {
        Err(CommandError::InvalidState)
    }
}
fn validate_world(
    world: &crate::world::closure::WorldReadOnlyReceipt,
    key: EffectKey,
) -> Result<(), CommandError> {
    if world.event != key.event
        || world.job != key.job
        || world.operation != key.operation_id()
        || world.receipt_id.is_nil()
        || !(1..=concert_domain::MAX_REVISION).contains(&world.revision)
    {
        return Err(CommandError::InvalidState);
    }
    Ok(())
}
fn decode(row: &sqlx::postgres::PgRow, key: EffectKey) -> Result<Applied, CommandError> {
    if row.try_get::<Uuid, _>("event_id").map_err(unavailable)? != key.event
        || row.try_get::<Uuid, _>("job_id").map_err(unavailable)? != key.job
        || row
            .try_get::<Uuid, _>("operation_id")
            .map_err(unavailable)?
            != key.operation_id()
    {
        return Err(CommandError::InvalidState);
    }
    Applied {
        key,
        receipt: row.try_get("receipt_id").map_err(unavailable)?,
        revision: row.try_get("revision").map_err(unavailable)?,
    }
    .validate(key)
}
/// Actual World and community mutations use this exact connection. The caller
/// must roll back on any error; this function cannot independently commit.
/// This low-level SQL component does not claim local native-cache removal.
pub async fn apply_read_only_in(
    c: &mut PgConnection,
    key: EffectKey,
) -> Result<Applied, CommandError> {
    require_scope_in(c, key).await?;
    if let Some(row) =
        sqlx::query("select * from concert_closure_read_only_barriers where job_id=$1")
            .bind(key.job)
            .fetch_optional(&mut *c)
            .await
            .map_err(unavailable)?
    {
        return decode(&row, key);
    }
    require_new_barrier_in(c, key).await?;
    let world =
        crate::world::closure::fence_event_read_only_in(c, key.event, key.job, key.operation_id())
            .await?;
    validate_world(&world, key)?;
    let community = closure::fence_event_read_only_in(c, key).await?;
    if community.event != key.event
        || community.job != key.job
        || community.operation != key.operation_id()
        || community.receipt_id.is_nil()
        || community.revision != 1
    {
        return Err(CommandError::InvalidState);
    }
    finish(c).await?;
    let row = sqlx::query("insert into concert_closure_read_only_barriers(event_id,job_id,operation_id,receipt_id,revision,world_receipt_id,world_revision,community_receipt_id,community_revision) values($1,$2,$3,$4,1,$5,$6,$7,$8) returning *")
        .bind(key.event).bind(key.job).bind(key.operation_id()).bind(Uuid::new_v4())
        .bind(world.receipt_id).bind(world.revision).bind(community.receipt_id).bind(community.revision)
        .fetch_one(c).await.map_err(unavailable)?;
    decode(&row, key)
}

/// No public constructor: production READ_ONLY ownership is prepared only by the
/// native driver. All later steps still require an explicit actual owning adapter.
pub struct ReadOnlyOwners<N> {
    later: N,
    prepared: Arc<Mutex<Option<EffectKey>>>,
}
#[async_trait]
impl<N: LifecycleOwners> LifecycleOwners for ReadOnlyOwners<N> {
    async fn apply_in(
        &self,
        c: &mut PgConnection,
        key: EffectKey,
        retention: Retention,
    ) -> Result<Applied, CommandError> {
        if key.step != Step::ReadOnly {
            return self.later.apply_in(c, key, retention).await;
        }
        if *self
            .prepared
            .lock()
            .map_err(|_| CommandError::Unavailable)?
            != Some(key)
        {
            return Err(CommandError::Unavailable);
        }
        apply_read_only_in(c, key).await
    }
}
struct Prepared {
    state: Arc<Mutex<Option<EffectKey>>>,
    key: EffectKey,
}
impl Prepared {
    fn install(state: Arc<Mutex<Option<EffectKey>>>, key: EffectKey) -> Result<Self, CommandError> {
        let mut value = state.lock().map_err(|_| CommandError::Unavailable)?;
        if value.is_some() {
            return Err(CommandError::Unavailable);
        }
        *value = Some(key);
        drop(value);
        Ok(Self { state, key })
    }
}
impl Drop for Prepared {
    fn drop(&mut self) {
        // SQL preparation permission is ephemeral. Native removal is intentionally
        // NOT finished here: cancellation/failure must retain the pending fence.
        if let Ok(mut state) = self.state.lock() {
            if *state == Some(self.key) {
                *state = None;
            }
        }
    }
}
/// Construct from the actual bridge's pool/event, preventing a detached SQL pool
/// from certifying a different runtime. No HTTP/root or permissive authority setup.
pub struct ReadOnlyCommandDriver<A, N> {
    service: PgEventLifecycle<A, ReadOnlyOwners<N>>,
    bridge: Arc<AdmissionBridge>,
    event: Uuid,
    prepared: Arc<Mutex<Option<EffectKey>>>,
}
impl<A: FencedLifecycleAuthority, N: LifecycleOwners> ReadOnlyCommandDriver<A, N> {
    pub fn new(authority: A, later: N, bridge: Arc<AdmissionBridge>) -> Result<Self, CommandError> {
        let (pool, _, _, event) = bridge.invalidation_context();
        if event.is_nil() {
            return Err(CommandError::InvalidState);
        }
        let prepared = Arc::new(Mutex::new(None));
        Ok(Self {
            service: PgEventLifecycle::new(
                pool,
                authority,
                ReadOnlyOwners {
                    later,
                    prepared: prepared.clone(),
                },
            ),
            bridge,
            event,
            prepared,
        })
    }
    /// Called only AFTER the delivery gate. Retain actual event/authority locks
    /// until the guarded native fence mutates views; expiry is checked inside
    /// that same native critical section. The BE15 producer reauthorizes later.
    async fn preparation_authority(
        &self,
        actor: Actor,
        key: Uuid,
        command: &CloseCommand,
    ) -> Result<
        Option<(
            sqlx::Transaction<'static, sqlx::Postgres>,
            std::time::Instant,
        )>,
        CommandError,
    > {
        let CloseCommand::Start {
            expected_revision, ..
        } = command
        else {
            return Ok(None);
        };
        let target = start_job_id(actor, key);
        let mut tx = self.service.pool.begin().await.map_err(unavailable)?;
        let (phase, revision) = authorize(
            &mut tx,
            &self.service.authority,
            AuthorityQuery {
                actor,
                job: target,
                access: Access::Start,
            },
            false,
        )
        .await?;
        let exists: bool = sqlx::query_scalar("select exists(select 1 from concert_command_receipts where actor_id=$1 and event_id=$2 and operation_id='closeConcertEvent' and idempotency_key=$3)")
            .bind(actor.actor).bind(actor.event).bind(key).fetch_one(&mut *tx).await.map_err(unavailable)?;
        if exists || !matches!(phase.as_str(), "OPEN" | "LIVE") || revision != *expected_revision {
            tx.rollback().await.map_err(unavailable)?;
            return Ok(None);
        }
        // Anchor BEFORE the DB query: subtracting its server clock from the
        // server deadline then adding to this earlier monotonic instant cannot
        // extend authority by network/SQL scheduling latency or local UTC skew.
        let observed = std::time::Instant::now();
        let (at, until): (DateTime<Utc>, DateTime<Utc>) = sqlx::query_as(
            "select clock_timestamp(),nullif(current_setting('mcr.lifecycle_authority_until',true),'')::timestamptz"
        ).fetch_one(&mut *tx).await.map_err(unavailable)?;
        let remaining = (until - at)
            .to_std()
            .map_err(|_| CommandError::Unavailable)?;
        let deadline = observed
            .checked_add(remaining)
            .ok_or(CommandError::Unavailable)?;
        Ok(Some((tx, deadline)))
    }
    pub async fn command(
        &self,
        actor: Actor,
        key: Uuid,
        command: CloseCommand,
        keys: &FingerprintKeys,
    ) -> Result<SafeReceipt, CommandError> {
        command.request(actor, key)?;
        if actor.event != self.event {
            return Err(CommandError::Denied);
        }
        let _delivery = self.bridge.invalidation_delivery_gate().lock().await;
        let Some((tx, deadline)) = self.preparation_authority(actor, key, &command).await? else {
            // Exact replay/conflicts and RESUME retain canonical BE15 behavior.
            // A new START racing back to OPEN without preparation fails closed.
            return self.service.command(actor, key, command, keys).await;
        };
        let effect = EffectKey {
            event: actor.event,
            job: start_job_id(actor, key),
            step: Step::ReadOnly,
        };
        let operation = effect.operation_id();
        // Authority locks remain held during the synchronous native mutation.
        // No SQL transaction is held when entering either asynchronous gate.
        // World checks the monotonic deadline AFTER acquiring its native/views
        // locks, so expiration while waiting cannot evict any view or add a marker.
        let prepared = self.bridge.begin_invalidation_if(
            operation,
            Removal::Scope(RemovalScope::Event(actor.event)),
            || std::time::Instant::now() < deadline,
        );
        tx.rollback().await.map_err(unavailable)?;
        prepared.map_err(|_| CommandError::Unavailable)?;
        let _admission = self.bridge.invalidation_admission_gate().lock().await;
        self.bridge
            .pause_invalidation()
            .await
            .map_err(|_| CommandError::Unavailable)?;
        let _prepared = Prepared::install(self.prepared.clone(), effect)?;
        let result = self.service.command(actor, key, command, keys).await;
        if result
            .as_ref()
            .is_ok_and(|r| r.state == ReceiptState::Succeeded)
        {
            // Durable barrier committed. Removing the pending marker cannot
            // restore views. Failure here leaves native publication fail-closed;
            // it must never turn a committed admission into a claimed rollback.
            let _ = self.bridge.finish_invalidation(operation);
        }
        // Failure/cancellation retains the native marker. Fresh qualified World
        // recovery must re-read committed phase/authority before clearing it.
        result
    }
    pub async fn job(&self, actor: Actor, job: Uuid) -> Result<Job, CommandError> {
        if actor.event != self.event {
            return Err(CommandError::Denied);
        }
        self.service.job(actor, job).await
    }
    pub async fn close_response(
        &self,
        actor: Actor,
        key: Uuid,
        body: &[u8],
        keys: &FingerprintKeys,
    ) -> axum::response::Response {
        if actor.event != self.event {
            return crate::event_lifecycle::http::denied_response();
        }
        super::http::close_response(&self.service, actor, key, body, keys).await
    }
    pub async fn run_next(&self, actor: Actor, job: Uuid) -> Result<Job, CommandError> {
        if actor.event != self.event {
            return Err(CommandError::Denied);
        }
        self.service.run_next(actor, job).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn world_component_receipt_must_match_every_binding() {
        let key = EffectKey {
            event: Uuid::new_v4(),
            job: Uuid::new_v4(),
            step: Step::ReadOnly,
        };
        let make = || crate::world::closure::WorldReadOnlyReceipt {
            event: key.event,
            job: key.job,
            operation: key.operation_id(),
            receipt_id: Uuid::new_v4(),
            revision: 1,
        };
        assert!(validate_world(&make(), key).is_ok());
        let mut wrong = make();
        wrong.event = Uuid::new_v4();
        assert!(validate_world(&wrong, key).is_err());
        let mut wrong = make();
        wrong.job = Uuid::new_v4();
        assert!(validate_world(&wrong, key).is_err());
        let mut wrong = make();
        wrong.operation = Uuid::new_v4();
        assert!(validate_world(&wrong, key).is_err());
        let mut wrong = make();
        wrong.receipt_id = Uuid::nil();
        assert!(validate_world(&wrong, key).is_err());
        let mut wrong = make();
        wrong.revision = 0;
        assert!(validate_world(&wrong, key).is_err());
        let mut wrong = make();
        wrong.revision = concert_domain::MAX_REVISION + 1;
        assert!(validate_world(&wrong, key).is_err());
    }
    #[test]
    fn preparation_cancellation_removes_only_ephemeral_permission() {
        let state = Arc::new(Mutex::new(None));
        let key = EffectKey {
            event: Uuid::new_v4(),
            job: Uuid::new_v4(),
            step: Step::ReadOnly,
        };
        let prepared = Prepared::install(state.clone(), key).unwrap();
        assert_eq!(*state.lock().unwrap(), Some(key));
        assert!(Prepared::install(state.clone(), key).is_err());
        drop(prepared);
        assert_eq!(*state.lock().unwrap(), None);
    }
    // Reuses the World-owned four-service fixture at dd002448/1f6b0c7a.
    // Operator role and artwork metadata remain synthetic; production owners,
    // World runtime, native transport and composite effects are actual adapters.
    use crate::adapters::r#in::world_ws::AdmissionPort;
    use crate::world::closure::tests as qualified;
    use crate::world::membership::CanonicalAuthorityPort;
    struct LaterUnavailable;
    #[async_trait]
    impl LifecycleOwners for LaterUnavailable {
        async fn apply_in(
            &self,
            _: &mut PgConnection,
            _: EffectKey,
            _: Retention,
        ) -> Result<Applied, CommandError> {
            Err(CommandError::Unavailable)
        }
    }
    async fn seed_party(s: &qualified::Stack) -> anyhow::Result<Uuid> {
        use concert_domain::community::party::{Party, PartyFields};
        let party = Party::create(
            Uuid::new_v4(),
            s.event.uuid(),
            qualified::actor(s).actor,
            PartyFields {
                title: "Driver closure fixture".into(),
                description: "".into(),
                ticket_section: None,
                capacity: 4,
                meeting_starts_at: Utc::now() + chrono::TimeDelta::minutes(5),
                meeting_ends_at: Utc::now() + chrono::TimeDelta::hours(1),
                requirements: "".into(),
                public_place_id: Uuid::new_v4(),
            },
            Utc::now(),
        )?;
        sqlx::query("insert into concert_community_parties(event_id,party_id,owner_id,revision,state,member_count,capacity,meeting_revision,audience_revision,aggregate) values($1,$2,$3,1,'OPEN',1,4,$4,$5,$6)")
            .bind(s.event.uuid()).bind(party.id).bind(party.owner).bind(party.meeting_revision)
            .bind(party.audience_revision).bind(serde_json::to_value(&party)?).execute(&s.concert).await?;
        sqlx::query("insert into concert_community_members(event_id,party_id,user_id,is_owner,state,revision) values($1,$2,$3,true,'ACTIVE',1)")
            .bind(s.event.uuid()).bind(party.id).bind(party.owner).execute(&s.concert).await?;
        Ok(party.id)
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "requires marker-owned four-owner driver controller"]
    async fn native_driver_rollback_recovery_commit_and_replay() -> anyhow::Result<()> {
        Box::pin(driver_rollback()).await
    }
    async fn driver_rollback() -> anyhow::Result<()> {
        println!("BE41_SOURCE_PIN={}", owned_source_pin());
        let s = qualified::Stack::new().await?;
        let party = seed_party(&s).await?;
        let intent = qualified::ticket(
            &s.repo(),
            &s.subject,
            s.room,
            s.anchor.utc + chrono::TimeDelta::seconds(60),
            141,
        )
        .await?;
        let old = s
            .bridge
            .admit(intent, Uuid::new_v4())
            .await
            .map_err(|_| anyhow::anyhow!("four-owner admission"))?;
        CanonicalAuthorityPort::install_pending(&s.bridge, &old, s.now()).unwrap();
        assert!(CanonicalAuthorityPort::publication_ready(&s.bridge, &old));
        let driver = ReadOnlyCommandDriver::new(
            qualified::OperatorFixture,
            LaterUnavailable,
            s.bridge.clone(),
        )
        .unwrap();
        let actor = qualified::actor(&s);
        let key = Uuid::new_v4();
        let operation = EffectKey {
            event: actor.event,
            job: start_job_id(actor, key),
            step: Step::ReadOnly,
        }
        .operation_id();
        // Fail AFTER actual World/community work, at composite receipt insertion.
        // Private disposable database only; no production helper is substituted.
        sqlx::raw_sql("create function be41_driver_fail() returns trigger language plpgsql as $$ begin raise exception 'owned composite rollback'; end $$; create trigger be41_driver_failure before insert on concert_closure_read_only_barriers for each row execute function be41_driver_fail();")
            .execute(&s.concert).await?;
        assert!(matches!(
            driver
                .command(actor, key, qualified::start(), &qualified::keys())
                .await,
            Err(CommandError::Unavailable)
        ));
        let phase: String =
            sqlx::query_scalar("select phase from concert_events where event_id=$1")
                .bind(actor.event)
                .fetch_one(&s.concert)
                .await?;
        assert_eq!(phase, "OPEN");
        let state: String = sqlx::query_scalar(
            "select state from concert_community_parties where event_id=$1 and party_id=$2",
        )
        .bind(actor.event)
        .bind(party)
        .fetch_one(&s.concert)
        .await?;
        assert_eq!(state, "OPEN");
        let effects:i64=sqlx::query_scalar("select (select count(*) from concert_closure_jobs where event_id=$1)+(select count(*) from concert_world_read_only_receipts where event_id=$1)+(select count(*) from concert_closure_community_barriers where event_id=$1)+(select count(*) from concert_closure_read_only_barriers where event_id=$1)").bind(actor.event).fetch_one(&s.concert).await?;
        assert_eq!(effects, 0);
        assert!(
            CanonicalAuthorityPort::current(&s.bridge, &old, s.now()).is_err(),
            "rollback must not restore cached grants"
        );
        let blocked = qualified::ticket(
            &s.repo(),
            &s.subject,
            s.room,
            s.anchor.utc + chrono::TimeDelta::seconds(60),
            142,
        )
        .await?;
        assert!(
            s.bridge.admit(blocked, Uuid::new_v4()).await.is_err(),
            "failed command keeps native pending removal"
        );
        sqlx::raw_sql("drop trigger be41_driver_failure on concert_closure_read_only_barriers; drop function be41_driver_fail();").execute(&s.concert).await?;
        // Explicit fixture recovery: committed OPEN was re-read above; old views
        // stay invalid. A fresh admission rechecks all four real source services.
        s.bridge.cleanup().await?;
        s.bridge.finish_invalidation(operation)?;
        let fresh = qualified::ticket(
            &s.repo(),
            &s.subject,
            s.room,
            s.anchor.utc + chrono::TimeDelta::seconds(60),
            143,
        )
        .await?;
        let fresh = s
            .bridge
            .admit(fresh, Uuid::new_v4())
            .await
            .map_err(|_| anyhow::anyhow!("fresh qualified recovery"))?;
        CanonicalAuthorityPort::install_pending(&s.bridge, &fresh, s.now()).unwrap();
        assert!(CanonicalAuthorityPort::publication_ready(&s.bridge, &fresh));
        assert!(CanonicalAuthorityPort::current(&s.bridge, &old, s.now()).is_err());
        let receipt = driver
            .command(actor, key, qualified::start(), &qualified::keys())
            .await?;
        assert_eq!(receipt.state, ReceiptState::Succeeded);
        let replay = driver
            .command(actor, key, qualified::start(), &qualified::keys())
            .await?;
        assert_eq!(replay.id, receipt.id);
        let binding_count:i64=sqlx::query_scalar("select count(*) from concert_closure_read_only_barriers b join concert_world_read_only_receipts w on (w.event_id,w.job_id,w.operation_id,w.receipt_id,w.revision)=(b.event_id,b.job_id,b.operation_id,b.world_receipt_id,b.world_revision) join concert_closure_community_barriers c on (c.event_id,c.job_id,c.operation_id,c.receipt_id,c.revision)=(b.event_id,b.job_id,b.operation_id,b.community_receipt_id,b.community_revision) where b.event_id=$1").bind(actor.event).fetch_one(&s.concert).await?;
        assert_eq!(binding_count, 1);
        let state: String = sqlx::query_scalar(
            "select state from concert_community_parties where event_id=$1 and party_id=$2",
        )
        .bind(actor.event)
        .bind(party)
        .fetch_one(&s.concert)
        .await?;
        assert_eq!(state, "CANCELLED");
        assert!(!CanonicalAuthorityPort::publication_ready(
            &s.bridge, &fresh
        ));
        s.finish().await
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "requires marker-owned four-owner native driver controller"]
    async fn native_driver_drains_inflight_socket_before_composite_commit() -> anyhow::Result<()> {
        Box::pin(driver_socket()).await
    }
    async fn driver_socket() -> anyhow::Result<()> {
        use crate::adapters::r#in::world_ws::{router, SocketTransport, WorldSocketState};
        use crate::world::{
            ingress::{OriginPolicy, SafeClose},
            runtime::WorldRuntime,
        };
        use futures::{SinkExt, StreamExt};
        use serde_json::{json, Value};
        use std::{
            sync::atomic::{AtomicBool, AtomicUsize, Ordering},
            time::{Duration, Instant},
        };
        use tokio_tungstenite::{
            connect_async,
            tungstenite::{client::IntoClientRequest, Message},
        };
        println!("BE41_SOURCE_PIN={}", owned_source_pin());
        let s = qualified::Stack::new().await?;
        let _party = seed_party(&s).await?;
        let transport = SocketTransport::default();
        let trace = qualified::Trace {
            native: transport.clone(),
            paused: Arc::new(AtomicBool::new(false)),
            waits: Arc::new(AtomicUsize::new(0)),
            sent: Arc::new(Mutex::new(vec![])),
            hold_user: Arc::new(Mutex::new(None)),
            held: Arc::new(AtomicBool::new(false)),
            release: Arc::new(AtomicBool::new(false)),
        };
        let (_, owner, _, _) = s.bridge.invalidation_context();
        let mut target = qualified::geometry::spec(s.target, 2);
        target.room_epoch = Uuid::from_u128(7);
        let runtime = WorldRuntime::new(
            s.event,
            owner,
            s.anchor.clone(),
            vec![qualified::geometry::spec(s.room, 2), target],
            s.bridge.clone(),
            s.bridge.clone(),
            trace.clone(),
        )
        .unwrap();
        let (started, origin) = runtime.clock_origin();
        let (handle, task) = runtime.spawn();
        let handle = Arc::new(handle);
        let state = WorldSocketState::new(
            OriginPolicy::new(vec!["http://localhost:9000".into()], true).unwrap(),
            transport.clone(),
            s.bridge.clone(),
            handle.clone(),
            started,
            origin,
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            axum::serve(listener, router(state))
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap()
        });
        let mut request = format!("ws://{address}/world-ws").into_client_request()?;
        request
            .headers_mut()
            .insert("origin", "http://localhost:9000".parse()?);
        let (mut socket, _) = connect_async(request).await?;
        let intent = qualified::ticket(
            &s.repo(),
            &s.subject,
            s.room,
            s.anchor.utc + chrono::TimeDelta::seconds(60),
            144,
        )
        .await?;
        socket.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"authenticate","ticket":intent.ticket.expose_for_consumption(),"eventId":s.event.uuid(),"roomId":s.room,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1}).to_string())).await?;
        let first = loop {
            let msg = tokio::time::timeout(Duration::from_secs(2), socket.next())
                .await?
                .ok_or_else(|| anyhow::anyhow!("snapshot missing"))??;
            if let Message::Text(raw) = msg {
                let v: Value = serde_json::from_str(&raw)?;
                if v["type"] == "snapshot" {
                    break v;
                }
            }
        };
        let binding = trace.sent.lock().unwrap()[0].0.clone();
        assert!(CanonicalAuthorityPort::publication_ready(
            &s.bridge, &binding
        ));
        *trace.hold_user.lock().unwrap() = Some(qualified::actor(&s).actor);
        let done = Arc::new(AtomicBool::new(false));
        let entered = Arc::new(AtomicBool::new(false));
        let driver = ReadOnlyCommandDriver::new(
            qualified::OperatorFixture,
            LaterUnavailable,
            s.bridge.clone(),
        )
        .unwrap();
        let actor = qualified::actor(&s);
        let close = {
            let held = trace.held.clone();
            let entered = entered.clone();
            let done = done.clone();
            tokio::spawn(async move {
                let began = Instant::now();
                while !held.load(Ordering::Acquire) {
                    anyhow::ensure!(
                        began.elapsed() < Duration::from_secs(1),
                        "native handoff not reached"
                    );
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                entered.store(true, Ordering::Release);
                let receipt = driver
                    .command(
                        actor,
                        Uuid::new_v4(),
                        qualified::start(),
                        &qualified::keys(),
                    )
                    .await;
                done.store(true, Ordering::Release);
                Ok::<_, anyhow::Error>(receipt?)
            })
        };
        // A real native send runs under World's publication mutex. The driver
        // must wait for that send before dropping views or starting its SQL close.
        let release = {
            let held = trace.held.clone();
            let entered = entered.clone();
            let done = done.clone();
            let release = trace.release.clone();
            std::thread::spawn(move || {
                let began = Instant::now();
                while !(held.load(Ordering::Acquire) && entered.load(Ordering::Acquire)) {
                    assert!(began.elapsed() < Duration::from_secs(1));
                    std::thread::sleep(Duration::from_millis(1));
                }
                std::thread::sleep(Duration::from_millis(60));
                let blocked = !done.load(Ordering::Acquire);
                release.store(true, Ordering::Release);
                blocked
            })
        };
        socket.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"input","streamId":first["streamId"],"roomEpoch":first["roomEpoch"],"seq":1,"axisX":1,"axisY":0,"buttons":[]}).to_string())).await?;
        let receipt = tokio::time::timeout(Duration::from_secs(3), close).await???;
        assert!(
            release.join().unwrap(),
            "driver passed an in-flight native socket handoff"
        );
        assert_eq!(receipt.state, ReceiptState::Succeeded);
        let committed = Instant::now();
        assert!(!CanonicalAuthorityPort::publication_ready(
            &s.bridge, &binding
        ));
        let before = trace.sent.lock().unwrap().len();
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert!(trace
            .sent
            .lock()
            .unwrap()
            .iter()
            .skip(before)
            .all(|(_, v, at)| *at < committed
                || !matches!(v["type"].as_str(), Some("snapshot" | "delta"))));
        let counts:(String,i64,i64,i64)=sqlx::query_as("select phase,(select count(*) from concert_world_owner_leases where event_id=$1),(select count(*) from concert_closure_read_only_barriers where event_id=$1),(select count(*) from concert_community_parties where event_id=$1 and state='CANCELLED') from concert_events where event_id=$1").bind(actor.event).fetch_one(&s.concert).await?;
        assert_eq!(counts, ("READ_ONLY".into(), 0, 1, 1));
        let _ = socket.close(None).await;
        transport.close_all(SafeClose::UNAVAILABLE);
        handle.request_stop();
        tokio::time::timeout(Duration::from_secs(2), task).await??;
        let _ = stop.send(());
        tokio::time::timeout(Duration::from_secs(2), server).await??;
        s.finish().await
    }
    struct RaceAuthority {
        allowed: Arc<std::sync::atomic::AtomicBool>,
        checks: Arc<std::sync::atomic::AtomicUsize>,
        short: bool,
    }
    #[async_trait]
    impl FencedLifecycleAuthority for RaceAuthority {
        async fn check_in(
            &self,
            c: &mut PgConnection,
            q: AuthorityQuery,
            now: DateTime<Utc>,
        ) -> Result<AuthorityFence, CommandError> {
            use std::sync::atomic::Ordering;
            self.checks.fetch_add(1, Ordering::Release);
            if !self.allowed.load(Ordering::Acquire) {
                return Err(CommandError::Denied);
            }
            let mut fence = qualified::OperatorFixture.check_in(c, q, now).await?;
            if self.short {
                fence.expires_at = now + chrono::TimeDelta::milliseconds(60);
            }
            Ok(fence)
        }
        async fn retention_in(
            &self,
            c: &mut PgConnection,
            a: Actor,
            at: DateTime<Utc>,
        ) -> Result<Retention, CommandError> {
            qualified::OperatorFixture.retention_in(c, a, at).await
        }
    }
    async fn race_view(
        s: &qualified::Stack,
        seed: u8,
    ) -> anyhow::Result<crate::world::simulation::Binding> {
        let intent = qualified::ticket(
            &s.repo(),
            &s.subject,
            s.room,
            s.anchor.utc + chrono::TimeDelta::seconds(60),
            seed,
        )
        .await?;
        let binding = s
            .bridge
            .admit(intent, Uuid::new_v4())
            .await
            .map_err(|_| anyhow::anyhow!("qualified race admission"))?;
        CanonicalAuthorityPort::install_pending(&s.bridge, &binding, s.now()).unwrap();
        assert!(CanonicalAuthorityPort::publication_ready(
            &s.bridge, &binding
        ));
        Ok(binding)
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "requires marker-owned four-owner authorization-race controller"]
    async fn native_driver_revocation_while_delivery_wait_keeps_world_views() -> anyhow::Result<()>
    {
        Box::pin(revoked_delivery()).await
    }
    async fn revoked_delivery() -> anyhow::Result<()> {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        use std::time::Duration;
        println!("BE41_SOURCE_PIN={}", owned_source_pin());
        let s = qualified::Stack::new().await?;
        let binding = race_view(&s, 145).await?;
        let allowed = Arc::new(AtomicBool::new(true));
        let checks = Arc::new(AtomicUsize::new(0));
        let driver = ReadOnlyCommandDriver::new(
            RaceAuthority {
                allowed: allowed.clone(),
                checks: checks.clone(),
                short: false,
            },
            LaterUnavailable,
            s.bridge.clone(),
        )
        .unwrap();
        let gate = s.bridge.invalidation_delivery_gate().lock().await;
        let actor = qualified::actor(&s);
        let began = Arc::new(AtomicBool::new(false));
        let command = {
            let began = began.clone();
            tokio::spawn(async move {
                began.store(true, Ordering::Release);
                driver
                    .command(
                        actor,
                        Uuid::new_v4(),
                        qualified::start(),
                        &qualified::keys(),
                    )
                    .await
            })
        };
        while !began.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
        assert_eq!(
            checks.load(Ordering::Acquire),
            0,
            "authority must be read after waiting for delivery gate"
        );
        allowed.store(false, Ordering::Release);
        drop(gate);
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(2), command).await??,
            Err(CommandError::Denied)
        ));
        assert!(CanonicalAuthorityPort::publication_ready(
            &s.bridge, &binding
        ));
        assert!(CanonicalAuthorityPort::current(&s.bridge, &binding, s.now()).is_ok());
        let phase: String =
            sqlx::query_scalar("select phase from concert_events where event_id=$1")
                .bind(actor.event)
                .fetch_one(&s.concert)
                .await?;
        assert_eq!(phase, "OPEN");
        s.finish().await
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "requires marker-owned four-owner authorization-race controller"]
    async fn native_driver_expiry_while_native_wait_keeps_world_views() -> anyhow::Result<()> {
        Box::pin(expired_native()).await
    }
    async fn expired_native() -> anyhow::Result<()> {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        use std::time::{Duration, Instant};
        println!("BE41_SOURCE_PIN={}", owned_source_pin());
        let s = qualified::Stack::new().await?;
        let binding = race_view(&s, 146).await?;
        let allowed = Arc::new(AtomicBool::new(true));
        let checks = Arc::new(AtomicUsize::new(0));
        let driver = ReadOnlyCommandDriver::new(
            RaceAuthority {
                allowed,
                checks: checks.clone(),
                short: true,
            },
            LaterUnavailable,
            s.bridge.clone(),
        )
        .unwrap();
        let fence = s.owners.fence();
        let (held, holding) = std::sync::mpsc::channel();
        let blocker = {
            let checks = checks.clone();
            std::thread::spawn(move || {
                let _fence = fence.lock().unwrap();
                held.send(()).unwrap();
                let began = Instant::now();
                while checks.load(Ordering::Acquire) == 0 {
                    assert!(began.elapsed() < Duration::from_secs(1));
                    std::thread::sleep(Duration::from_millis(1));
                }
                std::thread::sleep(Duration::from_millis(160));
            })
        };
        holding.recv_timeout(Duration::from_secs(1))?;
        let result = driver
            .command(
                qualified::actor(&s),
                Uuid::new_v4(),
                qualified::start(),
                &qualified::keys(),
            )
            .await;
        blocker.join().unwrap();
        assert!(matches!(result, Err(CommandError::Unavailable)));
        assert!(
            CanonicalAuthorityPort::publication_ready(&s.bridge, &binding),
            "expired native wait cannot remove views or add a pending marker"
        );
        assert!(CanonicalAuthorityPort::current(&s.bridge, &binding, s.now()).is_ok());
        let phase: String =
            sqlx::query_scalar("select phase from concert_events where event_id=$1")
                .bind(s.event.uuid())
                .fetch_one(&s.concert)
                .await?;
        assert_eq!(phase, "OPEN");
        s.finish().await
    }
}
