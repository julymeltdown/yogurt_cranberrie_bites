//! Copied World fixture from dd002448/48335f67; actual bounded current owners.
//! Replay uses separate actual mTLS endpoints; geometry/operator/art remain fixtures.
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
) -> anyhow::Result<AdmissionIntent> {
    let mut entropy = [0u8; 32];
    entropy[..16].copy_from_slice(Uuid::new_v4().as_bytes());
    entropy[16..].copy_from_slice(Uuid::new_v4().as_bytes());
    let ticket = AdmissionTicket::from_entropy(&entropy)?;
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
    pub(crate) auth: PgPool,
    pub(crate) concert: PgPool,
    pub(crate) friend: PgPool,
    pub(crate) profile: PgPool,
    auth_server: Running,
    friend_server: Running,
    profile_server: Running,
    pub(crate) event: EventId,
    pub(crate) subject: WorldSubject,
    pub(crate) room: Uuid,
    pub(crate) room_epoch: Uuid,
    pub(crate) target: Uuid,
    pub(crate) owners: Arc<ProductionOwners>,
    pub(crate) bridge: Arc<AdmissionBridge>,
    pub(crate) anchor: ClockAnchor,
    started: Instant,
}
impl Stack {
    pub(crate) async fn new() -> anyhow::Result<Self> {
        Self::with_owner(Uuid::new_v4()).await
    }
    pub(crate) async fn with_owner(consumer: Uuid) -> anyhow::Result<Self> {
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
        Self::compose(
            auth,
            concert,
            friend_pool,
            profile_pool,
            event,
            subject,
            room,
            target,
            consumer,
        )
        .await
    }
    pub(crate) async fn restore(
        consumer: Uuid,
        event: EventId,
        subject: WorldSubject,
        room: Uuid,
        target: Uuid,
    ) -> anyhow::Result<Self> {
        Self::compose(
            owned::pool("auth").await?,
            owned::pool("concert").await?,
            owned::pool("friend").await?,
            owned::pool("profile").await?,
            event,
            subject,
            room,
            target,
            consumer,
        )
        .await
    }
    async fn compose(
        auth: PgPool,
        concert: PgPool,
        friend_pool: PgPool,
        profile_pool: PgPool,
        event: EventId,
        subject: WorldSubject,
        room: Uuid,
        target: Uuid,
        consumer: Uuid,
    ) -> anyhow::Result<Self> {
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
        let room_epoch = Uuid::new_v4();
        let target_epoch = Uuid::new_v4();
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
            OwnerIdentity::new(consumer.to_string(), Uuid::new_v4())?,
            event,
            BTreeMap::from([(room, room_epoch), (target, target_epoch)]),
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
            room_epoch,
            target,
            owners,
            bridge,
            anchor,
            started,
        })
    }

    pub(crate) async fn add_user(&self) -> anyhow::Result<WorldSubject> {
        self.add_family(AuthUserId::from_uuid(Uuid::new_v4())).await
    }
    pub(crate) async fn add_family(&self, user: AuthUserId) -> anyhow::Result<WorldSubject> {
        let family = Uuid::new_v4();
        sqlx::query("insert into users(id,primary_email,password_hash,status) values($1,$2,'fixture-only','ACTIVE') on conflict(id) do nothing").bind(user.uuid()).bind(format!("{user:?}@remote.invalid")).execute(&self.auth).await?;
        sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values($1,'fixture-only',$2,$3,clock_timestamp()+interval '1 hour')").bind(Uuid::new_v4().to_string()).bind(user.uuid()).bind(family).execute(&self.auth).await?;
        sqlx::query("insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE') on conflict do nothing").bind(self.event.uuid()).bind(user.uuid()).execute(&self.concert).await?;
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false) on conflict do nothing").bind(self.event.uuid()).bind(user.uuid()).execute(&self.concert).await?;
        sqlx::query("insert into profiles(user_id,username,nickname,bio,avatar_url,account_visibility) values($1,$2,'Native Remote','private-profile-canary','https://private.invalid','PRIVATE') on conflict(user_id) do nothing").bind(user.uuid()).bind(format!("remote-{family}")).execute(&self.profile).await?;
        sqlx::query("insert into concert_avatar_settings(event_id,user_id,contract_version,manifest_id,manifest_version,body_id,eyes_id,hair_id,outfit_id,skin_tone_id) values($1,$2,'1.0.0','avatar_fixture',1,'body_fixture','eyes_fixture','hair_fixture','outfit_fixture','tone_fixture') on conflict do nothing").bind(self.event.uuid()).bind(user.uuid()).execute(&self.concert).await?;
        Ok(WorldSubject {
            key: ParticipantKey {
                event_id: self.event,
                user_id: user,
            },
            event_id: self.event,
            session_id: SessionId::new(family.to_string())?,
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
