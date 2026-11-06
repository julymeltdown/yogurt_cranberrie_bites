//! Test-only six-entry probe: each entry calls the production local visibility function, and this file adds no SQL predicate of its own.
use crate::adapters::out::notification::{routing::probe_recipients_in, source::Source};
use concert_domain::{notification::Envelope, AuthUserId, EventId};
use sqlx::PgPool;
use uuid::Uuid;

async fn pool() -> PgPool {
    let url = std::env::var("CONCERT_CONSUMER_TEST_URL")
        .expect("owned consumer probe database required");
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .expect("owned consumer probe database")
}

fn source(event: EventId, request: Uuid, actor: AuthUserId) -> Source {
    Source {
        envelope: Envelope {
            outbox_id: Uuid::new_v4(),
            receipt_id: Uuid::new_v4(),
            event_id: event.uuid(),
            resource_id: request,
            resource_revision: 1,
        },
        actor_id: actor.uuid(),
        operation: "createConcertSocialRequest".into(),
    }
}

async fn flags(pool: &PgPool, event: EventId, a: AuthUserId, b: AuthUserId, request: Uuid) -> [bool; 6] {
    let mut c = pool.acquire().await.unwrap();
    let presence = crate::world::recipient_owners::probe_presence_visible_in(&mut c, event, a, b).await;
    let party = crate::party_live::probe_visible_in(&mut c, event.uuid(), a.uuid(), b.uuid()).await;
    let gift = crate::gift_live::private::probe_gift_visible_in(&mut c, event.uuid(), a.uuid(), b.uuid()).await;
    let feed = crate::adapters::out::public_profile::probe_visible_in(&mut c, event.uuid(), a.uuid(), b.uuid()).await;
    let intro = crate::intro_live::probe_intro_visible_in(&mut c, event, b, a).await;
    let notification = probe_recipients_in(&mut c, &source(event, request, a)).await;
    [presence, party, gift, feed, intro, notification == 1]
}

#[tokio::test]
async fn committed_block_and_suspension_remove_all_six_read_entries_and_reread_after_revision() {
    let pool = pool().await;
    let event = EventId::from_uuid(Uuid::new_v4());
    let a = AuthUserId::from_uuid(Uuid::new_v4());
    let b = AuthUserId::from_uuid(Uuid::new_v4());
    let request = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Consumer probe','OPEN')")
        .bind(event.uuid()).bind(format!("consumer-{}", event.uuid())).execute(&pool).await.unwrap();
    for user in [a.uuid(), b.uuid()] {
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)")
            .bind(event.uuid()).bind(user).execute(&pool).await.unwrap();
        sqlx::query("insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')")
            .bind(event.uuid()).bind(user).execute(&pool).await.unwrap();
    }
    // Notification needs settings and a real request row; other probes read only the shared event.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("select set_config('mcr.operations_authority_until',(clock_timestamp()+interval '1 minute')::text,true),set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '1 minute')::text,true)")
        .execute(&mut *tx).await.unwrap();
    sqlx::query("insert into concert_event_settings(event_id,parties,giveaways,intros,social,mural,beacon_ttl_seconds,pickup_grace_seconds,social_grant_ttl_seconds,mural_cooldown_ms) values($1,true,true,true,true,true,60,60,3600,1000)")
        .bind(event.uuid()).execute(&mut *tx).await.unwrap();
    sqlx::query("insert into concert_social_requests(event_id,request_id,requester_id,recipient_id,intro_id,intro_revision,authority_revision,relationship_revision,state,expires_at) values($1,$2,$3,$4,$5,1,1,1,'PENDING',clock_timestamp()+interval '1 hour')")
        .bind(event.uuid()).bind(request).bind(a.uuid()).bind(b.uuid()).bind(Uuid::new_v4())
        .execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();

    let clean = flags(&pool, event, a, b, request).await;
    assert_eq!(clean, [true; 6], "baseline must expose all six entries");

    let revision_before: i64 = sqlx::query_scalar("select coalesce(max(revision),0) from concert_relationship_pairs where event_id=$1")
        .bind(event.uuid()).fetch_one(&pool).await.unwrap();
    let (low, high) = if a.uuid() <= b.uuid() { (a.uuid(), b.uuid()) } else { (b.uuid(), a.uuid()) };
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '1 minute')::text,true)")
        .execute(&mut *tx).await.unwrap();
    sqlx::query("insert into concert_relationship_pairs(event_id,low_user,high_user,low_blocks_high) values($1,$2,$3,true) on conflict(event_id,low_user,high_user) do update set low_blocks_high=true,revision=concert_relationship_pairs.revision+1")
        .bind(event.uuid()).bind(low).bind(high).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let revision_after: i64 = sqlx::query_scalar("select max(revision) from concert_relationship_pairs where event_id=$1")
        .bind(event.uuid()).fetch_one(&pool).await.unwrap();
    assert!(revision_after > revision_before, "policy revision must advance for cache invalidation");
    assert_eq!(flags(&pool, event, a, b, request).await, [false; 6], "committed block must remove all six");

    let mut tx = pool.begin().await.unwrap();
    sqlx::query("select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '1 minute')::text,true)")
        .execute(&mut *tx).await.unwrap();
    sqlx::query("update concert_relationship_pairs set low_blocks_high=false,revision=revision+1 where event_id=$1 and low_user=$2 and high_user=$3")
        .bind(event.uuid()).bind(low).bind(high).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(flags(&pool, event, a, b, request).await, [true; 6], "unblock restores all six");

    sqlx::query("update concert_event_admissions set state='SUSPENDED',restriction_revision=restriction_revision+1 where event_id=$1 and user_id=$2")
        .bind(event.uuid()).bind(b.uuid()).execute(&pool).await.unwrap();
    assert_eq!(flags(&pool, event, a, b, request).await, [false; 6], "committed suspension must remove all six");

    sqlx::query("update concert_event_admissions set state='ACTIVE',restriction_revision=restriction_revision+1 where event_id=$1 and user_id=$2")
        .bind(event.uuid()).bind(b.uuid()).execute(&pool).await.unwrap();
    assert_eq!(flags(&pool, event, a, b, request).await, [true; 6], "restored admission returns all six");
    let _ = pool.close().await;
}
