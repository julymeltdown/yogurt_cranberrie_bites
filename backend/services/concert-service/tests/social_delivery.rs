//! Owned PostgreSQL/real client failure-path qualification. Synthetic command
//! fixtures and the recording publisher do not certify public HTTP/WS or World.
pub use concert_service::{adapters, social_live, workers, world};
#[path = "../src/config/social_delivery.rs"]
mod delivery;
#[path = "social_remaining_support/owned.rs"]
mod owned;
use adapters::out::notification::{
    delivery::NotificationDeliveryClient, routing::SocialProjectionRouting,
    source::PgProjectionOwner,
};
use adapters::out::repository::outbox::DeliveryFailure;
use async_trait::async_trait;
use concert_application::notification::NotificationProjectionOwner;
use concert_domain::notification::Envelope;
use delivery::SocialDeliveryRuntime;
use social_live::invalidation::{
    SocialInvalidationSink, SocialPolicyPublisher, VerifiedSocialInvalidation,
};
use sqlx::{PgPool, Row};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use tokio::sync::{oneshot, Notify};
use uuid::Uuid;

struct Publisher {
    event: Uuid,
    fail: AtomicBool,
    hold: AtomicBool,
    active: Arc<AtomicUsize>,
    entered: Notify,
    seen: Mutex<Vec<(Uuid, i64)>>,
}
struct Active(Arc<AtomicUsize>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
#[async_trait]
impl SocialPolicyPublisher for Publisher {
    async fn publish(&self, s: &VerifiedSocialInvalidation) -> Result<(), DeliveryFailure> {
        if s.event() != self.event {
            return Ok(());
        } // No matching fixture view.
        self.active.fetch_add(1, Ordering::SeqCst);
        let _active = Active(self.active.clone());
        self.entered.notify_one();
        if self.hold.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        if self.fail.load(Ordering::SeqCst) {
            return Err(DeliveryFailure::Retryable);
        }
        self.seen
            .lock()
            .unwrap()
            .push((s.actor(), s.policy_revision()));
        Ok(())
    }
}
struct Fixture {
    pool: PgPool,
    event: Uuid,
    actor: Uuid,
    peer: Uuid,
    publisher: Arc<Publisher>,
}
impl Fixture {
    async fn new() -> Self {
        let pool = owned::pool().await;
        let event = Uuid::new_v4();
        sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Delivery fixture','LIVE')")
            .bind(event).bind(event.to_string()).execute(&pool).await.unwrap();
        let publisher = Arc::new(Publisher {
            event,
            fail: AtomicBool::new(false),
            hold: AtomicBool::new(false),
            active: Arc::new(AtomicUsize::new(0)),
            entered: Notify::new(),
            seen: Mutex::new(vec![]),
        });
        Self {
            pool,
            event,
            actor: Uuid::new_v4(),
            peer: Uuid::new_v4(),
            publisher,
        }
    }
    fn runtime(&self) -> SocialDeliveryRuntime {
        let root = std::path::PathBuf::from(std::env::var("MCR_SOCIAL_ROOT").unwrap());
        let tls = platform::grpc_tls::TlsMaterial::load(
            root.join("pki/concert.crt"),
            root.join("pki/concert.key"),
            root.join("pki/ca.crt"),
        )
        .unwrap();
        // Actual TLS transport to an explicitly closed loopback port. No test
        // listener, successful Notification stub or production fallback exists.
        let client = NotificationDeliveryClient::connect(
            "https://127.0.0.1:1",
            "delivery-owned-fixture-credential",
            &tls,
        )
        .unwrap();
        SocialDeliveryRuntime::new(
            self.pool.clone(),
            SocialInvalidationSink::new(self.pool.clone(), self.publisher.clone()),
            Arc::new(client),
        )
    }
    async fn transaction(&self) -> sqlx::Transaction<'_, sqlx::Postgres> {
        let mut tx = self.pool.begin().await.unwrap();
        // Synthetic retained-storage setup, never an acquired authority scope.
        for key in [
            "mcr.relationship_authority_until",
            "mcr.operations_authority_until",
        ] {
            sqlx::query(
                "select set_config($1,(clock_timestamp()+interval '5 seconds')::text,true)",
            )
            .bind(key)
            .execute(&mut *tx)
            .await
            .unwrap();
        }
        tx
    }
    async fn source(&self, op: &str, revision: i64, request: Option<Uuid>) -> Envelope {
        let mut tx = self.transaction().await;
        if op == "closeConcertEvent" {
            // Foreign-job storage fixture only: retain migration 0018's real
            // deadline guard without claiming an acquired closure authority.
            sqlx::query("select set_config('mcr.lifecycle_authority_until',(clock_timestamp()+interval '5 seconds')::text,true)")
                .execute(&mut *tx).await.unwrap();
        }
        let target = request.unwrap_or(self.peer);
        let e = Envelope {
            outbox_id: Uuid::new_v4(),
            receipt_id: Uuid::new_v4(),
            event_id: self.event,
            resource_id: target,
            resource_revision: revision,
        };
        let actor = if op == "decideConcertSocialRequest" {
            self.peer
        } else {
            self.actor
        };
        sqlx::query("insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state) values($1,$2,$3,$4,$5,$6,$7,'PENDING')")
            .bind(e.receipt_id).bind(actor).bind(self.event).bind(op).bind(Uuid::new_v4()).bind(format!("hmac-sha256:v1:fixture:{}","0".repeat(64))).bind(target).execute(&mut *tx).await.unwrap();
        sqlx::query("insert into concert_outbox(outbox_id,receipt_id,event_id,resource_id,resource_revision) values($1,$2,$3,$4,$5)")
            .bind(e.outbox_id).bind(e.receipt_id).bind(self.event).bind(target).bind(revision).execute(&mut *tx).await.unwrap();
        sqlx::query("update concert_command_receipts set state='SUCCEEDED',resource_id=target_id,resource_revision=$2,result_status=$3 where receipt_id=$1")
            .bind(e.receipt_id).bind(revision).bind(if op=="createConcertSocialRequest" {201i16} else {200i16}).execute(&mut *tx).await.unwrap();
        tx.commit().await.unwrap();
        e
    }
    async fn request(&self, revision: i64, deadline: &str) -> Uuid {
        let id = Uuid::new_v4();
        let mut tx = self.transaction().await;
        sqlx::query("insert into concert_social_requests(event_id,request_id,requester_id,recipient_id,intro_id,intro_revision,authority_revision,relationship_revision,state,expires_at) values($1,$2,$3,$4,$5,1,1,1,'PENDING',clock_timestamp()+$6::interval)")
            .bind(self.event).bind(id).bind(self.actor).bind(self.peer).bind(Uuid::new_v4()).bind(deadline).execute(&mut *tx).await.unwrap();
        if revision == 2 {
            sqlx::query("update concert_social_requests set revision=2,state='CANCELLED' where event_id=$1 and request_id=$2").bind(self.event).bind(id).execute(&mut *tx).await.unwrap();
        }
        tx.commit().await.unwrap();
        id
    }
    async fn routing_rows(&self) {
        let mut tx = self.transaction().await;
        sqlx::query("insert into concert_event_settings(event_id,parties,giveaways,intros,social,mural,beacon_ttl_seconds,pickup_grace_seconds,social_grant_ttl_seconds,mural_cooldown_ms) values($1,true,true,true,true,true,60,60,60,60)").bind(self.event).execute(&mut *tx).await.unwrap();
        for user in [self.actor, self.peer] {
            sqlx::query("insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')").bind(self.event).bind(user).execute(&mut *tx).await.unwrap();
            sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(self.event).bind(user).execute(&mut *tx).await.unwrap();
        }
        sqlx::query("insert into concert_relationship_pairs(event_id,low_user,high_user,low_blocks_high,high_blocks_low) values($1,$2,$3,false,false)")
            .bind(self.event).bind(self.actor.min(self.peer)).bind(self.actor.max(self.peer)).execute(&mut *tx).await.unwrap();
        tx.commit().await.unwrap();
    }
    async fn state(&self, e: Envelope) -> (String, i32) {
        let r = sqlx::query("select state,attempts from concert_outbox where outbox_id=$1")
            .bind(e.outbox_id)
            .fetch_one(&self.pool)
            .await
            .unwrap();
        (r.get("state"), r.get("attempts"))
    }
    async fn retire_fixture_jobs(&self) {
        // Explicit fixture teardown after assertions. Never count this as a
        // runtime ACK or delivered effect; preserve original attempt evidence.
        sqlx::query("update concert_outbox set state='DEAD',lease_token=null,lease_until=null,terminal_at=clock_timestamp(),last_error='REJECTED' where event_id=$1 and state in ('READY','LEASED')").bind(self.event).execute(&self.pool).await.unwrap();
    }
}

#[tokio::test]
#[ignore = "owned Social delivery allocation required"]
async fn exact_handle_and_intro_families_ack_after_sealed_policy_only() {
    let f = Fixture::new().await;
    let mut jobs = vec![];
    for op in [
        "setConcertHandle",
        "removeConcertHandle",
        "setConcertIntro",
        "publishConcertIntro",
        "closeConcertIntro",
    ] {
        jobs.push(f.source(op, 1, None).await);
    }
    let first = f.runtime().run_batch().await.unwrap();
    let second = f.runtime().run_batch().await.unwrap();
    assert_eq!(
        (
            first.claimed + second.claimed,
            first.acknowledged + second.acknowledged,
            first.failed + second.failed
        ),
        (5, 5, 0)
    );
    for e in jobs {
        assert_eq!(f.state(e).await, ("DELIVERED".into(), 1));
    }
    assert_eq!(f.publisher.seen.lock().unwrap().len(), 5);
    f.pool.close().await;
}

#[tokio::test]
#[ignore = "owned Social delivery allocation required"]
async fn foreign_and_unbound_block_jobs_keep_their_attempt_budget() {
    let f = Fixture::new().await;
    let mut jobs = vec![];
    for op in [
        "blockConcertUser",
        "unblockConcertUser",
        "closeConcertEvent",
        "setConcertSettings",
        "moderateConcertUser",
    ] {
        jobs.push(f.source(op, 1, None).await);
    }
    let exhausted = jobs[0];
    sqlx::query("update concert_outbox set state='LEASED',attempts=8,lease_token=$2,lease_until=clock_timestamp()-interval '1 second' where outbox_id=$1")
        .bind(exhausted.outbox_id).bind(Uuid::new_v4()).execute(&f.pool).await.unwrap();
    let batch = f.runtime().run_batch().await.unwrap();
    assert_eq!(batch.claimed, 0);
    assert_eq!(f.state(exhausted).await, ("LEASED".into(), 8));
    for e in jobs.into_iter().skip(1) {
        assert_eq!(f.state(e).await, ("READY".into(), 0));
    }
    // Execute the exact production predicate with a bound owner event. This
    // tests SQL scope only: no fake World consumer, publication or ACK.
    let current = Fixture::new().await;
    let own_block = current.source("blockConcertUser", 1, None).await;
    let selected = delivery::scoped_claim_for_test(&current.pool, current.event)
        .await
        .unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].id, own_block.outbox_id);
    assert_eq!(f.state(exhausted).await, ("LEASED".into(), 8));
    assert_eq!(current.state(own_block).await, ("LEASED".into(), 1));
    current.retire_fixture_jobs().await;
    current.pool.close().await;
    assert!(f.publisher.seen.lock().unwrap().is_empty());
    f.retire_fixture_jobs().await;
    f.pool.close().await;
}

#[tokio::test]
#[ignore = "owned Social delivery allocation required"]
async fn four_request_families_never_ack_when_actual_notification_transport_fails() {
    let f = Fixture::new().await;
    f.routing_rows().await;
    let mut jobs = vec![];
    for op in [
        "createConcertSocialRequest",
        "decideConcertSocialRequest",
        "cancelConcertSocialRequest",
        "revokeConcertSocialRequest",
    ] {
        let revision = if op == "createConcertSocialRequest" {
            1
        } else {
            2
        };
        let request = f.request(revision, "1 hour").await;
        let e = f.source(op, revision, Some(request)).await;
        // Prove the real projection can resolve before testing transport refusal.
        assert_eq!(
            PgProjectionOwner::new(f.pool.clone(), SocialProjectionRouting)
                .resolve(e)
                .await
                .unwrap()
                .len(),
            1
        );
        jobs.push(e);
    }
    let rt = f.runtime();
    let batch = rt.run_batch().await.unwrap();
    assert_eq!((batch.claimed, batch.acknowledged, batch.failed), (4, 0, 4));
    for e in &jobs {
        assert_eq!(f.state(*e).await, ("READY".into(), 1));
    }
    let published = f.publisher.seen.lock().unwrap().len();
    assert_eq!(published, 8);
    sqlx::query("update concert_outbox set available_at=clock_timestamp() where event_id=$1")
        .bind(f.event)
        .execute(&f.pool)
        .await
        .unwrap();
    let batch = rt.run_batch().await.unwrap();
    assert_eq!((batch.claimed, batch.acknowledged, batch.failed), (4, 0, 4));
    assert_eq!(f.publisher.seen.lock().unwrap().len(), published);
    f.retire_fixture_jobs().await;
    f.pool.close().await;
}

#[tokio::test]
#[ignore = "owned Social delivery allocation required"]
async fn failed_policy_publication_retries_before_any_global_ack() {
    let f = Fixture::new().await;
    let e = f.source("setConcertHandle", 1, None).await;
    f.publisher.fail.store(true, Ordering::SeqCst);
    assert_eq!(f.runtime().run_batch().await.unwrap().failed, 1);
    assert_eq!(f.state(e).await, ("READY".into(), 1));
    assert!(f.publisher.seen.lock().unwrap().is_empty());
    f.publisher.fail.store(false, Ordering::SeqCst);
    sqlx::query("update concert_outbox set available_at=clock_timestamp() where outbox_id=$1")
        .bind(e.outbox_id)
        .execute(&f.pool)
        .await
        .unwrap();
    assert_eq!(f.runtime().run_batch().await.unwrap().acknowledged, 1);
    assert_eq!(f.state(e).await, ("DELIVERED".into(), 2));
    f.pool.close().await;
}

#[tokio::test]
#[ignore = "owned Social delivery allocation required"]
async fn database_deadline_poll_publishes_without_acknowledging_command_jobs() {
    let f = Fixture::new().await;
    let request = f.request(1, "100 milliseconds").await;
    let foreign = f.source("closeConcertEvent", 1, None).await;
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let rt = f.runtime();
    rt.run_expiry_batch().await.unwrap();
    rt.run_expiry_batch().await.unwrap();
    let (count,done):(i64,i64)=sqlx::query_as("select count(*),count(published_at) from concert_social_policy_signals where event_id=$1 and deadline_request=$2").bind(f.event).bind(request).fetch_one(&f.pool).await.unwrap();
    assert_eq!((count, done), (2, 2));
    assert_eq!(f.publisher.seen.lock().unwrap().len(), 2);
    assert_eq!(f.state(foreign).await, ("READY".into(), 0));
    f.retire_fixture_jobs().await;
    f.pool.close().await;
}

#[tokio::test]
#[ignore = "owned Social delivery allocation required"]
async fn world_receiver_gates_all_claims_and_shutdown_or_closed_sender_never_ack() {
    let f = Fixture::new().await;
    let e = f.source("setConcertHandle", 1, None).await;
    let (world_tx, world_rx) = oneshot::channel();
    let (stop_tx, stop_rx) = oneshot::channel();
    let rt = f.runtime().with_world(world_rx);
    assert!(rt.run_batch().await.is_err());
    let task = tokio::spawn(rt.run_until(async {
        let _ = stop_rx.await;
    }));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(f.state(e).await, ("READY".into(), 0));
    stop_tx.send(()).unwrap();
    assert!(task.await.unwrap().is_ok());
    drop(world_tx);
    let (tx, rx) = oneshot::channel();
    drop(tx);
    assert!(f
        .runtime()
        .with_world(rx)
        .run_until(std::future::pending())
        .await
        .is_err());
    assert_eq!(f.state(e).await, ("READY".into(), 0));
    f.retire_fixture_jobs().await;
    f.pool.close().await;
}

#[tokio::test]
#[ignore = "owned Social delivery allocation required"]
async fn shutdown_aborts_inflight_publication_without_ack_or_detached_tasks() {
    let f = Fixture::new().await;
    let e = f.source("setConcertHandle", 1, None).await;
    f.publisher.hold.store(true, Ordering::SeqCst);
    let (stop_tx, stop_rx) = oneshot::channel();
    let rt = f.runtime();
    let task = tokio::spawn(rt.run_until(async {
        let _ = stop_rx.await;
    }));
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        f.publisher.entered.notified(),
    )
    .await
    .unwrap();
    stop_tx.send(()).unwrap();
    assert!(task.await.unwrap().is_ok());
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while f.publisher.active.load(Ordering::SeqCst) != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(f.state(e).await, ("LEASED".into(), 1));
    assert!(f.publisher.seen.lock().unwrap().is_empty());
    f.retire_fixture_jobs().await;
    f.pool.close().await;
}

#[tokio::test]
#[ignore = "owned Social privacy recovery allocation required"]
async fn deferred_expiry_publication_keeps_supervisor_alive_and_preserves_foreign_outbox() {
    let f = Fixture::new().await;
    let request = f.request(1, "150 milliseconds").await;
    let foreign = f.source("closeConcertEvent", 1, None).await;
    f.publisher.fail.store(true, Ordering::SeqCst);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let rt = f.runtime();
    // The actual supervisor used to return Err at its first normal publisher
    // Retryable. It must remain alive while both expiry sources stay pending.
    let (stop, stopped) = oneshot::channel();
    let mut task = tokio::spawn(rt.run_until(async {
        let _ = stopped.await;
    }));
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        f.publisher.entered.notified(),
    )
    .await
    .unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(80), &mut task)
            .await
            .is_err()
    );
    let pending: (i64, i64) = sqlx::query_as("select count(*),count(published_at) from concert_social_policy_signals where event_id=$1 and deadline_request=$2")
        .bind(f.event).bind(request).fetch_one(&f.pool).await.unwrap();
    assert_eq!(pending, (2, 0));
    assert_eq!(f.state(foreign).await, ("READY".into(), 0));
    f.publisher.fail.store(false, Ordering::SeqCst);
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let published: i64 = sqlx::query_scalar("select count(*) from concert_social_policy_signals where event_id=$1 and deadline_request=$2 and published_at is not null")
                .bind(f.event).bind(request).fetch_one(&f.pool).await.unwrap();
            if published == 2 { break; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    stop.send(()).unwrap();
    assert!(task.await.unwrap().is_ok());
    assert_eq!(f.state(foreign).await, ("READY".into(), 0));
    assert_eq!(f.publisher.seen.lock().unwrap().len(), 2);
    // No force-settlement or fixture job retirement is used for this proof.
    f.pool.close().await;
}

#[tokio::test]
#[ignore = "owned Social privacy recovery allocation required"]
async fn expiry_storage_failure_remains_a_fatal_supervisor_error() {
    let f = Fixture::new().await;
    let rt = f.runtime();
    f.pool.close().await;
    assert!(rt.run_expiry_batch().await.is_err());
    assert!(rt.run_until(std::future::pending()).await.is_err());
    assert!(f.publisher.seen.lock().unwrap().is_empty());
}
