// Adapted from relationship_authority.rs and world_auth/tests.rs at 1b7caec.
// Fixtures exercise real guarded owners; no controlled owner responses certify native behavior.
use super::*;
use crate::{
    adapters::out::relationship_authority::world_auth::{
        GrpcWorldAuthReader, WorldAuthRead, WorldOwnerRequest,
    },
    world::{CurrentOwnedMembership, PgWorldRepository},
};
use concert_application::{identity::SessionId, world_ticket::*};
use concert_domain::{AssetId, AuthUserId, EventId, ParticipantKey, Revision};
use platform::{
    actor_delegation::{
        authority_deadline::AuthorityRpcDeadline,
        relationship_read::{RelationshipReadSigner, RelationshipReadVerifier},
        ActorDelegationSigner, ActorDelegationVerifier,
    },
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::guard,
};
use uuid::Uuid;
fn deadline() -> AuthorityRpcDeadline {
    AuthorityRpcDeadline::new(
        Utc::now().timestamp_micros() + 1_800_000,
        std::time::Duration::from_secs(2),
    )
    .unwrap()
}
fn signer() -> RelationshipReadSigner {
    RelationshipReadSigner::from_seed(&[37; 32], &gateway(Workload::Concert)).unwrap()
}
fn gateway(w: Workload) -> ActorDelegationVerifier {
    ActorDelegationVerifier::new(
        w,
        &[
            ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32])
                .unwrap()
                .public_key(),
        ],
    )
    .unwrap()
}
fn dedicated(w: Workload) -> ActorDelegationVerifier {
    RelationshipReadVerifier::new(w, &[signer().public_key()], &gateway(w))
        .unwrap()
        .into_inner()
}
struct Running {
    endpoint: String,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn claim() -> TicketClaim {
    let now = Utc::now();
    let event = EventId::from_uuid(Uuid::new_v4());
    TicketClaim {
        record: TicketRecord {
            digest: AdmissionTicket::from_entropy(&[37; 32]).unwrap().digest(),
            subject: WorldSubject {
                key: ParticipantKey {
                    event_id: event,
                    user_id: AuthUserId::from_uuid(Uuid::new_v4()),
                },
                event_id: event,
                session_id: SessionId::new(Uuid::new_v4().to_string()).unwrap(),
            },
            pin: ResourcePin {
                event_id: event,
                room_id: RoomId::from_uuid(Uuid::new_v4()),
                map: MapPin::new(
                    AssetId::new("map_world_auth".into()).unwrap(),
                    Revision::new(1).unwrap(),
                ),
                avatar: AvatarPin::new(
                    AssetId::new("avatar_manifest".into()).unwrap(),
                    Revision::new(1).unwrap(),
                ),
            },
            issued_at: now - chrono::Duration::seconds(1),
            expires_at: now + chrono::Duration::seconds(20),
            access_expires_at: now + chrono::Duration::minutes(10),
        },
        connection_id: ConnectionId::from_uuid(Uuid::new_v4()).unwrap(),
    }
}
async fn owned_pool(owner: &str) -> sqlx::PgPool {
    use std::{os::unix::fs::MetadataExt, path::PathBuf, str::FromStr};
    let root =
        PathBuf::from(std::env::var("MCR_RECIPIENT_ROOT").expect("private harness root required"));
    assert_eq!(root.canonicalize().unwrap(), root);
    assert!(root.starts_with("/private/tmp") || root.starts_with("/tmp"));
    let m = std::fs::symlink_metadata(&root).unwrap();
    assert!(m.is_dir());
    assert_eq!(m.mode() & 0o077, 0);
    assert_eq!(
        format!("{}:{}", m.dev(), m.ino()),
        std::env::var("MCR_RECIPIENT_IDENTITY").unwrap()
    );
    let path = root.join("mcr-owner.json");
    let mm = std::fs::symlink_metadata(&path).unwrap();
    assert!(mm.is_file());
    assert_eq!(mm.uid(), m.uid());
    assert_eq!(mm.mode() & 0o077, 0);
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(marker["uid"].as_u64(), Some(u64::from(m.uid())));
    assert_eq!(
        marker["token"].as_str(),
        Some(std::env::var("MCR_RECIPIENT_TOKEN").unwrap().as_str())
    );
    for name in ["data", "socket"] {
        let p = root.join(name);
        assert!(std::fs::symlink_metadata(&p).unwrap().is_dir());
        assert_eq!(p.canonicalize().unwrap(), p);
    }
    let options = sqlx::postgres::PgConnectOptions::from_str(
        &std::env::var(format!("MCR_RECIPIENT_{}_URL", owner.to_uppercase())).unwrap(),
    )
    .unwrap();
    assert_eq!(options.get_username(), "mcr_owner");
    assert_eq!(
        options.get_database(),
        Some(format!("mcr_{owner}_recipient").as_str())
    );
    assert_eq!(options.get_socket(), Some(&root.join("socket")));
    assert_eq!(options.get_port(), 5432);
    assert!(options.get_options().is_none());
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await
        .unwrap();
    let row:(String,String,String,String,String,i32)=sqlx::query_as("select current_database(),current_user,current_setting('data_directory'),current_setting('unix_socket_directories'),current_setting('listen_addresses'),current_setting('server_version_num')::int").fetch_one(&pool).await.unwrap();
    assert_eq!(
        row,
        (
            format!("mcr_{owner}_recipient"),
            "mcr_owner".into(),
            root.join("data").to_str().unwrap().into(),
            root.join("socket").to_str().unwrap().into(),
            "".into(),
            160014
        )
    );
    pool
}
async fn native_auth(pool: sqlx::PgPool) -> Running {
    use auth_service::adapters::r#in::grpc::concert_relationship as owner;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let service=guard(&owner::POLICY,owner::proto::auth_concert_relationship_service_server::AuthConcertRelationshipServiceServer::new(owner::RelationshipServer::from_pool(pool)),CallerRegistry::new().register(Workload::Concert,"concert").unwrap(),dedicated(Workload::Auth)).unwrap();
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .layer(CallDeadlineLayer)
            .add_service(service)
            .serve_with_incoming(
                tonic::codegen::tokio_stream::wrappers::TcpListenerStream::new(listener),
            )
            .await
            .unwrap();
    });
    Running { endpoint, task }
}
async fn native_friend(pool: sqlx::PgPool) -> Running {
    use friend_service::adapters::r#in::grpc::concert_relationship as owner;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let service=guard(&owner::POLICY,owner::proto::friend_concert_relationship_service_server::FriendConcertRelationshipServiceServer::new(owner::RelationshipServer::from_pool(pool)),CallerRegistry::new().register(Workload::Concert,"concert").unwrap(),dedicated(Workload::Friend)).unwrap();
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .layer(CallDeadlineLayer)
            .add_service(service)
            .serve_with_incoming(
                tonic::codegen::tokio_stream::wrappers::TcpListenerStream::new(listener),
            )
            .await
            .unwrap();
    });
    Running { endpoint, task }
}

async fn insert_claim(
    repo: &PgWorldRepository,
    pool: &PgPool,
    c: &TicketClaim,
    owner: &OwnerIdentity,
) -> anyhow::Result<CurrentOwnedMembership> {
    let event = c.record.subject.event_id.uuid();
    let user = c.record.subject.key.user_id.uuid();
    sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,true)").bind(event).bind(user).execute(pool).await?;
    sqlx::query("insert into concert_event_admissions(event_id,user_id,state,restriction_revision) values($1,$2,'ACTIVE',1)").bind(event).bind(user).execute(pool).await?;
    assert_eq!(repo.issue(c.record.clone()).await?, IssueOutcome::Issued);
    let ConsumeOutcome::Consumed(consumed) = repo
        .consume(ConsumeClaim {
            digest: c.record.digest.clone(),
            connection_id: c.connection_id,
            requested: c.record.pin.clone(),
            now: Utc::now(),
        })
        .await?
    else {
        anyhow::bail!("consume required")
    };
    assert!(matches!(
        repo.admit(owner, &consumed, Utc::now()).await?,
        AdmissionOutcome::Admitted(_)
    ));
    // Extend only the private fixture's lease before capturing its authoritative row.
    // This prevents the multi-case native scenario from depending on wall-clock speed.
    sqlx::query("update concert_world_memberships set lease_until=least(access_expires_at,clock_timestamp()+interval '15 seconds') where event_id=$1 and user_id=$2")
        .bind(event).bind(user).execute(pool).await?;
    Ok(repo
        .current_owned_membership(c.record.subject.key, Utc::now())
        .await?
        .unwrap())
}
async fn context(
    auth: &Running,
    actor: &CurrentOwnedMembership,
    target: &CurrentOwnedMembership,
) -> anyhow::Result<WorldPresenceProjection> {
    let a = WorldSubject {
        key: actor.membership.key,
        event_id: actor.membership.key.event_id,
        session_id: actor.membership.session_id.clone(),
    };
    let s = WorldSubject {
        key: target.membership.key,
        event_id: target.membership.key.event_id,
        session_id: target.membership.session_id.clone(),
    };
    let req = WorldOwnerRequest::from_current(
        actor,
        &actor.owner,
        &a,
        actor.membership.connection_id,
        Uuid::new_v4(),
        deadline(),
    )?;
    let reader = GrpcWorldAuthReader::connect(&auth.endpoint, "concert", None, signer())?;
    let observed = reader.observe(&req).await?;
    Ok(req.projection_parent(&observed)?.presence_projection(
        target,
        &target.owner,
        &s,
        target.membership.connection_id,
    )?)
}
#[tokio::test]
#[ignore = "requires marker/inode-owned Auth/Friend/Concert PostgreSQL"]
async fn native_signed_presence_local_policy_revocation_and_deadlines() -> anyhow::Result<()> {
    // Heap-pin the long native scenario rather than copying its large async
    // state through libtest/Tokio's default test-thread stack.
    Box::pin(native_scenario()).await
}
async fn native_scenario() -> anyhow::Result<()> {
    let auth_pool = owned_pool("auth").await;
    let friend_pool = owned_pool("friend").await;
    let concert = owned_pool("concert").await;
    let auth = native_auth(auth_pool.clone()).await;
    let friend = native_friend(friend_pool.clone()).await;
    let c = claim();
    let mut s = claim();
    s.record.pin = c.record.pin.clone();
    s.record.subject.event_id = c.record.subject.event_id;
    s.record.subject.key.event_id = c.record.subject.event_id;
    s.record.digest = AdmissionTicket::from_entropy(&[38; 32])?.digest();
    let event = c.record.subject.event_id;
    let actor = c.record.subject.key.user_id.uuid();
    let subject = s.record.subject.key.user_id.uuid();
    for claim in [&c, &s] {
        let user = claim.record.subject.key.user_id.uuid();
        let family = Uuid::parse_str(claim.record.subject.session_id.as_str())?;
        sqlx::query("insert into users(id,primary_email,password_hash,status) values($1,$2,'private-canary','ACTIVE')").bind(user).bind(format!("{user}@recipient.invalid")).execute(&auth_pool).await?;
        sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values($1,'private-canary',$2,$3,clock_timestamp()+interval '1 hour')").bind(Uuid::new_v4().to_string()).bind(user).bind(family).execute(&auth_pool).await?;
    }
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Recipient fixture','OPEN')").bind(event.uuid()).bind(format!("recipient-{}",event.uuid().simple())).execute(&concert).await?;
    sqlx::query("insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,is_default) values($1,$2,'Fixture',4,'map_world_auth',1,'avatar_manifest',1,true)").bind(event.uuid()).bind(c.record.pin.room_id.uuid()).execute(&concert).await?;
    let owner = OwnerIdentity::new("recipient-fixture-owner".into(), Uuid::new_v4())?;
    let repo = PgWorldRepository::new(concert.clone());
    repo.claim_owner(
        event,
        c.record.pin.room_id,
        &owner,
        Utc::now(),
        chrono::Duration::seconds(20),
    )
    .await?;
    let a = insert_claim(&repo, &concert, &c, &owner).await?;
    let t = insert_claim(&repo, &concert, &s, &owner).await?;
    let reader = WorldRecipientReader::new(
        concert.clone(),
        GrpcRelationshipAuthority::connect(
            &auth.endpoint,
            &friend.endpoint,
            "concert",
            None,
            signer(),
        )?,
    );
    let ctx = context(&auth, &a, &t).await?;
    let first = reader.read(&ctx).await?;
    assert!(first.visible());
    first.check_scope(&ctx)?;
    assert!(first.valid_until() <= a.membership.lease_until.min(t.membership.lease_until));
    let self_subject = WorldSubject {
        key: a.membership.key,
        event_id: a.membership.key.event_id,
        session_id: a.membership.session_id.clone(),
    };
    let self_context = ctx.parent().presence_projection(
        &a,
        &a.owner,
        &self_subject,
        a.membership.connection_id,
    )?;
    assert_eq!(self_context.request_id(), ctx.request_id());
    assert!(self_context.deadline().stop() <= ctx.parent().deadline().stop());
    assert!(
        reader.read(&self_context).await?.visible(),
        "distinct target shares original request"
    );
    assert!(
        first.check_scope(&self_context).is_err(),
        "one target cannot consume another target observation"
    );
    assert_eq!(format!("{first:?}"), "RecipientObservation([REDACTED])");
    assert!(
        reader.read(&ctx).await.is_err(),
        "same owner attempts cannot be reset"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from concert_relationship_pairs where event_id=$1"
        )
        .bind(event.uuid())
        .fetch_one(&concert)
        .await?,
        0,
        "read must not commit neutral pairs"
    );
    sqlx::query(
        "insert into user_blocks(blocker_id,blocked_id,created_at) values($1,$2,clock_timestamp())",
    )
    .bind(subject)
    .bind(actor)
    .execute(&friend_pool)
    .await?;
    let blocked = reader.read(&context(&auth, &a, &t).await?).await?;
    assert!(!blocked.visible());
    assert_eq!(blocked.owner_revisions().auth, first.owner_revisions().auth);
    assert!(blocked.owner_revisions().friend.revision > first.owner_revisions().friend.revision);
    sqlx::query(
        "update user_blocks set deleted_at=clock_timestamp() where blocker_id=$1 and blocked_id=$2",
    )
    .bind(subject)
    .bind(actor)
    .execute(&friend_pool)
    .await?;
    sqlx::query("update concert_event_admissions set state='SUSPENDED',restriction_revision=restriction_revision+1 where event_id=$1 and user_id=$2").bind(event.uuid()).bind(subject).execute(&concert).await?;
    let local_denied = reader.read(&context(&auth, &a, &t).await?).await?;
    assert!(!local_denied.visible());
    assert!(
        local_denied.local_revisions().subject_restriction.get()
            > first.local_revisions().subject_restriction.get()
    );
    sqlx::query("update concert_event_admissions set state='ACTIVE',restriction_revision=restriction_revision+1 where event_id=$1 and user_id=$2").bind(event.uuid()).bind(subject).execute(&concert).await?;
    // Local block is an independent policy input, with a durable local revision.
    let mut block_tx = concert.begin().await?;
    crate::relationship::pin_deadline(&mut block_tx, Utc::now() + chrono::Duration::seconds(5))
        .await?;
    sqlx::query("insert into concert_relationship_pairs(event_id,low_user,high_user,low_blocks_high) values($1,$2,$3,true)")
        .bind(event.uuid()).bind(actor.min(subject)).bind(actor.max(subject)).execute(&mut *block_tx).await?;
    block_tx.commit().await?;
    assert!(!reader.read(&context(&auth, &a, &t).await?).await?.visible());
    let mut block_tx = concert.begin().await?;
    crate::relationship::pin_deadline(&mut block_tx, Utc::now() + chrono::Duration::seconds(5))
        .await?;
    sqlx::query("update concert_relationship_pairs set low_blocks_high=false,revision=revision+1 where event_id=$1").bind(event.uuid()).execute(&mut *block_tx).await?;
    block_tx.commit().await?;
    // Auth subject revocation advances Auth independently of Friend.
    sqlx::query("update users set status='SUSPENDED' where id=$1")
        .bind(subject)
        .execute(&auth_pool)
        .await?;
    let revoked_subject = reader.read(&context(&auth, &a, &t).await?).await?;
    assert!(!revoked_subject.visible());
    assert!(
        revoked_subject.owner_revisions().auth.revision > blocked.owner_revisions().auth.revision
    );
    sqlx::query("update users set status='ACTIVE' where id=$1")
        .bind(subject)
        .execute(&auth_pool)
        .await?;
    // Target connection drift after context derivation is rejected by the local read.
    let drift = context(&auth, &a, &t).await?;
    sqlx::query(
        "update concert_world_memberships set disconnected=true where event_id=$1 and user_id=$2",
    )
    .bind(event.uuid())
    .bind(subject)
    .execute(&concert)
    .await?;
    assert!(reader.read(&drift).await.is_err());
    sqlx::query(
        "update concert_world_memberships set disconnected=false where event_id=$1 and user_id=$2",
    )
    .bind(event.uuid())
    .bind(subject)
    .execute(&concert)
    .await?;
    let pin_drift = context(&auth, &a, &t).await?;
    sqlx::query("update concert_world_rooms set map_revision=2 where event_id=$1")
        .bind(event.uuid())
        .execute(&concert)
        .await?;
    assert!(reader.read(&pin_drift).await.is_err());
    sqlx::query("update concert_world_rooms set map_revision=1 where event_id=$1")
        .bind(event.uuid())
        .execute(&concert)
        .await?;
    let owner_drift = context(&auth, &a, &t).await?;
    sqlx::query("update concert_world_owner_leases set owner_epoch=$2 where event_id=$1")
        .bind(event.uuid())
        .bind(Uuid::new_v4())
        .execute(&concert)
        .await?;
    assert!(reader.read(&owner_drift).await.is_err());
    sqlx::query("update concert_world_owner_leases set owner_epoch=$2 where event_id=$1")
        .bind(event.uuid())
        .bind(owner.epoch)
        .execute(&concert)
        .await?;
    // Cancel while a real local query waits on an event-row lock.
    let waiting = context(&auth, &a, &t).await?;
    let mut blocker = concert.begin().await?;
    sqlx::query("select event_id from concert_events where event_id=$1 for update")
        .bind(event.uuid())
        .execute(&mut *blocker)
        .await?;
    let cancel = async {
        let stop = tokio::time::Instant::now() + std::time::Duration::from_millis(500);
        loop {
            let blocked:Option<i32>=sqlx::query_scalar("select pid from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and wait_event_type='Lock' limit 1").fetch_optional(&concert).await.unwrap();
            if let Some(pid) = blocked {
                waiting.cancel();
                return pid;
            }
            if tokio::time::Instant::now() >= stop {
                waiting.cancel();
                return 0;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    };
    let start = tokio::time::Instant::now();
    let (result, observed_wait) = tokio::join!(reader.read(&waiting), cancel);
    assert!(
        observed_wait > 0,
        "test must cancel an actually blocked local query"
    );
    assert!(result.is_err());
    assert!(start.elapsed() < std::time::Duration::from_millis(500));
    // Give the owning SQL task its bounded cancellation/rollback drain, then
    // prove no recipient backend remains blocked behind the still-held lock.
    tokio::time::timeout(std::time::Duration::from_millis(300), async {
        while !local::cancellation_drained(observed_wait) {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("target backend must acknowledge explicit cancel and both rollbacks");
    let pending:i64=sqlx::query_scalar("select count(*) from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and wait_event_type='Lock'").fetch_one(&concert).await?;
    assert_eq!(
        pending, 0,
        "cancelled local query must release its backend wait"
    );
    blocker.rollback().await?;
    let ctx = context(&auth, &a, &t).await?;
    ctx.cancel();
    assert!(reader.read(&ctx).await.is_err());
    // A live wall credential cannot extend the original operation stop.
    let ctx = context(&auth, &a, &t).await?;
    tokio::time::sleep_until(ctx.deadline().stop()).await;
    assert!(reader.read(&ctx).await.is_err());
    let ctx = context(&auth, &a, &t).await?;
    sqlx::query("update refresh_tokens set revoked_at=clock_timestamp() where user_id=$1")
        .bind(actor)
        .execute(&auth_pool)
        .await?;
    assert_eq!(reader.read(&ctx).await.unwrap_err(), WorldError::Denied);
    drop(reader);
    drop(auth);
    drop(friend);
    auth_pool.close().await;
    friend_pool.close().await;
    concert.close().await;
    Ok(())
}
