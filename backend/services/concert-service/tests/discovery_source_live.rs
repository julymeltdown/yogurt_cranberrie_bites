//! The `GrpcDiscoverySource` against a real profile-service gRPC server.
//!
//! This is the live two-service test: a real `DiscoverySourceService` is served
//! over loopback gRPC with the real method policy and the real caller registry,
//! and the concert service's `GrpcDiscoverySource` client reads it. The profile
//! database is the real one, migrated with the real migrations.
//!
//! What it proves that the port-level tests cannot:
//!
//! * the proto encoding round-trips every field, including the optional ones
//!   (`bio`, `profile_image_media_id`, `interaction_style`) and the cursor pair;
//! * the method policy admits the Concert workload and refuses anything else;
//! * the `(updated_at, profile_id)` ordering survives the wire;
//! * a tombstone is delivered as `deleted = true` rather than silently dropped.
//!
//! What it deliberately does **not** claim: this is one process serving a
//! loopback listener, not two containers. It exercises the same code path the
//! two-container deployment takes — the same server, policy, registry, client and
//! proto — but it does not prove container networking, TLS or service discovery.
use chrono::{DateTime, Utc};
use concert_service::discovery::projection::{DiscoverySource, GrpcDiscoverySource};
use platform::grpc_auth::{CallerRegistry, Workload};
// The proto is compiled into both crates, so the profile service's generated
// types and the concert service's are distinct Rust types with the same wire
// shape. The server is built from the profile service's types; the client from
// the concert service's, which is exactly the split a real deployment has.
use concert_service::discovery::projection::proto as client_proto;
use profile_service::discovery_source::{
    proto, DiscoverySourceServer, DISCOVERY_SOURCE_POLICY,
};
use sqlx::PgPool;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::{Channel, Server};
use uuid::Uuid;

const CONCERT_TOKEN: &str = "discovery-source-concert-token";
const OTHER_TOKEN: &str = "discovery-source-other-token";

async fn pool() -> anyhow::Result<PgPool> {
    let url = std::env::var("PROFILE_TEST_DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("PROFILE_TEST_DATABASE_URL is required"))?;
    let options: sqlx::postgres::PgConnectOptions = url.parse()?;
    anyhow::ensure!(
        options.get_username() == "mcr_owner",
        "live source fixture must use the owned cluster"
    );
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;
    migrate(&pool).await?;
    Ok(pool)
}

async fn migrate(pool: &PgPool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('discovery-live-migrations',0))")
        .execute(&mut *tx)
        .await?;
    let applied: bool =
        sqlx::query_scalar("select to_regclass('public.catalog_items') is not null")
            .fetch_one(&mut *tx)
            .await?;
    if applied {
        tx.commit().await?;
        return Ok(());
    }
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../profile-service/migrations");
    let mut entries: Vec<_> = std::fs::read_dir(&directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|extension| extension == "sql")
                && !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(".down.sql"))
        })
        .collect();
    entries.sort();
    for path in entries {
        let sql = std::fs::read_to_string(&path)?;
        sqlx::raw_sql(&sql).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// A live server plus a client connected to it.
struct Live {
    client: GrpcDiscoverySource,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<Result<(), tonic::transport::Error>>>,
}

impl Live {
    async fn start(pool: PgPool) -> anyhow::Result<Self> {
        let registry = CallerRegistry::new()
            .register(Workload::Concert, CONCERT_TOKEN)?
            .register(Workload::Gateway, OTHER_TOKEN)?;
        let service = platform::grpc_method_policy::guard_without_delegation(
            &DISCOVERY_SOURCE_POLICY,
            proto::discovery_source_service_server::DiscoverySourceServiceServer::new(
                DiscoverySourceServer::from_pool(pool),
            ),
            registry,
        )?;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(
            Server::builder()
                .add_service(service)
                .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                    let _ = stopped.await;
                }),
        );
        let channel = Channel::from_shared(format!("http://{address}"))?
            .connect()
            .await?;
        // The production client is built over a channel whose identity is
        // supplied by the transport layer; here the loopback server's registry
        // admits the Concert workload and the client presents it through the
        // same interceptor the real composition uses.
        let client = GrpcDiscoverySource::intercepted(
            client_proto::discovery_source_service_client::DiscoverySourceServiceClient::with_interceptor(
                channel,
                platform::grpc_auth::ServiceIdentityClientInterceptor::new(
                    Workload::Concert,
                    CONCERT_TOKEN,
                )?,
            ),
        );
        Ok(Self {
            client,
            stop: Some(stop),
            task: Some(task),
        })
    }

    async fn shutdown(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            let _ = tokio::time::timeout(Duration::from_secs(5), task).await;
        }
    }
}

struct Fixture {
    pool: PgPool,
}

impl Fixture {
    async fn new() -> anyhow::Result<Self> {
        Ok(Self { pool: pool().await? })
    }

    async fn profile(&self, tag: &str, bio: Option<&str>) -> anyhow::Result<Uuid> {
        let profile_id = Uuid::new_v4();
        sqlx::query(
            "insert into profile_identities(profile_id,account_id,nickname,bio,visibility) \
             values($1,$2,$3,$4,'PUBLIC')",
        )
        .bind(profile_id)
        .bind(Uuid::new_v4())
        .bind(format!("live-{tag}"))
        .bind(bio)
        .execute(&self.pool)
        .await?;
        Ok(profile_id)
    }

    async fn favorite(&self, profile_id: Uuid, label: &str) -> anyhow::Result<Uuid> {
        let item = Uuid::new_v4();
        sqlx::query(
            "insert into catalog_items(catalog_item_id,category,label,source,exclusion_reason) \
             values($1,'music',$2,'test','reviewed: not k-pop')",
        )
        .bind(item)
        .bind(format!("{label} {}", &item.simple().to_string()[..8]))
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "insert into profile_favorites(profile_id,category,catalog_item_id,position) \
             values($1,'music',$2,0)",
        )
        .bind(profile_id)
        .bind(item)
        .execute(&self.pool)
        .await?;
        Ok(item)
    }
}

// ---------------------------------------------------------------------------
// The live read
// ---------------------------------------------------------------------------

/// The client reads real rows through the real server, and every field survives
/// the wire.
#[tokio::test]
async fn the_client_reads_real_profiles_over_grpc() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let subject = fixture.profile("roundtrip", Some("a live bio")).await?;
    fixture.favorite(subject, "Live Band").await?;
    let live = Live::start(fixture.pool.clone()).await?;

    let page = live.client.read_changed(None, 100).await?;
    live.shutdown().await;

    let profile = page
        .profiles
        .iter()
        .find(|profile| profile.profile_id == subject)
        .expect("the profile must be in the page");
    assert_eq!(profile.nickname, "live-roundtrip");
    assert_eq!(profile.bio.as_deref(), Some("a live bio"));
    assert_eq!(profile.visibility, "PUBLIC");
    assert!(!profile.deleted);
    // The optional fields round-trip as absent, not as empty strings.
    assert_eq!(profile.profile_image_media_id, None);
    assert_eq!(profile.interaction_style, None);
    // The favorite survives with its catalog label.
    assert_eq!(profile.favorites.len(), 1);
    assert!(profile.favorites[0].label.starts_with("Live Band"));
    assert_eq!(profile.favorites[0].category, "music");
    Ok(())
}

/// The cursor round-trips: a second read after the first page's cursor returns
/// only what changed later.
#[tokio::test]
async fn the_cursor_round_trips_over_grpc() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let first = fixture.profile("cursor-first", None).await?;
    let live = Live::start(fixture.pool.clone()).await?;
    let page = live.client.read_changed(None, 100).await?;
    assert!(page.profiles.iter().any(|p| p.profile_id == first));
    let cursor = page.next.expect("a non-empty page has a cursor");

    // A new profile appears after the cursor.
    let second = fixture.profile("cursor-second", None).await?;
    let next = live.client.read_changed(Some(cursor), 100).await?;
    live.shutdown().await;
    assert!(
        next.profiles.iter().any(|p| p.profile_id == second),
        "the later profile must be returned after the cursor"
    );
    assert!(
        !next.profiles.iter().any(|p| p.profile_id == first),
        "a profile before the cursor must not be returned again"
    );
    Ok(())
}

/// A tombstone is delivered as `deleted = true` rather than dropped, so the
/// projection can remove the row.
#[tokio::test]
async fn a_tombstone_is_delivered_over_grpc() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let subject = fixture.profile("tombstone", None).await?;
    let live = Live::start(fixture.pool.clone()).await?;
    let page = live.client.read_changed(None, 100).await?;
    let cursor = page.next.expect("a cursor");
    // The profile is deleted after the cursor.
    sqlx::query(
        "update profile_identities set deleted_at=clock_timestamp(), revision=revision+1 \
         where profile_id=$1",
    )
    .bind(subject)
    .execute(&fixture.pool)
    .await?;
    let next = live.client.read_changed(Some(cursor), 100).await?;
    live.shutdown().await;
    let tombstone = next
        .profiles
        .iter()
        .find(|profile| profile.profile_id == subject)
        .expect("the tombstone must be delivered");
    assert!(tombstone.deleted);
    Ok(())
}

/// The method policy refuses a caller that is not the Concert workload.
#[tokio::test]
async fn a_non_concert_caller_is_refused() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let registry = CallerRegistry::new()
        .register(Workload::Concert, CONCERT_TOKEN)?
        .register(Workload::Gateway, OTHER_TOKEN)?;
    let service = platform::grpc_method_policy::guard_without_delegation(
        &DISCOVERY_SOURCE_POLICY,
        proto::discovery_source_service_server::DiscoverySourceServiceServer::new(
            DiscoverySourceServer::from_pool(fixture.pool.clone()),
        ),
        registry,
    )?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(
        Server::builder()
            .add_service(service)
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = stopped.await;
            }),
    );
    let channel = Channel::from_shared(format!("http://{address}"))?
        .connect()
        .await?;
    // The Gateway workload is registered but not permitted on this method.
    let mut client = client_proto::discovery_source_service_client::DiscoverySourceServiceClient::with_interceptor(
        channel,
        platform::grpc_auth::ServiceIdentityClientInterceptor::new(
            Workload::Gateway,
            OTHER_TOKEN,
        )?,
    );
    let result = client
        .read_changed(client_proto::ReadChangedRequest {
            after_updated_at_unix_micros: None,
            after_profile_id: None,
            limit: 10,
        })
        .await;
    let _ = stop.send(());
    let _ = tokio::time::timeout(Duration::from_secs(5), task).await;
    let status = result.expect_err("a non-Concert caller must be refused");
    assert!(
        matches!(
            status.code(),
            tonic::Code::PermissionDenied | tonic::Code::Unauthenticated
        ),
        "unexpected status: {status:?}"
    );
    Ok(())
}

/// The ordering survives the wire: the page is ordered by `(updated_at,
/// profile_id)`.
#[tokio::test]
async fn the_page_order_survives_the_wire() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let mut created = Vec::new();
    for index in 0..4 {
        created.push(fixture.profile(&format!("order-{index}"), None).await?);
    }
    let live = Live::start(fixture.pool.clone()).await?;
    let page = live.client.read_changed(None, 100).await?;
    live.shutdown().await;
    // Every created profile appears, and the page is sorted by the pair.
    for profile in &created {
        assert!(page.profiles.iter().any(|p| p.profile_id == *profile));
    }
    let ordered = page.profiles.windows(2).all(|pair| {
        (pair[0].updated_at, pair[0].profile_id) <= (pair[1].updated_at, pair[1].profile_id)
    });
    assert!(ordered, "the page must be ordered by (updated_at, profile_id)");
    Ok(())
}
