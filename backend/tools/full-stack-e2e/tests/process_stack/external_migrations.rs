//! Actual owner commands followed by externally verified serving. The process
//! journey serves through separate runtime logins with explicit service grants.
use anyhow::{ensure, Context, Result};
use sqlx::{Connection, Executor, PgConnection};
use std::{collections::BTreeMap, path::Path, time::Duration};

pub fn configure(environment: &mut BTreeMap<String, String>, prefix: &str) -> Result<()> {
    let key = format!("{prefix}_DATABASE_URL");
    let url = environment
        .get_mut(&key)
        .context("missing fixture database URL")?;
    ensure!(!url.contains('?'), "fixture URL already has query options");
    url.push_str("?options=-csearch_path%3Dshadow");
    environment.insert(
        format!("{prefix}_DATABASE_MIGRATION_MODE"),
        "external".into(),
    );
    Ok(())
}

pub async fn apply(
    binary: &Path,
    prefix: &str,
    environment: &mut BTreeMap<String, String>,
) -> Result<()> {
    let key = format!("{prefix}_DATABASE_URL");
    let url = environment
        .get(&key)
        .context("missing fixture database URL")?;
    let mut owner = PgConnection::connect(url).await?;
    owner.execute("create schema if not exists shadow").await?;
    owner.close().await?;
    let output = tokio::time::timeout(
        Duration::from_secs(90),
        tokio::process::Command::new(binary)
            .arg("--migrate")
            .env_clear()
            .env(&key, url)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .context("external owner command exceeded its budget")??;
    // Do not echo arbitrary child output, which could include connection values.
    ensure!(
        output.status.success(),
        "{prefix} external migration command failed"
    );
    ensure!(
        String::from_utf8_lossy(&output.stdout).contains("database command completed: Migrate"),
        "{prefix} did not acknowledge standalone migration"
    );
    eprintln!("PROCESS_STACK_EXTERNAL_MIGRATED={prefix}");
    let service = binary
        .file_name()
        .and_then(|s| s.to_str())
        .context("missing service name")?;
    let runtime_url = super::runtime_roles::install(url, service, prefix).await?;
    environment.insert(key, runtime_url);
    Ok(())
}
