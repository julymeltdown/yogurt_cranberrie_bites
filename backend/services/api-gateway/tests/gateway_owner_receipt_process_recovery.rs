//! Actual binaries, native PostgreSQL and test-CA mTLS. Journal ambiguity is a
//! seeded fixture, not evidence of a public HTTP lost reply or production rollout.
#[path = "support/mod.rs"]
mod support;

use anyhow::{Context, Result};
use api_gateway::{
    adapters::out::friend_client::GrpcFriendClient, application::ports::FriendClientPort,
};
use base64ct::{Base64, Encoding};
use platform::{
    actor_delegation::ActorDelegationSigner,
    grpc_auth::Workload,
    mutation_context::{self, MutationContext},
};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::{
    collections::BTreeMap,
    fs::File,
    net::TcpListener,
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Clone)]
struct Evidence(Arc<Mutex<(PathBuf, Value)>>);

impl Evidence {
    fn new(directory: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&directory)?;
        let proof = Self(Arc::new(Mutex::new((
            directory.join("process-proof.json"),
            json!({
                "status":"RUNNING", "events":[], "boundary":
                "Actual Friend/Gateway binaries in parity configuration, native PG and test-CA mTLS; seeded UNKNOWN journal. No public HTTP lost-reply, full-stack readiness or production acceptance."
            }),
        ))));
        proof.event(json!({"stage":"started"}))?;
        Ok(proof)
    }
    fn event(&self, mut event: Value) -> Result<()> {
        event["observedAt"] = json!(chrono::Utc::now());
        let mut record = self.0.lock().expect("proof lock");
        record.1["events"]
            .as_array_mut()
            .expect("events")
            .push(event);
        std::fs::write(&record.0, serde_json::to_vec_pretty(&record.1)?)?;
        Ok(())
    }
    fn finish(&self, status: &str) -> Result<()> {
        let mut record = self.0.lock().expect("proof lock");
        record.1["status"] = json!(status);
        std::fs::write(&record.0, serde_json::to_vec_pretty(&record.1)?)?;
        Ok(())
    }
}

struct ProofGuard(Evidence);
impl Drop for ProofGuard {
    fn drop(&mut self) {
        let record = self.0 .0.lock().expect("proof lock");
        let passed = record.1["status"] == "PASS";
        drop(record);
        if !passed {
            let _ = self.0.finish("INCOMPLETE");
        }
    }
}

struct OwnedChild {
    child: Option<Child>,
    name: String,
    nonce: Uuid,
    proof: Evidence,
}
impl OwnedChild {
    fn spawn(
        binary: &Path,
        name: &str,
        env: &BTreeMap<String, String>,
        directory: &Path,
        proof: &Evidence,
    ) -> Result<Self> {
        let nonce = Uuid::new_v4();
        let log = directory.join(format!("{name}-{nonce}.log"));
        let stream = File::create(&log)?;
        let child = Command::new(binary)
            .env_clear()
            .envs(env)
            .stdout(Stdio::from(stream.try_clone()?))
            .stderr(Stdio::from(stream))
            .spawn()
            .with_context(|| format!("start {name}"))?;
        let mut owned = Self {
            child: Some(child),
            name: name.into(),
            nonce,
            proof: proof.clone(),
        };
        // Own the handle before fallible evidence I/O, so failed recording also reaps it.
        if let Err(error) = proof.event(json!({"stage":"spawn", "name":name,"nonce":nonce,
            "pid":owned.child.as_ref().expect("child").id(), "log":log}))
        {
            let _ = owned.stop();
            return Err(error);
        }
        Ok(owned)
    }
    fn alive(&mut self) -> Result<()> {
        anyhow::ensure!(
            self.child
                .as_mut()
                .context("missing child")?
                .try_wait()?
                .is_none(),
            "{} exited; inspect process log",
            self.name
        );
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            let pid = child.id();
            let existing = child.try_wait()?;
            let status = match existing {
                Some(status) => status,
                None => {
                    child.kill()?;
                    child.wait()?
                }
            };
            self.proof.event(json!({"stage":"reaped", "name":self.name,"nonce":self.nonce,
                "pid":pid,"signal":status.signal(),"exitCode":status.code(),"killIssued":existing.is_none()}))?;
            anyhow::ensure!(
                existing.is_none() && status.signal() == Some(9),
                "owned process did not remain live until SIGKILL"
            );
        }
        Ok(())
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn address() -> Result<String> {
    Ok(TcpListener::bind("127.0.0.1:0")?.local_addr()?.to_string())
}
fn put(env: &mut BTreeMap<String, String>, key: &str, value: impl ToString) {
    env.insert(key.into(), value.to_string());
}
fn tls(env: &mut BTreeMap<String, String>, prefix: &str, leaf: &test_support::tls::LeafPaths) {
    for (key, value) in test_support::tls::env_names(prefix)
        .into_iter()
        .zip(leaf.env_values())
    {
        put(env, &key, value);
    }
}
async fn live(child: &mut OwnedChild, metrics: &str) -> Result<()> {
    let http = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(1))
        .build()?;
    let start = Instant::now();
    loop {
        child.alive()?;
        if http
            .get(format!("http://{metrics}/livez"))
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
        {
            return Ok(());
        }
        anyhow::ensure!(
            start.elapsed() < Duration::from_secs(30),
            "liveness timeout"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
async fn gateway_snapshot(pool: &PgPool, id: Uuid) -> Result<Value> {
    let text: String = sqlx::query_scalar("select json_build_object('state',state,'outcome',outcome,'evidence',outcome_evidence,'attempt',owner_receipt_attempt_count,'claim',owner_receipt_claim_id,'next',owner_receipt_next_check_at,'checked',owner_receipt_checked_at,'leaseLive',owner_receipt_next_check_at>now(),'audits',(select count(*) from admin_audit_events where action='OPERATION_RECONCILED_FROM_OWNER_RECEIPT' and resource_id=$1::text))::text from gateway_request_journal where id=$1")
        .bind(id).fetch_one(pool).await?;
    Ok(serde_json::from_str(&text)?)
}
async fn wait_gateway(
    pool: &PgPool,
    id: Uuid,
    child: &mut OwnedChild,
    predicate: impl Fn(&Value) -> bool,
) -> Result<Value> {
    let start = Instant::now();
    loop {
        child.alive()?;
        let snapshot = gateway_snapshot(pool, id).await?;
        if predicate(&snapshot) {
            return Ok(snapshot);
        }
        anyhow::ensure!(
            start.elapsed() < Duration::from_secs(95),
            "Gateway recovery timeout: {snapshot}"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
async fn owner_snapshot(pool: &PgPool, id: Uuid, actor: Uuid, target: Uuid) -> Result<Value> {
    let text: String = sqlx::query_scalar("select json_build_object('receipt',(select row_to_json(r) from friend_mutation_receipts r where operation_id=$1),'edge',(select row_to_json(f) from follows f where follower_id=$2 and followee_id=$3))::text")
        .bind(id).bind(actor).bind(target).fetch_one(pool).await?;
    Ok(serde_json::from_str(&text)?)
}
async fn seed_journal(pool: &PgPool, id: Uuid, actor: Uuid, target: Uuid) -> Result<()> {
    let path = format!("/api/follows/{target}");
    sqlx::query("insert into gateway_request_journal(id,actor_id,action,resource_path,state,outcome,outcome_evidence) values($1,$2,$3,$4,'UNKNOWN','UNKNOWN','LEGACY_UNRESOLVED')")
        .bind(id).bind(actor.to_string()).bind(format!("DELETE {path}")).bind(path).execute(pool).await?;
    Ok(())
}
async fn unfollow(
    client: &GrpcFriendClient,
    id: Uuid,
    actor: Uuid,
    target: Uuid,
    lookup: bool,
) -> Result<()> {
    let response = mutation_context::scope(
        Some(MutationContext::new(
            id,
            actor,
            Some(format!("process-{id}")),
            lookup,
        )),
        client.unfollow(actor.to_string(), target.to_string()),
    )
    .await?;
    anyhow::ensure!(
        !response.following && response.status == "INACTIVE",
        "unexpected unfollow response"
    );
    Ok(())
}

#[tokio::test]
async fn actual_owner_and_gateway_recover_durable_receipts_after_sigkill() -> Result<()> {
    // Explicit opt-in; the dedicated runner provides owned-cluster lineage and binaries.
    // A skipped default suite is not process acceptance evidence.
    let Ok(directory) = std::env::var("GATEWAY_TEST_PROCESS_EVIDENCE_DIR") else {
        eprintln!("PROCESS_ACCEPTANCE_NOT_RUN: use run-gateway-reconciliation-native.py --process");
        return Ok(());
    };
    anyhow::ensure!(
        std::env::var("GATEWAY_TEST_CREATE_ISOLATED_DATABASES").as_deref() == Ok("1"),
        "isolated fixtures required"
    );
    let directory = PathBuf::from(directory);
    let proof = Evidence::new(directory.clone())?;
    let _guard = ProofGuard(proof.clone());
    // The support app supplies only owned Gateway DB/config. Its router, fake
    // ports and in-process reconciler are never invoked by this test.
    let app = support::start_gateway_app(false).await?;
    let observed: String =
        sqlx::query_scalar("select system_identifier::text from pg_control_system()")
            .fetch_one(&app.pool)
            .await?;
    anyhow::ensure!(
        std::env::var("GATEWAY_TEST_INITDB_SYSTEM_IDENTIFIER").as_deref() == Ok(&observed),
        "lineage mismatch"
    );
    let friend_db = format!("port_rust_friend_process_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("create database {friend_db} template template0"))
        .execute(&app.pool)
        .await?;
    let friend_url = format!(
        "{}/{friend_db}",
        app.config
            .database_url
            .rsplit_once('/')
            .context("fixture URL")?
            .0
    );
    let friend_pool = PgPool::connect(&friend_url).await?;
    let empty: bool =
        sqlx::query_scalar("select not exists(select 1 from pg_tables where schemaname='public')")
            .fetch_one(&friend_pool)
            .await?;
    anyhow::ensure!(empty, "owner DB must start empty");
    proof.event(json!({"stage":"fixtures","systemIdentifier":observed,"friendDatabase":friend_db,"ownerEmptyBeforeMigrations":empty,"template":"template0"}))?;
    let friend_binary = PathBuf::from(std::env::var("GATEWAY_TEST_FRIEND_BINARY")?);
    let gateway_binary = PathBuf::from(std::env::var("GATEWAY_TEST_GATEWAY_BINARY")?);
    let friend_leaf = test_support::tls::issue("friend")?;
    let gateway_leaf = test_support::tls::issue("gateway")?;
    let mut seed = [0u8; 32];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut seed)
        .map_err(|_| anyhow::anyhow!("entropy"))?;
    let signer = ActorDelegationSigner::from_seed(Workload::Gateway, &seed)?;
    let credential = format!("owned-process-{}", Uuid::new_v4());
    let friend_grpc = address()?;
    let friend_metrics = address()?;
    let gateway_metrics = address()?;
    // Hold unused endpoint sockets throughout the test to prevent accidental
    // connection to an unrelated process that acquires a released port.
    let absent = TcpListener::bind("127.0.0.1:0")?;
    let absent_endpoint = format!("https://{}", absent.local_addr()?);
    let mut friend_env = BTreeMap::new();
    for (key, value) in [
        ("FRIEND_RUNTIME_MODE", "parity"),
        ("FRIEND_SEED_ENABLED", "false"),
        ("FRIEND_RECOMMENDATION_WORKER_ENABLED", "false"),
        ("FRIEND_RECOMMENDATION_RETENTION_WORKER_ENABLED", "false"),
        ("FRIEND_MUTATION_RECEIPT_RETENTION_WORKER_ENABLED", "false"),
    ] {
        put(&mut friend_env, key, value);
    }
    put(&mut friend_env, "FRIEND_HTTP_ADDR", address()?);
    put(&mut friend_env, "FRIEND_METRICS_ADDR", &friend_metrics);
    put(&mut friend_env, "FRIEND_GRPC_ADDR", &friend_grpc);
    put(&mut friend_env, "FRIEND_DATABASE_URL", &friend_url);
    put(
        &mut friend_env,
        "FRIEND_PROFILE_GRPC_ENDPOINT",
        &absent_endpoint,
    );
    put(&mut friend_env, "GATEWAY_SERVICE_CREDENTIAL", &credential);
    put(
        &mut friend_env,
        "FRIEND_SERVICE_CREDENTIAL",
        format!("owner-{}", Uuid::new_v4()),
    );
    put(
        &mut friend_env,
        "ACTOR_DELEGATION_VERIFYING_KEYS",
        Base64::encode_string(&signer.public_key()),
    );
    tls(&mut friend_env, "FRIEND", &friend_leaf);
    let mut gateway_env = BTreeMap::new();
    put(&mut gateway_env, "GATEWAY_RUNTIME_MODE", "parity");
    put(&mut gateway_env, "GATEWAY_SEED_ENABLED", "false");
    put(&mut gateway_env, "GATEWAY_HTTP_ADDR", address()?);
    put(&mut gateway_env, "GATEWAY_METRICS_ADDR", &gateway_metrics);
    put(
        &mut gateway_env,
        "GATEWAY_DATABASE_URL",
        &app.config.database_url,
    );
    put(
        &mut gateway_env,
        "GATEWAY_FRIEND_GRPC_ENDPOINT",
        format!("https://{friend_grpc}"),
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
    put(&mut gateway_env, "GATEWAY_JWT_KEYS_REFRESH_SECONDS", "0");
    tls(&mut gateway_env, "GATEWAY", &gateway_leaf);
    let material = platform::grpc_tls::TlsMaterial::load(
        &gateway_leaf.cert,
        &gateway_leaf.key,
        &gateway_leaf.ca,
    )?;
    let client = GrpcFriendClient::connect_with_metrics(
        format!("https://{friend_grpc}"),
        Workload::Gateway,
        &credential,
        Some(&material),
        signer,
        Default::default(),
    )
    .await?;
    let mut friend =
        OwnedChild::spawn(&friend_binary, "friend-1", &friend_env, &directory, &proof)?;
    live(&mut friend, &friend_metrics).await?;
    // Liveness is not dependency readiness; wait for this actual guarded RPC too.
    let actor = Uuid::new_v4();
    let target = Uuid::new_v4();
    let operation = Uuid::new_v4();
    sqlx::query("insert into follows(follower_id,followee_id) values($1,$2)")
        .bind(actor)
        .bind(target)
        .execute(&friend_pool)
        .await?;
    unfollow(&client, operation, actor, target, false).await?;
    let committed = owner_snapshot(&friend_pool, operation, actor, target).await?;
    anyhow::ensure!(
        committed["receipt"]["scope"] == "friend.v1.UnfollowUser"
            && !committed["receipt"]["outcome_json"].is_null()
            && committed["edge"]["delete_reason"] == "unfollow"
            && committed["edge"]["deleted_by"] == actor.to_string(),
        "actual owner effect/receipt absent"
    );
    proof.event(
        json!({"stage":"owner-committed-before-kill","operation":operation,"snapshot":committed}),
    )?;
    friend.stop()?;
    seed_journal(&app.pool, operation, actor, target).await?;
    let mut gateway = OwnedChild::spawn(
        &gateway_binary,
        "gateway-1",
        &gateway_env,
        &directory,
        &proof,
    )?;
    live(&mut gateway, &gateway_metrics).await?;
    let unavailable = wait_gateway(&app.pool, operation, &mut gateway, |v| {
        v["attempt"] == 1 && v["claim"].is_null() && !v["next"].is_null()
    })
    .await?;
    anyhow::ensure!(
        unavailable["outcome"] == "UNKNOWN" && unavailable["audits"] == 0,
        "owner outage must not conclude commit"
    );
    proof.event(
        json!({"stage":"owner-outage-backoff","operation":operation,"snapshot":unavailable}),
    )?;
    friend = OwnedChild::spawn(&friend_binary, "friend-2", &friend_env, &directory, &proof)?;
    live(&mut friend, &friend_metrics).await?;
    anyhow::ensure!(
        owner_snapshot(&friend_pool, operation, actor, target).await? == committed,
        "owner restart changed durable receipt/effect"
    );
    unfollow(&client, operation, actor, target, true).await?;
    anyhow::ensure!(
        owner_snapshot(&friend_pool, operation, actor, target).await? == committed,
        "replay repeated owner effect"
    );
    let recovered = wait_gateway(&app.pool, operation, &mut gateway, |v| {
        v["outcome"] == "COMMITTED"
    })
    .await?;
    anyhow::ensure!(
        recovered["evidence"] == "OWNER_RECEIPT"
            && recovered["audits"] == 1
            && recovered["claim"].is_null()
            && recovered["next"].is_null(),
        "actual worker proof/audit"
    );
    proof.event(json!({"stage":"owner-restart-recovered","operation":operation,"snapshot":recovered,"ownerSnapshot":committed}))?;
    gateway.stop()?;
    // Second case: kill Gateway while an actual Friend SELECT is lock-blocked.
    let target2 = Uuid::new_v4();
    let operation2 = Uuid::new_v4();
    sqlx::query("insert into follows(follower_id,followee_id) values($1,$2)")
        .bind(actor)
        .bind(target2)
        .execute(&friend_pool)
        .await?;
    unfollow(&client, operation2, actor, target2, false).await?;
    let owner2 = owner_snapshot(&friend_pool, operation2, actor, target2).await?;
    anyhow::ensure!(
        owner2["receipt"]["scope"] == "friend.v1.UnfollowUser"
            && !owner2["receipt"]["outcome_json"].is_null()
            && owner2["edge"]["delete_reason"] == "unfollow"
            && owner2["edge"]["deleted_by"] == actor.to_string(),
        "second actual owner effect/receipt absent"
    );
    seed_journal(&app.pool, operation2, actor, target2).await?;
    let mut held = friend_pool.begin().await?;
    sqlx::query("lock table friend_mutation_receipts in access exclusive mode")
        .execute(&mut *held)
        .await?;
    gateway = OwnedChild::spawn(
        &gateway_binary,
        "gateway-2",
        &gateway_env,
        &directory,
        &proof,
    )?;
    // Poll the durable claim and actual owner's lock wait, not a sleep-based guess.
    let claim = wait_gateway(&app.pool, operation2, &mut gateway, |v| {
        !v["claim"].is_null()
    })
    .await?;
    let start = Instant::now();
    let blocked = loop {
        gateway.alive()?;
        let count: i64 = sqlx::query_scalar("select count(*) from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and wait_event_type='Lock' and query like '%select operation_id, actor_id, scope, committed_at, replay_until%'").fetch_one(&friend_pool).await?;
        if count > 0 {
            break count;
        }
        anyhow::ensure!(
            start.elapsed() < Duration::from_millis(750),
            "actual owner lookup not blocked before deadline"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    };
    gateway.stop()?;
    let after_kill = gateway_snapshot(&app.pool, operation2).await?;
    anyhow::ensure!(
        after_kill["claim"] == claim["claim"]
            && after_kill["attempt"] == 1
            && after_kill["leaseLive"] == true
            && after_kill["outcome"] == "UNKNOWN"
            && after_kill["audits"] == 0,
        "crash must retain live durable claim"
    );
    proof.event(json!({"stage":"gateway-killed-during-owner-lookup","operation":operation2,"blockedOwnerQueries":blocked,"snapshot":after_kill}))?;
    held.rollback().await?;
    gateway = OwnedChild::spawn(
        &gateway_binary,
        "gateway-3",
        &gateway_env,
        &directory,
        &proof,
    )?;
    live(&mut gateway, &gateway_metrics).await?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    let live_lease = gateway_snapshot(&app.pool, operation2).await?;
    anyhow::ensure!(
        live_lease["leaseLive"] == true
            && live_lease["claim"] == claim["claim"]
            && live_lease["attempt"] == 1
            && live_lease["outcome"] == "UNKNOWN",
        "restart reclaimed unexpired lease"
    );
    proof.event(json!({"stage":"gateway-restart-preserves-live-lease","snapshot":live_lease}))?;
    let recovered2 = wait_gateway(&app.pool, operation2, &mut gateway, |v| {
        v["outcome"] == "COMMITTED"
    })
    .await?;
    anyhow::ensure!(
        recovered2["evidence"] == "OWNER_RECEIPT"
            && recovered2["audits"] == 1
            && recovered2["attempt"] == 2,
        "natural lease recovery must conclude exactly once"
    );
    anyhow::ensure!(
        owner_snapshot(&friend_pool, operation2, actor, target2).await? == owner2,
        "Gateway recovery mutated owner effect"
    );
    proof.event(json!({"stage":"gateway-crash-natural-lease-recovery","operation":operation2,"snapshot":recovered2,"ownerSnapshot":owner2}))?;
    gateway.stop()?;
    // Another startup pass must preserve each already committed conclusion/audit.
    gateway = OwnedChild::spawn(
        &gateway_binary,
        "gateway-4",
        &gateway_env,
        &directory,
        &proof,
    )?;
    live(&mut gateway, &gateway_metrics).await?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    let final1 = gateway_snapshot(&app.pool, operation).await?;
    let final2 = gateway_snapshot(&app.pool, operation2).await?;
    anyhow::ensure!(
        final1 == recovered && final2 == recovered2,
        "restart changed terminal evidence/audits"
    );
    proof.event(json!({"stage":"terminal-conclusions-preserved","snapshots":[final1,final2]}))?;
    gateway.stop()?;
    friend.stop()?;
    friend_pool.close().await;
    app.shutdown().await?;
    proof.event(json!({"stage":"owned-children-reaped-and-observer-pools-closed"}))?;
    proof.finish("PASS")?;
    Ok(())
}
