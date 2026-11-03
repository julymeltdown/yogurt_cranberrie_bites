//! Configured Gateway Profile client -> generated Profile server with the production
//! caller/actor/deadline guard -> real AppState -> minimal SQL owner on owned native PG16.
//! No controlled owner or hand-written reply is used. Transport is plaintext loopback; this
//! is not HTTP, TLS, Concert admission, event eligibility or browser acceptance.
//! The native controller runs this binary with one test thread because the Gateway Profile
//! edge limiter is process-global.
#[path = "support/concert_profile_owner_fixture.rs"]
mod fixture;
use api_gateway::adapters::out::profile_client::concert_identity::ConcertProfileReadError as E;
use api_gateway::config::GatewayMetrics;
use fixture::*;
use platform::{edge::Edge, edge_concurrency::EdgeLimiter};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use uuid::Uuid;

async fn wait_for<F, Fut>(what: &str, mut condition: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let until = Instant::now() + Duration::from_secs(3);
    while !condition().await {
        assert!(Instant::now() < until, "timed out observing {what}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
async fn lock_waiters(owner: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar("select count(*) from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query ilike '%statement_timestamp()%' and query ilike '%profiles%'")
        .fetch_one(owner)
        .await
        .unwrap()
}

async fn assert_reader_settings_restored(reader: &sqlx::PgPool) -> anyhow::Result<()> {
    let (timeout, read_only): (String, String) = tokio::time::timeout(
        Duration::from_millis(250),
        sqlx::query_as(
            "select current_setting('statement_timeout'), current_setting('transaction_read_only')",
        )
        .fetch_one(reader),
    )
    .await??;
    assert_eq!(
        timeout, "0",
        "transaction-local timeout leaked into the pool"
    );
    assert_eq!(read_only, "on", "reader lost its read-only session setting");
    Ok(())
}

#[tokio::test]
async fn expired_context_never_reaches_profile_server() -> anyhow::Result<()> {
    let server = Running::start(profile_state(lazy_pool()).await?).await?;
    let client = server.client().await?;
    let user = Uuid::new_v4();
    let ctx = context(user, Duration::from_millis(60));
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        client.read_concert_self_profile(&ctx).await.unwrap_err(),
        E::AccessExpired
    );
    assert_eq!(server.requests.load(Ordering::SeqCst), 0);
    server.stop().await
}
#[tokio::test]
async fn wrong_gateway_credential_is_denied_by_production_guard() -> anyhow::Result<()> {
    let server = Running::start(profile_state(lazy_pool()).await?).await?;
    let client =
        api_gateway::adapters::out::profile_client::GrpcProfileClient::connect_with_metrics(
            server.url.clone(),
            platform::grpc_auth::Workload::Gateway,
            "wrong-private-token",
            None,
            platform::actor_delegation::ActorDelegationSigner::local_parity(
                platform::grpc_auth::Workload::Gateway,
            ),
            GatewayMetrics::default(),
        )
        .await?;
    let ctx = context(Uuid::new_v4(), Duration::from_secs(2));
    assert_eq!(
        client.read_concert_self_profile(&ctx).await.unwrap_err(),
        E::Denied
    );
    assert_eq!(server.requests.load(Ordering::SeqCst), 1);
    server.stop().await
}

#[tokio::test]
async fn native_self_content_is_minimal_four_byte_exact_and_jwt_subject_bound() -> anyhow::Result<()>
{
    let owner = owner_pool().await?;
    let reader = reader_pool("mcr_gateway_profile_reader").await?;
    let name = "🎸".repeat(40);
    let bio = "🎵".repeat(160);
    let first = seed(&owner, &name, &bio).await?;
    let second = seed(&owner, "Other", "other bio").await?;
    let server = Running::start(profile_state(reader.clone()).await?).await?;
    let client = server.client().await?;
    let before = chrono::Utc::now();
    let ctx = context(first, Duration::from_secs(2));
    let observed = client.read_concert_self_profile(&ctx).await.unwrap();
    let after = chrono::Utc::now();
    assert_eq!(observed.user_id().unwrap(), first);
    assert_eq!(observed.display_name().unwrap(), name);
    assert_eq!(observed.bio().unwrap(), Some(bio.as_str()));
    assert!(before <= observed.observed_at().unwrap() && observed.observed_at().unwrap() <= after);
    let debug = format!("{observed:?}");
    for private in [
        PRIVATE_ROLE_CANARY,
        "private-handle-",
        "legacy-token-canary",
        "private-delete-canary",
    ] {
        assert!(!debug.contains(private));
    }
    // The JWT subject decides whose content is returned; no request field selects another user.
    let other_ctx = context(second, Duration::from_secs(2));
    let other = client.read_concert_self_profile(&other_ctx).await.unwrap();
    assert_eq!(
        (other.user_id().unwrap(), other.display_name().unwrap()),
        (second, "Other")
    );
    assert!(
        sqlx::query("select username,avatar_url,delete_reason from profiles where user_id=$1")
            .bind(first)
            .fetch_optional(&reader)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("update profiles set nickname='forbidden' where user_id=$1")
            .bind(first)
            .execute(&reader)
            .await
            .is_err()
    );
    drop((observed, other));
    assert_reader_settings_restored(&reader).await?;
    server.stop().await?;
    reader.close().await;
    owner.close().await;
    Ok(())
}
#[tokio::test]
async fn native_fresh_updates_empty_bio_and_unavailable_content_use_fixed_results(
) -> anyhow::Result<()> {
    let owner = owner_pool().await?;
    let reader = reader_pool("mcr_gateway_profile_reader").await?;
    let server = Running::start(profile_state(reader.clone()).await?).await?;
    let client = server.client().await?;
    let user = seed(&owner, "Before", "before bio").await?;
    let ctx = context(user, Duration::from_secs(2));
    assert_eq!(
        client
            .read_concert_self_profile(&ctx)
            .await
            .unwrap()
            .display_name()
            .unwrap(),
        "Before"
    );
    sqlx::query("update profiles set nickname='  After  ',bio='' where user_id=$1")
        .bind(user)
        .execute(&owner)
        .await?;
    let fresh = client.read_concert_self_profile(&ctx).await.unwrap();
    assert_eq!(
        (fresh.display_name().unwrap(), fresh.bio().unwrap()),
        ("  After  ", None)
    );
    drop(fresh);
    let deleted = seed(&owner, "Deleted", "private").await?;
    sqlx::query("update profiles set deleted_at=clock_timestamp() where user_id=$1")
        .bind(deleted)
        .execute(&owner)
        .await?;
    let fenced = seed(&owner, "Fenced", "private").await?;
    sqlx::query("insert into profile_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until) values($1,$2,$3,now()+interval '1 day')").bind(fenced).bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&owner).await?;
    let mut subjects = vec![Uuid::new_v4(), deleted, fenced];
    for (name, bio) in [
        (" ".into(), "bio".into()),
        (
            "bad
name"
                .into(),
            "bio".into(),
        ),
        (
            "Name".into(),
            "bad
bio"
            .into(),
        ),
        ("a".repeat(41), "bio".into()),
        ("🎸".repeat(41), "bio".into()),
        ("Name".into(), "b".repeat(161)),
        ("x".repeat(2 * 1024 * 1024), "bio".into()),
        ("Name".into(), "b".repeat(2 * 1024 * 1024)),
    ] {
        subjects.push(seed(&owner, &name, &bio).await?);
    }
    for subject in subjects {
        let ctx = context(subject, Duration::from_secs(2));
        assert_eq!(
            client.read_concert_self_profile(&ctx).await.unwrap_err(),
            E::NotFound
        );
    }
    // Uncommitted edits stay invisible; the committed result is read by the next request.
    let user = seed(&owner, "Committed", "bio").await?;
    let ctx = context(user, Duration::from_secs(2));
    let mut tx = owner.begin().await?;
    sqlx::query("update profiles set nickname='Pending' where user_id=$1")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    assert_eq!(
        client
            .read_concert_self_profile(&ctx)
            .await
            .unwrap()
            .display_name()
            .unwrap(),
        "Committed"
    );
    tx.commit().await?;
    assert_eq!(
        client
            .read_concert_self_profile(&ctx)
            .await
            .unwrap()
            .display_name()
            .unwrap(),
        "Pending"
    );
    server.stop().await?;
    reader.close().await;
    owner.close().await;
    Ok(())
}
#[tokio::test]
async fn native_permission_failure_is_fixed_unavailable() -> anyhow::Result<()> {
    let owner = owner_pool().await?;
    let pool = reader_pool("mcr_gateway_profile_noaccess").await?;
    let server = Running::start(profile_state(pool.clone()).await?).await?;
    let client = server.client().await?;
    let ctx = context(Uuid::new_v4(), Duration::from_secs(2));
    let error = client.read_concert_self_profile(&ctx).await.unwrap_err();
    assert_eq!(error, E::Unavailable);
    assert_eq!(error.to_string(), "Concert Profile owner unavailable");
    assert_reader_settings_restored(&pool).await?;
    server.stop().await?;
    pool.close().await;
    owner.close().await;
    Ok(())
}
#[tokio::test]
async fn native_lock_timeout_expires_context_then_same_client_and_pool_recover(
) -> anyhow::Result<()> {
    let owner = owner_pool().await?;
    let reader = reader_pool("mcr_gateway_profile_reader").await?;
    let user = seed(&owner, "Live", "bio").await?;
    let server = Running::start(profile_state(reader.clone()).await?).await?;
    let client = server.client().await?;
    let mut lock = owner.begin().await?;
    sqlx::query("lock table profiles in access exclusive mode")
        .execute(&mut *lock)
        .await?;
    // Original budget longer than the retained local ceiling: the Gateway Profile ceiling ends the wait.
    let long = context(user, Duration::from_secs(5));
    let started = Instant::now();
    let long_error = client.read_concert_self_profile(&long).await.unwrap_err();
    eprintln!(
        "local-ceiling result: error={long_error:?}, elapsed_us={}",
        started.elapsed().as_micros()
    );
    // The production DB timer uses integer milliseconds. It may fail closed
    // just before the exact authority stop; the parent retained this distinction.
    assert!(matches!(long_error, E::DeadlineExceeded | E::Unavailable));
    let ceiling = platform::grpc_deadline::local_deadline(Edge::GatewayProfile);
    assert!(
        started.elapsed() >= ceiling.saturating_sub(Duration::from_millis(10))
            && started.elapsed() < Duration::from_secs(2)
    );
    tokio::time::timeout(Duration::from_millis(350), async {
        wait_for(
            "owner cleanup after local ceiling, lock still held",
            || async { lock_waiters(&owner).await == 0 },
        )
        .await;
        sqlx::query("select 1").execute(&reader).await
    })
    .await??;
    assert_reader_settings_restored(&reader).await?;
    // A shorter original budget expires the context itself.
    let short = context(user, Duration::from_millis(150));
    let short_started = Instant::now();
    let short_error = client.read_concert_self_profile(&short).await.unwrap_err();
    eprintln!(
        "original-short result: error={short_error:?}, elapsed_us={}",
        short_started.elapsed().as_micros()
    );
    assert!(matches!(
        short_error,
        E::AccessExpired | E::DeadlineExceeded | E::Unavailable
    ));
    assert!(short_started.elapsed() >= Duration::from_millis(140));
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(short.check().is_err());
    // A fresh local 700 ms database budget would miss this bound. Keep the same
    // lock held and require the same single-connection pool to recover first.
    tokio::time::timeout(Duration::from_millis(350), async {
        wait_for("owner cleanup after shorter original deadline", || async {
            lock_waiters(&owner).await == 0
        })
        .await;
        sqlx::query("select 1").execute(&reader).await
    })
    .await??;
    assert!(short_started.elapsed() < ceiling.saturating_sub(Duration::from_millis(100)));
    assert_reader_settings_restored(&reader).await?;
    lock.rollback().await?;
    tokio::time::timeout(
        Duration::from_secs(3),
        sqlx::query("select 1").execute(&reader),
    )
    .await??;
    let live = context(user, Duration::from_secs(2));
    assert_eq!(
        client
            .read_concert_self_profile(&live)
            .await
            .unwrap()
            .display_name()
            .unwrap(),
        "Live"
    );
    server.stop().await?;
    reader.close().await;
    owner.close().await;
    Ok(())
}
#[tokio::test]
async fn native_cancelled_in_flight_read_releases_gateway_capacity_and_owner_recovers(
) -> anyhow::Result<()> {
    let owner = owner_pool().await?;
    let reader = reader_pool("mcr_gateway_profile_reader").await?;
    let user = seed(&owner, "Live", "bio").await?;
    let server = Running::start(profile_state(reader.clone()).await?).await?;
    let metrics = GatewayMetrics::default();
    let client = server.client_with(metrics.clone()).await?;
    let edge = Edge::GatewayProfile;
    assert_eq!(EdgeLimiter::global().available(edge), edge.max_in_flight());
    let mut lock = owner.begin().await?;
    sqlx::query("lock table profiles in access exclusive mode")
        .execute(&mut *lock)
        .await?;
    let ctx = context(user, Duration::from_secs(5));
    let task = {
        let client = client.clone();
        tokio::spawn(async move { client.read_concert_self_profile(&ctx).await.map(|_| ()) })
    };
    // Observe real work first: a Gateway edge permit is held and the SQL statement waits on the lock.
    wait_for("gateway permit", || async {
        EdgeLimiter::global().available(edge) < edge.max_in_flight()
    })
    .await;
    wait_for("owner lock wait", || async {
        lock_waiters(&owner).await >= 1
    })
    .await;
    assert!(!task.is_finished());
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    wait_for("gateway permit release", || async {
        EdgeLimiter::global().available(edge) == edge.max_in_flight()
    })
    .await;
    // Keep the lock held while observing eventual owner cancellation/deadline cleanup.
    // Releasing the lock first would let abandoned work finish and hide a leak.
    let cleanup_started = Instant::now();
    let mut waiting = lock_waiters(&owner).await;
    while waiting != 0 && cleanup_started.elapsed() < Duration::from_secs(3) {
        tokio::time::sleep(Duration::from_millis(10)).await;
        waiting = lock_waiters(&owner).await;
    }
    let reusable_while_locked = tokio::time::timeout(
        Duration::from_millis(250),
        sqlx::query("select 1").execute(&reader),
    )
    .await
    .is_ok_and(|result| result.is_ok());
    eprintln!(
        "cancellation cleanup: gateway_available={}, gateway_capacity={}, owner_lock_waiters={}, elapsed_ms={}, single_connection_reusable_while_locked={}",
        EdgeLimiter::global().available(edge), edge.max_in_flight(), waiting,
        cleanup_started.elapsed().as_millis(), reusable_while_locked,
    );
    // Release the fixture's lock even on failure; keep the prior evidence as the assertion.
    lock.rollback().await?;
    tokio::time::timeout(
        Duration::from_secs(3),
        sqlx::query("select 1").execute(&reader),
    )
    .await??;
    assert_eq!(
        waiting, 0,
        "owner query outlived cancellation/deadline while lock remained held"
    );
    assert!(
        reusable_while_locked,
        "single owner connection was occupied by abandoned work"
    );
    assert_reader_settings_restored(&reader).await?;
    let live = context(user, Duration::from_secs(2));
    assert_eq!(
        client
            .read_concert_self_profile(&live)
            .await
            .unwrap()
            .display_name()
            .unwrap(),
        "Live"
    );
    assert_eq!(EdgeLimiter::global().available(edge), edge.max_in_flight());
    server.stop().await?;
    reader.close().await;
    owner.close().await;
    Ok(())
}
#[tokio::test]
async fn native_stopped_database_is_fixed_unavailable() -> anyhow::Result<()> {
    // The controller runs only this case while its exact owned PostgreSQL server is stopped.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(150))
        .connect_lazy_with(owned_options()?);
    let server = Running::start(profile_state(pool.clone()).await?).await?;
    let client = server.client().await?;
    let ctx = context(Uuid::new_v4(), Duration::from_secs(2));
    assert_eq!(
        client.read_concert_self_profile(&ctx).await.unwrap_err(),
        E::Unavailable
    );
    server.stop().await?;
    pool.close().await;
    Ok(())
}
#[tokio::test]
async fn native_restarted_database_serves_a_fresh_read() -> anyhow::Result<()> {
    let owner = owner_pool().await?;
    let reader = reader_pool("mcr_gateway_profile_reader").await?;
    let user = seed(&owner, "Recovered", "bio").await?;
    let server = Running::start(profile_state(reader.clone()).await?).await?;
    let client = server.client().await?;
    let ctx = context(user, Duration::from_secs(2));
    assert_eq!(
        client
            .read_concert_self_profile(&ctx)
            .await
            .unwrap()
            .display_name()
            .unwrap(),
        "Recovered"
    );
    server.stop().await?;
    reader.close().await;
    owner.close().await;
    Ok(())
}
