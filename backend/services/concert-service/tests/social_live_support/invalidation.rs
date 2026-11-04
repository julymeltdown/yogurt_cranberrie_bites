//! Actual SQL/lease/deadline execution in the owned PG16 cluster. The command
//! fixture is deliberately synthetic and does not prove Auth, WS or IN03.
use crate::social_live::invalidation::{
    SocialInvalidationSink, SocialPolicyPublisher, VerifiedSocialInvalidation,
};
use async_trait::async_trait;
use concert_application::commands::{maintenance::InvalidationKind, SafeReference};
use concert_service::{
    adapters::out::repository::outbox::{Delivery, DeliveryFailure},
    workers::outbox::IdempotentInvalidationSink,
};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use uuid::Uuid;

#[derive(Default)]
struct Publisher {
    fail: AtomicBool,
    seen: Mutex<Vec<(Uuid, Uuid, i64, String)>>,
}
#[async_trait]
impl SocialPolicyPublisher for Publisher {
    async fn publish(&self, s: &VerifiedSocialInvalidation) -> Result<(), DeliveryFailure> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(DeliveryFailure::Retryable);
        }
        assert!(!s.signal_id().is_nil());
        self.seen.lock().unwrap().push((
            s.event(),
            s.actor(),
            s.policy_revision(),
            s.reason().as_str().into(),
        ));
        Ok(())
    }
}
async fn setup() -> (
    PgPool,
    Uuid,
    Uuid,
    Uuid,
    Arc<Publisher>,
    SocialInvalidationSink,
) {
    let pool = crate::owned::pool().await;
    let (event, a, b) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Policy metadata','LIVE')")
        .bind(event).bind(event.to_string()).execute(&pool).await.unwrap();
    let publisher = Arc::new(Publisher::default());
    let sink = SocialInvalidationSink::new(pool.clone(), publisher.clone());
    (pool, event, a, b, publisher, sink)
}
async fn pending(
    c: &mut PgConnection,
    event: Uuid,
    a: Uuid,
    target: Uuid,
    op: &str,
) -> (Uuid, Uuid) {
    let (receipt, outbox) = (Uuid::new_v4(), Uuid::new_v4());
    sqlx::query("insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state) values($1,$2,$3,$4,$5,$6,$7,'PENDING')")
        .bind(receipt).bind(a).bind(event).bind(op).bind(Uuid::new_v4()).bind(format!("hmac-sha256:v1:fixture:{}","0".repeat(64))).bind(target).execute(&mut *c).await.unwrap();
    sqlx::query("insert into concert_outbox(outbox_id,receipt_id,event_id,resource_id,resource_revision) values($1,$2,$3,$4,1)")
        .bind(outbox).bind(receipt).bind(event).bind(target).execute(c).await.unwrap();
    (receipt, outbox)
}
async fn succeed(c: &mut PgConnection, receipt: Uuid) {
    sqlx::query("update concert_command_receipts set state='SUCCEEDED',resource_id=target_id,resource_revision=1,result_status=200 where receipt_id=$1")
        .bind(receipt).execute(c).await.unwrap();
}
async fn source(p: &PgPool, event: Uuid, a: Uuid, b: Uuid, op: &str) -> Delivery {
    let mut tx = p.begin().await.unwrap();
    let (receipt, outbox) = pending(&mut tx, event, a, b, op).await;
    succeed(&mut tx, receipt).await;
    tx.commit().await.unwrap();
    let lease = Uuid::new_v4();
    sqlx::query("update concert_outbox set state='LEASED',attempts=1,lease_token=$2,lease_until=clock_timestamp()+interval '30 seconds' where outbox_id=$1")
        .bind(outbox).bind(lease).execute(p).await.unwrap();
    Delivery {
        id: outbox,
        receipt: Some(receipt),
        event,
        reference: SafeReference {
            resource: b,
            revision: 1,
        },
        kind: InvalidationKind::ResourceInvalidated,
        attempt: 1,
        lease,
    }
}

#[tokio::test]
#[ignore = "owned social_live native runner required"]
async fn policy_generation_is_atomic_with_success_and_replay_is_not_a_new_change() {
    let (p, e, a, b, _, s) = setup().await;
    assert_eq!(s.baseline(e, a).await.unwrap(), 1);
    let mut tx = p.begin().await.unwrap();
    let (r, _) = pending(&mut tx, e, a, b, "blockConcertUser").await;
    succeed(&mut tx, r).await;
    tx.rollback().await.unwrap();
    assert_eq!(s.baseline(e, a).await.unwrap(), 1);
    let d = source(&p, e, a, b, "blockConcertUser").await;
    assert_eq!(s.baseline(e, a).await.unwrap(), 2);
    assert_eq!(s.baseline(e, b).await.unwrap(), 2);
    // A replay cannot perform another state transition, hence no trigger bump.
    sqlx::query("update concert_command_receipts set state=state where receipt_id=$1")
        .bind(d.receipt)
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(s.baseline(e, a).await.unwrap(), 2);
    assert_eq!(s.baseline(e, Uuid::new_v4()).await.unwrap(), 1);
    p.close().await;
}

#[tokio::test]
#[ignore = "owned social_live native runner required"]
async fn policy_delivery_checks_every_source_dimension_and_retries_failed_publication() {
    let (p, e, a, b, pub_, s) = setup().await;
    let d = source(&p, e, a, b, "blockConcertUser").await;
    assert!(s.supports(&d).await.unwrap());
    for dimension in 0..7 {
        let mut fake = d.clone();
        match dimension {
            0 => fake.id = Uuid::new_v4(),
            1 => fake.receipt = Some(Uuid::new_v4()),
            2 => fake.event = Uuid::new_v4(),
            3 => fake.reference.resource = Uuid::new_v4(),
            4 => fake.reference.revision += 1,
            5 => fake.lease = Uuid::new_v4(),
            _ => fake.attempt += 1,
        }
        assert!(s.deliver(&fake).await.is_err());
    }
    assert!(pub_.seen.lock().unwrap().is_empty());
    // Deadline drain is not an alternate way to bypass the common queue lease.
    assert_eq!(s.drain_pending(100).await.unwrap(), 0);
    pub_.fail.store(true, Ordering::SeqCst);
    assert!(s.deliver(&d).await.is_err());
    let n:i64=sqlx::query_scalar("select count(*) from concert_social_policy_signals where source_outbox=$1 and published_at is not null").bind(d.id).fetch_one(&p).await.unwrap();
    assert_eq!(n, 0);
    pub_.fail.store(false, Ordering::SeqCst);
    s.deliver(&d).await.unwrap();
    s.deliver(&d).await.unwrap();
    let seen = pub_.seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert!(seen
        .iter()
        .all(|x| x.0 == e && (x.1 == a || x.1 == b) && x.2 == 2 && x.3 == "BLOCK_CHANGED"));
    drop(seen);
    // Stale lease never succeeds even when a previous attempt published rows.
    sqlx::query("update concert_outbox set lease_until=clock_timestamp()-interval '1 second' where outbox_id=$1").bind(d.id).execute(&p).await.unwrap();
    assert!(s.deliver(&d).await.is_err());
    p.close().await;
}

#[tokio::test]
#[ignore = "owned social_live native runner required"]
async fn held_baseline_fences_a_concurrent_source_commit_until_stream_registration() {
    let (p, e, a, b, _, s) = setup().await;
    let guard = s.held_baseline(e, a).await.unwrap();
    assert_eq!(guard.revision(), 1);
    let pool = p.clone();
    let (ready, wait) = tokio::sync::oneshot::channel();
    let mut task = tokio::spawn(async move {
        let mut tx = pool.begin().await.unwrap();
        let (r, _) = pending(&mut tx, e, a, b, "blockConcertUser").await;
        ready.send(()).unwrap();
        succeed(&mut tx, r).await;
        tx.commit().await.unwrap();
    });
    wait.await.unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(80), &mut task)
            .await
            .is_err()
    );
    guard.commit().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(s.baseline(e, a).await.unwrap(), 2);
    // A bootstrap lock must not extend an existing absolute proof deadline.
    let guard = s.held_baseline(e, a).await.unwrap();
    let pool = p.clone();
    let (ready, wait) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '100 milliseconds')::text,true)")
            .execute(&mut *tx).await.unwrap();
        let (r, _) = pending(&mut tx, e, a, b, "blockConcertUser").await;
        succeed(&mut tx, r).await;
        ready.send(()).unwrap();
        tx.commit().await
    });
    wait.await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    guard.commit().await.unwrap();
    assert!(task.await.unwrap().is_err());
    assert_eq!(s.baseline(e, a).await.unwrap(), 2);
    assert_eq!(s.baseline(e, b).await.unwrap(), 2);
    p.close().await;
}

#[tokio::test]
#[ignore = "owned social_live native runner required"]
async fn database_deadline_emits_durable_actor_scoped_signals_once_and_survives_sink_recreation() {
    let (p, e, a, b, pub_, s) = setup().await;
    let request = Uuid::new_v4();
    let intro = Uuid::new_v4();
    // A real future database deadline, not a caller clock override. No grant
    // or synthetic authority success is inferred from these storage fixtures.
    sqlx::query("insert into concert_intros(event_id,intro_id,owner_id,revision,state,text,created_at,updated_at,expires_at) values($1,$2,$3,1,'PUBLISHED','Expiry source',clock_timestamp()-interval '1 minute',clock_timestamp(),clock_timestamp()+interval '500 milliseconds')")
        .bind(e).bind(intro).bind(b).execute(&p).await.unwrap();
    let mut fixture = p.begin().await.unwrap();
    // Synthetic storage setup only; this local deadline satisfies the retained
    // deferred SQL guard and is not a canonical authority or endpoint proof.
    sqlx::query("select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '5 seconds')::text,true)")
        .execute(&mut *fixture).await.unwrap();
    sqlx::query("insert into concert_social_requests(event_id,request_id,requester_id,recipient_id,intro_id,intro_revision,authority_revision,relationship_revision,state,created_at,expires_at) values($1,$2,$3,$4,$5,1,1,1,'PENDING',clock_timestamp()-interval '1 minute',clock_timestamp()+interval '1100 milliseconds')")
        .bind(e).bind(request).bind(a).bind(b).bind(intro).execute(&mut *fixture).await.unwrap();
    fixture.commit().await.unwrap();
    assert_eq!(s.emit_expired(10).await.unwrap(), 0);
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    assert_eq!(s.emit_expired(10).await.unwrap(), 1);
    assert_eq!(s.emit_expired(10).await.unwrap(), 0);
    assert_eq!(s.baseline(e, a).await.unwrap(), 2);
    let next = SocialInvalidationSink::new(p.clone(), pub_.clone());
    pub_.fail.store(true, Ordering::SeqCst);
    assert!(next.drain_pending(10).await.is_err());
    pub_.fail.store(false, Ordering::SeqCst);
    assert_eq!(next.drain_pending(10).await.unwrap(), 2);
    assert_eq!(s.drain_pending(10).await.unwrap(), 0);
    assert!(pub_
        .seen
        .lock()
        .unwrap()
        .iter()
        .all(|v| v.0 == e && (v.1 == a || v.1 == b) && v.2 == 2 && v.3 == "EXPIRED"));
    // An emitted earlier Intro boundary must not mask a later request deadline.
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    assert_eq!(next.emit_expired(10).await.unwrap(), 1);
    assert_eq!(next.emit_expired(10).await.unwrap(), 0);
    assert_eq!(next.baseline(e, a).await.unwrap(), 3);
    assert_eq!(next.drain_pending(10).await.unwrap(), 2);
    let row=sqlx::query("select clock_timestamp()>=deadline_at due,published_at is not null done from concert_social_policy_signals where event_id=$1").bind(e).fetch_all(&p).await.unwrap();
    assert_eq!(row.len(), 4);
    assert!(row
        .iter()
        .all(|r| r.get::<bool, _>("due") && r.get::<bool, _>("done")));
    p.close().await;
}

#[tokio::test]
#[ignore = "owned social_live native runner required"]
async fn unrelated_source_is_not_acknowledged_and_baselines_reject_nil_scope() {
    let (p, e, a, b, pub_, s) = setup().await;
    let d = source(&p, e, a, b, "setConcertSettings").await;
    assert!(!s.supports(&d).await.unwrap());
    assert!(s.deliver(&d).await.is_err());
    assert!(s.baseline(Uuid::nil(), a).await.is_err());
    assert!(s.baseline(e, Uuid::nil()).await.is_err());
    assert!(s.emit_expired(0).await.is_err());
    assert!(s.drain_pending(101).await.is_err());
    assert!(pub_.seen.lock().unwrap().is_empty());
    p.close().await;
}
