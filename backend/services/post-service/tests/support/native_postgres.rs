//! Controller-owned native database provisioner for SQL-only receipt tests.
use anyhow::{ensure, Result};
use sqlx::{postgres::PgConnectOptions, PgPool};
use std::net::IpAddr;
use uuid::Uuid;

pub async fn database_url() -> Result<String> {
    let parent = std::env::var("POST_TEST_DATABASE_URL")?;
    let options: PgConnectOptions = parent.parse()?;
    let host = options
        .get_host()
        .trim_start_matches('[')
        .trim_end_matches(']');
    ensure!(
        host == "localhost" || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()),
        "Post owner native fixture requires loopback"
    );
    ensure!(
        options.get_socket().is_none()
            && options.get_options().is_none()
            && !parent.contains('?')
            && !parent.contains('#')
            && options
                .get_database()
                .is_some_and(|db| db.starts_with("port_rust_post_test_")),
        "owned Post parent required"
    );
    let admin = PgPool::connect(&parent).await?;
    let lineage: String =
        sqlx::query_scalar("select system_identifier::text from pg_control_system()")
            .fetch_one(&admin)
            .await?;
    ensure!(
        std::env::var("POST_TEST_INITDB_SYSTEM_IDENTIFIER").as_deref() == Ok(lineage.as_str()),
        "observed Post owner initdb lineage differs"
    );
    let database = format!("port_rust_post_owner_fixture_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("create database {database} template template0"))
        .execute(&admin)
        .await?;
    admin.close().await;
    let url = format!(
        "{}/{}",
        parent
            .rsplit_once('/')
            .ok_or_else(|| anyhow::anyhow!("parent fixture URL"))?
            .0,
        database
    );
    let observer = PgPool::connect(&url).await?;
    let empty: bool =
        sqlx::query_scalar("select not exists(select 1 from pg_tables where schemaname='public')")
            .fetch_one(&observer)
            .await?;
    ensure!(empty, "owner fixture must be empty before migrations");
    observer.close().await;
    println!(
        "POST_OWNER_PG_FIXTURE {}",
        serde_json::json!({"database":database,"systemIdentifier":lineage,"template":"template0","emptyBeforeMigrations":true})
    );
    Ok(url)
}
