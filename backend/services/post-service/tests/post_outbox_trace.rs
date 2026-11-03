//! Exercises the reviewed Postgres spans against real PostgreSQL. Testcontainers
//! is the default fixture; a disposable loopback database is opt-in for machines
//! without a working container runtime.

use anyhow::{Context, Result};
use chrono::Utc;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::{InMemorySpanExporter, Sampler, SdkTracerProvider};
use platform::trace_context::{self, TraceContextLayer};
use post_service::adapters::out::persistence::event_outbox::PgEventOutbox;
use post_service::adapters::out::persistence::post_repository::PgPostRepository;
use post_service::application::ports::{EventOutboxPort, PostRepositoryPort};
use post_service::domain::post::{
    DeliveryAttempt, DeliveryAttemptPhase, DeliveryAttemptSource, DeliveryTarget, Post, StoredEvent,
};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::collections::HashSet;
use std::net::IpAddr;
use std::path::PathBuf;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use tower::{Layer as _, ServiceExt as _};
use tracing::instrument::WithSubscriber as _;
use tracing_subscriber::prelude::*;
use uuid::Uuid;

#[allow(dead_code)]
#[path = "unit/support.rs"]
mod notification_support;

#[tokio::test]
async fn notification_fanout_checks_authoritative_postgres_state_before_outbox_delivery(
) -> Result<()> {
    use post_service::adapters::r#in::messaging::external_event_forwarder::ExternalEventForwarder;
    use post_service::adapters::r#in::messaging::internal_notification_listener::InternalNotificationListener;
    use post_service::application::dto::post_dto::CreatePostDto;
    use post_service::domain::post::RelationshipPolicySnapshot;
    use std::sync::Arc;

    let fixture = Fixture::new().await?;
    let result = async {
        let harness = notification_support::Harness::new();
        let author = Uuid::new_v4();
        let follower = Uuid::new_v4();
        *harness.friend.follower_ids.lock().unwrap() = vec![follower];
        harness.friend.policies.lock().unwrap().insert(
            (follower, author),
            RelationshipPolicySnapshot {
                following: true,
                ..Default::default()
            },
        );
        let notifications = Arc::new(notification_support::SpyNotificationClient::default());
        let repository = Arc::new(PgPostRepository::new(fixture.pool.clone()));
        let dispatcher = Arc::new(InternalNotificationListener::new(
            repository.clone(),
            harness.friend.clone(),
            notifications.clone(),
        ));
        let outbox = Arc::new(PgEventOutbox::new(fixture.pool.clone()));
        let forwarder = ExternalEventForwarder::new(
            outbox,
            harness.publisher.clone(),
            dispatcher,
            harness.feed_cache.clone(),
            harness.post_cache.clone(),
            harness.friend.clone(),
        Arc::new(post_service::adapters::out::persistence::timeline_projection_repository::PgTimelineProjectionRepository::new(fixture.pool.clone())),
        );
        let mut service = (*harness.service).clone();
        service.posts = repository;
        let mut ids = Vec::new();
        for visibility in ["PRIVATE", "PUBLIC", "FRIENDS", "PUBLIC", "PUBLIC"] {
            let created = service
                .create_post(CreatePostDto {
                    author_id: author.to_string(),
                    content: format!("notification {visibility} secret"),
                    visibility: visibility.into(),
                })
                .await?;
            ids.push(Uuid::parse_str(&created.id)?);
        }
        // Lower-level lifecycle changes before a delayed worker runs. There is
        // no public post-visibility-update API; these are explicit DB probes.
        sqlx::query("update posts set deleted_at = now() where id = $1")
            .bind(ids[3])
            .execute(&fixture.pool)
            .await?;
        sqlx::query("update posts set moderation_hidden_at = now() where id = $1")
            .bind(ids[4])
            .execute(&fixture.pool)
            .await?;
        assert_eq!(forwarder.republish_unpublished(Utc::now(), 20).await?, 5);
        let sent = notifications.notifications.lock().unwrap().clone();
        assert_eq!(sent.len(), 2);
        let sent_posts: HashSet<Uuid> = sent
            .iter()
            .map(|item| {
                let payload: serde_json::Value = serde_json::from_str(&item.payload_json).unwrap();
                Uuid::parse_str(payload["postId"].as_str().unwrap()).unwrap()
            })
            .collect();
        assert_eq!(sent_posts, HashSet::from([ids[1], ids[2]]));
        assert!(sent.iter().all(|item| item.user_id == follower));
        let unpublished: i64 =
            sqlx::query_scalar("select count(*) from post_event_outbox where not published")
                .fetch_one(&fixture.pool)
                .await?;
        assert_eq!(
            unpublished, 0,
            "suppression completes INTERNAL without poisoning replay progress"
        );
        let completed_internal: i64 =
            sqlx::query_scalar("select count(*) from event_publications where target = 'INTERNAL'")
                .fetch_one(&fixture.pool)
                .await?;
        assert_eq!(completed_internal, 5);
        Ok::<(), anyhow::Error>(())
    }
    .await;
    fixture.shutdown().await?;
    result
}

const LOCAL_DATABASE_URL_ENV: &str = "POST_TRACE_TEST_DATABASE_URL";
const LOCAL_DATABASE_OPT_IN_ENV: &str = "POST_TRACE_TEST_ALLOW_LOCAL_PG";
const LOCAL_DATABASE_PREFIX: &str = "port_rust_post_trace_test";
const LOCAL_INITDB_ENV: &str = "POST_TRACE_TEST_INITDB_SYSTEM_IDENTIFIER";
const POST_REPOSITORY_TARGET: &str = "post_service::adapters::out::persistence::post_repository";
const EVENT_OUTBOX_TARGET: &str = "post_service::adapters::out::persistence::event_outbox";
const DATABASE_SPAN_NAMES: [&str; 8] = [
    "post_write_transaction",
    "post_outbox_insert_event",
    "post_outbox_claim",
    "post_outbox_append_delivery_attempt",
    "post_outbox_delivery_check",
    "post_outbox_mark_delivery_completed",
    "post_outbox_mark_published",
    "post_outbox_publish_failure",
];

struct Fixture {
    _postgres: Option<ContainerAsync<Postgres>>,
    local_database: Option<LocalDatabase>,
    pool: PgPool,
}

struct LocalDatabase {
    admin_pool: PgPool,
    name: String,
}

impl Fixture {
    async fn new() -> Result<Self> {
        let (postgres, database_url, mut local_database) = match local_database_url()? {
            Some(database_url) => {
                let (database, fixture_url) = provision_local_database(&database_url).await?;
                (None, fixture_url, Some(database))
            }
            None => {
                let (postgres, fixture_url) =
                    test_support::postgres::start_postgres_with_url().await?;
                (Some(postgres), fixture_url, None)
            }
        };

        let pool = match PgPoolOptions::new()
            .max_connections(6)
            .connect(&database_url)
            .await
        {
            Ok(pool) => pool,
            Err(error) => {
                if let Some(local_database) = local_database.take() {
                    drop_local_database(local_database).await?;
                }
                return Err(error).context("connect Post-service trace PostgreSQL fixture");
            }
        };

        let migration_result = async {
            let migrations = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
            sqlx::migrate::Migrator::new(migrations.as_path())
                .await?
                .run(&pool)
                .await?;
            Ok::<(), anyhow::Error>(())
        }
        .await;
        if let Err(error) = migration_result {
            pool.close().await;
            if let Some(local_database) = local_database.take() {
                drop_local_database(local_database).await?;
            }
            return Err(error).context("migrate Post-service trace PostgreSQL fixture");
        }

        Ok(Self {
            _postgres: postgres,
            local_database,
            pool,
        })
    }

    async fn shutdown(mut self) -> Result<()> {
        self.pool.close().await;
        if let Some(local_database) = self.local_database.take() {
            drop_local_database(local_database).await?;
        }
        Ok(())
    }
}

fn local_database_url() -> Result<Option<String>> {
    let database_url = std::env::var(LOCAL_DATABASE_URL_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    let opt_in = std::env::var(LOCAL_DATABASE_OPT_IN_ENV).ok();
    match (database_url, opt_in.as_deref()) {
        (None, None) => Ok(None),
        (Some(url), Some("1")) => {
            validate_local_database_url(&url)?;
            Ok(Some(url))
        }
        (Some(_), _) => {
            anyhow::bail!("local Post trace PostgreSQL requires {LOCAL_DATABASE_OPT_IN_ENV}=1")
        }
        (None, Some(_)) => {
            anyhow::bail!("{LOCAL_DATABASE_OPT_IN_ENV} requires {LOCAL_DATABASE_URL_ENV}")
        }
    }
}

fn validate_local_database_url(database_url: &str) -> Result<PgConnectOptions> {
    anyhow::ensure!(
        !database_url.contains('?') && !database_url.contains('#'),
        "Post trace test URL must not contain connection overrides"
    );
    let options: PgConnectOptions = database_url
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid Post trace test PostgreSQL URL"))?;
    let database = options
        .get_database()
        .ok_or_else(|| anyhow::anyhow!("Post trace test URL must name a test database"))?;
    let database_suffix = database.strip_prefix(&format!("{LOCAL_DATABASE_PREFIX}_"));
    anyhow::ensure!(
        database_suffix.is_some_and(|suffix| !suffix.is_empty())
            && database.len() <= 63
            && database
                .bytes()
                .all(|byte| { byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' }),
        "Post trace test database must use the isolated {LOCAL_DATABASE_PREFIX}_* name"
    );
    anyhow::ensure!(options.get_socket().is_none(), "loopback TCP is required");
    let host = options
        .get_host()
        .trim_start_matches('[')
        .trim_end_matches(']');
    anyhow::ensure!(
        host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<IpAddr>()
                .map(|address| address.is_loopback())
                .unwrap_or(false),
        "Post trace test PostgreSQL must use loopback TCP"
    );
    anyhow::ensure!(
        options.get_options().is_none(),
        "Post trace test URL must not override PostgreSQL connection options"
    );
    Ok(options)
}

async fn provision_local_database(database_url: &str) -> Result<(LocalDatabase, String)> {
    let options = validate_local_database_url(database_url)?;
    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await?;
    let lineage: String =
        sqlx::query_scalar("select system_identifier::text from pg_control_system()")
            .fetch_one(&admin_pool)
            .await?;
    anyhow::ensure!(
        std::env::var(LOCAL_INITDB_ENV).as_deref() == Ok(lineage.as_str()),
        "observed Post trace initdb lineage differs"
    );
    let name = format!("{LOCAL_DATABASE_PREFIX}_{}", Uuid::new_v4().simple());
    sqlx::query(&format!(
        "create database {} template template0",
        quote_identifier(&name)
    ))
    .execute(&admin_pool)
    .await
    .context("create isolated Post trace database")?;
    let url = format!(
        "{}/{}",
        database_url.rsplit_once('/').context("trace parent URL")?.0,
        name
    );
    let observer = PgPool::connect(&url).await?;
    let empty: bool =
        sqlx::query_scalar("select not exists(select 1 from pg_tables where schemaname='public')")
            .fetch_one(&observer)
            .await?;
    observer.close().await;
    anyhow::ensure!(empty, "trace database must be empty before migrations");
    println!(
        "POST_TRACE_PG_FIXTURE {}",
        serde_json::json!({"database":name,"systemIdentifier":lineage,"template":"template0","emptyBeforeMigrations":true})
    );
    Ok((LocalDatabase { admin_pool, name }, url))
}

async fn drop_local_database(database: LocalDatabase) -> Result<()> {
    anyhow::ensure!(
        database
            .name
            .starts_with(&format!("{LOCAL_DATABASE_PREFIX}_"))
            && database
                .name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
        "refuse to drop an unrecognized Post trace fixture database"
    );
    sqlx::query(&format!(
        "drop database {}",
        quote_identifier(&database.name)
    ))
    .execute(&database.admin_pool)
    .await
    .context("drop isolated Post trace fixture database")?;
    let remaining: i64 = sqlx::query_scalar("select count(*) from pg_database where datname=$1")
        .bind(&database.name)
        .fetch_one(&database.admin_pool)
        .await?;
    anyhow::ensure!(remaining == 0, "owned Post trace database remains");
    println!(
        "POST_TRACE_PG_DISPOSED {}",
        serde_json::json!({"database":database.name,"remaining":remaining})
    );
    database.admin_pool.close().await;
    Ok(())
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn make_event(aggregate_id: Uuid, actor_id: Uuid, payload: &str) -> StoredEvent {
    let now = Utc::now();
    StoredEvent {
        event_id: Uuid::new_v4(),
        event_type: "POST_CREATED".into(),
        aggregate_id,
        aggregate_version: None,
        origin_traceparent: None,
        actor_id: Some(actor_id),
        attributes: vec!["visibility:PUBLIC".into()],
        payload: payload.into(),
        occurred_at: now,
        created_at: now,
        published: false,
        published_at: None,
        replay_resolution_id: None,
    }
}

fn test_span_filter(metadata: &tracing::Metadata<'_>) -> bool {
    if !metadata.is_span() {
        return false;
    }
    match metadata.target() {
        "platform::trace_context" => metadata.name() == "request",
        POST_REPOSITORY_TARGET => metadata.name() == "post_write_transaction",
        EVENT_OUTBOX_TARGET => DATABASE_SPAN_NAMES[1..].contains(&metadata.name()),
        _ => false,
    }
}

async fn run_database_trace(pool: PgPool) -> Result<()> {
    const INBOUND_TRACEPARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    const PRIVATE_STATE: &str = "vendor=private-post-trace-state";
    const PRIVATE_CONTENT: &str = "private-post-content-marker";
    const PRIVATE_EVENT_PAYLOAD: &str = "private-event-payload-marker";
    const PRIVATE_DELIVERY_PAYLOAD: &str = "private-delivery-payload-marker";
    const PRIVATE_PUBLISH_ERROR: &str = "private-publisher-failure-marker";

    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .with_sampler(Sampler::AlwaysOn)
        .with_max_events_per_span(0)
        .with_max_links_per_span(0)
        .build();
    let subscriber = tracing_subscriber::registry().with(
        tracing_opentelemetry::layer()
            .with_tracer(provider.tracer("post-service-db-test"))
            .with_filter(tracing_subscriber::filter::filter_fn(test_span_filter)),
    );
    let dispatch = tracing::Dispatch::new(subscriber);
    let service = TraceContextLayer.layer(tower::service_fn(move |_request: http::Request<()>| {
        let pool = pool.clone();
        async move {
            let request_context = trace_context::current().expect("request trace context");
            let actor_id = Uuid::new_v4();
            let post = Post {
                id: Uuid::new_v4(),
                author_id: actor_id,
                content: PRIVATE_CONTENT.into(),
                visibility: "PUBLIC".into(),
                created_at: Utc::now(),
                comment_count: 0,
                like_count: 0,
            };
            let post_event = make_event(post.id, actor_id, PRIVATE_EVENT_PAYLOAD);
            let (_, saved_post_event) = PgPostRepository::new(pool.clone())
                .save_post_with_event(&post, &post_event, None)
                .await?;
            anyhow::ensure!(
                saved_post_event.origin_traceparent.as_deref()
                    == Some(request_context.traceparent().as_str()),
                "post event retains the validated request traceparent"
            );

            let outbox = PgEventOutbox::new(pool.clone());
            let retry_event = make_event(Uuid::new_v4(), actor_id, PRIVATE_EVENT_PAYLOAD);
            let retry_event = outbox.save(&retry_event).await?;
            let claimed = outbox
                .find_unpublished(Utc::now() + chrono::Duration::seconds(2), 10)
                .await?;
            anyhow::ensure!(claimed.len() == 2, "both fixture events are claimed");

            anyhow::ensure!(
                !outbox
                    .delivery_completed(saved_post_event.event_id, "EXTERNAL")
                    .await?,
                "delivery starts incomplete"
            );
            outbox
                .mark_delivery_completed(
                    saved_post_event.event_id,
                    "EXTERNAL",
                    PRIVATE_DELIVERY_PAYLOAD,
                    Utc::now(),
                )
                .await?;
            anyhow::ensure!(
                outbox
                    .delivery_completed(saved_post_event.event_id, "EXTERNAL")
                    .await?,
                "delivery completion is persisted"
            );
            outbox
                .append_delivery_attempt(&DeliveryAttempt {
                    attempt_id: Uuid::new_v4(),
                    event_id: saved_post_event.event_id,
                    target: DeliveryTarget::External,
                    source: DeliveryAttemptSource::OutboxWorker,
                    resolution_id: None,
                    phase: DeliveryAttemptPhase::Succeeded,
                    occurred_at: Utc::now(),
                })
                .await?;
            outbox
                .mark_published(saved_post_event.event_id, Utc::now())
                .await?;
            outbox
                .record_publish_failure(retry_event.event_id, PRIVATE_PUBLISH_ERROR, 3)
                .await?;

            Ok::<_, anyhow::Error>((request_context, saved_post_event))
        }
    }));
    let request = http::Request::builder()
        .uri("/post-outbox-trace-test")
        .header(trace_context::TRACEPARENT_HEADER, INBOUND_TRACEPARENT)
        .header(trace_context::TRACESTATE_HEADER, PRIVATE_STATE)
        .body(())
        .expect("trace fixture request");
    let (request_context, saved_event) = service
        .oneshot(request)
        .with_subscriber(dispatch)
        .await
        .context("run Postgres-backed trace journey")?;

    provider
        .force_flush()
        .context("flush captured database spans")?;
    let spans = exporter
        .get_finished_spans()
        .context("read captured database spans")?;
    let request_span = spans
        .iter()
        .find(|span| {
            span.name == "request"
                && format!("{:016x}", span.span_context.span_id()) == request_context.span_id()
        })
        .context("captured request span")?;
    let database_spans = spans
        .iter()
        .filter(|span| DATABASE_SPAN_NAMES.iter().any(|name| span.name == *name))
        .collect::<Vec<_>>();
    let actual_names = database_spans
        .iter()
        .map(|span| span.name.as_ref())
        .collect::<HashSet<_>>();
    let expected_names = DATABASE_SPAN_NAMES.into_iter().collect::<HashSet<_>>();
    anyhow::ensure!(
        actual_names == expected_names,
        "all reviewed Post database span names are exported"
    );
    anyhow::ensure!(
        database_spans.len() >= DATABASE_SPAN_NAMES.len(),
        "each reviewed Post database operation emitted a span"
    );
    let request_span_id = format!("{:016x}", request_span.span_context.span_id());
    for span in database_spans {
        anyhow::ensure!(
            format!("{:032x}", span.span_context.trace_id()) == request_context.trace_id(),
            "database span shares the validated request trace"
        );
        anyhow::ensure!(
            format!("{:016x}", span.parent_span_id) == request_span_id,
            "database span is a child of the request span"
        );
        anyhow::ensure!(span.events.is_empty(), "database spans export no events");
        anyhow::ensure!(span.links.is_empty(), "database spans export no links");
        anyhow::ensure!(
            span.span_context.trace_state().header().is_empty(),
            "database spans export no private trace state"
        );
        let attributes = format!("{:?}", span.attributes);
        for private_value in [
            PRIVATE_STATE,
            PRIVATE_CONTENT,
            PRIVATE_EVENT_PAYLOAD,
            PRIVATE_DELIVERY_PAYLOAD,
            PRIVATE_PUBLISH_ERROR,
        ] {
            anyhow::ensure!(
                !attributes.contains(private_value),
                "database span excludes private application values"
            );
        }
        anyhow::ensure!(
            !attributes.contains("db.statement") && !attributes.contains("query"),
            "database span excludes SQL and query attributes"
        );
    }

    anyhow::ensure!(
        saved_event.origin_traceparent.as_deref() == Some(request_context.traceparent().as_str()),
        "outbox event continues to store only the validated request parent"
    );
    provider
        .shutdown_with_timeout(std::time::Duration::from_secs(3))
        .context("shut down in-memory trace provider")?;
    Ok(())
}

#[tokio::test]
async fn post_database_spans_are_emitted_with_safe_request_parenting() -> Result<()> {
    let fixture = Fixture::new().await?;
    let result = run_database_trace(fixture.pool.clone()).await;
    fixture.shutdown().await?;
    result
}

#[test]
fn local_database_guard_requires_a_named_loopback_disposable_database() {
    assert!(validate_local_database_url(
        "postgres://test-user@127.0.0.1:5432/port_rust_post_trace_test_isolated"
    )
    .is_ok());
    assert!(validate_local_database_url(
        "postgres://test-user@127.0.0.1:5432/port_rust_post_trace_test_"
    )
    .is_err());
    assert!(validate_local_database_url(
        "postgres://test-user@db.example.invalid:5432/port_rust_post_trace_test_isolated"
    )
    .is_err());
    assert!(validate_local_database_url(
        "postgres://test-user@127.0.0.1:5432/port_rust_post_trace_test_isolated?sslmode=disable"
    )
    .is_err());
}
