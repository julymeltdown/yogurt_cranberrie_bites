//! Real five-owner native Feed join. Adapted from PublicProfile db92f895
//! native.rs (source SHA-256 d3922a6dcc8c51f1b02aeeaba32d1e5f0736d75d1aa57e0bc9e0608288bb34b3).
//! Uses the public production release constructor, never its private art bypass.
//! Scope finish is component evidence only; Gateway HTTP delivery is not tested.
mod capture;
mod environment;
mod owners;

use super::{cursor::CursorCodec, live_read::FeedQuery, qualified_read::QualifiedReadFeed};
use crate::adapters::out::public_profile::PublicProfileReader;
use crate::adapters::out::relationship_authority::guarded::ChildRecoveryReference;
use crate::bootstrap::{BootstrapFeatures, ProductReleaseBundle};
use crate::product_rpc::root_read;
use platform::actor_delegation::concert_read::{
    ConcertPostReadSigner, ConcertPostReadVerifier, PostConcertRootReadSigner,
    PostConcertRootReadVerifier,
};
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use tonic::Status;

use crate::adapters::out::relationship_authority::guarded::{
    CanonicalOwnerFence, OwnerGuardScope, ScopeRegistry,
};
use crate::product_rpc::{
    scope_ledger::{
        ChildIntentSettler, ChildRecoveryKey, ChildRecoveryKeys, PgScopeLedger, ScopeKey,
    },
    settlement::{PgNativeAbortResolver, ProducerSettlementService, POLICY},
};
use platform::{
    actor_delegation::{relationship_read::RelationshipReadVerifier, ActorDelegationVerifier},
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::{guard, guard_without_delegation},
};
use uuid::Uuid;

struct Settle(Arc<PublicProfileReader>);
#[async_trait::async_trait]
impl ChildIntentSettler for Settle {
    async fn recover_one(&self, reference: &ChildRecoveryReference) -> Result<(), Status> {
        self.0.recover_one(reference).await
    }
}
struct Fixture {
    auth: PgPool,
    friend: PgPool,
    concert: PgPool,
    profile: PgPool,
    post: PgPool,
    post_url: String,
    reader: Arc<PublicProfileReader>,
    _registry: Arc<ScopeRegistry>,
    scope: Arc<OwnerGuardScope>,
    subject: Uuid,
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
impl Fixture {
    async fn new() -> anyhow::Result<Self> {
        let auth = environment::pool("auth").await?;
        let friend = environment::pool("friend").await?;
        let concert = environment::pool("concert").await?;
        let profile = environment::pool("profile").await?;
        let post = environment::pool("post").await?;
        let source = std::path::PathBuf::from(std::env::var("MCR_FEED_NATIVE_SOURCE_ROOT")?);
        anyhow::ensure!(
            source.is_absolute() && source.canonicalize()? == source,
            "Canonical frozen source required"
        );
        let bundle = source.join("backend/releases/operator-bundle/quality-v6");
        let release = ProductReleaseBundle::from_source_bound_files(
            &bundle.join("bootstrap.json"),
            &source.join("contracts/fixtures/assets/selected-static/avatar-manifest.json"),
            &source.join("backend/releases/reference-scene-v2/quality-v6/map-manifest.json"),
            [
                &bundle.join("theme-revenge.json"),
                &bundle.join("theme-parade.json"),
                &bundle.join("theme-danger.json"),
            ],
            BootstrapFeatures::default(),
            &source.join("frontend/public"),
            &source.join("backend/releases/reference-scene-v2/quality-v6/source-binding.json"),
        )
        .map_err(|_| anyhow::anyhow!("Actual source-bound release unavailable"))?;
        let document: Value = serde_json::from_slice(release.release().bootstrap_json())?;
        let event = Uuid::parse_str(
            document["event"]["event_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Release event required"))?,
        )?;
        let (actor, family, subjects) = environment::identities(&auth, 1).await?;
        let subject = subjects[0];
        let binding = capture::binding(actor, family, event).await?;
        let event = binding.event();
        // Actual database fixtures, never an identity-only positive guard.
        sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Public native','OPEN')")
            .bind(event).bind(format!("native-{}", event.simple())).execute(&concert).await?;
        for user in [actor, subject] {
            sqlx::query("insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')")
                .bind(event).bind(user).execute(&concert).await?;
            sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)")
                .bind(event).bind(user).execute(&concert).await?;
        }
        sqlx::query("insert into concert_avatar_settings(event_id,user_id,contract_version,representation,preset_id,manifest_id,manifest_version) values($1,$2,'1.1.0','whole_character','photo01_front_01','mcr_selected_static_20261103',1)")
            .bind(event).bind(subject).execute(&concert).await?;
        // The viewer intentionally has neither a public Profile nor an avatar.
        sqlx::query("insert into profiles(user_id,username,nickname,avatar_url,bio,account_visibility) values($1,$2,'Current canonical name','private-url-canary','Current canonical bio','PUBLIC')")
            .bind(subject).bind(format!("native-{}", subject.simple())).execute(&profile).await?;

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
                Arc::new(PgNativeAbortResolver::new(concert.clone(), instance)?),
            )
            .server(),
            CallerRegistry::new()
                .register(Workload::Auth, "auth-fixture")?
                .register(Workload::Friend, "friend-fixture")?,
        )?;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let root_url = format!("https://{}", listener.local_addr()?);
        let post_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let post_url = format!("https://{}", post_listener.local_addr()?);
        let running_auth = owners::auth(auth.clone(), &root_url, &post_url).await?;
        let running_friend = owners::friend(friend.clone(), &root_url, &post_url).await?;
        let gateway_concert =
            ActorDelegationVerifier::new(Workload::Concert, &[capture::gateway()?.public_key()])?;
        let gateway_post =
            ActorDelegationVerifier::new(Workload::Post, &[capture::gateway()?.public_key()])?;
        let post_signer = PostConcertRootReadSigner::from_seed(&[77; 32], &gateway_concert)?;
        let read_signer = ConcertPostReadSigner::from_seed(&[76; 32], &gateway_post)?;
        let callback = guard(
            &root_read::POLICY,
            root_read::ConcertRootReadService::new(registry.clone(), reader.clone()).server(),
            CallerRegistry::new().register(Workload::Post, "post-fixture")?,
            PostConcertRootReadVerifier::new(&[post_signer.public_key()], &gateway_concert)?
                .into_inner(),
        )?;
        let root_task = spawn(
            tonic::transport::Server::builder()
                .tls_config(environment::tls(Workload::Concert)?.server_config()?)?
                .layer(CallDeadlineLayer)
                .add_service(root_service)
                .add_service(callback)
                .serve_with_incoming(incoming(listener)),
        );
        let post_tls = environment::tls(Workload::Post)?;
        let runtime = post_service::concert_feed::read_runtime::QualifiedReadRuntime::connect(
            post.clone(),
            &running_auth.url,
            &running_friend.url,
            &root_url,
            "post-fixture",
            &post_tls,
            post_signer,
        )?;
        let (post_read, post_settlement) =
            post_service::adapters::r#in::grpc::concert_feed::qualified::services(
                runtime,
                CallerRegistry::new()
                    .register(Workload::Concert, "concert-fixture")?
                    .register(Workload::Auth, "auth-fixture")?
                    .register(Workload::Friend, "friend-fixture")?,
                ConcertPostReadVerifier::new(&[read_signer.public_key()], &gateway_post)?,
            )?;
        let post_task = spawn(
            tonic::transport::Server::builder()
                .tls_config(post_tls.server_config()?)?
                .layer(CallDeadlineLayer)
                .add_service(post_read)
                .add_service(post_settlement)
                .serve_with_incoming(incoming(post_listener)),
        );
        let fence = CanonicalOwnerFence::connect(
            &running_auth.url,
            &running_friend.url,
            "concert-fixture",
            &environment::tls(Workload::Concert)?,
            owners::signer()?,
            registry.clone(),
        )?;
        let scope = fence.acquire(binding).await?;
        let value = scope.producer_scope();
        ledger
            .bind_delivery(
                &ScopeKey {
                    instance,
                    scope: Uuid::parse_str(&value.scope_id)?,
                    binding_digest: value.binding_digest.as_slice().try_into()?,
                },
                &[90; 32],
            )
            .await?;
        Ok(Self {
            auth,
            friend,
            concert,
            profile,
            post,
            post_url,
            reader,
            _registry: registry,
            scope,
            subject,
            _auth: running_auth,
            _friend: running_friend,
            tasks: vec![profile_task, root_task, post_task],
        })
    }
    async fn finish(&self) -> anyhow::Result<()> {
        self.scope.mark_serialized().await?;
        self.scope.finish_response().await?;
        self.assert_clean().await
    }
    async fn assert_clean(&self) -> anyhow::Result<()> {
        for pool in [
            &self.auth,
            &self.friend,
            &self.concert,
            &self.profile,
            &self.post,
        ] {
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

struct WallClock;
impl concert_application::Clock for WallClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }
}
#[tokio::test]
#[ignore = "requires frozen owned five-database PG16/mTLS Feed join controller"]
async fn actual_post_reads_use_current_public_author_and_same_transaction_owner_tracking(
) -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let b = f.scope.binding();
    let post_id = Uuid::new_v4();
    sqlx::query("insert into post_concert_posts(post_id,event_id,author_id,content,revision,state,created_at) values($1,$2,$3,$4,1,'PUBLISHED',clock_timestamp()-interval '1 minute')")
        .bind(post_id).bind(b.event()).bind(f.subject)
        .bind(serde_json::json!({"text":"Actual Post owner content","links":[],"media_ids":[]}))
        .execute(&f.post).await?;
    let context = concert_application::feed::Context {
        actor: b.actor(),
        event: b.event(),
        session_family: b.session_family(),
        request: b.request_id(),
        authority_digest: b.parent_digest(),
        expires_at: chrono::DateTime::from_timestamp_micros(b.deadline().expires_micros())
            .ok_or_else(|| anyhow::anyhow!("Deadline required"))?,
    };
    let gateway =
        ActorDelegationVerifier::new(Workload::Post, &[capture::gateway()?.public_key()])?;
    let feed = QualifiedReadFeed::connect(
        &f.post_url,
        "concert-fixture",
        &environment::tls(Workload::Concert)?,
        ConcertPostReadSigner::from_seed(&[76; 32], &gateway)?,
        f.scope.clone(),
        f.reader.clone(),
        CursorCodec::new(&[79; 32])?,
        WallClock,
    )?;
    let body = f
        .scope
        .run(async {
            use concert_application::feed::{Action, FeedPolicy};
            super::policy::GuardedPolicy::new(f.scope.clone(), f.reader.clone())
                .map_err(|e| {
                    Status::unavailable(format!("native actor policy construction: {e:?}"))
                })?
                .observe(context, context.actor, Action::Read)
                .await
                .map_err(|e| {
                    Status::unavailable(format!("native actor policy observation: {e:?}"))
                })?;
            feed.page(context, FeedQuery::default())
                .await
                .map_err(|e| Status::unavailable(format!("native Feed read: {e:?}")))
        })
        .await?;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["items"][0]["post_id"], post_id.to_string());
    assert_eq!(body["items"][0]["author"]["user_id"], f.subject.to_string());
    assert_eq!(
        body["items"][0]["author"]["display_name"],
        "Current canonical name"
    );
    assert!(!body.to_string().contains("private-url-canary"));
    let tracked: i64 = sqlx::query_scalar("select count(*) from post_concert_read_tracking")
        .fetch_one(&f.post)
        .await?;
    assert!(
        tracked > 0,
        "Actual Post read must register its same transaction"
    );
    // A completed Post transaction and serialized JSON alone cannot release the owners.
    for pool in [&f.auth, &f.friend, &f.concert, &f.profile] {
        let count: i64 = sqlx::query_scalar(
            "select count(*) from pg_prepared_xacts where database=current_database()",
        )
        .fetch_one(pool)
        .await?;
        assert!(
            count > 0,
            "Actual owner/local/Profile exclusions remain held"
        );
    }
    f.finish().await?;
    Ok(())
}
