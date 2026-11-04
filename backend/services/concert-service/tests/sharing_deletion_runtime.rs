//! Sharing composition boundary: actual Auth signup/delete/worker, authenticated
//! SOURCE/Apply, same incumbent Social factory publisher, durable store receipt.
//! Helpers below are adapted from account_deletion_runtime.rs at cf1ca629;
//! AES-GCM/storage seed from social_account_deletion.rs at cf1ca629. These are
//! explicit storage fixtures, not canonical Social eligibility or WS acceptance.
#[path = "../../auth-service/tests/support/auth_fixture_config.rs"]
mod auth_fixture;
#[path = "canonical_owner_composition_support/environment.rs"]
mod environment;
#[path = "account_deletion_runtime_support/mod.rs"]
mod runtime_support;
use auth_service::{
    adapters::out::persistence::user_deletion_saga_repository::PgUserDeletionSagaRepository,
    application::{
        dto::{auth_dto::DeleteUserDto, deletion_dto::AuthUserDeletionTargetDto as Target},
        errors::AuthError,
        ports::{UserDeletionDispatcherPort, UserDeletionSagaPort},
    },
    config::{
        build_state_with_deletion_runtime, concert_deletion::ConcertDeletionSettings, AppState,
    },
    domain::user_deletion::{UserDeletionSaga, UserDeletionStepStatus, UserDeletionTarget},
};
use concert_service::{
    adapters::r#in::deletion_rpc::{self, proto},
    config::{
        account_deletion_runtime::{
            AccountDeletionRuntime, AccountDeletionSettings, AuthDeletionCallers,
            LiveDeletionOwners,
        },
        social_policy_runtime::SocialWorldPolicy,
    },
    world::account_deletion as common,
};
use platform::{
    grpc_auth::{CallerRegistry, Workload},
    grpc_tls::TlsMaterial,
};
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use runtime_support::*;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;
fn settings(endpoint: &str, ms: u64) -> anyhow::Result<ConcertDeletionSettings> {
    ConcertDeletionSettings::from_values(Some("managed"), Some(endpoint), Some(&ms.to_string()))
}
async fn state(
    pool: PgPool,
    concert: &str,
    notification: &str,
    ms: u64,
) -> anyhow::Result<AppState> {
    let mut c = auth_fixture::test_config(std::env::var("MCR_CANONICAL_AUTH_URL")?)?;
    c.outbound_identity = AUTH.into();
    c.tls_material = Some(environment::tls(Workload::Auth)?);
    c.profile_grpc_endpoint = "https://127.0.0.1:1".into();
    c.friend_grpc_endpoint = c.profile_grpc_endpoint.clone();
    c.post_grpc_endpoint = c.profile_grpc_endpoint.clone();
    c.notification_grpc_endpoint = notification.into();
    c.user_deletion_worker_enabled = true;
    c.user_deletion_worker_batch_size = 100;
    c.user_deletion_worker_max_attempts = 8;
    c.user_deletion_worker_lease_timeout_ms = 1000;
    build_state_with_deletion_runtime(&c, pool, settings(concert, ms)?).await
}
async fn identity(s: &AppState, p: &PgPool) -> anyhow::Result<Uuid> {
    let email = format!("runtime-{}@example.invalid", Uuid::new_v4().simple());
    let created = s.signup.signup_email(&email, "Owned-runtime-42!").await?;
    assert_eq!(created.status, "PENDING_VERIFICATION");
    let code: String = sqlx::query_scalar("select code from email_verifications where user_id=$1")
        .bind(created.user_id)
        .fetch_one(p)
        .await?;
    assert_eq!(
        s.signup.verify_email(&email, &code).await?.status,
        "VERIFIED"
    );
    let login = s
        .login
        .login(&email, "Owned-runtime-42!", None, None)
        .await?;
    assert!(!login.access_token.is_empty() && !login.refresh_token.is_empty());
    Ok(created.user_id)
}
async fn saga(p: &PgPool, id: Uuid) -> anyhow::Result<UserDeletionSaga> {
    Ok(PgUserDeletionSagaRepository::new(p.clone())
        .find_by_saga_id(id)
        .await?
        .unwrap()
        .into())
}
async fn park(p: &PgPool, id: Uuid) -> anyhow::Result<()> {
    // Scheduling only: preserve every required target/receipt/status; never
    // manufacture a positive owner response or remove a required step.
    sqlx::query("update user_deletion_steps set next_attempt_at=clock_timestamp()+interval '1 day', status=case when status='IN_PROGRESS' then 'RETRY' else status end, locked_at=case when status='IN_PROGRESS' then null else locked_at end where saga_id=$1").bind(id).execute(p).await?;
    Ok(())
}
async fn due(p: &PgPool, id: Uuid, target: &str) -> anyhow::Result<()> {
    sqlx::query("update user_deletion_steps set next_attempt_at=clock_timestamp()-interval '1 second' where saga_id=$1 and target=$2").bind(id).bind(target).execute(p).await?;
    Ok(())
}
fn assert_pending(s: &UserDeletionSaga, successes: &[UserDeletionTarget]) {
    assert_eq!(s.steps.len(), 8);
    assert!(s.completed_at.is_none());
    for step in &s.steps {
        assert_eq!(
            step.status == UserDeletionStepStatus::Succeeded,
            successes.contains(&step.target),
            "target {:?}",
            step.target
        );
    }
}
async fn apply(
    d: &auth_service::config::concert_deletion::RuntimeDispatcher,
    s: &UserDeletionSaga,
    target: Target,
) -> Result<(), AuthError> {
    d.apply(target, s.id, s.user_id, s.actor_id, s.retention_until)
        .await
}
fn encrypted(event: Uuid, user: Uuid) -> (Vec<u8>, Vec<u8>) {
    let random = SystemRandom::new();
    let mut key = [0; 32];
    let mut nonce = [0; 12];
    random.fill(&mut key).unwrap();
    random.fill(&mut nonce).unwrap();
    let key = aead::LessSafeKey::new(aead::UnboundKey::new(&aead::AES_256_GCM, &key).unwrap());
    let mut aad = b"MCR-social-handle-AES256GCM-v1".to_vec();
    for part in [
        event.as_bytes().as_slice(),
        user.as_bytes().as_slice(),
        b"INSTAGRAM".as_slice(),
        1i64.to_be_bytes().as_slice(),
        b"deletion_fixture".as_slice(),
    ] {
        aad.extend((part.len() as u64).to_be_bytes());
        aad.extend(part);
    }
    let mut body = b"private.deletion.fixture".to_vec();
    key.seal_in_place_append_tag(
        aead::Nonce::assume_unique_for_key(nonce),
        aead::Aad::from(aad),
        &mut body,
    )
    .unwrap();
    (nonce.to_vec(), body)
}

async fn seed(pool: &PgPool, user: Uuid, peer: Uuid) -> anyhow::Result<Uuid> {
    let event = Uuid::new_v4();
    let mut tx = pool.begin().await?;
    // Storage-only guard. No fabricated Intro witness, policy or Auth grant.
    sqlx::query("select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '5 seconds')::text,true)").execute(&mut *tx).await?;
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Sharing production factory fixture','OPEN')")
        .bind(event).bind(event.to_string()).execute(&mut *tx).await?;
    for actor in [user, peer] {
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)")
            .bind(event).bind(actor).execute(&mut *tx).await?;
        let (nonce, ciphertext) = encrypted(event, actor);
        sqlx::query("insert into concert_social_handles(event_id,user_id,platform,key_id,nonce,ciphertext) values($1,$2,'INSTAGRAM','deletion_fixture',$3,$4)")
            .bind(event).bind(actor).bind(nonce).bind(ciphertext).execute(&mut *tx).await?;
    }
    for (requester, recipient) in [(user, peer), (peer, user)] {
        let id = Uuid::new_v4();
        sqlx::query("insert into concert_social_requests(event_id,request_id,requester_id,recipient_id,intro_id,intro_revision,authority_revision,relationship_revision,state,expires_at) values($1,$2,$3,$4,$5,1,1,1,'PENDING',clock_timestamp()+interval '1 hour')")
            .bind(event).bind(id).bind(requester).bind(recipient).bind(Uuid::new_v4()).execute(&mut *tx).await?;
        sqlx::query("insert into concert_social_bindings(event_id,request_id,platform,requester_version,recipient_version,requester_consents,approved) values($1,$2,'INSTAGRAM',1,1,true,true)")
            .bind(event).bind(id).execute(&mut *tx).await?;
        sqlx::query("update concert_social_requests set state='APPROVED',revision=2 where event_id=$1 and request_id=$2")
            .bind(event).bind(id).execute(&mut *tx).await?;
        for (owner, viewer) in [(user, peer), (peer, user)] {
            sqlx::query("insert into concert_social_grants(event_id,request_id,owner_id,viewer_id,platform,handle_version,peer_handle_version,state,expires_at) select event_id,request_id,$3,$4,'INSTAGRAM',1,1,'ACTIVE',expires_at from concert_social_requests where event_id=$1 and request_id=$2")
                .bind(event).bind(id).bind(owner).bind(viewer).execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    Ok(event)
}
async fn delete(s: &AppState, p: &PgPool, user: Uuid) -> anyhow::Result<UserDeletionSaga> {
    let deleted = s
        .delete_user
        .execute(DeleteUserDto {
            user_id: user.to_string(),
            actor_id: user.to_string(),
            request_id: None,
            ip_address: None,
            user_agent: None,
        })
        .await?;
    let g = saga(p, deleted.saga_id.parse()?).await?;
    park(p, g.id).await?;
    Ok(g)
}
async fn receipt(p: &PgPool, id: Uuid) -> anyhow::Result<serde_json::Value> {
    Ok(sqlx::query_scalar(
        "select to_jsonb(p) from concert_social_account_deletions p where saga_id=$1",
    )
    .bind(id)
    .fetch_one(p)
    .await?)
}
async fn ordinary(p: &PgPool) -> anyhow::Result<(i64, i64)> {
    Ok(sqlx::query_as("select (select count(*) from concert_command_receipts),(select count(*) from concert_outbox)").fetch_one(p).await?)
}
async fn assert_removed(p: &PgPool, event: Uuid, user: Uuid, peer: Uuid) -> anyhow::Result<()> {
    let own: (bool,i64) = sqlx::query_as("select key_id is null and nonce is null and ciphertext is null,revision from concert_social_handles where event_id=$1 and user_id=$2")
        .bind(event).bind(user).fetch_one(p).await?;
    assert_eq!(own, (true, 2));
    let other: bool = sqlx::query_scalar("select ciphertext is not null and revision=1 from concert_social_handles where event_id=$1 and user_id=$2")
        .bind(event).bind(peer).fetch_one(p).await?;
    assert!(other);
    let counts:(i64,i64,i64,i64) = sqlx::query_as("select (select count(*) from concert_social_requests where event_id=$1 and state='REVOKED'),(select count(*) from concert_social_bindings where event_id=$1),(select count(*) from concert_social_grants where event_id=$1 and state='REVOKED'),(select count(*) from concert_social_deletion_audience where event_id=$1 and published_at is null)")
        .bind(event).fetch_one(p).await?;
    assert_eq!(counts, (2, 2, 4, 0));
    Ok(())
}
async fn foreign_membership(p: &PgPool, event: Uuid, actor: Uuid) -> anyhow::Result<()> {
    let room = Uuid::new_v4();
    sqlx::query("insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,avatar_manifest_id,avatar_manifest_version) values($1,$2,'Explicit foreign membership fixture',10,'map_fixture',1,'avatar_fixture',1)")
        .bind(event).bind(room).execute(p).await?;
    sqlx::query("insert into concert_world_memberships(event_id,user_id,room_id,session_id,connection_id,owner_process_id,owner_epoch,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,admitted_at,lease_until,access_expires_at) values($1,$2,$3,$4,$5,'foreign_fixture',$6,'map_fixture',1,'avatar_fixture',1,clock_timestamp(),clock_timestamp()+interval '1 hour',clock_timestamp()+interval '2 hours')")
        .bind(event).bind(actor).bind(room).bind(Uuid::new_v4()).bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(p).await?;
    Ok(())
}
#[derive(Serialize, Deserialize)]
struct SharingSaved {
    saga: Uuid,
    event: Uuid,
    user: Uuid,
    peer: Uuid,
    receipt: serde_json::Value,
    ordinary: (i64, i64),
    pid: u32,
    pg_started: String,
}
fn save_sharing(v: &SharingSaved) -> anyhow::Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(environment::root()?.join("sharing-runtime-private.json"))?;
    f.write_all(&serde_json::to_vec(v)?)?;
    f.sync_all()?;
    Ok(())
}
fn load_sharing() -> anyhow::Result<SharingSaved> {
    let p = environment::root()?.join("sharing-runtime-private.json");
    let m = std::fs::symlink_metadata(&p)?;
    anyhow::ensure!(
        m.is_file()
            && !m.file_type().is_symlink()
            && m.permissions().mode() & 0o777 == 0o600
            && m.len() < 16384,
        "Private bounded owned handoff"
    );
    Ok(serde_json::from_slice(&std::fs::read(p)?)?)
}

#[tokio::test]
#[ignore = "fresh allocated PG16, SOURCE/Apply mTLS and physical PostgreSQL restart"]
async fn actual_sharing_worker_incumbent_factory_refusal_cancellation_and_restart(
) -> anyhow::Result<()> {
    let auth = environment::pool("auth").await?;
    let concert = environment::pool("concert").await?;
    let policy = SocialWorldPolicy::compose(concert.clone());
    let source = source_server(auth.clone()).await?;
    let incumbent = policy.publisher.clone();
    assert!(Arc::ptr_eq(&incumbent, &policy.publisher));
    let rpc = factory_server(
        concert.clone(),
        &source.url,
        Workload::Concert,
        LiveDeletionOwners {
            worlds: None,
            sharing_publisher: Some(incumbent),
        },
    )
    .await?;
    // Other seven targets remain present but parked; their already qualified
    // producers are not repeated or manufactured by this Sharing-only cohort.
    let unavailable = "https://127.0.0.1:1";
    let s = state(auth.clone(), &rpc.url, unavailable, 5000).await?;
    let d = runtime_dispatcher(&rpc.url, unavailable, 5000)?;
    let phase = std::env::var("MCR_SHARING_RUNTIME_PHASE")?;
    if phase == "RESTARTED" {
        let saved = load_sharing()?;
        assert_ne!(saved.pid, std::process::id());
        let started: String = sqlx::query_scalar("select pg_postmaster_start_time()::text")
            .fetch_one(&auth)
            .await?;
        assert_ne!(saved.pg_started, started);
        let g = saga(&auth, saved.saga).await?;
        assert_pending(&g, &[UserDeletionTarget::ConcertSharing]);
        let run = s.user_deletion_worker.run_once(chrono::Utc::now()).await?;
        assert_eq!((run.claimed, run.succeeded, run.retried), (0, 0, 0));
        apply(&d, &g, Target::ConcertSharing).await?;
        assert_eq!(receipt(&concert, g.id).await?, saved.receipt);
        assert_eq!(ordinary(&concert).await?, saved.ordinary);
        assert_removed(&concert, saved.event, saved.user, saved.peer).await?;
        assert_pending(
            &saga(&auth, g.id).await?,
            &[UserDeletionTarget::ConcertSharing],
        );
        println!("SHARING FACTORY RESTART PASS: fresh PID, physical PG restart, immutable receipt replay, seven pending targets");
    } else {
        assert_eq!(phase, "SOURCE");
        let user = identity(&s, &auth).await?;
        let peer = identity(&s, &auth).await?;
        let event = seed(&concert, user, peer).await?;
        foreign_membership(&concert, event, peer).await?;
        let before = ordinary(&concert).await?;
        let g = delete(&s, &auth, user).await?;
        due(&auth, g.id, "CONCERT_SHARING").await?;
        let run = s.user_deletion_worker.run_once(chrono::Utc::now()).await?;
        assert_eq!((run.claimed, run.succeeded, run.retried), (1, 0, 1));
        assert_pending(&saga(&auth, g.id).await?, &[]);
        assert!(receipt(&concert, g.id).await?["completed_at"].is_null());
        let pending:i64=sqlx::query_scalar("select count(*) from concert_social_deletion_audience where saga_id=$1 and published_at is null")
            .bind(g.id).fetch_one(&concert).await?;
        assert!(
            pending > 0,
            "uncovered real serving membership must prevent Sharing ACK"
        );
        assert_eq!(ordinary(&concert).await?, before);
        // Explicit storage transition, not proof of a real WebSocket disconnect.
        sqlx::query("update concert_world_memberships set disconnected=true where event_id=$1")
            .bind(event)
            .execute(&concert)
            .await?;
        park(&auth, g.id).await?;
        due(&auth, g.id, "CONCERT_SHARING").await?;
        let run = s.user_deletion_worker.run_once(chrono::Utc::now()).await?;
        assert_eq!((run.claimed, run.succeeded, run.retried), (1, 1, 0));
        assert_pending(
            &saga(&auth, g.id).await?,
            &[UserDeletionTarget::ConcertSharing],
        );
        assert_removed(&concert, event, user, peer).await?;
        let committed = receipt(&concert, g.id).await?;
        assert!(!committed["completed_at"].is_null());
        apply(&d, &g, Target::ConcertSharing).await?;
        assert_eq!(receipt(&concert, g.id).await?, committed);
        assert_eq!(ordinary(&concert).await?, before);
        cancellation(&auth, &concert, &rpc.url, unavailable, &s).await?;
        save_sharing(&SharingSaved {
            saga: g.id,
            event,
            user,
            peer,
            receipt: committed,
            ordinary: before,
            pid: std::process::id(),
            pg_started: sqlx::query_scalar("select pg_postmaster_start_time()::text")
                .fetch_one(&auth)
                .await?,
        })?;
        println!("SHARING FACTORY SOURCE PASS: real Auth worker/SOURCE/Apply, incumbent publisher refusal/retry, actual cleanup, cancellation non-ACK");
    }
    rpc.finish().await?;
    source.finish().await?;
    drop(policy);
    drop(d);
    drop(s);
    auth.close().await;
    concert.close().await;
    Ok(())
}

async fn cancellation(
    auth: &PgPool,
    concert: &PgPool,
    rpc: &str,
    notification: &str,
    s: &AppState,
) -> anyhow::Result<()> {
    let user = identity(s, auth).await?;
    let g = delete(s, auth, user).await?;
    park(auth, g.id).await?;
    due(auth, g.id, "CONCERT_SHARING").await?;
    let mut blocker = concert.begin().await?;
    common::lock_user(&mut blocker, g.user_id).await?;
    let bound = state(auth.clone(), rpc, notification, 180).await?;
    let w = bound.user_deletion_worker.clone();
    let task = tokio::spawn(async move { w.run_once(chrono::Utc::now()).await });
    // Observe actual SQL reached from factory RPC before cancelling; a sleeping
    // client or a sent request alone is insufficient transaction evidence.
    tokio::time::timeout(Duration::from_secs(2),async{loop{let wait:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and wait_event='advisory' and query like '%mcr-world-account-deletion:%')").fetch_one(concert).await.unwrap();if wait{break;}tokio::time::sleep(Duration::from_millis(2)).await;}}).await?;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::sleep(Duration::from_millis(220)).await;
    blocker.rollback().await?;
    tokio::time::timeout(Duration::from_secs(2),async{loop{let live:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and xact_start is not null)").fetch_one(concert).await.unwrap();if !live{break;}tokio::time::sleep(Duration::from_millis(2)).await;}}).await?;
    let count:i64=sqlx::query_scalar("select (select count(*) from concert_world_account_deletion_fences where user_id=$1)+(select count(*) from concert_social_account_deletions where saga_id=$2)").bind(g.user_id).bind(g.id).fetch_one(concert).await?;
    assert_eq!(count, 0);
    assert_pending(&saga(auth, g.id).await?, &[]);
    // Expired dispatch never settles it; a new bounded dispatch can commit and
    // the actual durable worker recovers its lease without duplicating effects.
    let d = runtime_dispatcher(rpc, notification, 5000)?;
    assert!(platform::call_deadline::scope(
        std::time::Instant::now(),
        apply(&d, &g, Target::ConcertSharing)
    )
    .await
    .is_err());
    let run = bound
        .user_deletion_worker
        .run_once(chrono::Utc::now() + chrono::Duration::seconds(2))
        .await?;
    assert_eq!(run.succeeded, 1);
    assert_pending(
        &saga(auth, g.id).await?,
        &[UserDeletionTarget::ConcertSharing],
    );
    park(auth, g.id).await?;
    Ok(())
}
