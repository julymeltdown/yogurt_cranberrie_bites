//! Actual owning repository, guarded mTLS RPC and isolated PostgreSQL. These
//! checks qualify suppression/retention transport, never global private erasure.
#[path = "concert_inbox_support/mod.rs"]
mod owned;
use chrono::{DateTime, Utc};
use notification_service::{
    adapters::r#in::grpc::{
        method_policy::NOTIFICATION_METHOD_POLICY,
        server::{proto, NotificationGrpcServer},
    },
    config::{build_state, AppConfig},
};
use platform::{
    grpc_auth::{CallerRegistry, ServiceIdentityClientInterceptor, Workload},
    grpc_tls::TlsMaterial,
};
use sqlx::PgPool;
use std::{os::unix::fs::OpenOptionsExt, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::oneshot;
use tonic::{
    service::interceptor::InterceptedService,
    transport::{Channel, Server},
    Request,
};
use uuid::Uuid;
const AUTH: &str = "be33-transport-auth";
const POST: &str = "be33-transport-post";
type Client = proto::notification_service_client::NotificationServiceClient<
    InterceptedService<Channel, ServiceIdentityClientInterceptor>,
>;
fn root() -> anyhow::Result<PathBuf> {
    Ok(PathBuf::from(std::env::var("CONCERT_TEST_OWNER_ROOT")?))
}
fn tls(role: Workload) -> anyhow::Result<TlsMaterial> {
    let pki = root()?.join("pki");
    TlsMaterial::load(
        pki.join(format!("{}.crt", role.as_str())),
        pki.join(format!("{}.key", role.as_str())),
        pki.join("ca.crt"),
    )
}
struct Fixture {
    pool: PgPool,
    endpoint: String,
    stop: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<Result<(), tonic::transport::Error>>>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}
impl Fixture {
    async fn new() -> anyhow::Result<Self> {
        let pool = owned::owned_pool("mcr_concert_notification").await?;
        migration_runtime::prepare_database(
            &pool,
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations"),
            migration_runtime::MigrationMode::Startup,
            &[],
        )
        .await?;
        let registry = CallerRegistry::new()
            .register(Workload::Auth, AUTH)?
            .register(Workload::Post, POST)?;
        let config = AppConfig {
            database_migration_mode: migration_runtime::MigrationMode::Startup,
            http_addr: "127.0.0.1:0".parse()?,
            metrics_addr: "127.0.0.1:0".parse()?,
            grpc_addr: "127.0.0.1:0".parse()?,
            database_url: std::env::var("CONCERT_TEST_NOTIFICATION_DATABASE_URL")?,
            database_pool_max_connections: 6,
            actor_delegation: platform::actor_delegation::ActorDelegationVerifier::local_parity(
                Workload::Notification,
            ),
            caller_registry: registry.clone(),
            tls_material: Some(tls(Workload::Notification)?),
            retention_purge_worker_enabled: false,
            retention_purge_worker_interval_ms: 1000,
            retention_purge_actor_id: Uuid::new_v4().to_string(),
        };
        let state = Arc::new(build_state(&config, pool.clone()).await?);
        let service = platform::grpc_method_policy::guard_without_delegation(
            &NOTIFICATION_METHOD_POLICY,
            proto::notification_service_server::NotificationServiceServer::new(
                NotificationGrpcServer::new(state),
            ),
            registry,
        )?;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("https://{}", listener.local_addr()?);
        let (stop, shutdown) = oneshot::channel();
        let server = Server::builder()
            .tls_config(tls(Workload::Notification)?.server_config()?)?
            .layer(platform::call_deadline::CallDeadlineLayer)
            .add_service(service);
        let task = tokio::spawn(server.serve_with_incoming_shutdown(
            tokio_stream::wrappers::TcpListenerStream::new(listener),
            async {
                let _ = shutdown.await;
            },
        ));
        Ok(Self {
            pool,
            endpoint,
            stop: Some(stop),
            task: Some(task),
        })
    }
    async fn client(&self, role: Workload, token: &str) -> anyhow::Result<Client> {
        let channel = tls(role)?
            .connect(&self.endpoint, Workload::Notification)
            .await?;
        Ok(Client::new(InterceptedService::new(
            channel,
            ServiceIdentityClientInterceptor::new(role, token)?,
        )))
    }
    async fn finish(mut self) -> anyhow::Result<()> {
        let _ = self.stop.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(3), self.task.take().unwrap()).await???;
        self.pool.close().await;
        Ok(())
    }
}
fn command() -> proto::ApplyUserDeletionRequest {
    proto::ApplyUserDeletionRequest {
        saga_id: Uuid::new_v4().to_string(),
        user_id: Uuid::new_v4().to_string(),
        actor_id: Uuid::new_v4().to_string(),
        retention_until: "2027-01-15T08:00:00.123456789+09:00".into(),
    }
}
fn bounded(value: proto::ApplyUserDeletionRequest) -> Request<proto::ApplyUserDeletionRequest> {
    let mut request = Request::new(value);
    request.set_timeout(Duration::from_secs(2));
    request
}
#[tokio::test]
#[ignore = "explicit owned PG16/PKI resource offer required"]
async fn actual_rpc_returns_committed_exact_receipt_and_rejects_rebound_and_non_auth(
) -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let mut auth = f.client(Workload::Auth, AUTH).await?;
    let c = command();
    let response = auth
        .apply_user_deletion(bounded(c.clone()))
        .await?
        .into_inner();
    assert!(response.applied);
    let receipt = response.committed_receipt.as_ref().unwrap();
    assert_eq!(
        (
            receipt.version,
            receipt.saga_id.as_str(),
            receipt.user_id.as_str(),
            receipt.actor_id.as_str()
        ),
        (
            1,
            c.saga_id.as_str(),
            c.user_id.as_str(),
            c.actor_id.as_str()
        )
    );
    let retention = DateTime::parse_from_rfc3339(&c.retention_until)?;
    assert_eq!(
        (receipt.retention_seconds, receipt.retention_nanos),
        (Some(retention.timestamp()), Some(123456789))
    );
    assert!(receipt.fence_revision > 0 && receipt.effect_xid.parse::<u64>()? > 0);
    assert!(DateTime::parse_from_rfc3339(&receipt.recorded_at).is_ok());
    let row:(String,i64)=sqlx::query_as("select effect_xid::text,fence_revision from notification_deletion_receipts_v1 where saga_id=$1").bind(Uuid::parse_str(&c.saga_id)?).fetch_one(&f.pool).await?;
    assert_eq!(row, (receipt.effect_xid.clone(), receipt.fence_revision));
    let mut equivalent = c.clone();
    equivalent.retention_until = retention.with_timezone(&Utc).to_rfc3339();
    assert_eq!(
        auth.apply_user_deletion(bounded(equivalent))
            .await?
            .into_inner(),
        response
    );
    for field in 0..3 {
        let mut rebound = c.clone();
        match field {
            0 => rebound.user_id = Uuid::new_v4().to_string(),
            1 => rebound.actor_id = Uuid::new_v4().to_string(),
            _ => rebound.retention_until = "2027-01-15T08:00:00.123456790+09:00".into(),
        }
        assert!(auth.apply_user_deletion(bounded(rebound)).await.is_err());
    }
    let mut post = f.client(Workload::Post, POST).await?;
    let denied = command();
    let denied_saga = Uuid::parse_str(&denied.saga_id)?;
    assert!(post.apply_user_deletion(bounded(denied)).await.is_err());
    let count: i64 = sqlx::query_scalar(
        "select count(*) from notification_deletion_receipts_v1 where saga_id=$1",
    )
    .bind(denied_saga)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(count, 0);
    f.finish().await
}
#[tokio::test]
#[ignore = "explicit owned PG16/PKI resource offer required"]
async fn actual_rpc_never_returns_receipt_on_deferred_commit_failure() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let mut auth = f.client(Workload::Auth, AUTH).await?;
    let c = command();
    sqlx::raw_sql("create function be33_rpc_commit_failure() returns trigger language plpgsql as $$ begin raise exception 'owned RPC commit failure'; end $$; create constraint trigger be33_rpc_commit_failure after insert on notification_deletion_receipts_v1 deferrable initially deferred for each row execute function be33_rpc_commit_failure();").execute(&f.pool).await?;
    let failed = auth.apply_user_deletion(bounded(c.clone())).await;
    sqlx::raw_sql("drop trigger be33_rpc_commit_failure on notification_deletion_receipts_v1; drop function be33_rpc_commit_failure();").execute(&f.pool).await?;
    assert!(failed.is_err());
    let leftovers:i64=sqlx::query_scalar("select (select count(*) from notification_deletion_receipts_v1 where saga_id=$1)+(select count(*) from notification_deletion_fences_v1 where subject_id=$2)+(select count(*) from notification_subject_states where subject_id=$2)+(select count(*) from notification_user_deletion_receipts where saga_id=$1)+(select count(*) from notification_audit_events where id=$1)").bind(Uuid::parse_str(&c.saga_id)?).bind(Uuid::parse_str(&c.user_id)?).fetch_one(&f.pool).await?;
    assert_eq!(leftovers, 0);
    assert!(auth
        .apply_user_deletion(bounded(c))
        .await?
        .into_inner()
        .committed_receipt
        .is_some());
    f.finish().await
}
#[tokio::test]
#[ignore = "external paired Auth controller owns start/stop and actual PG restart"]
async fn serve_actual_rpc_for_auth_worker() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let endpoint = root()?.join("notification-receipt-endpoint");
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&endpoint)?;
    file.write_all(f.endpoint.as_bytes())?;
    file.sync_all()?;
    let stop = root()?.join("notification-receipt-stop");
    tokio::time::timeout(Duration::from_secs(150), async {
        while !stop.exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?;
    f.finish().await?;
    std::fs::remove_file(endpoint)?;
    Ok(())
}
