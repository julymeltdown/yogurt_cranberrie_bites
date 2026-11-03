//! Bounded, failure-only observations of a still-owned disposable PostgreSQL.
use super::PostgresContainer;
use std::future::Future;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};

const LOG_BYTES: usize = 65_536;
const DIAGNOSTIC_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub struct PostgresStartupDiagnostics {
    pub container_id: String,
    pub metadata: String,
    pub endpoint: String,
    pub stdout: String,
    pub stderr: String,
}

/// Call before returning the original startup error and dropping the handle.
/// This reads only the supplied fixture; it never restarts, retains or removes
/// it. Every observation is bounded and failures remain diagnostic strings.
/// Docker Config/Env and the SQL connection URL are deliberately not collected.
pub async fn startup_failure_diagnostics(
    container: &PostgresContainer,
) -> PostgresStartupDiagnostics {
    let metadata = bounded_operation(async {
        // Reuse Testcontainers' configured daemon, not the CLI's current context.
        let client = testcontainers::core::client::docker_client_instance().await?;
        let inspection = client.inspect_container(container.id(), None).await?;
        let state = inspection.state.as_ref();
        Ok(format!(
            "image_id={:?} running={:?} restarting={:?} oom_killed={:?} exit_code={:?}",
            inspection.image,
            state.and_then(|s| s.running),
            state.and_then(|s| s.restarting),
            state.and_then(|s| s.oom_killed),
            state.and_then(|s| s.exit_code),
        ))
    });
    let endpoint = bounded_operation(async {
        let host = container.get_host().await?;
        let port = container.get_host_port_ipv4(5432).await?;
        Ok(format!("host={host:?} port={port}"))
    });
    let (metadata, endpoint, stdout, stderr) = tokio::join!(
        metadata,
        endpoint,
        bounded_log(container.stdout(false), LOG_BYTES, DIAGNOSTIC_TIMEOUT),
        bounded_log(container.stderr(false), LOG_BYTES, DIAGNOSTIC_TIMEOUT),
    );
    PostgresStartupDiagnostics {
        container_id: container.id().to_owned(),
        metadata,
        endpoint,
        stdout,
        stderr,
    }
}

/// Observe a failed fixture startup while its owner still retains the container.
/// Diagnostics cannot replace the operation's original typed error. The caller
/// remains responsible for scope-based cleanup, including on the error path.
pub async fn with_startup_diagnostics<T>(
    container: Option<&PostgresContainer>,
    label: &'static str,
    operation: impl Future<Output = anyhow::Result<T>>,
) -> anyhow::Result<T> {
    let started = std::time::Instant::now();
    match operation.await {
        Ok(value) => Ok(value),
        Err(error) => {
            let elapsed = started.elapsed();
            let phase = if error
                .downcast_ref::<sqlx::migrate::MigrateError>()
                .is_some()
            {
                "migration resolution or execution"
            } else {
                "initial pool configuration or connection"
            };
            if let Some(container) = container {
                use std::io::Write;
                let observed = startup_failure_diagnostics(container).await;
                // A broken diagnostic sink must not replace the startup failure.
                let _ = writeln!(
                    std::io::stderr().lock(),
                    "POSTGRES_FIXTURE_STARTUP_FAILURE label={label:?} phase={phase:?} elapsed_ms={} {observed:?}",
                    elapsed.as_millis(),
                );
            }
            Err(error.context(format!("{label}: {phase}")))
        }
    }
}

async fn bounded_operation(operation: impl Future<Output = anyhow::Result<String>>) -> String {
    match tokio::time::timeout(DIAGNOSTIC_TIMEOUT, operation).await {
        Ok(Ok(value)) => value,
        Ok(Err(_)) => "[observation unavailable]".to_owned(),
        Err(_) => "[observation timed out]".to_owned(),
    }
}

async fn bounded_log(reader: impl AsyncRead + Unpin, limit: usize, budget: Duration) -> String {
    let mut bytes = Vec::new();
    let mut reader = reader.take((limit + 1) as u64);
    let result = tokio::time::timeout(budget, reader.read_to_end(&mut bytes)).await;
    let truncated = bytes.len() > limit;
    bytes.truncate(limit);
    let mut value = String::from_utf8_lossy(&bytes).into_owned();
    let marker = match result {
        Ok(Ok(_)) if truncated => Some("[log truncated]"),
        Ok(Ok(_)) => None,
        Ok(Err(_)) => Some("[log unavailable]"),
        Err(_) => Some("[log timed out]"),
    };
    if let Some(marker) = marker {
        if !value.is_empty() {
            value.push('\n');
        }
        value.push_str(marker);
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tokio::io::ReadBuf;

    #[tokio::test]
    async fn long_log_is_bounded_and_marked() {
        let bytes = b"0123456789";
        assert_eq!(
            bounded_log(&bytes[..], 4, Duration::from_secs(1)).await,
            "0123\n[log truncated]"
        );
        assert_eq!(
            bounded_log(&bytes[..4], 4, Duration::from_secs(1)).await,
            "0123"
        );
    }

    #[tokio::test]
    async fn stalled_log_returns_a_diagnostic_without_waiting_for_eof() {
        use tokio::io::AsyncWriteExt;
        let (reader, mut held_writer) = tokio::io::duplex(16);
        held_writer.write_all(b"partial").await.unwrap();
        assert_eq!(
            bounded_log(reader, 16, Duration::from_millis(5)).await,
            "partial\n[log timed out]"
        );
    }

    struct BrokenReader;
    impl AsyncRead for BrokenReader {
        fn poll_read(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            _: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            Poll::Ready(Err(std::io::Error::other("private transport detail")))
        }
    }

    #[tokio::test]
    async fn failed_log_read_does_not_expose_transport_error_details() {
        assert_eq!(
            bounded_log(BrokenReader, 16, Duration::from_secs(1)).await,
            "[log unavailable]"
        );
    }

    #[tokio::test]
    async fn actual_failed_connection_is_observed_before_owned_container_removal(
    ) -> anyhow::Result<()> {
        let (container, url) = crate::postgres::start_postgres_with_url().await?;
        let options: sqlx::postgres::PgConnectOptions = url.parse()?;
        let failure =
            sqlx::PgPool::connect_with(options.password("deliberately-wrong-fixture-password"))
                .await
                .expect_err("wrong password must fail");
        assert!(matches!(failure, sqlx::Error::Database(_)));
        let observed = startup_failure_diagnostics(&container).await;
        assert_eq!(observed.container_id, container.id());
        assert!(observed.metadata.contains("sha256:"), "{observed:?}");
        assert!(
            observed.metadata.contains("running=Some(true)"),
            "{observed:?}"
        );
        assert!(
            observed.stderr.contains("password authentication failed"),
            "{observed:?}"
        );
        assert!(!observed.metadata.contains("POSTGRES_PASSWORD"));
        assert!(container.is_running().await?);
        let id = container.id().to_owned();
        container.rm().await?;
        let client = testcontainers::core::client::docker_client_instance().await?;
        let error = client
            .inspect_container(&id, None)
            .await
            .expect_err("owned fixture removed");
        assert!(matches!(
            error,
            testcontainers::bollard::errors::Error::DockerResponseServerError {
                status_code: 404,
                ..
            }
        ));
        Ok(())
    }

    #[tokio::test]
    async fn failed_startup_preserves_typed_error_and_drop_removes_owned_fixture(
    ) -> anyhow::Result<()> {
        let (container, url) = crate::postgres::start_postgres_with_url().await?;
        let id = container.id().to_owned();
        let failure = async move {
            let options: sqlx::postgres::PgConnectOptions = url.parse()?;
            with_startup_diagnostics(Some(&container), "controlled startup", async {
                Ok(sqlx::PgPool::connect_with(
                    options.password("deliberately-wrong-fixture-password"),
                )
                .await?)
            })
            .await
            // Like Gateway's early error return, the owned container drops here.
        }
        .await
        .expect_err("bad credentials must stay a startup failure");
        assert!(matches!(
            failure.downcast_ref::<sqlx::Error>(),
            Some(sqlx::Error::Database(_))
        ));
        assert!(failure
            .to_string()
            .contains("initial pool configuration or connection"));
        let client = testcontainers::core::client::docker_client_instance().await?;
        let removed = client
            .inspect_container(&id, None)
            .await
            .expect_err("error-path owner dropped");
        assert!(matches!(
            removed,
            testcontainers::bollard::errors::Error::DockerResponseServerError {
                status_code: 404,
                ..
            }
        ));
        Ok(())
    }
}
