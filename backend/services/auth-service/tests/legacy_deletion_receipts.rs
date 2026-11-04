//! PROFILE/FRIEND/POST canonical Auth worker acceptance, separate actual RPCs/stores.
#[path = "../../../scripts/mcr/legacy_deletion_owners/owned.rs"]
mod owned;
#[path = "../../../scripts/mcr/legacy_deletion_owners/seed.rs"]
mod seed;
use auth_service::{
    adapters::{
        out::{
            grpc::user_deletion_dispatcher::{
                friend_proto, post_proto, profile_proto, verify_friend_deletion_receipt,
                verify_post_deletion_receipt, verify_profile_deletion_receipt,
                GrpcUserDeletionDispatcher, UserDeletionDispatchRequest,
            },
            persistence::{
                user_deletion_saga_repository::PgUserDeletionSagaRepository,
                user_repository::{PgAuthAuditRepository, PgUserRepository},
            },
            runtime::SystemAuthRuntime,
        },
        r#in::messaging::user_deletion_worker::{UserDeletionWorker, UserDeletionWorkerPolicy},
    },
    application::{
        dto::{auth_dto::DeleteUserDto, deletion_dto::AuthUserDeletionTargetDto as Target},
        errors::AuthError,
        ports::{UserDeletionDispatcherPort, UserDeletionSagaPort},
        use_cases::delete_user::DeleteUserUseCase,
    },
    domain::user_deletion::{UserDeletionSaga, UserDeletionStepStatus, UserDeletionTarget},
};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use platform::{grpc_auth::Workload, grpc_tls::TlsMaterial};
use sqlx::PgPool;
use std::{os::unix::fs::OpenOptionsExt, path::PathBuf, sync::Arc};
use uuid::Uuid;
fn root() -> anyhow::Result<PathBuf> {
    Ok(std::env::var("LEGACY_TEST_OWNER_ROOT")?.into())
}
fn tls() -> anyhow::Result<TlsMaterial> {
    let p = root()?.join("pki");
    TlsMaterial::load(p.join("auth.crt"), p.join("auth.key"), p.join("ca.crt"))
}
fn dispatch() -> anyhow::Result<GrpcUserDeletionDispatcher> {
    GrpcUserDeletionDispatcher::connect(
        std::fs::read_to_string(root()?.join("profile-endpoint"))?,
        std::fs::read_to_string(root()?.join("friend-endpoint"))?,
        std::fs::read_to_string(root()?.join("post-endpoint"))?,
        "https://127.0.0.1:9".into(),
        "be42-legacy-auth",
        Some(&tls()?),
    )
}
fn command() -> UserDeletionDispatchRequest {
    UserDeletionDispatchRequest {
        saga_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        actor_id: Uuid::new_v4(),
        retention_until: DateTime::from_timestamp(1800000000, 123456789).unwrap(),
    }
}
async fn apply(
    d: &GrpcUserDeletionDispatcher,
    c: &UserDeletionDispatchRequest,
    target: Target,
) -> Result<(), AuthError> {
    d.apply(target, c.saga_id, c.user_id, c.actor_id, c.retention_until)
        .await
}
fn assert_steps(s: &UserDeletionSaga, expected: usize) {
    assert_eq!(s.steps.len(), 8);
    assert!(s.completed_at.is_none());
    assert_eq!(
        s.steps
            .iter()
            .filter(|s| s.status == UserDeletionStepStatus::Succeeded)
            .count(),
        expected
    );
    for step in &s.steps {
        if !matches!(
            step.target,
            UserDeletionTarget::Profile | UserDeletionTarget::Friend | UserDeletionTarget::Post
        ) {
            assert_ne!(step.status, UserDeletionStepStatus::Succeeded);
            assert!(step.completed_at.is_none());
        }
    }
}

#[tokio::test]
#[ignore = "three actual separately running owner RPCs and fresh private PG16 offer required"]
async fn actual_eight_step_worker_commits_only_three_real_owners_and_recovers_rollback(
) -> anyhow::Result<()> {
    let p = pool().await?;
    let d = dispatch()?;
    let c = begin(&p).await?;
    let mut peers = Vec::new();
    for owner in ["profile", "friend", "post"] {
        let store = owned::owned_pool(&format!("mcr_legacy_{owner}")).await?;
        let peer = seed::populate(&store, owner, c.user_id).await?;
        peers.push((owner, store, peer));
    }
    let profile = &peers[0].1;
    sqlx::raw_sql("create function auth_fixture_rollback() returns trigger language plpgsql as $$ begin raise exception 'controlled owner rollback'; end $$; create trigger auth_fixture_rollback before insert on profile_deletion_receipts_v1 for each row execute function auth_fixture_rollback();").execute(profile).await?;
    let result = worker(&p, d.clone()).run_once(Utc::now()).await?;
    assert_eq!(result.succeeded, 2);
    let s = saga(&p, c.saga_id).await?;
    assert_steps(&s, 2);
    let profile_step = s
        .steps
        .iter()
        .find(|s| s.target == UserDeletionTarget::Profile)
        .unwrap();
    assert_ne!(profile_step.status, UserDeletionStepStatus::Succeeded);
    assert!(profile_step.completed_at.is_none());
    let count: i64 =
        sqlx::query_scalar("select count(*) from profile_deletion_receipts_v1 where saga_id=$1")
            .bind(c.saga_id)
            .fetch_one(profile)
            .await?;
    assert_eq!(count, 0);
    let clean: bool = sqlx::query_scalar("select profile_deletion_clean_v1($1)")
        .bind(c.user_id)
        .fetch_one(profile)
        .await?;
    assert!(!clean);
    sqlx::raw_sql("drop trigger auth_fixture_rollback on profile_deletion_receipts_v1; drop function auth_fixture_rollback();").execute(profile).await?;
    sqlx::query("update user_deletion_steps set next_attempt_at=clock_timestamp()-interval '1 second' where saga_id=$1 and target='PROFILE'").bind(c.saga_id).execute(&p).await?;
    assert_eq!(
        worker(&p, d.clone()).run_once(Utc::now()).await?.succeeded,
        1
    );
    assert_steps(&saga(&p, c.saga_id).await?, 3);
    let mut receipts = serde_json::Map::new();
    for (owner, store, peer) in peers {
        let clean: bool = sqlx::query_scalar(&format!("select {owner}_deletion_clean_v1($1)"))
            .bind(c.user_id)
            .fetch_one(&store)
            .await?;
        assert!(clean);
        seed::assert_peer_survives(&store, owner, peer).await?;
        let receipt: serde_json::Value = sqlx::query_scalar(&format!(
            "select to_jsonb(r) from {owner}_deletion_receipts_v1 r where saga_id=$1"
        ))
        .bind(c.saga_id)
        .fetch_one(&store)
        .await?;
        receipts.insert(owner.into(), receipt);
        store.close().await;
    }
    for target in [Target::Profile, Target::Friend, Target::Post] {
        apply(&d, &c, target).await?;
    }
    let checkpoint = serde_json::json!({"saga":c.saga_id,"user":c.user_id,"actor":c.actor_id,"retention":c.retention_until.to_rfc3339(),"receipts":receipts});
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(root()?.join("legacy-checkpoint.json"))?;
    file.write_all(&serde_json::to_vec(&checkpoint)?)?;
    file.sync_all()?;
    p.close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "controller must physically restart PostgreSQL and all three actual RPC processes first"]
async fn actual_restart_preserves_all_three_receipts_and_five_incomplete_steps(
) -> anyhow::Result<()> {
    let p = pool().await?;
    let d = dispatch()?;
    let cp: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root()?.join("legacy-checkpoint.json"))?)?;
    let c = UserDeletionDispatchRequest {
        saga_id: Uuid::parse_str(cp["saga"].as_str().unwrap())?,
        user_id: Uuid::parse_str(cp["user"].as_str().unwrap())?,
        actor_id: Uuid::parse_str(cp["actor"].as_str().unwrap())?,
        retention_until: DateTime::parse_from_rfc3339(cp["retention"].as_str().unwrap())?
            .with_timezone(&Utc),
    };
    for (owner, target) in [
        ("profile", Target::Profile),
        ("friend", Target::Friend),
        ("post", Target::Post),
    ] {
        apply(&d, &c, target).await?;
        let mut rebound = c.clone();
        rebound.retention_until += ChronoDuration::nanoseconds(1);
        assert!(apply(&d, &rebound, target).await.is_err());
        let store = owned::owned_pool(&format!("mcr_legacy_{owner}")).await?;
        let actual: serde_json::Value = sqlx::query_scalar(&format!(
            "select to_jsonb(r) from {owner}_deletion_receipts_v1 r where saga_id=$1"
        ))
        .bind(c.saga_id)
        .fetch_one(&store)
        .await?;
        assert_eq!(actual, cp["receipts"][owner]);
        let clean: bool = sqlx::query_scalar(&format!("select {owner}_deletion_clean_v1($1)"))
            .bind(c.user_id)
            .fetch_one(&store)
            .await?;
        assert!(clean);
        store.close().await;
    }
    assert_steps(&saga(&p, c.saga_id).await?, 3);
    p.close().await;
    Ok(())
}
async fn pool() -> anyhow::Result<PgPool> {
    // The existing marker/inode/socket guard authorizes this second isolated DB;
    // it contains only Auth's migrations, never Concert tables or live data.
    let p = owned::owned_pool("mcr_legacy_auth").await?;
    migration_runtime::prepare_database(
        &p,
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations"),
        migration_runtime::MigrationMode::Startup,
        &[],
    )
    .await?;
    Ok(p)
}
async fn begin(p: &PgPool) -> anyhow::Result<UserDeletionDispatchRequest> {
    let user = Uuid::new_v4();
    sqlx::query("insert into users(id,primary_email,password_hash,status) values($1,$2,'synthetic','ACTIVE')").bind(user).bind(format!("{user}@example.invalid")).execute(p).await?;
    let response = DeleteUserUseCase::new(
        Arc::new(PgUserRepository::new(
            p.clone(),
            Arc::new(SystemAuthRuntime),
        )),
        Arc::new(PgAuthAuditRepository::new(p.clone())),
        PgUserDeletionSagaRepository::new(p.clone()),
        Arc::new(SystemAuthRuntime),
    )
    .execute(DeleteUserDto {
        user_id: user.to_string(),
        actor_id: user.to_string(),
        request_id: None,
        ip_address: None,
        user_agent: None,
    })
    .await?;
    let repo = PgUserDeletionSagaRepository::new(p.clone());
    let saga: UserDeletionSaga = repo
        .find_by_saga_id(Uuid::parse_str(&response.saga_id)?)
        .await?
        .unwrap()
        .into();
    assert_eq!(saga.steps.len(), 8);
    // Bounded scheduling fixture only: keep the other seven real required steps
    // pending. This does not fake their owner responses or remove inventory.
    Ok(UserDeletionDispatchRequest {
        saga_id: saga.id,
        user_id: saga.user_id,
        actor_id: saga.actor_id,
        retention_until: saga.retention_until,
    })
}
async fn saga(p: &PgPool, id: Uuid) -> anyhow::Result<UserDeletionSaga> {
    Ok(PgUserDeletionSagaRepository::new(p.clone())
        .find_by_saga_id(id)
        .await?
        .unwrap()
        .into())
}
fn worker(
    p: &PgPool,
    d: GrpcUserDeletionDispatcher,
) -> UserDeletionWorker<PgUserDeletionSagaRepository, GrpcUserDeletionDispatcher> {
    UserDeletionWorker::new(
        PgUserDeletionSagaRepository::new(p.clone()),
        d,
        UserDeletionWorkerPolicy {
            batch_size: 100,
            max_attempts: 8,
            lease_timeout: ChronoDuration::seconds(1),
            base_retry: ChronoDuration::seconds(1),
        },
    )
}

#[test]
fn profile_binding_rejects_boolean_partial_owner_and_every_rebound() {
    let c = command();
    let valid = profile_proto::ApplyUserDeletionResponse {
        applied: true,
        committed_receipt: Some(profile_proto::CommittedDeletionReceipt {
            version: 1,
            owner: "PROFILE".into(),
            saga_id: c.saga_id.to_string(),
            user_id: c.user_id.to_string(),
            actor_id: c.actor_id.to_string(),
            retention_seconds: Some(c.retention_until.timestamp()),
            retention_nanos: Some(c.retention_until.timestamp_subsec_nanos()),
            fence_revision: 7,
            effect_xid: "12345".into(),
            recorded_at: "2026-11-04T15:00:00Z".into(),
        }),
    };
    assert!(verify_profile_deletion_receipt(&c, &valid).is_ok()); // validator only, no owning commit claim
    for kind in 0..23 {
        let mut r = valid.clone();
        match kind {
            0 => r.applied = false,
            1 => r.committed_receipt = None,
            2 => r.committed_receipt = Some(Default::default()),
            n => {
                let v = r.committed_receipt.as_mut().unwrap();
                match n {
                    3 => v.version = 0,
                    4 => v.version = 2,
                    5 => v.saga_id = Uuid::new_v4().to_string(),
                    6 => v.user_id = Uuid::new_v4().to_string(),
                    7 => v.actor_id = Uuid::new_v4().to_string(),
                    8 => v.retention_seconds = Some(c.retention_until.timestamp() + 1),
                    9 => v.retention_nanos = Some(c.retention_until.timestamp_subsec_nanos() + 1),
                    10 => v.retention_nanos = Some(1000000000),
                    11 => v.fence_revision = 0,
                    12 => v.fence_revision = -1,
                    13 => v.effect_xid.clear(),
                    14 => v.effect_xid = "0".into(),
                    15 => v.effect_xid = "18446744073709551616".into(),
                    16 => v.effect_xid = "012345".into(),
                    17 => v.recorded_at.clear(),
                    18 => v.saga_id = Uuid::nil().to_string(),
                    19 => v.retention_seconds = None,
                    20 => v.retention_nanos = None,
                    21 => v.owner = "OTHER".into(),
                    _ => v.owner.clear(),
                }
            }
        };
        let error = verify_profile_deletion_receipt(&c, &r).unwrap_err();
        assert_eq!(
            error.downcast_ref::<AuthError>(),
            Some(&AuthError::DependencyUnavailable),
            "case {kind}"
        );
        assert!(!error.to_string().contains(&c.user_id.to_string()));
    }
    for (seconds, nanos) in [(-1, 999999999), (946684799, 123456789), (1800000000, 0)] {
        let mut c = c.clone();
        c.retention_until = DateTime::from_timestamp(seconds, nanos).unwrap();
        let mut r = valid.clone();
        let v = r.committed_receipt.as_mut().unwrap();
        v.retention_seconds = Some(seconds);
        v.retention_nanos = Some(nanos);
        assert!(verify_profile_deletion_receipt(&c, &r).is_ok());
        r.committed_receipt.as_mut().unwrap().retention_nanos = Some(nanos.saturating_add(1));
        assert!(verify_profile_deletion_receipt(&c, &r).is_err());
    }
}

#[test]
fn friend_binding_rejects_boolean_partial_owner_and_every_rebound() {
    let c = command();
    let valid = friend_proto::ApplyUserDeletionResponse {
        applied: true,
        committed_receipt: Some(friend_proto::CommittedDeletionReceipt {
            version: 1,
            owner: "FRIEND".into(),
            saga_id: c.saga_id.to_string(),
            user_id: c.user_id.to_string(),
            actor_id: c.actor_id.to_string(),
            retention_seconds: Some(c.retention_until.timestamp()),
            retention_nanos: Some(c.retention_until.timestamp_subsec_nanos()),
            fence_revision: 7,
            effect_xid: "12345".into(),
            recorded_at: "2026-11-04T15:00:00Z".into(),
        }),
    };
    assert!(verify_friend_deletion_receipt(&c, &valid).is_ok()); // validator only, no owning commit claim
    for kind in 0..23 {
        let mut r = valid.clone();
        match kind {
            0 => r.applied = false,
            1 => r.committed_receipt = None,
            2 => r.committed_receipt = Some(Default::default()),
            n => {
                let v = r.committed_receipt.as_mut().unwrap();
                match n {
                    3 => v.version = 0,
                    4 => v.version = 2,
                    5 => v.saga_id = Uuid::new_v4().to_string(),
                    6 => v.user_id = Uuid::new_v4().to_string(),
                    7 => v.actor_id = Uuid::new_v4().to_string(),
                    8 => v.retention_seconds = Some(c.retention_until.timestamp() + 1),
                    9 => v.retention_nanos = Some(c.retention_until.timestamp_subsec_nanos() + 1),
                    10 => v.retention_nanos = Some(1000000000),
                    11 => v.fence_revision = 0,
                    12 => v.fence_revision = -1,
                    13 => v.effect_xid.clear(),
                    14 => v.effect_xid = "0".into(),
                    15 => v.effect_xid = "18446744073709551616".into(),
                    16 => v.effect_xid = "012345".into(),
                    17 => v.recorded_at.clear(),
                    18 => v.saga_id = Uuid::nil().to_string(),
                    19 => v.retention_seconds = None,
                    20 => v.retention_nanos = None,
                    21 => v.owner = "OTHER".into(),
                    _ => v.owner.clear(),
                }
            }
        };
        let error = verify_friend_deletion_receipt(&c, &r).unwrap_err();
        assert_eq!(
            error.downcast_ref::<AuthError>(),
            Some(&AuthError::DependencyUnavailable),
            "case {kind}"
        );
        assert!(!error.to_string().contains(&c.user_id.to_string()));
    }
    for (seconds, nanos) in [(-1, 999999999), (946684799, 123456789), (1800000000, 0)] {
        let mut c = c.clone();
        c.retention_until = DateTime::from_timestamp(seconds, nanos).unwrap();
        let mut r = valid.clone();
        let v = r.committed_receipt.as_mut().unwrap();
        v.retention_seconds = Some(seconds);
        v.retention_nanos = Some(nanos);
        assert!(verify_friend_deletion_receipt(&c, &r).is_ok());
        r.committed_receipt.as_mut().unwrap().retention_nanos = Some(nanos.saturating_add(1));
        assert!(verify_friend_deletion_receipt(&c, &r).is_err());
    }
}

#[test]
fn post_binding_rejects_boolean_partial_owner_and_every_rebound() {
    let c = command();
    let valid = post_proto::ApplyUserDeletionResponse {
        applied: true,
        committed_receipt: Some(post_proto::CommittedDeletionReceipt {
            version: 1,
            owner: "POST".into(),
            saga_id: c.saga_id.to_string(),
            user_id: c.user_id.to_string(),
            actor_id: c.actor_id.to_string(),
            retention_seconds: Some(c.retention_until.timestamp()),
            retention_nanos: Some(c.retention_until.timestamp_subsec_nanos()),
            fence_revision: 7,
            effect_xid: "12345".into(),
            recorded_at: "2026-11-04T15:00:00Z".into(),
        }),
    };
    assert!(verify_post_deletion_receipt(&c, &valid).is_ok()); // validator only, no owning commit claim
    for kind in 0..23 {
        let mut r = valid.clone();
        match kind {
            0 => r.applied = false,
            1 => r.committed_receipt = None,
            2 => r.committed_receipt = Some(Default::default()),
            n => {
                let v = r.committed_receipt.as_mut().unwrap();
                match n {
                    3 => v.version = 0,
                    4 => v.version = 2,
                    5 => v.saga_id = Uuid::new_v4().to_string(),
                    6 => v.user_id = Uuid::new_v4().to_string(),
                    7 => v.actor_id = Uuid::new_v4().to_string(),
                    8 => v.retention_seconds = Some(c.retention_until.timestamp() + 1),
                    9 => v.retention_nanos = Some(c.retention_until.timestamp_subsec_nanos() + 1),
                    10 => v.retention_nanos = Some(1000000000),
                    11 => v.fence_revision = 0,
                    12 => v.fence_revision = -1,
                    13 => v.effect_xid.clear(),
                    14 => v.effect_xid = "0".into(),
                    15 => v.effect_xid = "18446744073709551616".into(),
                    16 => v.effect_xid = "012345".into(),
                    17 => v.recorded_at.clear(),
                    18 => v.saga_id = Uuid::nil().to_string(),
                    19 => v.retention_seconds = None,
                    20 => v.retention_nanos = None,
                    21 => v.owner = "OTHER".into(),
                    _ => v.owner.clear(),
                }
            }
        };
        let error = verify_post_deletion_receipt(&c, &r).unwrap_err();
        assert_eq!(
            error.downcast_ref::<AuthError>(),
            Some(&AuthError::DependencyUnavailable),
            "case {kind}"
        );
        assert!(!error.to_string().contains(&c.user_id.to_string()));
    }
    for (seconds, nanos) in [(-1, 999999999), (946684799, 123456789), (1800000000, 0)] {
        let mut c = c.clone();
        c.retention_until = DateTime::from_timestamp(seconds, nanos).unwrap();
        let mut r = valid.clone();
        let v = r.committed_receipt.as_mut().unwrap();
        v.retention_seconds = Some(seconds);
        v.retention_nanos = Some(nanos);
        assert!(verify_post_deletion_receipt(&c, &r).is_ok());
        r.committed_receipt.as_mut().unwrap().retention_nanos = Some(nanos.saturating_add(1));
        assert!(verify_post_deletion_receipt(&c, &r).is_err());
    }
}
