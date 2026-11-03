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
