//! Actual owner enrollment, Profile RPC, source-bound product construction and
//! Concert SQL. No positive authority is fabricated by the release template.
//! Gateway HTTP, operator admission and visual acceptance remain separate.
use super::{capture, environment, owners};

use concert_service::adapters::out::relationship_authority::guarded::{
    CanonicalOwnerFence, ScopeRegistry,
};
use concert_service::adapters::out::{
    public_profile::PublicProfileReader, relationship_authority::guarded::ChildRecoveryReference,
};
use concert_service::product_rpc::{
    scope_ledger::{ChildIntentSettler, ChildRecoveryKey, ChildRecoveryKeys, PgScopeLedger},
    settlement::{ProducerSettlementService, POLICY},
};
use platform::{
    actor_delegation::{relationship_read::RelationshipReadVerifier, ActorDelegationVerifier},
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::{guard, guard_without_delegation},
};
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tonic::Status;
use uuid::Uuid;

struct Settle(Arc<PublicProfileReader>);
#[async_trait::async_trait]
impl ChildIntentSettler for Settle {
    async fn recover_one(&self, reference: &ChildRecoveryReference) -> Result<(), Status> {
        self.0.recover_one(reference).await
    }
}
pub(super) struct Fixture {
    pub(super) auth: PgPool,
    pub(super) friend: PgPool,
    pub(super) concert: PgPool,
    pub(super) profile: PgPool,
    profile_addr: std::net::SocketAddr,
    pub(super) reader: Arc<PublicProfileReader>,
    pub(super) ledger: PgScopeLedger,
    _registry: Arc<ScopeRegistry>,
    pub(super) fence: CanonicalOwnerFence,
    pub(super) family: Uuid,
    pub(super) event: Uuid,
    pub(super) instance: Uuid,
    pub(super) actor: Uuid,
    pub(super) subject: Uuid,
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
fn incoming(
    listener: tokio::net::TcpListener,
) -> impl futures::Stream<Item = Result<tokio::net::TcpStream, std::io::Error>> + Send + 'static {
    futures::stream::unfold(listener, |listener| async {
        let result = listener.accept().await.map(|(stream, _)| stream);
        Some((result, listener))
    })
}
fn spawn<F>(future: F) -> tokio::task::JoinHandle<()>
where
    F: std::future::Future<Output = Result<(), tonic::transport::Error>> + Send + 'static,
{
    tokio::spawn(async move { future.await.expect("actual native fixture server") })
}
/// Exact published operator candidate; no synthetic catalog or private reader
/// initializer. The production constructor validates source bytes itself.
fn product_release() -> anyhow::Result<Arc<concert_application::bootstrap::ValidatedProductRelease>>
{
    use concert_service::bootstrap::{BootstrapFeatures, ProductReleaseBundle};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap();
    let bundle = root.join("backend/releases/operator-bundle/quality-v6");
    let spatial = root.join("backend/releases/reference-scene-v2/quality-v6");
    let themes = [
        bundle.join("theme-revenge.json"),
        bundle.join("theme-parade.json"),
        bundle.join("theme-danger.json"),
    ];
    ProductReleaseBundle::from_source_bound_files(
        &bundle.join("bootstrap.json"),
        &root.join("contracts/fixtures/assets/selected-static/avatar-manifest.json"),
        &spatial.join("map-manifest.json"),
        [&themes[0], &themes[1], &themes[2]],
        BootstrapFeatures::default(),
        &root.join("frontend/public"),
        &spatial.join("source-binding.json"),
    )
    .map_err(|_| anyhow::anyhow!("Actual source-bound public Profile release unavailable"))
}
async fn profile_server(
    profile: PgPool,
    address: std::net::SocketAddr,
) -> anyhow::Result<(std::net::SocketAddr, tokio::task::JoinHandle<()>)> {
    use profile_service::adapters::r#in::grpc::concert_public_profile::{
        proto, PublicProfileServer, PUBLIC_PROFILE_POLICY,
    };
    let profile_service = guard(
        &PUBLIC_PROFILE_POLICY,
        proto::concert_public_profile_service_server::ConcertPublicProfileServiceServer::new(
            PublicProfileServer::from_pool(profile),
        ),
        CallerRegistry::new().register(Workload::Concert, "concert-fixture")?,
        RelationshipReadVerifier::new(
            Workload::Profile,
            &[owners::signer()?.public_key()],
            &ActorDelegationVerifier::new(Workload::Profile, &[capture::gateway()?.public_key()])?,
        )?
        .into_inner(),
    )?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    let address = listener.local_addr()?;
    let profile_task = spawn(
        tonic::transport::Server::builder()
            .tls_config(environment::tls(Workload::Profile)?.server_config()?)?
            .layer(CallDeadlineLayer)
            .add_service(profile_service)
            .serve_with_incoming(incoming(listener)),
    );

    Ok((address, profile_task))
}
impl Fixture {
    pub(super) async fn stop_profile(&mut self) {
        self.tasks[0].abort();
        let _ = (&mut self.tasks[0]).await;
    }
    pub(super) async fn restart_profile(&mut self) -> anyhow::Result<()> {
        let (_, task) = profile_server(self.profile.clone(), self.profile_addr).await?;
        self.tasks[0] = task;
        Ok(())
    }
    pub(super) async fn new() -> anyhow::Result<Self> {
        let auth = environment::pool("auth").await?;
        let friend = environment::pool("friend").await?;
        let concert = environment::pool("concert").await?;
        let profile = environment::pool("profile").await?;
        let (actor, family, subjects) = environment::identities(&auth, 1).await?;
        let subject = subjects[0];
        let release = product_release()?;
        let document: Value = serde_json::from_slice(release.release().bootstrap_json())?;
        let event = Uuid::parse_str(
            document["event"]["event_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Release event missing"))?,
        )?;

        // Actual database fixtures, never an identity-only positive guard.
        sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Public native','OPEN') on conflict(event_id) do update set phase='OPEN',revision=concert_events.revision+1")
            .bind(event).bind(format!("native-{}", event.simple())).execute(&concert).await?;
        for user in [actor, subject] {
            sqlx::query("insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')")
                .bind(event).bind(user).execute(&concert).await?;
            sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)")
                .bind(event).bind(user).execute(&concert).await?;
        }
        for user in [actor, subject] {
            sqlx::query("insert into concert_avatar_settings(event_id,user_id,contract_version,representation,preset_id,manifest_id,manifest_version) values($1,$2,'1.1.0','whole_character','photo01_front_01','mcr_selected_static_20261103',1)").bind(event).bind(user).execute(&concert).await?;
            sqlx::query("insert into profiles(user_id,username,nickname,avatar_url,bio,account_visibility) values($1,$2,'Canonical fixture name','private-avatar-canary','Canonical fixture bio','PUBLIC')").bind(user).bind(format!("intro-{}",user.simple())).execute(&profile).await?;
        }
        sqlx::query("insert into concert_meeting_settings(event_id,event_cutoff) values($1,clock_timestamp()+interval '1 day') on conflict(event_id) do nothing").bind(event).execute(&concert).await?;

        let (profile_addr, profile_task) =
            profile_server(profile.clone(), "127.0.0.1:0".parse()?).await?;
        let profile_url = format!("https://{profile_addr}");
        let reader = Arc::new(PublicProfileReader::connect(
            concert.clone(),
            release,
            &profile_url,
            "concert-fixture",
            Some(&environment::tls(Workload::Concert)?),
            owners::signer()?,
        )?);
        let instance = Uuid::new_v4();
        let keys = Arc::new(ChildRecoveryKeys::new(
            ChildRecoveryKey::new("public-native-v1", &[89; 32])?,
            vec![],
        )?);
        let ledger = PgScopeLedger::new(
            concert.clone(),
            instance,
            keys,
            Arc::new(Settle(reader.clone())),
        )?;
        let registry = ScopeRegistry::new(instance, Arc::new(ledger.clone()))?;
        let root_service = guard_without_delegation(
            &POLICY,
            ProducerSettlementService::new(
                ledger.clone(),
                Arc::new(concert_service::intro_live::IntroNativeResolver::new(
                    concert.clone(),
                    instance,
                )?),
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
        let running_auth = owners::auth(auth.clone(), &root_url).await?;
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
            profile_addr,
            reader,
            ledger,
            _registry: registry,
            fence,
            family,
            event,
            instance,
            actor,
            subject,
            _auth: running_auth,
            _friend: running_friend,
            tasks: vec![profile_task, root_task],
        })
    }
    pub(super) async fn assert_clean(&self) -> anyhow::Result<()> {
        for pool in [&self.auth, &self.friend, &self.concert, &self.profile] {
            let count: i64 = sqlx::query_scalar(
                "select count(*) from pg_prepared_xacts where database=current_database()",
            )
            .fetch_one(pool)
            .await?;
            assert_eq!(count, 0, "Actual children/owner scopes must settle");
        }
        Ok(())
    }
}
