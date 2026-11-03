//! Local storage benchmark for PostgreSQL admission versus a Redis Lua prototype.
//!
//! Run with `cargo run --release -p api-gateway --features admission-bench
//! --example admission-benchmark`. The Redis side is a fixed-window script
//! prototype, not a production adapter: it does not yet implement the Gateway's
//! bounded per-bucket cardinality and overflow policy.

use anyhow::{Context, Result};
use api_gateway::config::InMemoryRateLimiter;
use api_gateway::domain::admission::{AdmissionPolicies, RouteClass};
use redis::{Client as RedisClient, Script};
use serde_json::json;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use test_support::postgres::start_postgres;
use test_support::redis::start_redis;
use tokio::sync::Barrier;
use tokio::task::JoinSet;
use uuid::Uuid;

const DEFAULT_REQUESTS_PER_CASE: usize = 2_000;
const DEFAULT_WARMUP_REQUESTS: usize = 100;
const DEFAULT_CONCURRENCY: &[usize] = &[1, 8, 32, 64];
const RATE_LIMIT: u32 = 100_000;
const WINDOW_SECONDS: u32 = 60;
const REDIS_FIXED_WINDOW: &str = r#"
local current = redis.call('GET', KEYS[1])
if current and tonumber(current) >= tonumber(ARGV[1]) then
  return 0
end
local count = redis.call('INCR', KEYS[1])
if count == 1 then
  redis.call('EXPIRE', KEYS[1], ARGV[2])
end
return 1
"#;

#[derive(Clone, Copy)]
enum Shape {
    HotKey,
    ManyKeys,
}

impl Shape {
    fn as_str(self) -> &'static str {
        match self {
            Self::HotKey => "hot-key",
            Self::ManyKeys => "many-keys",
        }
    }

    fn key_index(self, ordinal: usize) -> usize {
        match self {
            Self::HotKey => 0,
            Self::ManyKeys => ordinal,
        }
    }
}

struct WorkerSamples {
    latency: Vec<Duration>,
    lock_wait: Vec<Duration>,
    non_admitted: usize,
}

struct Measurement {
    latency: Vec<Duration>,
    lock_wait: Vec<Duration>,
    non_admitted: usize,
    elapsed: Duration,
}

#[tokio::main]
async fn main() -> Result<()> {
    let requests = env_usize("ADMISSION_BENCH_REQUESTS", DEFAULT_REQUESTS_PER_CASE)?;
    let warmup = env_usize("ADMISSION_BENCH_WARMUP", DEFAULT_WARMUP_REQUESTS)?;
    let concurrency = env_list("ADMISSION_BENCH_CONCURRENCY", DEFAULT_CONCURRENCY)?;
    if requests == 0 || warmup == 0 || concurrency.is_empty() || concurrency.contains(&0) {
        anyhow::bail!("request counts and concurrency values must be positive");
    }

    println!(
        "{}",
        json!({
            "kind": "environment",
            "timestamp_utc": chrono::Utc::now().to_rfc3339(),
            "os": std::env::consts::OS,
            "architecture": std::env::consts::ARCH,
            "package_version": env!("CARGO_PKG_VERSION"),
            "requests_per_case": requests,
            "warmup_requests_per_case": warmup,
            "concurrency": concurrency,
            "postgres_quota_pool_connections": 4,
            "postgres_acquire_timeout_ms": 200,
            "postgres_statement_timeout_ms": 200,
            "postgres_advisory_lock_timeout_ms": 150,
            "redis_prototype": "single-key atomic fixed-window Lua; cardinality policy not implemented"
        })
    );

    let postgres = start_postgres().await?;
    let postgres_host = postgres.get_host().await?;
    let postgres_port = postgres.get_host_port_ipv4(5432).await?;
    let database_url =
        format!("postgres://postgres:postgres@{postgres_host}:{postgres_port}/postgres");
    let quota_options = PgConnectOptions::from_str(&database_url)?
        .options([("statement_timeout", "200"), ("lock_timeout", "150")]);
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_millis(200))
        .connect_with(quota_options)
        .await?;
    let migrations = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    sqlx::migrate::Migrator::new(migrations.as_path())
        .await?
        .run(&pool)
        .await?;

    let redis = start_redis().await?;
    let redis_host = redis.get_host().await?;
    let redis_port = redis.get_host_port_ipv4(6379).await?;
    let redis_client = RedisClient::open(format!("redis://{redis_host}:{redis_port}/"))?;
    let script = Arc::new(Script::new(REDIS_FIXED_WINDOW));

    let policies = AdmissionPolicies::uniform(RATE_LIMIT);
    let limiter = Arc::new(
        InMemoryRateLimiter::with_policies(
            policies,
            Duration::from_secs(u64::from(WINDOW_SECONDS)),
        )
        .with_database(pool.clone()),
    );

    for shape in [Shape::HotKey, Shape::ManyKeys] {
        for &workers in &concurrency {
            let run_id = Uuid::new_v4().simple().to_string();
            let warm_prefix = format!("{run_id}:warm");
            let measured_prefix = format!("{run_id}:measure");

            // Warm both backends at the same concurrency; report only fresh keys.
            run_postgres(limiter.clone(), shape, &warm_prefix, workers, warmup).await?;
            run_redis(
                redis_client.clone(),
                script.clone(),
                shape,
                &warm_prefix,
                workers,
                warmup,
            )
            .await?;

            let postgres_measurement = run_postgres(
                limiter.clone(),
                shape,
                &format!("pg:{measured_prefix}"),
                workers,
                requests,
            )
            .await?;
            emit("postgres", shape, workers, requests, postgres_measurement);

            let redis_measurement = run_redis(
                redis_client.clone(),
                script.clone(),
                shape,
                &format!("redis:{measured_prefix}"),
                workers,
                requests,
            )
            .await?;
            emit(
                "redis-lua-prototype",
                shape,
                workers,
                requests,
                redis_measurement,
            );
        }
    }

    pool.close().await;
    drop(redis);
    drop(postgres);
    Ok(())
}

async fn run_postgres(
    limiter: Arc<InMemoryRateLimiter>,
    shape: Shape,
    prefix: &str,
    workers: usize,
    requests: usize,
) -> Result<Measurement> {
    let barrier = Arc::new(Barrier::new(workers + 1));
    let mut tasks = JoinSet::new();
    for worker in 0..workers {
        let limiter = limiter.clone();
        let barrier = barrier.clone();
        let prefix = prefix.to_owned();
        let count = requests / workers + usize::from(worker < requests % workers);
        tasks.spawn(async move {
            barrier.wait().await;
            let mut samples = WorkerSamples {
                latency: Vec::with_capacity(count),
                lock_wait: Vec::with_capacity(count),
                non_admitted: 0,
            };
            for sequence in 0..count {
                let ordinal = sequence * workers + worker;
                let network = format!("benchmark:{prefix}:client-{}", shape.key_index(ordinal));
                let started = Instant::now();
                let (outcome, lock_wait) =
                    limiter.admit_observed(RouteClass::Write, &network).await;
                samples.latency.push(started.elapsed());
                if let Some(lock_wait) = lock_wait {
                    samples.lock_wait.push(lock_wait);
                }
                if !outcome.is_admitted() {
                    samples.non_admitted += 1;
                }
            }
            samples
        });
    }

    let started = Instant::now();
    barrier.wait().await;
    let mut aggregate = WorkerSamples {
        latency: Vec::with_capacity(requests),
        lock_wait: Vec::with_capacity(requests),
        non_admitted: 0,
    };
    while let Some(result) = tasks.join_next().await {
        merge(
            &mut aggregate,
            result.context("PostgreSQL benchmark worker panicked")?,
        );
    }
    Ok(Measurement {
        latency: aggregate.latency,
        lock_wait: aggregate.lock_wait,
        non_admitted: aggregate.non_admitted,
        elapsed: started.elapsed(),
    })
}

async fn run_redis(
    client: RedisClient,
    script: Arc<Script>,
    shape: Shape,
    prefix: &str,
    workers: usize,
    requests: usize,
) -> Result<Measurement> {
    let mut connections = Vec::with_capacity(workers);
    for _ in 0..workers {
        connections.push(client.get_multiplexed_async_connection().await?);
    }
    let barrier = Arc::new(Barrier::new(workers + 1));
    let mut tasks = JoinSet::new();
    for (worker, mut connection) in connections.into_iter().enumerate() {
        let script = script.clone();
        let barrier = barrier.clone();
        let prefix = prefix.to_owned();
        let count = requests / workers + usize::from(worker < requests % workers);
        tasks.spawn(async move {
            barrier.wait().await;
            let mut samples = WorkerSamples {
                latency: Vec::with_capacity(count),
                lock_wait: Vec::new(),
                non_admitted: 0,
            };
            for sequence in 0..count {
                let ordinal = sequence * workers + worker;
                let key = format!(
                    "admission:write:benchmark:{prefix}:client-{}",
                    shape.key_index(ordinal)
                );
                let started = Instant::now();
                let admitted: i64 = script
                    .key(key)
                    .arg(RATE_LIMIT)
                    .arg(WINDOW_SECONDS)
                    .invoke_async(&mut connection)
                    .await?;
                samples.latency.push(started.elapsed());
                if admitted == 0 {
                    samples.non_admitted += 1;
                }
            }
            Ok::<_, anyhow::Error>(samples)
        });
    }

    let started = Instant::now();
    barrier.wait().await;
    let mut aggregate = WorkerSamples {
        latency: Vec::with_capacity(requests),
        lock_wait: Vec::new(),
        non_admitted: 0,
    };
    while let Some(result) = tasks.join_next().await {
        merge(
            &mut aggregate,
            result.context("Redis benchmark worker panicked")??,
        );
    }
    Ok(Measurement {
        latency: aggregate.latency,
        lock_wait: aggregate.lock_wait,
        non_admitted: aggregate.non_admitted,
        elapsed: started.elapsed(),
    })
}

fn merge(target: &mut WorkerSamples, mut source: WorkerSamples) {
    target.latency.append(&mut source.latency);
    target.lock_wait.append(&mut source.lock_wait);
    target.non_admitted += source.non_admitted;
}

fn emit(backend: &str, shape: Shape, workers: usize, requests: usize, measurement: Measurement) {
    let elapsed_seconds = measurement.elapsed.as_secs_f64();
    println!(
        "{}",
        json!({
            "kind": "result",
            "backend": backend,
            "workload": shape.as_str(),
            "concurrency": workers,
            "requests": requests,
            "throughput_ops_per_second": requests as f64 / elapsed_seconds,
            "elapsed_ms": elapsed_seconds * 1_000.0,
            "latency_ms": {
                "p50": percentile_ms(&measurement.latency, 0.50),
                "p95": percentile_ms(&measurement.latency, 0.95),
                "p99": percentile_ms(&measurement.latency, 0.99),
            },
            "postgres_lock_wait_ms": if measurement.lock_wait.is_empty() {
                None
            } else {
                Some(json!({
                    "p50": percentile_ms(&measurement.lock_wait, 0.50),
                    "p95": percentile_ms(&measurement.lock_wait, 0.95),
                    "p99": percentile_ms(&measurement.lock_wait, 0.99),
                }))
            },
            "non_admitted": measurement.non_admitted,
        })
    );
}

fn percentile_ms(samples: &[Duration], percentile: f64) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }
    let mut samples = samples.iter().map(Duration::as_nanos).collect::<Vec<_>>();
    samples.sort_unstable();
    let rank = ((samples.len() as f64 * percentile).ceil() as usize).max(1);
    Some(samples[rank.min(samples.len()) - 1] as f64 / 1_000_000.0)
}

fn env_usize(name: &str, default: usize) -> Result<usize> {
    std::env::var(name)
        .map(|value| value.parse().with_context(|| format!("invalid {name}")))
        .unwrap_or(Ok(default))
}

fn env_list(name: &str, default: &[usize]) -> Result<Vec<usize>> {
    match std::env::var(name) {
        Ok(value) => value
            .split(',')
            .map(|part| {
                part.trim()
                    .parse()
                    .with_context(|| format!("invalid value in {name}"))
            })
            .collect(),
        Err(std::env::VarError::NotPresent) => Ok(default.to_vec()),
        Err(error) => Err(error.into()),
    }
}
