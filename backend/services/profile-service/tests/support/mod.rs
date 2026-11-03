#![allow(dead_code)]

use anyhow::Result;
use axum::body::Body;
use axum::http::{Method, Request};
use http_body_util::BodyExt;
use profile_service::adapters::r#in::grpc::server::ProfileGrpcServer;
use profile_service::adapters::r#in::http as http_routes;
use profile_service::config::{build_state, connect_pool, AppConfig, AppState};
use std::sync::Arc;
use std::sync::OnceLock;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use tower::ServiceExt;

pub struct TestApp {
    pub _postgres: Option<ContainerAsync<Postgres>>,
    pub pool: sqlx::PgPool,
    pub state: Arc<AppState>,
}

/// Callers accepted by the test server: the edge, the feed/search reader, the
/// friendship policy reader, and the account-deletion coordinator.
/// Minimal valid image containers for upload tests.
///
/// The avatar domain verifies that declared bytes match the content type, so
/// fixtures must carry a real signature rather than a placeholder string.
///
/// These are encoded by the same library that decodes them at upload time, so
/// they are real images rather than signature prefixes. A hand-written byte
/// array that merely starts with a PNG signature passes the container check but
/// is exactly the polyglot shape the decoder refuses, so it would test the wrong
/// thing.
fn encode_fixture(format: image::ImageFormat) -> Vec<u8> {
    // JPEG has no alpha channel, so the encoded colour type has to match what
    // each format can actually represent rather than being forced uniform.
    let source = image::RgbaImage::from_pixel(2, 2, image::Rgba([200, 100, 50, 255]));
    let dynamic = match format {
        image::ImageFormat::Jpeg => {
            image::DynamicImage::ImageRgb8(image::DynamicImage::ImageRgba8(source).to_rgb8())
        }
        _ => image::DynamicImage::ImageRgba8(source),
    };
    let mut bytes = std::io::Cursor::new(Vec::new());
    dynamic
        .write_to(&mut bytes, format)
        .unwrap_or_else(|error| panic!("encode a {format:?} fixture: {error}"));
    bytes.into_inner()
}

fn png_bytes() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| encode_fixture(image::ImageFormat::Png))
}

fn jpeg_bytes() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| encode_fixture(image::ImageFormat::Jpeg))
}

fn gif_bytes() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| encode_fixture(image::ImageFormat::Gif))
}

fn webp_bytes() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| encode_fixture(image::ImageFormat::WebP))
}

pub fn png_fixture() -> Vec<u8> {
    png_bytes().to_vec()
}

pub fn jpeg_fixture() -> Vec<u8> {
    jpeg_bytes().to_vec()
}

pub fn gif_fixture() -> Vec<u8> {
    gif_bytes().to_vec()
}

pub fn webp_fixture() -> Vec<u8> {
    webp_bytes().to_vec()
}

pub fn test_caller_registry() -> platform::grpc_auth::CallerRegistry {
    platform::grpc_auth::CallerRegistry::new()
        .register(
            platform::grpc_auth::Workload::Gateway,
            "test-gateway-credential",
        )
        .expect("gateway caller")
        .register(platform::grpc_auth::Workload::Post, "test-post-credential")
        .expect("post caller")
        .register(
            platform::grpc_auth::Workload::Friend,
            "test-friend-credential",
        )
        .expect("friend caller")
        .register(platform::grpc_auth::Workload::Auth, "test-auth-credential")
        .expect("auth caller")
}

pub async fn start_postgres_app(seed_enabled: bool) -> Result<TestApp> {
    // Testcontainers remains the default. A dedicated external database lets
    // real-PostgreSQL acceptance run in environments where Docker is absent.
    let external_database_url = std::env::var("PROFILE_TEST_DATABASE_URL")
        .ok()
        .filter(|url| !url.trim().is_empty());
    let (postgres, database_url) = match external_database_url {
        Some(database_url)
            if std::env::var("PROFILE_TEST_CREATE_ISOLATED_DATABASES").as_deref() == Ok("1") =>
        {
            let controller = sqlx::PgPool::connect(&database_url).await?;
            let lineage: String =
                sqlx::query_scalar("select system_identifier::text from pg_control_system()")
                    .fetch_one(&controller)
                    .await?;
            anyhow::ensure!(
                std::env::var("PROFILE_TEST_INITDB_SYSTEM_IDENTIFIER").as_deref() == Ok(&lineage),
                "Profile fixture must use the runner-owned initdb cluster"
            );
            let database = format!(
                "port_rust_profile_fixture_{}",
                uuid::Uuid::new_v4().simple()
            );
            sqlx::query(&format!("create database {database} template template0"))
                .execute(&controller)
                .await?;
            controller.close().await;
            let base = database_url
                .rsplit_once('/')
                .ok_or_else(|| anyhow::anyhow!("fixture URL"))?
                .0;
            let isolated_url = format!("{base}/{database}");
            let observer = sqlx::PgPool::connect(&isolated_url).await?;
            let empty: bool = sqlx::query_scalar(
                "select not exists(select 1 from pg_tables where schemaname='public')",
            )
            .fetch_one(&observer)
            .await?;
            anyhow::ensure!(empty, "Profile fixture must be empty before migrations");
            observer.close().await;
            eprintln!(
                "PROFILE_FRESH_FIXTURE {}",
                serde_json::json!({"database":database,"systemIdentifier":lineage,"template":"template0","emptyBeforeMigrations":empty})
            );
            (None, isolated_url)
        }
        Some(database_url) => (None, database_url),
        None => {
            let (postgres, database_url) =
                test_support::postgres::start_postgres_with_url().await?;
            (Some(postgres), database_url)
        }
    };
    let config = AppConfig {
        database_migration_mode: migration_runtime::MigrationMode::Startup,
        http_addr: "127.0.0.1:18082".parse()?,
        metrics_addr: "127.0.0.1:0".parse()?,
        grpc_addr: "127.0.0.1:15052".parse()?,
        database_pool_max_connections: 5,
        database_url,
        seed_enabled,
        actor_delegation: platform::actor_delegation::ActorDelegationVerifier::local_parity(
            platform::grpc_auth::Workload::Profile,
        ),
        caller_registry: test_caller_registry(),
        tls_material: None,
        retention_purge_worker_enabled: true,
        retention_purge_worker_interval_ms: 1000,
        retention_purge_actor_id: "99999999-9999-9999-9999-999999999999".into(),
        mutation_receipt_retention_worker_enabled: false,
        mutation_receipt_retention_worker_interval_ms: 3_600_000,
        mutation_receipt_retention_batch_size: 1_000,
    };
    let pool = connect_pool(&config).await?;
    let state = Arc::new(build_state(&config, pool.clone()).await?);
    Ok(TestApp {
        _postgres: postgres,
        pool,
        state,
    })
}

pub fn grpc_server(state: Arc<AppState>) -> ProfileGrpcServer {
    ProfileGrpcServer::new(state)
}

impl TestApp {
    pub async fn metrics(&self) -> Result<String> {
        let response = http_routes::metrics_router(self.state.metrics.clone())
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/metrics")
                    .body(Body::empty())?,
            )
            .await?;
        Ok(String::from_utf8(
            response.into_body().collect().await?.to_bytes().to_vec(),
        )?)
    }
}
