use crate::container::{retry_transient_start, CONTAINER_START_ATTEMPTS, CONTAINER_START_BACKOFF};
use anyhow::Result;
use testcontainers::ImageExt;
use testcontainers::{runners::AsyncRunner, ContainerAsync};
use testcontainers_modules::postgres::Postgres;

mod diagnostics;
pub use diagnostics::{
    startup_failure_diagnostics, with_startup_diagnostics, PostgresStartupDiagnostics,
};

pub type PostgresContainer = ContainerAsync<Postgres>;

/// Start PostgreSQL and return the container together with a connection URL.
///
/// Fixtures that need the URL themselves must use this rather than calling
/// `Postgres::start()` directly. A direct start skips the transient-failure
/// classification below, so a Docker create timeout on a loaded host surfaces as
/// an immediate test failure instead of a bounded retry.
pub async fn start_postgres_with_url() -> Result<(PostgresContainer, String)> {
    let container = start_postgres().await?;
    let host = container.get_host().await?;
    let port = container.get_host_port_ipv4(5432).await?;
    Ok((
        container,
        format!("postgres://postgres:postgres@{host}:{port}/postgres"),
    ))
}

// Match docker-compose.parity.yml and the release-image lifecycle substrate.
// testcontainers-modules 0.15 otherwise silently selects PostgreSQL 11.
/// Start the disposable PostgreSQL fixture, or an isolated database in a
/// runner-owned native cluster when MCR_TEST_POSTGRES_DATABASE_URL is set.
///
/// The native path creates a fresh database from template0 and never reuses a
/// database name. Callers that need container-only diagnostics receive None.
pub async fn start_postgres_with_url_or_native() -> Result<(Option<PostgresContainer>, String)> {
    match std::env::var("MCR_TEST_POSTGRES_DATABASE_URL") {
        Ok(url) if !url.trim().is_empty() => {
            anyhow::ensure!(
                !url.contains('?') && !url.contains('#'),
                "native fixture URL must not carry options or fragments"
            );
            let admin = sqlx::PgPool::connect(&url).await?;
            static FIXTURE_SEQUENCE: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(0);
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos();
            let sequence =
                FIXTURE_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let database =
                format!("mcr_fixture_{}_{nanos}_{sequence}", std::process::id());
            sqlx::query(&format!("create database {database} template template0"))
                .execute(&admin)
                .await?;
            admin.close().await;
            let base = url
                .rsplit_once('/')
                .map(|(base, _)| base)
                .ok_or_else(|| anyhow::anyhow!("native fixture URL database"))?;
            Ok((None, format!("{base}/{database}")))
        }
        Ok(_) => {
            let (container, url) = start_postgres_with_url().await?;
            Ok((Some(container), url))
        }
        Err(std::env::VarError::NotPresent) => {
            let (container, url) = start_postgres_with_url().await?;
            Ok((Some(container), url))
        }
        Err(error) => Err(error.into()),
    }
}

pub async fn start_postgres() -> Result<PostgresContainer> {
    // A loaded host can time out Docker's create call even though the daemon is
    // healthy. Retry only that transient infrastructure error; the container
    // still has to start.
    retry_transient_start(
        CONTAINER_START_ATTEMPTS,
        CONTAINER_START_BACKOFF,
        || async {
            Postgres::default()
                .with_tag("16")
                .start()
                .await
                .map_err(anyhow::Error::from)
        },
    )
    .await
}
