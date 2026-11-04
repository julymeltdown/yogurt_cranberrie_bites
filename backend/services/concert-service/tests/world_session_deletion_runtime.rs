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
