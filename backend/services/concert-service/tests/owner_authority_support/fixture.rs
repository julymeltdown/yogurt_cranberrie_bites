//! Real production root ledger/settlement server and real owner RPCs. The first
//! cases register no producer transactions/children, so recovery adapters reject
//! every call and count them. No fixture can manufacture a positive owner scope.
use super::{environment, owners};
use concert_service::{
    adapters::out::relationship_authority::guarded::{
        CanonicalOwnerFence, ChildRecoveryReference, OwnerGuardScope, ScopeRegistry,
    },
    product_rpc::{
        scope_ledger::{
            ChildIntentSettler, ChildRecoveryKey, ChildRecoveryKeys, PgScopeLedger,
            ProducerTransaction, ScopeKey,
        },
        settlement::{
            NativeTransactionResolution, NativeTransactionResolver, ProducerSettlementService,
            POLICY,
        },
    },
};
use platform::{
    actor_delegation::owner_binding::OwnerBinding,
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::guard_without_delegation,
};
use sqlx::PgPool;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tonic::Status;
use uuid::Uuid;

struct RejectEffects(AtomicUsize);
#[async_trait::async_trait]
impl ChildIntentSettler for RejectEffects {
    async fn recover_one(&self, _: &ChildRecoveryReference) -> Result<(), Status> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(Status::unavailable(
            "No child effects configured in core fixture",
        ))
    }
}
#[async_trait::async_trait]
impl NativeTransactionResolver for RejectEffects {
    async fn resolve(
        &self,
        _: &OwnerBinding,
        _: &ProducerTransaction,
    ) -> Result<NativeTransactionResolution, Status> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(Status::unavailable(
            "No producer effects configured in core fixture",
        ))
    }
}
pub(super) struct Fixture {
    pub(super) auth: PgPool,
    pub(super) friend: PgPool,
    pub(super) concert: PgPool,
    pub(super) registry: Arc<ScopeRegistry>,
    pub(super) fence: CanonicalOwnerFence,
    ledger: PgScopeLedger,
    settlement_url: String,
    effects: Arc<RejectEffects>,
    _auth: owners::Running,
    _friend: owners::Running,
    root_task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.root_task.abort();
    }
}
impl Fixture {
    pub(super) async fn new() -> anyhow::Result<Self> {
        Self::build(false).await
    }
    pub(super) async fn with_native_abort() -> anyhow::Result<Self> {
        Self::build(true).await
    }
    async fn build(native_abort: bool) -> anyhow::Result<Self> {
        let auth = environment::pool("auth").await?;
        let friend = environment::pool("friend").await?;
        let concert = environment::pool("concert").await?;
        let instance = Uuid::new_v4();
        let effects = Arc::new(RejectEffects(AtomicUsize::new(0)));
        // Stable explicit fixture key, never an environment/parity product default.
        let keys = Arc::new(ChildRecoveryKeys::new(
            ChildRecoveryKey::new("canonical-fixture-v1", &[88; 32])?,
            vec![],
        )?);
        let ledger = PgScopeLedger::new(concert.clone(), instance, keys, effects.clone())?;
        let native: Arc<dyn NativeTransactionResolver> = if native_abort {
            Arc::new(
                concert_service::product_rpc::settlement::PgNativeAbortResolver::new(
                    concert.clone(),
                    instance,
                )?,
            )
        } else {
            effects.clone()
        };
        let registry = ScopeRegistry::new(instance, Arc::new(ledger.clone()))?;
        let service = guard_without_delegation(
            &POLICY,
            ProducerSettlementService::new(ledger.clone(), native).server(),
            CallerRegistry::new()
                .register(Workload::Auth, "auth-fixture")?
                .register(Workload::Friend, "friend-fixture")?,
        )?;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("https://{}", listener.local_addr()?);
        let stream = futures::stream::unfold(listener, |listener| async {
            let result = listener.accept().await.map(|(stream, _)| stream);
            Some((result, listener))
        });
        let server = tonic::transport::Server::builder()
            .tls_config(environment::tls(Workload::Concert)?.server_config()?)?
            .add_service(service);
        let root_task = tokio::spawn(async move {
            server
                .serve_with_incoming(stream)
                .await
                .expect("actual root settlement server");
        });
        let running_auth = owners::auth(auth.clone(), &url).await?;
        let running_friend = owners::friend(friend.clone(), &url).await?;
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
            registry,
            fence,
            ledger,
            settlement_url: url,
            effects,
            _auth: running_auth,
            _friend: running_friend,
            root_task,
        })
    }
    pub(super) fn delivery_publisher(
        &self,
    ) -> concert_service::product_rpc::delivery::DeliveryPublisher {
        concert_service::product_rpc::delivery::DeliveryPublisher::new(
            self.ledger.clone(),
            self.registry.clone(),
        )
    }
    pub(super) async fn bind_delivery(&self, scope: &OwnerGuardScope) -> anyhow::Result<()> {
        let value = scope.producer_scope();
        self.ledger
            .bind_delivery(
                &ScopeKey {
                    instance: Uuid::parse_str(&value.instance_id)?,
                    scope: Uuid::parse_str(&value.scope_id)?,
                    binding_digest: value.binding_digest.as_slice().try_into()?,
                },
                &[89; 32],
            )
            .await?;
        Ok(())
    }
    pub(super) fn interrupted_fence(
        &self,
        friend_endpoint: &str,
    ) -> anyhow::Result<CanonicalOwnerFence> {
        CanonicalOwnerFence::connect(
            &self._auth.url,
            friend_endpoint,
            "concert-fixture",
            &environment::tls(Workload::Concert)?,
            owners::signer()?,
            self.registry.clone(),
        )
    }
    pub(super) async fn no_unfinished_root(&self) -> anyhow::Result<bool> {
        let count:i64=sqlx::query_scalar("select count(*) from concert_product_owner_scopes where instance_id=$1 and phase<>'ROOT_FINISHED'").bind(self.ledger.instance()).fetch_one(&self.concert).await?;
        Ok(count == 0)
    }
    pub(super) async fn recover_auth_fresh(&self) -> anyhow::Result<u32> {
        use auth_service::concert_authority::{
            proto, OperationBudget, PgOwnerAuthority, RpcSettlementResolver,
        };
        let resolver = RpcSettlementResolver::new(
            &environment::tls(Workload::Auth)?,
            "auth-fixture",
            &[(proto::Producer::Concert, self.settlement_url.clone())],
        )?;
        let fresh = PgOwnerAuthority::new(self.auth.clone(), Arc::new(resolver))?;
        Ok(fresh
            .recover(&OperationBudget::new(
                std::time::Instant::now() + std::time::Duration::from_secs(2),
            )?)
            .await?)
    }
    pub(super) fn assert_no_recovery_calls(&self) {
        assert_eq!(self.effects.0.load(Ordering::SeqCst), 0);
    }
}
