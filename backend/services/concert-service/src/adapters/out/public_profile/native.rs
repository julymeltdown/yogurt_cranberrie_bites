//! Actual owner enrollment, Profile RPC, source-bound product construction and
//! Concert SQL. No positive authority is fabricated by the release template.
//! Gateway HTTP, operator admission and visual acceptance remain separate.
mod capture;
mod environment;
mod owners;

use super::*;
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
use std::time::{Duration, Instant};
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
    reader: Arc<PublicProfileReader>,
    ledger: PgScopeLedger,
    _registry: Arc<ScopeRegistry>,
    scope: Arc<OwnerGuardScope>,
    actor: Uuid,
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
/// Exact published operator candidate; no synthetic catalog or private reader
/// initializer. The production constructor validates source bytes itself.
fn product_release() -> anyhow::Result<Arc<concert_application::bootstrap::ValidatedProductRelease>>
{
    use crate::bootstrap::{BootstrapFeatures, ProductReleaseBundle};
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
impl Fixture {
    async fn new() -> anyhow::Result<Self> {
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
        let binding = capture::binding(actor, family, event).await?;
        // Actual database fixtures, never an identity-only positive guard.
        sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Public native','OPEN') on conflict(event_id) do update set phase='OPEN',revision=concert_events.revision+1")
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
            reader,
            ledger,
            _registry: registry,
            scope,
            actor,
            subject,
            _auth: running_auth,
            _friend: running_friend,
            tasks: vec![profile_task, root_task],
        })
    }
    async fn finish(&self) -> anyhow::Result<()> {
        self.scope.mark_serialized().await?;
        self.scope.finish_response().await?;
        self.assert_clean().await
    }
    async fn assert_clean(&self) -> anyhow::Result<()> {
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
async fn wait_blocked(pool: &PgPool) -> anyhow::Result<()> {
    let stop = Instant::now() + Duration::from_millis(750);
    loop {
        let blocked: bool = sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and 0=any(pg_blocking_pids(pid)))")
            .fetch_one(pool).await?;
        if blocked {
            return Ok(());
        }
        anyhow::ensure!(
            Instant::now() < stop,
            "Actual prepared child did not exclude writer"
        );
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}
#[tokio::test]
#[ignore = "requires frozen owned canonical four-database PG16/mTLS controller"]
async fn actual_owner_public_read_reuses_local_hold_through_serialization() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let viewer = f.scope.public_profile_subject(f.actor).await?;
    f.reader
        .policy(&viewer)
        .await
        .map_err(|e| anyhow::anyhow!("viewer policy: {e:?}"))?;
    let subject = f.scope.public_profile_subject(f.subject).await?;
    let first = f
        .reader
        .read(&subject)
        .await
        .map_err(|e| anyhow::anyhow!("public read: {e:?}"))?;
    let second = f
        .reader
        .read(&subject)
        .await
        .map_err(|e| anyhow::anyhow!("public reread: {e:?}"))?;
    assert_eq!(first.json, second.json);
    let body: Value = serde_json::from_slice(&first.json)?;
    assert_eq!(body.as_object().unwrap().len(), 4);
    assert_eq!(body["user_id"], f.subject.to_string());
    assert_eq!(body["display_name"], "Current canonical name");
    assert_eq!(body["bio"], "Current canonical bio");
    assert_eq!(body["avatar"]["config"]["preset_id"], "photo01_front_01");
    assert!(!String::from_utf8(first.json)?.contains("private-url-canary"));
    let intents: i64 = sqlx::query_scalar(
        "select count(*) from concert_product_child_intents where instance_id=$1",
    )
    .bind(f.ledger.instance())
    .fetch_one(&f.concert)
    .await?;
    assert_eq!(
        intents, 3,
        "Viewer local plus one shared author local/Profile child"
    );
    let subject_id = f.subject;
    let event = f.reader.event;
    let profile = f.profile.clone();
    let privacy = tokio::spawn(async move {
        sqlx::query("update profiles set account_visibility='PRIVATE' where user_id=$1")
            .bind(subject_id)
            .execute(&profile)
            .await
    });
    let concert = f.concert.clone();
    let avatar = tokio::spawn(async move {
        sqlx::query("update concert_avatar_settings set preset_id='photo01_front_02',revision=revision+1 where event_id=$1 and user_id=$2")
            .bind(event).bind(subject_id).execute(&concert).await
    });
    wait_blocked(&f.profile).await?;
    wait_blocked(&f.concert).await?;
    assert!(
        f.scope.finish_response().await.is_err(),
        "No serialization, no release"
    );
    f.scope.mark_serialized().await?;
    assert!(!privacy.is_finished() && !avatar.is_finished());
    subject.check().await?;
    f.scope.finish_response().await?;
    tokio::time::timeout(Duration::from_secs(1), privacy).await???;
    tokio::time::timeout(Duration::from_secs(1), avatar).await???;
    assert!(
        matches!(
            f.reader.read(&subject).await,
            Err(concert_domain::feed::FeedError::DependencyUnavailable)
        ),
        "Sealed root denial must fail the whole read, never become a skippable author"
    );
    f.assert_clean().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires frozen owned canonical four-database PG16/mTLS controller"]
async fn actual_local_denials_and_failed_profile_intent_settle_without_fallback(
) -> anyhow::Result<()> {
    for denial in [
        "missing-avatar",
        "missing-admission",
        "suspended",
        "missing-participant",
        "blocked-low",
        "blocked-high",
        "draft-event",
        "archived-event",
        "private-profile",
    ] {
        let f = Fixture::new().await?;
        let event = f.reader.event;
        match denial {
            "missing-avatar" => {
                sqlx::query("delete from concert_avatar_settings where event_id=$1 and user_id=$2")
                    .bind(event)
                    .bind(f.subject)
                    .execute(&f.concert)
                    .await?;
            }
            "missing-admission" => {
                sqlx::query(
                    "delete from concert_event_admissions where event_id=$1 and user_id=$2",
                )
                .bind(event)
                .bind(f.subject)
                .execute(&f.concert)
                .await?;
            }
            "draft-event" | "archived-event" => {
                sqlx::query(
                    "update concert_events set phase=$2,revision=revision+1 where event_id=$1",
                )
                .bind(event)
                .bind(if denial == "draft-event" {
                    "DRAFT"
                } else {
                    "ARCHIVED"
                })
                .execute(&f.concert)
                .await?;
            }
            "suspended" => {
                sqlx::query("update concert_event_admissions set state='SUSPENDED',restriction_revision=restriction_revision+1 where event_id=$1 and user_id=$2").bind(event).bind(f.subject).execute(&f.concert).await?;
            }
            "missing-participant" => {
                sqlx::query("delete from concert_participants where event_id=$1 and user_id=$2")
                    .bind(event)
                    .bind(f.subject)
                    .execute(&f.concert)
                    .await?;
            }
            "blocked-low" | "blocked-high" => {
                let (low, high) = if f.actor < f.subject {
                    (f.actor, f.subject)
                } else {
                    (f.subject, f.actor)
                };
                sqlx::query("insert into concert_relationship_pairs(event_id,low_user,high_user,low_blocks_high,high_blocks_low) values($1,$2,$3,$4,$5)").bind(event).bind(low).bind(high).bind(denial=="blocked-low").bind(denial=="blocked-high").execute(&f.concert).await?;
            }
            "private-profile" => {
                sqlx::query("update profiles set account_visibility='PRIVATE' where user_id=$1")
                    .bind(f.subject)
                    .execute(&f.profile)
                    .await?;
            }
            _ => unreachable!(),
        }
        let subject = f.scope.public_profile_subject(f.subject).await?;
        assert!(
            f.reader.read(&subject).await.is_err(),
            "{denial} cannot return a fallback author"
        );
        f.finish().await?;
    }
    Ok(())
}
