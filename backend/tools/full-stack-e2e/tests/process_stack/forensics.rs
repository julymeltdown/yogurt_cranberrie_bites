//! Bounded, read-only failure evidence collected while fixtures are still alive.

use anyhow::Result;
use futures::Future;
use reqwest::{header::HeaderMap, Method, StatusCode};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgConnectOptions, postgres::PgPoolOptions, PgPool};
use std::{collections::VecDeque, path::PathBuf, str::FromStr, sync::Arc, sync::Mutex};
use tokio::time::{sleep, timeout, Duration, Instant};
use uuid::Uuid;

const REQUEST_LIMIT: usize = 32;
const CAPTURE_LIMIT: Duration = Duration::from_secs(45);
const SETTLEMENT_LIMIT: Duration = Duration::from_secs(30);

#[derive(Clone, Default)]
pub struct RequestLedger(Arc<Mutex<VecDeque<Value>>>);

tokio::task_local! {
    static REQUESTS: RequestLedger;
}

pub async fn scope<F: Future>(ledger: RequestLedger, future: F) -> F::Output {
    REQUESTS.scope(ledger, future).await
}

pub struct RequestObservation {
    pub request_id: Uuid,
    started: Instant,
    ledger: Option<RequestLedger>,
}

impl RequestObservation {
    pub fn begin(method: &Method, path: &str) -> Self {
        let request_id = Uuid::new_v4();
        let ledger = REQUESTS.try_with(Clone::clone).ok();
        if let Some(ledger) = &ledger {
            let mut requests = ledger.0.lock().unwrap_or_else(|error| error.into_inner());
            if requests.len() == REQUEST_LIMIT {
                requests.pop_front();
            }
            requests.push_back(json!({
                "requestId": request_id,
                "method": method.as_str(),
                "route": route_template(path),
                "status": null,
                "operationId": null,
                "responseReceived": false,
            }));
        }
        Self {
            request_id,
            started: Instant::now(),
            ledger,
        }
    }

    pub fn response(&self, status: StatusCode, headers: &HeaderMap) {
        let operation_id = headers
            .get("x-operation-id")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| Uuid::parse_str(value).ok());
        self.update(|record| {
            record["status"] = json!(status.as_u16());
            record["operationId"] = json!(operation_id);
            record["responseReceived"] = json!(true);
        });
    }

    fn update(&self, update: impl FnOnce(&mut Value)) {
        if let Some(ledger) = &self.ledger {
            let mut requests = ledger.0.lock().unwrap_or_else(|error| error.into_inner());
            if let Some(record) = requests
                .iter_mut()
                .find(|record| record["requestId"] == self.request_id.to_string())
            {
                update(record);
            }
        }
    }
}

impl Drop for RequestObservation {
    fn drop(&mut self) {
        self.update(|record| record["elapsedMs"] = json!(self.started.elapsed().as_millis()));
    }
}

// Dynamic paths, query values, and arbitrary caller strings are never retained.
fn route_template(path: &str) -> &'static str {
    let path = path.split('?').next().unwrap_or("");
    match path {
        "/api/profile/privacy" => "/api/profile/privacy",
        "/auth/signup" => "/auth/signup",
        "/auth/email/verify" => "/auth/email/verify",
        "/auth/account/deletion" => "/auth/account/deletion",
        "/api/posts" => "/api/posts",
        "/api/feed" => "/api/feed",
        _ if path.starts_with("/api/profile/") => "/api/profile/{resource}",
        _ if path.starts_with("/api/operations/") => "/api/operations/{operationId}",
        _ => "other",
    }
}

pub struct CaptureContext<'a> {
    pub run_id: &'a str,
    pub migration_mode: &'static str,
    pub postgres_failure: Option<Value>,
    pub gateway_database_url: &'a str,
    pub profile_database_url: &'a str,
    pub gateway_metrics_url: String,
    pub service_pids: Vec<(&'static str, u32)>,
    pub dependency_containers: Vec<(&'static str, String)>,
}

pub async fn capture(context: CaptureContext<'_>, ledger: &RequestLedger) -> Result<PathBuf> {
    let requests: Vec<Value> = ledger
        .0
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .iter()
        .cloned()
        .collect();
    let request_ids: Vec<String> = requests
        .iter()
        .filter_map(|record| record["requestId"].as_str().map(str::to_owned))
        .collect();
    let mut operation_ids: Vec<Uuid> = requests
        .iter()
        .filter_map(|record| {
            record["operationId"]
                .as_str()
                .and_then(|id| Uuid::parse_str(id).ok())
        })
        .collect();
    let directory =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/process-stack-forensics");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(format!("{}.json", context.run_id));
    let mut report = json!({
        "schemaVersion": 1,
        "runId": context.run_id,
        "migrationMode": context.migration_mode,
        "postgresFailure": context.postgres_failure,
        "requests": requests,
        "requestCoverage": "process_json_request calls and the explicitly instrumented blocked-comment request in the scoped journey task; other direct client calls and spawned tasks are excluded",
        "captureComplete": false,
        "outcomeInterpretation": "Snapshots are evidence, not a replacement for the original failed assertion.",
    });
    persist(&path, &report)?;

    let captured = timeout(CAPTURE_LIMIT, async {
        let gateway = connect(context.gateway_database_url).await;
        let profile = connect(context.profile_database_url).await;
        let mut subjects = Vec::new();
        if let Ok(pool) = &gateway {
            if let Ok(intents) = sqlx::query_as::<_, (Uuid, Option<String>)>("select id, actor_id from gateway_request_journal where request_id = any($1) or id = any($2) order by created_at desc limit 32")
                .bind(&request_ids).bind(&operation_ids).fetch_all(pool).await {
                for (operation_id, actor) in intents {
                    operation_ids.push(operation_id);
                    if let Some(actor) = actor.and_then(|actor| Uuid::parse_str(&actor).ok()) {
                        subjects.push((operation_id, actor));
                    }
                }
                operation_ids.sort_unstable();
                operation_ids.dedup();
            }
        }
        report["immediate"] = snapshot(gateway.as_ref().ok(), profile.as_ref().ok(), &operation_ids, &subjects).await;
        report["resources"] = process_resources(&context.service_pids).await;
        report["dependencyResources"] = dependency_resources(&context.dependency_containers).await;
        report["hostResources"] = host_resources().await;
        report["poolMetrics"] = json!({"capturedAt": chrono::Utc::now(), "timing": "before teardown, after the immediate database snapshot", "data": pool_metrics(&context.gateway_metrics_url).await});
        persist(&path, &report)?;

        let settlement_deadline = Instant::now() + SETTLEMENT_LIMIT;
        let mut settled = false;
        if let Ok(pool) = &gateway {
            while !operation_ids.is_empty() && Instant::now() < settlement_deadline {
                match sqlx::query_scalar::<_, i64>("select count(*) from gateway_request_journal where id = any($1) and state in ('IN_PROGRESS', 'QUEUED')")
                    .bind(&operation_ids).fetch_one(pool).await {
                    Ok(0) => { settled = true; break; }
                    Err(_) => break,
                    _ => sleep(Duration::from_millis(200)).await,
                }
            }
        }
        report["settled"] = snapshot(gateway.as_ref().ok(), profile.as_ref().ok(), &operation_ids, &subjects).await;
        report["noPendingJournalRows"] = json!(settled);
        report["captureComplete"] = json!(true);
        Ok::<(), anyhow::Error>(())
    }).await;
    match captured {
        Ok(Ok(())) => {}
        Ok(Err(_)) => report["captureError"] = json!("CAPTURE_WRITE_FAILED"),
        Err(_) => report["captureError"] = json!("CAPTURE_DEADLINE_EXCEEDED"),
    }
    persist(&path, &report)?;
    Ok(path)
}

pub fn verify_complete(path: &PathBuf) -> Result<()> {
    let report: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    anyhow::ensure!(
        report["captureComplete"] == true && report.get("captureError").is_none(),
        "process-stack diagnostic capture did not complete"
    );
    for snapshot in ["immediate", "settled"] {
        for section in [
            "journal",
            "queue",
            "profileReceiptsAndRows",
            "gatewayConnections",
            "profileConnections",
        ] {
            anyhow::ensure!(
                report[snapshot][section].is_array(),
                "process-stack diagnostic section {snapshot}/{section} was unavailable"
            );
        }
    }
    anyhow::ensure!(
        report["poolMetrics"]["data"]["available"] == true,
        "process-stack pool metrics were unavailable before teardown"
    );
    Ok(())
}

pub async fn host_resources() -> Value {
    let output = timeout(
        Duration::from_secs(2),
        tokio::process::Command::new("sysctl")
            .args(["-n", "vm.loadavg"])
            .output(),
    )
    .await;
    let raw = match output {
        Ok(Ok(output)) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
        _ => std::fs::read_to_string("/proc/loadavg").unwrap_or_default(),
    };
    let load: Vec<f64> = raw
        .split_whitespace()
        .filter_map(|part| part.parse::<f64>().ok())
        .take(3)
        .collect();
    json!({"capturedAt": chrono::Utc::now(), "logicalCpuCount": std::thread::available_parallelism().ok().map(|count| count.get()), "loadAverage": load})
}

async fn dependency_resources(containers: &[(&'static str, String)]) -> Value {
    if containers.is_empty() {
        return json!([]);
    }
    let mut command = tokio::process::Command::new("docker");
    command.args(["stats", "--no-stream", "--format", "{{json .}}"]);
    for (_, id) in containers {
        command.arg(id);
    }
    match timeout(Duration::from_secs(4), command.output()).await {
        Ok(Ok(output)) if output.status.success() => {
            let rows: Vec<Value> = String::from_utf8_lossy(&output.stdout).lines().filter_map(|line| serde_json::from_str::<Value>(line).ok()).filter_map(|row| {
                let id = row["ID"].as_str()?;
                let (dependency, _) = containers.iter().find(|(_, full_id)| full_id.starts_with(id))?;
                Some(json!({"dependency": dependency, "cpuPercent": row["CPUPerc"], "memoryUsage": row["MemUsage"], "memoryPercent": row["MemPerc"], "blockIo": row["BlockIO"], "networkIo": row["NetIO"], "pids": row["PIDs"]}))
            }).collect();
            json!({"capturedAt": chrono::Utc::now(), "rows": rows})
        }
        _ => json!({"error": "DEPENDENCY_STATS_UNAVAILABLE"}),
    }
}

async fn connect(url: &str) -> Result<PgPool, sqlx::Error> {
    let options = PgConnectOptions::from_str(url)?
        .application_name("process-stack-forensics")
        .options([
            ("default_transaction_read_only", "on"),
            ("statement_timeout", "2000"),
            ("lock_timeout", "100"),
        ]);
    PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(2))
        .connect_with(options)
        .await
}

async fn snapshot(
    gateway: Option<&PgPool>,
    profile: Option<&PgPool>,
    operation_ids: &[Uuid],
    subjects: &[(Uuid, Uuid)],
) -> Value {
    let mut value = json!({});
    value["journal"] = query_rows(
        gateway,
        r#"
        select coalesce(jsonb_agg(to_jsonb(snapshot)), '[]'::jsonb) from (
            select id, state, outcome, outcome_evidence, response_status, created_at, finished_at
            from gateway_request_journal where id = any($1) order by created_at desc limit 32
        ) snapshot
    "#,
        operation_ids,
    )
    .await;
    value["queue"] = query_rows(gateway, r#"
        select coalesce(jsonb_agg(to_jsonb(snapshot)), '[]'::jsonb) from (
            select operation_id, state, attempt_count, available_at, lease_expires_at
            from gateway_async_operation_queue where operation_id = any($1) order by operation_id limit 32
        ) snapshot
    "#, operation_ids).await;
    let subject_operations: Vec<Uuid> = subjects.iter().map(|(operation, _)| *operation).collect();
    let subject_actors: Vec<Uuid> = subjects.iter().map(|(_, actor)| *actor).collect();
    value["profileReceiptsAndRows"] = match profile {
        Some(pool) => match sqlx::query_scalar::<_, Value>(r#"
        select coalesce(jsonb_agg(to_jsonb(snapshot)), '[]'::jsonb) from (
            select intent.operation_id, receipt.operation_id is not null as has_receipt,
                   receipt.scope, receipt.committed_at, receipt.replay_until,
                   receipt.outcome_json is not null as has_outcome,
                   profile.account_visibility, profile.updated_at,
                   profile.deleted_at is not null as profile_deleted
            from unnest($1::uuid[], $2::uuid[]) as intent(operation_id, actor_id)
            left join profile_mutation_receipts receipt on receipt.operation_id = intent.operation_id
            left join profiles profile on profile.user_id = intent.actor_id
            order by intent.operation_id limit 32
        ) snapshot
    "#).bind(&subject_operations).bind(&subject_actors).fetch_one(pool).await {
            Ok(rows) => rows,
            Err(error) => query_error(&error),
        },
        None => json!({"error": "CONNECTION_UNAVAILABLE"}),
    };
    for (name, pool) in [
        ("gatewayConnections", gateway),
        ("profileConnections", profile),
    ] {
        value[name] = match pool {
            Some(pool) => match sqlx::query_scalar::<_, Value>(r#"
                select coalesce(jsonb_agg(to_jsonb(snapshot)), '[]'::jsonb) from (
                    select pid, state, wait_event_type, wait_event,
                           case
                               when state <> 'active' then 'not_active'
                               when query ilike '%gateway_async_operation_queue%' then 'async_queue'
                               when query ilike '%gateway_request_journal%' then 'journal'
                               when query ilike '%admin_audit_events%' then 'audit'
                               when query ilike '%telemetry_records%' then 'telemetry'
                               when query ilike '%gateway_rate_limit%' then 'quota'
                               when query ilike '%profile_mutation_receipts%' then 'profile_receipt'
                               when query ilike '%profiles%' then 'profile'
                               else 'unclassified'
                           end as statement_class,
                           case when application_name in ('gateway-main', 'gateway-journal', 'gateway-background', 'gateway-quota', 'process-stack-forensics')
                                then application_name else 'unclassified' end as pool,
                           extract(epoch from (now() - query_start)) * 1000 as query_age_ms,
                           extract(epoch from (now() - xact_start)) * 1000 as transaction_age_ms,
                           pg_blocking_pids(pid) as blocking_pids
                    from pg_stat_activity where datname = current_database()
                      and pid <> pg_backend_pid() order by pid limit 64
                ) snapshot
            "#).fetch_one(pool).await {
                Ok(rows) => rows,
                Err(error) => query_error(&error),
            },
            None => json!({"error": "CONNECTION_UNAVAILABLE"}),
        };
    }
    value
}

async fn query_rows(pool: Option<&PgPool>, query: &str, operation_ids: &[Uuid]) -> Value {
    match pool {
        Some(pool) => match sqlx::query_scalar::<_, Value>(query)
            .bind(operation_ids)
            .fetch_one(pool)
            .await
        {
            Ok(rows) => rows,
            Err(error) => query_error(&error),
        },
        None => json!({"error": "CONNECTION_UNAVAILABLE"}),
    }
}

fn query_error(error: &sqlx::Error) -> Value {
    let code = match error {
        sqlx::Error::Database(database) => database.code().map(|code| code.into_owned()),
        _ => None,
    };
    json!({"error": "QUERY_FAILED", "sqlstate": code})
}

async fn pool_metrics(url: &str) -> Value {
    let result = async {
        let response = reqwest::Client::new().get(url).send().await?;
        let text = response.error_for_status()?.text().await?;
        Ok::<_, reqwest::Error>(
            text.lines()
                .filter(|line| line.starts_with("gateway_db_pool_"))
                .take(64)
                .map(str::to_owned)
                .collect::<Vec<_>>(),
        )
    };
    match timeout(Duration::from_secs(2), result).await {
        Ok(Ok(lines)) => json!({"available": !lines.is_empty(), "lines": lines}),
        _ => json!({"error": "METRICS_UNAVAILABLE"}),
    }
}

async fn process_resources(pids: &[(&'static str, u32)]) -> Value {
    let ids = pids
        .iter()
        .map(|(_, pid)| pid.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let result = tokio::process::Command::new("ps")
        .args(["-p", &ids, "-o", "pid=,pcpu=,rss=,stat="])
        .output();
    match timeout(Duration::from_secs(2), result).await {
        Ok(Ok(output)) => {
            json!({"services": pids, "columns": ["pid", "cpuPercent", "rssKiB", "state"], "rows": String::from_utf8_lossy(&output.stdout)})
        }
        _ => json!({"error": "PROCESS_STATS_UNAVAILABLE"}),
    }
}

fn persist(path: &PathBuf, report: &Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(report)?;
    std::fs::write(path, &bytes)?;
    std::fs::write(
        path.with_extension("json.sha256"),
        format!("{:x}\n", Sha256::digest(&bytes)),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::FutureExt;
    use std::panic::AssertUnwindSafe;
    use test_support::postgres;

    #[tokio::test]
    async fn requests_keep_correlation_without_credentials_or_dynamic_paths() {
        let ledger = RequestLedger::default();
        scope(ledger.clone(), async {
            let request = RequestObservation::begin(
                &Method::PUT,
                "/api/profile/privacy?email=private@example.invalid",
            );
            let mut headers = HeaderMap::new();
            headers.insert("x-operation-id", Uuid::nil().to_string().parse().unwrap());
            headers.insert("set-cookie", "credential-must-not-survive".parse().unwrap());
            request.response(StatusCode::GATEWAY_TIMEOUT, &headers);
            drop(request);
            let unfinished =
                RequestObservation::begin(&Method::GET, "/api/profile/private@example.invalid");
            drop(unfinished);
        })
        .await;
        let records = ledger.0.lock().unwrap();
        let serialized = serde_json::to_string(&*records).unwrap();
        assert!(!serialized.contains("private@example.invalid"));
        assert!(!serialized.contains("credential-must-not-survive"));
        assert_eq!(records[0]["operationId"], Uuid::nil().to_string());
        assert_eq!(records[0]["status"], 504);
        assert_eq!(records[1]["responseReceived"], false);
        assert!(records[1]["requestId"]
            .as_str()
            .and_then(|id| Uuid::parse_str(id).ok())
            .is_some());
    }

    #[tokio::test]
    async fn real_postgres_capture_preserves_panic_receipt_and_missing_receipt_evidence(
    ) -> Result<()> {
        let pg = postgres::start_postgres().await?;
        let host = pg.get_host().await?;
        let port = pg.get_host_port_ipv4(5432).await?;
        let url = |database: &str| format!("postgres://postgres:postgres@{host}:{port}/{database}");
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(&url("postgres"))
            .await?;
        for database in ["capture_gateway", "capture_profile"] {
            sqlx::query(&format!("create database {database}"))
                .execute(&admin)
                .await?;
        }
        let gateway_url = url("capture_gateway");
        let profile_url = url("capture_profile");
        let gateway = PgPoolOptions::new()
            .max_connections(1)
            .connect(&gateway_url)
            .await?;
        let profile = PgPoolOptions::new()
            .max_connections(1)
            .connect(&profile_url)
            .await?;
        for (service, pool) in [("api-gateway", &gateway), ("profile-service", &profile)] {
            let migrations = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../services/{service}/migrations"));
            sqlx::migrate::Migrator::new(migrations.as_path())
                .await?
                .run(pool)
                .await?;
        }
        let actor = Uuid::new_v4();
        sqlx::query("insert into profiles (user_id, username, nickname, avatar_url, bio, account_visibility) values ($1, 'capture-owner', 'private nickname', '', 'private biography', 'PRIVATE')")
            .bind(actor).execute(&profile).await?;
        let committed_id = Uuid::new_v4();
        let missing_receipt_id = Uuid::new_v4();
        let ledger = RequestLedger::default();
        let mut request_id = Uuid::nil();
        let panic: std::result::Result<(), _> = scope(
            ledger.clone(),
            AssertUnwindSafe(async {
                let observation = RequestObservation::begin(&Method::PUT, "/api/profile/privacy");
                request_id = observation.request_id;
                let mut headers = HeaderMap::new();
                headers.insert("x-operation-id", committed_id.to_string().parse().unwrap());
                observation.response(StatusCode::GATEWAY_TIMEOUT, &headers);
                panic!("injected journey assertion; fixture ownership must survive");
            })
            .catch_unwind(),
        )
        .await;
        assert!(panic.is_err());
        for operation_id in [committed_id, missing_receipt_id] {
            sqlx::query("insert into gateway_request_journal (id, actor_id, action, resource_path, request_id, state, outcome, outcome_evidence, response_status, response_body, request_fingerprint, ip_address, user_agent, idempotency_key) values ($1, $2, 'PUT /api/profile/privacy', '/api/profile/privacy', $3, 'COMPLETED', 'COMMITTED', 'SUCCESS_RESPONSE', 200, $4, 'private-fingerprint', 'private-ip', 'private-agent', $5)")
                .bind(operation_id).bind(actor.to_string()).bind(request_id.to_string())
                .bind(b"private response bytes".as_slice()).bind(operation_id.to_string())
                .execute(&gateway).await?;
        }
        sqlx::query("insert into profile_mutation_receipts (operation_id, actor_id, scope, command_fingerprint, outcome_json) values ($1, $2, 'profile.v1.UpdateProfilePrivacy', 'private-command-fingerprint', '{\"private\":\"owner outcome bytes\"}'::jsonb)")
            .bind(committed_id).bind(actor).execute(&profile).await?;
        let read_only = connect(&gateway_url).await?;
        assert!(sqlx::query("delete from gateway_request_journal")
            .execute(&read_only)
            .await
            .is_err());
        read_only.close().await;
        let run_id = format!("capture-test-{}", Uuid::new_v4());
        let path = capture(
            CaptureContext {
                run_id: &run_id,
                migration_mode: "startup",
                postgres_failure: None,
                gateway_database_url: &gateway_url,
                profile_database_url: &profile_url,
                gateway_metrics_url: "http://127.0.0.1:1/metrics".into(),
                service_pids: Vec::new(),
                dependency_containers: Vec::new(),
            },
            &ledger,
        )
        .await?;
        let bytes = std::fs::read(&path)?;
        let report: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(report["captureComplete"], true);
        assert_eq!(report["migrationMode"], "startup");
        for snapshot in ["immediate", "settled"] {
            for section in [
                "journal",
                "queue",
                "profileReceiptsAndRows",
                "gatewayConnections",
                "profileConnections",
            ] {
                assert!(
                    report[snapshot][section].is_array(),
                    "{snapshot}/{section} capture failed: {}",
                    report[snapshot][section]
                );
            }
        }
        assert_eq!(report["immediate"]["journal"].as_array().unwrap().len(), 2);
        let rows = report["settled"]["profileReceiptsAndRows"]
            .as_array()
            .unwrap();
        let committed = rows
            .iter()
            .find(|row| row["operation_id"] == committed_id.to_string())
            .unwrap();
        let absent = rows
            .iter()
            .find(|row| row["operation_id"] == missing_receipt_id.to_string())
            .unwrap();
        assert_eq!(committed["has_receipt"], true);
        assert_eq!(absent["has_receipt"], false);
        assert_eq!(absent["account_visibility"], "PRIVATE");
        let text = String::from_utf8(bytes.clone())?;
        for secret in [
            "private nickname",
            "private biography",
            "private response bytes",
            "private-fingerprint",
            "private-command-fingerprint",
            "private-ip",
            "private-agent",
            "owner outcome bytes",
        ] {
            assert!(
                !text.contains(secret),
                "forensic artifact leaked a sensitive fixture field"
            );
        }
        assert_eq!(
            std::fs::read_to_string(path.with_extension("json.sha256"))?,
            format!("{:x}\n", Sha256::digest(&bytes))
        );
        admin.close().await;
        gateway.close().await;
        profile.close().await;
        Ok(())
    }
}
