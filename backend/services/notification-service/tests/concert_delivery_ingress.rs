//! Actual mTLS ingress and Notification-owned PostgreSQL. Reuses BE33's owned
//! fixture guard; no successful Auth/Friend/target mock participates in delivery.
#[path = "concert_inbox_support/mod.rs"]
mod support;
use notification_service::concert::{
    delivery::{NotificationDeliveryIngress, DELIVERY_POLICY},
    transport::proto,
};
use platform::{
    grpc_auth::{CallerRegistry, ServiceIdentityClientInterceptor, Workload},
    grpc_tls::TlsMaterial,
};
use sqlx::{PgPool, Row};
use std::{path::PathBuf, time::Duration};
use tokio::sync::oneshot;
use tonic::{
    service::interceptor::InterceptedService,
    transport::{Channel, Server},
    Request,
};
use uuid::Uuid;
type Client = proto::concert_delivery_service_client::ConcertDeliveryServiceClient<
    InterceptedService<Channel, ServiceIdentityClientInterceptor>,
>;
const CONCERT_TOKEN: &str = "be33-test-concert-workload";
const POST_TOKEN: &str = "be33-test-post-workload";
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
fn tls(role: Workload) -> anyhow::Result<TlsMaterial> {
    let pki = PathBuf::from(std::env::var("CONCERT_TEST_OWNER_ROOT")?).join("pki");
    TlsMaterial::load(
        pki.join(format!("{}.crt", role.as_str())),
        pki.join(format!("{}.key", role.as_str())),
        pki.join("ca.crt"),
    )
}
impl Fixture {
    async fn new() -> anyhow::Result<Self> {
        let pool = support::owned_pool("mcr_concert_notification").await?;
        migration_runtime::prepare_database(
            &pool,
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations"),
            migration_runtime::MigrationMode::Startup,
            &[],
        )
        .await?;
        let registry = CallerRegistry::new()
            .register(Workload::Concert, CONCERT_TOKEN)?
            .register(Workload::Post, POST_TOKEN)?;
        let guarded = platform::grpc_method_policy::guard_without_delegation(
            &DELIVERY_POLICY,
            NotificationDeliveryIngress::new(pool.clone()).server(),
            registry,
        )?;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("https://{}", listener.local_addr()?);
        let (stop, shutdown) = oneshot::channel();
        let server = Server::builder()
            .tls_config(tls(Workload::Notification)?.server_config()?)?
            .layer(platform::call_deadline::CallDeadlineLayer)
            .add_service(guarded);
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
    async fn client(
        &self,
        certificate: Workload,
        identity: Workload,
        token: &str,
    ) -> anyhow::Result<Client> {
        let channel = tls(certificate)?
            .connect(&self.endpoint, Workload::Notification)
            .await?;
        Ok(Client::new(InterceptedService::new(
            channel,
            ServiceIdentityClientInterceptor::new(identity, token)?,
        )))
    }
    async fn finish(mut self) -> anyhow::Result<()> {
        let _ = self.stop.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(3), self.task.take().unwrap()).await???;
        self.pool.close().await;
        Ok(())
    }
}
fn message() -> proto::DeliverRequest {
    proto::DeliverRequest {
        envelope: Some(proto::DeliveryEnvelope {
            outbox_id: Uuid::new_v4().to_string(),
            receipt_id: Uuid::new_v4().to_string(),
            event_id: Uuid::new_v4().to_string(),
            resource_id: Uuid::new_v4().to_string(),
            resource_revision: 1,
        }),
        recipients: vec![proto::DeliveryRecipient {
            user_id: Uuid::new_v4().to_string(),
            source_actor_id: Some(Uuid::new_v4().to_string()),
            kind: 3,
            target: Some(proto::Target {
                target_type: 5,
                target_id: Uuid::new_v4().to_string(),
            }),
        }],
    }
}
fn request(value: proto::DeliverRequest) -> Request<proto::DeliverRequest> {
    let mut request = Request::new(value);
    request.set_timeout(Duration::from_secs(2));
    request
}
#[tokio::test]
async fn concurrent_repeated_delivery_freezes_fanout_after_actual_commit() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let mut a = f
        .client(Workload::Concert, Workload::Concert, CONCERT_TOKEN)
        .await?;
    let mut b = a.clone();
    let message = message();
    let expected = message.envelope.clone();
    let (left, right) = tokio::join!(
        a.deliver(request(message.clone())),
        b.deliver(request(message.clone()))
    );
    assert_eq!(left?.into_inner().committed_envelope, expected);
    assert_eq!(right?.into_inner().committed_envelope, expected);
    let mut changed = message.clone();
    changed.recipients[0].user_id = Uuid::new_v4().to_string();
    assert_eq!(
        a.deliver(request(changed))
            .await?
            .into_inner()
            .committed_envelope,
        expected
    );
    let outbox = Uuid::parse_str(&expected.unwrap().outbox_id)?;
    let rows = sqlx::query("select user_id,type,payload_json from notifications where event_id=$1")
        .bind(outbox)
        .fetch_all(&f.pool)
        .await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get::<Uuid, _>("user_id").to_string(),
        message.recipients[0].user_id
    );
    assert_eq!(rows[0].get::<String, _>("type"), "MCR_SOCIAL");
    assert_eq!(rows[0].get::<String, _>("payload_json"), "");
    let mut conflict = message.clone();
    conflict.envelope.as_mut().unwrap().resource_revision = 2;
    assert_eq!(
        a.deliver(request(conflict)).await.unwrap_err().code(),
        tonic::Code::AlreadyExists
    );
    // Simulated owning purge: keep the immutable delivery receipt, remove item.
    sqlx::query("delete from notifications where event_id=$1")
        .bind(outbox)
        .execute(&f.pool)
        .await?;
    a.deliver(request(message)).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from notifications where event_id=$1")
            .bind(outbox)
            .fetch_one(&f.pool)
            .await?,
        0
    );
    drop((a, b));
    f.finish().await
}
#[tokio::test]
async fn wrong_workload_and_mismatched_certificate_cannot_insert() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let message = message();
    let id = Uuid::parse_str(&message.envelope.as_ref().unwrap().outbox_id)?;
    let mut post = f.client(Workload::Post, Workload::Post, POST_TOKEN).await?;
    assert_eq!(
        post.deliver(request(message.clone()))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::PermissionDenied
    );
    let mut mismatch = f
        .client(Workload::Post, Workload::Concert, CONCERT_TOKEN)
        .await?;
    assert!(matches!(
        mismatch.deliver(request(message)).await.unwrap_err().code(),
        tonic::Code::Unauthenticated | tonic::Code::PermissionDenied
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from concert_notification_deliveries where outbox_id=$1"
        )
        .bind(id)
        .fetch_one(&f.pool)
        .await?,
        0
    );
    drop((post, mismatch));
    f.finish().await
}
#[tokio::test]
async fn invalid_closed_input_is_rejected_before_any_delivery_receipt() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let mut client = f
        .client(Workload::Concert, Workload::Concert, CONCERT_TOKEN)
        .await?;
    for mutation in 0..5 {
        let mut value = message();
        let outbox = Uuid::parse_str(&value.envelope.as_ref().unwrap().outbox_id)?;
        match mutation {
            0 => value.recipients[0].kind = 0,
            1 => value.recipients[0].target.as_mut().unwrap().target_type = 99,
            2 => value.recipients.push(value.recipients[0].clone()),
            3 => value.envelope.as_mut().unwrap().resource_revision = 0,
            _ => value.recipients[0].user_id = Uuid::nil().to_string(),
        }
        assert_eq!(
            client.deliver(request(value)).await.unwrap_err().code(),
            tonic::Code::InvalidArgument
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "select count(*) from concert_notification_deliveries where outbox_id=$1"
            )
            .bind(outbox)
            .fetch_one(&f.pool)
            .await?,
            0
        );
    }
    drop(client);
    f.finish().await
}
