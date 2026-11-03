//! Database-only service commands and read-only migration receipt verification.
//! This infrastructure crate must not be imported by domain/application crates.
use anyhow::{ensure, Context, Result};
use sqlx::{
    migrate::Migrator,
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::{ffi::OsString, path::Path, str::FromStr, time::Duration};

/// Startup preserves compatibility; external verifies owner-applied schema.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MigrationMode {
    #[default]
    Startup,
    External,
}

impl MigrationMode {
    /// Only an absent variable defaults. Empty, malformed and non-Unicode
    /// settings fail closed, without echoing the supplied value.
    pub fn from_env_value(value: std::result::Result<String, std::env::VarError>) -> Result<Self> {
        match value {
            Err(std::env::VarError::NotPresent) => Ok(Self::Startup),
            Ok(value) if value == "startup" => Ok(Self::Startup),
            Ok(value) if value == "external" => Ok(Self::External),
            _ => anyhow::bail!("database migration mode must be startup or external"),
        }
    }
}

/// A service-owned boolean metadata query. Probes execute in the same bounded,
/// read-only snapshot as receipts, without reading protected business rows.
/// This checks selected required objects, not exhaustive schema drift or grants.
#[derive(Clone, Copy, Debug)]
pub struct SchemaProbe {
    pub name: &'static str,
    pub sql: &'static str,
}

/// Standalone owner commands and external serving share public. Startup keeps
/// its existing URL schema selection, including isolated local test schemas.
pub fn serving_options(options: PgConnectOptions, mode: MigrationMode) -> PgConnectOptions {
    match mode {
        MigrationMode::Startup => options,
        MigrationMode::External => options.options([("search_path", "public")]),
    }
}

/// Called before building service state, starting workers or binding listeners.
pub async fn prepare_database(
    pool: &PgPool,
    directory: &Path,
    mode: MigrationMode,
    probes: &[SchemaProbe],
) -> Result<()> {
    let migrator = load_migrations(directory).await?;
    match mode {
        MigrationMode::Startup => migrator
            .run(pool)
            .await
            .context("apply database migrations"),
        MigrationMode::External => verify_schema(pool, &migrator, probes).await,
    }
}

/// External serving requires both exact receipts and a nonempty service probe
/// set. Verification cannot create the ledger or invoke the SQLx migrator.
pub async fn verify_schema(
    pool: &PgPool,
    migrator: &Migrator,
    probes: &[SchemaProbe],
) -> Result<()> {
    ensure!(
        !probes.is_empty(),
        "external migration mode requires schema probes"
    );
    tokio::time::timeout(
        Duration::from_secs(30),
        verify_state(pool, migrator, probes),
    )
    .await
    .context("database schema verification exceeded 30 seconds")?
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Serve,
    Migrate,
    Verify,
    Help,
}

/// Parse before reading application configuration or opening any resources.
/// Do not echo unknown arguments: a mistaken argument can contain credentials.
pub fn parse_command(args: impl IntoIterator<Item = OsString>) -> Result<Command> {
    let mut args = args.into_iter();
    let Some(arg) = args.next() else {
        return Ok(Command::Serve);
    };
    ensure!(
        args.next().is_none(),
        "expected at most one service command"
    );
    match arg.to_str() {
        Some("--migrate") => Ok(Command::Migrate),
        Some("--verify-migrations") => Ok(Command::Verify),
        Some("--help") | Some("-h") => Ok(Command::Help),
        _ => anyhow::bail!("unknown service command; use --help"),
    }
}

/// Return true after handling a standalone command. Serve returns false without
/// inspecting the URL, SQL directory, peers, providers, listeners or telemetry.
pub async fn handle_command(
    args: impl IntoIterator<Item = OsString>,
    database_variable: &'static str,
    migration_directory: &Path,
) -> Result<bool> {
    let command = parse_command(args)?;
    match command {
        Command::Serve => return Ok(false),
        Command::Help => {
            println!("Usage: service [--migrate | --verify-migrations | --help]\nNo arguments starts the service. Database commands use the public schema and require {database_variable} and the packaged migration directory; no application/provider credentials are loaded.");
            return Ok(true);
        }
        Command::Migrate | Command::Verify => {}
    }
    let url = std::env::var(database_variable)
        .map_err(|_| anyhow::anyhow!("database command requires valid {database_variable}"))?;
    ensure!(
        !url.trim().is_empty(),
        "database command URL must not be empty"
    );
    let migrator = load_migrations(migration_directory).await?;
    // Service schemas and receipts belong to public. Pin it for BOTH execution
    // and verification; a URL-supplied search_path must not redirect migrations.
    let options = PgConnectOptions::from_str(&url)
        .map_err(|_| anyhow::anyhow!("database command: invalid connection options"))?
        .options([("search_path", "public")]);
    // URL parser/server errors can echo user-controlled option values. This is
    // the CLI output boundary: retain detailed errors only in library APIs.
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|_| anyhow::anyhow!("database command: connection failed"))?;
    let result = match command {
        Command::Migrate => migrator
            .run(&pool)
            .await
            .map_err(|_| anyhow::anyhow!("database command: migration failed")),
        Command::Verify => verify_receipts_with_timeout(&pool, &migrator)
            .await
            .map_err(|_| anyhow::anyhow!("database command: receipt verification failed")),
        _ => unreachable!(),
    };
    pool.close().await;
    result?;
    println!("database command completed: {command:?}");
    Ok(true)
}

pub async fn load_migrations(directory: &Path) -> Result<Migrator> {
    let migrator = Migrator::new(directory)
        .await
        .context("load packaged database migrations")?;
    let mut previous = None;
    for migration in migrator
        .iter()
        .filter(|m| m.migration_type.is_up_migration())
    {
        ensure!(
            migration.version > 0 && previous.is_none_or(|p| migration.version > p),
            "packaged migrations must have unique increasing positive versions"
        );
        previous = Some(migration.version);
    }
    ensure!(
        previous.is_some(),
        "packaged forward migration set must not be empty"
    );
    Ok(migrator)
}

/// Verify exact version/checksum/success receipts in one read-only snapshot.
/// Unlike Migrator::run, this never creates the ledger or applies DDL. Receipt
/// agreement alone is not an exhaustive physical-schema or privilege audit.
pub async fn verify_receipts(pool: &PgPool, migrator: &Migrator) -> Result<()> {
    verify_state(pool, migrator, &[]).await
}

async fn verify_state(pool: &PgPool, migrator: &Migrator, probes: &[SchemaProbe]) -> Result<()> {
    let expected: Vec<_> = migrator
        .iter()
        .filter(|m| m.migration_type.is_up_migration())
        .collect();
    ensure!(
        !expected.is_empty(),
        "packaged forward migration set must not be empty"
    );
    let row_limit = i64::try_from(expected.len())
        .context("migration count overflow")?
        .checked_add(1)
        .context("migration count overflow")?;
    let mut tx = pool
        .begin()
        .await
        .context("begin migration receipt verification")?;
    sqlx::query("set transaction isolation level repeatable read, read only")
        .execute(&mut *tx)
        .await?;
    sqlx::query("set local statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("set local lock_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    let rows: Vec<(i64, bool, Vec<u8>)> = sqlx::query_as(
        "select version, success, checksum from public._sqlx_migrations order by version limit $1",
    )
    .bind(row_limit)
    .fetch_all(&mut *tx)
    .await
    .context("read existing migration receipts")?;
    ensure!(
        rows.len() == expected.len(),
        "migration receipt count differs from packaged migrations"
    );
    for ((version, success, checksum), migration) in rows.iter().zip(expected) {
        ensure!(
            *version == migration.version,
            "migration receipt version differs from packaged migrations"
        );
        ensure!(
            *success,
            "migration receipt records an unsuccessful migration"
        );
        ensure!(
            checksum.as_slice() == migration.checksum.as_ref(),
            "migration receipt checksum differs from packaged migrations"
        );
    }
    for probe in probes {
        let present: bool = sqlx::query_scalar(probe.sql)
            .fetch_one(&mut *tx)
            .await
            .with_context(|| format!("read required schema probe: {}", probe.name))?;
        ensure!(present, "required schema probe failed: {}", probe.name);
    }
    tx.commit()
        .await
        .context("finish read-only migration receipt verification")?;
    Ok(())
}

/// Optional bounded wrapper for callers that require one total verification
/// budget in addition to server-side per-statement limits.
pub async fn verify_receipts_with_timeout(pool: &PgPool, migrator: &Migrator) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(30), verify_receipts(pool, migrator))
        .await
        .context("migration receipt verification exceeded 30 seconds")?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_mode_defaults_only_when_absent_and_never_echoes_invalid_values() {
        assert_eq!(
            MigrationMode::from_env_value(Err(std::env::VarError::NotPresent)).unwrap(),
            MigrationMode::Startup
        );
        assert_eq!(
            MigrationMode::from_env_value(Ok("startup".into())).unwrap(),
            MigrationMode::Startup
        );
        assert_eq!(
            MigrationMode::from_env_value(Ok("external".into())).unwrap(),
            MigrationMode::External
        );
        for value in [
            "",
            " ",
            "External",
            " external",
            "external ",
            "secret-invalid-mode",
        ] {
            let error = MigrationMode::from_env_value(Ok(value.into())).unwrap_err();
            assert_eq!(
                error.to_string(),
                "database migration mode must be startup or external"
            );
        }
        let error = MigrationMode::from_env_value(Err(std::env::VarError::NotUnicode(
            OsString::from("secret-invalid-mode"),
        )))
        .unwrap_err();
        assert!(!error.to_string().contains("secret-invalid-mode"));
    }

    #[test]
    fn commands_are_explicit_and_unknown_arguments_are_not_echoed() {
        for (args, expected) in [
            (vec![], Command::Serve),
            (vec!["--migrate"], Command::Migrate),
            (vec!["--verify-migrations"], Command::Verify),
            (vec!["--help"], Command::Help),
            (vec!["-h"], Command::Help),
        ] {
            assert_eq!(
                parse_command(args.into_iter().map(OsString::from)).unwrap(),
                expected
            );
        }
        let error = parse_command([OsString::from("secret-database-url")]).unwrap_err();
        assert!(!error.to_string().contains("secret-database-url"));
        assert!(parse_command(["--migrate".into(), "--help".into()]).is_err());
    }

    #[tokio::test]
    async fn serve_and_help_do_not_require_database_configuration_or_files() {
        let absent = Path::new("/nonexistent-port-rust-migration-command-fixture");
        assert!(!handle_command([], "PORT_RUST_TEST_UNSET_DATABASE", absent)
            .await
            .unwrap());
        assert!(
            handle_command(["--help".into()], "PORT_RUST_TEST_UNSET_DATABASE", absent)
                .await
                .unwrap()
        );
    }
}
