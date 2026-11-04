use super::{capture, environment, fixture::Fixture};
use concert_service::adapters::out::relationship_authority::guarded::{
    ChildQuery, ChildRecoveryReference,
};
use sqlx::PgPool;
use std::time::{Duration, Instant};
use uuid::Uuid;

async fn pending(pool: &PgPool) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "select count(*) from pg_prepared_xacts where database=current_database()",
    )
    .fetch_one(pool)
    .await?)
}
async fn wait_writer(pool: &PgPool) -> anyhow::Result<()> {
    let stop = Instant::now() + Duration::from_millis(750);
    loop {
        let blocked:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and 0=any(pg_blocking_pids(pid)))").fetch_one(pool).await?;
        if blocked {
            return Ok(());
        }
        anyhow::ensure!(
            Instant::now() < stop,
            "Actual writer never blocked on prepared owner exclusion"
        );
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}
#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS canonical controller"]
async fn both_owners_exclude_writers_through_actual_scope_settlement() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let (actor, family, subjects) = environment::identities(&f.auth, 1).await?;
    let scope = f
        .fence
        .acquire(capture::binding(actor, family).await?)
        .await?;
    f.bind_delivery(&scope).await?;
    assert_eq!((pending(&f.auth).await?, pending(&f.friend).await?), (1, 1));
    let auth = f.auth.clone();
    let auth_writer = tokio::spawn(async move {
        sqlx::query("update users set password_hash='canonical-waiting-writer' where id=$1")
            .bind(actor)
            .execute(&auth)
            .await
    });
    let friend = f.friend.clone();
    let subject = subjects[0];
    let friend_writer = tokio::spawn(async move {
        sqlx::query("insert into user_blocks(blocker_id,blocked_id,created_at) values($1,$2,clock_timestamp())").bind(actor).bind(subject).execute(&friend).await
    });
    wait_writer(&f.auth).await?;
    wait_writer(&f.friend).await?;
    // Enrollment behind waiting invalidators must read the retained owner view.
    let admitted = scope.public_profile_subject(subject).await?;
    assert!(admitted.security_revisions()?.0 > 0);
    scope.run(async { Ok(()) }).await?;
    assert!(!auth_writer.is_finished() && !friend_writer.is_finished());
    assert!(
        scope.finish_response().await.is_err(),
        "Unserialized scope cannot finish"
    );
    scope.mark_serialized().await?;
    assert_eq!(
        (pending(&f.auth).await?, pending(&f.friend).await?),
        (1, 1),
        "Concert serialization cannot release owners"
    );
    scope.finish_response().await?;
    tokio::time::timeout(Duration::from_secs(1), auth_writer).await???;
    tokio::time::timeout(Duration::from_secs(1), friend_writer).await???;
    assert_eq!((pending(&f.auth).await?, pending(&f.friend).await?), (0, 0));
    assert!(admitted.check().await.is_err());
    f.assert_no_recovery_calls();
    println!("real Auth/Friend writer exclusion, retained enrollment, serialization-only hold and actual root/owner RPC settlement PASS; direct core terminal fixture, no HTTP claim");
    Ok(())
}

#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS canonical controller"]
async fn full_page_and_callback_drain_obey_original_scope() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let (actor, family, subjects) = environment::identities(&f.auth, 100).await?;
    let scope = f
        .fence
        .acquire(capture::binding(actor, family).await?)
        .await?;
    f.bind_delivery(&scope).await?;
    let actor_subject = scope.public_profile_subject(actor).await?;
    for subject in subjects {
        let admitted = scope.public_profile_subject(subject).await?;
        let generations = admitted.security_revisions()?;
        assert!(generations.0 > 0 && generations.1 > 0);
    }
    scope.binding().check()?;
    assert_eq!(
        scope
            .public_profile_subject(Uuid::new_v4())
            .await
            .err()
            .unwrap()
            .code(),
        tonic::Code::ResourceExhausted
    );
    let view = scope.root_read_view();
    let permit = f.registry.lookup_root_read(&view)?;
    let query = ChildQuery::from_subject(&actor_subject);
    let reference = ChildRecoveryReference::ProfilePublic {
        child_id: query.profile_child_id(Uuid::new_v4())?,
        query,
        completion_secret: Uuid::new_v4().simple().to_string(),
    };
    scope.mark_serialized().await?;
    let completing = scope.clone();
    let done = tokio::spawn(async move { completing.finish_response().await });
    let stop = Instant::now() + Duration::from_millis(500);
    while f.registry.lookup_root_read(&view).is_ok() {
        anyhow::ensure!(Instant::now() < stop, "Actual root was never sealed");
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    assert!(
        !done.is_finished(),
        "Existing callback must drain before completion"
    );
    // This is the lock inversion regression: an admitted callback must acquire
    // admission, observe sealed state and exit while the terminal task drains it.
    let denied = tokio::time::timeout(
        Duration::from_millis(200),
        actor_subject.register_child_intent(&reference),
    )
    .await?;
    assert_eq!(denied.unwrap_err().code(), tonic::Code::PermissionDenied);
    drop(permit);
    tokio::time::timeout(Duration::from_secs(2), done).await???;
    assert_eq!((pending(&f.auth).await?, pending(&f.friend).await?), (0, 0));
    f.assert_no_recovery_calls();
    println!("full100subjects plusactor under originalscope, overflowdenial and actual callback permit drain without serial inversion PASS; no child projection/HTTP/restart claim");
    Ok(())
}

#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS canonical controller"]
async fn registered_actual_transaction_quarantines_until_native_abort() -> anyhow::Result<()> {
    let f = Fixture::with_native_abort().await?;
    let (actor, family, _) = environment::identities(&f.auth, 0).await?;
    let scope = f
        .fence
        .acquire(capture::binding(actor, family).await?)
        .await?;
    f.bind_delivery(&scope).await?;
    let mut actual = f.concert.begin().await?;
    let reference = Uuid::new_v4().as_bytes().to_vec();
    scope
        .register_transaction_in(&mut actual, scope.binding().digest(), reference.clone())
        .await?;
    scope
        .register_transaction_in(&mut actual, scope.binding().digest(), reference)
        .await?;
    let producer = scope.producer_scope();
    let registered:i64=sqlx::query_scalar("select count(*) from concert_product_owner_transactions where instance_id=$1 and scope_id=$2")
        .bind(Uuid::parse_str(&producer.instance_id)?).bind(Uuid::parse_str(&producer.scope_id)?).fetch_one(&f.concert).await?;
    assert_eq!(
        registered, 1,
        "Repeated same native identity has one durable registration"
    );
    scope.mark_serialized().await?;
    assert!(
        scope.finish_response().await.is_err(),
        "Active registered producer transaction cannot certify completion"
    );
    assert_eq!((pending(&f.auth).await?, pending(&f.friend).await?), (1, 1));
    actual.rollback().await?;
    scope.finish_response().await?;
    assert_eq!((pending(&f.auth).await?, pending(&f.friend).await?), (0, 0));
    f.assert_no_recovery_calls();
    println!("same actual TX repeated Track admission, real in-progress quarantine, actual native rollback proof and idempotent completion retry PASS; no committed receipt/HTTP/restart claim");
    Ok(())
}

#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS canonical controller"]
async fn cancelled_unexposed_acquisition_recovers_from_actual_root_proof() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let (actor, family, _) = environment::identities(&f.auth, 0).await?;
    // This newly owned listener never accepts or provides a grant. It stalls the
    // negative Friend handshake after the real Auth service prepared its guard.
    let unavailable = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let fence = f.interrupted_fence(&format!("https://{}", unavailable.local_addr()?))?;
    let binding = capture::binding(actor, family).await?;
    let stop = binding.deadline().stop();
    let task = tokio::spawn(async move { fence.acquire(binding).await });
    let observation = Instant::now() + Duration::from_millis(450);
    loop {
        let active: i64 = sqlx::query_scalar(
            "select count(*) from concert_owner_guard_leases where state='ACTIVE'",
        )
        .fetch_one(&f.auth)
        .await?;
        if pending(&f.auth).await? == 1 && active == 1 {
            break;
        }
        anyhow::ensure!(
            !task.is_finished() && Instant::now() < observation,
            "Real Auth acquisition was not observed before cancellation"
        );
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    task.abort();
    assert!(task.await.err().is_some_and(|e| e.is_cancelled()));
    drop(unavailable);
    let cleanup_stop = Instant::now() + Duration::from_secs(2);
    while !f.no_unfinished_root().await? {
        anyhow::ensure!(
            Instant::now() < cleanup_stop,
            "Unexposed cancellation never persisted actual no-root-work proof"
        );
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    assert_eq!(
        pending(&f.auth).await?,
        1,
        "Cancellation cannot release the actual owner guard"
    );
    tokio::time::sleep_until(stop + Duration::from_millis(20)).await;
    assert_eq!(
        pending(&f.auth).await?,
        1,
        "Expiry alone cannot release prepared exclusion"
    );
    assert_eq!(
        f.recover_auth_fresh().await?,
        1,
        "Fresh owner instance must verify actual root proof before recovery"
    );
    assert_eq!((pending(&f.auth).await?, pending(&f.friend).await?), (0, 0));
    f.assert_no_recovery_calls();
    println!("actual partial-owner acquisition cancellation, durable unexposed root closure, no expiry release and fresh owner-object authenticated recovery PASS; no physical process/PG restart claim");
    Ok(())
}
