//! Composition of actual guarded Auth, Friend, Profile and Concert owners.
//! Adapted from auth_bridge.rs at aa33fa7 and recipient/profile producer fixtures.
//! Synthetic avatar metadata is test-only; remote publication qualification is absent.
use super::*;
use crate::adapters::out::relationship_authority::world_auth::GrpcWorldAuthReader;
use crate::world::{
    ingress::{decode, AdmissionIntent, ClientMessage},
    interest::Facing,
    membership::MembershipAdapter,
};
use crate::world::{
    interest::PublicIdentity,
    membership::{CanonicalAuthorityPort, ClockAnchor},
};
use crate::{
    adapters::r#in::world_ws::AdmissionPort,
    bootstrap::BootstrapService,
    world::{
        admission_bridge::AdmissionBridge, production_owners::ProductionOwners,
        simulation::ResourcePin, PgWorldRepository,
    },
};
use chrono::{DateTime, TimeDelta, Utc};
use concert_application::world_ticket::*;
use concert_application::{
    bootstrap::{BootstrapDefaults, BootstrapRelease, ValidatedProductRelease},
    identity::SessionId,
};
use concert_domain::EventId;
use concert_domain::{AssetId, AuthUserId, AvatarSelection, ParticipantKey, Revision};
use platform::{
    actor_delegation::{
        relationship_read::{RelationshipReadSigner, RelationshipReadVerifier},
        ActorDelegationSigner, ActorDelegationVerifier,
    },
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, Workload},
};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};
use std::{os::unix::fs::MetadataExt, path::PathBuf, str::FromStr};
use tokio::sync::oneshot;

#[path = "owned.rs"]
mod owned;
fn gateway(audience: Workload) -> ActorDelegationVerifier {
    ActorDelegationVerifier::new(
        audience,
        &[
            ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32])
                .unwrap()
                .public_key(),
        ],
    )
    .unwrap()
}
fn signer() -> RelationshipReadSigner {
    RelationshipReadSigner::from_seed(&[37; 32], &gateway(Workload::Concert)).unwrap()
}
struct Running {
    endpoint: String,
    stop: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}
impl Running {
    async fn finish(mut self) {
        let _ = self.stop.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(2), &mut self.task)
            .await
            .unwrap()
            .unwrap();
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.task.abort();
    }
}
async fn server(pool: PgPool) -> Running {
    use auth_service::adapters::r#in::grpc::concert_relationship::{
        proto, RelationshipServer, POLICY,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let verifier = RelationshipReadVerifier::new(
        Workload::Auth,
        &[signer().public_key()],
        &gateway(Workload::Auth),
    )
    .unwrap()
    .into_inner();
    let guarded = platform::grpc_method_policy::guard(
        &POLICY,
        proto::auth_concert_relationship_service_server::AuthConcertRelationshipServiceServer::new(
            RelationshipServer::from_pool(pool),
        ),
        CallerRegistry::new()
            .register(Workload::Concert, "world-auth-fixture-concert")
            .unwrap(),
        verifier,
    )
    .unwrap();
    let (stop, rx) = oneshot::channel();
    let incoming = futures::stream::unfold(listener, |listener| async move {
        Some((listener.accept().await.map(|(s, _)| s), listener))
    });
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .layer(CallDeadlineLayer)
            .add_service(guarded)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = rx.await;
            })
            .await
            .unwrap();
    });
    Running {
        endpoint,
        stop: Some(stop),
        task,
    }
}
fn asset(s: &str) -> AssetId {
    AssetId::new(s.into()).unwrap()
}
fn pins() -> ResourcePin {
    ResourcePin {
        map_id: asset("map_fixture"),
        map_revision: Revision::initial(),
        avatar_manifest_id: asset("avatar_fixture"),
        avatar_manifest_version: Revision::initial(),
    }
}
fn identity() -> PublicIdentity {
    PublicIdentity {
        display_name: "Synthetic Profile".into(),
        avatar_revision: Revision::initial(),
        avatar: AvatarSelection {
            manifest_id: asset("avatar_fixture"),
            manifest_version: Revision::initial(),
            appearance: concert_domain::AvatarAppearance::Layered {
                body: asset("body_fixture"),
                eyes: asset("eyes_fixture"),
                hair: asset("hair_fixture"),
                outfit: asset("outfit_fixture"),
                accessory: None,
                skin_tone: asset("tone_fixture"),
            },
        },
        facing: Facing::Down,
        moving: false,
    }
}
async fn friend_server(pool: PgPool) -> Running {
    use friend_service::adapters::r#in::grpc::concert_relationship::{
        proto, RelationshipServer, POLICY,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let verifier = RelationshipReadVerifier::new(
        Workload::Friend,
        &[signer().public_key()],
        &gateway(Workload::Friend),
    )
    .unwrap()
    .into_inner();
    let guarded = platform::grpc_method_policy::guard(
        &POLICY,
        proto::friend_concert_relationship_service_server::FriendConcertRelationshipServiceServer::new(
            RelationshipServer::from_pool(pool),
        ),
        CallerRegistry::new()
            .register(Workload::Concert, "world-auth-fixture-concert")
            .unwrap(),
        verifier,
    )
    .unwrap();
    let (stop, rx) = oneshot::channel();
    let incoming = futures::stream::unfold(listener, |listener| async move {
        Some((listener.accept().await.map(|(s, _)| s), listener))
    });
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .layer(CallDeadlineLayer)
            .add_service(guarded)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = rx.await;
            })
            .await
            .unwrap();
    });
    Running {
        endpoint,
        stop: Some(stop),
        task,
    }
}
async fn profile_server(pool: PgPool) -> Running {
    use profile_service::adapters::r#in::grpc::world_identity::{
        proto, WorldProfileServer, WORLD_PROFILE_POLICY,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let verifier = RelationshipReadVerifier::new(
        Workload::Profile,
        &[signer().public_key()],
        &gateway(Workload::Profile),
    )
    .unwrap()
    .into_inner();
    let guarded = platform::grpc_method_policy::guard(
        &WORLD_PROFILE_POLICY,
        proto::concert_world_profile_service_server::ConcertWorldProfileServiceServer::new(
            WorldProfileServer::from_pool(pool),
        ),
        CallerRegistry::new()
            .register(Workload::Concert, "world-auth-fixture-concert")
            .unwrap(),
        verifier,
    )
    .unwrap();
    let (stop, rx) = oneshot::channel();
    let incoming = futures::stream::unfold(listener, |listener| async move {
        Some((listener.accept().await.map(|(s, _)| s), listener))
    });
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .layer(CallDeadlineLayer)
            .add_service(guarded)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = rx.await;
            })
            .await
            .unwrap();
    });
    Running {
        endpoint,
        stop: Some(stop),
        task,
    }
}

pub(crate) async fn ticket(
    repo: &PgWorldRepository,
    subject: &WorldSubject,
    room: Uuid,
    expiry: DateTime<Utc>,
    seed: u8,
) -> anyhow::Result<AdmissionIntent> {
    let ticket = AdmissionTicket::from_entropy(&[seed; 32])?;
    let now = Utc::now();
    assert_eq!(
        repo.issue(TicketRecord {
            digest: ticket.digest(),
            subject: subject.clone(),
            pin: concert_application::world_ticket::ResourcePin {
                event_id: subject.event_id,
                room_id: RoomId::from_uuid(room),
                map: MapPin::new(asset("map_fixture"), Revision::initial()),
                avatar: AvatarPin::new(asset("avatar_fixture"), Revision::initial())
            },
            issued_at: now,
            expires_at: (now + TimeDelta::seconds(20)).min(expiry),
            access_expires_at: expiry
        })
        .await?,
        IssueOutcome::Issued
    );
    let raw=serde_json::json!({"protocolVersion":"1.0.0","type":"authenticate","ticket":ticket.as_str(),"eventId":subject.event_id.uuid(),"roomId":room,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1}).to_string();
    let ClientMessage::Authenticate(intent) = decode(raw.as_bytes()).unwrap() else {
        unreachable!()
    };
    Ok(intent)
}

pub(crate) struct Stack {
    auth: PgPool,
    pub(crate) concert: PgPool,
    friend: PgPool,
    profile: PgPool,
    auth_server: Running,
    friend_server: Running,
    profile_server: Running,
    pub(crate) event: EventId,
    pub(crate) subject: WorldSubject,
    pub(crate) room: Uuid,
    pub(crate) target: Uuid,
    pub(crate) owners: Arc<ProductionOwners>,
    pub(crate) bridge: Arc<AdmissionBridge>,
    pub(crate) anchor: ClockAnchor,
    started: Instant,
}
impl Stack {
    pub(crate) async fn new() -> anyhow::Result<Self> {
        let auth = owned::pool("auth").await?;
        let concert = owned::pool("concert").await?;
        let friend_pool = owned::pool("friend").await?;
        let profile_pool = owned::pool("profile").await?;
        let event = EventId::from_uuid(Uuid::new_v4());
        let user = AuthUserId::from_uuid(Uuid::new_v4());
        let family = Uuid::new_v4();
        let subject = WorldSubject {
            key: ParticipantKey {
                event_id: event,
                user_id: user,
            },
            event_id: event,
            session_id: SessionId::new(family.to_string())?,
        };
        let room = Uuid::new_v4();
        let target = Uuid::new_v4();
        sqlx::query("insert into users(id,primary_email,password_hash,status) values($1,$2,'fixture-only','ACTIVE')").bind(user.uuid()).bind(format!("{family}@auth-bridge.invalid")).execute(&auth).await?;
        sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values($1,'fixture-only',$2,$3,clock_timestamp()+interval '1 hour')").bind(Uuid::new_v4().to_string()).bind(user.uuid()).bind(family).execute(&auth).await?;
        sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Bridge Auth fixture','OPEN')").bind(event.uuid()).bind(format!("bridge-{}",event.uuid().simple())).execute(&concert).await?;
        sqlx::query(
            "insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')",
        )
        .bind(event.uuid())
        .bind(user.uuid())
        .execute(&concert)
        .await?;
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(event.uuid()).bind(user.uuid()).execute(&concert).await?;
        sqlx::query("insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,is_default) values($1,$2,'First',2,'map_fixture',1,'avatar_fixture',1,true),($1,$3,'Second',2,'map_fixture',1,'avatar_fixture',1,false)").bind(event.uuid()).bind(room).bind(target).execute(&concert).await?;
        sqlx::query("insert into profiles(user_id,username,nickname,bio,avatar_url,account_visibility) values($1,$2,'Native Owner','private-profile-canary','https://private.invalid','PRIVATE')").bind(user.uuid()).bind(format!("private-{family}")).execute(&profile_pool).await?;
        sqlx::query("insert into concert_avatar_settings(event_id,user_id,contract_version,manifest_id,manifest_version,body_id,eyes_id,hair_id,outfit_id,skin_tone_id) values($1,$2,'1.0.0','avatar_fixture',1,'body_fixture','eyes_fixture','hair_fixture','outfit_fixture','tone_fixture')").bind(event.uuid()).bind(user.uuid()).execute(&concert).await?;
        let release = Arc::new(
            ValidatedProductRelease::from_validated(
                BootstrapRelease::new(
                    BootstrapDefaults {
                        theme: "REVENGE".parse()?,
                        avatar: identity().avatar,
                    },
                    b"{}".to_vec(),
                )
                .unwrap(),
                "a".repeat(64),
            )
            .unwrap(),
        );
        let server = server(auth.clone()).await;
        let friend_server = friend_server(friend_pool.clone()).await;
        let profile_server = profile_server(profile_pool.clone()).await;
        let reader = Arc::new(GrpcWorldAuthReader::connect(
            &server.endpoint,
            "world-auth-fixture-concert",
            None,
            signer(),
        )?);
        let owners = ProductionOwners::new(
        concert.clone(),
        crate::adapters::out::world_profile::GrpcWorldProfileReader::connect(
            &profile_server.endpoint,
            "world-auth-fixture-concert",
            None,
            signer(),
        )?,
        crate::world::recipient_owners::WorldRecipientReader::new(
            concert.clone(),
            crate::adapters::out::relationship_authority::grpc::GrpcRelationshipAuthority::connect(
                &server.endpoint,
                &friend_server.endpoint,
                "world-auth-fixture-concert",
                None,
                signer(),
            )?,
        ),
        concert_domain::avatar::AvatarCatalog::from_validated_manifest(&serde_json::json!({
            "contract_version":"1.0.0","kind":"avatar_manifest","manifest_id":"avatar_fixture","manifest_version":1,
            "layer_order":["body","eyes","hair","outfit","accessory"],
            "atlas":{"items":[{"item_id":"body_fixture","layer_id":"body"},{"item_id":"eyes_fixture","layer_id":"eyes"},{"item_id":"hair_fixture","layer_id":"hair"},{"item_id":"outfit_fixture","layer_id":"outfit"},{"item_id":"accessory_fixture","layer_id":"accessory"}]},
            "skin_tones":[{"skin_tone_id":"tone_fixture"}]
        })).map_err(|_| anyhow::anyhow!("synthetic catalog invalid"))?,
    );
        let anchor = ClockAnchor {
            monotonic_ms: 0,
            utc: Utc::now(),
        };
        let started = Instant::now();
        let bridge = AdmissionBridge::new_production(
            concert.clone(),
            Arc::new(BootstrapService::new(concert.clone(), release)),
            owners.clone(),
            reader,
            OwnerIdentity::new(Uuid::new_v4().to_string(), Uuid::new_v4())?,
            event,
            BTreeMap::from([(room, Uuid::from_u128(6)), (target, Uuid::from_u128(7))]),
            pins(),
            anchor.clone(),
            started,
        )?;
        bridge.claim_rooms().await?;
        Ok(Self {
            auth,
            concert,
            friend: friend_pool,
            profile: profile_pool,
            auth_server: server,
            friend_server,
            profile_server,
            event,
            subject,
            room,
            target,
            owners,
            bridge,
            anchor,
            started,
        })
    }
    pub(crate) fn repo(&self) -> PgWorldRepository {
        PgWorldRepository::new(self.concert.clone())
    }
    pub(crate) fn now(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }
    pub(crate) async fn finish(self) -> anyhow::Result<()> {
        self.owners.pause().await?;
        drop(self.bridge);
        drop(self.owners);
        self.auth_server.finish().await;
        self.friend_server.finish().await;
        self.profile_server.finish().await;
        self.auth.close().await;
        self.concert.close().await;
        self.friend.close().await;
        self.profile.close().await;
        Ok(())
    }
}

use crate::event_lifecycle::{
    Applied, AuthorityFence, AuthorityQuery, EffectKey, FencedLifecycleAuthority, LifecycleOwners,
    PgEventLifecycle,
};
use async_trait::async_trait;
use concert_application::{
    commands::*,
    event_lifecycle::{Actor, CloseCommand},
};
use concert_domain::event_lifecycle::{Retention, Step};
use sqlx::PgConnection;
pub(crate) struct OperatorFixture;
#[async_trait]
impl FencedLifecycleAuthority for OperatorFixture {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        q: AuthorityQuery,
        now: DateTime<Utc>,
    ) -> Result<AuthorityFence, CommandError> {
        let present: bool = sqlx::query_scalar(
            "select exists(select 1 from concert_participants where event_id=$1 and user_id=$2)",
        )
        .bind(q.actor.event)
        .bind(q.actor.actor)
        .fetch_one(c)
        .await
        .map_err(|_| CommandError::Unavailable)?;
        if !present {
            return Err(CommandError::Denied);
        }
        // Operator-role authentication is the missing fixture half; the World
        // component, four source services, SQL effect and native permit are real.
        Ok(AuthorityFence {
            query: q,
            auth_revision: 1,
            role_revision: 1,
            policy_revision: 1,
            checked_at: now,
            expires_at: now + TimeDelta::seconds(5),
        })
    }
    async fn retention_in(
        &self,
        _: &mut PgConnection,
        _: Actor,
        at: DateTime<Utc>,
    ) -> Result<Retention, CommandError> {
        Ok(Retention {
            policy_revision: 1,
            closed_at: at,
            cleanup_after: at + TimeDelta::days(30),
        })
    }
}
struct WorldComponent {
    fail_after: bool,
}
#[async_trait]
impl LifecycleOwners for WorldComponent {
    async fn apply_in(
        &self,
        c: &mut PgConnection,
        key: EffectKey,
        _: Retention,
    ) -> Result<Applied, CommandError> {
        if key.step != Step::ReadOnly {
            return Err(CommandError::Unavailable);
        }
        let r = fence_event_read_only_in(c, key.event, key.job, key.operation_id()).await?;
        if self.fail_after {
            return Err(CommandError::Unavailable);
        }
        Ok(Applied {
            key,
            receipt: r.receipt_id,
            revision: r.revision,
        })
    }
}
pub(crate) fn keys() -> FingerprintKeys {
    FingerprintKeys::new(
        SigningKey::new("world-read-only-fixture", &[93; 32]).unwrap(),
        vec![],
    )
    .unwrap()
}
pub(crate) fn start() -> CloseCommand {
    CloseCommand::Start {
        expected_revision: 1,
        reason: "Owned World component validation".into(),
    }
}
pub(crate) fn actor(s: &Stack) -> Actor {
    Actor {
        actor: s.subject.key.user_id.uuid(),
        event: s.event.uuid(),
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires marker-owned four-owner World READ_ONLY controller"]
async fn native_component_rollback_replay_and_owner_capture() -> anyhow::Result<()> {
    Box::pin(component()).await
}
async fn component() -> anyhow::Result<()> {
    let s = Stack::new().await?;
    let repo = s.repo();
    let intent = ticket(
        &repo,
        &s.subject,
        s.room,
        s.anchor.utc + TimeDelta::seconds(60),
        121,
    )
    .await?;
    let old = s
        .bridge
        .admit(intent, Uuid::new_v4())
        .await
        .map_err(|_| anyhow::anyhow!("qualified admission failed"))?;
    CanonicalAuthorityPort::install_pending(&s.bridge, &old, s.now()).unwrap();
    let spare = ticket(
        &repo,
        &s.subject,
        s.target,
        s.anchor.utc + TimeDelta::seconds(60),
        122,
    )
    .await?;
    assert!(CanonicalAuthorityPort::publication_ready(&s.bridge, &old));
    let operation_marker = Uuid::new_v4();
    s.bridge.begin_invalidation(
        operation_marker,
        crate::world::invalidation::Removal::Scope(crate::world::invalidation::Scope::Event(
            s.event.uuid(),
        )),
    )?;
    s.owners.pause().await?;
    assert!(!CanonicalAuthorityPort::publication_ready(&s.bridge, &old));
    let key = Uuid::new_v4();
    let failed = PgEventLifecycle::new(
        s.concert.clone(),
        OperatorFixture,
        WorldComponent { fail_after: true },
    );
    assert!(matches!(
        failed.command(actor(&s), key, start(), &keys()).await,
        Err(CommandError::Unavailable)
    ));
    let phase: String = sqlx::query_scalar("select phase from concert_events where event_id=$1")
        .bind(s.event.uuid())
        .fetch_one(&s.concert)
        .await?;
    assert_eq!(phase, "OPEN");
    let live: i64 =
        sqlx::query_scalar("select count(*) from concert_world_owner_leases where event_id=$1")
            .bind(s.event.uuid())
            .fetch_one(&s.concert)
            .await?;
    assert_eq!(live, 2);
    let captures: i64 = sqlx::query_scalar(
        "select count(*) from concert_world_read_only_receipts where event_id=$1",
    )
    .bind(s.event.uuid())
    .fetch_one(&s.concert)
    .await?;
    assert_eq!(captures, 0);
    let tickets: i64 =
        sqlx::query_scalar("select count(*) from concert_world_tickets where event_id=$1")
            .bind(s.event.uuid())
            .fetch_one(&s.concert)
            .await?;
    assert_eq!(
        tickets, 2,
        "rollback restores both consumed and spare tickets"
    );
    assert!(
        CanonicalAuthorityPort::current(&s.bridge, &old, s.now()).is_err(),
        "rollback never restores old cached authority"
    );
    assert!(
        s.bridge.admit(spare, Uuid::new_v4()).await.is_err(),
        "pending removal blocks late publication after rollback"
    );
    s.bridge.cleanup().await?;
    // Recovery is explicit and restores no cached grant. A new real four-owner
    // admission is still required after the committed phase is re-read as OPEN.
    s.bridge.finish_invalidation(operation_marker)?;
    let fresh = ticket(
        &repo,
        &s.subject,
        s.room,
        s.anchor.utc + TimeDelta::seconds(60),
        123,
    )
    .await?;
    let fresh = s
        .bridge
        .admit(fresh, Uuid::new_v4())
        .await
        .map_err(|_| anyhow::anyhow!("fresh qualified recovery failed"))?;
    CanonicalAuthorityPort::install_pending(&s.bridge, &fresh, s.now()).unwrap();
    assert!(CanonicalAuthorityPort::publication_ready(&s.bridge, &fresh));
    assert!(CanonicalAuthorityPort::current(&s.bridge, &old, s.now()).is_err());
    s.owners.pause().await?;
    let service = PgEventLifecycle::new(
        s.concert.clone(),
        OperatorFixture,
        WorldComponent { fail_after: false },
    );
    let receipt = service.command(actor(&s), key, start(), &keys()).await?;
    assert_eq!(receipt.state, ReceiptState::Succeeded);
    let job = receipt.reference.unwrap().resource;
    let effect = EffectKey {
        event: s.event.uuid(),
        job,
        step: Step::ReadOnly,
    };
    let mut tx = s.concert.begin().await?;
    let first = fence_event_read_only_in(&mut tx, effect.event, job, effect.operation_id()).await?;
    let again = fence_event_read_only_in(&mut tx, effect.event, job, effect.operation_id()).await?;
    assert_eq!(first, again);
    tx.rollback().await?;
    let mut autocommit = s.concert.acquire().await?;
    assert!(matches!(
        fence_event_read_only_in(&mut autocommit, effect.event, job, effect.operation_id()).await,
        Err(CommandError::InvalidInput)
    ));
    drop(autocommit);
    let capture: serde_json::Value =
        sqlx::query_scalar("select owners from concert_world_read_only_receipts where job_id=$1")
            .bind(job)
            .fetch_one(&s.concert)
            .await?;
    let (_, process, epoch, _) = s.bridge.invalidation_context();
    assert!(capture
        .as_array()
        .unwrap()
        .iter()
        .all(|o| o["owner"] == process.to_string() && o["epoch"] == epoch.to_string()));
    assert_eq!(capture.as_array().unwrap().len(), 2);
    let active: i64 =
        sqlx::query_scalar("select count(*) from concert_world_owner_leases where event_id=$1")
            .bind(s.event.uuid())
            .fetch_one(&s.concert)
            .await?;
    assert_eq!(active, 0);
    let outbox = crate::adapters::out::repository::outbox::PgOutbox::new(s.concert.clone());
    let d = outbox
        .claim(100)
        .await?
        .into_iter()
        .find(|d| d.receipt == Some(receipt.id))
        .unwrap();
    let ack = crate::world::invalidation::WorldInvalidationConsumer::new(s.bridge.clone())
        .apply(&d)
        .await
        .map_err(|e| anyhow::anyhow!("captured closure owner binding {e:?}"))?;
    assert!(!ack.duplicate);
    assert!(CanonicalAuthorityPort::current(&s.bridge, &fresh, s.now()).is_err());
    let queue: String = sqlx::query_scalar("select state from concert_outbox where outbox_id=$1")
        .bind(d.id)
        .fetch_one(&s.concert)
        .await?;
    assert_eq!(
        queue, "LEASED",
        "local World application is not global fanout ack"
    );
    s.finish().await
}

use crate::adapters::r#in::world_ws::{router, SocketTransport, WorldSocketState};
use crate::world::{
    ingress::{OriginPolicy, SafeClose},
    runtime::{TransportPort, WorldRuntime, WriteError},
    simulation::Binding,
};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Mutex,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
};
#[path = "geometry.rs"]
pub(crate) mod geometry;
#[derive(Clone)]
pub(crate) struct Trace {
    pub(crate) native: SocketTransport,
    pub(crate) paused: Arc<AtomicBool>,
    pub(crate) waits: Arc<AtomicUsize>,
    pub(crate) sent: Arc<Mutex<Vec<(Binding, Value, Instant)>>>,
    pub(crate) hold_user: Arc<Mutex<Option<Uuid>>>,
    pub(crate) held: Arc<AtomicBool>,
    pub(crate) release: Arc<AtomicBool>,
}
impl TransportPort for Trace {
    fn installed(&mut self, b: &Binding, n: u64) -> Result<(), WriteError> {
        self.native.installed(b, n)
    }
    fn ready(&self, b: &Binding) -> bool {
        if self.paused.load(Ordering::Acquire) {
            self.waits.fetch_add(1, Ordering::Relaxed);
            false
        } else {
            self.native.ready(b)
        }
    }
    fn send(
        &mut self,
        b: &Binding,
        d: crate::world::replication::Delivery,
    ) -> Result<(), WriteError> {
        let body = serde_json::from_slice(&d.bytes).unwrap();
        let hold = {
            let mut user = self.hold_user.lock().unwrap();
            if *user == Some(b.key.user_id.uuid()) {
                user.take();
                true
            } else {
                false
            }
        };
        if hold {
            self.held.store(true, Ordering::Release);
            let began = Instant::now();
            while !self.release.load(Ordering::Acquire) {
                assert!(
                    began.elapsed() < Duration::from_secs(1),
                    "native handoff hold not released"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        self.native.send(b, d)?;
        let mut sent = self.sent.lock().unwrap();
        assert!(sent.len() < 1000);
        sent.push((b.clone(), body, Instant::now()));
        Ok(())
    }
    fn close(&mut self, b: &Binding, r: SafeClose) {
        self.native.close(b, r)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires marker-owned four-owner READ_ONLY native controller"]
async fn native_real_publication_permit_and_socket_handoff_drain_before_start() -> anyhow::Result<()>
{
    Box::pin(native_drain()).await
}
async fn native_drain() -> anyhow::Result<()> {
    let s = Stack::new().await?;
    let transport = SocketTransport::default();
    let trace = Trace {
        native: transport.clone(),
        paused: Arc::new(AtomicBool::new(false)),
        waits: Arc::new(AtomicUsize::new(0)),
        sent: Arc::new(Mutex::new(vec![])),
        hold_user: Arc::new(Mutex::new(None)),
        held: Arc::new(AtomicBool::new(false)),
        release: Arc::new(AtomicBool::new(false)),
    };
    let (_, owner, _, _) = s.bridge.invalidation_context();
    let mut target = geometry::spec(s.target, 2);
    target.room_epoch = Uuid::from_u128(7);
    let runtime = WorldRuntime::new(
        s.event,
        owner,
        s.anchor.clone(),
        vec![geometry::spec(s.room, 2), target],
        s.bridge.clone(),
        s.bridge.clone(),
        trace.clone(),
    )
    .unwrap();
    let (started, origin) = runtime.clock_origin();
    let (handle, runtime_task) = runtime.spawn();
    let handle = Arc::new(handle);
    let state = WorldSocketState::new(
        OriginPolicy::new(vec!["http://localhost:9000".into()], true).unwrap(),
        transport.clone(),
        s.bridge.clone(),
        handle.clone(),
        started,
        origin,
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let (stop, stopped) = oneshot::channel();
    let server = tokio::spawn(async move {
        axum::serve(listener, router(state))
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
            .unwrap()
    });
    let mut request = format!("ws://{address}/world-ws").into_client_request()?;
    request
        .headers_mut()
        .insert("origin", "http://localhost:9000".parse()?);
    let (mut socket, _) = connect_async(request).await?;
    let intent = ticket(
        &s.repo(),
        &s.subject,
        s.room,
        s.anchor.utc + TimeDelta::seconds(60),
        124,
    )
    .await?;
    socket.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"authenticate","ticket":intent.ticket.expose_for_consumption(),"eventId":s.event.uuid(),"roomId":s.room,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1}).to_string())).await?;
    let first = loop {
        let msg = tokio::time::timeout(Duration::from_secs(2), socket.next())
            .await?
            .ok_or_else(|| anyhow::anyhow!("no initial snapshot"))??;
        if let Message::Text(raw) = msg {
            let v: Value = serde_json::from_str(&raw)?;
            if v["type"] == "snapshot" {
                break v;
            }
        }
    };
    let binding = trace.sent.lock().unwrap()[0].0.clone();
    assert!(CanonicalAuthorityPort::publication_ready(
        &s.bridge, &binding
    ));
    // Actual START waits on the actual qualified permit's event SHARE. This is
    // not a stand-in SHARE lock or a boolean successful owning component.
    let done = Arc::new(AtomicBool::new(false));
    let close = {
        let service = PgEventLifecycle::new(
            s.concert.clone(),
            OperatorFixture,
            WorldComponent { fail_after: false },
        );
        let actor = actor(&s);
        let done = done.clone();
        tokio::spawn(async move {
            let r = service
                .command(actor, Uuid::new_v4(), start(), &keys())
                .await;
            done.store(true, Ordering::Release);
            r
        })
    };
    let began = Instant::now();
    loop {
        let waiting:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like '%select phase,revision from concert_events%for update%')").fetch_one(&s.concert).await?;
        if waiting {
            break;
        }
        anyhow::ensure!(
            began.elapsed() < Duration::from_millis(700),
            "START never waited for actual publication permit"
        );
        tokio::time::sleep(Duration::from_millis(3)).await;
    }
    assert!(!done.load(Ordering::Acquire));
    assert!(CanonicalAuthorityPort::publication_ready(
        &s.bridge, &binding
    ));
    *trace.hold_user.lock().unwrap() = Some(s.subject.key.user_id.uuid());
    let paused = Arc::new(AtomicBool::new(false));
    let pausing = Arc::new(AtomicBool::new(false));
    let pause = {
        let held = trace.held.clone();
        let paused = paused.clone();
        let pausing = pausing.clone();
        let owners = s.owners.clone();
        let rt = tokio::runtime::Handle::current();
        std::thread::spawn(move || {
            let began = Instant::now();
            while !held.load(Ordering::Acquire) {
                assert!(began.elapsed() < Duration::from_secs(1));
                std::thread::sleep(Duration::from_millis(1));
            }
            pausing.store(true, Ordering::Release);
            let r = rt.block_on(owners.pause());
            paused.store(true, Ordering::Release);
            r
        })
    };
    let release = {
        let held = trace.held.clone();
        let release = trace.release.clone();
        let done = done.clone();
        std::thread::spawn(move || {
            let began = Instant::now();
            while !held.load(Ordering::Acquire) {
                assert!(began.elapsed() < Duration::from_secs(1));
                std::thread::sleep(Duration::from_millis(1));
            }
            std::thread::sleep(Duration::from_millis(60));
            let fenced = pausing.load(Ordering::Acquire)
                && !paused.load(Ordering::Acquire)
                && !done.load(Ordering::Acquire);
            release.store(true, Ordering::Release);
            fenced
        })
    };
    socket.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"input","streamId":first["streamId"],"roomEpoch":first["roomEpoch"],"seq":1,"axisX":1,"axisY":0,"buttons":[]}).to_string())).await?;
    tokio::task::spawn_blocking(move || pause.join().unwrap()).await??;
    assert!(
        release.join().unwrap(),
        "native disable and START cannot pass the in-flight actual socket handoff"
    );
    let receipt = close.await??;
    assert_eq!(receipt.state, ReceiptState::Succeeded);
    let committed = Instant::now();
    assert!(!CanonicalAuthorityPort::publication_ready(
        &s.bridge, &binding
    ));
    let before = trace.sent.lock().unwrap().len();
    tokio::time::sleep(Duration::from_millis(120)).await;
    assert!(trace
        .sent
        .lock()
        .unwrap()
        .iter()
        .skip(before)
        .all(|(_, v, at)| *at < committed
            || !matches!(v["type"].as_str(), Some("snapshot" | "delta"))));
    let owners: i64 =
        sqlx::query_scalar("select count(*) from concert_world_owner_leases where event_id=$1")
            .bind(s.event.uuid())
            .fetch_one(&s.concert)
            .await?;
    assert_eq!(owners, 0);
    assert!(
        s.bridge.maintain().await.is_err(),
        "direct keepalive/owner renewal deny committed READ_ONLY"
    );
    let _ = socket.close(None).await;
    transport.close_all(SafeClose::UNAVAILABLE);
    handle.request_stop();
    tokio::time::timeout(Duration::from_secs(2), runtime_task).await??;
    let _ = stop.send(());
    tokio::time::timeout(Duration::from_secs(2), server).await??;
    s.finish().await
}
