//! Operator fixture for the actual seven-service Sessions journey. This creates
//! only an explicitly synthetic policy, never users/admissions/session receipts.
//! The owning Python driver performs public signup/mail/verify/login/World/delete.
use auth_service::enrollment_policy::{ApprovedPublication, EnrollmentStore, PolicyDocument};
use chrono::{Duration, Utc};
use serde_json::json;
use std::{
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::PathBuf,
};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires the allocated seven-service owner's fresh PG16 before principal provisioning"]
async fn publish_isolated_policy_for_actual_serving_sessions() -> anyhow::Result<()> {
    let (root, pool) = owned_auth().await?;
    let users: i64 = sqlx::query_scalar("select count(*) from users")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(users == 0, "fresh pre-provisioning Auth required");
    let policy = ApprovedPublication {
        revision: Uuid::new_v4(),
        documents: vec![PolicyDocument {
            id: "sessions-isolated-test".into(),
            title: "Isolated Sessions test policy - no production legal approval".into(),
            url: "https://legal.example.test/sessions-isolated-test".into(),
            sha256: "ab".repeat(32),
            required_acknowledgement_id: "sessions-isolated-test-v1".into(),
        }],
        approval_reference: "ISOLATED_SESSIONS_OPERATOR_TEST_INPUT_NOT_PRODUCTION_APPROVAL".into(),
        published_at: Utc::now() - Duration::seconds(1),
        expires_at: Utc::now() + Duration::minutes(10),
    };
    EnrollmentStore::new(pool.clone()).publish(&policy).await?;
    let output = PathBuf::from(std::env::var("MCR_SESSION_DELETION_POLICY_OUTPUT")?);
    anyhow::ensure!(
        output.parent() == Some(root.as_path()),
        "private owner output required"
    );
    let mut stream = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(output)?;
    stream.write_all(&serde_json::to_vec(&json!({"policyRevision":policy.revision,"acknowledgementIds":["sessions-isolated-test-v1"],"productionApproval":false}))?)?;
    pool.close().await;
    Ok(())
}

async fn owned_auth() -> anyhow::Result<(PathBuf, sqlx::PgPool)> {
    let root = PathBuf::from(std::env::var("MCR_SESSION_DELETION_OWNER_ROOT")?);
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        root.is_absolute()
            && info.is_dir()
            && !info.file_type().is_symlink()
            && info.mode() & 0o077 == 0,
        "private owned root required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino())
            == std::env::var("MCR_SESSION_DELETION_OWNER_IDENTITY")?,
        "owner inode changed"
    );
    let root = root.canonicalize()?;
    let marker = root.join("mcr-owner.json");
    let marker_info = std::fs::symlink_metadata(&marker)?;
    anyhow::ensure!(
        marker_info.is_file()
            && !marker_info.file_type().is_symlink()
            && marker_info.mode() & 0o077 == 0
            && marker_info.uid() == info.uid(),
        "private marker required"
    );
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(&marker)?)?;
    anyhow::ensure!(
        value["token"].as_str() == Some(&std::env::var("MCR_SESSION_DELETION_OWNER_TOKEN")?),
        "owned cluster marker required"
    );
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&std::env::var("MCR_SESSION_DELETION_AUTH_URL")?)
        .await?;
    let (directory, database, listeners, version): (String, String, String, i32) = sqlx::query_as(
        "select current_setting('data_directory'),current_database(),current_setting('listen_addresses'),current_setting('server_version_num')::int"
    ).fetch_one(&pool).await?;
    anyhow::ensure!(
        directory == root.join("data").to_string_lossy()
            && database == "mcr_auth"
            && listeners.is_empty()
            && version / 10000 == 16,
        "owned Auth PG16 required"
    );
    Ok((root, pool))
}

async fn native_snapshot(pool: &sqlx::PgPool, user: Uuid) -> anyhow::Result<serde_json::Value> {
    Ok(sqlx::query_scalar(
        "select jsonb_build_object('progress',(select to_jsonb(p) from concert_world_session_deletion_progress p where user_id=$1),'victims',(select coalesce(jsonb_agg(to_jsonb(v) order by victim_id),'[]') from concert_world_session_deletion_victims v where user_id=$1),'fence',(select to_jsonb(f) from concert_world_account_deletion_fences f where user_id=$1))",
    ).bind(user).fetch_one(pool).await?)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires the actual populated main/worker Sessions source round, or its physical-restart continuation"]
async fn real_serving_sessions_exact_replay_and_cancelled_lock_wait() -> anyhow::Result<()> {
    use concert_service::adapters::r#in::deletion_rpc::proto::{
        account_deletion_service_client::AccountDeletionServiceClient, ApplyRequest, Command,
    };
    use platform::{
        grpc_auth::{ServiceIdentityClientInterceptor, Workload},
        grpc_tls::TlsMaterial,
    };
    use sqlx::Row;
    use std::time::Duration;
    use tonic::service::interceptor::InterceptedService;
    let (root, auth) = owned_auth().await?;
    let user: Uuid = std::env::var("MCR_SESSION_DELETION_SUBJECT")?.parse()?;
    anyhow::ensure!(!user.is_nil(), "actual subject required");
    let phase = std::env::var("MCR_SESSION_DELETION_REPLAY_PHASE")?;
    anyhow::ensure!(
        matches!(phase.as_str(), "source" | "restart"),
        "explicit replay phase required"
    );
    let row = sqlx::query(
        "select g.id,g.actor_id,g.retention_until from user_deletion_sagas g join user_deletion_steps s on s.saga_id=g.id and s.target='CONCERT_SESSIONS' where g.user_id=$1 and s.status='SUCCEEDED' and s.attempt_count>0",
    ).bind(user).fetch_one(&auth).await?;
    let retention: chrono::DateTime<Utc> = row.try_get("retention_until")?;
    let command = Command {
        saga_id: row.try_get::<Uuid, _>("id")?.to_string(),
        user_id: user.to_string(),
        actor_id: row.try_get::<Uuid, _>("actor_id")?.to_string(),
        target: 2,
        retention_seconds: retention.timestamp(),
        retention_nanos: retention.timestamp_subsec_nanos() as i32,
    };
    let concert = sqlx::postgres::PgPoolOptions::new()
        .max_connections(3)
        .connect(&std::env::var("MCR_SESSION_DELETION_CONCERT_URL")?)
        .await?;
    let (directory, database): (String, String) =
        sqlx::query_as("select current_setting('data_directory'),current_database()")
            .fetch_one(&concert)
            .await?;
    anyhow::ensure!(
        directory == root.join("data").to_string_lossy() && database == "mcr_concert",
        "same owned native Concert database required"
    );
    let before = native_snapshot(&concert, user).await?;
    anyhow::ensure!(
        before["progress"]["saga_id"] == command.saga_id
            && before["progress"]["completed_at"].is_string()
            && before["progress"]["removed_count"].as_i64().unwrap_or(0) > 0
            && before["victims"]
                .as_array()
                .is_some_and(|v| !v.is_empty() && v.iter().all(|x| x["completed_at"].is_string())),
        "actual populated terminal Sessions receipt required"
    );
    let tls = TlsMaterial::load(
        root.join("pki/auth.crt"),
        root.join("pki/auth.key"),
        root.join("pki/ca.crt"),
    )?;
    let channel = tls
        .endpoint(
            &std::env::var("MCR_SESSION_DELETION_CONCERT_ENDPOINT")?,
            Workload::Concert,
        )?
        .connect()
        .await?;
    let client = AccountDeletionServiceClient::new(InterceptedService::new(
        channel,
        ServiceIdentityClientInterceptor::new(
            Workload::Auth,
            &std::env::var("AUTH_SERVICE_CREDENTIAL")?,
        )?,
    ));
    let request = |duration: Duration| -> anyhow::Result<_> {
        let nonce = Uuid::new_v4().as_bytes().to_vec();
        let expires = Utc::now().timestamp_micros() + duration.as_micros() as i64;
        let mut request = tonic::Request::new(ApplyRequest {
            command: Some(command.clone()),
            request_nonce: nonce,
            expires_unix_micros: expires,
        });
        request.set_timeout(duration);
        Ok(request)
    };
    let first = request(Duration::from_secs(3))?;
    let exact = first.get_ref().clone();
    let response = client.clone().apply(first).await?;
    anyhow::ensure!(
        response
            .metadata()
            .get("cache-control")
            .and_then(|v| v.to_str().ok())
            == Some("no-store"),
        "actual no-store RPC required"
    );
    let first = response.into_inner();
    anyhow::ensure!(
        first.command == Some(command.clone())
            && first.request_nonce == exact.request_nonce
            && first.expires_unix_micros == exact.expires_unix_micros
            && first.expires_unix_micros > Utc::now().timestamp_micros()
            && first.fence_revision > 0
            && first.source_binding.len() == 32
            && first.source_binding.iter().any(|b| *b != 0),
        "exact authenticated native Sessions replay required"
    );
    anyhow::ensure!(
        native_snapshot(&concert, user).await? == before,
        "replay mutated terminal receipt"
    );
    let mut blocker = concert.begin().await?;
    let blocker_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await?;
    concert_service::world::account_deletion::lock_user(&mut blocker, user).await?;
    let pending = request(Duration::from_millis(300))?;
    let expired_request = pending.get_ref().clone();
    let mut wire = client.clone();
    let task = tokio::spawn(async move { wire.apply(pending).await });
    let until = tokio::time::Instant::now() + Duration::from_millis(250);
    let mut saw_native_wait = false;
    while tokio::time::Instant::now() < until && !task.is_finished() {
        saw_native_wait = sqlx::query_scalar(
            "select exists(select 1 from pg_stat_activity a where datname='mcr_concert' and $1=any(pg_blocking_pids(a.pid)) and wait_event='advisory')",
        ).bind(blocker_pid).fetch_one(&concert).await?;
        if saw_native_wait {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let cancelled = tokio::time::timeout(Duration::from_secs(1), task).await??;
    anyhow::ensure!(
        saw_native_wait && cancelled.is_err(),
        "actual bounded native lock wait must not ACK"
    );
    blocker.rollback().await?;
    tokio::time::sleep(Duration::from_millis(40)).await;
    anyhow::ensure!(
        native_snapshot(&concert, user).await? == before,
        "cancelled native wait mutated receipt"
    );
    // The original expiry is preserved even though a fresh short transport
    // timeout is needed to deliver this explicit stale-request rejection.
    anyhow::ensure!(
        expired_request.expires_unix_micros <= Utc::now().timestamp_micros(),
        "original expiry not reached"
    );
    let mut stale = tonic::Request::new(expired_request);
    stale.set_timeout(Duration::from_millis(300));
    anyhow::ensure!(
        client.clone().apply(stale).await.is_err(),
        "expired operation ACKed"
    );
    let fresh = request(Duration::from_secs(3))?;
    let exact = fresh.get_ref().clone();
    let final_receipt = client.clone().apply(fresh).await?.into_inner();
    anyhow::ensure!(
        final_receipt.command == Some(command.clone())
            && final_receipt.request_nonce == exact.request_nonce
            && final_receipt.expires_unix_micros == exact.expires_unix_micros
            && final_receipt.expires_unix_micros > Utc::now().timestamp_micros()
            && final_receipt.fence_revision == first.fence_revision
            && final_receipt.source_binding == first.source_binding
            && native_snapshot(&concert, user).await? == before,
        "fresh exact replay changed terminal identity"
    );
    let value = json!({"phase":phase,"command":{"sagaId":command.saga_id,
        "userId":command.user_id,"actorId":command.actor_id,"target":command.target,
        "retentionSeconds":command.retention_seconds,"retentionNanos":command.retention_nanos},"snapshot":before,
        "sourceBinding":first.source_binding,"fenceRevision":first.fence_revision,
        "actualNativeLockWait":saw_native_wait,"cancelledNoAck":true,"expiredNoAck":true});
    let path = root.join(format!("sessions-replay-{phase}.json"));
    let mut stream = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    // Generated protobuf messages do not promise a public JSON representation.
    stream.write_all(&serde_json::to_vec(&value)?)?;
    concert.close().await;
    auth.close().await;
    Ok(())
}
