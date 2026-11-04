//! Adapted from native.rs and owner_authority_support/product.rs; provenance.json.
//! Actual owner stores, Profile mTLS, production reader, ledger and native resolver.
use super::{capture, environment, owners, release};
use concert_service::{
    adapters::out::{
        public_profile::PublicProfileReader,
        relationship_authority::guarded::{
            CanonicalOwnerFence, ChildRecoveryReference, ScopeRegistry,
        },
    },
    product_rpc::{
        scope_ledger::{ChildIntentSettler, ChildRecoveryKey, ChildRecoveryKeys, PgScopeLedger},
        settlement::{PgNativeAbortResolver, ProducerSettlementService, POLICY},
    },
};
use platform::{
    actor_delegation::{relationship_read::RelationshipReadVerifier, ActorDelegationVerifier},
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::{guard, guard_without_delegation},
};
use sqlx::PgPool;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use uuid::Uuid;

struct Settle {
    reader: Arc<PublicProfileReader>,
    local_unavailable: Arc<AtomicBool>,
}
#[async_trait::async_trait]
impl ChildIntentSettler for Settle {
    async fn recover_one(&self, reference: &ChildRecoveryReference) -> Result<(), tonic::Status> {
        if self.local_unavailable.load(Ordering::SeqCst)
            && matches!(reference, ChildRecoveryReference::LocalPublic { .. })
        {
            return Err(tonic::Status::unavailable(
                "CR01 negative Local settlement fault",
            ));
        }
        self.reader.recover_one(reference).await
    }
}
fn incoming(
    listener: tokio::net::TcpListener,
) -> impl futures::Stream<Item = Result<tokio::net::TcpStream, std::io::Error>> + Send + 'static {
    futures::stream::unfold(listener, |listener| async {
        Some((listener.accept().await.map(|(stream, _)| stream), listener))
    })
}
fn spawn<F>(future: F) -> tokio::task::JoinHandle<()>
where
    F: std::future::Future<Output = Result<(), tonic::transport::Error>> + Send + 'static,
{
    tokio::spawn(async move { future.await.expect("actual CR01 fixture service") })
}
pub(super) struct Fixture {
    pub auth: PgPool,
    pub friend: PgPool,
    pub concert: PgPool,
    pub profile: PgPool,
    pub reader: Arc<PublicProfileReader>,
    pub ledger: PgScopeLedger,
    pub registry: Arc<ScopeRegistry>,
    pub fence: CanonicalOwnerFence,
    pub event: Uuid,
    child_unavailable: Arc<AtomicBool>,
    pub profile_completion_failures: Arc<AtomicUsize>,
    root_url: String,
    _auth: owners::Running,
    _friend: owners::Running,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}
impl Fixture {
    pub async fn new(instance: Uuid, deny_auth_complete: bool) -> anyhow::Result<Self> {
        let keys = Arc::new(ChildRecoveryKeys::new(
            ChildRecoveryKey::new("public-native-v1", &[89; 32])?,
            vec![],
        )?);
        Self::with_keys(instance, deny_auth_complete, keys).await
    }
    pub async fn with_keys(
        instance: Uuid,
        deny_auth_complete: bool,
        keys: Arc<ChildRecoveryKeys>,
    ) -> anyhow::Result<Self> {
        let auth = environment::pool("auth").await?;
        let friend = environment::pool("friend").await?;
        let concert = environment::pool("concert").await?;
        let profile = environment::pool("profile").await?;
        let release = release::product_release()?;
        let document: serde_json::Value =
            serde_json::from_slice(release.release().bootstrap_json())?;
        let event: Uuid = document["event"]["event_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("selected release event missing"))?
            .parse()?;
        use profile_service::adapters::r#in::grpc::concert_public_profile::{
            proto, PublicProfileServer, PUBLIC_PROFILE_POLICY,
        };
        let profile_service = guard(
            &PUBLIC_PROFILE_POLICY,
            proto::concert_public_profile_service_server::ConcertPublicProfileServiceServer::new(
                PublicProfileServer::from_pool(profile.clone()),
            ),
            CallerRegistry::new().register(Workload::Concert, "concert-fixture")?,
            RelationshipReadVerifier::new(
                Workload::Profile,
                &[owners::signer()?.public_key()],
                &ActorDelegationVerifier::new(
                    Workload::Profile,
                    &[capture::gateway()?.public_key()],
                )?,
            )?
            .into_inner(),
        )?;
        let child_unavailable = Arc::new(AtomicBool::new(false));
        let profile_completion_failures = Arc::new(AtomicUsize::new(0));
        let profile_service = super::fault::NegativeComplete::controlled(
            profile_service,
            child_unavailable.clone(),
            profile_completion_failures.clone(),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let profile_url = format!("https://{}", listener.local_addr()?);
        let profile_task = spawn(
            tonic::transport::Server::builder()
                .tls_config(environment::tls(Workload::Profile)?.server_config()?)?
                .layer(CallDeadlineLayer)
                .add_service(profile_service)
                .serve_with_incoming(incoming(listener)),
        );
        let reader = Arc::new(PublicProfileReader::connect(
            concert.clone(),
            release,
            &profile_url,
            "concert-fixture",
            Some(&environment::tls(Workload::Concert)?),
            owners::signer()?,
        )?);
        let ledger = PgScopeLedger::new(
            concert.clone(),
            instance,
            keys,
            Arc::new(Settle {
                reader: reader.clone(),
                local_unavailable: child_unavailable.clone(),
            }),
        )?;
        // Always construct a NEW registry. Restart never activates/rebuilds a lost scope.
        let registry = ScopeRegistry::new(instance, Arc::new(ledger.clone()))?;
        let root_service = guard_without_delegation(
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
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let root_url = format!("https://{}", listener.local_addr()?);
        let root_task = spawn(
            tonic::transport::Server::builder()
                .tls_config(environment::tls(Workload::Concert)?.server_config()?)?
                .add_service(root_service)
                .serve_with_incoming(incoming(listener)),
        );
        let running_auth = owners::auth(auth.clone(), &root_url, deny_auth_complete).await?;
        let running_friend = owners::friend(friend.clone(), &root_url).await?;
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
            profile,
            reader,
            ledger,
            registry,
            fence,
            event,
            child_unavailable,
            profile_completion_failures,
            root_url,
            _auth: running_auth,
            _friend: running_friend,
            tasks: vec![profile_task, root_task],
        })
    }
    pub fn make_child_settlement_unavailable(&self) {
        self.child_unavailable.store(true, Ordering::SeqCst);
    }
    /// Actual bounded owner sweep. Original expiry selects candidate leases;
    /// only genuine RootFinished RPC proof authorizes release. Unfinished-root
    /// scenarios invoke this as a negative case and must retain their guards.
    pub async fn recover_owners(&self) -> anyhow::Result<()> {
        use auth_service::concert_authority::{
            proto as auth_proto, OperationBudget as AuthBudget, PgOwnerAuthority as AuthOwner,
            RpcSettlementResolver as AuthResolver,
        };
        use friend_service::concert_authority::{
            proto as friend_proto, OperationBudget as FriendBudget,
            PgOwnerAuthority as FriendOwner, RpcSettlementResolver as FriendResolver,
        };
        use std::time::{Duration, Instant};
        // Exactly the SAME running new Root endpoint used by the actual owner
        // RPC servers above, with each owner's own mTLS identity and credential.
        let auth_resolver = AuthResolver::new(
            &environment::tls(Workload::Auth)?,
            "auth-fixture",
            &[(auth_proto::Producer::Concert, self.root_url.clone())],
        )?;
        let friend_resolver = FriendResolver::new(
            &environment::tls(Workload::Friend)?,
            "friend-fixture",
            &[(friend_proto::Producer::Concert, self.root_url.clone())],
        )?;
        let auth = AuthOwner::new(self.auth.clone(), Arc::new(auth_resolver))?;
        let friend = FriendOwner::new(self.friend.clone(), Arc::new(friend_resolver))?;
        auth.recover(&AuthBudget::new(Instant::now() + Duration::from_secs(2))?)
            .await?;
        friend
            .recover(&FriendBudget::new(Instant::now() + Duration::from_secs(2))?)
            .await?;
        Ok(())
    }
    pub async fn seed(&self) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
        let (actor, family, subjects) = environment::identities(&self.auth, 1).await?;
        let subject = subjects[0];
        // Setup DATA only. No lifecycle, ACK, prepared-XID or delivery writes.
        sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Public native','OPEN') on conflict(event_id) do update set phase='OPEN',revision=concert_events.revision+1")
            .bind(self.event).bind(format!("native-{}",self.event.simple())).execute(&self.concert).await?;
        for user in [actor, subject] {
            sqlx::query("insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')")
                .bind(self.event).bind(user).execute(&self.concert).await?;
            sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)")
                .bind(self.event).bind(user).execute(&self.concert).await?;
        }
        sqlx::query("insert into concert_avatar_settings(event_id,user_id,contract_version,representation,preset_id,manifest_id,manifest_version) values($1,$2,'1.1.0','whole_character','photo01_front_01','mcr_selected_static_20261103',1)")
            .bind(self.event).bind(subject).execute(&self.concert).await?;
        sqlx::query("insert into profiles(user_id,username,nickname,avatar_url,bio,account_visibility) values($1,$2,'Current canonical name','private-url-canary','Current canonical bio','PUBLIC')")
            .bind(subject).bind(format!("native-{}",subject.simple())).execute(&self.profile).await?;
        Ok((actor, family, subject))
    }
}
