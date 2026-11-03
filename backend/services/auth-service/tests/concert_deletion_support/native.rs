use super::*;
use auth_application::dto::auth_dto::DeleteUserDto;
use auth_service::adapters::out::persistence::user_deletion_saga_repository::PgUserDeletionSagaRepository;
use auth_service::adapters::out::persistence::user_repository::{
    PgAuthAuditRepository, PgUserRepository,
};
use auth_service::adapters::out::runtime::SystemAuthRuntime;
use auth_service::adapters::r#in::messaging::user_deletion_worker::{
    UserDeletionWorker, UserDeletionWorkerPolicy,
};
use auth_service::domain::user_deletion::{
    BeginUserDeletion, RetryUserDeletion, UserDeletionSaga, UserDeletionSagaStatus,
    UserDeletionStep, UserDeletionStepStatus, UserDeletionTarget,
};
use sqlx::{PgPool, Row};

async fn pool() -> anyhow::Result<PgPool> {
    // No Docker/default URL fallback. The runner owns this entire fresh cluster.
    let url = std::env::var("MCR_DELETION_TEST_DATABASE_URL")?;
    let p = PgPool::connect(&url).await?;
    let identity: String =
        sqlx::query_scalar("select system_identifier::text from pg_control_system()")
            .fetch_one(&p)
            .await?;
    anyhow::ensure!(
        identity == std::env::var("MCR_DELETION_TEST_SYSTEM_IDENTIFIER")?,
        "foreign cluster"
    );
    let row = sqlx::query("select current_database() db, current_setting('server_version_num')::int version, current_setting('listen_addresses') listeners").fetch_one(&p).await?;
    anyhow::ensure!(
        row.get::<String, _>("db") == "mcr_account_deletion"
            && row.get::<i32, _>("version") / 10000 == 16
            && row.get::<String, _>("listeners").is_empty(),
        "unsafe native fixture"
    );
    Ok(p)
}
async fn migrate(p: &PgPool, legacy_only: bool) -> anyhow::Result<()> {
    let mut m = sqlx::migrate::Migrator::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations")
            .as_path(),
    )
    .await?;
    if legacy_only {
        m.migrations = std::borrow::Cow::Owned(
            m.migrations
                .into_owned()
                .into_iter()
                .filter(|m| m.version < 15)
                .collect(),
        );
    }
    // A failed SQLx migration can leave its session advisory lock held. Give
    // each invocation a detached connection so failure closes the session;
    // the forward-recovery attempt then models a fresh migrator process.
    let mut connection = p.acquire().await?.detach();
    m.run(&mut connection).await?;
    Ok(())
}
async fn user(p: &PgPool) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query("insert into users(id,primary_email,password_hash,status) values($1,$2,'synthetic','ACTIVE')").bind(id).bind(format!("{id}@example.invalid")).execute(p).await?;
    Ok(id)
}
async fn delete(
    p: &PgPool,
    id: Uuid,
) -> anyhow::Result<auth_application::dto::auth_dto::DeleteUserResponseDto> {
    Ok(
        auth_application::use_cases::delete_user::DeleteUserUseCase::new(
            Arc::new(PgUserRepository::new(
                p.clone(),
                Arc::new(SystemAuthRuntime),
            )),
            Arc::new(PgAuthAuditRepository::new(p.clone())),
            PgUserDeletionSagaRepository::new(p.clone()),
            Arc::new(SystemAuthRuntime),
        )
        .execute(DeleteUserDto {
            user_id: id.to_string(),
            actor_id: id.to_string(),
            request_id: None,
            ip_address: None,
            user_agent: None,
        })
        .await?,
    )
}
fn policy(max_attempts: i32) -> UserDeletionWorkerPolicy {
    UserDeletionWorkerPolicy {
        batch_size: 100,
        max_attempts,
        lease_timeout: Duration::seconds(1),
        base_retry: Duration::seconds(1),
    }
}
async fn saga(r: &PgUserDeletionSagaRepository, id: Uuid) -> anyhow::Result<UserDeletionSaga> {
    Ok(r.find_by_saga_id(id).await?.unwrap().into())
}

async fn canonical_inventory_guards(p: &PgPool) -> anyhow::Result<()> {
    let r = PgUserDeletionSagaRepository::new(p.clone());
    for targets in [
        vec![],
        vec![
            UserDeletionTarget::Profile,
            UserDeletionTarget::Friend,
            UserDeletionTarget::Post,
            UserDeletionTarget::Notification,
            UserDeletionTarget::Profile,
        ],
    ] {
        let id = user(p).await?;
        let now = Utc::now();
        let request = BeginUserDeletion {
            saga_id: Uuid::new_v4(),
            user_id: id,
            actor_id: id,
            reason: "synthetic legacy caller".into(),
            retention_until: now + Duration::days(30),
            targets,
            audit: auth_service::domain::session::AuthAuditEvent {
                id: Uuid::new_v4(),
                actor_id: Some(id),
                subject_user_id: Some(id),
                action: "USER_DELETION_REQUESTED".into(),
                outcome: "SUCCESS".into(),
                request_id: None,
                ip_address: None,
                user_agent: None,
                metadata_json: "{}".into(),
                occurred_at: now,
            },
        };
        let first: UserDeletionSaga = r.begin(request.clone().into()).await?.unwrap().into();
        assert_eq!(
            first.steps.len(),
            8,
            "empty/four/duplicate caller inventory must normalize"
        );
        let repeated: UserDeletionSaga = r.begin(request.into()).await?.unwrap().into();
        assert_eq!(repeated.id, first.id);
        assert_eq!(repeated.steps.len(), 8);
        // Corrupt the owned synthetic fixture to represent a malformed legacy
        // four-step saga; marking every surviving step must not complete it.
        sqlx::query("delete from user_deletion_steps where saga_id=$1 and target like 'CONCERT_%'")
            .bind(first.id)
            .execute(p)
            .await?;
        for step in r
            .claim_due(now + Duration::seconds(1), Duration::seconds(1), 100)
            .await?
        {
            r.mark_step_succeeded(&step, now + Duration::seconds(1))
                .await?;
        }
        let malformed = saga(&r, first.id).await?;
        assert_eq!(malformed.status, UserDeletionSagaStatus::InProgress);
        assert!(malformed.completed_at.is_none());
        // Also reject eight SUCCEEDED states when one timestamp is missing.
        sqlx::query("insert into user_deletion_steps(saga_id,target,status,completed_at) select $1,t,'SUCCEEDED',null from unnest(array['CONCERT_TICKETS','CONCERT_SESSIONS','CONCERT_SHARING','CONCERT_PRIVATE_DATA']) t").bind(first.id).execute(p).await?;
        sqlx::query("update user_deletion_steps set status='PENDING',next_attempt_at=$2 where saga_id=$1 and target='PROFILE'").bind(first.id).bind(now).execute(p).await?;
        let claimed = r
            .claim_due(now + Duration::seconds(2), Duration::seconds(1), 100)
            .await?;
        assert_eq!(claimed.len(), 1);
        r.mark_step_succeeded(&claimed[0], now + Duration::seconds(2))
            .await?;
        assert_eq!(
            saga(&r, first.id).await?.status,
            UserDeletionSagaStatus::InProgress
        );
        sqlx::query("update user_deletion_sagas set status='DEAD_LETTERED' where id=$1")
            .bind(first.id)
            .execute(p)
            .await?;
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires account_deletion_smoke.py owned PG16 runner"]
async fn canonical_upgrade_retry_recovery_and_partial_failure() -> anyhow::Result<()> {
    let p = pool().await?;
    migrate(&p, true).await?;
    assert!(
        sqlx::query_scalar::<_, bool>("select success from _sqlx_migrations where version=14")
            .fetch_one(&p)
            .await?,
        "upgrade must include published Auth0014"
    );
    // Three old four-target sagas must retain their state/steps while acquiring
    // required Concert work; COMPLETED must reopen rather than falsely certify.
    let mut old = Vec::new();
    for status in ["COMPLETED", "IN_PROGRESS", "DEAD_LETTERED"] {
        let id = user(&p).await?;
        let sid = Uuid::new_v4();
        sqlx::query("update users set status='DELETED' where id=$1")
            .bind(id)
            .execute(&p)
            .await?;
        sqlx::query("insert into user_deletion_sagas(id,user_id,actor_id,status,retention_until,completed_at) values($1,$2,$2,$3,now()+interval '30 days',case when $3='COMPLETED' then now() else null end)").bind(sid).bind(id).bind(status).execute(&p).await?;
        sqlx::query("insert into user_deletion_steps(saga_id,target,status,completed_at) select $1,t,'SUCCEEDED',now() from unnest(array['PROFILE','FRIEND','POST','NOTIFICATION']) t").bind(sid).execute(&p).await?;
        old.push((sid, status));
    }
    // Inject a controlled upgrade failure. SQLx must roll back the constraint
    // change/backfill/receipt together; forward recovery uses unchanged 0015.
    sqlx::query("alter table user_deletion_steps add constraint synthetic_upgrade_failure check (target <> 'CONCERT_PRIVATE_DATA')").execute(&p).await?;
    assert!(migrate(&p, false).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from user_deletion_steps where target like 'CONCERT_%'"
        )
        .fetch_one(&p)
        .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from _sqlx_migrations where version=15")
            .fetch_one(&p)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from user_deletion_sagas where status='COMPLETED'"
        )
        .fetch_one(&p)
        .await?,
        1
    );
    sqlx::query("alter table user_deletion_steps drop constraint synthetic_upgrade_failure")
        .execute(&p)
        .await?;
    migrate(&p, false).await?;
    migrate(&p, false).await?; // SQLx reapply is a no-op, not repeated backfill.
    let r = PgUserDeletionSagaRepository::new(p.clone());
    for (sid, status) in &old {
        let s = saga(&r, *sid).await?;
        assert_eq!(s.steps.len(), 8);
        assert_eq!(
            s.steps
                .iter()
                .filter(|s| s.status == UserDeletionStepStatus::Succeeded)
                .count(),
            4
        );
        assert_eq!(
            s.status,
            if *status == "DEAD_LETTERED" {
                UserDeletionSagaStatus::DeadLettered
            } else {
                UserDeletionSagaStatus::InProgress
            }
        );
        assert!(s.completed_at.is_none());
    }
    // Clear upgrade fixtures from work scheduling without hiding their rows.
    sqlx::query("update user_deletion_sagas set status='DEAD_LETTERED'")
        .execute(&p)
        .await?;
    canonical_inventory_guards(&p).await?;
    let id = user(&p).await?;
    sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,expires_at,family_id) values('native-deletion','synthetic',$1,now()+interval '1 hour',$1)").bind(id).execute(&p).await?;
    let accepted = delete(&p, id).await?;
    assert!(accepted.deleted);
    assert_eq!(accepted.saga.steps.len(), 8);
    let sid = Uuid::parse_str(&accepted.saga_id)?;
    assert_eq!(delete(&p, id).await?.saga_id, accepted.saga_id);
    let (a, b) = tokio::join!(delete(&p, id), delete(&p, id));
    assert_eq!(a?.saga_id, b?.saga_id);
    let revoked: bool = sqlx::query_scalar(
        "select revoked_at is not null from refresh_tokens where jti='native-deletion'",
    )
    .fetch_one(&p)
    .await?;
    assert!(revoked);
    assert_eq!(
        sqlx::query_scalar::<_, String>("select status from users where id=$1")
            .bind(id)
            .fetch_one(&p)
            .await?,
        "DELETED"
    );
    let owner = Arc::new(Owner::default());
    *owner.fail_sharing.lock().unwrap() = true;
    let dispatcher = ConcertDeletionDispatcher::new(
        Legacy,
        owners(owner.clone()),
        StdDuration::from_millis(50),
    )?;
    let t = Utc::now();
    let first = UserDeletionWorker::new(r.clone(), dispatcher.clone(), policy(2))
        .run_once(t)
        .await?;
    assert_eq!((first.succeeded, first.retried), (7, 1));
    let s = saga(&r, sid).await?;
    assert_eq!(s.status, UserDeletionSagaStatus::InProgress);
    assert_eq!(
        s.steps
            .iter()
            .filter(|s| s.status == UserDeletionStepStatus::Retry)
            .count(),
        1
    );
    assert!(s.steps.iter().all(|s| s
        .last_error
        .as_deref()
        .map_or(true, |e| e == "Dependency unavailable")));
    let second = UserDeletionWorker::new(r.clone(), dispatcher.clone(), policy(2))
        .run_once(t + Duration::seconds(2))
        .await?;
    assert_eq!(second.dead_lettered, 1);
    assert_eq!(
        saga(&r, sid).await?.status,
        UserDeletionSagaStatus::DeadLettered
    );
    let resolution = RetryUserDeletion {
        saga_id: sid,
        resolution_id: Uuid::new_v4(),
        actor_id: Uuid::new_v4(),
        reason: "Owner recovery verified for native qualification".into(),
        evidence_reference: "synthetic-native-owner".into(),
        occurred_at: t + Duration::seconds(3),
    };
    r.retry_dead_lettered(resolution.clone().into()).await?;
    r.retry_dead_lettered(resolution.clone().into()).await?;
    let stale: UserDeletionStep = r
        .claim_due(t + Duration::seconds(4), Duration::seconds(1), 100)
        .await?
        .into_iter()
        .next()
        .unwrap()
        .into();
    assert_eq!(stale.target, UserDeletionTarget::ConcertSharing);
    // Exercise the operator-generation fence with the same attempt count in
    // a new cycle, so attempt fencing alone cannot mask a regression.
    r.mark_step_failed(&(&stale).into(), "Dependency unavailable", t, true, t)
        .await?;
    let mut next_resolution = resolution.clone();
    next_resolution.resolution_id = Uuid::new_v4();
    next_resolution.occurred_at = t + Duration::seconds(4);
    r.retry_dead_lettered(next_resolution.into()).await?;
    let current: UserDeletionStep = r
        .claim_due(t + Duration::seconds(4), Duration::seconds(1), 100)
        .await?
        .into_iter()
        .next()
        .unwrap()
        .into();
    assert_eq!(current.attempt_count, stale.attempt_count);
    assert_ne!(current.retry_generation, stale.retry_generation);
    r.mark_step_succeeded(&(&stale).into(), t).await?;
    assert_eq!(
        saga(&r, sid).await?.status,
        UserDeletionSagaStatus::InProgress
    );
    assert_eq!(
        saga(&r, sid)
            .await?
            .steps
            .iter()
            .find(|s| s.target == UserDeletionTarget::ConcertSharing)
            .unwrap()
            .status,
        UserDeletionStepStatus::InProgress
    );
    // New connection/worker recovers an expired lease. Owner sees the same key.
    p.close().await;
    let p = pool().await?;
    let r = PgUserDeletionSagaRepository::new(p.clone());
    *owner.fail_sharing.lock().unwrap() = false;
    let third = UserDeletionWorker::new(r.clone(), dispatcher, policy(3))
        .run_once(t + Duration::seconds(6))
        .await?;
    assert_eq!(third.succeeded, 1);
    let finished = saga(&r, sid).await?;
    assert_eq!(finished.status, UserDeletionSagaStatus::Completed);
    assert!(finished
        .steps
        .iter()
        .all(|s| s.status == UserDeletionStepStatus::Succeeded && s.completed_at.is_some()));
    // An obsolete failed reply cannot overwrite the recovered success.
    r.mark_step_failed(&(&stale).into(), "Dependency unavailable", t, true, t)
        .await?;
    assert_eq!(
        saga(&r, sid).await?.status,
        UserDeletionSagaStatus::Completed
    );
    assert_eq!(owner.effects.lock().unwrap().len(), 4);
    // Persist missing-owner failure for a real postmaster restart assertion.
    let missing = user(&p).await?;
    delete(&p, missing).await?;
    let absent = ConcertDeletionDispatcher::new(
        Legacy,
        ConcertDeletionOwners::default(),
        StdDuration::from_millis(50),
    )?;
    let pending = UserDeletionWorker::new(r.clone(), absent, policy(3))
        .run_once(t + Duration::seconds(8))
        .await?;
    assert_eq!((pending.succeeded, pending.retried), (4, 4));
    assert_eq!(delete(&p, missing).await?.saga.status, "IN_PROGRESS");
    p.close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "runner executes only after actual owned postmaster restart"]
async fn restart_keeps_required_failures_and_success_receipts() -> anyhow::Result<()> {
    let p = pool().await?;
    migrate(&p, false).await?;
    let completed: i64 =
        sqlx::query_scalar("select count(*) from user_deletion_sagas where status='COMPLETED'")
            .fetch_one(&p)
            .await?;
    assert_eq!(completed, 1);
    let retry: i64 = sqlx::query_scalar("select count(*) from user_deletion_steps s join user_deletion_sagas g on g.id=s.saga_id where g.status='IN_PROGRESS' and s.status='RETRY' and s.target like 'CONCERT_%' and s.last_error='Dependency unavailable'").fetch_one(&p).await?;
    assert_eq!(retry, 4);
    let malformed: i64 = sqlx::query_scalar("select count(*) from user_deletion_sagas g where g.status='COMPLETED' and (select count(*) from user_deletion_steps s where s.saga_id=g.id and s.status='SUCCEEDED')<>8").fetch_one(&p).await?;
    assert_eq!(malformed, 0);
    // An applied source checksum mismatch must be refused, with forward
    // recovery using the original source and restored owned fixture receipt.
    let checksum: Vec<u8> =
        sqlx::query_scalar("select checksum from _sqlx_migrations where version=15")
            .fetch_one(&p)
            .await?;
    sqlx::query("update _sqlx_migrations set checksum=$1 where version=15")
        .bind(vec![0_u8])
        .execute(&p)
        .await?;
    assert!(migrate(&p, false).await.is_err());
    sqlx::query("update _sqlx_migrations set checksum=$1 where version=15")
        .bind(checksum)
        .execute(&p)
        .await?;
    migrate(&p, false).await?;
    p.close().await;
    Ok(())
}
