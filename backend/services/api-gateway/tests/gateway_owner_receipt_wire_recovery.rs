//! Actual socket failures with actual Gateway/Friend binaries and PostgreSQL.
//! Recovery requests never use controller-only knowledge of an operation ID.
#[path = "support/owner_process.rs"]
mod owner_process;
#[path = "support/owner_wire_fixture.rs"]
mod owner_wire_fixture;
#[path = "support/mod.rs"]
mod support;

use anyhow::{Context, Result};
use owner_wire_fixture::WireFixture;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::{
    io::ErrorKind,
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::{io::AsyncWriteExt, net::TcpStream};
use uuid::Uuid;

fn directory(case: &str) -> Option<PathBuf> {
    let root = std::env::var("GATEWAY_TEST_WIRE_PROCESS_EVIDENCE_DIR").ok()?;
    Some(PathBuf::from(root).join(case))
}
fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()?)
}
async fn seed_edge(fixture: &WireFixture, actor: Uuid, target: Uuid) -> Result<()> {
    sqlx::query("insert into follows(follower_id,followee_id) values($1,$2)")
        .bind(actor)
        .bind(target)
        .execute(&fixture.owner_pool)
        .await?;
    Ok(())
}
async fn owner_snapshot(
    pool: &PgPool,
    operation: Uuid,
    actor: Uuid,
    target: Uuid,
) -> Result<Value> {
    let raw:String=sqlx::query_scalar("select json_build_object('receipt',(select row_to_json(r) from friend_mutation_receipts r where operation_id=$1),'edge',(select row_to_json(f) from follows f where follower_id=$2 and followee_id=$3))::text")
        .bind(operation).bind(actor).bind(target).fetch_one(pool).await?;
    Ok(serde_json::from_str(&raw)?)
}
async fn journal_snapshot(pool: &PgPool, operation: Uuid) -> Result<Value> {
    let raw:String=sqlx::query_scalar("select json_build_object('state',state,'outcome',outcome,'evidence',outcome_evidence,'status',response_status,'key',idempotency_key,'actor',actor_id,'originalAudits',(select count(*) from admin_audit_events where id=$1),'receiptAudits',(select count(*) from admin_audit_events where action='OPERATION_RECONCILED_FROM_OWNER_RECEIPT' and resource_id=$1::text),'replayAudits',(select count(*) from admin_audit_events where action='OPERATION_OWNER_RECONCILED' and resource_id=$1::text))::text from gateway_request_journal where id=$1")
        .bind(operation).fetch_one(pool).await?;
    Ok(serde_json::from_str(&raw)?)
}
async fn observed_commit(fixture: &mut WireFixture, actor: Uuid, key: &str) -> Result<Uuid> {
    let start = Instant::now();
    loop {
        fixture.gateway_alive()?;
        fixture.owner.alive()?;
        let operation:Option<Uuid>=sqlx::query_scalar("select operation_id from friend_mutation_receipts where actor_id=$1 and idempotency_key=$2 and outcome_json is not null")
            .bind(actor).bind(key).fetch_optional(&fixture.owner_pool).await?;
        if let Some(id) = operation {
            return Ok(id);
        }
        anyhow::ensure!(
            start.elapsed() < Duration::from_secs(3),
            "actual owner commit not observed"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}
async fn wait_journal(
    fixture: &mut WireFixture,
    id: Uuid,
    predicate: impl Fn(&Value) -> bool,
) -> Result<Value> {
    let start = Instant::now();
    loop {
        fixture.gateway_alive()?;
        let row = journal_snapshot(fixture.pool(), id).await?;
        if predicate(&row) {
            return Ok(row);
        }
        anyhow::ensure!(
            start.elapsed() < Duration::from_secs(35),
            "actual journal recovery timeout: {row}"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
async fn replay(
    http: &reqwest::Client,
    origin: &str,
    token: &str,
    key: &str,
    target: Uuid,
) -> Result<(Uuid, Value)> {
    // The request only has the caller's original key/target/token. The ID is
    // learned from this actual recovery response, never from an observer query.
    let response = http
        .delete(format!("{origin}/api/follows/{target}"))
        .bearer_auth(token)
        .header("idempotency-key", key)
        .send()
        .await?;
    anyhow::ensure!(
        response.status() == 200
            && response
                .headers()
                .get("idempotency-replayed")
                .is_some_and(|v| v == "true"),
        "actual wire recovery response"
    );
    let id = response
        .headers()
        .get("x-operation-id")
        .context("recovery operation id")?
        .to_str()?
        .parse()?;
    let body: Value = response.json().await?;
    anyhow::ensure!(
        body["following"] == false && body["status"] == "INACTIVE",
        "actual replay body"
    );
    Ok((id, body))
}
async fn caller_status(
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
        "public status/cache policy"
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
        "private receipt/journal/operator data leaked"
    );
    Ok(body)
}
async fn assert_stable_owner(
    fixture: &WireFixture,
    id: Uuid,
    actor: Uuid,
    target: Uuid,
    key: &str,
    baseline: &Value,
) -> Result<()> {
    let receipt = owner_snapshot(&fixture.owner_pool, id, actor, target).await?;
    anyhow::ensure!(
        &receipt == baseline
            && receipt["receipt"]["actor_id"] == actor.to_string()
            && receipt["receipt"]["idempotency_key"] == key
            && receipt["receipt"]["scope"] == "friend.v1.UnfollowUser"
            && !receipt["receipt"]["outcome_json"].is_null()
            && receipt["edge"]["delete_reason"] == "unfollow"
            && receipt["edge"]["deleted_by"] == actor.to_string(),
        "actual owner proof changed/mismatched"
    );
    let count: i64 = sqlx::query_scalar(
        "select count(*) from friend_mutation_receipts where actor_id=$1 and idempotency_key=$2",
    )
    .bind(actor)
    .bind(key)
    .fetch_one(&fixture.owner_pool)
    .await?;
    anyhow::ensure!(count == 1, "recovery repeated owner identity");
    Ok(())
}

#[tokio::test]
async fn caller_disconnect_before_any_operation_header_recovers_using_original_key() -> Result<()> {
    let Some(directory) = directory("caller-disconnect") else {
        eprintln!("WIRE_PROCESS_ACCEPTANCE_NOT_RUN: use native runner --process");
        return Ok(());
    };
    let mut fixture=WireFixture::start(directory,"Actual caller TCP disconnect before response/operation-ID header, actual owner commit, Gateway SIGKILL and original-key recovery. Controller completion-lock trigger only; no seeded journal/outcome or timestamp adjustment. Test-issued JWT, parity loopback mTLS, no production/fullstack acceptance.",false).await?;
    let result = caller_disconnect_case(&mut fixture).await;
    fixture.finish(result).await
}

async fn caller_disconnect_case(fixture: &mut WireFixture) -> Result<()> {
    let actor = Uuid::new_v4();
    let target = Uuid::new_v4();
    let key = format!("disconnect-{}", Uuid::new_v4());
    seed_edge(fixture, actor, target).await?;
    let token = support::bearer_token(&actor.to_string(), &["ROLE_USER"]);
    let barrier = i64::from_be_bytes(Uuid::new_v4().as_bytes()[..8].try_into()?) & i64::MAX;
    sqlx::raw_sql(&format!("create function test_wire_completion_barrier() returns trigger language plpgsql as $$ begin if old.state='IN_PROGRESS' and new.state='COMPLETED' then perform pg_advisory_xact_lock({barrier}); end if; return new; end $$; create trigger test_wire_completion_barrier before update on gateway_request_journal for each row execute function test_wire_completion_barrier();")).execute(fixture.pool()).await?;
    let mut held = fixture.pool().begin().await?;
    sqlx::query("select pg_advisory_xact_lock($1)")
        .bind(barrier)
        .execute(&mut *held)
        .await?;
    fixture.start_gateway("disconnect-gateway-1").await?;
    let mut caller = TcpStream::connect(&fixture.gateway_address).await?;
    caller.write_all(format!("DELETE /api/follows/{target} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {token}\r\nIdempotency-Key: {key}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",fixture.gateway_address).as_bytes()).await?;
    let observer_id = observed_commit(fixture, actor, &key).await?;
    let baseline = owner_snapshot(&fixture.owner_pool, observer_id, actor, target).await?;
    assert_stable_owner(fixture, observer_id, actor, target, &key, &baseline).await?;
    let start = Instant::now();
    let blocked = loop {
        fixture.gateway_alive()?;
        let count:i64=sqlx::query_scalar("select count(*) from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and wait_event_type='Lock' and query like '%update gateway_request_journal%' and query like '%with updated as%'").fetch_one(fixture.pool()).await?;
        if count > 0 {
            break count;
        }
        anyhow::ensure!(
            start.elapsed() < Duration::from_millis(750),
            "actual Gateway completion not lock-blocked before disconnect"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    };
    let before = journal_snapshot(fixture.pool(), observer_id).await?;
    anyhow::ensure!(
        before["state"] == "IN_PROGRESS"
            && before["outcome"] == "PENDING"
            && before["originalAudits"] == 0,
        "actual uncompleted intent"
    );
    let mut byte = [0u8; 1];
    anyhow::ensure!(
        caller
            .try_read(&mut byte)
            .is_err_and(|error| error.kind() == ErrorKind::WouldBlock),
        "caller already received a response/operation-ID header"
    );
    drop(caller);
    fixture.proof.event(json!({"stage":"caller-disconnected-before-any-response-header","observerOnlyOperationId":observer_id,"callerKnownOperationId":false,"responseBytesRead":0,"responseBytesAvailableBeforeClose":false,"blockedCompletionQueries":blocked,"journal":before,"ownerSnapshot":baseline}))?;
    fixture.kill_gateway()?;
    anyhow::ensure!(
        journal_snapshot(fixture.pool(), observer_id).await? == before,
        "crash changed durable intent"
    );
    held.rollback().await?;
    sqlx::raw_sql("drop trigger test_wire_completion_barrier on gateway_request_journal; drop function test_wire_completion_barrier();").execute(fixture.pool()).await?;
    fixture.start_gateway("disconnect-gateway-2").await?;
    let http = client()?;
    let (learned_id, body) = replay(&http, &fixture.origin, &token, &key, target).await?;
    anyhow::ensure!(
        learned_id == observer_id,
        "original-key recovery created another operation"
    );
    assert_stable_owner(fixture, learned_id, actor, target, &key, &baseline).await?;
    let recovered = journal_snapshot(fixture.pool(), learned_id).await?;
    anyhow::ensure!(
        recovered["state"] == "COMPLETED"
            && recovered["outcome"] == "COMMITTED"
            && recovered["evidence"] == "OWNER_REPLAY_RECEIPT"
            && recovered["originalAudits"] == 0
            && recovered["receiptAudits"] == 0
            && recovered["replayAudits"] == 1,
        "receipt-backed public recovery/audit"
    );
    let status = caller_status(&http, &fixture.origin, &token, learned_id).await?;
    anyhow::ensure!(
        status["outcome"] == "COMMITTED"
            && status["state"] == "COMPLETED"
            && status["responseStatus"] == 200
            && status["replayable"] == true,
        "caller learned committed outcome"
    );
    let repeated = replay(&http, &fixture.origin, &token, &key, target).await?;
    anyhow::ensure!(
        repeated.0 == learned_id
            && journal_snapshot(fixture.pool(), learned_id).await? == recovered,
        "repeat recovery changed identity/audit"
    );
    assert_stable_owner(fixture, learned_id, actor, target, &key, &baseline).await?;
    fixture.proof.event(json!({"stage":"original-key-only-recovery","callerLearnedOperationId":learned_id,"observerIdUsedInRecoveryRequest":false,"body":body,"status":status,"journal":recovered}))?;
    Ok(())
}

#[tokio::test]
async fn encrypted_owner_reply_loss_recovers_via_real_scheduler_and_identical_key() -> Result<()> {
    let Some(directory) = directory("owner-reply-loss") else {
        eprintln!("WIRE_PROCESS_ACCEPTANCE_NOT_RUN: use native runner --process");
        return Ok(());
    };
    let mut fixture=WireFixture::start(directory,"Actual byte-transparent TCP relay with warmed end-to-end mTLS; encrypted owner reply withheld after actual PG commit. Actual Gateway UNKNOWN, SIGKILL/restart, scheduled receipt recovery and same-key replay. No TLS termination, synthetic owner reply, seeded journal/outcome or timestamp adjustment; test-issued JWT/parity loopback, no production/fullstack acceptance.",true).await?;
    let result = owner_reply_loss_case(&mut fixture).await;
    fixture.finish(result).await
}

async fn owner_reply_loss_case(fixture: &mut WireFixture) -> Result<()> {
    let actor = Uuid::new_v4();
    let target = Uuid::new_v4();
    let key = format!("owner-loss-{}", Uuid::new_v4());
    seed_edge(fixture, actor, target).await?;
    let token = support::bearer_token(&actor.to_string(), &["ROLE_USER"]);
    fixture.start_gateway("owner-loss-gateway-1").await?;
    let http = client()?;
    let warm = http
        .get(format!("{}/api/follows/counts", fixture.origin))
        .bearer_auth(&token)
        .send()
        .await?;
    anyhow::ensure!(
        warm.status() == 200,
        "actual mTLS connection warm-up failed"
    );
    let counts: Value = warm.json().await?;
    anyhow::ensure!(
        counts["followingCount"] == 1,
        "warm-up did not reach actual seeded owner state"
    );
    let relay = fixture.relay.as_ref().context("relay")?;
    anyhow::ensure!(
        relay.connections() == 1,
        "warm-up used unexpected connection inventory"
    );
    relay.gate(true);
    fixture.proof.event(json!({"stage":"mTLS-warmed-then-owner-byte-gate-enabled","warmCounts":counts,"relayConnections":relay.connections(),"tlsTerminatedByRelay":false}))?;
    let response = http
        .delete(format!("{}/api/follows/{target}", fixture.origin))
        .bearer_auth(&token)
        .header("idempotency-key", &key)
        .send()
        .await?;
    fixture.proof.event(json!({"stage":"observed-owner-loss-http-response","httpStatus":response.status().as_u16(),"cacheControl":response.headers().get("cache-control").and_then(|value|value.to_str().ok()),"operationIdHeaderPresent":response.headers().contains_key("x-operation-id")}))?;
    anyhow::ensure!(
        response.status() == 504
            && response
                .headers()
                .get("cache-control")
                .is_some_and(|v| v == "no-store"),
        "actual lost owner reply must yield caller timeout/id"
    );
    let caller_id: Uuid = response
        .headers()
        .get("x-operation-id")
        .context("timeout operation id")?
        .to_str()?
        .parse()?;
    let timeout_body: Value = response.json().await?;
    let observer_id = observed_commit(fixture, actor, &key).await?;
    anyhow::ensure!(caller_id == observer_id, "caller/owner operation mismatch");
    let baseline = owner_snapshot(&fixture.owner_pool, observer_id, actor, target).await?;
    assert_stable_owner(fixture, observer_id, actor, target, &key, &baseline).await?;
    let unknown = wait_journal(fixture, caller_id, |v| {
        v["state"] == "UNKNOWN" && v["outcome"] == "UNKNOWN"
    })
    .await?;
    let relay = fixture.relay.as_ref().context("relay")?;
    anyhow::ensure!(
        relay.held_bytes() > 0
            && relay.connections() == 1
            && unknown["originalAudits"] == 1
            && unknown["receiptAudits"] == 0
            && unknown["replayAudits"] == 0,
        "actual UNKNOWN/lost owner response evidence"
    );
    fixture.proof.event(json!({"stage":"actual-owner-commit-with-reply-withheld","callerOperationId":caller_id,"httpStatus":504,"timeoutBody":timeout_body,"journal":unknown,"ownerSnapshot":baseline,"heldEncryptedOwnerBytes":relay.held_bytes(),"relayConnections":relay.connections()}))?;
    fixture.kill_gateway()?;
    fixture.relay.as_ref().context("relay")?.gate(false);
    fixture.start_gateway("owner-loss-gateway-2").await?;
    let scheduled = wait_journal(fixture, caller_id, |v| {
        v["outcome"] == "COMMITTED" && v["evidence"] == "OWNER_RECEIPT"
    })
    .await?;
    anyhow::ensure!(
        scheduled["state"] == "UNKNOWN"
            && scheduled["originalAudits"] == 1
            && scheduled["receiptAudits"] == 1
            && scheduled["replayAudits"] == 0,
        "real scheduled evidence/audit inventory"
    );
    assert_stable_owner(fixture, caller_id, actor, target, &key, &baseline).await?;
    let before_replay = caller_status(&http, &fixture.origin, &token, caller_id).await?;
    anyhow::ensure!(
        before_replay["state"] == "UNKNOWN"
            && before_replay["outcome"] == "COMMITTED"
            && before_replay["replayable"] == false,
        "caller positive proof separate from replay snapshot"
    );
    fixture.proof.event(json!({"stage":"actual-scheduler-recovers-positive-owner-proof","journal":scheduled,"status":before_replay}))?;
    let replayed = replay(&http, &fixture.origin, &token, &key, target).await?;
    anyhow::ensure!(replayed.0 == caller_id, "same key changed operation id");
    assert_stable_owner(fixture, caller_id, actor, target, &key, &baseline).await?;
    let recovered = journal_snapshot(fixture.pool(), caller_id).await?;
    anyhow::ensure!(
        recovered["state"] == "COMPLETED"
            && recovered["outcome"] == "COMMITTED"
            && recovered["evidence"] == "OWNER_REPLAY_RECEIPT"
            && recovered["originalAudits"] == 1
            && recovered["receiptAudits"] == 1
            && recovered["replayAudits"] == 1,
        "append-only original/scheduler/replay proof chain"
    );
    let repeated = replay(&http, &fixture.origin, &token, &key, target).await?;
    anyhow::ensure!(
        repeated.0 == caller_id && journal_snapshot(fixture.pool(), caller_id).await? == recovered,
        "repeated replay duplicated proof/audit"
    );
    assert_stable_owner(fixture, caller_id, actor, target, &key, &baseline).await?;
    let status = caller_status(&http, &fixture.origin, &token, caller_id).await?;
    anyhow::ensure!(
        status["state"] == "COMPLETED"
            && status["outcome"] == "COMMITTED"
            && status["responseStatus"] == 200
            && status["replayable"] == true,
        "public response recovery"
    );
    fixture.proof.event(json!({"stage":"public-replay-preserves-original-and-scheduler-audits","body":replayed.1,"journal":recovered,"status":status}))?;
    Ok(())
}
