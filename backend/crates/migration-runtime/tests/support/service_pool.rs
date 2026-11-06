use anyhow::{ensure, Context, Result};
use migration_runtime::MigrationMode;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::future::Future;

pub async fn verify<F, Fut>(connect: F, table: &str, column: &str, trigger: &str) -> Result<()>
where
    F: Fn(String, MigrationMode) -> Fut,
    Fut: Future<Output = Result<PgPool>>,
{
    let (container, url) = test_support::postgres::start_postgres_with_url_or_native()
        .await
        .context("external schema fixture acquisition")?;
    let container_id = container
        .as_ref()
        .map(|value| value.id().to_owned())
        .unwrap_or_else(|| "native".to_owned());
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default();
    let endpoint = url
        .parse::<sqlx::postgres::PgConnectOptions>()
        .map(|options| format!("{}:{}", options.get_host(), options.get_port()))
        .unwrap_or_else(|_| "[unavailable]".into());
    // Never print the URL/password. This identity allows the failed test's
    // bounded observations to be matched to retained Docker lifecycle events.
    use std::io::Write;
    let _ = writeln!(std::io::stderr().lock(),
        "EXTERNAL_SCHEMA_FIXTURE test={:?} table={table} timestamp_ms={timestamp} container_id={} endpoint={endpoint}",
        std::thread::current().name(), container_id);
    let mut phase = "initial owner connection";
    let outcome: Result<()> = async {
        let mut owner = PgConnection::connect(&url).await?;
        // External startup must reject an unmigrated database even for its owner.
        phase = "empty-schema rejection";
        let rejected = connect(url.clone(), MigrationMode::External).await;
        require_pool_rejection(rejected, ExpectedRejection::SqlState("42P01"), phase).await?;
        phase = "empty-schema receipts remain absent";
        let absent: bool = sqlx::query_scalar("select to_regclass('public._sqlx_migrations') is null")
            .fetch_one(&mut owner)
            .await?;
        ensure!(absent, "external startup created migration receipts");
        phase = "startup migration";
        let startup = connect(url.clone(), MigrationMode::Startup).await?;
        startup.close().await;
        phase = "reader role setup";
        let reader_role = format!(
            "migration_reader_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.as_nanos())
                .unwrap_or_default()
        );
        owner.execute(format!("create role {reader_role} login password 'fixture-password' nosuperuser nocreatedb nocreaterole; revoke create on schema public from public; grant usage on schema public to {reader_role}; grant select on public._sqlx_migrations to {reader_role}; create schema shadow; grant usage on schema shadow to {reader_role}").as_str()).await?;
        let (scheme, authority_and_path) = url
            .split_once("://")
            .ok_or_else(|| anyhow::anyhow!("fixture URL scheme"))?;
        let (_, authority_and_path) = authority_and_path
            .split_once('@')
            .ok_or_else(|| anyhow::anyhow!("fixture URL credentials"))?;
        let (host_port, path) = authority_and_path
            .split_once('/')
            .ok_or_else(|| anyhow::anyhow!("fixture URL database"))?;
        let reader_url = format!(
            "{scheme}://{reader_role}:fixture-password@{host_port}/{path}"
        );
        ensure!(reader_url != url, "fixture credentials were not replaced");
        // An actual URL option must not divert external mode away from public.
        let redirected = format!("{reader_url}?options=-csearch_path%3Dshadow");
        phase = "reader external verification";
        let reader = connect(redirected.clone(), MigrationMode::External).await?;
        phase = "reader public schema selection";
        let schema: String = sqlx::query_scalar("select current_schema()")
            .fetch_one(&reader)
            .await?;
        ensure!(schema == "public");
        for (label, statement) in [
            ("business-table SELECT denial", format!("select * from public.{table} limit 0")),
            ("runtime DDL denial", "create table public.forbidden(id int)".into()),
            ("runtime ledger UPDATE denial", "update public._sqlx_migrations set success=false".into()),
        ] {
            phase = label;
            let rejected = reader.execute(statement.as_str()).await.map_err(anyhow::Error::from);
            require_rejection(rejected.map(|_| ()), ExpectedRejection::SqlState("42501"), phase)?;
        }
        reader.close().await;
        for (label, damage, restore, expected) in [
            ("renamed required column", format!("alter table public.{table} rename column {column} to missing_required_column"), format!("alter table public.{table} rename column missing_required_column to {column}"), ExpectedRejection::SchemaProbe),
            ("disabled required trigger", format!("alter table public.{table} disable trigger {trigger}"), format!("alter table public.{table} enable trigger {trigger}"), ExpectedRejection::SchemaProbe),
            ("renamed required table", format!("alter table public.{table} rename to missing_required_table"), format!("alter table public.missing_required_table rename to {table}"), ExpectedRejection::SchemaProbe),
            ("unsuccessful migration receipt", "update public._sqlx_migrations set success=false where version=(select min(version) from public._sqlx_migrations)".into(), "update public._sqlx_migrations set success=true".into(), ExpectedRejection::UnsuccessfulReceipt),
        ] {
            phase = "apply controlled schema damage";
            owner.execute(damage.as_str()).await.with_context(|| format!("damage case: {label}"))?;
            phase = label;
            let result = connect(redirected.clone(), MigrationMode::External).await;
            // Close an unexpectedly successful pool before restoring or failing.
            // Keep the rejection result while restoring even for unexpected errors.
            let rejection = require_pool_rejection(result, expected, phase).await;
            phase = "restore controlled schema damage";
            let restoration = owner.execute(restore.as_str()).await.map(|_| ()).map_err(anyhow::Error::from);
            phase = label;
            finish_damage_case(rejection, restoration, label)?;
        }
        phase = "final healthy external verification";
        connect(redirected, MigrationMode::External)
            .await?
            .close()
            .await;
        phase = "close owner connection";
        owner.close().await?;
        Ok(())
    }.await;
    if outcome.is_err() {
        if let Some(container) = container.as_ref() {
            let observed = test_support::postgres::startup_failure_diagnostics(container).await;
            let _ = writeln!(
                std::io::stderr().lock(),
                "EXTERNAL_SCHEMA_FIXTURE_FAILURE table={table} phase={phase:?} {observed:?}"
            );
        }
    }
    outcome.with_context(|| format!("external schema pool acceptance: {phase}"))
}

#[derive(Clone, Copy, Debug)]
enum ExpectedRejection {
    SqlState(&'static str),
    SchemaProbe,
    UnsuccessfulReceipt,
}

impl ExpectedRejection {
    fn matches(self, error: &anyhow::Error) -> bool {
        // SQLx failures take precedence over surrounding context, which must not
        // disguise a disconnected database or exhausted pool as schema rejection.
        let sqlx = error
            .chain()
            .find_map(|cause| cause.downcast_ref::<sqlx::Error>());
        match (self, sqlx) {
            (Self::SqlState(expected), Some(sqlx::Error::Database(database))) => {
                database.code().as_deref() == Some(expected)
            }
            (_, Some(_)) | (Self::SqlState(_), None) => false,
            (Self::SchemaProbe, None) => error
                .root_cause()
                .to_string()
                .starts_with("required schema probe failed: "),
            (Self::UnsuccessfulReceipt, None) => {
                error.root_cause().to_string()
                    == "migration receipt records an unsuccessful migration"
            }
        }
    }
}

fn require_rejection(result: Result<()>, expected: ExpectedRejection, phase: &str) -> Result<()> {
    record_rejection(phase, result.as_ref().err());
    match result {
        Ok(()) => anyhow::bail!("unexpected success during {phase}; expected {expected:?}"),
        Err(error) if expected.matches(&error) => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("unexpected rejection during {phase}; expected {expected:?}")),
    }
}

async fn require_pool_rejection(
    result: Result<PgPool>,
    expected: ExpectedRejection,
    phase: &str,
) -> Result<()> {
    let result = match result {
        Ok(pool) => {
            pool.close().await;
            Ok(())
        }
        Err(error) => Err(error),
    };
    require_rejection(result, expected, phase)
}

// Keep both typed causes when PostgreSQL disappears during verification and
// restoration. The unexpected verification error remains the primary source.
#[derive(Debug)]
struct RestorationFailure {
    phase: String,
    error: anyhow::Error,
}

impl std::fmt::Display for RestorationFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "restore case {} also failed ({})",
            self.phase,
            rejection_kind(Some(&self.error))
        )
    }
}

fn finish_damage_case(rejection: Result<()>, restoration: Result<()>, phase: &str) -> Result<()> {
    match (rejection, restoration) {
        (Err(error), Err(restoration)) => Err(error).context(RestorationFailure {
            phase: phase.to_owned(),
            error: restoration,
        }),
        (Ok(()), Err(error)) => Err(error).with_context(|| format!("restore case: {phase}")),
        (result, Ok(())) => result,
    }
}

fn rejection_kind(error: Option<&anyhow::Error>) -> String {
    match error {
        None => "unexpected success".to_owned(),
        Some(error) => match error.downcast_ref::<sqlx::Error>() {
            Some(sqlx::Error::Database(database)) => {
                format!("postgres SQLSTATE {}", database.code().unwrap_or_default())
            }
            Some(sqlx::Error::Io(io)) => format!("transport {:?}", io.kind()),
            Some(sqlx::Error::PoolTimedOut) => "pool timeout".into(),
            Some(_) => "other SQLx error".into(),
            None if error.chain().any(|cause| {
                cause
                    .to_string()
                    .starts_with("required schema probe failed: ")
            }) =>
            {
                "required schema probe rejection".into()
            }
            None if error.chain().any(|cause| {
                cause.to_string() == "migration receipt records an unsuccessful migration"
            }) =>
            {
                "unsuccessful migration receipt".into()
            }
            None => "other non-SQLx rejection".into(),
        },
    }
}

fn record_rejection(phase: &str, error: Option<&anyhow::Error>) {
    let kind = rejection_kind(error);
    use std::io::Write;
    let _ = writeln!(
        std::io::stderr().lock(),
        "EXTERNAL_SCHEMA_EXPECTED_REJECTION phase={phase:?} category={kind}"
    );
}

#[cfg(test)]
mod rejection_oracle_tests {
    use super::*;

    #[test]
    fn transport_and_pool_failures_never_satisfy_negative_acceptance() {
        for expected in [
            ExpectedRejection::SqlState("42P01"),
            ExpectedRejection::SqlState("42501"),
            ExpectedRejection::SchemaProbe,
            ExpectedRejection::UnsuccessfulReceipt,
        ] {
            for cause in [
                sqlx::Error::Io(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
                sqlx::Error::PoolTimedOut,
                sqlx::Error::PoolClosed,
            ] {
                let error = anyhow::Error::from(cause)
                    .context("required schema probe failed: misleading context")
                    .context("migration receipt records an unsuccessful migration")
                    .context("Gateway startup: prepare main database schema");
                let preserved =
                    require_rejection(Err(error), expected, "negative control").unwrap_err();
                assert!(
                    preserved.downcast_ref::<sqlx::Error>().is_some(),
                    "typed failure must survive"
                );
            }
        }
    }

    #[test]
    fn schema_markers_must_be_root_causes_and_match_the_expected_case() {
        for (message, expected, wrong) in [
            (
                "required schema probe failed: required column",
                ExpectedRejection::SchemaProbe,
                ExpectedRejection::UnsuccessfulReceipt,
            ),
            (
                "migration receipt records an unsuccessful migration",
                ExpectedRejection::UnsuccessfulReceipt,
                ExpectedRejection::SchemaProbe,
            ),
        ] {
            let wrapped = || {
                anyhow::anyhow!(message).context("Gateway startup: prepare main database schema")
            };
            assert!(require_rejection(Err(wrapped()), expected, "expected rejection").is_ok());
            assert!(require_rejection(Err(wrapped()), wrong, "wrong rejection").is_err());
            let unrelated = anyhow::anyhow!("unrelated startup failure").context(message);
            assert!(require_rejection(Err(unrelated), expected, "misleading context").is_err());
        }
    }

    #[test]
    fn simultaneous_verification_and_restoration_failures_preserve_both_causes() {
        let refusal = anyhow::Error::from(sqlx::Error::Io(std::io::Error::from(
            std::io::ErrorKind::ConnectionRefused,
        )));
        let rejection = require_rejection(
            Err(refusal),
            ExpectedRejection::SchemaProbe,
            "renamed required table",
        );
        let restored = Err(anyhow::Error::from(sqlx::Error::PoolClosed));
        let combined =
            finish_damage_case(rejection, restored, "renamed required table").unwrap_err();
        assert!(
            matches!(combined.downcast_ref::<sqlx::Error>(), Some(sqlx::Error::Io(io)) if io.kind() == std::io::ErrorKind::ConnectionRefused)
        );
        let secondary = combined
            .downcast_ref::<RestorationFailure>()
            .expect("restoration cause must be retained");
        assert_eq!(secondary.phase, "renamed required table");
        assert!(matches!(
            secondary.error.downcast_ref::<sqlx::Error>(),
            Some(sqlx::Error::PoolClosed)
        ));
    }

    #[test]
    fn restoration_outcome_never_hides_a_single_failure() {
        let restoration = Err(anyhow::Error::from(sqlx::Error::PoolTimedOut));
        let failed_restore =
            finish_damage_case(Ok(()), restoration, "required column").unwrap_err();
        assert!(matches!(
            failed_restore.downcast_ref::<sqlx::Error>(),
            Some(sqlx::Error::PoolTimedOut)
        ));
        let rejection = Err(anyhow::Error::from(sqlx::Error::PoolClosed));
        let failed_verification =
            finish_damage_case(rejection, Ok(()), "required column").unwrap_err();
        assert!(matches!(
            failed_verification.downcast_ref::<sqlx::Error>(),
            Some(sqlx::Error::PoolClosed)
        ));
        assert!(finish_damage_case(Ok(()), Ok(()), "required column").is_ok());
    }

    #[test]
    fn successful_operations_never_satisfy_negative_acceptance() {
        for expected in [
            ExpectedRejection::SqlState("42P01"),
            ExpectedRejection::SqlState("42501"),
            ExpectedRejection::SchemaProbe,
            ExpectedRejection::UnsuccessfulReceipt,
        ] {
            assert!(require_rejection(Ok(()), expected, "unexpected success").is_err());
        }
    }
}
