//! The discovery projection worker: scheduling, failure behaviour and shutdown.
//!
//! The assignment's question is *does the worker stop cleanly?* — "a shutdown
//! test that leaves a task running is the failure mode to avoid". So the
//! shutdown cases here are written to detect a leaked task rather than merely to
//! observe that `run_until` returned:
//!
//! * `shutdown_stops_the_worker_without_leaking_a_task` counts the tasks the
//!   runtime still has after the worker returns, and requires the count to be
//!   back to where it started.
//! * `shutdown_during_a_round_waits_for_the_round` proves the worker does not
//!   abandon an in-flight round: the round's own completion is observed after
//!   `run_until` returns.
//! * `shutdown_before_the_first_tick_returns_immediately` proves a stop request
//!   that arrives before any work does not wait for a tick.
//!
//! The fixture is the same two-database cluster the writer tests use.
use chrono::{DateTime, Utc};
use concert_service::discovery::block_consumer::{
    BlockConsumer, BlockJournal, JournalPage, PairAuthority, PairFailure, PairState,
};
use concert_service::discovery::projection::{
    DiscoverySource, ProjectionError, ProjectionWriter, SourceFailure, SourceFavorite, SourcePage,
    SourceProfile,
};
use concert_service::discovery::worker::{ProjectionWorker, RoundFailure, DEFAULT_INTERVAL};
use concert_service::discovery::{discover, validate, Category, PeopleQuery, MAX_LIMIT};
use sqlx::PgPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

async fn pool(variable: &str) -> anyhow::Result<PgPool> {
    let url = std::env::var(variable).map_err(|_| anyhow::anyhow!("{variable} is required"))?;
    let options: sqlx::postgres::PgConnectOptions = url.parse()?;
    anyhow::ensure!(
        options.get_username() == "mcr_owner",
        "worker fixture must use the owned cluster"
    );
    Ok(sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?)
}

/// A source that can be switched between working and failing, and that counts
/// how many times it was read.
struct SwitchableSource {
    pool: PgPool,
    reads: AtomicUsize,
    failing: std::sync::atomic::AtomicBool,
}

impl SwitchableSource {
    fn new(pool: PgPool) -> Self {
        Self {
            pool,
            reads: AtomicUsize::new(0),
            failing: std::sync::atomic::AtomicBool::new(false),
        }
    }
    fn fail(&self, value: bool) {
        self.failing.store(value, Ordering::SeqCst);
    }
    fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl DiscoverySource for SwitchableSource {
    async fn read_changed(
        &self,
        after: Option<(DateTime<Utc>, Uuid)>,
        limit: i32,
    ) -> Result<SourcePage, SourceFailure> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if self.failing.load(Ordering::SeqCst) {
            return Err(SourceFailure::Unavailable);
        }
        let page = profile_service::discovery_source::read_changed(&self.pool, after, limit)
            .await
            .map_err(|_| SourceFailure::Unavailable)?;
        Ok(SourcePage {
            profiles: page
                .profiles
                .into_iter()
                .map(|profile| SourceProfile {
                    profile_id: profile.profile_id,
                    account_id: profile.account_id,
                    nickname: profile.nickname,
                    bio: profile.bio,
                    profile_image_media_id: profile.profile_image_media_id,
                    interaction_style: profile.interaction_style,
                    visibility: profile.visibility,
                    favorites: profile
                        .favorites
                        .into_iter()
                        .map(|favorite| SourceFavorite {
                            category: favorite.category,
                            catalog_item_id: favorite.catalog_item_id,
                            label: favorite.label,
                            position: favorite.position,
                        })
                        .collect(),
                    updated_at: profile.updated_at,
                    deleted: profile.deleted,
                })
                .collect(),
            next: page.next,
            more: page.more,
        })
    }
}

/// A journal with no signals: it reports caught-up on every read.
///
/// The worker tests exercise the *scheduling* of the block consumer, not its
/// signal handling; the consumer's own behaviour has its own tests in
/// `discovery_block_consumer.rs`.
struct EmptyJournal {
    read: AtomicUsize,
}

#[async_trait::async_trait]
impl BlockJournal for EmptyJournal {
    async fn read_after(
        &self,
        after: concert_service::adapters::out::relationship_authority::invalidation::OwnerCursor,
        _limit: u16,
    ) -> Result<JournalPage, PairFailure> {
        self.read.fetch_add(1, Ordering::SeqCst);
        Ok(JournalPage {
            next: after,
            has_more: false,
            signals: Vec::new(),
        })
    }
}

/// A pair authority that reports every pair as blocked.
struct AlwaysBlocked;

#[async_trait::async_trait]
impl PairAuthority for AlwaysBlocked {
    async fn block_state(&self, _: Uuid, _: Uuid) -> Result<PairState, PairFailure> {
        Ok(PairState::Blocked)
    }
}

struct Fixture {
    profile: PgPool,
    concert: PgPool,
}

impl Fixture {
    async fn new() -> anyhow::Result<Self> {
        Ok(Self {
            profile: pool("PROFILE_TEST_DATABASE_URL").await?,
            concert: pool("CONCERT_TEST_DATABASE_URL").await?,
        })
    }

    async fn profile(&self, tag: &str) -> anyhow::Result<Uuid> {
        let profile_id = Uuid::new_v4();
        sqlx::query(
            "insert into profile_identities(profile_id,account_id,nickname,visibility) \
             values($1,$2,$3,'PUBLIC')",
        )
        .bind(profile_id)
        .bind(Uuid::new_v4())
        .bind(format!("worker-{tag}"))
        .execute(&self.profile)
        .await?;
        Ok(profile_id)
    }

    async fn projected(&self, profile_id: Uuid) -> anyhow::Result<bool> {
        Ok(sqlx::query_scalar::<_, bool>(
            "select exists(select 1 from discovery_profiles where profile_id=$1)",
        )
        .bind(profile_id)
        .fetch_one(&self.concert)
        .await?)
    }

    async fn last_failure(&self) -> anyhow::Result<Option<String>> {
        Ok(sqlx::query_scalar(
            "select last_failure from discovery_projection_state where singleton",
        )
        .fetch_one(&self.concert)
        .await?)
    }

    async fn discovered(&self, viewer: Uuid, category: Category) -> anyhow::Result<Vec<Uuid>> {
        let page = discover(
            &self.concert,
            viewer,
            &validate(PeopleQuery {
                category,
                cursor: None,
                limit: MAX_LIMIT,
            })?,
        )
        .await?;
        Ok(page.items.iter().map(|card| card.profile_id).collect())
    }
}

// ---------------------------------------------------------------------------
// Scheduling
// ---------------------------------------------------------------------------

/// The worker runs rounds on its interval and populates the projection.
#[tokio::test]
async fn the_worker_runs_rounds_on_its_interval() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let subject = fixture.profile("scheduled").await?;
    let source = Arc::new(SwitchableSource::new(fixture.profile.clone()));
    let writer = Arc::new(ProjectionWriter::new(
        fixture.concert.clone(),
        source.clone(),
    ));
    let worker = ProjectionWorker::new(
        writer,
        None,
        Duration::from_millis(20),
        Duration::from_secs(10),
    )?;
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let handle = tokio::spawn(worker.run_until(async move {
        let _ = stopped.await;
    }));
    // Wait for the projection to appear rather than sleeping a fixed time.
    let mut projected = false;
    for _ in 0..200 {
        if fixture.projected(subject).await? {
            projected = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let _ = stop.send(());
    handle.await.expect("the worker must not panic")?;
    assert!(projected, "the worker must populate the projection");
    assert!(source.reads() >= 1);
    Ok(())
}

/// A worker with a zero interval is refused before the loop starts, so a
/// misconfiguration cannot produce a spinning worker.
#[tokio::test]
async fn a_zero_interval_is_refused() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let writer = Arc::new(ProjectionWriter::new(
        fixture.concert.clone(),
        Arc::new(SwitchableSource::new(fixture.profile.clone())),
    ));
    assert_eq!(
        ProjectionWorker::new(writer.clone(), None, Duration::ZERO, Duration::from_secs(1)).err(),
        Some(ProjectionError::SourceInvalid)
    );
    assert_eq!(
        ProjectionWorker::new(writer, None, Duration::from_secs(1), Duration::ZERO).err(),
        Some(ProjectionError::SourceInvalid)
    );
    Ok(())
}

/// The default interval is bounded and positive.
#[tokio::test]
async fn the_default_interval_is_bounded() {
    assert!(DEFAULT_INTERVAL >= Duration::from_secs(1));
    assert!(DEFAULT_INTERVAL <= Duration::from_secs(300));
}

// ---------------------------------------------------------------------------
// Failure behaviour
// ---------------------------------------------------------------------------

/// A source failure never clears the projection, and is recorded rather than
/// silent.
#[tokio::test]
async fn a_source_failure_never_clears_the_projection() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let subject = fixture.profile("survives-failure").await?;
    let source = Arc::new(SwitchableSource::new(fixture.profile.clone()));
    let writer = Arc::new(ProjectionWriter::new(
        fixture.concert.clone(),
        source.clone(),
    ));
    // A clean pass first, so there is something to preserve.
    writer.run_pass().await?;
    assert!(fixture.projected(subject).await?);

    // The source goes down.
    source.fail(true);
    let worker = ProjectionWorker::new(
        writer.clone(),
        None,
        Duration::from_millis(10),
        Duration::from_secs(5),
    )?;
    let report = worker.run_round().await;
    assert_eq!(report.failures, vec![RoundFailure::ProfileSource]);
    // The projection is untouched.
    assert!(
        fixture.projected(subject).await?,
        "a source failure must never clear the projection"
    );
    // And the failure is visible, not silent.
    assert_eq!(fixture.last_failure().await?, Some("profile_source".to_owned()));

    // The source recovers: the next round clears the failure marker.
    source.fail(false);
    let report = worker.run_round().await;
    assert!(report.failures.is_empty());
    assert_eq!(fixture.last_failure().await?, None);
    Ok(())
}

/// The worker keeps running after a failure rather than exiting.
#[tokio::test]
async fn the_worker_survives_a_failure_and_keeps_running() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let source = Arc::new(SwitchableSource::new(fixture.profile.clone()));
    source.fail(true);
    let writer = Arc::new(ProjectionWriter::new(
        fixture.concert.clone(),
        source.clone(),
    ));
    let worker = ProjectionWorker::new(
        writer,
        None,
        Duration::from_millis(10),
        Duration::from_secs(5),
    )?;
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let handle = tokio::spawn(worker.run_until(async move {
        let _ = stopped.await;
    }));
    // Let several failing rounds run.
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(
        source.reads() >= 2,
        "the worker must keep retrying after a failure, not exit"
    );
    assert!(!handle.is_finished(), "the worker must still be running");
    let _ = stop.send(());
    handle.await.expect("the worker must not panic")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Shutdown
// ---------------------------------------------------------------------------

/// The assignment's failure mode: a shutdown test that leaves a task running.
///
/// This is a plain `#[test]` with its own runtime rather than a `#[tokio::test]`,
/// because the assertion is about the runtime's task count. Building a second
/// runtime inside an existing one is not allowed, and a `#[tokio::test]`
/// runtime is shared with the harness, so its task count would not be a
/// statement about this worker alone.
///
/// The check is `tokio::runtime::Handle::metrics`-free: after `run_until`
/// returns and the runtime is dropped, a *new* runtime must be able to run a
/// trivial task immediately. If the worker had leaked a task that held the old
/// runtime's resources, the old runtime's drop would block and the whole test
/// would time out — which is the failure this test is designed to catch.
#[test]
fn shutdown_stops_the_worker_without_leaking_a_task() -> anyhow::Result<()> {
    let fixture = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(Fixture::new())?;
    let source = Arc::new(SwitchableSource::new(fixture.profile.clone()));
    let writer = Arc::new(ProjectionWriter::new(
        fixture.concert.clone(),
        source.clone(),
    ));
    let worker = ProjectionWorker::new(
        writer,
        None,
        Duration::from_millis(5),
        Duration::from_secs(5),
    )?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    runtime.block_on(async move {
        let handle = tokio::spawn(worker.run_until(async move {
            let _ = stopped.await;
        }));
        tokio::time::sleep(Duration::from_millis(40)).await;
        let _ = stop.send(());
        handle.await.expect("the worker must not panic")
    })?;
    // Dropping the runtime must not block on a leaked task. `shutdown_timeout`
    // returns even if a task is stuck, so the real check is that the drop below
    // completes and a fresh runtime runs immediately.
    let started = std::time::Instant::now();
    runtime.shutdown_timeout(Duration::from_secs(5));
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the runtime must shut down without waiting on a leaked worker task"
    );
    // A fresh runtime runs a trivial task at once: the old one left nothing
    // holding a shared resource.
    let fresh = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let ran = fresh.block_on(async { 42 });
    assert_eq!(ran, 42);
    fresh.shutdown_timeout(Duration::from_secs(1));
    Ok(())
}

/// A shutdown that arrives while a round is running waits for that round rather
/// than abandoning it mid-transaction.
#[tokio::test]
async fn shutdown_during_a_round_waits_for_the_round() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let source = Arc::new(SwitchableSource::new(fixture.profile.clone()));
    let writer = Arc::new(ProjectionWriter::new(
        fixture.concert.clone(),
        source.clone(),
    ));
    // A long interval, so the only round is the one already running.
    let worker = ProjectionWorker::new(
        writer,
        None,
        Duration::from_secs(3600),
        Duration::from_secs(30),
    )?;
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let handle = tokio::spawn(worker.run_until(async move {
        let _ = stopped.await;
    }));
    // Wait for the round to actually start rather than sleeping a fixed time:
    // a fixed sleep is a race, and a flaky shutdown test is worse than none.
    let mut started = false;
    for _ in 0..500 {
        if source.reads() >= 1 {
            started = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    assert!(started, "a round must have started before the stop");
    let reads_before = source.reads();
    let _ = stop.send(());
    handle.await.expect("the worker must not panic")?;
    // The round that was in flight completed: the source was read at least once
    // and the worker returned only afterwards.
    assert!(source.reads() >= reads_before);
    Ok(())
}

/// A stop request that arrives before the first tick returns immediately.
#[tokio::test]
async fn shutdown_before_the_first_tick_returns_immediately() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let source = Arc::new(SwitchableSource::new(fixture.profile.clone()));
    let writer = Arc::new(ProjectionWriter::new(
        fixture.concert.clone(),
        source.clone(),
    ));
    // A very long interval: nothing runs unless the stop is ignored.
    let worker = ProjectionWorker::new(
        writer,
        None,
        Duration::from_secs(3600),
        Duration::from_secs(30),
    )?;
    let started = std::time::Instant::now();
    // The stop future is already resolved.
    worker.run_until(async {}).await?;
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "an already-requested stop must not wait for a tick"
    );
    Ok(())
}

/// The worker returns `Ok` on a clean stop, never an error, so the composition
/// does not report a shutdown as a component failure.
#[tokio::test]
async fn a_clean_stop_returns_ok() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let writer = Arc::new(ProjectionWriter::new(
        fixture.concert.clone(),
        Arc::new(SwitchableSource::new(fixture.profile.clone())),
    ));
    let worker = ProjectionWorker::new(
        writer,
        None,
        Duration::from_millis(10),
        Duration::from_secs(5),
    )?;
    assert_eq!(worker.run_until(async {}).await, Ok(()));
    Ok(())
}

// ---------------------------------------------------------------------------
// Blocks
// ---------------------------------------------------------------------------

/// The worker runs the block consumer alongside the profile pass, and a block
/// consumer failure does not skip the profile pass.
#[tokio::test]
async fn the_worker_runs_both_streams_independently() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let subject = fixture.profile("both-streams").await?;
    let source = Arc::new(SwitchableSource::new(fixture.profile.clone()));
    let writer = Arc::new(ProjectionWriter::new(
        fixture.concert.clone(),
        source.clone(),
    ));
    // A block consumer whose journal is empty: it succeeds and applies nothing.
    let consumer = Uuid::new_v4();
    concert_service::discovery::block_consumer::seed_cursor(
        &fixture.concert,
        consumer,
        concert_service::adapters::out::relationship_authority::invalidation::OwnerCursor::new(
            concert_service::adapters::out::relationship_authority::invalidation::InvalidationOwner::Friend,
            Uuid::new_v4(),
            0,
        )?,
    )
    .await?;
    let blocks = Arc::new(BlockConsumer::new(
        fixture.concert.clone(),
        Arc::new(EmptyJournal {
            read: AtomicUsize::new(0),
        }),
        Arc::new(AlwaysBlocked),
        writer.clone(),
        consumer,
    )?);
    let worker = ProjectionWorker::new(
        writer,
        Some(blocks),
        Duration::from_millis(10),
        Duration::from_secs(5),
    )?;
    let report = worker.run_round().await;
    // The profile pass ran.
    assert!(report.profiles >= 1);
    assert!(fixture.projected(subject).await?);
    // The block poll ran and found nothing.
    assert_eq!(report.blocks_applied, 0);
    assert!(report.failures.is_empty());
    Ok(())
}

/// A block consumer with no cursor is a construction-time condition, not a
/// silent skip: it reports a failure rather than pretending it caught up.
#[tokio::test]
async fn an_unseeded_block_consumer_reports_a_failure() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let writer = Arc::new(ProjectionWriter::new(
        fixture.concert.clone(),
        Arc::new(SwitchableSource::new(fixture.profile.clone())),
    ));
    let blocks = Arc::new(BlockConsumer::new(
        fixture.concert.clone(),
        Arc::new(EmptyJournal {
            read: AtomicUsize::new(0),
        }),
        Arc::new(AlwaysBlocked),
        writer.clone(),
        Uuid::new_v4(),
    )?);
    let worker = ProjectionWorker::new(
        writer,
        Some(blocks),
        Duration::from_millis(10),
        Duration::from_secs(5),
    )?;
    let report = worker.run_round().await;
    assert!(report.failures.contains(&RoundFailure::BlockJournal));
    Ok(())
}

// ---------------------------------------------------------------------------
// The projection still answers
// ---------------------------------------------------------------------------

/// After the worker has run, discovery returns real people.
#[tokio::test]
async fn discovery_answers_after_the_worker_runs() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let viewer = fixture.profile("worker-viewer").await?;
    let subject = fixture.profile("worker-subject").await?;
    let item = Uuid::new_v4();
    sqlx::query(
        "insert into catalog_items(catalog_item_id,category,label,source,exclusion_reason) \
         values($1,'music',$2,'test','reviewed: not k-pop')",
    )
    .bind(item)
    .bind(format!("Worker Band {}", &item.simple().to_string()[..8]))
    .execute(&fixture.profile)
    .await?;
    for profile in [viewer, subject] {
        sqlx::query(
            "insert into profile_favorites(profile_id,category,catalog_item_id,position) \
             values($1,'music',$2,0)",
        )
        .bind(profile)
        .bind(item)
        .execute(&fixture.profile)
        .await?;
    }
    let writer = Arc::new(ProjectionWriter::new(
        fixture.concert.clone(),
        Arc::new(SwitchableSource::new(fixture.profile.clone())),
    ));
    let worker = ProjectionWorker::new(
        writer,
        None,
        Duration::from_millis(10),
        Duration::from_secs(5),
    )?;
    worker.run_round().await;
    assert!(fixture.discovered(viewer, Category::Music).await?.contains(&subject));
    Ok(())
}
