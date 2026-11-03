//! Actual authenticated Gateway HTTP dispatch, owner commit, caller timeout and
//! process restart. Neither the journal intent nor its outcome is fixture-seeded.
#[path = "support/owner_process.rs"]
mod owner_process;
#[path = "support/mod.rs"]
mod support;

use anyhow::{Context, Result};
use base64ct::{Base64, Encoding};
use owner_process::{address, live, put, tls, Evidence, OwnedChild, ProofGuard};
use platform::{actor_delegation::ActorDelegationSigner, grpc_auth::Workload};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::{
    collections::BTreeMap,
    net::TcpListener,
    path::PathBuf,
    time::{Duration, Instant},
};
use uuid::Uuid;

async fn owner_snapshot(pool: &PgPool, id: Uuid, actor: Uuid, target: Uuid) -> Result<Value> {
    let raw: String = sqlx::query_scalar("select json_build_object('receipt',(select row_to_json(r) from friend_mutation_receipts r where operation_id=$1),'edge',(select row_to_json(f) from follows f where follower_id=$2 and followee_id=$3))::text")
        .bind(id).bind(actor).bind(target).fetch_one(pool).await?;
    Ok(serde_json::from_str(&raw)?)
}

async fn journal_snapshot(pool: &PgPool, id: Uuid) -> Result<Value> {
    let raw: String = sqlx::query_scalar("select json_build_object('id',id,'actor',actor_id,'key',idempotency_key,'state',state,'outcome',outcome,'evidence',outcome_evidence,'status',response_status,'created',created_at,'originalAudits',(select count(*) from admin_audit_events where id=$1),'replayAudits',(select count(*) from admin_audit_events where action='OPERATION_OWNER_RECONCILED' and resource_id=$1::text))::text from gateway_request_journal where id=$1")
        .bind(id).fetch_one(pool).await?;
    Ok(serde_json::from_str(&raw)?)
}

async fn public_status(
    http: &reqwest::Client,
    origin: &str,
    token: &str,
    id: Uuid,
) -> Result<Value> {
    let response = http
        .get(format!("{origin}/api/operations/{id}"))
        .bearer_auth(token)
        .send()
        .await?;
    anyhow::ensure!(
        response.status() == 200
            && response
                .headers()
                .get("cache-control")
                .is_some_and(|v| v == "no-store"),
        "caller status/cache policy"
    );
    let body: Value = response.json().await?;
    let mut keys: Vec<_> = body
        .as_object()
        .context("status object")?
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    anyhow::ensure!(
        keys == [
            "createdAt",
            "finishedAt",
            "operationId",
            "outcome",
            "replayable",
            "responseStatus",
            "state"
        ],
        "caller status leaked private journal/receipt/operator fields"
    );
    Ok(body)
}

#[tokio::test]
async fn actual_public_timeout_recovers_same_operation_after_gateway_sigkill() -> Result<()> {
    let Ok(directory) = std::env::var("GATEWAY_TEST_HTTP_PROCESS_EVIDENCE_DIR") else {
        eprintln!(
            "HTTP_PROCESS_ACCEPTANCE_NOT_RUN: use run-gateway-reconciliation-native.py --process"
        );
        return Ok(());
    };
    anyhow::ensure!(
        std::env::var("GATEWAY_TEST_CREATE_ISOLATED_DATABASES").as_deref() == Ok("1"),
        "isolated fixture required"
    );
    let directory = PathBuf::from(directory);
    let proof = Evidence::new(directory.clone(), "Actual public Gateway HTTP/JWT verification, actual Friend mTLS/owner commit, Gateway caller timeout and SIGKILL/restart with identical-key recovery. Test-issued JWT and parity loopback configuration; controller advisory-lock trigger gates journal completion only. No seeded journal/outcome, live Auth/provider, scheduled stale-age recovery or production/full-stack acceptance.")?;
    let _guard = ProofGuard(proof.clone());
    // Configuration/owned PG observer only; no support router/client is exercised.
    let app = support::start_gateway_app(false).await?;
    let lineage: String =
        sqlx::query_scalar("select system_identifier::text from pg_control_system()")
            .fetch_one(&app.pool)
            .await?;
    anyhow::ensure!(
        std::env::var("GATEWAY_TEST_INITDB_SYSTEM_IDENTIFIER").as_deref() == Ok(&lineage),
        "owned initdb lineage"
    );
    let database = format!("port_rust_friend_http_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("create database {database} template template0"))
        .execute(&app.pool)
        .await?;
    let owner_url = format!(
        "{}/{database}",
        app.config
            .database_url
            .rsplit_once('/')
            .context("fixture URL")?
            .0
    );
    let owner_pool = PgPool::connect(&owner_url).await?;
    let empty: bool =
        sqlx::query_scalar("select not exists(select 1 from pg_tables where schemaname='public')")
            .fetch_one(&owner_pool)
            .await?;
    anyhow::ensure!(
        empty,
        "owner fixture must be empty before actual binary migrations"
    );
    proof.event(json!({"stage":"fixtures","systemIdentifier":lineage,"ownerDatabase":database,"template":"template0","emptyBeforeMigrations":empty}))?;
    let owner_leaf = test_support::tls::issue("friend")?;
    let gateway_leaf = test_support::tls::issue("gateway")?;
    let mut seed = [0u8; 32];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut seed)
        .map_err(|_| anyhow::anyhow!("entropy"))?;
    let signer = ActorDelegationSigner::from_seed(Workload::Gateway, &seed)?;
    let credential = format!("http-process-{}", Uuid::new_v4());
    let owner_grpc = address()?;
    let owner_metrics = address()?;
    let gateway_metrics = address()?;
    let gateway_http = address()?;
    let origin = format!("http://{gateway_http}");
    let absent = TcpListener::bind("127.0.0.1:0")?;
    let absent_endpoint = format!("https://{}", absent.local_addr()?);
    let mut owner_env = BTreeMap::new();
    for (key, value) in [
        ("FRIEND_RUNTIME_MODE", "parity"),
        ("FRIEND_SEED_ENABLED", "false"),
        ("FRIEND_RECOMMENDATION_WORKER_ENABLED", "false"),
        ("FRIEND_RECOMMENDATION_RETENTION_WORKER_ENABLED", "false"),
        ("FRIEND_MUTATION_RECEIPT_RETENTION_WORKER_ENABLED", "false"),
    ] {
        put(&mut owner_env, key, value);
    }
    put(&mut owner_env, "FRIEND_HTTP_ADDR", address()?);
    put(&mut owner_env, "FRIEND_METRICS_ADDR", &owner_metrics);
    put(&mut owner_env, "FRIEND_GRPC_ADDR", &owner_grpc);
    put(&mut owner_env, "FRIEND_DATABASE_URL", &owner_url);
    put(
        &mut owner_env,
        "FRIEND_PROFILE_GRPC_ENDPOINT",
        &absent_endpoint,
    );
    put(&mut owner_env, "GATEWAY_SERVICE_CREDENTIAL", &credential);
    put(
        &mut owner_env,
        "FRIEND_SERVICE_CREDENTIAL",
        format!("owner-{}", Uuid::new_v4()),
    );
    put(
        &mut owner_env,
        "ACTOR_DELEGATION_VERIFYING_KEYS",
        Base64::encode_string(&signer.public_key()),
    );
    tls(&mut owner_env, "FRIEND", &owner_leaf);
    let mut gateway_env = BTreeMap::new();
    put(&mut gateway_env, "GATEWAY_RUNTIME_MODE", "parity");
    put(&mut gateway_env, "GATEWAY_SEED_ENABLED", "false");
    put(&mut gateway_env, "GATEWAY_HTTP_ADDR", &gateway_http);
    put(&mut gateway_env, "GATEWAY_METRICS_ADDR", &gateway_metrics);
    put(
        &mut gateway_env,
        "GATEWAY_DATABASE_URL",
        &app.config.database_url,
    );
    put(
        &mut gateway_env,
        "GATEWAY_FRIEND_GRPC_ENDPOINT",
        format!("https://{owner_grpc}"),
    );
    for owner in ["AUTH", "PROFILE", "POST", "NOTIFICATION"] {
        put(
            &mut gateway_env,
            &format!("GATEWAY_{owner}_GRPC_ENDPOINT"),
            &absent_endpoint,
        );
    }
    put(&mut gateway_env, "GATEWAY_SERVICE_CREDENTIAL", &credential);
    put(
        &mut gateway_env,
        "ACTOR_DELEGATION_SIGNING_SEED",
        Base64::encode_string(&seed),
    );
    put(
        &mut gateway_env,
        "GATEWAY_JWT_JWKS_JSON",
        &app.config.jwt_jwks_json,
    );
    put(
        &mut gateway_env,
        "GATEWAY_JWT_ISSUER",
        &app.config.jwt_issuer,
    );
    put(
        &mut gateway_env,
        "GATEWAY_JWT_AUDIENCE",
        &app.config.jwt_audience,
    );
    put(&mut gateway_env, "GATEWAY_JWT_KEYS_REFRESH_SECONDS", "0");
    tls(&mut gateway_env, "GATEWAY", &gateway_leaf);
    let owner_binary = PathBuf::from(std::env::var("GATEWAY_TEST_FRIEND_BINARY")?);
    let gateway_binary = PathBuf::from(std::env::var("GATEWAY_TEST_GATEWAY_BINARY")?);
    let mut owner = OwnedChild::spawn(
        &owner_binary,
        "http-friend-1",
        &owner_env,
        &directory,
        &proof,
    )?;
    live(&mut owner, &owner_metrics).await?;
    let actor = Uuid::new_v4();
    let target = Uuid::new_v4();
    sqlx::query("insert into follows(follower_id,followee_id) values($1,$2)")
        .bind(actor)
        .bind(target)
        .execute(&owner_pool)
        .await?;
    let token = support::bearer_token(&actor.to_string(), &["ROLE_USER"]);
    let other_token = support::bearer_token(&Uuid::new_v4().to_string(), &["ROLE_USER"]);
    let key = format!("http-recovery-{}", Uuid::new_v4());
    // A controller-owned lock delays only the real completion UPDATE. It neither
    // creates the intent nor changes any journal state, result or timestamp.
    let barrier = i64::from_be_bytes(Uuid::new_v4().as_bytes()[..8].try_into()?) & i64::MAX;
    sqlx::raw_sql(&format!("create function test_http_completion_barrier() returns trigger language plpgsql as $$ begin if old.state='IN_PROGRESS' and new.state='COMPLETED' then perform pg_advisory_xact_lock({barrier}); end if; return new; end $$; create trigger test_http_completion_barrier before update on gateway_request_journal for each row execute function test_http_completion_barrier();")).execute(&app.pool).await?;
    let mut held = app.pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock($1)")
        .bind(barrier)
        .execute(&mut *held)
        .await?;
    let mut gateway = OwnedChild::spawn(
        &gateway_binary,
        "http-gateway-1",
        &gateway_env,
        &directory,
        &proof,
    )?;
    live(&mut gateway, &gateway_metrics).await?;
    let http = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()?;
    let timeout = http
        .delete(format!("{origin}/api/follows/{target}"))
        .bearer_auth(&token)
        .header("idempotency-key", &key)
        .header("x-correlation-id", "real-http-recovery")
        .send()
        .await?;
    anyhow::ensure!(
        timeout.status() == 504
            && timeout
                .headers()
                .get("cache-control")
                .is_some_and(|v| v == "no-store"),
        "actual caller budget must return operation-id timeout"
    );
    let operation: Uuid = timeout
        .headers()
        .get("x-operation-id")
        .context("timeout operation id")?
        .to_str()?
        .parse()?;
    let timeout_body: Value = timeout.json().await?;
    let committed = owner_snapshot(&owner_pool, operation, actor, target).await?;
    anyhow::ensure!(
        committed["receipt"]["actor_id"] == actor.to_string()
            && committed["receipt"]["idempotency_key"] == key
            && committed["receipt"]["scope"] == "friend.v1.UnfollowUser"
            && !committed["receipt"]["outcome_json"].is_null()
            && committed["edge"]["delete_reason"] == "unfollow"
            && committed["edge"]["deleted_by"] == actor.to_string(),
        "actual HTTP owner commit/identity"
    );
    let start = Instant::now();
    let blocked = loop {
        gateway.alive()?;
        let count: i64 = sqlx::query_scalar("select count(*) from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and wait_event_type='Lock' and query like '%update gateway_request_journal%' and query like '%with updated as%'").fetch_one(&app.pool).await?;
        if count > 0 {
            break count;
        }
        anyhow::ensure!(
            start.elapsed() < Duration::from_secs(3),
            "actual Gateway completion lock wait not observed"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    };
    let before = journal_snapshot(&app.pool, operation).await?;
    anyhow::ensure!(
        before["actor"] == actor.to_string()
            && before["key"] == key
            && before["state"] == "IN_PROGRESS"
            && before["outcome"] == "PENDING"
            && before["originalAudits"] == 0
            && before["replayAudits"] == 0,
        "unrecorded original completion"
    );
    proof.event(json!({"stage":"public-timeout-with-owner-commit","operation":operation,"httpStatus":504,"timeoutBody":timeout_body,"blockedCompletionQueries":blocked,"journal":before,"ownerSnapshot":committed}))?;
    gateway.stop()?;
    anyhow::ensure!(
        journal_snapshot(&app.pool, operation).await? == before,
        "Gateway crash changed committed intent"
    );
    held.rollback().await?;
    sqlx::raw_sql("drop trigger test_http_completion_barrier on gateway_request_journal; drop function test_http_completion_barrier();").execute(&app.pool).await?;
    gateway = OwnedChild::spawn(
        &gateway_binary,
        "http-gateway-2",
        &gateway_env,
        &directory,
        &proof,
    )?;
    live(&mut gateway, &gateway_metrics).await?;
    let pending = public_status(&http, &origin, &token, operation).await?;
    anyhow::ensure!(
        pending["operationId"] == operation.to_string()
            && pending["state"] == "IN_PROGRESS"
            && pending["outcome"] == "PENDING"
            && pending["responseStatus"].is_null()
            && pending["replayable"] == false,
        "public status before receipt replay"
    );
    let forbidden = http
        .get(format!("{origin}/api/operations/{operation}"))
        .bearer_auth(&other_token)
        .send()
        .await?;
    let missing = http
        .get(format!("{origin}/api/operations/{}", Uuid::new_v4()))
        .bearer_auth(&other_token)
        .send()
        .await?;
    anyhow::ensure!(
        forbidden.status() == 404 && missing.status() == 404,
        "foreign operation must be indistinguishable from missing"
    );
    let forbidden_body: Value = forbidden.json().await?;
    let missing_body: Value = missing.json().await?;
    anyhow::ensure!(
        forbidden_body["code"] == missing_body["code"]
            && forbidden_body["message"] == missing_body["message"],
        "foreign operation disclosure"
    );
    let unsigned = support::unsigned_bearer_token(&actor.to_string(), &["ROLE_USER"]);
    let unauthenticated = http
        .get(format!("{origin}/api/operations/{operation}"))
        .bearer_auth(unsigned)
        .send()
        .await?;
    anyhow::ensure!(
        unauthenticated.status() == 401,
        "actual JWT verifier accepted unsigned bearer"
    );
    proof.event(json!({"stage":"restart-public-status-and-isolation","status":pending,"foreignStatus":404,"missingStatus":404,"unsignedStatus":401}))?;
    for attempt in 1..=2 {
        let response = http
            .delete(format!("{origin}/api/follows/{target}"))
            .bearer_auth(&token)
            .header("idempotency-key", &key)
            .send()
            .await?;
        anyhow::ensure!(
            response.status() == 200
                && response
                    .headers()
                    .get("idempotency-replayed")
                    .is_some_and(|v| v == "true")
                && response
                    .headers()
                    .get("x-operation-id")
                    .context("replay operation id")?
                    .to_str()?
                    == operation.to_string(),
            "actual same-key owner replay response"
        );
        let body: Value = response.json().await?;
        anyhow::ensure!(
            body["following"] == false && body["status"] == "INACTIVE",
            "replayed public body"
        );
        anyhow::ensure!(
            owner_snapshot(&owner_pool, operation, actor, target).await? == committed,
            "public retry repeated owner effect/receipt"
        );
        let journal = journal_snapshot(&app.pool, operation).await?;
        anyhow::ensure!(
            journal["state"] == "COMPLETED"
                && journal["outcome"] == "COMMITTED"
                && journal["evidence"] == "OWNER_REPLAY_RECEIPT"
                && journal["status"] == 200
                && journal["originalAudits"] == 0
                && journal["replayAudits"] == 1,
            "public replay atomic conclusion/single audit"
        );
        proof.event(json!({"stage":"public-identical-key-replay","attempt":attempt,"body":body,"journal":journal}))?;
    }
    let other_target = Uuid::new_v4();
    sqlx::query("insert into follows(follower_id,followee_id) values($1,$2)")
        .bind(actor)
        .bind(other_target)
        .execute(&owner_pool)
        .await?;
    let conflict = http
        .delete(format!("{origin}/api/follows/{other_target}"))
        .bearer_auth(&token)
        .header("idempotency-key", &key)
        .send()
        .await?;
    anyhow::ensure!(
        conflict.status() == 409,
        "same key with changed command must conflict"
    );
    let active: bool = sqlx::query_scalar(
        "select deleted_at is null from follows where follower_id=$1 and followee_id=$2",
    )
    .bind(actor)
    .bind(other_target)
    .fetch_one(&owner_pool)
    .await?;
    let receipts: i64 =
        sqlx::query_scalar("select count(*) from friend_mutation_receipts where actor_id=$1")
            .bind(actor)
            .fetch_one(&owner_pool)
            .await?;
    let journals: i64 = sqlx::query_scalar(
        "select count(*) from gateway_request_journal where actor_id=$1 and idempotency_key=$2",
    )
    .bind(actor.to_string())
    .bind(&key)
    .fetch_one(&app.pool)
    .await?;
    anyhow::ensure!(
        active && receipts == 1 && journals == 1,
        "conflicting retry dispatched another effect or identity"
    );
    let final_status = public_status(&http, &origin, &token, operation).await?;
    anyhow::ensure!(
        final_status["state"] == "COMPLETED"
            && final_status["outcome"] == "COMMITTED"
            && final_status["responseStatus"] == 200
            && final_status["replayable"] == true,
        "caller receipt-backed recovery status"
    );
    proof.event(json!({"stage":"public-final-status-and-conflict","status":final_status,"changedCommandHttpStatus":409,"ownerReceipts":receipts,"journalIdentities":journals,"otherEdgeActive":active}))?;
    gateway.stop()?;
    owner.stop()?;
    owner_pool.close().await;
    app.shutdown().await?;
    proof.event(json!({"stage":"owned-children-reaped-and-observer-pools-closed"}))?;
    proof.finish("PASS")?;
    Ok(())
}
