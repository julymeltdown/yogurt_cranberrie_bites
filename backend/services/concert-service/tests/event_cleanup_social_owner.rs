//! Real canonical Social rows + actual CleanupRuntime. Authority, retention and
//! preceding mural/snapshot effects are explicit test fixtures, not production.
pub use concert_service::event_cleanup_live;
#[path = "../src/event_cleanup_social_owner/mod.rs"]
mod event_cleanup_social_owner;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::{
    commands::{CommandError, FingerprintKeys, SigningKey},
    event_cleanup::*,
    event_lifecycle::{Actor, CloseCommand},
};
use concert_domain::event_lifecycle::{Retention, Step};
use concert_service::event_lifecycle::*;
use event_cleanup_live::*;
use event_cleanup_social_owner::SocialGrantsOwner;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

#[test]
fn social_owner_source_pin() {
    let mut h = Sha256::new();
    for source in [
        include_bytes!("../src/event_cleanup_social_owner/mod.rs").as_slice(),
        include_bytes!("event_cleanup_social_owner.rs").as_slice(),
    ] {
        h.update(source);
        h.update([0]);
    }
    assert_eq!(SocialGrantsOwner.owner(), Owner::SocialGrants);
    println!("SOCIAL_OWNER_SOURCE_PIN={:x}", h.finalize());
}

async fn pool() -> PgPool {
    let socket = std::env::var("MCR_SOCIAL_OWNER_SOCKET").expect("owned runner required");
    let root = std::path::Path::new(&socket).parent().unwrap();
    assert!(root
        .to_str()
        .unwrap()
        .starts_with("/private/tmp/mcr-social-owner-pg-"));
    let token = std::env::var("MCR_SOCIAL_OWNER_TOKEN").unwrap();
    assert_eq!(std::fs::read_to_string(root.join("owner")).unwrap(), token);
    let options = sqlx::postgres::PgConnectOptions::new()
        .host(&socket)
        .port(55448)
        .database("mcr_social_owner_fixture")
        .username("mcr_social_owner_fixture");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await
        .unwrap();
    let actual: String = sqlx::query_scalar("select owner_token from social_owner_fixture_marker")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(actual, token);
    pool
}

struct Authority {
    relationship: bool,
}
#[async_trait]
impl FencedLifecycleAuthority for Authority {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        q: AuthorityQuery,
        now: DateTime<Utc>,
    ) -> Result<AuthorityFence, CommandError> {
        let allowed:Option<bool>=sqlx::query_scalar("select allowed from social_owner_fixture_authority where event_id=$1 and actor_id=$2 for share")
            .bind(q.actor.event).bind(q.actor.actor).fetch_optional(&mut *c).await.map_err(|_|CommandError::Unavailable)?;
        if allowed != Some(true) {
            return Err(CommandError::Denied);
        }
        if self.relationship {
            // Explicit test-only relationship authority seam. Production leaf
            // neither sets this deadline nor interprets it as privileged proof.
            sqlx::query("select set_config('mcr.relationship_authority_until',$1,true)")
                .bind((now + Duration::seconds(20)).to_rfc3339())
                .execute(c)
                .await
                .map_err(|_| CommandError::Unavailable)?;
        }
        Ok(AuthorityFence {
            query: q,
            auth_revision: 1,
            role_revision: 1,
            policy_revision: 1,
            checked_at: now,
            expires_at: now + Duration::seconds(20),
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
struct Prerequisites;
#[async_trait]
impl LifecycleOwners for Prerequisites {
    async fn apply_in(
        &self,
        c: &mut PgConnection,
        key: EffectKey,
        _: Retention,
    ) -> Result<Applied, CommandError> {
        if key.step == Step::ReadOnly {
            return read_only::apply_read_only_in(c, key).await;
        }
        // Fixture ONLY: these preceding owners are outside this leaf's claim.
        if !matches!(key.step, Step::FreezeMural | Step::Snapshot) {
            return Err(CommandError::Unavailable);
        }
        Ok(Applied {
            key,
            receipt: key.operation_id(),
            revision: 1,
        })
    }
}
fn runtime(
    pool: &PgPool,
    owner: Arc<dyn LocalOwner>,
    limit: u16,
    relationship: bool,
) -> CleanupRuntime {
    CleanupRuntime::new(
        pool.clone(),
        Some(Arc::new(Authority { relationship })),
        Owners::new(vec![owner]).unwrap(),
        limit,
    )
    .unwrap()
}
async fn fences(c: &mut PgConnection) {
    sqlx::query("select set_config('mcr.lifecycle_authority_until',(clock_timestamp()+interval '30 seconds')::text,true),set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '30 seconds')::text,true)")
        .execute(c).await.unwrap();
}

async fn seed(pool: &PgPool, count: usize) -> (Actor, Uuid) {
    let actor = Actor {
        actor: Uuid::new_v4(),
        event: Uuid::new_v4(),
    };
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Social owner isolated fixture','LIVE')")
        .bind(actor.event).bind(actor.event.to_string()).execute(pool).await.unwrap();
    sqlx::query("insert into social_owner_fixture_authority values($1,$2,true)")
        .bind(actor.event)
        .bind(actor.actor)
        .execute(pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    fences(&mut tx).await;
    for n in 0..count {
        let req = Uuid::from_u128(n as u128 + 1);
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        for who in [a, b] {
            sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)")
                .bind(actor.event).bind(who).execute(&mut *tx).await.unwrap();
            for platform in ["INSTAGRAM", "X"] {
                sqlx::query("insert into concert_social_handles(event_id,user_id,platform,key_id,nonce,ciphertext) values($1,$2,$3,'fixture',decode(repeat('01',12),'hex'),decode(repeat('02',20),'hex'))")
                    .bind(actor.event).bind(who).bind(platform).execute(&mut *tx).await.unwrap();
            }
        }
        sqlx::query("insert into concert_social_requests(event_id,request_id,requester_id,recipient_id,intro_id,intro_revision,authority_revision,relationship_revision,state,expires_at) values($1,$2,$3,$4,$5,1,1,1,'PENDING',clock_timestamp()+interval '1 hour')")
            .bind(actor.event).bind(req).bind(a).bind(b).bind(Uuid::new_v4()).execute(&mut *tx).await.unwrap();
        for platform in ["INSTAGRAM", "X"] {
            sqlx::query("insert into concert_social_bindings(event_id,request_id,platform,requester_version,recipient_version,requester_consents,approved) values($1,$2,$3,1,1,true,true)")
                .bind(actor.event).bind(req).bind(platform).execute(&mut *tx).await.unwrap();
            for (owner, viewer) in [(a, b), (b, a)] {
                sqlx::query("insert into concert_social_grants(event_id,request_id,owner_id,viewer_id,platform,handle_version,peer_handle_version,state,expires_at) select event_id,request_id,$3,$4,$5,1,1,'ACTIVE',expires_at from concert_social_requests where event_id=$1 and request_id=$2")
                    .bind(actor.event).bind(req).bind(owner).bind(viewer).bind(platform).execute(&mut *tx).await.unwrap();
            }
        }
        sqlx::query("update concert_social_requests set state='APPROVED',revision=revision+1 where event_id=$1 and request_id=$2")
            .bind(actor.event).bind(req).execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap();
    let keys = FingerprintKeys::new(
        SigningKey::new("social_owner_fixture", &[0x48; 32]).unwrap(),
        vec![],
    )
    .unwrap();
    let driver = PgEventLifecycle::new(
        pool.clone(),
        Authority { relationship: true },
        Prerequisites,
    );
    let job = driver
        .command(
            actor,
            Uuid::new_v4(),
            CloseCommand::Start {
                expected_revision: 1,
                reason: "Social owner isolated fixture".into(),
            },
            &keys,
        )
        .await
        .unwrap()
        .reference
        .unwrap()
        .resource;
    assert_eq!(
        driver.run_next(actor, job).await.unwrap().next(),
        Some(Step::Snapshot)
    );
    assert_eq!(
        driver.run_next(actor, job).await.unwrap().next(),
        Some(Step::ExpireGrants)
    );
    (actor, job)
}
async fn count(pool: &PgPool, job: Uuid) -> i64 {
    sqlx::query_scalar("select count(*) from concert_event_cleanup_batches where job_id=$1")
        .bind(job)
        .fetch_one(pool)
        .await
        .unwrap()
}
async fn live_count(pool: &PgPool, event: Uuid) -> i64 {
    sqlx::query_scalar("select (select count(*) from concert_social_grants where event_id=$1 and state='ACTIVE')+(select count(*) from concert_social_requests where event_id=$1 and state in ('PENDING','APPROVED'))")
        .bind(event).fetch_one(pool).await.unwrap()
}
async fn private_rows(pool: &PgPool, event: Uuid) -> serde_json::Value {
    sqlx::query_scalar("select jsonb_build_object('handles',(select jsonb_agg(to_jsonb(h) order by user_id,platform) from concert_social_handles h where event_id=$1),'bindings',(select jsonb_agg(to_jsonb(b) order by request_id,platform) from concert_social_bindings b where event_id=$1))")
        .bind(event).fetch_one(pool).await.unwrap()
}
async fn current_request(
    c: &mut PgConnection,
    actor: Actor,
    job: Uuid,
    limit: u16,
) -> BatchRequest {
    let row=sqlx::query("select j.retention_revision,j.closed_at,j.cleanup_after,h.ordinal,h.cursor from concert_closure_jobs j join concert_event_cleanup_progress h using(job_id) where j.job_id=$1")
        .bind(job).fetch_one(c).await.unwrap();
    BatchRequest::new(
        Scope {
            actor: actor.actor,
            event: actor.event,
            job,
            step: Step::ExpireGrants,
            owner: Owner::SocialGrants,
        },
        Retention {
            policy_revision: row.get("retention_revision"),
            closed_at: row.get("closed_at"),
            cleanup_after: row.get("cleanup_after"),
        },
        row.get::<i64, _>("ordinal") as u64,
        row.get::<Option<Vec<u8>>, _>("cursor")
            .map(|v| Cursor::new(v).unwrap()),
        limit,
    )
    .unwrap()
}

#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn social_owner_prepare() {
    let pool = pool().await;
    let migrator = sqlx::migrate::Migrator::new(std::path::PathBuf::from(
        std::env::var("MCR_SOCIAL_OWNER_MIGRATIONS").unwrap(),
    ))
    .await
    .unwrap();
    migrator.run(&pool).await.unwrap();
    migration_runtime::verify_receipts(&pool, &migrator)
        .await
        .unwrap();
    let before: serde_json::Value = sqlx::query_scalar(
        "select jsonb_agg(to_jsonb(m) order by version) from _sqlx_migrations m",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    migrator.run(&pool).await.unwrap();
    let after: serde_json::Value = sqlx::query_scalar(
        "select jsonb_agg(to_jsonb(m) order by version) from _sqlx_migrations m",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, after);
    sqlx::raw_sql("create table social_owner_fixture_authority(event_id uuid,actor_id uuid,allowed boolean,primary key(event_id,actor_id)); create table social_owner_fixture_restart(actor_id uuid,event_id uuid,job_id uuid,first_operation uuid);")
        .execute(&pool).await.unwrap();
    println!("GENUINE_SQLX_MIGRATIONS={}", migrator.iter().count());
}

#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn social_owner_bounded_pages_preserve_consent_handles_and_exact_replay() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, 3).await;
    let private = private_rows(&pool, actor.event).await;
    let (other, _) = seed(&pool, 1).await;
    let run = runtime(&pool, Arc::new(SocialGrantsOwner), 2, true);
    let mut previous = 0;
    loop {
        let p = run
            .run_owner(actor, job, Owner::SocialGrants)
            .await
            .unwrap();
        assert!((1..=2).contains(&(p.processed - previous)));
        previous = p.processed;
        assert_eq!(private_rows(&pool, actor.event).await, private);
        assert_eq!(live_count(&pool, other.event).await, 5);
        if p.state == State::Complete {
            break;
        }
        assert_eq!(p.state, State::Pending);
        assert!(!run.ready(actor, job).await.unwrap());
    }
    assert_eq!(previous, 15);
    assert_eq!(count(&pool, job).await, 8);
    assert_eq!(live_count(&pool, actor.event).await, 0);
    assert!(run.ready(actor, job).await.unwrap());
    let before:serde_json::Value=sqlx::query_scalar("select jsonb_agg(to_jsonb(b) order by ordinal) from concert_event_cleanup_batches b where job_id=$1").bind(job).fetch_one(&pool).await.unwrap();
    assert_eq!(
        run.run_owner(actor, job, Owner::SocialGrants)
            .await
            .unwrap()
            .processed,
        15
    );
    let after:serde_json::Value=sqlx::query_scalar("select jsonb_agg(to_jsonb(b) order by ordinal) from concert_event_cleanup_batches b where job_id=$1").bind(job).fetch_one(&pool).await.unwrap();
    assert_eq!(before, after);
    let parent: serde_json::Value =
        sqlx::query_scalar("select progress from concert_closure_jobs where job_id=$1")
            .bind(job)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(parent["steps"][3]["state"], "PENDING");
    let signals: i64 =
        sqlx::query_scalar("select count(*) from concert_social_policy_signals where event_id=$1")
            .bind(actor.event)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        signals, 0,
        "Local revocation is not complete private withdrawal"
    );
}

#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn social_owner_empty_and_terminal_rows_are_checked_without_revision_changes() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, 0).await;
    let run = runtime(&pool, Arc::new(SocialGrantsOwner), 1, true);
    let p = run
        .run_owner(actor, job, Owner::SocialGrants)
        .await
        .unwrap();
    assert_eq!(p.state, State::Complete);
    assert_eq!(p.processed, 0);
    assert!(run.ready(actor, job).await.unwrap());
    let (actor, job) = seed(&pool, 1).await;
    let mut tx = pool.begin().await.unwrap();
    fences(&mut tx).await;
    sqlx::query(
        "update concert_social_grants set state='EXPIRED',revision=revision+1 where event_id=$1",
    )
    .bind(actor.event)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "update concert_social_requests set state='EXPIRED',revision=revision+1 where event_id=$1",
    )
    .bind(actor.event)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let run = runtime(&pool, Arc::new(SocialGrantsOwner), 100, true);
    assert_eq!(
        run.run_owner(actor, job, Owner::SocialGrants)
            .await
            .unwrap()
            .processed,
        5
    );
    let revisions: Vec<i64> =
        sqlx::query_scalar("select revision from concert_social_grants where event_id=$1")
            .bind(actor.event)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(revisions, vec![2; 4]);
}

struct RejectProof;
#[async_trait]
impl LocalOwner for RejectProof {
    fn owner(&self) -> Owner {
        Owner::SocialGrants
    }
    async fn apply_in(
        &self,
        c: &mut PgConnection,
        r: &BatchRequest,
    ) -> Result<Outcome, CommandError> {
        SocialGrantsOwner.apply_in(c, r).await
    }
    async fn verify_in(
        &self,
        _: &mut PgConnection,
        _: &BatchRequest,
        _: Receipt,
    ) -> Result<(), CommandError> {
        Err(CommandError::InvalidState)
    }
}
#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn social_owner_verification_failure_rolls_back_effect_and_cursor_then_resumes_same_key() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, 1).await;
    let p = runtime(&pool, Arc::new(RejectProof), 2, true)
        .run_owner(actor, job, Owner::SocialGrants)
        .await
        .unwrap();
    assert_eq!(p.state, State::PartialFailure);
    assert_eq!(p.processed, 0);
    assert_eq!(count(&pool, job).await, 0);
    assert_eq!(live_count(&pool, actor.event).await, 5);
    let mut c = pool.acquire().await.unwrap();
    let request = current_request(&mut c, actor, job, 2).await;
    let operation = request.operation();
    drop(c);
    let p = runtime(&pool, Arc::new(SocialGrantsOwner), 2, true)
        .run_owner(actor, job, Owner::SocialGrants)
        .await
        .unwrap();
    assert_eq!(p.processed, 2);
    let stored: Uuid = sqlx::query_scalar(
        "select operation_id from concert_event_cleanup_batches where job_id=$1 and ordinal=0",
    )
    .bind(job)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, operation);
}

#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn social_owner_missing_authority_deadline_or_transaction_never_advances() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, 1).await;
    let p = runtime(&pool, Arc::new(SocialGrantsOwner), 2, false)
        .run_owner(actor, job, Owner::SocialGrants)
        .await
        .unwrap();
    assert_eq!(p.state, State::PartialFailure);
    assert_eq!(count(&pool, job).await, 0);
    let mut c = pool.acquire().await.unwrap();
    let r = current_request(&mut c, actor, job, 2).await;
    assert!(SocialGrantsOwner.apply_in(&mut c, &r).await.is_err());
    drop(c);
    sqlx::query("update social_owner_fixture_authority set allowed=false where event_id=$1")
        .bind(actor.event)
        .execute(&pool)
        .await
        .unwrap();
    assert!(runtime(&pool, Arc::new(SocialGrantsOwner), 2, true)
        .run_owner(actor, job, Owner::SocialGrants)
        .await
        .is_err());
    assert_eq!(live_count(&pool, actor.event).await, 5);
}

#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn social_owner_exact_receipt_and_scope_reject_forgery_before_append() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, 1).await;
    let missing = CleanupRuntime::new(
        pool.clone(),
        Some(Arc::new(Authority { relationship: true })),
        Owners::default(),
        100,
    )
    .unwrap();
    assert_eq!(
        missing
            .run_owner(actor, job, Owner::SocialGrants)
            .await
            .unwrap()
            .state,
        State::Pending
    );
    let mut tx = pool.begin().await.unwrap();
    fences(&mut tx).await;
    let r = current_request(&mut tx, actor, job, 100).await;
    let receipt = match SocialGrantsOwner.apply_in(&mut tx, &r).await.unwrap() {
        Outcome::Converged { receipt } => receipt,
        _ => panic!("one page"),
    };
    assert_eq!(count(&pool, job).await, 0, "verify precedes engine append");
    for forged in [
        Receipt {
            id: Uuid::new_v4(),
            ..receipt
        },
        Receipt {
            processed: 0,
            ..receipt
        },
        Receipt {
            binding: [0; 32],
            ..receipt
        },
        Receipt {
            operation: Uuid::new_v4(),
            ..receipt
        },
        Receipt {
            revision: 2,
            ..receipt
        },
    ] {
        assert!(SocialGrantsOwner
            .verify_in(&mut tx, &r, forged)
            .await
            .is_err());
    }
    let bad = BatchRequest::new(
        Scope {
            step: Step::FreezeMural,
            owner: Owner::Mural,
            ..r.scope()
        },
        r.retention(),
        0,
        None,
        100,
    )
    .unwrap();
    assert!(SocialGrantsOwner.apply_in(&mut tx, &bad).await.is_err());
    let changed = BatchRequest::new(
        r.scope(),
        Retention {
            policy_revision: 2,
            ..r.retention()
        },
        0,
        None,
        100,
    )
    .unwrap();
    assert!(SocialGrantsOwner.apply_in(&mut tx, &changed).await.is_err());
    tx.rollback().await.unwrap();
    assert_eq!(live_count(&pool, actor.event).await, 5);
}

#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn social_owner_concurrent_workers_share_one_cursor() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, 1).await;
    let run = Arc::new(runtime(&pool, Arc::new(SocialGrantsOwner), 2, true));
    let (a, b) = tokio::join!(
        run.run_owner(actor, job, Owner::SocialGrants),
        run.run_owner(actor, job, Owner::SocialGrants)
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.processed + b.processed, 6);
    assert_eq!(count(&pool, job).await, 2);
    assert_eq!(
        run.run_owner(actor, job, Owner::SocialGrants)
            .await
            .unwrap()
            .processed,
        5
    );
}

#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn social_owner_expired_commit_fence_rolls_back_verified_native_effects() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, 1).await;
    let missing = CleanupRuntime::new(
        pool.clone(),
        Some(Arc::new(Authority { relationship: true })),
        Owners::default(),
        100,
    )
    .unwrap();
    missing
        .run_owner(actor, job, Owner::SocialGrants)
        .await
        .unwrap();
    for setting in [
        "mcr.lifecycle_authority_until",
        "mcr.relationship_authority_until",
    ] {
        let mut tx = pool.begin().await.unwrap();
        fences(&mut tx).await;
        let request = current_request(&mut tx, actor, job, 100).await;
        let receipt = match SocialGrantsOwner.apply_in(&mut tx, &request).await.unwrap() {
            Outcome::Converged { receipt } => receipt,
            _ => panic!("single page"),
        };
        SocialGrantsOwner
            .verify_in(&mut tx, &request, receipt)
            .await
            .unwrap();
        sqlx::query(
            "select set_config($1,(clock_timestamp()+interval '10 milliseconds')::text,true)",
        )
        .bind(setting)
        .execute(&mut *tx)
        .await
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        assert!(SocialGrantsOwner
            .verify_in(&mut tx, &request, receipt)
            .await
            .is_err());
        // Relationship effects have a native deferred guard. A lifecycle-only
        // deadline is enforced by the engine's progress append/commit guard.
        if setting == "mcr.relationship_authority_until" {
            assert!(tx.commit().await.is_err());
        } else {
            tx.rollback().await.unwrap();
        }
        assert_eq!(live_count(&pool, actor.event).await, 5);
        assert_eq!(count(&pool, job).await, 0);
    }
}

#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn social_owner_changed_terminal_source_and_malformed_cursor_fail_closed() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, 1).await;
    let run = runtime(&pool, Arc::new(SocialGrantsOwner), 100, true);
    assert_eq!(
        run.run_owner(actor, job, Owner::SocialGrants)
            .await
            .unwrap()
            .state,
        State::Complete
    );
    let mut tx = pool.begin().await.unwrap();
    fences(&mut tx).await;
    let request = current_request(&mut tx, actor, job, 100).await;
    let malformed = BatchRequest::new(
        request.scope(),
        request.retention(),
        1,
        Some(Cursor::new(vec![1, 2, 3]).unwrap()),
        100,
    )
    .unwrap();
    assert!(SocialGrantsOwner
        .apply_in(&mut tx, &malformed)
        .await
        .is_err());
    // Deliberate trusted-database corruption fixture: remove one terminal
    // postimage, without disabling SQL guards or modifying immutable evidence.
    sqlx::query("delete from concert_social_grants where ctid in (select ctid from concert_social_grants where event_id=$1 limit 1)")
        .bind(actor.event).execute(&mut *tx).await.unwrap();
    let stored = sqlx::query("select * from concert_event_cleanup_batches where job_id=$1")
        .bind(job)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let receipt = Receipt {
        binding: stored.get::<Vec<u8>, _>("binding").try_into().unwrap(),
        operation: stored.get("operation_id"),
        id: stored.get("receipt_id"),
        revision: stored.get("owner_revision"),
        processed: stored.get::<i16, _>("processed") as u16,
    };
    assert!(SocialGrantsOwner
        .verify_in(&mut tx, &request, receipt)
        .await
        .is_err());
    tx.rollback().await.unwrap();
    assert!(run.ready(actor, job).await.unwrap());
}

#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn social_owner_restart_seed() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, 2).await;
    let p = runtime(&pool, Arc::new(SocialGrantsOwner), 2, true)
        .run_owner(actor, job, Owner::SocialGrants)
        .await
        .unwrap();
    assert_eq!(p.processed, 2);
    assert_eq!(p.state, State::Pending);
    let operation: Uuid = sqlx::query_scalar(
        "select operation_id from concert_event_cleanup_batches where job_id=$1 and ordinal=0",
    )
    .bind(job)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("insert into social_owner_fixture_restart values($1,$2,$3,$4)")
        .bind(actor.actor)
        .bind(actor.event)
        .bind(job)
        .bind(operation)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
}
#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn social_owner_restart_resume() {
    let pool = pool().await;
    let row = sqlx::query("select * from social_owner_fixture_restart")
        .fetch_one(&pool)
        .await
        .unwrap();
    let actor = Actor {
        actor: row.get("actor_id"),
        event: row.get("event_id"),
    };
    let job = row.get("job_id");
    let run = runtime(&pool, Arc::new(SocialGrantsOwner), 2, true);
    loop {
        let p = run
            .run_owner(actor, job, Owner::SocialGrants)
            .await
            .unwrap();
        if p.state == State::Complete {
            assert_eq!(p.processed, 10);
            break;
        }
        assert_eq!(p.state, State::Pending);
    }
    let first: Uuid = sqlx::query_scalar(
        "select operation_id from concert_event_cleanup_batches where job_id=$1 and ordinal=0",
    )
    .bind(job)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(first, row.get::<Uuid, _>("first_operation"));
    assert_eq!(count(&pool, job).await, 5);
    assert!(run.ready(actor, job).await.unwrap());
    assert_eq!(live_count(&pool, actor.event).await, 0);
    let prepared: i64 = sqlx::query_scalar("select count(*) from pg_prepared_xacts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(prepared, 0);
}
