//! Fresh test databases are authorized only by an exact runner-owned PG16 identity.
use anyhow::Result;
use uuid::Uuid;

pub async fn native_database_url() -> Result<Option<String>> {
    match std::env::var("AUTH_RUNTIME_TEST_DATABASE_URL") {
        Ok(database_url) => {
            anyhow::ensure!(
                !database_url.trim().is_empty(),
                "native Auth fixture URL is empty"
            );
            let controller = sqlx::PgPool::connect(&database_url).await?;
            let lineage: String =
                sqlx::query_scalar("select system_identifier::text from pg_control_system()")
                    .fetch_one(&controller)
                    .await?;
            anyhow::ensure!(
                std::env::var("AUTH_RUNTIME_TEST_INITDB_SYSTEM_IDENTIFIER").as_deref()
                    == Ok(&lineage),
                "native Auth fixture must use the runner-owned initdb cluster"
            );
            let version: i32 =
                sqlx::query_scalar("select current_setting('server_version_num')::int")
                    .fetch_one(&controller)
                    .await?;
            anyhow::ensure!(version / 10000 == 16, "native Auth fixture requires PG16");
            let database = format!("port_rust_auth_http_fixture_{}", Uuid::new_v4().simple());
            sqlx::query(&format!("create database {database} template template0"))
                .execute(&controller)
                .await?;
            controller.close().await;
            let base = database_url
                .rsplit_once('/')
                .ok_or_else(|| anyhow::anyhow!("native Auth fixture URL"))?
                .0;
            let isolated_url = format!("{base}/{database}");
            let observer = sqlx::PgPool::connect(&isolated_url).await?;
            let empty: bool = sqlx::query_scalar(
                "select not exists(select 1 from pg_tables where schemaname='public')",
            )
            .fetch_one(&observer)
            .await?;
            observer.close().await;
            anyhow::ensure!(empty, "native Auth fixture is not empty before migrations");
            eprintln!(
                "AUTH_HTTP_FIXTURE {}",
                serde_json::json!({"database":database,"template":"template0","systemIdentifier":lineage,"emptyBeforeMigrations":empty})
            );
            Ok(Some(isolated_url))
        }
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error.into()),
    }
}
