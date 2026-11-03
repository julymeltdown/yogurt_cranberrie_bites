//! Owned PostgreSQL lifetime tests. SQL-locked synthetic authority, NOT real Auth.
mod age_claim_reader_support;
#[path = "../src/adapters/out/age_provider/claim_reader.rs"]
mod claim_reader;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use claim_reader::*;
use concert_application::{
    age_policy::{ClaimQuery, ClaimReader, Context, PolicyError},
    Clock,
};
use concert_domain::{
    age_policy::{AgeBounds, AgeStatus},
    AuthUserId, EventId,
};
use sqlx::{PgConnection, PgPool};
use std::sync::{
    atomic::{AtomicI32, Ordering},
    Arc, Mutex,
};
use tokio::sync::Notify;
use uuid::Uuid;

struct TestClock(Mutex<Option<DateTime<Utc>>>);
impl Clock for TestClock {
    fn now(&self) -> DateTime<Utc> {
        self.0.lock().unwrap().unwrap_or_else(Utc::now)
    }
}
struct FixtureAuthority {
    pid: AtomicI32,
    misbind: bool,
}
#[async_trait]
impl ProjectionAuthority for FixtureAuthority {
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        q: ClaimQuery,
    ) -> Result<AuthorityDecision, PolicyError> {
        let row:Option<(bool,bool)> = sqlx::query_as("select allowed,provider from be26_reader_fixture_authority where actor=$1 and subject=$2 and event=$3 and session=$4 and request=$5 and deadline=$6 and requested_at=$7 for share")
            .bind(q.context.actor.uuid()).bind(q.subject.uuid()).bind(q.context.event.uuid()).bind(q.context.session).bind(q.context.request).bind(q.context.expires_at).bind(q.requested_at).fetch_optional(&mut *c).await.map_err(|_|PolicyError::Unavailable)?;
        let Some((true, verification_available)) = row else {
            return Err(PolicyError::Denied);
        };
        let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
            .fetch_one(c)
            .await
            .map_err(|_| PolicyError::Unavailable)?;
        self.pid.store(pid, Ordering::SeqCst);
        let mut bound = q;
        if self.misbind {
            bound.context.session = Uuid::new_v4();
        }
        Ok(AuthorityDecision {
            query: bound,
            verification_available,
        })
    }
}
struct Fixture {
    p: PgPool,
    q: ClaimQuery,
    authority: Arc<FixtureAuthority>,
    clock: Arc<TestClock>,
    reader: PgClaimProjection,
}
async fn setup() -> anyhow::Result<Fixture> {
    let p = age_claim_reader_support::owned_pool().await?;
    let q = ClaimQuery {
        context: Context {
            actor: AuthUserId::from_uuid(Uuid::new_v4()),
            event: EventId::from_uuid(Uuid::new_v4()),
            session: Uuid::new_v4(),
            request: Uuid::new_v4(),
            expires_at: Utc::now() + Duration::minutes(1),
        },
        subject: AuthUserId::from_uuid(Uuid::nil()),
        requested_at: Utc::now(),
    };
    let q = ClaimQuery {
        subject: q.context.actor,
        ..q
    };
    sqlx::query("create table if not exists be26_reader_fixture_authority(actor uuid,subject uuid,event uuid,session uuid,request uuid,deadline timestamptz,requested_at timestamptz,allowed boolean,provider boolean,primary key(actor,event,session,request))").execute(&p).await?;
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Owned reader fixture','OPEN')").bind(q.context.event.uuid()).bind(format!("reader-{}",q.context.event.uuid())).execute(&p).await?;
    sqlx::query("insert into be26_reader_fixture_authority values($1,$2,$3,$4,$5,$6,$7,true,true)")
        .bind(q.context.actor.uuid())
        .bind(q.subject.uuid())
        .bind(q.context.event.uuid())
        .bind(q.context.session)
        .bind(q.context.request)
        .bind(q.context.expires_at)
        .bind(q.requested_at)
        .execute(&p)
        .await?;
    let authority = Arc::new(FixtureAuthority {
        pid: AtomicI32::new(0),
        misbind: false,
    });
    let clock = Arc::new(TestClock(Mutex::new(None)));
    let reader = PgClaimProjection::new(p.clone(), authority.clone(), clock.clone());
    Ok(Fixture {
        p,
        q,
        authority,
        clock,
        reader,
    })
}
async fn add_claim(
    f: &Fixture,
    status: &str,
    verified: DateTime<Utc>,
    expires: DateTime<Utc>,
    age: Option<i16>,
    birthday: Option<chrono::NaiveDate>,
) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query("insert into concert_age_owners(user_id) values($1) on conflict do nothing")
        .bind(f.q.subject.uuid())
        .execute(&f.p)
        .await?;
    sqlx::query("insert into concert_age_verifications(verification_id,user_id,event_id,session_id,verifier_id,source_revision,state_digest,return_target,created_at,expires_at,state) select $1,$2,$3,$4,'fixture','fixture-v1',decode(repeat('62',32),'hex'),'AGE_PRIVACY',t,t+interval '10 minutes','PENDING' from(select clock_timestamp() t) n")
        .bind(id).bind(f.q.subject.uuid()).bind(f.q.context.event.uuid()).bind(f.q.context.session).execute(&f.p).await?;
    sqlx::query("insert into concert_age_claims(claim_id,user_id,event_id,verifier_id,source_revision,status,verified_at,expires_at,age_at_verification,next_birthday_at) values($1,$2,$3,'fixture','fixture-v1',$4,$5,$6,$7,$8)")
        .bind(id).bind(f.q.subject.uuid()).bind(f.q.context.event.uuid()).bind(status).bind(verified).bind(expires).bind(age).bind(birthday).execute(&f.p).await?;
    Ok(id)
}
async fn valid_claim(f: &Fixture) -> anyhow::Result<Uuid> {
    let at = Utc::now() - Duration::days(2);
    add_claim(
        f,
        "VERIFIED",
        at,
        at + Duration::days(90),
        Some(25),
        Some((at + Duration::hours(9)).date_naive() + Duration::days(30)),
    )
    .await
}
async fn status(f: &Fixture) -> Result<AgeStatus, PolicyError> {
    f.reader
        .project(f.q, |age| Box::pin(async move { Ok(age.status()) }))
        .await
}
async fn wait_lock(p: &PgPool, pid: i32) -> anyhow::Result<()> {
    tokio::time::timeout(std::time::Duration::from_secs(1),async {
        loop {
            let yes:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where pid=$1 and wait_event_type='Lock')").bind(pid).fetch_one(p).await?;
            if yes { return Ok::<(),sqlx::Error>(()); }
            tokio::task::yield_now().await;
        }
    }).await??;
    Ok(())
}
async fn pool_released(f: &Fixture) -> anyhow::Result<()> {
    tokio::time::timeout(std::time::Duration::from_secs(2),async {
        loop {
            let active:i64=sqlx::query_scalar("select count(*) from pg_stat_activity where pid=$1 and pid<>pg_backend_pid() and xact_start is not null").bind(f.authority.pid.load(Ordering::SeqCst)).fetch_one(&f.p).await?;
            if active==0 { return Ok::<(),sqlx::Error>(()); }
            tokio::task::yield_now().await;
        }
    }).await??;
    let mut tx = f.p.begin().await?;
    sqlx::query("select actor from be26_reader_fixture_authority where actor=$1 for update nowait")
        .bind(f.q.context.actor.uuid())
        .fetch_one(&mut *tx)
        .await?;
    tx.rollback().await?;
    Ok(())
}
#[tokio::test]
async fn legacy_trait_and_absent_authority_are_unavailable() -> anyhow::Result<()> {
    let f = setup().await?;
    assert!(matches!(
        f.reader.read(f.q).await,
        Err(PolicyError::Unavailable)
    ));
    let absent = PgClaimProjection::unavailable(f.p.clone(), f.clock.clone());
    assert!(matches!(
        absent
            .project::<(), _>(f.q, |_| Box::pin(async {
                panic!("unavailable authority projected")
            }))
            .await,
        Err(PolicyError::Unavailable)
    ));
    pool_released(&f).await?;
    Ok(())
}
#[tokio::test]
async fn exact_actor_subject_session_event_request_and_deadline_bindings() -> anyhow::Result<()> {
    let f = setup().await?;
    let mut wrongs = vec![];
    let mut q = f.q;
    q.context.actor = AuthUserId::from_uuid(Uuid::new_v4());
    wrongs.push(q);
    let mut q = f.q;
    q.subject = AuthUserId::from_uuid(Uuid::new_v4());
    wrongs.push(q);
    let mut q = f.q;
    q.context.session = Uuid::new_v4();
    wrongs.push(q);
    let mut q = f.q;
    q.context.event = EventId::from_uuid(Uuid::new_v4());
    wrongs.push(q);
    let mut q = f.q;
    q.context.request = Uuid::new_v4();
    wrongs.push(q);
    let mut q = f.q;
    q.context.expires_at += Duration::seconds(1);
    wrongs.push(q);
    let mut q = f.q;
    q.requested_at -= Duration::seconds(1);
    wrongs.push(q);
    for q in wrongs {
        assert!(matches!(
            f.reader.project(q, |_| Box::pin(async { Ok(()) })).await,
            Err(PolicyError::Denied)
        ));
    }
    let bad = PgClaimProjection::new(
        f.p.clone(),
        Arc::new(FixtureAuthority {
            pid: AtomicI32::new(0),
            misbind: true,
        }),
        f.clock.clone(),
    );
    assert!(matches!(
        bad.project(f.q, |_| Box::pin(async { Ok(()) })).await,
        Err(PolicyError::Denied)
    ));
    Ok(())
}
#[tokio::test]
async fn provider_availability_is_separate_from_persisted_claim_validity() -> anyhow::Result<()> {
    let f = setup().await?;
    sqlx::query("update be26_reader_fixture_authority set provider=false where actor=$1")
        .bind(f.q.context.actor.uuid())
        .execute(&f.p)
        .await?;
    assert_eq!(status(&f).await?, AgeStatus::ProviderUnavailable);
    valid_claim(&f).await?;
    let (status, available, permitted) = f
        .reader
        .project(f.q, |age| {
            Box::pin(async move {
                Ok((
                    age.status(),
                    age.verification_available(),
                    age.permits(AgeBounds::new(Some(20), Some(30)).unwrap(), false),
                ))
            })
        })
        .await?;
    assert_eq!(status, AgeStatus::Verified);
    assert!(!available);
    assert!(permitted);
    Ok(())
}
#[tokio::test]
async fn newest_expired_revoked_replaced_unsupported_never_falls_back() -> anyhow::Result<()> {
    for (stored, want) in [
        ("EXPIRED", AgeStatus::Expired),
        ("REVOKED", AgeStatus::Revoked),
        ("REPLACED", AgeStatus::Revoked),
        ("UNSUPPORTED", AgeStatus::Unsupported),
    ] {
        let f = setup().await?;
        valid_claim(&f).await?;
        let at = Utc::now() - Duration::days(1);
        let expiry = if stored == "EXPIRED" {
            Utc::now() - Duration::seconds(1)
        } else {
            at + Duration::days(90)
        };
        let (age, next) = if matches!(stored, "EXPIRED" | "UNSUPPORTED") {
            (None, None)
        } else {
            (
                Some(25),
                Some((at + Duration::hours(9)).date_naive() + Duration::days(30)),
            )
        };
        add_claim(&f, stored, at, expiry, age, next).await?;
        assert_eq!(status(&f).await?, want);
    }
    Ok(())
}
#[tokio::test]
async fn exact_seoul_boundary_rejects_projection_crossing_birthday() -> anyhow::Result<()> {
    let mut f = setup().await?;
    let today = (Utc::now() + Duration::hours(9)).date_naive();
    let boundary = (today.and_hms_opt(0, 0, 0).unwrap() - Duration::hours(9)).and_utc();
    f.q.requested_at = boundary - Duration::nanoseconds(1);
    sqlx::query("update be26_reader_fixture_authority set requested_at=$2 where actor=$1")
        .bind(f.q.context.actor.uuid())
        .bind(f.q.requested_at)
        .execute(&f.p)
        .await?;
    add_claim(
        &f,
        "VERIFIED",
        boundary - Duration::days(2),
        boundary + Duration::days(80),
        Some(100),
        Some(today),
    )
    .await?;
    *f.clock.0.lock().unwrap() = Some(f.q.requested_at);
    let clock = f.clock.clone();
    let result = f
        .reader
        .project(f.q, move |age| {
            Box::pin(async move {
                assert_eq!(age.status(), AgeStatus::Verified);
                assert!(age.permits(AgeBounds::new(Some(100), Some(101)).unwrap(), false));
                *clock.0.lock().unwrap() = Some(boundary);
                Ok(())
            })
        })
        .await;
    assert!(matches!(result, Err(PolicyError::Denied)));
    assert_eq!(status(&f).await?, AgeStatus::Unsupported);
    Ok(())
}
#[tokio::test]
async fn concurrent_replacement_waits_until_private_projection_finishes() -> anyhow::Result<()> {
    let f = setup().await?;
    let old = valid_claim(&f).await?;
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let reader = f.reader.clone();
    let q = f.q;
    let e = entered.clone();
    let r = release.clone();
    let read = tokio::spawn(async move {
        reader
            .project(q, move |age| {
                Box::pin(async move {
                    e.notify_one();
                    r.notified().await;
                    Ok(age.status())
                })
            })
            .await
    });
    entered.notified().await;
    let mut tx = f.p.begin().await?;
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *tx)
        .await?;
    let actor = f.q.subject.uuid();
    let writer = tokio::spawn(async move {
        sqlx::query("select user_id from concert_age_owners where user_id=$1 for update")
            .bind(actor)
            .fetch_one(&mut *tx)
            .await?;
        sqlx::query("update concert_age_claims set status='REPLACED' where claim_id=$1")
            .bind(old)
            .execute(&mut *tx)
            .await?;
        tx.commit().await
    });
    wait_lock(&f.p, pid).await?;
    assert!(!writer.is_finished());
    release.notify_one();
    assert_eq!(read.await??, AgeStatus::Verified);
    writer.await??;
    assert_eq!(status(&f).await?, AgeStatus::Revoked);
    pool_released(&f).await?;
    Ok(())
}
#[tokio::test]
async fn owner_revocation_is_serialized_and_next_projection_denied() -> anyhow::Result<()> {
    let f = setup().await?;
    valid_claim(&f).await?;
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let reader = f.reader.clone();
    let q = f.q;
    let e = entered.clone();
    let r = release.clone();
    let read = tokio::spawn(async move {
        reader
            .project(q, move |age| {
                Box::pin(async move {
                    e.notify_one();
                    r.notified().await;
                    Ok(age.status())
                })
            })
            .await
    });
    entered.notified().await;
    let mut tx = f.p.begin().await?;
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *tx)
        .await?;
    let actor = f.q.context.actor.uuid();
    let revoke = tokio::spawn(async move {
        sqlx::query("update be26_reader_fixture_authority set allowed=false where actor=$1")
            .bind(actor)
            .execute(&mut *tx)
            .await?;
        tx.commit().await
    });
    wait_lock(&f.p, pid).await?;
    release.notify_one();
    assert_eq!(read.await??, AgeStatus::Verified);
    revoke.await??;
    assert!(matches!(status(&f).await, Err(PolicyError::Denied)));
    pool_released(&f).await?;
    Ok(())
}
#[tokio::test]
async fn source_connection_loss_discards_projection_and_releases_locks() -> anyhow::Result<()> {
    let f = setup().await?;
    valid_claim(&f).await?;
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let reader = f.reader.clone();
    let q = f.q;
    let e = entered.clone();
    let r = release.clone();
    let read = tokio::spawn(async move {
        reader
            .project(q, move |age| {
                Box::pin(async move {
                    let output = age.status();
                    e.notify_one();
                    r.notified().await;
                    Ok(output)
                })
            })
            .await
    });
    entered.notified().await;
    let pid = f.authority.pid.load(Ordering::SeqCst);
    let killed: bool = sqlx::query_scalar("select pg_terminate_backend($1)")
        .bind(pid)
        .fetch_one(&f.p)
        .await?;
    assert!(killed);
    // Source locks are now gone, although a detached local snapshot would still exist.
    sqlx::query("update be26_reader_fixture_authority set allowed=false where actor=$1")
        .bind(f.q.context.actor.uuid())
        .execute(&f.p)
        .await?;
    release.notify_one();
    assert!(matches!(read.await?, Err(PolicyError::Unavailable)));
    pool_released(&f).await?;
    Ok(())
}
#[tokio::test]
async fn cancellation_and_projection_error_roll_back_and_release_pool() -> anyhow::Result<()> {
    let f = setup().await?;
    let entered = Arc::new(Notify::new());
    let e = entered.clone();
    let reader = f.reader.clone();
    let q = f.q;
    let task = tokio::spawn(async move {
        reader
            .project(q, move |_| {
                Box::pin(async move {
                    e.notify_one();
                    std::future::pending::<Result<(), PolicyError>>().await
                })
            })
            .await
    });
    entered.notified().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    pool_released(&f).await?;
    let count: i64 = sqlx::query_scalar("select count(*) from concert_age_owners where user_id=$1")
        .bind(f.q.subject.uuid())
        .fetch_one(&f.p)
        .await?;
    assert_eq!(count, 0);
    assert!(matches!(
        f.reader
            .project(f.q, |_| Box::pin(async {
                Err::<(), _>(PolicyError::Denied)
            }))
            .await,
        Err(PolicyError::Denied)
    ));
    pool_released(&f).await?;
    let count: i64 = sqlx::query_scalar("select count(*) from concert_age_owners where user_id=$1")
        .bind(f.q.subject.uuid())
        .fetch_one(&f.p)
        .await?;
    assert_eq!(count, 0);
    Ok(())
}
#[tokio::test]
async fn original_deadline_cancels_projection_without_result_or_leaked_transaction(
) -> anyhow::Result<()> {
    let mut f = setup().await?;
    f.q.context.expires_at = Utc::now() + Duration::seconds(2);
    sqlx::query("update be26_reader_fixture_authority set deadline=$2 where actor=$1")
        .bind(f.q.context.actor.uuid())
        .bind(f.q.context.expires_at)
        .execute(&f.p)
        .await?;
    let entered = Arc::new(Notify::new());
    let e = entered.clone();
    let reader = f.reader.clone();
    let q = f.q;
    let read = tokio::spawn(async move {
        reader
            .project(q, move |_| {
                Box::pin(async move {
                    e.notify_one();
                    std::future::pending::<Result<(), PolicyError>>().await
                })
            })
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(1), entered.notified()).await?;
    assert!(f.authority.pid.load(Ordering::SeqCst) > 0);
    // Prove the pending closure holds the actual authority row lock.
    let mut tx = f.p.begin().await?;
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *tx)
        .await?;
    let actor = f.q.context.actor.uuid();
    let writer = tokio::spawn(async move {
        sqlx::query("select actor from be26_reader_fixture_authority where actor=$1 for update")
            .bind(actor)
            .fetch_one(&mut *tx)
            .await?;
        tx.rollback().await
    });
    wait_lock(&f.p, pid).await?;
    assert!(matches!(read.await?, Err(PolicyError::Unavailable)));
    writer.await??;
    pool_released(&f).await?;
    Ok(())
}
struct CommitDelayAuthority {
    base: Arc<FixtureAuthority>,
    calls: std::sync::atomic::AtomicUsize,
}
#[async_trait]
impl ProjectionAuthority for CommitDelayAuthority {
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        q: ClaimQuery,
    ) -> Result<AuthorityDecision, PolicyError> {
        let decision = self.base.authorize_in(c, q).await?;
        if self.calls.fetch_add(1, Ordering::SeqCst) == 1 {
            sqlx::query("insert into be26_reader_commit_delay values($1)")
                .bind(q.context.request)
                .execute(c)
                .await
                .map_err(|_| PolicyError::Unavailable)?;
        }
        Ok(decision)
    }
}
#[tokio::test]
async fn claim_expiry_during_known_commit_discards_private_projection() -> anyhow::Result<()> {
    let f = setup().await?;
    // Test-only deferred trigger delays COMMIT after the final source-time check.
    sqlx::query("create table be26_reader_commit_delay(request uuid primary key)")
        .execute(&f.p)
        .await?;
    sqlx::query("create function be26_reader_delay_commit() returns trigger language plpgsql as $$ begin perform pg_sleep(0.4); return new; end $$").execute(&f.p).await?;
    sqlx::query("create constraint trigger be26_reader_delay after insert on be26_reader_commit_delay deferrable initially deferred for each row execute function be26_reader_delay_commit()").execute(&f.p).await?;
    let at = Utc::now() - Duration::days(2);
    add_claim(
        &f,
        "VERIFIED",
        at,
        Utc::now() + Duration::milliseconds(250),
        Some(25),
        Some((at + Duration::hours(9)).date_naive() + Duration::days(30)),
    )
    .await?;
    let authority = Arc::new(CommitDelayAuthority {
        base: f.authority.clone(),
        calls: std::sync::atomic::AtomicUsize::new(0),
    });
    let reader = PgClaimProjection::new(f.p.clone(), authority.clone(), f.clock.clone());
    let result = reader
        .project(f.q, |age| {
            Box::pin(async move {
                assert_eq!(age.status(), AgeStatus::Verified);
                Ok(true)
            })
        })
        .await;
    assert!(matches!(result, Err(PolicyError::Denied)));
    assert_eq!(authority.calls.load(Ordering::SeqCst), 2);
    let committed: i64 =
        sqlx::query_scalar("select count(*) from be26_reader_commit_delay where request=$1")
            .bind(f.q.context.request)
            .fetch_one(&f.p)
            .await?;
    assert_eq!(committed, 1); // Known COMMIT occurred; its late value was withheld.
    pool_released(&f).await?;
    Ok(())
}
