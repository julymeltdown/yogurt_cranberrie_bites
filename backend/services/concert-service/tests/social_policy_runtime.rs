//! Real Social factory/SQL and incumbent-registry component checks. Stored
//! identities/memberships are explicit fixtures, not Auth/admission or IN03 proof.
pub use concert_service::{adapters, social_live};
#[path = "social_remaining_support/owned.rs"]
mod owned;
use concert_service::config::social_policy_runtime::SocialWorldPolicy;
use social_live::invalidation::SocialInvalidationSink;
use sqlx::{PgPool, Row};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

#[tokio::test]
async fn composed_publisher_follows_transport_lifetime_without_an_arc_cycle_or_database_io() {
    let p = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgresql://localhost/unused")
        .unwrap();
    let factory = SocialWorldPolicy::compose(p);
    let weak = Arc::downgrade(&factory.publisher);
    let transport = factory.transport.clone();
    drop(factory);
    assert!(
        weak.upgrade().is_some(),
        "held baseline must retain its actual publisher"
    );
    drop(transport);
    assert!(
        weak.upgrade().is_none(),
        "registry-only publisher must not cycle back to baseline owner"
    );
}

async fn event(p: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Actual policy factory component','LIVE')")
        .bind(id).bind(id.to_string()).execute(p).await.unwrap();
    id
}
async fn foreign_membership(p: &PgPool, event: Uuid, actor: Uuid) {
    let room = Uuid::new_v4();
    sqlx::query("insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,avatar_manifest_id,avatar_manifest_version) values($1,$2,'Policy component',10,'map_fixture',1,'avatar_fixture',1)")
        .bind(event).bind(room).execute(p).await.unwrap();
    sqlx::query("insert into concert_world_memberships(event_id,user_id,room_id,session_id,connection_id,owner_process_id,owner_epoch,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,admitted_at,lease_until,access_expires_at) values($1,$2,$3,$4,$5,'foreign_fixture',$6,'map_fixture',1,'avatar_fixture',1,clock_timestamp(),clock_timestamp()+interval '1 hour',clock_timestamp()+interval '2 hours')")
        .bind(event).bind(actor).bind(room).bind(Uuid::new_v4()).bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(p).await.unwrap();
}
async fn request(p: &PgPool, event: Uuid, a: Uuid, b: Uuid) -> Uuid {
    let intro = Uuid::new_v4();
    let request = Uuid::new_v4();
    sqlx::query("insert into concert_intros(event_id,intro_id,owner_id,revision,state,text,expires_at) values($1,$2,$3,1,'PUBLISHED','Factory expiry component',clock_timestamp()+interval '1 hour')")
        .bind(event).bind(intro).bind(b).execute(p).await.unwrap();
    let mut tx = p.begin().await.unwrap();
    sqlx::query("select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '4 seconds')::text,true)").execute(&mut *tx).await.unwrap();
    sqlx::query("insert into concert_social_requests(event_id,request_id,requester_id,recipient_id,intro_id,intro_revision,authority_revision,relationship_revision,state,created_at,expires_at) values($1,$2,$3,$4,$5,1,1,1,'PENDING',clock_timestamp()-interval '1 minute',clock_timestamp()+interval '250 milliseconds')")
        .bind(event).bind(request).bind(a).bind(b).bind(intro).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    request
}

#[tokio::test]
#[ignore = "owned Social privacy recovery allocation required"]
async fn same_exposed_publisher_refuses_uncovered_serving_memberships_and_retries_exact_source() {
    let p = owned::pool().await;
    let e = event(&p).await;
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    foreign_membership(&p, e, a).await;
    foreign_membership(&p, e, b).await;
    let factory = SocialWorldPolicy::compose(p.clone());
    let supplied = factory.publisher.clone();
    assert!(Arc::ptr_eq(&supplied, &factory.publisher));
    let deletion_port_sink = SocialInvalidationSink::new(p.clone(), supplied);
    let request = request(&p, e, a, b).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    factory.sink.emit_expired(100).await.unwrap();
    // Both real construction paths refuse an empty registry while the database
    // reports active foreign connections; no mock publisher supplies success.
    assert_eq!(
        factory.sink.drain_pending_round(2).await.unwrap().deferred,
        2
    );
    assert_eq!(
        deletion_port_sink
            .drain_pending_round(2)
            .await
            .unwrap()
            .deferred,
        2
    );
    let done: i64 = sqlx::query_scalar("select count(*) from concert_social_policy_signals where event_id=$1 and deadline_request=$2 and published_at is not null")
        .bind(e).bind(request).fetch_one(&p).await.unwrap();
    assert_eq!(done, 0);
    // Explicit membership-storage transition only; this does NOT certify the
    // real disconnect/WS/cache path. Disconnected rows no longer claim serving.
    sqlx::query("update concert_world_memberships set disconnected=true where event_id=$1")
        .bind(e)
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(
        deletion_port_sink
            .drain_pending_round(8)
            .await
            .unwrap()
            .published,
        2
    );
    assert_eq!(
        factory.sink.drain_pending_round(8).await.unwrap().attempted,
        0
    );
    let baseline = factory.sink.held_baseline(e, a).await.unwrap();
    assert_eq!(baseline.revision(), 2);
    baseline.commit().await.unwrap();
    drop(factory);
    let rebuilt = SocialWorldPolicy::compose(p.clone());
    assert_eq!(rebuilt.sink.baseline(e, a).await.unwrap(), 2);
    assert_eq!(rebuilt.sink.baseline(e, b).await.unwrap(), 2);
    p.close().await;
}

#[tokio::test]
#[ignore = "owned Social privacy recovery allocation required"]
async fn held_factory_baseline_excludes_generation_writer_and_ignores_other_event_memberships() {
    let p = owned::pool().await;
    let e = event(&p).await;
    let other = event(&p).await;
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    foreign_membership(&p, other, a).await;
    let factory = SocialWorldPolicy::compose(p.clone());
    let held = factory.sink.held_baseline(e, a).await.unwrap();
    assert_eq!(held.revision(), 1);
    let pool = p.clone();
    let (entered, entered_rx) = tokio::sync::oneshot::channel();
    let mut writer = tokio::spawn(async move {
        entered.send(()).unwrap();
        sqlx::query("update concert_social_policy_generations set revision=revision+1 where event_id=$1 and actor_id=$2 returning revision")
            .bind(e).bind(a).fetch_one(&pool).await.unwrap().get::<i64,_>("revision")
    });
    entered_rx.await.unwrap();
    assert!(tokio::time::timeout(Duration::from_millis(50), &mut writer)
        .await
        .is_err());
    held.commit().await.unwrap();
    assert_eq!(writer.await.unwrap(), 2);
    request(&p, e, a, b).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    factory.sink.emit_expired(100).await.unwrap();
    assert_eq!(
        factory.sink.drain_pending_round(8).await.unwrap().published,
        2
    );
    assert_eq!(factory.sink.baseline(e, a).await.unwrap(), 3);
    assert_eq!(factory.sink.baseline(e, b).await.unwrap(), 2);
    p.close().await;
}
