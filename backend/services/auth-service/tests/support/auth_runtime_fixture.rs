//! Fresh PG16 databases tied to the owned native controller, or owned Docker PG16.
use anyhow::{Context, Result};
use sqlx::{postgres::PgPoolOptions, PgPool};
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

pub(crate) struct Fixture {
    pub(crate) pool: PgPool,
    pub(crate) database: String,
    _postgres: Option<ContainerAsync<Postgres>>,
}
impl Fixture {
    pub(crate) async fn new() -> Result<Self> {
        let native_url = std::env::var("AUTH_RUNTIME_TEST_DATABASE_URL").ok();
        let (postgres, url) = match native_url {
            Some(url) => (None, url),
            None => {
                let (postgres, url) = test_support::postgres::start_postgres_with_url().await?;
                (Some(postgres), url)
            }
        };
        let controller = PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await?;
        let lineage: String =
            sqlx::query_scalar("select system_identifier::text from pg_control_system()")
                .fetch_one(&controller)
                .await?;
        if postgres.is_none() {
            anyhow::ensure!(
                std::env::var("AUTH_RUNTIME_TEST_INITDB_SYSTEM_IDENTIFIER").as_deref()
                    == Ok(lineage.as_str()),
                "runner-owned initdb required"
            );
        }
        let version: i32 = sqlx::query_scalar("select current_setting('server_version_num')::int")
            .fetch_one(&controller)
            .await?;
        anyhow::ensure!(version / 10000 == 16, "PG16 required");
        let database = format!("port_rust_auth_runtime_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("create database {database} template template0"))
            .execute(&controller)
            .await?;
        controller.close().await;
        let base = url.rsplit_once('/').context("controller URL database")?.0;
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .connect(&format!("{base}/{database}"))
            .await?;
        let count: i64 = sqlx::query_scalar(
            "select count(*) from information_schema.tables where table_schema='public'",
        )
        .fetch_one(&pool)
        .await?;
        anyhow::ensure!(count == 0, "fixture not empty before migrations");
        eprintln!(
            "AUTH_RUNTIME_FIXTURE {}",
            serde_json::json!({"database":database,"systemIdentifier":lineage,"template":"template0","emptyBeforeMigrations":true})
        );
        sqlx::migrate::Migrator::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("migrations")
                .as_path(),
        )
        .await?
        .run(&pool)
        .await?;
        Ok(Self {
            pool,
            database,
            _postgres: postgres,
        })
    }
    pub(crate) async fn empty_effects(&self) -> Result<()> {
        for table in [
            "users",
            "user_identities",
            "refresh_tokens",
            "auth_audit_events",
            "login_audit",
        ] {
            let count: i64 = sqlx::query_scalar(&format!("select count(*) from {table}"))
                .fetch_one(&self.pool)
                .await?;
            anyhow::ensure!(count == 0, "unexpected effect in {table}");
        }
        Ok(())
    }
}
