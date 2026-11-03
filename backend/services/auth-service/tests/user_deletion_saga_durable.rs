//! The 202 account-deletion response is sent only after its cross-service work
//! has been committed to PostgreSQL. A fresh worker must recover the work after
//! the process that claimed it disappears. The default fixture uses
//! Testcontainers; local PostgreSQL requires an isolated database URL and an
//! explicit write opt-in (`AUTH_USER_DELETION_TEST_ALLOW_LOCAL_PG=1`), then
//! gives each fixture a private schema. The database name must start with
//! `port_rust_auth_test_`.

#[path = "support/owned_database.rs"]
mod owned_database;

use anyhow::Context;
use async_trait::async_trait;
use auth_service::adapters::out::cache::session_store::PgRefreshTokenStore;
use auth_service::adapters::out::persistence::auth_lifecycle_cleanup_repository::PgAuthLifecycleCleanupRepository;
use auth_service::adapters::out::persistence::user_deletion_saga_repository::PgUserDeletionSagaRepository;
use auth_service::adapters::out::persistence::user_repository::{
    PgAuthAuditRepository, PgUserRepository,
};
use auth_service::adapters::r#in::messaging::mail_outbox_worker::{
    EmailOutboxWorker, MailDestination,
};
use auth_service::adapters::r#in::messaging::user_deletion_worker::{
    UserDeletionWorker, UserDeletionWorkerPolicy,
};
use auth_service::application::auth_transactions::AuthTransactionsPort;
use auth_service::application::dto::auth_dto::{DeleteUserDto, RetryUserDeletionDto};
use auth_service::application::ports::{
    AuthLifecycleCleanupPort, RefreshTokenStorePort, UserDeletionDispatcherPort,
    UserDeletionSagaPort, UserRepositoryPort,
};
use auth_service::application::use_cases::delete_user::DeleteUserUseCase;
use auth_service::application::use_cases::retry_user_deletion::RetryUserDeletionUseCase;
use auth_service::domain::session::RefreshTokenRecord;
use auth_service::domain::user::{User, UserStatus};
use auth_service::domain::user_deletion::{UserDeletionSagaStatus, UserDeletionTarget};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use chrono::{Duration, Utc};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::{InMemorySpanExporter, Sampler, SdkTracerProvider};
use platform::trace_context::{self, TraceContext};
use serde_json::Value;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::collections::HashSet;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use tracing::instrument::WithSubscriber as _;
use tracing::Instrument as _;
use tracing_subscriber::prelude::*;
use uuid::Uuid;

const LOCAL_DATABASE_URL_ENV: &str = "AUTH_USER_DELETION_TEST_DATABASE_URL";
const LOCAL_DATABASE_OPT_IN_ENV: &str = "AUTH_USER_DELETION_TEST_ALLOW_LOCAL_PG";
const LOCAL_DATABASE_PREFIX: &str = "port_rust_auth_test";
const LOCAL_SCHEMA_PREFIX: &str = "auth_deletion_fixture_";

type DispatchObservation = (UserDeletionTarget, Option<TraceContext>);

struct Fixture {
    _postgres: Option<ContainerAsync<Postgres>>,
    local_schema: Option<LocalSchema>,
    pool: PgPool,
    database_url: String,
}

struct LocalSchema {
    admin_pool: PgPool,
    name: String,
}

impl Fixture {
    async fn new() -> anyhow::Result<Self> {
        let (postgres, database_url, local_schema) = if let Some(url) =
            owned_database::native_database_url().await?
        {
            (None, url, None)
        } else {
            match local_database_url()? {
                Some(database_url) => {
                    let (local_schema, fixture_url) = provision_local_schema(&database_url).await?;
                    (None, fixture_url, Some(local_schema))
                }
                None => {
                    let (postgres, fixture_url) =
                        test_support::postgres::start_postgres_with_url().await?;
                    (Some(postgres), fixture_url, None)
                }
            }
        };
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&database_url)
            .await
            .context("connect account-deletion PostgreSQL fixture")?;
        let migrations = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
        sqlx::migrate::Migrator::new(migrations.as_path())
            .await
            .context("load account-deletion PostgreSQL migrations")?
            .run(&pool)
            .await
            .context("migrate account-deletion PostgreSQL fixture")?;
        Ok(Self {
            _postgres: postgres,
            local_schema,
            pool,
            database_url,
        })
    }

    async fn independent_pool(&self) -> anyhow::Result<PgPool> {
        PgPoolOptions::new()
            .max_connections(4)
            .connect(&self.database_url)
            .await
            .context("connect restarted account-deletion worker pool")
    }

    async fn shutdown(mut self) -> anyhow::Result<()> {
        self.pool.close().await;
        if let Some(local_schema) = self.local_schema.take() {
            anyhow::ensure!(
                local_schema.name.starts_with(LOCAL_SCHEMA_PREFIX),
                "unsafe account-deletion fixture schema"
            );
            sqlx::query(&format!(
                "drop schema if exists {} cascade",
                quote_identifier(&local_schema.name)
            ))
            .execute(&local_schema.admin_pool)
            .await
            .context("drop local account-deletion fixture schema")?;
            local_schema.admin_pool.close().await;
        }
        Ok(())
    }
}

fn local_database_url() -> anyhow::Result<Option<String>> {
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
        (Some(_), _) => anyhow::bail!(
            "local account-deletion PostgreSQL requires {LOCAL_DATABASE_OPT_IN_ENV}=1"
        ),
        (None, Some(_)) => {
            anyhow::bail!("{LOCAL_DATABASE_OPT_IN_ENV} requires {LOCAL_DATABASE_URL_ENV}")
        }
    }
}

fn validate_local_database_url(database_url: &str) -> anyhow::Result<PgConnectOptions> {
    anyhow::ensure!(
        !database_url.contains('?') && !database_url.contains('#'),
        "local account-deletion URL must not contain connection overrides"
    );
    let (_, remainder) = database_url
        .split_once("://")
        .ok_or_else(|| anyhow::anyhow!("invalid local account-deletion database URL"))?;
    let (_, database) = remainder
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("local account-deletion URL must name a test database"))?;
    anyhow::ensure!(
        database.starts_with(&format!("{LOCAL_DATABASE_PREFIX}_"))
            && database.len() <= 63
            && database
                .bytes()
                .all(|byte| { byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' }),
        "local account-deletion database must use the isolated {LOCAL_DATABASE_PREFIX}_* name"
    );
    let options: PgConnectOptions = database_url
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid local account-deletion database URL"))?;
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
        "loopback TCP is required"
    );
    anyhow::ensure!(
        options.get_database() == Some(database),
        "unexpected account-deletion test database"
    );
    anyhow::ensure!(
        options.get_options().is_none(),
        "local account-deletion URL must not override connection options"
    );
    Ok(options)
}

async fn provision_local_schema(database_url: &str) -> anyhow::Result<(LocalSchema, String)> {
    let options = validate_local_database_url(database_url)?;
    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await
        .map_err(|_| anyhow::anyhow!("failed to connect to local account-deletion PostgreSQL"))?;
    let schema = format!("{LOCAL_SCHEMA_PREFIX}{}", Uuid::new_v4().simple());
    sqlx::query(&format!("create schema {}", quote_identifier(&schema)))
        .execute(&admin_pool)
        .await?;
    let fixture_url = format!(
        "{database_url}?application_name={schema}&options=-c%20search_path%3D{schema}%2Cpg_catalog"
    );
    Ok((
        LocalSchema {
            admin_pool,
            name: schema,
        },
        fixture_url,
    ))
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[derive(Clone, Default)]
struct RecordingDispatcher {
    calls: Arc<Mutex<Vec<DispatchObservation>>>,
}

#[async_trait]
impl UserDeletionDispatcherPort for RecordingDispatcher {
    async fn apply(
        &self,
        target: auth_service::application::dto::deletion_dto::AuthUserDeletionTargetDto,
        _saga_id: Uuid,
        _user_id: Uuid,
        _actor_id: Uuid,
        _retention_until: chrono::DateTime<Utc>,
    ) -> auth_service::application::errors::AuthResult<()> {
        let target = auth_service::domain::user_deletion::UserDeletionTarget::from(target);

        let result: anyhow::Result<_> = async {
            self.calls
                .lock()
                .expect("dispatcher lock")
                .push((target, trace_context::current()));
            Ok(())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
}

#[derive(Clone, Default)]
struct MailTraceReceiver {
    requests: Arc<Mutex<Vec<(bool, Value)>>>,
}

async fn receive_mail_trace(
    State(receiver): State<MailTraceReceiver>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> StatusCode {
    let mut requests = receiver.requests.lock().expect("mail receiver lock");
    requests.push((headers.contains_key("traceparent"), body));
    if requests.len() == 1 {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    }
}

#[tokio::test]
async fn mail_outbox_trace_follows_retries_and_is_not_sent_to_the_provider() -> anyhow::Result<()> {
    const PARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

    let fixture = Fixture::new().await?;
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .with_sampler(Sampler::AlwaysOn)
        .build();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("auth-mail-outbox-test")));
    let dispatch = tracing::Dispatch::new(subscriber);
    let receiver = MailTraceReceiver::default();
    let mut server = None;

    let test_result = async {
        let user = User {
            id: Uuid::new_v4(),
            primary_email: "mail-trace@example.invalid".into(),
            password_hash: "stored-hash".into(),
            status: UserStatus::PendingVerification,
            last_login_at: None,
        };
        let mut request_context =
            TraceContext::child_from_traceparent(PARENT).expect("valid request traceparent");
        let request_span = tracing::info_span!(
            target: "platform::trace_context",
            "request",
            trace_id = tracing::field::Empty,
            span_id = tracing::field::Empty,
            parent_span_id = tracing::field::Empty,
            sampled = tracing::field::Empty,
        );
        request_context.bind_to_span(&request_span);
        request_context.record_on_span(&request_span);
        let expected_trace_id = request_context.trace_id().to_owned();
        let expected_request_span_id = request_context.span_id().to_owned();
        let origin_traceparent = request_context.traceparent();
        let users = PgUserRepository::new(fixture.pool.clone(), std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime));

        trace_context::scope(
            request_context.clone(),
            users
                .register_email(&(&user).into(), "123456")
                .instrument(request_span.clone()),
        )
        .await
        .context("register user and persist mail trace parent")?;
        drop(request_span);

        let persisted: Option<String> = sqlx::query_scalar(
            "select origin_traceparent from auth_mail_outbox where user_id = $1 and kind = 'VERIFY_EMAIL'",
        )
        .bind(user.id)
        .fetch_one(&fixture.pool)
        .await
        .context("read persisted mail trace parent")?;
        anyhow::ensure!(persisted.as_deref() == Some(origin_traceparent.as_str()));

        let router = Router::new()
            .route("/mail", post(receive_mail_trace))
            .with_state(receiver.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .context("bind local mail provider")?;
        let endpoint = format!("http://{}/mail", listener.local_addr()?);
        server = Some(tokio::spawn(async move { axum::serve(listener, router).await }));
        let worker = EmailOutboxWorker::new(
            fixture.pool.clone(),
            Some(MailDestination::new(&endpoint, "test-mail-token", false)?),
            None,
        )?;

        anyhow::ensure!(worker.run_once().await? == 0, "first provider attempt retries");
        let retried_traceparent: Option<String> = sqlx::query_scalar(
            "select origin_traceparent from auth_mail_outbox where user_id = $1 and kind = 'VERIFY_EMAIL'",
        )
        .bind(user.id)
        .fetch_one(&fixture.pool)
        .await?;
        anyhow::ensure!(retried_traceparent.as_deref() == Some(origin_traceparent.as_str()));
        sqlx::query(
            "update auth_mail_outbox set next_attempt_at = now() where user_id = $1 and kind = 'VERIFY_EMAIL'",
        )
        .bind(user.id)
        .execute(&fixture.pool)
        .await?;
        anyhow::ensure!(worker.run_once().await? == 1, "retry delivery succeeds");

        let (traceparent, payload_scrubbed, delivered, attempts): (
            Option<String>,
            bool,
            bool,
            i32,
        ) = sqlx::query_as(
            "select origin_traceparent, payload_json is null, delivered_at is not null, attempts from auth_mail_outbox where user_id = $1 and kind = 'VERIFY_EMAIL'",
        )
        .bind(user.id)
        .fetch_one(&fixture.pool)
        .await
        .context("verify terminal mail row cleanup")?;
        anyhow::ensure!(traceparent.is_none());
        anyhow::ensure!(payload_scrubbed && delivered && attempts == 2);

        let requests = receiver.requests.lock().expect("mail receiver requests");
        anyhow::ensure!(requests.len() == 2);
        anyhow::ensure!(requests.iter().all(|(has_traceparent, _)| !has_traceparent));
        anyhow::ensure!(requests[0].1["verification_code"] == "123456");
        drop(requests);

        provider.force_flush().context("flush mail delivery spans")?;
        let spans = exporter
            .get_finished_spans()
            .context("read exported mail delivery spans")?;
        let request = spans
            .iter()
            .find(|span| span.name == "request")
            .context("exported request parent span")?;
        let deliveries: Vec<_> = spans
            .iter()
            .filter(|span| span.name == "auth_mail_delivery")
            .collect();
        anyhow::ensure!(deliveries.len() == 2);
        for delivery in deliveries {
            anyhow::ensure!(
                format!("{:032x}", delivery.span_context.trace_id()) == expected_trace_id
            );
            anyhow::ensure!(delivery.parent_span_id == request.span_context.span_id());
            anyhow::ensure!(delivery.links.is_empty());
            let attributes = format!("{:?}", delivery.attributes);
            let events = format!("{:?}", delivery.events);
            anyhow::ensure!(attributes.contains("verification"));
            anyhow::ensure!(attributes.contains("retry_scheduled") || attributes.contains("delivered"));
            for sensitive in [
                user.primary_email.as_str(),
                "123456",
                "test-mail-token",
                endpoint.as_str(),
            ] {
                anyhow::ensure!(!attributes.contains(sensitive));
                anyhow::ensure!(!events.contains(sensitive));
            }
        }
        anyhow::ensure!(
            format!("{:016x}", request.span_context.span_id()) == expected_request_span_id
        );

        Ok::<_, anyhow::Error>(())
    }
    .with_subscriber(dispatch)
    .await;

    if let Some(server) = server.take() {
        server.abort();
        let _ = server.await;
    }
    let cleanup_result = fixture.shutdown().await;
    let shutdown_result = provider.shutdown_with_timeout(std::time::Duration::from_secs(3));
    test_result?;
    cleanup_result?;
    shutdown_result.context("shut down mail trace exporter")?;
    Ok(())
}

#[tokio::test]
async fn accepted_deletion_is_durable_and_a_fresh_worker_recovers_expired_leases(
) -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let user_id = Uuid::new_v4();
    let user = User {
        id: user_id,
        primary_email: "durable-deletion@example.invalid".into(),
        password_hash: "stored-hash".into(),
        status: UserStatus::Active,
        last_login_at: None,
    };
    let users = PgUserRepository::new(
        fixture.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    users.save(&(&user).into()).await?;

    let refresh_tokens = PgRefreshTokenStore::new(
        fixture.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    refresh_tokens
        .store(
            &(&RefreshTokenRecord {
                user_id,
                jti: "deletion-session".into(),
                token_hash: "deletion-session-hash".into(),
                family_id: user_id,
                expires_at: Utc::now() + Duration::hours(1),
            })
                .into(),
        )
        .await?;

    let saga_repository = Arc::new(PgUserDeletionSagaRepository::new(fixture.pool.clone()));
    let use_case = DeleteUserUseCase::new(
        users.clone(),
        Arc::new(PgAuthAuditRepository::new(fixture.pool.clone())),
        saga_repository.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    let accepted_at = Utc::now();
    let request_context = TraceContext::child_from_traceparent(
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
    )
    .expect("valid request traceparent");
    let origin_traceparent = request_context.traceparent();
    let accepted = trace_context::scope(
        request_context.clone(),
        use_case.execute(DeleteUserDto {
            user_id: user_id.to_string(),
            actor_id: user_id.to_string(),
            request_id: Some("durable-delete-request".into()),
            ip_address: Some("203.0.113.7".into()),
            user_agent: Some("durable-saga-test".into()),
        }),
    )
    .await?;

    assert!(accepted.deleted);
    assert_eq!(accepted.saga.status, "IN_PROGRESS");
    assert_eq!(accepted.saga.steps.len(), UserDeletionTarget::ALL.len());
    let saga_id = Uuid::parse_str(&accepted.saga_id)?;
    assert_eq!(accepted.saga.id, saga_id.to_string());
    let serialized_saga = serde_json::to_value(&accepted.saga)?;
    assert!(serialized_saga.get("origin_traceparent").is_none());

    // These rows are the acceptance receipt: the user transition, audit, saga,
    // and every target step commit before the use case returns its 202 payload.
    let persisted_status: String = sqlx::query_scalar("select status from users where id = $1")
        .bind(user_id)
        .fetch_one(&fixture.pool)
        .await?;
    let saga_count: i64 = sqlx::query_scalar(
        "select count(*) from user_deletion_sagas where id = $1 and status = 'IN_PROGRESS'",
    )
    .bind(saga_id)
    .fetch_one(&fixture.pool)
    .await?;
    let persisted_traceparent: Option<String> =
        sqlx::query_scalar("select origin_traceparent from user_deletion_sagas where id = $1")
            .bind(saga_id)
            .fetch_one(&fixture.pool)
            .await?;
    let pending_steps: i64 = sqlx::query_scalar(
        "select count(*) from user_deletion_steps where saga_id = $1 and status = 'PENDING'",
    )
    .bind(saga_id)
    .fetch_one(&fixture.pool)
    .await?;
    let audit_count: i64 = sqlx::query_scalar(
        "select count(*) from auth_audit_events where action = 'USER_DELETION_REQUESTED' and request_id = 'durable-delete-request'",
    )
    .fetch_one(&fixture.pool)
    .await?;
    assert_eq!(persisted_status, "DELETED");
    assert_eq!(saga_count, 1);
    assert_eq!(
        persisted_traceparent.as_deref(),
        Some(origin_traceparent.as_str())
    );
    assert_eq!(pending_steps, UserDeletionTarget::ALL.len() as i64);
    assert_eq!(audit_count, 1);
    assert!(refresh_tokens.find("deletion-session").await?.is_none());

    // A separate pool/repository simulates a new process. It sees the committed
    // saga, and a claimed lease becomes recoverable after the old worker dies.
    let first_claim_time = accepted_at + Duration::minutes(1);
    let abandoned_claim = saga_repository
        .claim_due(first_claim_time, Duration::minutes(5), 10)
        .await?
        .into_iter()
        .map(auth_service::domain::user_deletion::UserDeletionStep::from)
        .collect::<Vec<_>>();
    assert_eq!(abandoned_claim.len(), UserDeletionTarget::ALL.len());
    assert!(abandoned_claim
        .iter()
        .all(|step| { step.origin_traceparent.as_deref() == Some(origin_traceparent.as_str()) }));

    let restarted_pool = fixture.independent_pool().await?;
    let restarted_repository = Arc::new(PgUserDeletionSagaRepository::new(restarted_pool.clone()));
    let persisted = restarted_repository
        .find_by_user_id(user_id)
        .await?
        .map(auth_service::domain::user_deletion::UserDeletionSaga::from)
        .expect("committed saga survives process replacement");
    assert_eq!(persisted.id, saga_id);
    assert_eq!(persisted.status, UserDeletionSagaStatus::InProgress);
    assert!(restarted_repository
        .claim_due(
            first_claim_time + Duration::minutes(4),
            Duration::minutes(5),
            10,
        )
        .await?
        .into_iter()
        .map(auth_service::domain::user_deletion::UserDeletionStep::from)
        .collect::<Vec<_>>()
        .is_empty());

    let dispatcher = RecordingDispatcher::default();
    let worker = UserDeletionWorker::new(
        restarted_repository.clone(),
        dispatcher.clone(),
        UserDeletionWorkerPolicy {
            batch_size: 10,
            max_attempts: 3,
            lease_timeout: Duration::minutes(5),
            base_retry: Duration::seconds(1),
        },
    );
    let recovered = worker
        .run_once(first_claim_time + Duration::minutes(6))
        .await?;
    assert_eq!(recovered.claimed, UserDeletionTarget::ALL.len() as i64);
    assert_eq!(recovered.succeeded, UserDeletionTarget::ALL.len() as i64);
    assert_eq!(recovered.retried, 0);
    assert_eq!(recovered.dead_lettered, 0);

    let completed = restarted_repository
        .find_by_user_id(user_id)
        .await?
        .map(auth_service::domain::user_deletion::UserDeletionSaga::from)
        .expect("completed saga");
    assert_eq!(completed.status, UserDeletionSagaStatus::Completed);
    assert!(completed.steps.iter().all(|step| {
        step.status == auth_service::domain::user_deletion::UserDeletionStepStatus::Succeeded
    }));
    let mut dispatched = dispatcher.calls.lock().expect("dispatcher lock").clone();
    dispatched.sort_by_key(|(target, _)| target.as_str());
    let mut expected = UserDeletionTarget::ALL.to_vec();
    expected.sort_by_key(|target| target.as_str());
    assert_eq!(
        dispatched
            .iter()
            .map(|(target, _)| *target)
            .collect::<Vec<_>>(),
        expected
    );
    let worker_span_ids = dispatched
        .iter()
        .map(|(_, context)| {
            let context = context
                .as_ref()
                .expect("worker dispatch carries restored trace context");
            assert_eq!(context.trace_id(), request_context.trace_id());
            assert_eq!(context.parent_span_id(), Some(request_context.span_id()));
            assert_ne!(context.span_id(), request_context.span_id());
            context.span_id().to_owned()
        })
        .collect::<HashSet<_>>();
    assert_eq!(worker_span_ids.len(), UserDeletionTarget::ALL.len());

    restarted_pool.close().await;
    fixture.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn failed_queue_persistence_does_not_return_acceptance_or_partially_delete_the_user(
) -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let user_id = Uuid::new_v4();
    let user = User {
        id: user_id,
        primary_email: "rollback-deletion@example.invalid".into(),
        password_hash: "stored-hash".into(),
        status: UserStatus::Active,
        last_login_at: None,
    };
    let users = PgUserRepository::new(
        fixture.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    users.save(&(&user).into()).await?;
    let refresh_tokens = PgRefreshTokenStore::new(
        fixture.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    refresh_tokens
        .store(
            &(&RefreshTokenRecord {
                user_id,
                jti: "rollback-deletion-session".into(),
                token_hash: "rollback-deletion-session-hash".into(),
                family_id: user_id,
                expires_at: Utc::now() + Duration::hours(1),
            })
                .into(),
        )
        .await?;

    sqlx::raw_sql(
        "create function reject_deletion_step() returns trigger language plpgsql as $$ begin raise exception 'injected step failure'; end $$; create trigger reject_deletion_step before insert on user_deletion_steps for each row execute function reject_deletion_step();",
    )
    .execute(&fixture.pool)
    .await?;

    let use_case = DeleteUserUseCase::new(
        users,
        Arc::new(PgAuthAuditRepository::new(fixture.pool.clone())),
        Arc::new(PgUserDeletionSagaRepository::new(fixture.pool.clone())),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    let result = use_case
        .execute(DeleteUserDto {
            user_id: user_id.to_string(),
            actor_id: user_id.to_string(),
            request_id: Some("failed-durable-delete".into()),
            ip_address: None,
            user_agent: None,
        })
        .await;
    assert!(
        result.is_err(),
        "a failed durable insert cannot be accepted"
    );

    let persisted_status: String = sqlx::query_scalar("select status from users where id = $1")
        .bind(user_id)
        .fetch_one(&fixture.pool)
        .await?;
    let saga_count: i64 =
        sqlx::query_scalar("select count(*) from user_deletion_sagas where user_id = $1")
            .bind(user_id)
            .fetch_one(&fixture.pool)
            .await?;
    let audit_count: i64 = sqlx::query_scalar(
        "select count(*) from auth_audit_events where request_id = 'failed-durable-delete'",
    )
    .fetch_one(&fixture.pool)
    .await?;
    assert_eq!(persisted_status, "ACTIVE");
    assert_eq!(saga_count, 0);
    assert_eq!(audit_count, 0);
    assert!(refresh_tokens
        .find("rollback-deletion-session")
        .await?
        .is_some());
    fixture.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn operator_retry_is_atomic_idempotent_and_fences_previous_worker_claims(
) -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let user_id = Uuid::new_v4();
    let admin_id = Uuid::new_v4();
    let saga_repository = Arc::new(PgUserDeletionSagaRepository::new(fixture.pool.clone()));
    let use_case = DeleteUserUseCase::new(
        PgUserRepository::new(
            fixture.pool.clone(),
            std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
        ),
        Arc::new(PgAuthAuditRepository::new(fixture.pool.clone())),
        saga_repository.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    let user = User {
        id: user_id,
        primary_email: "operator-retry@example.invalid".into(),
        password_hash: "stored-hash".into(),
        status: UserStatus::Active,
        last_login_at: None,
    };
    PgUserRepository::new(
        fixture.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    )
    .save(&(&user).into())
    .await?;
    let second_user_id = Uuid::new_v4();
    PgUserRepository::new(
        fixture.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    )
    .save(
        &(&User {
            id: second_user_id,
            primary_email: "operator-retry-second@example.invalid".into(),
            password_hash: "stored-hash".into(),
            status: UserStatus::Active,
            last_login_at: None,
        })
            .into(),
    )
    .await?;
    let second_created = use_case
        .execute(DeleteUserDto {
            user_id: second_user_id.to_string(),
            actor_id: second_user_id.to_string(),
            request_id: Some("operator-retry-second-delete".into()),
            ip_address: None,
            user_agent: None,
        })
        .await?;
    let second_saga_id = Uuid::parse_str(&second_created.saga_id)?;
    let created = use_case
        .execute(DeleteUserDto {
            user_id: user_id.to_string(),
            actor_id: user_id.to_string(),
            request_id: Some("operator-retry-delete".into()),
            ip_address: None,
            user_agent: None,
        })
        .await?;
    let saga_id = Uuid::parse_str(&created.saga_id)?;
    let now = Utc::now();
    let old_claims = saga_repository
        .claim_due(now, Duration::minutes(5), 10)
        .await?
        .into_iter()
        .map(auth_service::domain::user_deletion::UserDeletionStep::from)
        .collect::<Vec<_>>();
    let stale_claim = old_claims
        .iter()
        .find(|step| step.saga_id == saga_id && step.target == UserDeletionTarget::Profile)
        .expect("profile claim")
        .clone();
    let second_dead_letter_step = old_claims
        .iter()
        .find(|step| step.saga_id == second_saga_id && step.target == UserDeletionTarget::Profile)
        .expect("second profile claim")
        .clone();
    saga_repository
        .mark_step_failed(
            &(&stale_claim).into(),
            "provider cleanup unavailable",
            now + Duration::seconds(30),
            true,
            now,
        )
        .await?;
    saga_repository
        .mark_step_failed(
            &(&second_dead_letter_step).into(),
            "profile cleanup unavailable for second saga",
            now + Duration::seconds(30),
            true,
            now,
        )
        .await?;
    let listed = saga_repository
        .list_dead_lettered(None, 100)
        .await?
        .into_iter()
        .map(auth_service::domain::user_deletion::UserDeletionSaga::from)
        .collect::<Vec<_>>();
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().all(|saga| {
        saga.status == UserDeletionSagaStatus::DeadLettered
            && saga.steps.len() == UserDeletionTarget::ALL.len()
    }));
    assert!(listed[0].updated_at <= listed[1].updated_at);
    let detail = saga_repository
        .find_by_saga_id(saga_id)
        .await?
        .map(auth_service::domain::user_deletion::UserDeletionSaga::from)
        .expect("lookup by saga identity");
    assert_eq!(detail.id, saga_id);
    let after = Some((listed[0].updated_at, listed[0].id));
    let second_page = saga_repository
        .list_dead_lettered(after, 100)
        .await?
        .into_iter()
        .map(auth_service::domain::user_deletion::UserDeletionSaga::from)
        .collect::<Vec<_>>();
    assert_eq!(second_page.len(), 1);
    assert_eq!(second_page[0].id, listed[1].id);
    assert_eq!(
        saga_repository
            .find_by_user_id(user_id)
            .await?
            .map(auth_service::domain::user_deletion::UserDeletionSaga::from)
            .unwrap()
            .status,
        UserDeletionSagaStatus::DeadLettered
    );

    let retry_use_case = RetryUserDeletionUseCase::new(
        saga_repository.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    let resolution_id = Uuid::new_v4();
    let retry_request = RetryUserDeletionDto {
        saga_id: saga_id.to_string(),
        resolution_id: resolution_id.to_string(),
        actor_id: admin_id.to_string(),
        reason: "Reviewed the provider incident and approved a fresh cleanup cycle".into(),
        evidence_reference: "INC-2026-0912".into(),
    };
    let accepted = retry_use_case.execute(retry_request.clone()).await?;
    assert!(accepted.accepted);
    assert_eq!(accepted.saga.status, "IN_PROGRESS");
    let remaining = saga_repository
        .list_dead_lettered(None, 100)
        .await?
        .into_iter()
        .map(auth_service::domain::user_deletion::UserDeletionSaga::from)
        .collect::<Vec<_>>();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, second_saga_id);
    assert!(accepted
        .saga
        .steps
        .iter()
        .all(|step| step.status == "PENDING" && step.attempt_count == 0));

    // A result from the pre-retry dispatch cannot overwrite the new generation.
    let after_retry = saga_repository
        .mark_step_succeeded(&(&stale_claim).into(), now + Duration::seconds(1))
        .await
        .map(auth_service::domain::user_deletion::UserDeletionSaga::from)?;
    assert_eq!(after_retry.status, UserDeletionSagaStatus::InProgress);
    assert!(after_retry
        .steps
        .iter()
        .any(|step| step.target == UserDeletionTarget::Profile
            && step.status
                == auth_service::domain::user_deletion::UserDeletionStepStatus::Pending));

    let repeated = retry_use_case.execute(retry_request.clone()).await?;
    assert_eq!(repeated.saga.status, "IN_PROGRESS");
    let conflicting = retry_use_case
        .execute(RetryUserDeletionDto {
            reason: "This changes the resolution request and must conflict".into(),
            ..retry_request.clone()
        })
        .await
        .expect_err("resolution ID cannot be reused with a different body");
    assert!(matches!(
        Some(&conflicting),
        Some(auth_service::application::errors::AuthError::Conflict(_))
    ));
    let repeated_new_resolution = retry_use_case
        .execute(RetryUserDeletionDto {
            resolution_id: Uuid::new_v4().to_string(),
            ..retry_request.clone()
        })
        .await
        .expect_err("an active saga cannot start a second retry cycle");
    assert!(matches!(
        Some(&repeated_new_resolution),
        Some(auth_service::application::errors::AuthError::Conflict(_))
    ));

    let resolution_count: i64 = sqlx::query_scalar(
        "select count(*) from user_deletion_retry_resolutions where resolution_id = $1",
    )
    .bind(resolution_id)
    .fetch_one(&fixture.pool)
    .await?;
    let audit_count: i64 = sqlx::query_scalar(
        "select count(*) from auth_audit_events where action = 'USER_DELETION_RETRY_AUTHORIZED' and subject_user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&fixture.pool)
    .await?;
    assert_eq!(resolution_count, 1);
    assert_eq!(audit_count, 1);
    let persisted_actor: Uuid = sqlx::query_scalar(
        "select actor_id from user_deletion_retry_resolutions where resolution_id = $1",
    )
    .bind(resolution_id)
    .fetch_one(&fixture.pool)
    .await?;
    assert_eq!(persisted_actor, admin_id);
    let immutable_receipt_update = sqlx::query(
        "update user_deletion_retry_resolutions set reason = 'rewritten' where resolution_id = $1",
    )
    .bind(resolution_id)
    .execute(&fixture.pool)
    .await;
    assert!(immutable_receipt_update.is_err());

    let worker = UserDeletionWorker::new(
        saga_repository.clone(),
        RecordingDispatcher::default(),
        UserDeletionWorkerPolicy {
            batch_size: 10,
            max_attempts: 3,
            lease_timeout: Duration::minutes(5),
            base_retry: Duration::seconds(1),
        },
    );
    let finished = worker.run_once(now + Duration::minutes(1)).await?;
    assert_eq!(finished.succeeded, UserDeletionTarget::ALL.len() as i64);
    assert_eq!(
        saga_repository
            .find_by_user_id(user_id)
            .await?
            .map(auth_service::domain::user_deletion::UserDeletionSaga::from)
            .unwrap()
            .status,
        UserDeletionSagaStatus::Completed
    );
    fixture.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn retention_keeps_incomplete_deletion_work_and_preserves_retired_user_ids(
) -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let users = PgUserRepository::new(
        fixture.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    let saga_repository = Arc::new(PgUserDeletionSagaRepository::new(fixture.pool.clone()));
    let use_case = DeleteUserUseCase::new(
        users.clone(),
        Arc::new(PgAuthAuditRepository::new(fixture.pool.clone())),
        saga_repository.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    let cleanup = PgAuthLifecycleCleanupRepository::new(fixture.pool.clone());
    let now = Utc::now();
    let expired_at = now - Duration::days(1);

    let incomplete_user_id = Uuid::new_v4();
    users
        .save(
            &(&User {
                id: incomplete_user_id,
                primary_email: "incomplete-deletion@example.invalid".into(),
                password_hash: "stored-hash".into(),
                status: UserStatus::Active,
                last_login_at: None,
            })
                .into(),
        )
        .await?;
    let incomplete = use_case
        .execute(DeleteUserDto {
            user_id: incomplete_user_id.to_string(),
            actor_id: incomplete_user_id.to_string(),
            request_id: Some("incomplete-retention-delete".into()),
            ip_address: None,
            user_agent: None,
        })
        .await?;
    let incomplete_saga_id = Uuid::parse_str(&incomplete.saga_id)?;
    let claim_time = Utc::now() + Duration::seconds(1);
    let claimed = saga_repository
        .claim_due(claim_time, Duration::minutes(5), 10)
        .await?
        .into_iter()
        .map(auth_service::domain::user_deletion::UserDeletionStep::from)
        .collect::<Vec<_>>();
    let profile_step = claimed
        .iter()
        .find(|step| {
            step.saga_id == incomplete_saga_id && step.target == UserDeletionTarget::Profile
        })
        .expect("profile cleanup claim");
    saga_repository
        .mark_step_failed(
            &(profile_step).into(),
            "provider cleanup unavailable",
            expired_at,
            true,
            claim_time,
        )
        .await?;
    sqlx::query("update users set retention_until = $2 where id = $1")
        .bind(incomplete_user_id)
        .bind(expired_at)
        .execute(&fixture.pool)
        .await?;

    let first_cleanup = cleanup.cleanup_expired(now).await?;
    assert_eq!(
        first_cleanup.deleted_users, 0,
        "expired retention must not erase a dead-lettered cleanup saga"
    );
    let retained_user_count: i64 = sqlx::query_scalar("select count(*) from users where id = $1")
        .bind(incomplete_user_id)
        .fetch_one(&fixture.pool)
        .await?;
    let retained_saga_status: String =
        sqlx::query_scalar("select status from user_deletion_sagas where id = $1")
            .bind(incomplete_saga_id)
            .fetch_one(&fixture.pool)
            .await?;
    let retained_step_count: i64 =
        sqlx::query_scalar("select count(*) from user_deletion_steps where saga_id = $1")
            .bind(incomplete_saga_id)
            .fetch_one(&fixture.pool)
            .await?;
    assert_eq!(retained_user_count, 1);
    assert_eq!(retained_saga_status, "DEAD_LETTERED");
    assert_eq!(retained_step_count, UserDeletionTarget::ALL.len() as i64);

    let completed_user_id = Uuid::new_v4();
    users
        .save(
            &(&User {
                id: completed_user_id,
                primary_email: "completed-deletion@example.invalid".into(),
                password_hash: "stored-hash".into(),
                status: UserStatus::Active,
                last_login_at: None,
            })
                .into(),
        )
        .await?;
    let completed = use_case
        .execute(DeleteUserDto {
            user_id: completed_user_id.to_string(),
            actor_id: completed_user_id.to_string(),
            request_id: Some("completed-retention-delete".into()),
            ip_address: None,
            user_agent: None,
        })
        .await?;
    let completed_saga_id = Uuid::parse_str(&completed.saga_id)?;
    let worker = UserDeletionWorker::new(
        saga_repository,
        RecordingDispatcher::default(),
        UserDeletionWorkerPolicy {
            batch_size: 10,
            max_attempts: 3,
            lease_timeout: Duration::minutes(5),
            base_retry: Duration::seconds(1),
        },
    );
    let completed_run = worker.run_once(Utc::now() + Duration::minutes(1)).await?;
    assert_eq!(
        completed_run.succeeded,
        UserDeletionTarget::ALL.len() as i64
    );
    sqlx::query("update users set retention_until = $2 where id = $1")
        .bind(completed_user_id)
        .bind(expired_at)
        .execute(&fixture.pool)
        .await?;

    let second_cleanup = cleanup.cleanup_expired(Utc::now()).await?;
    assert_eq!(second_cleanup.deleted_users, 1);
    let completed_user_count: i64 = sqlx::query_scalar("select count(*) from users where id = $1")
        .bind(completed_user_id)
        .fetch_one(&fixture.pool)
        .await?;
    let completed_saga_count: i64 =
        sqlx::query_scalar("select count(*) from user_deletion_sagas where id = $1")
            .bind(completed_saga_id)
            .fetch_one(&fixture.pool)
            .await?;
    let tombstone_count: i64 =
        sqlx::query_scalar("select count(*) from auth_user_tombstones where user_id = $1")
            .bind(completed_user_id)
            .fetch_one(&fixture.pool)
            .await?;
    assert_eq!(completed_user_count, 0);
    assert_eq!(completed_saga_count, 0);
    assert_eq!(tombstone_count, 1);

    let recreation = User {
        id: completed_user_id,
        primary_email: "recreated-deletion@example.invalid".into(),
        password_hash: "different-hash".into(),
        status: UserStatus::Active,
        last_login_at: None,
    };
    assert!(
        users.save(&(&recreation).into()).await.is_err(),
        "a physically purged user ID must remain retired"
    );
    fixture.shutdown().await?;
    Ok(())
}
