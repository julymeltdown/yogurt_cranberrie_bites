//! Actual canonical owners and Notification over isolated mTLS. No positive
//! RootTargetSnapshotOwner exists here: None must refuse even empty/count.
use super::{environment, owners};
use concert_service::{
    adapters::out::relationship_authority::guarded::{
        proto as guard, CanonicalOwnerFence, ChildQuery, ChildRecoveryReference, ScopeRegistry,
    },
    notification_live::{NotificationClient, NotificationRecoveryBinding, NotificationRuntime},
    product_rpc::{
        child_settlement::NotificationInboxIntentSettler,
        delivery::DeliveryPublisher,
        scope_ledger::{ChildIntentSettler, ChildRecoveryKey, ChildRecoveryKeys, PgScopeLedger},
        settlement::{PgNativeAbortResolver, ProducerSettlementService, POLICY},
    },
};
use platform::{
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::guard_without_delegation,
};
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use tonic::Status;
use uuid::Uuid;

// Test composition only: delegates to the actual NB01/NB02 ledger lookup and
// actual Notification recovery transport. Explicitly cleared on Drop; no mock
// successful outcome or actor-forwarded positive binding is possible.
struct Recovery(Mutex<Option<PgScopeLedger>>);
#[async_trait::async_trait]
impl NotificationRecoveryBinding for Recovery {
    async fn load_binding(&self, q: &ChildQuery) -> Result<guard::GuardBinding, Status> {
        let ledger = self
            .0
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| Status::unavailable("fixture not composed"))?;
        ledger.load_binding(q).await
    }
}
struct Settler(Mutex<Option<Arc<NotificationClient>>>);
#[async_trait::async_trait]
impl ChildIntentSettler for Settler {
    async fn recover_one(&self, r: &ChildRecoveryReference) -> Result<(), Status> {
        let client = self
            .0
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| Status::unavailable("fixture not composed"))?;
        client.recover_notification_inbox(r).await
    }
}
pub(super) struct Fixture {
    pub auth: PgPool,
    pub friend: PgPool,
    pub concert: PgPool,
    pub notification: PgPool,
    pub fence: CanonicalOwnerFence,
    pub runtime: NotificationRuntime,
    pub publisher: DeliveryPublisher,
    recovery: Arc<Recovery>,
    settler: Arc<Settler>,
    _auth: owners::Running,
    _friend: owners::Running,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
        self.recovery.0.lock().unwrap().take();
        self.settler.0.lock().unwrap().take();
    }
}
fn incoming(
    listener: tokio::net::TcpListener,
) -> impl futures::Stream<Item = Result<tokio::net::TcpStream, std::io::Error>> + Send + 'static {
    futures::stream::unfold(listener, |l| async {
        let v = l.accept().await.map(|(s, _)| s);
        Some((v, l))
    })
}
impl Fixture {
    pub async fn new() -> anyhow::Result<Self> {
        let auth = environment::pool("auth").await?;
        let friend = environment::pool("friend").await?;
        let concert = environment::pool("concert").await?;
        let notification = environment::pool("notification").await?;
        let root_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let root_url = format!("https://{}", root_listener.local_addr()?);
        let n_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let n_url = format!("https://{}", n_listener.local_addr()?);
        let running_auth = owners::auth(auth.clone(), &root_url, &n_url).await?;
        let running_friend = owners::friend(friend.clone(), &root_url, &n_url).await?;
        let tls = environment::tls(Workload::Notification)?;
        let inbox = notification_service::concert::authority::NotificationInboxService::connect(
            notification.clone(),
            &running_auth.url,
            &running_friend.url,
            "notification-fixture",
            Some(&tls),
            None,
        )
        .await?;
        let n_service = guard_without_delegation(
            &notification_service::concert::authority::INBOX_POLICY,
            inbox.clone().inbox_server(),
            CallerRegistry::new().register(Workload::Concert, "concert-fixture")?,
        )?;
        let n_settle = guard_without_delegation(
            &notification_service::concert::authority::SETTLEMENT_POLICY,
            inbox.settlement_server(),
            CallerRegistry::new()
                .register(Workload::Auth, "auth-fixture")?
                .register(Workload::Friend, "friend-fixture")?,
        )?;
        let server = tonic::transport::Server::builder()
            .layer(platform::call_deadline::CallDeadlineLayer)
            .tls_config(tls.server_config()?)?
            .add_service(n_service)
            .add_service(n_settle);
        let notification_task = tokio::spawn(async move {
            server
                .serve_with_incoming(incoming(n_listener))
                .await
                .expect("actual Notification server");
        });
        let instance = Uuid::new_v4();
        let recovery = Arc::new(Recovery(Mutex::new(None)));
        let settler = Arc::new(Settler(Mutex::new(None)));
        let keys = Arc::new(ChildRecoveryKeys::new(
            ChildRecoveryKey::new("notification-product-fixture", &[88; 32])?,
            vec![],
        )?);
        let ledger = PgScopeLedger::new(concert.clone(), instance, keys, settler.clone())?;
        *recovery.0.lock().unwrap() = Some(ledger.clone());
        let client = Arc::new(NotificationClient::connect(
            &n_url,
            "concert-fixture",
            Some(&environment::tls(Workload::Concert)?),
            Some(recovery.clone()),
        )?);
        *settler.0.lock().unwrap() = Some(client.clone());
        let registry = ScopeRegistry::new(instance, Arc::new(ledger.clone()))?;
        let root = guard_without_delegation(
            &POLICY,
            ProducerSettlementService::new(
                ledger.clone(),
                Arc::new(PgNativeAbortResolver::new(concert.clone(), instance)?),
            )
            .server(),
            CallerRegistry::new()
                .register(Workload::Auth, "auth-fixture")?
                .register(Workload::Friend, "friend-fixture")?,
        )?;
        let server = tonic::transport::Server::builder()
            .layer(platform::call_deadline::CallDeadlineLayer)
            .tls_config(environment::tls(Workload::Concert)?.server_config()?)?
            .add_service(root);
        let root_task = tokio::spawn(async move {
            server
                .serve_with_incoming(incoming(root_listener))
                .await
                .expect("actual Root settlement server");
        });
        let fence = CanonicalOwnerFence::connect(
            &running_auth.url,
            &running_friend.url,
            "concert-fixture",
            &environment::tls(Workload::Concert)?,
            owners::signer()?,
            registry.clone(),
        )?;
        Ok(Self {
            auth,
            friend,
            concert,
            notification,
            fence,
            runtime: NotificationRuntime::new(client),
            publisher: DeliveryPublisher::new(ledger.clone(), registry.clone()),
            recovery,
            settler,
            _auth: running_auth,
            _friend: running_friend,
            tasks: vec![notification_task, root_task],
        })
    }
}
