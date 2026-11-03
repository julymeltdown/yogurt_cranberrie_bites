//! Composition of actual guarded Auth, Friend, Profile and Concert owners.
//! Adapted from auth_bridge.rs at aa33fa7 and recipient/profile producer fixtures.
//! Synthetic avatar metadata is test-only; remote publication qualification is absent.
use super::*;
use crate::adapters::out::relationship_authority::world_auth::GrpcWorldAuthReader;
use crate::world::{
    ingress::{decode, ClientMessage},
    interest::Facing,
    membership::MembershipAdapter,
};
use concert_application::{
    bootstrap::{BootstrapDefaults, BootstrapRelease, ValidatedProductRelease},
    identity::SessionId,
};
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
use std::{os::unix::fs::MetadataExt, path::PathBuf, str::FromStr};
use tokio::sync::oneshot;

// Copied without weakening marker/inode/database checks from the qualified
// Auth producer native fixture at 30ca2a499edabb75720273a312d2a4196f18201d.
async fn pool(owner: &str) -> anyhow::Result<PgPool> {
    anyhow::ensure!(
        matches!(owner, "auth" | "concert" | "friend" | "profile"),
        "foreign owner refused"
    );
    let root = PathBuf::from(std::env::var("MCR_WORLD_COMPOSITION_ROOT")?);
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        root.is_absolute()
            && root.canonicalize()? == root
            && info.is_dir()
            && info.mode() & 0o077 == 0,
        "private owned fixture required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino())
            == std::env::var("MCR_WORLD_COMPOSITION_IDENTITY")?,
        "fixture inode changed"
    );
    let marker = root.join("mcr-owner.json");
    let mi = std::fs::symlink_metadata(&marker)?;
    anyhow::ensure!(
        mi.is_file()
            && !mi.file_type().is_symlink()
            && mi.mode() & 0o077 == 0
            && mi.uid() == info.uid(),
        "private marker required"
    );
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(marker)?)?;
    anyhow::ensure!(
        value["uid"].as_u64() == Some(info.uid() as u64)
            && value["token"].as_str()
                == Some(std::env::var("MCR_WORLD_COMPOSITION_TOKEN")?.as_str()),
        "fixture owner mismatch"
    );
    for name in ["data", "socket"] {
        let child = root.join(name);
        let ci = std::fs::symlink_metadata(&child)?;
        anyhow::ensure!(
            ci.is_dir() && !ci.file_type().is_symlink() && child.canonicalize()? == child,
            "foreign fixture child refused"
        );
    }
    let options = PgConnectOptions::from_str(&std::env::var(format!(
        "MCR_WORLD_COMPOSITION_{}_URL",
        owner.to_uppercase()
    ))?)?;
    let database = format!("mcr_{owner}_world_composition");
    anyhow::ensure!(
        options.get_database() == Some(database.as_str())
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_options().is_none()
            && options.get_port() == 5432,
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await?;
    let data: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        data == root.join("data").to_string_lossy(),
        "foreign data directory refused"
    );
    let identity: (String, String, String, i32) = sqlx::query_as(
        "select current_database(),current_user,current_setting('listen_addresses'),current_setting('server_version_num')::int"
    ).fetch_one(&pool).await?;
    anyhow::ensure!(
        identity == (database, "mcr_owner".into(), "".into(), 160014),
        "foreign server refused"
    );
    Ok(pool)
}

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
            body: asset("body_fixture"),
            eyes: asset("eyes_fixture"),
            hair: asset("hair_fixture"),
            outfit: asset("outfit_fixture"),
            accessory: None,
            skin_tone: asset("tone_fixture"),
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

async fn ticket(
    bridge: &Arc<AdmissionBridge>,
    subject: &WorldSubject,
    room: Uuid,
    expiry: DateTime<Utc>,
    seed: u8,
) -> anyhow::Result<AdmissionIntent> {
    let ticket = AdmissionTicket::from_entropy(&[seed; 32])?;
    let now = Utc::now();
    assert_eq!(
        bridge
            .repository
            .issue(TicketRecord {
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
async fn lease(pool: &PgPool, key: ParticipantKey) -> anyhow::Result<DateTime<Utc>> {
    Ok(sqlx::query_scalar(
        "select lease_until from concert_world_memberships where event_id=$1 and user_id=$2",
    )
    .bind(key.event_id.uuid())
    .bind(key.user_id.uuid())
    .fetch_one(pool)
    .await?)
}

#[tokio::test]
#[ignore = "requires marker-owned four-owner composition controller"]
async fn native_actual_owners_admit_switch_renew_and_revocation() -> anyhow::Result<()> {
    Box::pin(native_scenario()).await
}
async fn native_scenario() -> anyhow::Result<()> {
    let auth = pool("auth").await?;
    let concert = pool("concert").await?;
    let friend_pool = pool("friend").await?;
    let profile_pool = pool("profile").await?;
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
        BTreeMap::from([(room, Uuid::new_v4()), (target, Uuid::new_v4())]),
        pins(),
        anchor.clone(),
        started,
    )?;
    bridge.claim_rooms().await?;
    let expiry = anchor.utc + TimeDelta::seconds(60);
    let first = ticket(&bridge, &subject, room, expiry, 41).await?;
    assert!(bridge
        .repository
        .current_owned_membership(subject.key, Utc::now())
        .await?
        .is_none());
    let b = bridge
        .admit(first.clone(), Uuid::new_v4())
        .await
        .map_err(|_| anyhow::anyhow!("production Auth admission failed"))?;
    let mut adapter = MembershipAdapter::new(bridge.clone(), anchor.clone(), 50).unwrap();
    adapter.install(&b, bridge.now_ms()?).unwrap();
    let observed = CanonicalAuthorityPort::current(&bridge, &b, bridge.now_ms()?).unwrap();
    assert_eq!(observed.access_expires_at, expiry);
    assert_eq!(observed.identity.display_name, "Native Owner");
    assert!(CanonicalAuthorityPort::publication_ready(&bridge, &b));
    assert!(observed.lease_expires_at <= Utc::now() + TimeDelta::seconds(2));
    let auth_deadline = anchor.deadline(observed.lease_expires_at).unwrap();
    assert!(
        CanonicalAuthorityPort::current(&bridge, &b, auth_deadline).is_err(),
        "Auth window expiry is enforced without a heartbeat"
    );
    // Local revocation writes must block while the native-send permit is enabled.
    let mut competing = concert.begin().await?;
    sqlx::query("set local lock_timeout='40ms'")
        .execute(&mut *competing)
        .await?;
    let lock_error = sqlx::query("update concert_event_admissions set state='SUSPENDED',restriction_revision=restriction_revision+1 where event_id=$1 and user_id=$2").bind(event.uuid()).bind(user.uuid()).execute(&mut *competing).await.unwrap_err();
    assert!(
        matches!(lock_error,sqlx::Error::Database(ref e) if e.code().as_deref()==Some("55P03"))
    );
    competing.rollback().await?;
    owners.pause().await?;
    assert!(!CanonicalAuthorityPort::publication_ready(&bridge, &b));
    bridge.maintain().await?;
    assert!(CanonicalAuthorityPort::publication_ready(&bridge, &b));
    sqlx::query("update profiles set nickname='Current Owner' where user_id=$1")
        .bind(user.uuid())
        .execute(&profile_pool)
        .await?;
    bridge.maintain().await?;
    assert_eq!(
        CanonicalAuthorityPort::current(&bridge, &b, bridge.now_ms()?)
            .unwrap()
            .identity
            .display_name,
        "Current Owner"
    );

    assert!(bridge.admit(first, Uuid::new_v4()).await.is_err());
    let consumed: i64 = sqlx::query_scalar(
        "select count(*) from concert_world_tickets where event_id=$1 and consumed_at is not null",
    )
    .bind(event.uuid())
    .fetch_one(&concert)
    .await?;
    assert_eq!(consumed, 1, "sealed Auth must not consume a second ticket");
    // A second real owner makes directional Friend privacy observable.
    let other_user = AuthUserId::from_uuid(Uuid::new_v4());
    let other_family = Uuid::new_v4();
    let other = WorldSubject {
        key: ParticipantKey {
            event_id: event,
            user_id: other_user,
        },
        event_id: event,
        session_id: SessionId::new(other_family.to_string())?,
    };
    sqlx::query(
        "insert into users(id,primary_email,password_hash,status) values($1,$2,'fixture','ACTIVE')",
    )
    .bind(other_user.uuid())
    .bind(format!("{other_family}@composition.invalid"))
    .execute(&auth)
    .await?;
    sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values($1,'fixture',$2,$3,clock_timestamp()+interval '1 hour')").bind(Uuid::new_v4().to_string()).bind(other_user.uuid()).bind(other_family).execute(&auth).await?;
    sqlx::query("insert into profiles(user_id,username,nickname,bio,avatar_url) values($1,$2,'Other Owner','private-canary','https://private.invalid')")
        .bind(other_user.uuid())
        .bind(format!("other-{other_family}"))
        .execute(&profile_pool)
        .await?;
    owners.pause().await?;
    sqlx::query(
        "insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')",
    )
    .bind(event.uuid())
    .bind(other_user.uuid())
    .execute(&concert)
    .await?;
    sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(event.uuid()).bind(other_user.uuid()).execute(&concert).await?;
    sqlx::query("insert into concert_avatar_settings(event_id,user_id,contract_version,manifest_id,manifest_version,body_id,eyes_id,hair_id,outfit_id,skin_tone_id) values($1,$2,'1.0.0','avatar_fixture',1,'body_fixture','eyes_fixture','hair_fixture','outfit_fixture','tone_fixture')").bind(event.uuid()).bind(other_user.uuid()).execute(&concert).await?;
    let other_ticket = ticket(&bridge, &other, room, expiry, 51).await?;
    let other_binding = bridge
        .admit(other_ticket, Uuid::new_v4())
        .await
        .map_err(|_| anyhow::anyhow!("second owner admission failed"))?;
    adapter.install(&other_binding, bridge.now_ms()?).unwrap();
    bridge.maintain().await?;
    let policy = RecipientPolicy::current(
        &bridge,
        &b,
        &[b.clone(), other_binding.clone()],
        bridge.now_ms()?,
    )
    .unwrap();
    assert!(policy.visible_users.contains(&other_user.uuid()));
    sqlx::query(
        "insert into user_blocks(blocker_id,blocked_id,created_at) values($1,$2,clock_timestamp())",
    )
    .bind(other_user.uuid())
    .bind(user.uuid())
    .execute(&friend_pool)
    .await?;
    bridge.maintain().await?;
    let policy = RecipientPolicy::current(
        &bridge,
        &b,
        &[b.clone(), other_binding.clone()],
        bridge.now_ms()?,
    )
    .unwrap();
    assert!(!policy.visible_users.contains(&other_user.uuid()));
    assert!(policy.visible_users.contains(&user.uuid()));
    sqlx::query(
        "update user_blocks set deleted_at=clock_timestamp() where blocker_id=$1 and blocked_id=$2",
    )
    .bind(other_user.uuid())
    .bind(user.uuid())
    .execute(&friend_pool)
    .await?;
    bridge.maintain().await?;
    assert!(RecipientPolicy::current(
        &bridge,
        &b,
        &[b.clone(), other_binding.clone()],
        bridge.now_ms()?
    )
    .unwrap()
    .visible_users
    .contains(&other_user.uuid()));
    bridge.disconnected(&other_binding).await;
    assert_eq!(bridge.cleanup().await?, 0);
    bridge.maintain().await?;
    let before = lease(&concert, subject.key).await?;
    assert!(before <= expiry);
    assert!(
        before <= Utc::now() + TimeDelta::seconds(5),
        "initial persisted lease uses milliseconds rather than seconds"
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    bridge.maintain().await?;
    let renewed = lease(&concert, subject.key).await?;
    assert!(renewed <= Utc::now() + TimeDelta::seconds(5));
    assert!(renewed > Utc::now());
    assert!(renewed > before);
    // Repeated fresh reads must advance a bounded lease without extending the
    // original credential expiry or the effective Auth publication window.
    tokio::time::sleep(Duration::from_millis(100)).await;
    bridge.maintain().await?;
    assert!(lease(&concert, subject.key).await? > renewed);
    assert_eq!(
        CanonicalAuthorityPort::current(&bridge, &b, bridge.now_ms()?)
            .unwrap()
            .access_expires_at,
        expiry
    );
    let change = ticket(&bridge, &subject, target, expiry, 42).await?;
    let moved = bridge
        .switch(&b, change, Uuid::new_v4())
        .await
        .map_err(|_| anyhow::anyhow!("production Auth switch failed"))?;
    adapter.install(&moved, bridge.now_ms()?).unwrap();
    assert_eq!(moved.room_id, target);
    assert!(
        lease(&concert, subject.key).await? <= Utc::now() + TimeDelta::seconds(5),
        "switch persisted lease uses the same five-second bound"
    );
    assert!(
        CanonicalAuthorityPort::current(&bridge, &moved, bridge.now_ms()?)
            .unwrap()
            .lease_expires_at
            <= Utc::now() + TimeDelta::seconds(2)
    );
    assert_eq!(
        CanonicalAuthorityPort::current(&bridge, &moved, bridge.now_ms()?)
            .unwrap()
            .access_expires_at,
        expiry
    );
    let before_denial = lease(&concert, subject.key).await?;
    sqlx::query("update refresh_tokens set revoked_at=clock_timestamp() where user_id=$1")
        .bind(user.uuid())
        .execute(&auth)
        .await?;
    bridge.maintain().await?;
    assert_eq!(
        lease(&concert, subject.key).await?,
        before_denial,
        "denied fresh Auth cannot extend DB lease"
    );
    assert!(CanonicalAuthorityPort::current(&bridge, &moved, bridge.now_ms()?).is_err());
    assert_eq!(bridge.cleanup().await?, 0);
    assert!(bridge
        .repository
        .current_owned_membership(subject.key, Utc::now())
        .await?
        .is_none());
    sqlx::query("update users set status='SUSPENDED' where id=$1")
        .bind(user.uuid())
        .execute(&auth)
        .await?;
    let denied = ticket(&bridge, &subject, target, expiry, 43).await?;
    assert_eq!(
        bridge.admit(denied, Uuid::new_v4()).await.unwrap_err().code,
        4403
    );
    assert!(bridge
        .repository
        .current_owned_membership(subject.key, Utc::now())
        .await?
        .is_none());
    sqlx::query("update users set status='ACTIVE' where id=$1")
        .bind(user.uuid())
        .execute(&auth)
        .await?;
    sqlx::query("update refresh_tokens set revoked_at=null where user_id=$1")
        .bind(user.uuid())
        .execute(&auth)
        .await?;
    let last = ticket(&bridge, &subject, target, expiry, 44).await?;
    let last = bridge
        .admit(last, Uuid::new_v4())
        .await
        .map_err(|_| anyhow::anyhow!("restored owner denied"))?;
    adapter.install(&last, bridge.now_ms()?).unwrap();
    let before_drift = lease(&concert, subject.key).await?;
    owners.pause().await?;
    sqlx::query("update concert_world_memberships set access_expires_at=access_expires_at+interval '1 second' where event_id=$1 and user_id=$2")
        .bind(event.uuid()).bind(user.uuid()).execute(&concert).await?;
    bridge.maintain().await?;
    assert_eq!(lease(&concert, subject.key).await?, before_drift);
    assert!(
        CanonicalAuthorityPort::current(&bridge, &last, bridge.now_ms()?).is_err(),
        "canonical credential expiry drift cannot replace the captured lifetime"
    );
    assert_eq!(bridge.cleanup().await?, 0);
    sqlx::query("update profiles set deleted_at=clock_timestamp() where user_id=$1")
        .bind(user.uuid())
        .execute(&profile_pool)
        .await?;
    let deleted_profile_ticket = ticket(&bridge, &subject, target, expiry, 46).await?;
    assert!(bridge
        .admit(deleted_profile_ticket, Uuid::new_v4())
        .await
        .is_err());
    assert_eq!(bridge.cleanup().await?, 0);
    sqlx::query("update profiles set deleted_at=null where user_id=$1")
        .bind(user.uuid())
        .execute(&profile_pool)
        .await?;
    let last = ticket(&bridge, &subject, target, expiry, 45).await?;
    let last = bridge
        .admit(last, Uuid::new_v4())
        .await
        .map_err(|_| anyhow::anyhow!("fresh admission after expiry drift failed"))?;
    adapter.install(&last, bridge.now_ms()?).unwrap();
    let before_outage = lease(&concert, subject.key).await?;
    server.finish().await;
    bridge.maintain().await?;
    assert_eq!(
        lease(&concert, subject.key).await?,
        before_outage,
        "unavailable Auth cannot extend DB lease"
    );
    assert!(CanonicalAuthorityPort::current(&bridge, &last, bridge.now_ms()?).is_err());
    assert_eq!(bridge.cleanup().await?, 0);
    owners.pause().await?;
    drop(bridge);
    drop(owners);
    friend_server.finish().await;
    profile_server.finish().await;
    friend_pool.close().await;
    profile_pool.close().await;
    concert.close().await;
    auth.close().await;
    Ok(())
}
