//! Real PostgreSQL receipt verification, including a read-only non-owner role.
use anyhow::Result;
use migration_runtime::{
    load_migrations, prepare_database, serving_options, verify_receipts_with_timeout,
    verify_schema, MigrationMode, SchemaProbe,
};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    Connection, Executor, PgConnection,
};
use std::{path::PathBuf, str::FromStr};

#[tokio::test]
async fn all_six_histories_verify_without_owner_or_write_privileges() -> Result<()> {
    let (_container, url) = test_support::postgres::start_postgres_with_url().await?;
    let mut admin = PgConnection::connect(&url).await?;
    admin.execute("create role migration_reader login password 'fixture-password' nosuperuser nocreatedb nocreaterole").await?;
    for (service, probe_sql) in [
        (
            "auth-service",
            include_str!("../../../services/auth-service/src/config/schema-probe.sql"),
        ),
        (
            "profile-service",
            include_str!("../../../services/profile-service/src/config/schema-probe.sql"),
        ),
        (
            "friend-service",
            include_str!("../../../services/friend-service/src/config/schema-probe.sql"),
        ),
        (
            "post-service",
            include_str!("../../../services/post-service/src/config/schema-probe.sql"),
        ),
        (
            "notification-service",
            include_str!("../../../services/notification-service/src/config/schema-probe.sql"),
        ),
        (
            "api-gateway",
            include_str!("../../../services/api-gateway/src/config/schema-probe.sql"),
        ),
    ] {
        let probes = [SchemaProbe {
            name: "service schema",
            sql: probe_sql,
        }];
        let database = service.replace('-', "_");
        admin
            .execute(format!("create database {database}").as_str())
            .await?;
        let options = PgConnectOptions::from_str(&url)?.database(&database);
        let owner = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await?;
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../services")
            .join(service)
            .join("migrations");
        let migrator = load_migrations(&directory).await?;
        // Missing receipts must fail without creating an empty ledger.
        assert!(verify_receipts_with_timeout(&owner, &migrator)
            .await
            .is_err());
        let absent: bool =
            sqlx::query_scalar("select to_regclass('public._sqlx_migrations') is null")
                .fetch_one(&owner)
                .await?;
        assert!(absent);
        assert!(
            prepare_database(&owner, &directory, MigrationMode::External, &probes)
                .await
                .is_err()
        );
        migrator.run(&owner).await?;
        assert!(verify_schema(&owner, &migrator, &[]).await.is_err());
        // A privileged owner proves the transaction is read-only rather than
        // relying on a reader's lack of UPDATE permission.
        let write = verify_schema(&owner, &migrator, &[SchemaProbe {
            name: "forbidden write", sql: "with changed as (update public._sqlx_migrations set success=false returning version) select count(*) >= 0 from changed",
        }]).await.expect_err("read-only verifier must prohibit owner writes");
        let code = write
            .chain()
            .find_map(|cause| cause.downcast_ref::<sqlx::Error>())
            .and_then(|error| error.as_database_error())
            .and_then(|error| error.code());
        assert_eq!(code.as_deref(), Some("25006"));
        verify_receipts_with_timeout(&owner, &migrator).await?;
        verify_schema(&owner, &migrator, &[SchemaProbe {
            name: "transaction bounds",
            sql: "select current_setting('transaction_read_only') = 'on' and current_setting('transaction_isolation') = 'repeatable read' and current_setting('statement_timeout') = '5s' and current_setting('lock_timeout') = '2s'",
        }]).await?;
        owner.execute("create schema shadow").await?;
        for (mode, expected) in [
            (MigrationMode::Startup, "shadow"),
            (MigrationMode::External, "public"),
        ] {
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect_with(serving_options(
                    options.clone().options([("search_path", "shadow")]),
                    mode,
                ))
                .await?;
            let actual: String = sqlx::query_scalar("select current_schema()")
                .fetch_one(&pool)
                .await?;
            assert_eq!(actual, expected);
            pool.close().await;
        }
        owner.execute("revoke create on schema public from public; grant usage on schema public to migration_reader; grant select on public._sqlx_migrations to migration_reader").await?;
        let reader = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(
                options
                    .username("migration_reader")
                    .password("fixture-password"),
            )
            .await?;
        verify_receipts_with_timeout(&reader, &migrator).await?;
        prepare_database(&reader, &directory, MigrationMode::External, &probes).await?;
        assert!(reader
            .execute("create table public.verification_must_not_write(id int)")
            .await
            .is_err());
        assert!(reader
            .execute("update public._sqlx_migrations set success=false")
            .await
            .is_err());
        if service == "auth-service" {
            // Exercise actual cancellation and pooled-connection recovery, not
            // just the transaction settings returned by SHOW/current_setting.
            let mut blocker = owner.begin().await?;
            sqlx::query("lock table public._sqlx_migrations in access exclusive mode")
                .execute(&mut *blocker)
                .await?;
            let blocked = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                verify_schema(&reader, &migrator, &probes),
            )
            .await?;
            blocker.rollback().await?;
            let error = blocked.expect_err("ledger lock must be bounded");
            assert_sqlstate(&error, "55P03");
            verify_schema(&reader, &migrator, &probes).await?;
            let delayed = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                verify_schema(
                    &reader,
                    &migrator,
                    &[SchemaProbe {
                        name: "statement timeout",
                        sql: "select true from pg_catalog.pg_sleep(6)",
                    }],
                ),
            )
            .await?
            .expect_err("probe statement must be bounded");
            assert_sqlstate(&delayed, "57014");
            let reset: bool = sqlx::query_scalar("select current_setting('transaction_read_only') = 'off' and current_setting('statement_timeout') = '0' and current_setting('lock_timeout') = '0'").fetch_one(&reader).await?;
            assert!(
                reset,
                "cancelled verification leaked transaction settings into the pool"
            );
            verify_schema(&reader, &migrator, &probes).await?;
        }
        if service == "notification-service" {
            // Receipts alone still agree while the required unique index is
            // missing, nonunique, partial, or has the wrong ordered keys.
            owner.execute("alter index public.notifications_user_event_type_idx rename to saved_notification_index").await?;
            verify_receipts_with_timeout(&reader, &migrator).await?;
            assert!(verify_schema(&reader, &migrator, &probes).await.is_err());
            for definition in [
                "create index notifications_user_event_type_idx on public.notifications(user_id,event_id,type)",
                "create unique index notifications_user_event_type_idx on public.notifications(user_id,event_id,type) where event_id is not null",
                "create unique index notifications_user_event_type_idx on public.notifications(event_id,user_id,type)",
            ] {
                owner.execute(definition).await?;
                let rejected = verify_schema(&reader, &migrator, &probes).await.is_err();
                owner.execute("drop index public.notifications_user_event_type_idx").await?;
                assert!(rejected, "accepted damaged unique index: {definition}");
            }
            owner.execute("alter index public.saved_notification_index rename to notifications_user_event_type_idx").await?;
            verify_schema(&reader, &migrator, &probes).await?;
        }
        let original: (i64, Vec<u8>) = sqlx::query_as(
            "select version,checksum from public._sqlx_migrations order by version limit 1",
        )
        .fetch_one(&owner)
        .await?;
        sqlx::query("update public._sqlx_migrations set success=false where version=$1")
            .bind(original.0)
            .execute(&owner)
            .await?;
        assert!(verify_receipts_with_timeout(&reader, &migrator)
            .await
            .is_err());
        assert!(
            prepare_database(&reader, &directory, MigrationMode::External, &probes)
                .await
                .is_err()
        );
        sqlx::query("update public._sqlx_migrations set success=true,checksum='broken'::bytea where version=$1").bind(original.0).execute(&owner).await?;
        assert!(verify_receipts_with_timeout(&reader, &migrator)
            .await
            .is_err());
        assert!(
            prepare_database(&reader, &directory, MigrationMode::External, &probes)
                .await
                .is_err()
        );
        sqlx::query("update public._sqlx_migrations set checksum=$2 where version=$1")
            .bind(original.0)
            .bind(original.1)
            .execute(&owner)
            .await?;
        owner.execute("insert into public._sqlx_migrations(version,description,success,checksum,execution_time) values(999999,'unexpected',true,'unexpected'::bytea,0)").await?;
        assert!(verify_receipts_with_timeout(&reader, &migrator)
            .await
            .is_err());
        assert!(
            prepare_database(&reader, &directory, MigrationMode::External, &probes)
                .await
                .is_err()
        );
        owner
            .execute("delete from public._sqlx_migrations where version=999999")
            .await?;
        verify_receipts_with_timeout(&reader, &migrator).await?;
        prepare_database(&reader, &directory, MigrationMode::External, &probes).await?;
        sqlx::query("delete from public._sqlx_migrations where version=$1")
            .bind(original.0)
            .execute(&owner)
            .await?;
        assert!(verify_receipts_with_timeout(&reader, &migrator)
            .await
            .is_err());
        assert!(
            prepare_database(&reader, &directory, MigrationMode::External, &probes)
                .await
                .is_err()
        );
        reader.close().await;
        owner.close().await;
    }
    admin.close().await?;
    Ok(())
}

fn assert_sqlstate(error: &anyhow::Error, expected: &str) {
    let code = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<sqlx::Error>())
        .and_then(|error| error.as_database_error())
        .and_then(|error| error.code());
    assert_eq!(code.as_deref(), Some(expected));
}
