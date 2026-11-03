//! Actual authenticated public profile deletion and required owner audit faults.
#[path = "support/owner_process.rs"]
mod owner_process;
#[path = "support/owner_wire_fixture.rs"]
mod owner_wire_fixture;
#[path = "support/mod.rs"]
mod support;

use anyhow::{Context, Result};
use owner_wire_fixture::WireFixture;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use uuid::Uuid;

async fn snapshot(fixture: &WireFixture, user: Uuid) -> Result<Value> {
    let raw:String=sqlx::query_scalar("select json_build_object('profile',(select row_to_json(p) from profiles p where user_id=$1),'avatar',(select row_to_json(a) from avatars a where user_id=$1),'successAudits',(select count(*) from profile_audit_events where profile_user_id=$1 and action='PROFILE_DELETED' and outcome='SUCCESS'),'receipts',(select count(*) from profile_mutation_receipts where actor_id=$1))::text")
        .bind(user).fetch_one(&fixture.owner_pool).await?;
    Ok(serde_json::from_str(&raw)?)
}
fn assert_active(row: &Value) -> Result<()> {
    anyhow::ensure!(
        row["profile"]["deleted_at"].is_null()
            && row["avatar"]["deleted_at"].is_null()
            && row["successAudits"] == 0
            && row["receipts"] == 0,
        "unexpected owner deletion/audit/receipt: {row}"
    );
    Ok(())
}
async fn status(
    http: &reqwest::Client,
    fixture: &WireFixture,
    token: &str,
    id: Uuid,
) -> Result<Value> {
    let response = http
        .get(format!("{}/api/operations/{id}", fixture.origin))
        .bearer_auth(token)
        .send()
        .await?;
    anyhow::ensure!(
        response.status() == 200
            && response
                .headers()
                .get("cache-control")
                .is_some_and(|v| v == "no-store"),
        "own public status/cache policy failed"
    );
    let row: Value = response.json().await?;
    let keys = row.as_object().context("status object")?;
    anyhow::ensure!(
        keys.len() == 7
            && [
                "operationId",
                "state",
                "outcome",
                "responseStatus",
                "replayable",
                "createdAt",
                "finishedAt"
            ]
            .iter()
            .all(|key| keys.contains_key(*key)),
        "public status schema changed: {row}"
    );
    Ok(row)
}
async fn operation_id(response: &reqwest::Response) -> Result<Uuid> {
    Ok(response
        .headers()
        .get("x-operation-id")
        .context("actual operation id")?
        .to_str()?
        .parse()?)
}
async fn case(fixture: &mut WireFixture) -> Result<()> {
    let user = Uuid::new_v4();
    sqlx::query("insert into profiles(user_id,username,nickname,avatar_url,bio) values($1,$2,'public audit','/avatars/audit.png','public audit fixture')")
        .bind(user).bind(format!("audit_{}",&user.simple().to_string()[..12])).execute(&fixture.owner_pool).await?;
    sqlx::query("insert into avatars(user_id,content,content_type,filename) values($1,$2,'image/png','audit.png')")
        .bind(user).bind(vec![137u8,80,78,71,13,10,26,10,0,0,0,13,73,72,68,82,0,0,0,1,0,0,0,1,8,2,0,0,0,144,119,83,222,0,0,0,12,73,68,65,84,120,156,99,248,255,255,63,0,5,254,2,254,13,239,70,184,0,0,0,0,73,69,78,68,174,66,96,130]).execute(&fixture.owner_pool).await?;
    fixture.start_gateway("profile-gateway-1").await?;
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let token = support::bearer_token(&user.to_string(), &["ROLE_USER"]);
    let foreign = support::bearer_token(&Uuid::new_v4().to_string(), &["ROLE_USER"]);
    let uri = format!("{}/api/profile/{user}", fixture.origin);
    let unsigned = http.delete(&uri).send().await?;
    let denied = http.delete(&uri).bearer_auth(&foreign).send().await?;
    anyhow::ensure!(
        unsigned.status() == 401 && denied.status() == 403,
        "actual JWT/ownership guard failed"
    );
    assert_active(&snapshot(fixture, user).await?)?;
    fixture.proof.event(json!({"stage":"public-auth-and-ownership-denied","missingTokenStatus":401,"foreignActorStatus":403,"ownerSnapshot":snapshot(fixture,user).await?}))?;

    sqlx::raw_sql("create function test_public_profile_audit_fault() returns trigger language plpgsql as $$ begin if new.action='PROFILE_DELETED' and new.outcome='SUCCESS' then raise exception 'test public mandatory audit unavailable'; end if; return new; end $$; create trigger test_public_profile_audit_fault before insert on profile_audit_events for each row execute function test_public_profile_audit_fault();")
        .execute(&fixture.owner_pool).await?;
    let failed = http
        .delete(&uri)
        .bearer_auth(&token)
        .header("x-request-id", "public-profile-audit-fault")
        .send()
        .await?;
    let failed_id = operation_id(&failed).await?;
    let failed_http = failed.status().as_u16();
    fixture.proof.event(json!({"stage":"observed-public-audit-failure-response","httpStatus":failed_http,"cacheControl":failed.headers().get("cache-control").and_then(|v|v.to_str().ok()),"operationId":failed_id}))?;
    anyhow::ensure!(
        failed_http == 500
            && failed
                .headers()
                .get("cache-control")
                .is_some_and(|v| v == "no-store"),
        "audit failure must preserve declared500 and UNKNOWN nonstorage policy"
    );
    let failed_body: Value = failed.json().await?;
    let rolled_back = snapshot(fixture, user).await?;
    assert_active(&rolled_back)?;
    let failed_status = status(&http, fixture, &token, failed_id).await?;
    anyhow::ensure!(
        failed_status["state"] == "UNKNOWN"
            && failed_status["outcome"] == "UNKNOWN"
            && failed_status["replayable"] == false,
        "Gateway guessed audit-failure outcome: {failed_status}"
    );
    fixture.proof.event(json!({"stage":"actual-owner-audit-fault-rolls-back-public-deletion","httpStatus":failed_http,"body":failed_body,"operationId":failed_id,"callerStatus":failed_status,"ownerSnapshot":rolled_back}))?;
    sqlx::raw_sql("drop trigger test_public_profile_audit_fault on profile_audit_events; drop function test_public_profile_audit_fault();").execute(&fixture.owner_pool).await?;

    let key = i64::from_be_bytes(Uuid::new_v4().as_bytes()[..8].try_into()?) & i64::MAX;
    sqlx::raw_sql(&format!("create function test_public_profile_audit_gate() returns trigger language plpgsql as $$ begin if new.action='PROFILE_DELETED' and new.outcome='SUCCESS' then perform pg_advisory_xact_lock({key}); end if; return new; end $$; create trigger test_public_profile_audit_gate before insert on profile_audit_events for each row execute function test_public_profile_audit_gate();"))
        .execute(&fixture.owner_pool).await?;
    let mut held = fixture.owner_pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock($1)")
        .bind(key)
        .execute(&mut *held)
        .await?;
    let client = http.clone();
    let crash_uri = uri.clone();
    let crash_token = token.clone();
    let request = tokio::spawn(async move {
        client
            .delete(&crash_uri)
            .bearer_auth(&crash_token)
            .header("x-request-id", "public-profile-audit-crash")
            .send()
            .await
    });
    let start = Instant::now();
    let blocked = loop {
        fixture.owner.alive()?;
        fixture.gateway_alive()?;
        let count:i64=sqlx::query_scalar("select count(*) from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and wait_event_type='Lock' and query like '%insert into profile_audit_events%'").fetch_one(&fixture.owner_pool).await?;
        if count > 0 {
            break count;
        }
        anyhow::ensure!(
            start.elapsed() < Duration::from_millis(650),
            "actual owner audit wait not observed before caller deadline"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    };
    assert_active(&snapshot(fixture, user).await?)?;
    fixture.owner.stop()?;
    let crashed = tokio::time::timeout(Duration::from_secs(5), request).await???;
    let crashed_id = operation_id(&crashed).await?;
    let crashed_http = crashed.status().as_u16();
    anyhow::ensure!(
        matches!(crashed_http, 500 | 503 | 504)
            && crashed
                .headers()
                .get("cache-control")
                .is_some_and(|v| v == "no-store"),
        "owner crash must return declared uncertain failure with no-store"
    );
    let crashed_body: Value = crashed.json().await?;
    assert_active(&snapshot(fixture, user).await?)?;
    let crashed_status = status(&http, fixture, &token, crashed_id).await?;
    anyhow::ensure!(
        crashed_status["state"] == "UNKNOWN"
            && crashed_status["outcome"] == "UNKNOWN"
            && crashed_status["replayable"] == false,
        "Gateway guessed owner crash outcome: {crashed_status}"
    );
    fixture.proof.event(json!({"stage":"actual-profile-sigkill-during-required-audit","blockedOwnerAuditQueries":blocked,"httpStatus":crashed_http,"body":crashed_body,"operationId":crashed_id,"callerStatus":crashed_status,"ownerSnapshot":snapshot(fixture,user).await?}))?;
    held.commit().await?;
    sqlx::raw_sql("drop trigger test_public_profile_audit_gate on profile_audit_events; drop function test_public_profile_audit_gate();").execute(&fixture.owner_pool).await?;
    fixture.restart_owner("http-profile-2").await?;
    assert_active(&snapshot(fixture, user).await?)?;
    fixture.proof.event(json!({"stage":"owner-restart-retains-observed-rollback","newUnkeyedRequestAuthorizedByControllerObservation":true,"automaticClientRetryQualified":false}))?;

    let committed = http
        .delete(&uri)
        .bearer_auth(&token)
        .header("x-request-id", "public-profile-audit-committed")
        .header("user-agent", "profile-public-process-probe")
        .send()
        .await?;
    let committed_id = operation_id(&committed).await?;
    anyhow::ensure!(
        committed.status() == 200 && committed_id != failed_id && committed_id != crashed_id,
        "new unkeyed operation identity/response changed"
    );
    let body: Value = committed.json().await?;
    anyhow::ensure!(
        body == json!({"userId":user,"deleted":true,"accountDeleted":false}),
        "public profile-only schema changed: {body}"
    );
    let after = snapshot(fixture, user).await?;
    anyhow::ensure!(
        !after["profile"]["deleted_at"].is_null()
            && !after["avatar"]["deleted_at"].is_null()
            && after["profile"]["deleted_by"] == user.to_string()
            && after["avatar"]["deleted_by"] == user.to_string()
            && after["profile"]["retention_until"] == after["avatar"]["retention_until"]
            && after["successAudits"] == 1
            && after["receipts"] == 0,
        "actual public commit effect/audit mismatch: {after}"
    );
    let audit:(Uuid,Uuid,String,String)=sqlx::query_as("select actor_id,profile_user_id,request_id,user_agent from profile_audit_events where profile_user_id=$1 and action='PROFILE_DELETED' and outcome='SUCCESS'")
        .bind(user).fetch_one(&fixture.owner_pool).await?;
    anyhow::ensure!(
        audit
            == (
                user,
                user,
                "public-profile-audit-committed".into(),
                "profile-public-process-probe".into()
            ),
        "public metadata/actor not bound to owner audit"
    );
    let committed_status = status(&http, fixture, &token, committed_id).await?;
    anyhow::ensure!(
        committed_status["state"] == "COMPLETED"
            && committed_status["outcome"] == "COMMITTED"
            && committed_status["responseStatus"] == 200,
        "positive public status changed: {committed_status}"
    );
    fixture.proof.event(json!({"stage":"actual-public-profile-only-commit-with-one-owner-audit","operationId":committed_id,"body":body,"callerStatus":committed_status,"ownerSnapshot":after}))?;
    fixture.kill_gateway()?;
    fixture.start_gateway("profile-gateway-2").await?;
    let repeated = http.delete(&uri).bearer_auth(&token).send().await?;
    anyhow::ensure!(
        repeated.status() == 404 && snapshot(fixture, user).await? == after,
        "later delete repeated effect/audit"
    );
    for id in [failed_id, crashed_id] {
        let unresolved = status(&http, fixture, &token, id).await?;
        anyhow::ensure!(
            unresolved["outcome"] == "UNKNOWN" && unresolved["replayable"] == false,
            "later independent commit falsely resolved original operation"
        );
    }
    fixture.proof.event(json!({"stage":"gateway-restart-preserves-audit-and-original-uncertainty","repeatHttpStatus":404,"ownerSnapshot":snapshot(fixture,user).await?,"originalFailureOperationIds":[failed_id,crashed_id],"newCommittedOperationId":committed_id}))?;
    Ok(())
}

#[tokio::test]
async fn actual_public_profile_deletion_requires_atomic_audit_through_fault_and_owner_restart(
) -> Result<()> {
    let Ok(directory) = std::env::var("GATEWAY_TEST_PROFILE_PROCESS_EVIDENCE_DIR") else {
        eprintln!("PROFILE_PROCESS_ACCEPTANCE_NOT_RUN: use native runner --profile-process");
        return Ok(());
    };
    let mut fixture=WireFixture::start_profile(PathBuf::from(directory),"Actual Gateway/Profile executables, test-issued JWT verification and signed mTLS public profile deletion. Owner mandatory audit failure and SIGKILL before commit; observed rollback precedes a new unkeyed request. No seeded journal/outcome, deletion receipt, automatic client retry, live Auth/account deletion, production/fullstack acceptance.").await?;
    let result = case(&mut fixture).await;
    fixture.finish(result).await
}
