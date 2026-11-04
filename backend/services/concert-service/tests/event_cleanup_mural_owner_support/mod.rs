//! Explicit authority fixtures only. Real SQLx, retained lifecycle/cleanup engine,
//! actual mural producer and production pixel repository are exercised unchanged.
//! Authority/policy fixtures adapted from event_cleanup_support/mod.rs at base
//! 0bd35b223d2590fb2b343171d6d8ac30d37ff857; no synthetic mural owner is reused.
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::{
    commands::{CommandError, FingerprintKeys, SigningKey},
    event_cleanup::*,
    event_lifecycle::{Actor, CloseCommand},
};
use concert_domain::event_lifecycle::{Retention, Step};
use concert_service::{event_cleanup_live::*, event_lifecycle::*};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

pub async fn pool() -> PgPool {
    let socket = std::env::var("MCR_MURAL_OWNER_SOCKET").expect("owned runner required");
    let root = std::path::Path::new(&socket).parent().unwrap();
    assert!(root
        .to_str()
        .unwrap()
        .starts_with("/private/tmp/mcr-mural-owner-pg-"));
    assert_eq!(
        std::fs::read_to_string(root.join("owner")).unwrap(),
        std::env::var("MCR_MURAL_OWNER_TOKEN").unwrap()
    );
    let options = sqlx::postgres::PgConnectOptions::new()
        .host(&socket)
        .port(55447)
        .database("mcr_mural_owner_fixture")
        .username("mcr_mural_owner_fixture");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await
        .unwrap();
    let token: String = sqlx::query_scalar("select owner_token from mural_owner_fixture_marker")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(token, std::env::var("MCR_MURAL_OWNER_TOKEN").unwrap());
    pool
}

pub struct Authority(pub i64);
#[async_trait]
impl FencedLifecycleAuthority for Authority {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        q: AuthorityQuery,
        now: DateTime<Utc>,
    ) -> Result<AuthorityFence, CommandError> {
        let allowed: bool = sqlx::query_scalar("select allowed from mural_owner_fixture_authority where event_id=$1 and actor_id=$2 for share")
            .bind(q.actor.event).bind(q.actor.actor).fetch_optional(c).await.map_err(|_|CommandError::Unavailable)?.ok_or(CommandError::Denied)?;
        if !allowed {
            return Err(CommandError::Denied);
        }
        Ok(AuthorityFence {
            query: q,
            auth_revision: 1,
            role_revision: 1,
            policy_revision: 1,
            checked_at: now,
            expires_at: now + Duration::milliseconds(self.0),
        })
    }
    async fn retention_in(
        &self,
        _: &mut PgConnection,
        _: Actor,
        closed_at: DateTime<Utc>,
    ) -> Result<Retention, CommandError> {
        Ok(Retention {
            policy_revision: 1,
            closed_at,
            cleanup_after: closed_at + Duration::hours(1),
        })
    }
}
struct ReadOnly;
#[async_trait]
impl LifecycleOwners for ReadOnly {
    async fn apply_in(
        &self,
        c: &mut PgConnection,
        key: EffectKey,
        _: Retention,
    ) -> Result<Applied, CommandError> {
        concert_service::event_lifecycle::read_only::apply_read_only_in(c, key).await
    }
}
pub fn keys() -> FingerprintKeys {
    FingerprintKeys::new(
        SigningKey::new("mural_owner_fixture", &[0x47; 32]).unwrap(),
        vec![],
    )
    .unwrap()
}
pub async fn close(pool: &PgPool, actor: Actor) -> Uuid {
    PgEventLifecycle::new(pool.clone(), Authority(10000), ReadOnly)
        .command(
            actor,
            Uuid::new_v4(),
            CloseCommand::Start {
                expected_revision: 1,
                reason: "Mural owner isolated authority fixture".into(),
            },
            &keys(),
        )
        .await
        .unwrap()
        .reference
        .unwrap()
        .resource
}
pub async fn seed_open(pool: &PgPool, state: &str, revision: i64) -> Actor {
    let actor = Actor {
        actor: Uuid::new_v4(),
        event: Uuid::new_v4(),
    };
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Actual mural fixture','LIVE')")
        .bind(actor.event).bind(actor.event.to_string()).execute(pool).await.unwrap();
    sqlx::query("insert into mural_owner_fixture_authority values($1,$2,true)")
        .bind(actor.event)
        .bind(actor.actor)
        .execute(pool)
        .await
        .unwrap();
    let palette: Vec<String> = (0..16).map(|n| format!("#{n:06X}")).collect();
    let pixels: Vec<u8> = (0..16384).map(|n| (n % 16) as u8).collect();
    sqlx::query("insert into concert_mural_state(event_id,revision,snapshot_id,state,pixels,palette,cooldown_ms,reset_revision) values($1,$2,$3,$4,$5,$6,15000,$2)")
        .bind(actor.event).bind(revision).bind(Uuid::new_v4()).bind(state).bind(pixels).bind(palette).execute(pool).await.unwrap();
    checkpoint(pool, actor.event).await;
    sqlx::query(
        "insert into concert_mural_limits values($1,$2,clock_timestamp()+interval '1 hour')",
    )
    .bind(actor.event)
    .bind(Uuid::new_v4())
    .execute(pool)
    .await
    .unwrap();
    actor
}
pub async fn checkpoint(pool: &PgPool, event: Uuid) {
    sqlx::query("insert into concert_mural_snapshots select snapshot_id,event_id,revision,state,pixels,palette,snapshot_created_at from concert_mural_state where event_id=$1")
        .bind(event).execute(pool).await.unwrap();
}
pub async fn seed(pool: &PgPool, state: &str, revision: i64) -> (Actor, Uuid) {
    let actor = seed_open(pool, state, revision).await;
    let job = close(pool, actor).await;
    (actor, job)
}
pub fn runtime(pool: &PgPool, owner: Arc<dyn LocalOwner>) -> CleanupRuntime {
    CleanupRuntime::new(
        pool.clone(),
        Some(Arc::new(Authority(10000))),
        Owners::new(vec![owner]).unwrap(),
        2,
    )
    .unwrap()
}
pub async fn mural(pool: &PgPool, event: Uuid) -> serde_json::Value {
    sqlx::query_scalar("select to_jsonb(m) from concert_mural_state m where event_id=$1")
        .bind(event)
        .fetch_one(pool)
        .await
        .unwrap()
}
pub async fn receipt(pool: &PgPool, job: Uuid) -> Receipt {
    let row = sqlx::query("select * from concert_event_cleanup_batches where job_id=$1")
        .bind(job)
        .fetch_one(pool)
        .await
        .unwrap();
    Receipt {
        binding: row.get::<Vec<u8>, _>("binding").try_into().unwrap(),
        operation: row.get("operation_id"),
        id: row.get("receipt_id"),
        revision: row.get("owner_revision"),
        processed: row.get::<i16, _>("processed") as u16,
    }
}
pub async fn request(pool: &PgPool, actor: Actor, job: Uuid) -> BatchRequest {
    let row=sqlx::query("select retention_revision,closed_at,cleanup_after from concert_closure_jobs where job_id=$1").bind(job).fetch_one(pool).await.unwrap();
    BatchRequest::new(
        Scope {
            actor: actor.actor,
            event: actor.event,
            job,
            step: Step::FreezeMural,
            owner: Owner::Mural,
        },
        Retention {
            policy_revision: row.get("retention_revision"),
            closed_at: row.get("closed_at"),
            cleanup_after: row.get("cleanup_after"),
        },
        0,
        None,
        2,
    )
    .unwrap()
}
pub async fn fixture_deadline(c: &mut PgConnection) {
    // Test-only fixture, not production authority. The real engine sets this
    // deadline after check_in; direct supplied-connection tests emulate that step.
    sqlx::query("select set_config('mcr.lifecycle_authority_until',(clock_timestamp()+interval '10 seconds')::text,true)").execute(c).await.unwrap();
}
pub async fn batches(pool: &PgPool, job: Uuid) -> i64 {
    sqlx::query_scalar("select count(*) from concert_event_cleanup_batches where job_id=$1")
        .bind(job)
        .fetch_one(pool)
        .await
        .unwrap()
}
pub async fn parent_pending(pool: &PgPool, job: Uuid) {
    let progress: serde_json::Value =
        sqlx::query_scalar("select progress from concert_closure_jobs where job_id=$1")
            .bind(job)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(progress["steps"][1]["state"], "PENDING");
    let count: i64 = sqlx::query_scalar(
        "select count(*) from concert_closure_effects where job_id=$1 and step='FREEZE_MURAL'",
    )
    .bind(job)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}
