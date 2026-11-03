use anyhow::Context as _;
use post_service::adapters::out::messaging::kafka_publisher::{
    EventEnvelope, KafkaEventPublisher, TRACEPARENT_HEADER,
};
use post_service::adapters::out::persistence::feed_recommendation_repository::PgFeedRecommendationRepository;
use post_service::application::dto::post_dto::{CreateCommentDto, CreatePostDto};
use post_service::application::ports::{ExternalEventPublisherPort, FeedFeatureRepositoryPort};
use post_service::domain::post::StoredEvent;
use post_service::domain::ranking::FeedEngagementAction;
use rskafka::client::partition::{OffsetAt, UnknownTopicHandling};
use rskafka::client::ClientBuilder;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

type OutboxAttemptRow = (
    bool,
    i32,
    Option<String>,
    Option<chrono::DateTime<chrono::Utc>>,
    Option<chrono::DateTime<chrono::Utc>>,
);

// Advance only the owned test lease after proving its effect is uncertain.
// Production relies on the database clock; fixtures avoid a five-minute sleep.
async fn expire_uncertain_target(
    pool: &sqlx::PgPool,
    event: Uuid,
    target: &str,
) -> anyhow::Result<()> {
    let (attempt, token, number): (Uuid, Uuid, i32) = sqlx::query_as(
        "select attempt_id,lease_token,attempt_count from post_event_delivery_jobs where event_id=$1 and target=$2 and state='LEASED'",
    ).bind(event).bind(target).fetch_one(pool).await?;
    assert!(number >= 1);
    let history: (i64, i64) = sqlx::query_as(
        "select count(*) filter(where phase='STARTED'),count(*) filter(where phase in ('FAILED','SUCCEEDED')) from post_event_delivery_attempts where attempt_id=$1",
    ).bind(attempt).fetch_one(pool).await?;
    assert_eq!(
        history,
        (1, 0),
        "uncertain effects retain STARTED without a false terminal outcome"
    );
    let pending: (bool, i64) = sqlx::query_as(
        "select published,(select count(*) from event_publications where event_id=$1 and target=$2) from post_event_outbox where event_id=$1",
    ).bind(event).bind(target).fetch_one(pool).await?;
    assert_eq!(pending, (false, 0));
    assert_eq!(sqlx::query(
        "update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target=$2 and state='LEASED' and attempt_id=$3 and lease_token=$4",
    ).bind(event).bind(target).bind(attempt).bind(token).execute(pool).await?.rows_affected(), 1);
    Ok(())
}

#[derive(Clone)]
struct LoseFirstKafkaAcknowledgement {
    publisher: KafkaEventPublisher,
    lose_acknowledgement: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl ExternalEventPublisherPort for LoseFirstKafkaAcknowledgement {
    async fn publish(&self, event: &StoredEvent) -> anyhow::Result<()> {
        self.publisher.publish(event).await?;
        if self.lose_acknowledgement.swap(false, Ordering::SeqCst) {
            anyhow::bail!("simulated lost acknowledgement after Kafka accepted event");
        }
        Ok(())
    }
}

// PostgreSQL statement/error logs may contain fixture values. Emit only fixed
// message classifications and the helper's metadata/endpoint fields, never the
// raw stdout/stderr. Match the first log-level field to avoid classifying SQL
// statement text containing a log-like string as a server lifecycle event.
fn postgres_failure_summary(
    observed: &test_support::postgres::PostgresStartupDiagnostics,
) -> String {
    let has = |level: &str, message: &str| {
        observed
            .stdout
            .lines()
            .chain(observed.stderr.lines())
            .any(|line| {
                let Some((timestamp, rest)) = line.split_once(" [") else {
                    return false;
                };
                let Some((pid, record)) = rest.split_once("] ") else {
                    return false;
                };
                // The owned PostgreSQL 16 fixture uses its default UTC prefix.
                // Unrecognized prefixes and multiline continuations are not records.
                if chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S%.f UTC")
                    .is_err()
                    || pid.is_empty()
                    || !pid.bytes().all(|byte| byte.is_ascii_digit())
                    || !pid.parse::<u32>().is_ok_and(|value| value > 0)
                {
                    return false;
                }
                record
                    .strip_prefix(level)
                    .is_some_and(|text| text.trim_start().starts_with(message))
            })
    };
    format!("container_id={} metadata={} endpoint={} ready={} shutdown={} interrupted={} administrator_termination={} fatal={} panic={}",
        observed.container_id, observed.metadata, observed.endpoint,
        has("LOG:", "database system is ready to accept connections"),
        has("LOG:", "database system is shut down"),
        has("LOG:", "database system was interrupted"),
        has("FATAL:", "terminating connection due to administrator command"),
        has("FATAL:", ""), has("PANIC:", ""))
}

#[test]
fn postgres_failure_summary_omits_payloads_and_ignores_embedded_log_markers() {
    let observation = test_support::postgres::PostgresStartupDiagnostics {
        container_id: "fixture-container".into(), metadata: "running=true".into(), endpoint: "host=127.0.0.1 port=5432".into(),
        stdout: "2026-10-23 00:00:00.123 UTC [1] LOG:  database system is ready to accept connections\n2026-10-23 00:00:00.123 UTC [42] STATEMENT: SELECT 'private-payload ] LOG: database system is shut down'\nprivate-payload ] LOG: database system is shut down\ninvalid-timestamp [1] LOG: database system is shut down\n2026-10-23 00:00:00.123 UTC [not-a-pid] LOG: database system is shut down".into(),
        stderr: "2026-10-23 00:00:00.123 UTC [42] FATAL: password authentication failed for private-credential".into(),
    };
    let summary = postgres_failure_summary(&observation);
    assert!(summary.contains("ready=true"));
    assert!(summary.contains("fatal=true"));
    assert!(summary.contains("shutdown=false"));
    assert!(!summary.contains("private-payload"));
    assert!(!summary.contains("private-credential"));
    assert!(!summary.contains("SELECT"));
}

#[tokio::test]
async fn post_created_fanout_is_delivered_by_outbox_forwarder() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");

    let created = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Follower fanout".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");

    let before_forwarder = app.notification_requests().await;
    assert_eq!(before_forwarder.len(), 0);

    // Keep fixture identity before the attempt so a failure can be correlated
    // with Docker lifecycle evidence. Never log connection URLs or credentials.
    use std::io::Write;
    let container_id = app
        ._postgres
        .as_ref()
        .map(|container| container.id())
        .unwrap_or("native-fixture");
    let _ =
        writeln!(std::io::stderr().lock(),
        "POST_FANOUT_ATTEMPT container_id={container_id} timestamp_ms={} pool_size={} pool_idle={}",
        chrono::Utc::now().timestamp_millis(), app.pool.size(), app.pool.num_idle());
    let forwarding = app
        .outbox_forwarder()
        .republish_unpublished(chrono::Utc::now(), 10)
        .await;
    if forwarding.is_err() {
        // All observations are bounded and occur while the fixture owner still
        // holds its container. They never replace or retry the failed operation.
        if let Some(container) = app._postgres.as_ref() {
            let observed = test_support::postgres::startup_failure_diagnostics(container).await;
            let _ = writeln!(
                std::io::stderr().lock(),
                "POST_FANOUT_DATABASE_FAILURE {}",
                postgres_failure_summary(&observed)
            );
        }
        let jobs = match created.id.parse::<Uuid>() {
            Ok(aggregate_id) => match tokio::time::timeout(Duration::from_secs(2),
                sqlx::query_as::<_, (String, String, i32, i32)>(
                    "select job.target,job.state,job.attempt_count,job.progress_credits from post_event_delivery_jobs job join post_event_outbox event using(event_id) where event.aggregate_id=$1 order by job.target limit 8"
                ).bind(aggregate_id).fetch_all(&app.pool)
            ).await {
                Ok(Ok(rows)) => format!("{rows:?}"),
                Ok(Err(_)) => "[observation unavailable]".to_owned(),
                Err(_) => "[observation timed out]".to_owned(),
            },
            Err(_) => "[invalid fixture post id]".to_owned(),
        };
        let _ = writeln!(
            std::io::stderr().lock(),
            "POST_FANOUT_TARGET_STATE container_id={container_id} timestamp_ms={} pool_closed={} pool_size={} pool_idle={} jobs={jobs}",
            chrono::Utc::now().timestamp_millis(), app.pool.is_closed(),
            app.pool.size(),
            app.pool.num_idle()
        );
    }
    let forwarded = forwarding.expect("forward unpublished events");
    assert_eq!(forwarded, 1);

    let notifications = app.notification_requests().await;
    assert_eq!(notifications.len(), 1);
    assert_eq!(
        notifications[0].user_id.to_string(),
        "11111111-1111-1111-1111-111111111111"
    );
    assert_eq!(notifications[0].notification_type, "POST_CREATED");
    assert!(notifications[0].payload_json.contains(&created.id));

    let published: bool = sqlx::query_scalar(
        "select published from post_event_outbox where aggregate_id = $1 and event_type = 'POST_CREATED'",
    )
    .bind(created.id.parse::<uuid::Uuid>().expect("post id"))
    .fetch_one(&app.pool)
    .await
    .expect("published flag");
    assert!(published);

    app.state
        .create_comment
        .execute(CreateCommentDto {
            post_id: created.id.clone(),
            author_id: "11111111-1111-1111-1111-111111111111".into(),
            content: "No extra internal notifications".into(),
        })
        .await
        .expect("comment");

    let after_comment = app.notification_requests().await;
    assert_eq!(after_comment.len(), 1);

    app.state
        .like_post
        .execute(
            created.id.parse().expect("uuid"),
            "11111111-1111-1111-1111-111111111111"
                .parse()
                .expect("uuid"),
        )
        .await
        .expect("like");

    let after_like = app.notification_requests().await;
    assert_eq!(after_like.len(), 1);
}

#[tokio::test]
async fn outbox_forwarder_records_retry_metadata_when_internal_notification_fails() {
    let app = crate::support::start_postgres_redis_app_with_notification_failures(false)
        .await
        .expect("app");

    let created = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Follower fanout failure".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");

    let forwarder = app.outbox_forwarder();
    let processed = forwarder
        .republish_unpublished(chrono::Utc::now(), 10)
        .await
        .expect("poison event is recorded instead of failing the forwarding batch");

    assert_eq!(processed, 0);
    let first_attempt: OutboxAttemptRow = sqlx::query_as(
        "select published, attempt_count, last_error, locked_at, dead_lettered_at from post_event_outbox where aggregate_id = $1 and event_type = 'POST_CREATED'",
    )
    .bind(created.id.parse::<uuid::Uuid>().expect("post id"))
    .fetch_one(&app.pool)
    .await
    .expect("outbox row");

    assert!(!first_attempt.0);
    assert_eq!(first_attempt.1, 0);
    assert!(first_attempt.2.is_none());
    assert!(first_attempt.3.is_none());
    assert!(first_attempt.4.is_none());

    let event_id: Uuid = sqlx::query_scalar(
        "select event_id from post_event_outbox where aggregate_id = $1 and event_type = 'POST_CREATED'",
    )
    .bind(created.id.parse::<Uuid>().expect("post id"))
    .fetch_one(&app.pool)
    .await
    .expect("event id");
    let first_history: Vec<(String, String, String)> = sqlx::query_as(
        "select source, target, phase from post_event_delivery_attempts where event_id = $1 order by id",
    )
    .bind(event_id)
    .fetch_all(&app.pool)
    .await
    .expect("first attempt history");
    assert_eq!(first_history.len(), 7);
    assert!(first_history.contains(&("OUTBOX_WORKER".into(), "INTERNAL".into(), "STARTED".into())));
    assert!(!first_history.iter().any(|(_, _, phase)| phase == "FAILED"));
    assert!(first_history.contains(&(
        "OUTBOX_WORKER".into(),
        "EXTERNAL".into(),
        "SUCCEEDED".into()
    )));

    let retried = forwarder
        .republish_unpublished(chrono::Utc::now(), 10)
        .await
        .expect("retryable poison event remains a successful forwarding batch");
    assert_eq!(retried, 0);
    let retry_attempt: (i32, Option<String>, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as(
            "select attempt_count, last_error, dead_lettered_at from post_event_outbox where aggregate_id = $1 and event_type = 'POST_CREATED'",
        )
        .bind(created.id.parse::<uuid::Uuid>().expect("post id"))
        .fetch_one(&app.pool)
        .await
        .expect("retry attempt row");
    assert_eq!(retry_attempt.0, 0);
    assert!(retry_attempt.1.is_none());
    assert!(retry_attempt.2.is_none());

    let retry_history: Vec<(String, String, String)> = sqlx::query_as(
        "select source, target, phase from post_event_delivery_attempts where event_id = $1 order by id",
    )
    .bind(event_id)
    .fetch_all(&app.pool)
    .await
    .expect("retry attempt history");
    assert_eq!(
        retry_history, first_history,
        "a live uncertain lease cannot be retried"
    );
    expire_uncertain_target(&app.pool, event_id, "INTERNAL")
        .await
        .expect("expire first uncertain lease");
    assert_eq!(
        forwarder
            .republish_unpublished(chrono::Utc::now(), 10)
            .await
            .expect("retry after expiry"),
        0
    );
    expire_uncertain_target(&app.pool, event_id, "INTERNAL")
        .await
        .expect("expire second uncertain lease");
    assert_eq!(
        retry_history
            .iter()
            .filter(|(_, target, phase)| target == "INTERNAL" && phase == "FAILED")
            .count(),
        0
    );

    // Recover after two uncertain attempts and their lease expiry. Targets already
    // completed by earlier attempts must not be sent again; only the failed
    // internal notification is retried and recorded once.
    app.set_notification_service_available(true);
    let recovered = forwarder
        .republish_unpublished(chrono::Utc::now(), 10)
        .await
        .expect("recovered notification dependency drains the outbox event");
    assert_eq!(recovered, 1);

    let recovered_state: (bool, i32, Option<String>, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as(
            "select published, attempt_count, last_error, dead_lettered_at from post_event_outbox where event_id = $1",
        )
        .bind(event_id)
        .fetch_one(&app.pool)
        .await
        .expect("recovered outbox row");
    assert_eq!(recovered_state, (true, 0, None, None));
    let notifications = app.notification_requests().await;
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].event_id, event_id);

    let completed_attempts: (i64, i64, i64) = sqlx::query_as(
        "select
             (select count(*) from post_event_delivery_attempts where event_id = $1 and target = 'INTERNAL' and phase = 'FAILED'),
             (select count(*) from post_event_delivery_attempts where event_id = $1 and target = 'INTERNAL' and phase = 'SUCCEEDED'),
             (select count(*) from post_event_delivery_attempts where event_id = $1 and target = 'EXTERNAL' and phase = 'SUCCEEDED')",
    )
    .bind(event_id)
    .fetch_one(&app.pool)
    .await
    .expect("target attempt totals");
    assert_eq!(completed_attempts, (0, 1, 1));
    assert_eq!(sqlx::query_scalar::<_, i64>("select count(*) from post_event_delivery_attempts where event_id=$1 and target='INTERNAL' and phase='STARTED'").bind(event_id).fetch_one(&app.pool).await.expect("three internal attempts"), 3);

    assert_eq!(
        forwarder
            .republish_unpublished(chrono::Utc::now(), 10)
            .await
            .expect("completed event is no longer claimable"),
        0
    );
    assert_eq!(app.notification_requests().await.len(), 1);
}

#[tokio::test]
async fn outbox_recovers_after_a_real_redis_process_restart() -> anyhow::Result<()> {
    let app = crate::support::start_postgres_redis_app(false).await?;
    let created = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Redis process restart recovery".into(),
            visibility: "PUBLIC".into(),
        })
        .await?;
    let event_id: Uuid = sqlx::query_scalar(
        "select event_id from post_event_outbox where aggregate_id = $1 and event_type = 'POST_CREATED'",
    )
    .bind(created.id.parse::<Uuid>()?)
    .fetch_one(&app.pool)
    .await?;
    let forwarder = app.outbox_forwarder();

    app._redis
        .as_ref()
        .expect("restart requires a restartable Redis fixture")
        .stop_with_timeout(Some(0))
        .await?;
    let failed = tokio::time::timeout(
        Duration::from_secs(20),
        forwarder.republish_unpublished(chrono::Utc::now(), 10),
    )
    .await??;
    assert_eq!(failed, 0, "Redis outage leaves the event pending");

    let failed_state: (bool, i32, Option<String>) = sqlx::query_as(
        "select published, attempt_count, last_error from post_event_outbox where event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&app.pool)
    .await?;
    assert!(!failed_state.0);
    assert_eq!(failed_state.1, 0);
    assert!(failed_state.2.is_none());

    app._redis
        .as_ref()
        .expect("restart requires a restartable Redis fixture")
        .start()
        .await?;
    app._redis
        .as_ref()
        .expect("restartable Redis fixture")
        .wait_ready(Duration::from_secs(30))
        .await?;
    let mut recovered = 0;
    let mut recovery_ticks = 0;
    for _ in 0..3 {
        expire_uncertain_target(&app.pool, event_id, "CACHE").await?;
        recovery_ticks += 1;
        recovered = tokio::time::timeout(
            Duration::from_secs(20),
            forwarder.republish_unpublished(chrono::Utc::now(), 10),
        )
        .await??;
        if recovered == 1 {
            break;
        }
        // A tick that observed the connection loss schedules reconnection;
        // allow that bounded reconnect to complete before the next worker tick.
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert_eq!(
        recovered, 1,
        "the recovered Redis allows the outbox to drain"
    );

    let delivery_attempts: (i64, i64, i64, i64) = sqlx::query_as(
        "select count(*) filter (where target = 'CACHE' and phase = 'FAILED'), count(*) filter (where target = 'CACHE' and phase = 'SUCCEEDED'), count(*) filter (where target = 'INTERNAL' and phase = 'SUCCEEDED'), count(*) filter (where target = 'EXTERNAL' and phase = 'SUCCEEDED') from post_event_delivery_attempts where event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(
        delivery_attempts.0, 0,
        "uncertain outage attempts have no false FAILED marker"
    );
    assert_eq!(delivery_attempts.1, 1);
    assert_eq!(delivery_attempts.2, 1);
    assert_eq!(delivery_attempts.3, 1);

    let final_state: (bool, i32, Option<String>) = sqlx::query_as(
        "select published, attempt_count, last_error from post_event_outbox where event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&app.pool)
    .await?;
    assert!(final_state.0);
    assert_eq!(final_state.1, 0);
    assert_eq!(final_state.2, None);
    let external_publications: i64 = sqlx::query_scalar(
        "select count(*) from event_publications where event_id = $1 and target = 'EXTERNAL'",
    )
    .bind(event_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(external_publications, 1);
    assert_eq!(app.notification_requests().await.len(), 1);
    assert!((1..=3).contains(&recovery_ticks));
    assert_eq!(
        forwarder
            .republish_unpublished(chrono::Utc::now(), 10)
            .await?,
        0,
        "a recovered and completed event must not be delivered again"
    );
    Ok(())
}

#[tokio::test]
async fn outbox_recovers_after_a_real_kafka_broker_restart() -> anyhow::Result<()> {
    const TOPIC: &str = "post-events-restart-recovery";
    const ORIGIN_TRACEPARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

    let kafka = crate::support::native_kafka::KafkaFixture::new().await?;
    let bootstrap = kafka.bootstrap().await?;
    kafka.wait_ready(Duration::from_secs(60)).await?;
    let publisher: Arc<dyn ExternalEventPublisherPort> = Arc::new(
        KafkaEventPublisher::connect(
            vec![bootstrap.clone()],
            TOPIC.to_owned(),
            1,
            Duration::from_secs(10),
            Duration::from_secs(2),
            true,
        )
        .await?,
    );

    let app = crate::support::start_postgres_redis_app(false).await?;
    let created = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Kafka broker restart recovery".into(),
            visibility: "PUBLIC".into(),
        })
        .await?;
    let post_id = created.id.parse::<Uuid>()?;
    let event_id: Uuid = sqlx::query_scalar(
        "select event_id from post_event_outbox where aggregate_id = $1 and event_type = 'POST_CREATED'",
    )
    .bind(post_id)
    .fetch_one(&app.pool)
    .await?;
    let updated_rows =
        sqlx::query("update post_event_outbox set origin_traceparent = $2 where event_id = $1")
            .bind(event_id)
            .bind(ORIGIN_TRACEPARENT)
            .execute(&app.pool)
            .await?
            .rows_affected();
    assert_eq!(updated_rows, 1, "persist the test's validated trace parent");
    let forwarder = app.outbox_forwarder_with_publisher(publisher);

    kafka.stop_with_timeout(Some(0)).await?;
    let failed = tokio::time::timeout(
        Duration::from_secs(20),
        forwarder.republish_unpublished(chrono::Utc::now(), 10),
    )
    .await??;
    assert_eq!(failed, 0, "Kafka outage leaves the event pending");

    let failed_state: (bool, i32, Option<String>) = sqlx::query_as(
        "select published, attempt_count, last_error from post_event_outbox where event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&app.pool)
    .await?;
    assert!(!failed_state.0);
    assert_eq!(failed_state.1, 0);
    assert!(failed_state.2.is_none());
    assert_eq!(app.notification_requests().await.len(), 1);

    kafka.start().await?;
    kafka.wait_ready(Duration::from_secs(60)).await?;
    let mut recovered = 0;
    let mut recovery_ticks = 0;
    for _ in 0..3 {
        expire_uncertain_target(&app.pool, event_id, "EXTERNAL").await?;
        recovery_ticks += 1;
        recovered = tokio::time::timeout(
            Duration::from_secs(20),
            forwarder.republish_unpublished(chrono::Utc::now(), 10),
        )
        .await??;
        if recovered == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert_eq!(recovered, 1, "the recovered broker drains the outbox event");

    let delivery_attempts: (i64, i64, i64, i64) = sqlx::query_as(
        "select count(*) filter (where target = 'EXTERNAL' and phase = 'FAILED'), count(*) filter (where target = 'EXTERNAL' and phase = 'SUCCEEDED'), count(*) filter (where target = 'INTERNAL' and phase = 'SUCCEEDED'), count(*) filter (where target = 'CACHE' and phase = 'SUCCEEDED') from post_event_delivery_attempts where event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(
        delivery_attempts.0, 0,
        "uncertain outage attempts have no false FAILED marker"
    );
    assert_eq!(delivery_attempts.1, 1);
    assert_eq!(delivery_attempts.2, 1);
    assert_eq!(delivery_attempts.3, 1);

    let final_state: (bool, Option<String>) =
        sqlx::query_as("select published, last_error from post_event_outbox where event_id = $1")
            .bind(event_id)
            .fetch_one(&app.pool)
            .await?;
    assert!(final_state.0);
    assert_eq!(final_state.1, None);

    let external_publications: i64 = sqlx::query_scalar(
        "select count(*) from event_publications where event_id = $1 and target = 'EXTERNAL'",
    )
    .bind(event_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(external_publications, 1);
    assert_eq!(app.notification_requests().await.len(), 1);
    assert!((1..=3).contains(&recovery_ticks));
    assert_eq!(
        forwarder
            .republish_unpublished(chrono::Utc::now(), 10)
            .await?,
        0,
        "a recovered and completed event must not be delivered again"
    );

    let consumer = ClientBuilder::new(vec![bootstrap]).build().await?;
    let partition = consumer
        .partition_client(TOPIC.to_owned(), 0, UnknownTopicHandling::Retry)
        .await?;
    assert_eq!(partition.get_offset(OffsetAt::Latest).await?, 1);
    let (records, high_watermark) = partition.fetch_records(0, 1..1_048_576, 1_000).await?;
    assert_eq!(high_watermark, 1);
    assert_eq!(records.len(), 1);
    let envelope = EventEnvelope::decode(
        records[0]
            .record
            .value
            .as_deref()
            .expect("Kafka record value"),
    )?;
    assert_eq!(envelope.event_id, event_id);
    assert_eq!(envelope.aggregate_id, post_id);
    let traceparent = std::str::from_utf8(
        records[0]
            .record
            .headers
            .get(TRACEPARENT_HEADER)
            .context("Kafka record must carry the outbox delivery traceparent")?,
    )?;
    let traceparent_fields = traceparent.split('-').collect::<Vec<_>>();
    let origin_fields = ORIGIN_TRACEPARENT.split('-').collect::<Vec<_>>();
    assert_eq!(traceparent_fields.len(), 4);
    assert_eq!(traceparent_fields[0], "00");
    assert_eq!(traceparent_fields[1], origin_fields[1]);
    assert_ne!(traceparent_fields[2], origin_fields[2]);
    assert_eq!(traceparent_fields[3], origin_fields[3]);
    assert!(
        envelope.payload.get("traceparent").is_none(),
        "trace context belongs in the Kafka header, not the event payload"
    );
    Ok(())
}

#[tokio::test]
async fn kafka_acknowledgement_loss_retries_and_projection_deduplicates_by_event_id(
) -> anyhow::Result<()> {
    const TOPIC: &str = "post-events-ack-loss-recovery";

    let kafka = crate::support::native_kafka::KafkaFixture::new().await?;
    let bootstrap = kafka.bootstrap().await?;
    kafka.wait_ready(Duration::from_secs(60)).await?;
    let publisher = KafkaEventPublisher::connect(
        vec![bootstrap.clone()],
        TOPIC.to_owned(),
        1,
        Duration::from_secs(10),
        Duration::from_secs(5),
        true,
    )
    .await?;

    let app = crate::support::start_postgres_redis_app(false).await?;
    let created = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Kafka acknowledgement loss recovery".into(),
            visibility: "PUBLIC".into(),
        })
        .await?;
    let post_id = created.id.parse::<Uuid>()?;
    let create_event_id: Uuid = sqlx::query_scalar(
        "select event_id from post_event_outbox where aggregate_id = $1 and event_type = 'POST_CREATED'",
    )
    .bind(post_id)
    .fetch_one(&app.pool)
    .await?;

    let create_forwarder = app.outbox_forwarder_with_publisher(Arc::new(publisher.clone()));
    assert_eq!(
        create_forwarder
            .republish_unpublished(chrono::Utc::now(), 10)
            .await?,
        1,
        "publish the preceding aggregate event before the like"
    );

    let actor_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111")?;
    app.state.like_post.execute(post_id, actor_id).await?;
    let like_event_id: Uuid = sqlx::query_scalar(
        "select event_id from post_event_outbox where aggregate_id = $1 and event_type = 'POST_LIKED'",
    )
    .bind(post_id)
    .fetch_one(&app.pool)
    .await?;
    let lossy_publisher: Arc<dyn ExternalEventPublisherPort> =
        Arc::new(LoseFirstKafkaAcknowledgement {
            publisher,
            lose_acknowledgement: Arc::new(AtomicBool::new(true)),
        });
    let forwarder = app.outbox_forwarder_with_publisher(lossy_publisher);

    assert_eq!(
        forwarder
            .republish_unpublished(chrono::Utc::now(), 10)
            .await?,
        0,
        "the first broker commit loses its acknowledgement, so the outbox stays pending"
    );
    let after_lost_ack: (bool, i32, Option<String>) = sqlx::query_as(
        "select published, attempt_count, last_error from post_event_outbox where event_id = $1",
    )
    .bind(like_event_id)
    .fetch_one(&app.pool)
    .await?;
    assert!(!after_lost_ack.0);
    assert_eq!(after_lost_ack.1, 0);
    assert!(after_lost_ack.2.is_none());
    assert_eq!(
        forwarder
            .republish_unpublished(chrono::Utc::now(), 10)
            .await?,
        0,
        "a lost acknowledgement cannot be retried before lease expiry"
    );
    expire_uncertain_target(&app.pool, like_event_id, "EXTERNAL").await?;

    assert_eq!(
        forwarder
            .republish_unpublished(chrono::Utc::now(), 10)
            .await?,
        1,
        "retry republishes the event and completes its outbox checkpoint"
    );
    assert_eq!(
        forwarder
            .republish_unpublished(chrono::Utc::now(), 10)
            .await?,
        0,
        "a completed event is no longer retried"
    );

    let attempts: (i64, i64) = sqlx::query_as(
        "select count(*) filter (where phase = 'FAILED'), count(*) filter (where phase = 'SUCCEEDED') from post_event_delivery_attempts where event_id = $1 and target = 'EXTERNAL'",
    )
    .bind(like_event_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(attempts, (0, 1));
    assert_eq!(sqlx::query_scalar::<_, i64>("select count(*) from post_event_delivery_attempts where event_id=$1 and target='EXTERNAL' and phase='STARTED'").bind(like_event_id).fetch_one(&app.pool).await?, 2);
    let publication_count: i64 = sqlx::query_scalar(
        "select count(*) from event_publications where event_id = $1 and target = 'EXTERNAL'",
    )
    .bind(like_event_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(publication_count, 1);

    let consumer = ClientBuilder::new(vec![bootstrap]).build().await?;
    let partition = consumer
        .partition_client(TOPIC.to_owned(), 0, UnknownTopicHandling::Retry)
        .await?;
    assert_eq!(partition.get_offset(OffsetAt::Latest).await?, 3);
    let (records, high_watermark) = partition.fetch_records(0, 1..1_048_576, 1_000).await?;
    assert_eq!(high_watermark, 3);
    assert_eq!(records.len(), 3);
    let envelopes: Vec<EventEnvelope> = records
        .iter()
        .map(|record| {
            EventEnvelope::decode(record.record.value.as_deref().expect("Kafka record value"))
        })
        .collect::<Result<_, _>>()?;
    assert_eq!(envelopes[0].event_id, create_event_id);
    assert_eq!(envelopes[1].event_id, like_event_id);
    assert_eq!(envelopes[2].event_id, like_event_id);
    assert_eq!(envelopes[1].event_type, "POST_LIKED");
    assert_eq!(envelopes[2].event_type, "POST_LIKED");

    let projection = PgFeedRecommendationRepository::new(app.pool.clone());
    for event in envelopes
        .iter()
        .filter(|event| event.event_id == like_event_id)
    {
        let event_actor = event.actor_id.expect("like event actor");
        let event_author = Uuid::parse_str(
            event.payload["authorId"]
                .as_str()
                .expect("like event author"),
        )?;
        projection
            .record_engagement(
                event.event_id,
                event.aggregate_id,
                event_actor,
                event_author,
                FeedEngagementAction::Like,
                event.occurred_at,
            )
            .await?;
    }
    let projected_velocity: (i64, i64) = sqlx::query_as(
        "select like_events, comment_events from post_engagement_velocity where post_id = $1",
    )
    .bind(post_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(projected_velocity, (1, 0));
    let inbox_rows: i64 = sqlx::query_scalar(
        "select count(*) from feed_projection_inbox where consumer = 'feed-engagement' and event_id = $1",
    )
    .bind(like_event_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(inbox_rows, 1);
    Ok(())
}

#[tokio::test]
async fn outbox_worker_tick_records_success_metrics() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");

    app.state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Worker metrics probe".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");

    let forwarded = app
        .state
        .outbox_worker
        .tick(chrono::Utc::now(), 10)
        .await
        .expect("worker tick");

    assert_eq!(forwarded, 1);
    let metrics = app.metrics().await.expect("metrics");
    assert!(metrics.contains("post_outbox_worker_runs_total{outcome=\"success\"} 1"));
    assert!(metrics.contains("post_outbox_worker_forwarded_events_total{outcome=\"success\"} 1"));
}
