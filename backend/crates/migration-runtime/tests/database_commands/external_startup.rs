//! Negative acceptance through each actual main function and environment loader.
//! Held listener ports prevent an accidentally accepted case from exposing a
//! service. Expected migration errors must win over bind/configuration errors.
use anyhow::{ensure, Context, Result};
use std::{ffi::OsStr, net::TcpListener, path::Path, time::Duration};

pub async fn rejected(
    binary: &Path,
    prefix: &str,
    url: &str,
    mode: &OsStr,
    expected: &str,
) -> Result<()> {
    let listeners = [
        TcpListener::bind("127.0.0.1:0")?,
        TcpListener::bind("127.0.0.1:0")?,
        TcpListener::bind("127.0.0.1:0")?,
    ];
    let mut command = tokio::process::Command::new(binary);
    command
        .env_clear()
        .kill_on_drop(true)
        .env(format!("{prefix}_RUNTIME_MODE"), "parity")
        .env(format!("{prefix}_DATABASE_URL"), url)
        .env(format!("{prefix}_DATABASE_MIGRATION_MODE"), mode);
    for (kind, listener) in ["HTTP", "METRICS", "GRPC"].iter().zip(&listeners) {
        command.env(
            format!("{prefix}_{kind}_ADDR"),
            listener.local_addr()?.to_string(),
        );
    }
    let output = tokio::time::timeout(Duration::from_secs(15), command.output())
        .await
        .context("invalid external startup did not exit within its budget")??;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        !output.status.success(),
        "{prefix} accepted invalid external startup"
    );
    ensure!(
        text.contains(expected),
        "{prefix} failed outside expected startup stage {expected:?}"
    );
    ensure!(
        !text.contains("PRIVATE_MODE_MARKER"),
        "{prefix} exposed invalid mode input"
    );
    ensure!(
        !text.contains("fixture-password"),
        "{prefix} exposed fixture credentials"
    );
    Ok(())
}
