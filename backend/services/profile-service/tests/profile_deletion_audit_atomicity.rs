//! Real PostgreSQL failure controls for mandatory profile-deletion audit.
#[path = "support/mod.rs"]
mod support;

use anyhow::{Context, Result};
use profile_service::adapters::r#in::grpc::server::proto;
use profile_service::application::dto::profile_dto::{DeleteProfileDto, UpdateProfileDto};
use profile_service::application::errors::ProfileError;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::time::{Duration, Instant};
use uuid::Uuid;

async fn seed(app: &support::TestApp) -> Result<Uuid> {
    let user = Uuid::new_v4();
    app.state
        .update_profile
        .execute(UpdateProfileDto {
            user_id: user.to_string(),
            username: format!("audit_{}", &user.simple().to_string()[..12]),
            nickname: "audit fixture".into(),
            avatar_url: "/avatars/audit.png".into(),
            bio: "audit fixture".into(),
        })
        .await?;
    sqlx::query("insert into avatars(user_id,content,content_type,filename) values ($1,$2,'image/png','audit.png')")
        .bind(user).bind(support::png_fixture()).execute(&app.pool).await?;
    Ok(user)
}
fn request(user: Uuid) -> DeleteProfileDto {
    DeleteProfileDto {
        user_id: user.to_string(),
        actor_id: user.to_string(),
        request_id: Some(" audit-delete ".into()),
        ip_address: Some(" 203.0.113.72 ".into()),
        user_agent: Some(" audit-probe ".into()),
    }
}
async fn snapshot(pool: &PgPool, user: Uuid) -> Result<Value> {
    let raw: String = sqlx::query_scalar("select json_build_object('profileDeleted',p.deleted_at is not null,'avatarDeleted',a.deleted_at is not null,'deletedBy',p.deleted_by,'reason',p.delete_reason,'retentionUntil',p.retention_until,'avatarRetentionUntil',a.retention_until,'successAudits',(select count(*) from profile_audit_events where profile_user_id=$1 and action='PROFILE_DELETED' and outcome='SUCCESS'))::text from profiles p join avatars a using(user_id) where p.user_id=$1")
        .bind(user).fetch_one(pool).await?;
    Ok(serde_json::from_str(&raw)?)
}
async fn audit_gate(pool: &PgPool, user: Uuid, key: i64) -> Result<()> {
    sqlx::raw_sql(&format!("create function test_deletion_audit_gate() returns trigger language plpgsql as $$ begin if new.profile_user_id='{user}'::uuid and new.action='PROFILE_DELETED' and new.outcome='SUCCESS' then perform pg_advisory_xact_lock({key}); end if; return new; end $$; create trigger test_deletion_audit_gate before insert on profile_audit_events for each row execute function test_deletion_audit_gate();"))
        .execute(pool).await?;
    Ok(())
}
async fn waiting_audit_backend(pool: &PgPool) -> Result<i32> {
    let start = Instant::now();
    loop {
        let pid: Option<i32> = sqlx::query_scalar("select pid from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and wait_event_type='Lock' and query like '%insert into profile_audit_events%' limit 1")
            .fetch_optional(pool).await?;
        if let Some(pid) = pid {
            return Ok(pid);
        }
        anyhow::ensure!(
            start.elapsed() < Duration::from_secs(5),
            "real audit insert did not reach fixture gate"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::test]
async fn audit_insert_failure_rolls_back_profile_and_avatar_then_retry_succeeds() -> Result<()> {
    let app = support::start_postgres_app(false).await?;
    let user = seed(&app).await?;
    sqlx::raw_sql("create function test_reject_deletion_audit() returns trigger language plpgsql as $$ begin if new.action='PROFILE_DELETED' and new.outcome='SUCCESS' then raise exception 'test mandatory audit unavailable'; end if; return new; end $$; create trigger test_reject_deletion_audit before insert on profile_audit_events for each row execute function test_reject_deletion_audit();")
        .execute(&app.pool).await?;
    let error = app
        .state
        .delete_profile
        .execute(request(user))
        .await
        .expect_err("audit failure must be returned");
    anyhow::ensure!(
        error == profile_service::application::errors::ProfileError::Persistence,
        "wrong failure: {error}"
    );
    let failed = snapshot(&app.pool, user).await?;
    eprintln!(
        "PROFILE_DELETE_AUDIT_OBSERVATION {}",
        json!({"stage":"audit-insert-failed","snapshot":failed})
    );
    anyhow::ensure!(
        failed["profileDeleted"] == false
            && failed["avatarDeleted"] == false
            && failed["successAudits"] == 0,
        "mandatory audit failure left committed deletion: {failed}"
    );
    sqlx::raw_sql("drop trigger test_reject_deletion_audit on profile_audit_events; drop function test_reject_deletion_audit();").execute(&app.pool).await?;
    let recovered = app.state.delete_profile.execute(request(user)).await?;
    anyhow::ensure!(
        recovered.deleted && !recovered.account_deleted,
        "profile-only delete response changed"
    );
    let row = snapshot(&app.pool, user).await?;
    anyhow::ensure!(
        row["profileDeleted"] == true && row["avatarDeleted"] == true && row["successAudits"] == 1,
        "recovery missing atomic audit: {row}"
    );
    app.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn audit_wait_hides_uncommitted_deletion_and_concurrent_delete_writes_one_audit() -> Result<()>
{
    let app = support::start_postgres_app(false).await?;
    let user = seed(&app).await?;
    let key = i64::from_be_bytes(Uuid::new_v4().as_bytes()[..8].try_into()?) & i64::MAX;
    audit_gate(&app.pool, user, key).await?;
    let mut held = app.pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock($1)")
        .bind(key)
        .execute(&mut *held)
        .await?;
    let writer = app.state.delete_profile.clone();
    let first = tokio::spawn(async move { writer.execute(request(user)).await });
    let pid = waiting_audit_backend(&app.pool).await?;
    let before = snapshot(&app.pool, user).await?;
    anyhow::ensure!(
        before["profileDeleted"] == false
            && before["avatarDeleted"] == false
            && before["successAudits"] == 0,
        "deletion became visible before audit commit: {before}"
    );
    let writer = app.state.delete_profile.clone();
    let second = tokio::spawn(async move { writer.execute(request(user)).await });
    let grpc = support::grpc_server(app.state.clone());
    let third = tokio::spawn(async move {
        let mut wire = tonic::Request::new(proto::DeleteProfileRequest {
            user_id: user.to_string(),
            actor_id: user.to_string(),
            request_id: "concurrent-grpc-loser".into(),
            ip_address: "203.0.113.72".into(),
            user_agent: "audit-probe".into(),
        });
        wire.extensions_mut()
            .insert(platform::grpc_auth::VerifiedCaller(
                platform::grpc_auth::Workload::Gateway,
            ));
        proto::profile_service_server::ProfileService::delete_profile(&grpc, wire).await
    });
    let start = Instant::now();
    loop {
        let blocked:i64=sqlx::query_scalar("select count(*) from pg_stat_activity where datname=current_database() and pid<>$1 and wait_event_type='Lock' and query like '%update profiles%' and query like '%deleted_at = now()%'")
            .bind(pid).fetch_one(&app.pool).await?;
        if blocked >= 2 {
            break;
        }
        anyhow::ensure!(
            start.elapsed() < Duration::from_secs(5),
            "concurrent delete did not wait on actual profile update"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    held.commit().await?;
    let response = tokio::time::timeout(Duration::from_secs(5), first).await???;
    anyhow::ensure!(
        response.deleted && !response.account_deleted,
        "delete response changed"
    );
    let loser = tokio::time::timeout(Duration::from_secs(5), second)
        .await??
        .expect_err("one concurrent delete must lose");
    anyhow::ensure!(
        loser == ProfileError::ProfileNotFound,
        "wrong concurrent failure: {loser}"
    );
    let wire_loser = tokio::time::timeout(Duration::from_secs(5), third)
        .await??
        .expect_err("concurrent gRPC delete must lose");
    anyhow::ensure!(
        wire_loser.code() == tonic::Code::NotFound && wire_loser.message() == "Profile not found",
        "concurrent wire failure changed: {wire_loser}"
    );
    let row = snapshot(&app.pool, user).await?;
    anyhow::ensure!(
        row["profileDeleted"] == true
            && row["avatarDeleted"] == true
            && row["successAudits"] == 1
            && row["deletedBy"] == user.to_string()
            && row["reason"] == "user_request"
            && row["retentionUntil"] == row["avatarRetentionUntil"],
        "atomic deletion identity/retention/audit mismatch: {row}"
    );
    let audit:(String,String,String,String)=sqlx::query_as("select request_id,ip_address,user_agent,metadata_json from profile_audit_events where profile_user_id=$1 and action='PROFILE_DELETED' and outcome='SUCCESS'")
        .bind(user).fetch_one(&app.pool).await?;
    let metadata: Value = serde_json::from_str(&audit.3)?;
    anyhow::ensure!(
        audit.0 == "audit-delete"
            && audit.1 == "203.0.113.72"
            && audit.2 == "audit-probe"
            && metadata["username"] == format!("audit_{}", &user.simple().to_string()[..12]),
        "audit metadata changed"
    );
    let retention: chrono::DateTime<chrono::Utc> =
        serde_json::from_value(metadata["retentionUntil"].clone())?;
    let remaining = retention - chrono::Utc::now();
    anyhow::ensure!(
        remaining > chrono::Duration::days(29) && remaining <= chrono::Duration::days(30),
        "existing30-day retention changed"
    );
    eprintln!(
        "PROFILE_DELETE_AUDIT_OBSERVATION {}",
        json!({"stage":"atomic-commit-concurrent-loser","snapshot":row})
    );
    app.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn terminating_owner_backend_during_audit_wait_rolls_back_effect_and_allows_retry(
) -> Result<()> {
    let app = support::start_postgres_app(false).await?;
    let user = seed(&app).await?;
    let key = i64::from_be_bytes(Uuid::new_v4().as_bytes()[..8].try_into()?) & i64::MAX;
    audit_gate(&app.pool, user, key).await?;
    let mut held = app.pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock($1)")
        .bind(key)
        .execute(&mut *held)
        .await?;
    let writer = app.state.delete_profile.clone();
    let task = tokio::spawn(async move { writer.execute(request(user)).await });
    let pid = waiting_audit_backend(&app.pool).await?;
    let terminated: bool = sqlx::query_scalar("select pg_terminate_backend($1)")
        .bind(pid)
        .fetch_one(&app.pool)
        .await?;
    anyhow::ensure!(terminated, "owned test backend was not terminated");
    let interrupted = tokio::time::timeout(Duration::from_secs(5), task).await??;
    anyhow::ensure!(
        interrupted.is_err(),
        "backend termination must fail operation"
    );
    let failed = snapshot(&app.pool, user).await?;
    anyhow::ensure!(
        failed["profileDeleted"] == false
            && failed["avatarDeleted"] == false
            && failed["successAudits"] == 0,
        "interrupted audit left committed effect: {failed}"
    );
    held.commit().await?;
    sqlx::raw_sql("drop trigger test_deletion_audit_gate on profile_audit_events; drop function test_deletion_audit_gate();").execute(&app.pool).await?;
    app.state
        .delete_profile
        .execute(request(user))
        .await
        .context("retry interrupted deletion")?;
    let recovered = snapshot(&app.pool, user).await?;
    anyhow::ensure!(
        recovered["profileDeleted"] == true
            && recovered["avatarDeleted"] == true
            && recovered["successAudits"] == 1,
        "interrupted retry duplicated/lost audit: {recovered}"
    );
    eprintln!(
        "PROFILE_DELETE_AUDIT_OBSERVATION {}",
        json!({"stage":"backend-terminated-then-retried","beforeRetry":failed,"afterRetry":recovered})
    );
    app.pool.close().await;
    Ok(())
}

/// T14.3: the account-hide path's mandatory audit must share its effect
/// transaction, the way deletion's already does.
///
/// `update_visibility_with_receipt` writes the `UPDATE_PRIVACY` audit and the
/// receipt in the same transaction as the visibility change, but only the
/// deletion path had a failure-injection regression showing that an unavailable
/// audit rolls the effect back. Without this case, a later edit could move the
/// audit onto the pool and a hidden account would survive an unaudited write.
#[tokio::test]
async fn privacy_audit_failure_rolls_back_hidden_visibility_then_retry_succeeds() -> Result<()> {
    use profile_service::application::dto::mutation_dto::MutationContextDto;
    use profile_service::application::dto::profile_dto::UpdateProfilePrivacyDto;

    let app = support::start_postgres_app(false).await?;
    let user = seed(&app).await?;
    // Real failure control: any attempt to record the success audit fails.
    sqlx::raw_sql(
        "create function test_reject_privacy_audit() returns trigger language plpgsql as $$ begin if new.action='UPDATE_PRIVACY' and new.outcome='SUCCESS' then raise exception 'test mandatory privacy audit unavailable'; end if; return new; end $$; create trigger test_reject_privacy_audit before insert on profile_audit_events for each row execute function test_reject_privacy_audit();",
    )
    .execute(&app.pool)
    .await?;

    let context = MutationContextDto {
        operation_id: Uuid::new_v4(),
        actor_id: user,
        idempotency_key: Some(format!("audit-privacy-{user}")),
        lookup_only: false,
    };
    let request = || UpdateProfilePrivacyDto {
        user_id: user.to_string(),
        actor_id: user.to_string(),
        account_visibility: "PRIVATE".into(),
    };
    let refused = app
        .state
        .update_privacy
        .execute_with_operation(request(), Some(context.clone()))
        .await;
    anyhow::ensure!(
        refused.is_err(),
        "an unavailable mandatory audit must fail the hide: {refused:?}"
    );

    let rolled_back = privacy_snapshot(&app.pool, user).await?;
    anyhow::ensure!(
        rolled_back["visibility"] == "PUBLIC"
            && rolled_back["successAudits"] == 0
            && rolled_back["receipts"] == 0,
        "mandatory privacy audit failure left a committed hide: {rolled_back}"
    );

    sqlx::raw_sql(
        "drop trigger test_reject_privacy_audit on profile_audit_events; drop function test_reject_privacy_audit();",
    )
    .execute(&app.pool)
    .await?;
    let recovered = app
        .state
        .update_privacy
        .execute_with_operation(request(), Some(context))
        .await?;
    anyhow::ensure!(
        recovered.account_visibility == "PRIVATE",
        "retry did not hide the account: {recovered:?}"
    );
    let committed = privacy_snapshot(&app.pool, user).await?;
    anyhow::ensure!(
        committed["visibility"] == "PRIVATE"
            && committed["successAudits"] == 1
            && committed["receipts"] == 1,
        "retry duplicated or lost the audited effect: {committed}"
    );
    eprintln!(
        "PROFILE_PRIVACY_AUDIT_OBSERVATION {}",
        json!({
            "stage": "privacy-audit-unavailable-then-retried",
            "beforeRetry": rolled_back,
            "afterRetry": committed
        })
    );
    app.pool.close().await;
    Ok(())
}

async fn privacy_snapshot(pool: &PgPool, user: Uuid) -> Result<Value> {
    let raw: String = sqlx::query_scalar(
        "select json_build_object( 'visibility', p.account_visibility, 'successAudits', (select count(*) from profile_audit_events where profile_user_id=p.user_id and action='UPDATE_PRIVACY' and outcome='SUCCESS'), 'receipts', (select count(*) from profile_mutation_receipts where actor_id=p.user_id) )::text from profiles p where p.user_id=$1",
    )
    .bind(user)
    .fetch_one(pool)
    .await?;
    Ok(serde_json::from_str(&raw)?)
}
