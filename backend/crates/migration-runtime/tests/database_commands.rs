//! Standalone binary acceptance; run with the six release executables available.
use anyhow::{ensure, Context, Result};
use sqlx::{migrate::Migrate, Connection, Executor, PgConnection};
use std::{
    path::{Path, PathBuf},
    process::Output,
    time::Duration,
};
use tokio::process::Command;
#[path = "database_commands/external_startup.rs"]
mod external_startup;

async fn command(binary: &Path, arg: &str, variable: &str, url: Option<&str>) -> Result<Output> {
    let mut child = Command::new(binary);
    child.arg(arg).env_clear().kill_on_drop(true);
    if let Some(url) = url {
        child.env(variable, url);
    }
    tokio::time::timeout(Duration::from_secs(60), child.output())
        .await
        .context("standalone database command exceeded its test budget")?
        .context("could not execute standalone database command")
}

fn output_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[tokio::test]
#[ignore = "requires the six service executables; use run-database-commands.sh"]
async fn six_service_commands_need_only_database_configuration_and_use_public_schema() -> Result<()>
{
    let binaries = PathBuf::from(
        std::env::var_os("PORT_RUST_BINARY_DIR").context("PORT_RUST_BINARY_DIR is required")?,
    );
    let (_container, url) = test_support::postgres::start_postgres_with_url().await?;
    let mut admin = PgConnection::connect(&url).await?;
    admin.execute("create role migration_reader login password 'fixture-password' nosuperuser nocreatedb nocreaterole").await?;
    let base = url
        .rsplit_once('/')
        .context("fixture URL has no database")?
        .0;
    for (service, variable) in [
        ("auth-service", "AUTH_DATABASE_URL"),
        ("profile-service", "PROFILE_DATABASE_URL"),
        ("friend-service", "FRIEND_DATABASE_URL"),
        ("post-service", "POST_DATABASE_URL"),
        ("notification-service", "NOTIFICATION_DATABASE_URL"),
        ("api-gateway", "GATEWAY_DATABASE_URL"),
    ] {
        let binary = binaries.join(service);
        let prefix = variable
            .strip_suffix("_DATABASE_URL")
            .context("database variable prefix")?;

        ensure!(binary.is_file(), "missing service executable: {service}");
        let help = command(&binary, "--help", variable, None).await?;
        ensure!(
            help.status.success(),
            "{service} help needed application configuration"
        );
        let marker = "PRIVATE_OPTION_MARKER";
        let invalid = command(&binary, marker, variable, None).await?;
        ensure!(
            !invalid.status.success() && !output_text(&invalid).contains(marker),
            "{service} echoed an unknown argument"
        );
        let missing = command(&binary, "--migrate", variable, None).await?;
        ensure!(
            !missing.status.success(),
            "{service} accepted an implicit database URL"
        );
        let malformed =
            format!("postgres://postgres:PRIVATE_PASSWORD_MARKER@127.0.0.1/db?sslmode={marker}");
        let rejected = command(&binary, "--migrate", variable, Some(&malformed)).await?;
        let stderr = output_text(&rejected);
        ensure!(
            !rejected.status.success()
                && !stderr.contains(marker)
                && !stderr.contains("PRIVATE_PASSWORD_MARKER"),
            "{service} exposed malformed connection options"
        );

        let database = format!("cli_{}", service.replace('-', "_"));
        admin
            .execute(format!("create database {database}").as_str())
            .await?;
        let service_url = format!("{base}/{database}");
        let bad_password =
            service_url.replace("postgres:postgres@", "postgres:PRIVATE_PASSWORD_MARKER@");
        let bad_connection = command(&binary, "--migrate", variable, Some(&bad_password)).await?;
        ensure!(
            !bad_connection.status.success()
                && output_text(&bad_connection).contains("database command: connection failed")
                && !output_text(&bad_connection).contains("PRIVATE_PASSWORD_MARKER"),
            "{service} connection failure exposed credentials"
        );
        let reader_url_without_options =
            service_url.replace("postgres:postgres@", "migration_reader:fixture-password@");
        let denied_migration = command(
            &binary,
            "--migrate",
            variable,
            Some(&reader_url_without_options),
        )
        .await?;
        ensure!(
            !denied_migration.status.success()
                && output_text(&denied_migration).contains("database command: migration failed")
                && !output_text(&denied_migration).contains("fixture-password"),
            "{service} migration failure output was not sanitized"
        );
        let mut owner = PgConnection::connect(&service_url).await?;
        owner.execute("create schema shadow").await?;
        let redirected = format!("{service_url}?options=-csearch_path%3Dshadow");
        for mode in ["", "PRIVATE_MODE_MARKER"] {
            external_startup::rejected(
                &binary,
                prefix,
                &redirected,
                mode.as_ref(),
                "database migration mode must be startup or external",
            )
            .await?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            external_startup::rejected(
                &binary,
                prefix,
                &redirected,
                std::ffi::OsStr::from_bytes(b"PRIVATE_MODE_MARKER\xff"),
                "database migration mode must be startup or external",
            )
            .await?;
        }
        external_startup::rejected(
            &binary,
            prefix,
            &redirected,
            "external".as_ref(),
            "read existing migration receipts",
        )
        .await?;

        let empty = command(&binary, "--verify-migrations", variable, Some(&redirected)).await?;
        ensure!(
            !empty.status.success(),
            "{service} accepted an empty migration history"
        );
        let absent: bool = sqlx::query_scalar("select to_regclass('public._sqlx_migrations') is null and to_regclass('shadow._sqlx_migrations') is null").fetch_one(&mut owner).await?;
        ensure!(absent, "{service} verification created a ledger");
        // Hold SQLx's actual database migration lock, then observe BOTH command
        // backends waiting before release. Merely launching two futures would
        // not prove contention or lock use.
        owner.lock().await?;
        let release_barrier = async {
            let observed = tokio::time::timeout(Duration::from_secs(10), async {
                loop {
                    let waiting: i64 = sqlx::query_scalar("select count(*) from pg_locks where locktype='advisory' and not granted and pid in (select pid from pg_stat_activity where datname=current_database())")
                        .fetch_one(&mut owner).await?;
                    if waiting == 2 { return Ok::<_, anyhow::Error>(()); }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }).await;
            let released = owner.unlock().await;
            observed.context("both migration commands did not wait for the database lock")??;
            released?;
            Ok::<_, anyhow::Error>(())
        };
        let (first, second, barrier) = tokio::join!(
            command(&binary, "--migrate", variable, Some(&redirected)),
            command(&binary, "--migrate", variable, Some(&redirected)),
            release_barrier,
        );
        barrier?;
        ensure!(
            first?.status.success() && second?.status.success(),
            "{service} concurrent migrations failed"
        );
        let public_only: bool = sqlx::query_scalar("select to_regclass('public._sqlx_migrations') is not null and to_regclass('shadow._sqlx_migrations') is null").fetch_one(&mut owner).await?;
        ensure!(
            public_only,
            "{service} migrated a URL-selected shadow schema"
        );
        owner.execute("revoke create on schema public from public; grant usage on schema public to migration_reader; grant select on public._sqlx_migrations to migration_reader").await?;
        let reader_url =
            redirected.replace("postgres:postgres@", "migration_reader:fixture-password@");
        ensure!(
            reader_url != redirected,
            "fixture reader credentials were not selected"
        );
        owner.execute("create table shadow._sqlx_migrations as table public._sqlx_migrations; grant usage on schema shadow to migration_reader; grant select on shadow._sqlx_migrations to migration_reader; update shadow._sqlx_migrations set success=false").await?;
        let verified = command(&binary, "--verify-migrations", variable, Some(&reader_url)).await?;
        ensure!(
            verified.status.success(),
            "{service} verification required write/owner privileges"
        );
        owner
            .execute("update shadow._sqlx_migrations set success=true")
            .await?;
        owner.execute("update public._sqlx_migrations set success=false where version=(select min(version) from public._sqlx_migrations)").await?;
        let dirty = command(&binary, "--verify-migrations", variable, Some(&reader_url)).await?;
        ensure!(
            !dirty.status.success(),
            "{service} accepted a failed migration receipt"
        );
        external_startup::rejected(
            &binary,
            prefix,
            &reader_url,
            "external".as_ref(),
            "migration receipt records an unsuccessful migration",
        )
        .await?;
        owner
            .execute("update public._sqlx_migrations set success=true")
            .await?;
        let required_table = match service {
            "auth-service" => "users",
            "profile-service" => "profiles",
            "friend-service" => "follows",
            "post-service" => "posts",
            "notification-service" => "notifications",
            "api-gateway" => "gateway_request_journal",
            _ => unreachable!(),
        };
        owner
            .execute(
                format!("alter table public.{required_table} rename to missing_required_table")
                    .as_str(),
            )
            .await?;
        let rejected = external_startup::rejected(
            &binary,
            prefix,
            &reader_url,
            "external".as_ref(),
            "required schema probe failed",
        )
        .await;
        owner
            .execute(
                format!("alter table public.missing_required_table rename to {required_table}")
                    .as_str(),
            )
            .await?;
        rejected?;
        eprintln!("EXTERNAL_STARTUP_REJECTION_PASS={service}");
        owner.close().await?;
    }
    admin.close().await?;
    Ok(())
}
