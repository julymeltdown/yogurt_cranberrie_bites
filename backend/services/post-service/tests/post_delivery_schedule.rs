//! Real PostgreSQL scheduling transactions. The default fixture is a container;
//! an explicitly supplied disposable loopback database can provision separate
//! databases per test when the container runtime is unavailable.
use anyhow::{ensure, Result};
use chrono::{Duration, Utc};
use post_service::adapters::out::persistence::{
    delivery_schedule::PgDeliverySchedule, event_outbox::PgEventOutbox,
};
use post_service::application::dto::delivery_schedule_dto::{
    DeliveryLeaseOutcomeDto, DeliveryScheduleConfigDto, TargetDeliveryClaimDto,
};
use post_service::application::ports::{DeliverySchedulePort, EventOutboxPort};
use post_service::domain::post::{DeliveryTarget, StoredEvent};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Row};
use std::{net::IpAddr, path::PathBuf};
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

#[path = "unit/support.rs"]
#[allow(dead_code)]
mod ports;

#[path = "post_delivery_schedule/affinity_baseline.rs"]
mod affinity_baseline;
#[path = "post_delivery_schedule/affinity_dependency.rs"]
mod affinity_dependency;
#[path = "post_delivery_schedule/affinity_provenance.rs"]
mod affinity_provenance;
#[path = "post_delivery_schedule/affinity_resolution.rs"]
mod affinity_resolution;
#[path = "post_delivery_schedule/affinity_source.rs"]
mod affinity_source;
#[path = "post_delivery_schedule/cache_authority.rs"]
mod cache_authority;
#[path = "post_delivery_schedule/delivery_authority.rs"]
mod delivery_authority;
#[path = "post_delivery_schedule/fanout.rs"]
mod fanout;
#[path = "post_delivery_schedule/fanout_authority.rs"]
mod fanout_authority;
#[path = "post_delivery_schedule/fresh_provisioning.rs"]
mod fresh_provisioning;
#[path = "post_delivery_schedule/inspection.rs"]
mod inspection;
#[path = "post_delivery_schedule/notification_authority.rs"]
mod notification_authority;
#[path = "post_delivery_schedule/projection.rs"]
mod projection;
#[path = "post_delivery_schedule/recommendation_admission.rs"]
mod recommendation_admission;
#[path = "post_delivery_schedule/recommendation_retry.rs"]
mod recommendation_retry;
#[path = "post_delivery_schedule/replay.rs"]
mod replay;
#[path = "post_delivery_schedule/runtime_grants.rs"]
mod runtime_grants;
#[path = "post_delivery_schedule/worker.rs"]
mod worker;

#[tokio::test]
async fn recovery_boundary_crashed_final_attempt_resolves_without_parent_summary() -> Result<()> {
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let config = DeliveryScheduleConfigDto {
        max_attempts: 1,
        ..Default::default()
    };
    let crashed = f.claim(DeliveryTarget::Internal, &config).await?;
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target='INTERNAL'").bind(event).execute(&f.pool).await?;
    assert!(
        sqlx::query_scalar::<_, bool>(
            "select dead_lettered_at is null from post_event_outbox where event_id=$1"
        )
        .bind(event)
        .fetch_one(&f.pool)
        .await?
    );
    assert!(
        f.resolve(event, Uuid::new_v4()).await?.created,
        "authoritative exhausted claim needs no summary-maintenance tick"
    );
    assert_eq!(
        f.schedule().complete_target(&crashed).await?,
        DeliveryLeaseOutcomeDto::Lost
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_attempts where attempt_id=$1 and phase<>'STARTED'").bind(crashed.attempt_id).fetch_one(&f.pool).await?,0);
    let retry = f.claim(DeliveryTarget::Internal, &config).await?;
    assert_eq!(retry.attempt_number, 1);
    assert_ne!(retry.lease_token, crashed.lease_token);
    f.shutdown().await
}

#[tokio::test]
async fn recovery_boundary_nonrequired_dead_job_cannot_trigger_operator_release() -> Result<()> {
    use post_service::domain::post::DeadLetterReplayError;
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let claim = f
        .claim(DeliveryTarget::Timeline, &Default::default())
        .await?;
    f.schedule().complete_target(&claim).await?;
    // An excluded job is deliberately corrupted to test that the frozen mask,
    // not the parent summary or an arbitrary DEAD row, authorizes recovery.
    sqlx::query("update post_event_delivery_jobs set state='DEAD',attempt_count=5 where event_id=$1 and target='EXTERNAL'").bind(event).execute(&f.pool).await?;
    sqlx::query("update post_event_outbox set dead_lettered_at=now() where event_id=$1")
        .bind(event)
        .execute(&f.pool)
        .await?;
    let error = f
        .resolve(event, Uuid::new_v4())
        .await
        .expect_err("nonrequired targets cannot release the event");
    assert!(matches!(
        error.downcast_ref::<DeadLetterReplayError>(),
        Some(DeadLetterReplayError::NotDeadLettered)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from post_event_dead_letter_resolutions")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    f.shutdown().await
}

impl Fixture {
    async fn resolve(
        &self,
        event: Uuid,
        resolution: Uuid,
    ) -> Result<post_service::application::dto::post_dto::ResolveDeadLetterReplayResponseDto> {
        use post_service::application::dto::post_dto::ResolveDeadLetterReplayRequestDto;
        use post_service::application::use_cases::resolve_dead_letter_replay::ResolveDeadLetterReplayUseCase;
        ResolveDeadLetterReplayUseCase::new(std::sync::Arc::new(PgEventOutbox::new(
            self.pool.clone(),
        )))
        .execute(ResolveDeadLetterReplayRequestDto {
            event_id: event.to_string(),
            resolution_id: resolution.to_string(),
            actor_id: Uuid::nil().to_string(),
            reason: "Operator inspected delivery evidence".into(),
            evidence_reference: "fixture:recovery".into(),
        })
        .await
    }

    async fn due(&self, event: Uuid, target: DeliveryTarget) -> Result<()> {
        sqlx::query("update post_event_delivery_jobs set next_attempt_at=clock_timestamp()-interval '1 second' where event_id=$1 and target=$2")
            .bind(event).bind(target.as_str()).execute(&self.pool).await?;
        Ok(())
    }
}

#[tokio::test]
async fn recovery_resolution_rejects_live_target_lease() -> Result<()> {
    use post_service::domain::post::DeadLetterReplayError;
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let config = DeliveryScheduleConfigDto {
        external_enabled: true,
        max_attempts: 1,
        ..Default::default()
    };
    let failed = f.claim(DeliveryTarget::External, &config).await?;
    f.schedule().fail_target(&failed).await?;
    let live = f.claim(DeliveryTarget::Internal, &config).await?;
    let error = f
        .resolve(event, Uuid::new_v4())
        .await
        .expect_err("resolution must refuse a live target lease");
    assert!(matches!(
        error.downcast_ref::<DeadLetterReplayError>(),
        Some(DeadLetterReplayError::Conflict)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from post_event_dead_letter_resolutions")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    assert_eq!(sqlx::query_scalar::<_,Uuid>("select lease_token from post_event_delivery_jobs where event_id=$1 and target='INTERNAL'").bind(event).fetch_one(&f.pool).await?,live.lease_token);
    f.shutdown().await
}

#[tokio::test]
async fn recovery_resolution_releases_only_dead_target_and_preserves_healthy_state() -> Result<()> {
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let config = DeliveryScheduleConfigDto {
        external_enabled: true,
        max_attempts: 2,
        ..Default::default()
    };
    let done = f.claim(DeliveryTarget::Timeline, &config).await?;
    f.schedule().complete_target(&done).await?;
    let healthy = f.claim(DeliveryTarget::Internal, &config).await?;
    f.schedule().fail_target(&healthy).await?;
    for _ in 0..2 {
        f.due(event, DeliveryTarget::External).await?;
        let failed = f.claim(DeliveryTarget::External, &config).await?;
        f.schedule().fail_target(&failed).await?;
    }
    let before: serde_json::Value = sqlx::query_scalar("select jsonb_agg(to_jsonb(j) order by target) from post_event_delivery_jobs j where event_id=$1 and target<>'EXTERNAL'").bind(event).fetch_one(&f.pool).await?;
    let history: serde_json::Value = sqlx::query_scalar(
        "select jsonb_agg(to_jsonb(a) order by id) from post_event_delivery_attempts a",
    )
    .fetch_one(&f.pool)
    .await?;
    let mask: Vec<String> = sqlx::query_scalar(
        "select delivery_required_targets from post_event_outbox where event_id=$1",
    )
    .bind(event)
    .fetch_one(&f.pool)
    .await?;
    let resolution = Uuid::new_v4();
    assert!(f.resolve(event, resolution).await?.created);
    let released = sqlx::query("select state,attempt_count,release_resolution_id from post_event_delivery_jobs where event_id=$1 and target='EXTERNAL'").bind(event).fetch_one(&f.pool).await?;
    assert_eq!(released.get::<String, _>("state"), "PENDING");
    assert_eq!(released.get::<i32, _>("attempt_count"), 0);
    assert_eq!(
        released.get::<Option<Uuid>, _>("release_resolution_id"),
        Some(resolution)
    );
    let after: serde_json::Value = sqlx::query_scalar("select jsonb_agg(to_jsonb(j) order by target) from post_event_delivery_jobs j where event_id=$1 and target<>'EXTERNAL'").bind(event).fetch_one(&f.pool).await?;
    assert_eq!(
        before, after,
        "healthy retry and successful checkpoint job must be byte-identical"
    );
    assert_eq!(
        history,
        sqlx::query_scalar::<_, serde_json::Value>(
            "select jsonb_agg(to_jsonb(a) order by id) from post_event_delivery_attempts a"
        )
        .fetch_one(&f.pool)
        .await?
    );
    assert_eq!(
        mask,
        sqlx::query_scalar::<_, Vec<String>>(
            "select delivery_required_targets from post_event_outbox where event_id=$1"
        )
        .bind(event)
        .fetch_one(&f.pool)
        .await?
    );
    let replayed = f.claim(DeliveryTarget::External, &config).await?;
    f.schedule().complete_target(&replayed).await?;
    f.due(event, DeliveryTarget::Internal).await?;
    let ordinary = f.claim(DeliveryTarget::Internal, &config).await?;
    let rows = sqlx::query("select attempt_id,source,resolution_id from post_event_delivery_attempts where attempt_id=any($1)").bind(vec![replayed.attempt_id,ordinary.attempt_id]).fetch_all(&f.pool).await?;
    for row in rows {
        if row.get::<Uuid, _>("attempt_id") == replayed.attempt_id {
            assert_eq!(row.get::<String, _>("source"), "ADMIN_REPLAY");
            assert_eq!(
                row.get::<Option<Uuid>, _>("resolution_id"),
                Some(resolution)
            );
        } else {
            assert_eq!(row.get::<String, _>("source"), "OUTBOX_WORKER");
            assert_eq!(row.get::<Option<Uuid>, _>("resolution_id"), None);
        }
    }
    f.shutdown().await
}

#[tokio::test]
async fn recovery_resolution_releases_expired_exhausted_claim_without_false_terminal() -> Result<()>
{
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let config = DeliveryScheduleConfigDto {
        external_enabled: true,
        max_attempts: 1,
        ..Default::default()
    };
    let failed = f.claim(DeliveryTarget::External, &config).await?;
    f.schedule().fail_target(&failed).await?;
    let crashed = f.claim(DeliveryTarget::Internal, &config).await?;
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target='INTERNAL'").bind(event).execute(&f.pool).await?;
    let resolution = Uuid::new_v4();
    f.resolve(event, resolution).await?;
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select state from post_event_delivery_jobs where event_id=$1 and target='INTERNAL'"
        )
        .bind(event)
        .fetch_one(&f.pool)
        .await?,
        "PENDING"
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_attempts where attempt_id=$1 and phase<>'STARTED'").bind(crashed.attempt_id).fetch_one(&f.pool).await?,0);
    assert_eq!(
        f.schedule().complete_target(&crashed).await?,
        DeliveryLeaseOutcomeDto::Lost
    );
    let retry = f.claim(DeliveryTarget::Internal, &config).await?;
    assert_eq!(retry.attempt_number, 1);
    assert_ne!(retry.lease_token, crashed.lease_token);
    f.shutdown().await
}

async fn completion_wait_probe(lock_job: bool) -> Result<()> {
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let claim = f
        .claim(DeliveryTarget::Internal, &Default::default())
        .await?;
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()+interval '400 milliseconds' where event_id=$1 and target='INTERNAL'").bind(event).execute(&f.pool).await?;
    let mut blocker = f.pool.begin().await?;
    let blocker_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await?;
    let query = if lock_job {
        "select event_id from post_event_delivery_jobs where event_id=$1 and target='INTERNAL' for update"
    } else {
        "select event_id from post_event_outbox where event_id=$1 for update"
    };
    sqlx::query(query)
        .bind(event)
        .fetch_one(&mut *blocker)
        .await?;
    let schedule = f.schedule();
    let actor = tokio::spawn(async move { schedule.complete_target(&claim).await });
    // Verify the actual backend is waiting on our lock before letting its
    // database-clock lease expire. Timing alone is not concurrency evidence.
    tokio::time::timeout(std::time::Duration::from_secs(3),async {
        loop {
            let blocked: bool = sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and $1=any(pg_blocking_pids(pid)))").bind(blocker_pid).fetch_one(&f.pool).await?;
            if blocked { break; }
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
        loop {
            let expired: bool = sqlx::query_scalar("select lease_expires_at<clock_timestamp() from post_event_delivery_jobs where event_id=$1 and target='INTERNAL'").bind(event).fetch_one(&f.pool).await?;
            if expired { break; }
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
        Ok::<(),anyhow::Error>(())
    }).await??;
    blocker.commit().await?;
    assert_eq!(
        actor.await??,
        DeliveryLeaseOutcomeDto::Lost,
        "expiry must be evaluated after every completion lock wait"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase<>'STARTED'"
        )
        .fetch_one(&f.pool)
        .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from event_publications")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    f.shutdown().await
}

#[tokio::test]
async fn recovery_completion_waiting_on_job_lock_rechecks_expiry() -> Result<()> {
    completion_wait_probe(true).await
}

#[tokio::test]
async fn completion_waiting_on_parent_lock_refuses_expired_token() -> Result<()> {
    completion_wait_probe(false).await
}

#[tokio::test]
async fn concurrent_final_completions_publish_exactly_one_parent_transition() -> Result<()> {
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let first = f
        .claim(DeliveryTarget::Timeline, &Default::default())
        .await?;
    f.schedule().complete_target(&first).await?;
    let cache = f.claim(DeliveryTarget::Cache, &Default::default()).await?;
    let internal = f
        .claim(DeliveryTarget::Internal, &Default::default())
        .await?;
    let schedule = f.schedule();
    let (left, right) = tokio::join!(
        schedule.complete_target(&cache),
        schedule.complete_target(&internal)
    );
    let outcomes = [left?, right?];
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| **o
                == DeliveryLeaseOutcomeDto::Recorded {
                    event_published: true
                })
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| **o
                == DeliveryLeaseOutcomeDto::Recorded {
                    event_published: false
                })
            .count(),
        1
    );
    assert!(
        sqlx::query_scalar::<_, bool>("select published from post_event_outbox where event_id=$1")
            .bind(event)
            .fetch_one(&f.pool)
            .await?
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from event_publications")
            .fetch_one(&f.pool)
            .await?,
        3
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase='SUCCEEDED'"
        )
        .fetch_one(&f.pool)
        .await?,
        3
    );
    f.shutdown().await
}

#[tokio::test]
async fn lease_status_uses_database_identity_and_clock_without_renewal() -> Result<()> {
    use post_service::application::dto::delivery_schedule_dto::DeliveryLeaseStatusDto;
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let config = DeliveryScheduleConfigDto::default();
    let claim = f.claim(DeliveryTarget::Internal, &config).await?;
    let before: serde_json::Value = sqlx::query_scalar("select to_jsonb(j) from post_event_delivery_jobs j where event_id=$1 and target='INTERNAL'").bind(event).fetch_one(&f.pool).await?;
    match f.schedule().lease_status(&claim).await? {
        DeliveryLeaseStatusDto::Owned { remaining_ms } => {
            assert!(remaining_ms > 0 && remaining_ms <= config.lease_duration_ms)
        }
        DeliveryLeaseStatusDto::Lost => panic!("fresh database lease must be owned"),
    }
    let mut forged = claim.clone();
    forged.lease_expires_at = Utc::now() - Duration::days(1);
    assert!(
        matches!(
            f.schedule().lease_status(&forged).await?,
            DeliveryLeaseStatusDto::Owned { .. }
        ),
        "DTO time is not authoritative"
    );
    for field in 0..4 {
        let mut forged = claim.clone();
        match field {
            0 => forged.lease_token = Uuid::new_v4(),
            1 => forged.attempt_id = Uuid::new_v4(),
            2 => forged.target = DeliveryTarget::External,
            _ => forged.event.event_id = Uuid::new_v4(),
        }
        assert_eq!(
            f.schedule().lease_status(&forged).await?,
            DeliveryLeaseStatusDto::Lost
        );
    }
    let after: serde_json::Value = sqlx::query_scalar("select to_jsonb(j) from post_event_delivery_jobs j where event_id=$1 and target='INTERNAL'").bind(event).fetch_one(&f.pool).await?;
    assert_eq!(
        before, after,
        "status lookup must not renew or alter the lease"
    );
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target='INTERNAL'").bind(event).execute(&f.pool).await?;
    assert_eq!(
        f.schedule().lease_status(&claim).await?,
        DeliveryLeaseStatusDto::Lost
    );
    f.shutdown().await
}

#[tokio::test]
async fn idempotent_resolution_does_not_reset_a_subsequent_live_retry() -> Result<()> {
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let config = DeliveryScheduleConfigDto {
        external_enabled: true,
        max_attempts: 1,
        ..Default::default()
    };
    let failed = f.claim(DeliveryTarget::External, &config).await?;
    f.schedule().fail_target(&failed).await?;
    let resolution = Uuid::new_v4();
    assert!(f.resolve(event, resolution).await?.created);
    let retry = f.claim(DeliveryTarget::External, &config).await?;
    let before: serde_json::Value=sqlx::query_scalar("select to_jsonb(j) from post_event_delivery_jobs j where event_id=$1 and target='EXTERNAL'").bind(event).fetch_one(&f.pool).await?;
    assert!(!f.resolve(event, resolution).await?.created);
    let after: serde_json::Value=sqlx::query_scalar("select to_jsonb(j) from post_event_delivery_jobs j where event_id=$1 and target='EXTERNAL'").bind(event).fetch_one(&f.pool).await?;
    assert_eq!(before, after);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from post_event_dead_letter_resolutions")
            .fetch_one(&f.pool)
            .await?,
        1
    );
    f.schedule().complete_target(&retry).await?;
    f.shutdown().await
}

#[tokio::test]
async fn release_failure_rolls_back_immutable_decision_and_job_changes() -> Result<()> {
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let config = DeliveryScheduleConfigDto {
        external_enabled: true,
        max_attempts: 1,
        ..Default::default()
    };
    let failed = f.claim(DeliveryTarget::External, &config).await?;
    f.schedule().fail_target(&failed).await?;
    let before: serde_json::Value=sqlx::query_scalar("select jsonb_agg(to_jsonb(j) order by target) from post_event_delivery_jobs j where event_id=$1").bind(event).fetch_one(&f.pool).await?;
    sqlx::raw_sql("create function fixture_release_fault() returns trigger language plpgsql as $$ begin if old.state='DEAD' and new.state='PENDING' then raise exception 'injected release failure'; end if; return new; end $$; create trigger fixture_fault after update on post_event_delivery_jobs for each row execute function fixture_release_fault();").execute(&f.pool).await?;
    let resolution = Uuid::new_v4();
    let error = f
        .resolve(event, resolution)
        .await
        .expect_err("release must fail after decision insertion");
    assert!(format!("{error:#}").contains("injected release failure"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from post_event_dead_letter_resolutions")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    let after: serde_json::Value=sqlx::query_scalar("select jsonb_agg(to_jsonb(j) order by target) from post_event_delivery_jobs j where event_id=$1").bind(event).fetch_one(&f.pool).await?;
    assert_eq!(before, after);
    assert!(
        sqlx::query_scalar::<_, bool>(
            "select dead_lettered_at is not null from post_event_outbox where event_id=$1"
        )
        .bind(event)
        .fetch_one(&f.pool)
        .await?
    );
    sqlx::query("drop trigger fixture_fault on post_event_delivery_jobs")
        .execute(&f.pool)
        .await?;
    assert!(f.resolve(event, resolution).await?.created);
    f.shutdown().await
}

#[tokio::test]
async fn legacy_upgrade_resolution_preserves_checkpoint_and_releases_global_budget() -> Result<()> {
    let f = Fixture::new(16).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    f.checkpoint(event, DeliveryTarget::Timeline).await?;
    sqlx::query(
        "update post_event_outbox set dead_lettered_at=now(),attempt_count=5 where event_id=$1",
    )
    .bind(event)
    .execute(&f.pool)
    .await?;
    f.upgrade().await?;
    let before: serde_json::Value=sqlx::query_scalar("select to_jsonb(j) from post_event_delivery_jobs j where event_id=$1 and target='TIMELINE'").bind(event).fetch_one(&f.pool).await?;
    let resolution = Uuid::new_v4();
    assert!(f.resolve(event, resolution).await?.created);
    let after: serde_json::Value=sqlx::query_scalar("select to_jsonb(j) from post_event_delivery_jobs j where event_id=$1 and target='TIMELINE'").bind(event).fetch_one(&f.pool).await?;
    assert_eq!(before, after);
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_jobs where event_id=$1 and state='PENDING' and attempt_count=0 and release_resolution_id=$2").bind(event).bind(resolution).fetch_one(&f.pool).await?,3);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from event_publications")
            .fetch_one(&f.pool)
            .await?,
        1
    );
    let config = DeliveryScheduleConfigDto {
        external_enabled: true,
        ..Default::default()
    };
    let claim = f.claim(DeliveryTarget::External, &config).await?;
    assert_eq!(claim.attempt_number, 1);
    assert_eq!(
        sqlx::query_scalar::<_, Option<Uuid>>(
            "select resolution_id from post_event_delivery_attempts where attempt_id=$1"
        )
        .bind(claim.attempt_id)
        .fetch_one(&f.pool)
        .await?,
        Some(resolution)
    );
    f.shutdown().await
}

#[tokio::test]
async fn target_mode_refuses_stale_parent_dead_flag_without_exhausted_jobs() -> Result<()> {
    use post_service::domain::post::DeadLetterReplayError;
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let claim = f
        .claim(DeliveryTarget::Timeline, &Default::default())
        .await?;
    f.schedule().complete_target(&claim).await?;
    sqlx::query(
        "update post_event_outbox set dead_lettered_at=now(),attempt_count=5 where event_id=$1",
    )
    .bind(event)
    .execute(&f.pool)
    .await?;
    let error = f
        .resolve(event, Uuid::new_v4())
        .await
        .expect_err("stale parent summary must not reset healthy target state");
    assert!(matches!(
        error.downcast_ref::<DeadLetterReplayError>(),
        Some(DeadLetterReplayError::NotDeadLettered)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from post_event_dead_letter_resolutions")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    f.shutdown().await
}

#[tokio::test]
async fn expired_healthy_lease_keeps_its_budget_and_provenance_during_other_resolution(
) -> Result<()> {
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let config = DeliveryScheduleConfigDto {
        external_enabled: true,
        max_attempts: 2,
        ..Default::default()
    };
    for _ in 0..2 {
        f.due(event, DeliveryTarget::External).await?;
        let claim = f.claim(DeliveryTarget::External, &config).await?;
        f.schedule().fail_target(&claim).await?;
    }
    let uncertain = f.claim(DeliveryTarget::Internal, &config).await?;
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target='INTERNAL'").bind(event).execute(&f.pool).await?;
    let before: serde_json::Value=sqlx::query_scalar("select to_jsonb(j) from post_event_delivery_jobs j where event_id=$1 and target='INTERNAL'").bind(event).fetch_one(&f.pool).await?;
    assert!(f.resolve(event, Uuid::new_v4()).await?.created);
    let after: serde_json::Value=sqlx::query_scalar("select to_jsonb(j) from post_event_delivery_jobs j where event_id=$1 and target='INTERNAL'").bind(event).fetch_one(&f.pool).await?;
    assert_eq!(before, after);
    let retry = f.claim(DeliveryTarget::Internal, &config).await?;
    assert_eq!(retry.attempt_number, 2);
    assert_ne!(retry.lease_token, uncertain.lease_token);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select source from post_event_delivery_attempts where attempt_id=$1"
        )
        .bind(retry.attempt_id)
        .fetch_one(&f.pool)
        .await?,
        "OUTBOX_WORKER"
    );
    f.shutdown().await
}

struct Fixture {
    pool: PgPool,
    local: Option<(PgPool, String)>,
    _container: Option<ContainerAsync<Postgres>>,
    provisioning: fresh_provisioning::FreshFixtureProvisioningDto,
    controller_pool: Option<PgPool>,
    runtime_role: Option<String>,
}

impl Fixture {
    async fn new(through: i64) -> Result<Self> {
        // Controller-owned identifiers are created before CREATE DATABASE and
        // are never reconstructed from database contents or application startup.
        let provisioning_nonce = Uuid::new_v4();
        let writer_generation = Uuid::new_v4();
        let (admin, options, container) =
            if let Ok(url) = std::env::var("POST_DELIVERY_TEST_DATABASE_URL") {
                ensure!(
                    std::env::var("POST_DELIVERY_TEST_ALLOW_LOCAL_PG").as_deref() == Ok("1"),
                    "local fixture requires explicit opt-in"
                );
                ensure!(
                    !url.contains('?') && !url.contains('#'),
                    "connection overrides are forbidden"
                );
                let options: PgConnectOptions = url.parse()?;
                ensure!(
                    options.get_socket().is_none() && options.get_options().is_none(),
                    "loopback TCP without connection overrides is required"
                );
                let host = options
                    .get_host()
                    .trim_start_matches('[')
                    .trim_end_matches(']');
                ensure!(
                    host == "localhost" || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()),
                    "loopback PostgreSQL required"
                );
                let database = options.get_database().unwrap_or_default();
                ensure!(
                    database.starts_with("port_rust_delivery_test_")
                        && database
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
                    "isolated delivery test database required"
                );
                let admin = PgPoolOptions::new()
                    .max_connections(2)
                    .connect_with(options.clone())
                    .await?;
                (admin, options, None)
            } else {
                let (container, url) = test_support::postgres::start_postgres_with_url().await?;
                let options: PgConnectOptions = url.parse()?;
                let admin = test_support::postgres::with_startup_diagnostics(
                    Some(&container),
                    "Post delivery fixture controller",
                    async {
                        Ok(PgPoolOptions::new()
                            .max_connections(2)
                            .connect_with(options.clone())
                            .await?)
                    },
                )
                .await?;
                (admin, options, Some(container))
            };
        let name = format!("port_rust_delivery_fixture_{}", provisioning_nonce.simple());
        let prepared = fresh_provisioning::create(
            &admin,
            options,
            container.is_some(),
            provisioning_nonce,
            writer_generation,
            through,
        )
        .await;
        let (pool, provisioning) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                if let Some(container) = container.as_ref() {
                    use std::io::Write;
                    let observed =
                        test_support::postgres::startup_failure_diagnostics(container).await;
                    // Observe before scope cleanup discards the failed server.
                    // Output failure must not replace the provisioning error.
                    let _ = writeln!(
                        std::io::stderr().lock(),
                        "POSTGRES_FIXTURE_PROVISIONING_FAILURE {observed:?}",
                    );
                }
                admin.close().await;
                return Err(error);
            }
        };
        let fixture = Self {
            pool,
            local: Some((admin, name)),
            _container: container,
            provisioning,
            controller_pool: None,
            runtime_role: None,
        };
        Ok(fixture)
    }

    async fn migrations(&self) -> Result<sqlx::migrate::Migrator> {
        Ok(sqlx::migrate::Migrator::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations"),
        )
        .await?)
    }

    async fn upgrade(&self) -> Result<()> {
        let migrations = self.migrations().await?;
        migrations.run(&self.pool).await?;
        fresh_provisioning::verify_migrations(&self.pool, &migrations).await?;
        Ok(())
    }

    fn schedule(&self) -> PgDeliverySchedule {
        PgDeliverySchedule::new(self.pool.clone())
    }

    async fn save(&self, aggregate: Uuid, version: Option<i64>, age_seconds: i64) -> Result<Uuid> {
        let now = Utc::now() - Duration::seconds(age_seconds);
        let event = StoredEvent {
            event_id: Uuid::new_v4(),
            event_type: "POST_CREATED".into(),
            aggregate_id: aggregate,
            aggregate_version: version,
            actor_id: None,
            attributes: vec![],
            payload: "{\"postId\":\"fixture\"}".into(),
            occurred_at: now,
            created_at: now,
            published: false,
            published_at: None,
            replay_resolution_id: None,
            origin_traceparent: None,
        };
        let saved = PgEventOutbox::new(self.pool.clone()).save(&event).await?;
        if version.is_none() {
            // New writes are always sequenced by migration 0012's trigger.
            // Explicit SQL models a retained pre-0012 event for this probe.
            sqlx::query("update post_event_outbox set aggregate_version=null where event_id=$1")
                .bind(event.event_id)
                .execute(&self.pool)
                .await?;
        } else {
            ensure!(
                saved.aggregate_version == version,
                "fixture version must match the database-owned sequence"
            );
        }
        Ok(event.event_id)
    }

    async fn claim(
        &self,
        target: DeliveryTarget,
        config: &DeliveryScheduleConfigDto,
    ) -> Result<TargetDeliveryClaimDto> {
        let mut batch = self
            .schedule()
            .claim_target(target, Utc::now(), 1, config)
            .await?;
        ensure!(
            batch.claims.len() == 1,
            "expected one claim; got {}",
            batch.claims.len()
        );
        Ok(batch.claims.remove(0))
    }

    async fn checkpoint(&self, event: Uuid, target: DeliveryTarget) -> Result<()> {
        sqlx::query("insert into event_publications(event_id,target,published_at,payload) select event_id,$2,now(),payload from post_event_outbox where event_id=$1")
            .bind(event).bind(target.as_str()).execute(&self.pool).await?;
        Ok(())
    }

    async fn resolution(&self, event: Uuid) -> Result<Uuid> {
        let id = Uuid::new_v4();
        sqlx::query("insert into post_event_dead_letter_resolutions values($1,$2,$3,'Operator inspected delivery evidence','fixture:recovery',now())")
            .bind(id).bind(event).bind(Uuid::new_v4()).execute(&self.pool).await?;
        sqlx::query("update post_event_outbox set replay_resolution_id=$2 where event_id=$1")
            .bind(event)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(id)
    }

    async fn shutdown(self) -> Result<()> {
        self.pool.close().await;
        if let Some(controller) = self.controller_pool {
            controller.close().await;
        }
        if let Some((admin, name)) = self.local {
            // Only this fixture's generated isolated database is terminated. SQLx
            // connection shutdown can outlive pool.close at the server.
            sqlx::query(&format!("drop database {name} with (force)"))
                .execute(&admin)
                .await?;
            if let Some(role) = self.runtime_role {
                sqlx::query(&format!("drop role {role}"))
                    .execute(&admin)
                    .await?;
            }
            admin.close().await;
        }
        Ok(())
    }
}

#[tokio::test]
async fn review_mixed_legacy_versioned_ordering_has_no_cycle() -> Result<()> {
    let f = Fixture::new(22).await?;
    let aggregate = Uuid::new_v4();
    let versioned = f.save(aggregate, Some(1), 20).await?;
    let legacy = f.save(aggregate, None, 10).await?;
    let config = DeliveryScheduleConfigDto::default();
    let claim = f.claim(DeliveryTarget::Internal, &config).await?;
    assert_eq!(
        claim.event.event_id, legacy,
        "legacy must precede versioned even with reversed timestamps"
    );
    f.schedule().complete_target(&claim).await?;
    assert_eq!(
        f.claim(DeliveryTarget::Internal, &config)
            .await?
            .event
            .event_id,
        versioned
    );
    f.shutdown().await
}

#[tokio::test]
async fn review_fully_checkpointed_upgrade_finalizes_without_attempt() -> Result<()> {
    let f = Fixture::new(16).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    for target in DeliveryTarget::ALL {
        f.checkpoint(event, target).await?;
    }
    f.upgrade().await?;
    let batch = f
        .schedule()
        .claim_target(
            DeliveryTarget::Internal,
            Utc::now(),
            1,
            &DeliveryScheduleConfigDto::default(),
        )
        .await?;
    assert!(batch.claims.is_empty());
    assert_eq!(batch.reconciled_published_events, 1);
    let row = sqlx::query("select published,(select count(*) from post_event_delivery_attempts) attempts from post_event_outbox where event_id=$1").bind(event).fetch_one(&f.pool).await?;
    assert!(row.get::<bool, _>("published"));
    assert_eq!(row.get::<i64, _>("attempts"), 0);
    assert_eq!(
        f.schedule()
            .claim_target(
                DeliveryTarget::Internal,
                Utc::now(),
                1,
                &DeliveryScheduleConfigDto::default()
            )
            .await?
            .reconciled_published_events,
        0
    );
    f.shutdown().await
}

#[tokio::test]
async fn review_healthy_target_does_not_inherit_parent_resolution() -> Result<()> {
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    f.resolution(event).await?;
    let claim = f
        .claim(
            DeliveryTarget::Internal,
            &DeliveryScheduleConfigDto::default(),
        )
        .await?;
    let row = sqlx::query(
        "select source,resolution_id from post_event_delivery_attempts where attempt_id=$1",
    )
    .bind(claim.attempt_id)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(row.get::<String, _>("source"), "OUTBOX_WORKER");
    assert_eq!(row.get::<Option<Uuid>, _>("resolution_id"), None);
    f.shutdown().await
}

#[tokio::test]
async fn released_target_keeps_its_resolution_and_rejects_another_events_decision() -> Result<()> {
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 10).await?;
    let resolution = f.resolution(event).await?;
    // This isolates persisted release attribution. The operator resolver is
    // not wired to the scheduler yet and is not accepted by this probe.
    sqlx::query("update post_event_delivery_jobs set release_resolution_id=$2 where event_id=$1 and target='CACHE'").bind(event).bind(resolution).execute(&f.pool).await?;
    let other = f.save(Uuid::new_v4(), Some(1), 1).await?;
    assert!(sqlx::query("update post_event_delivery_jobs set release_resolution_id=$2 where event_id=$1 and target='CACHE'").bind(other).bind(resolution).execute(&f.pool).await.is_err());
    let claim = f.claim(DeliveryTarget::Cache, &Default::default()).await?;
    f.schedule().complete_target(&claim).await?;
    let rows = sqlx::query("select source,resolution_id from post_event_delivery_attempts where attempt_id=$1 order by id").bind(claim.attempt_id).fetch_all(&f.pool).await?;
    assert_eq!(rows.len(), 2);
    for row in rows {
        assert_eq!(row.get::<String, _>("source"), "ADMIN_REPLAY");
        assert_eq!(
            row.get::<Option<Uuid>, _>("resolution_id"),
            Some(resolution)
        );
    }
    f.shutdown().await
}

#[tokio::test]
async fn concurrent_claimers_get_only_one_lease_for_a_target_head() -> Result<()> {
    let f = Fixture::new(22).await?;
    f.save(Uuid::new_v4(), Some(1), 1).await?;
    let schedule = f.schedule();
    let config = DeliveryScheduleConfigDto::default();
    let (left, right) = tokio::join!(
        schedule.claim_target(DeliveryTarget::Cache, Utc::now(), 1, &config),
        schedule.claim_target(DeliveryTarget::Cache, Utc::now(), 1, &config)
    );
    assert_eq!(left?.claims.len() + right?.claims.len(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from post_event_delivery_attempts")
            .fetch_one(&f.pool)
            .await?,
        1
    );
    f.shutdown().await
}

#[tokio::test]
async fn missing_required_job_cannot_publish_parent() -> Result<()> {
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let first = f
        .claim(DeliveryTarget::Timeline, &Default::default())
        .await?;
    f.schedule().complete_target(&first).await?;
    sqlx::query("delete from post_event_delivery_jobs where event_id=$1 and target='CACHE'")
        .bind(event)
        .execute(&f.pool)
        .await?;
    let last = f
        .claim(DeliveryTarget::Internal, &Default::default())
        .await?;
    assert_eq!(
        f.schedule().complete_target(&last).await?,
        DeliveryLeaseOutcomeDto::Recorded {
            event_published: false
        }
    );
    assert!(
        !sqlx::query_scalar::<_, bool>("select published from post_event_outbox where event_id=$1")
            .bind(event)
            .fetch_one(&f.pool)
            .await?
    );
    f.shutdown().await
}

#[tokio::test]
async fn successful_targets_finalize_once_and_freeze_required_mask() -> Result<()> {
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_jobs where event_id=$1"
        )
        .bind(event)
        .fetch_one(&f.pool)
        .await?,
        4
    );
    let config = DeliveryScheduleConfigDto::default();
    let first = f.claim(DeliveryTarget::Timeline, &config).await?;
    assert_eq!(
        f.schedule().complete_target(&first).await?,
        DeliveryLeaseOutcomeDto::Recorded {
            event_published: false
        }
    );
    let changed = DeliveryScheduleConfigDto {
        external_enabled: true,
        max_attempts: 20,
        ..config.clone()
    };
    for target in [DeliveryTarget::Cache, DeliveryTarget::Internal] {
        let claim = f.claim(target, &changed).await?;
        assert_eq!(
            f.schedule().complete_target(&claim).await?,
            DeliveryLeaseOutcomeDto::Recorded {
                event_published: target == DeliveryTarget::Internal
            }
        );
        assert_eq!(
            f.schedule().complete_target(&claim).await?,
            DeliveryLeaseOutcomeDto::Lost
        );
    }
    let row = sqlx::query("select state,max_attempts,(select count(*) from event_publications where target='EXTERNAL') checkpoints from post_event_delivery_jobs where event_id=$1 and target='EXTERNAL'").bind(event).fetch_one(&f.pool).await?;
    assert_eq!(row.get::<String, _>("state"), "SKIPPED");
    assert_eq!(row.get::<i32, _>("max_attempts"), 5);
    assert_eq!(row.get::<i64, _>("checkpoints"), 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from post_event_delivery_attempts")
            .fetch_one(&f.pool)
            .await?,
        6
    );
    f.shutdown().await
}

#[tokio::test]
async fn dead_target_blocks_only_its_successors_without_starving_other_heads() -> Result<()> {
    let f = Fixture::new(22).await?;
    let aggregate = Uuid::new_v4();
    let first = f.save(aggregate, Some(1), 30).await?;
    let second = f.save(aggregate, Some(2), 20).await?;
    let other = f.save(Uuid::new_v4(), Some(1), 10).await?;
    let config = DeliveryScheduleConfigDto {
        external_enabled: true,
        max_attempts: 1,
        ..Default::default()
    };
    let failed = f.claim(DeliveryTarget::External, &config).await?;
    assert_eq!(failed.event.event_id, first);
    f.schedule().fail_target(&failed).await?;
    assert_eq!(
        f.claim(DeliveryTarget::External, &config)
            .await?
            .event
            .event_id,
        other,
        "blocked successor must be excluded before limit"
    );
    let healthy = f.claim(DeliveryTarget::Internal, &config).await?;
    assert_eq!(healthy.event.event_id, first);
    f.schedule().complete_target(&healthy).await?;
    assert_eq!(
        f.claim(DeliveryTarget::Internal, &config)
            .await?
            .event
            .event_id,
        second
    );
    f.shutdown().await
}

#[tokio::test]
async fn expired_tokens_cannot_finish_reclaimed_attempts() -> Result<()> {
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let config = DeliveryScheduleConfigDto::default();
    let first = f.claim(DeliveryTarget::Cache, &config).await?;
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=now()-interval '1 second' where event_id=$1 and target='CACHE'").bind(event).execute(&f.pool).await?;
    assert_eq!(
        f.schedule().complete_target(&first).await?,
        DeliveryLeaseOutcomeDto::Lost
    );
    let second = f.claim(DeliveryTarget::Cache, &config).await?;
    assert_ne!(first.lease_token, second.lease_token);
    assert_eq!(second.attempt_number, 2);
    assert_eq!(
        f.schedule().fail_target(&first).await?,
        DeliveryLeaseOutcomeDto::Lost
    );
    f.schedule().complete_target(&second).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase='STARTED'"
        )
        .fetch_one(&f.pool)
        .await?,
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase='SUCCEEDED'"
        )
        .fetch_one(&f.pool)
        .await?,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase='FAILED'"
        )
        .fetch_one(&f.pool)
        .await?,
        0
    );
    f.shutdown().await
}

#[tokio::test]
async fn exhausted_crashed_lease_stays_uncertain_without_false_failure() -> Result<()> {
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let config = DeliveryScheduleConfigDto {
        max_attempts: 1,
        ..Default::default()
    };
    f.claim(DeliveryTarget::Internal, &config).await?;
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=now()-interval '1 second' where event_id=$1 and target='INTERNAL'").bind(event).execute(&f.pool).await?;
    assert!(f
        .schedule()
        .claim_target(DeliveryTarget::Internal, Utc::now(), 1, &config)
        .await?
        .claims
        .is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select state from post_event_delivery_jobs where event_id=$1 and target='INTERNAL'"
        )
        .bind(event)
        .fetch_one(&f.pool)
        .await?,
        "DEAD"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase<>'STARTED'"
        )
        .fetch_one(&f.pool)
        .await?,
        0
    );
    f.shutdown().await
}

#[tokio::test]
async fn claim_history_failure_rolls_back_registration_and_lease() -> Result<()> {
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    sqlx::raw_sql("create function fixture_reject() returns trigger language plpgsql as $$ begin raise exception 'injected history failure'; end $$; create trigger fixture_fault before insert on post_event_delivery_attempts for each row execute function fixture_reject();").execute(&f.pool).await?;
    assert!(f
        .schedule()
        .claim_target(DeliveryTarget::Cache, Utc::now(), 1, &Default::default())
        .await
        .is_err());
    let row = sqlx::query("select delivery_required_targets,(select sum(attempt_count) from post_event_delivery_jobs) attempts from post_event_outbox where event_id=$1").bind(event).fetch_one(&f.pool).await?;
    assert!(row
        .get::<Option<Vec<String>>, _>("delivery_required_targets")
        .is_none());
    assert_eq!(row.get::<i64, _>("attempts"), 0);
    sqlx::query("drop trigger fixture_fault on post_event_delivery_attempts")
        .execute(&f.pool)
        .await?;
    f.claim(DeliveryTarget::Cache, &Default::default()).await?;
    f.shutdown().await
}

#[tokio::test]
async fn checkpoint_failure_rolls_back_terminal_and_allows_same_claim_retry() -> Result<()> {
    let f = Fixture::new(22).await?;
    f.save(Uuid::new_v4(), Some(1), 1).await?;
    let claim = f.claim(DeliveryTarget::Cache, &Default::default()).await?;
    sqlx::raw_sql("create function fixture_reject() returns trigger language plpgsql as $$ begin raise exception 'injected checkpoint failure'; end $$; create trigger fixture_fault before insert on event_publications for each row execute function fixture_reject();").execute(&f.pool).await?;
    assert!(f.schedule().complete_target(&claim).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase='SUCCEEDED'"
        )
        .fetch_one(&f.pool)
        .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select state from post_event_delivery_jobs where event_id=$1 and target='CACHE'"
        )
        .bind(claim.event.event_id)
        .fetch_one(&f.pool)
        .await?,
        "LEASED"
    );
    sqlx::query("drop trigger fixture_fault on event_publications")
        .execute(&f.pool)
        .await?;
    assert_eq!(
        f.schedule().complete_target(&claim).await?,
        DeliveryLeaseOutcomeDto::Recorded {
            event_published: false
        }
    );
    f.shutdown().await
}

#[tokio::test]
async fn retry_backoff_is_bounded_and_policy_is_frozen() -> Result<()> {
    let f = Fixture::new(22).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let config = DeliveryScheduleConfigDto {
        retry_base_ms: 1000,
        retry_cap_ms: 1500,
        max_attempts: 3,
        ..Default::default()
    };
    let first = f.claim(DeliveryTarget::Cache, &config).await?;
    f.schedule().fail_target(&first).await?;
    let delay: f64 = sqlx::query_scalar("select extract(epoch from (next_attempt_at-clock_timestamp()))::float8*1000 from post_event_delivery_jobs where event_id=$1 and target='CACHE'").bind(event).fetch_one(&f.pool).await?;
    assert!(
        delay > 0.0 && delay <= 1000.0,
        "actual first retry delay {delay}"
    );
    assert!(f
        .schedule()
        .claim_target(DeliveryTarget::Cache, Utc::now(), 1, &config)
        .await?
        .claims
        .is_empty());
    sqlx::query("update post_event_delivery_jobs set next_attempt_at=now()-interval '1 second' where event_id=$1 and target='CACHE'").bind(event).execute(&f.pool).await?;
    let changed = DeliveryScheduleConfigDto {
        max_attempts: 99,
        retry_base_ms: 10,
        retry_cap_ms: 20,
        ..config
    };
    let second = f.claim(DeliveryTarget::Cache, &changed).await?;
    f.schedule().fail_target(&second).await?;
    let row = sqlx::query("select retry_base_ms,retry_cap_ms,max_attempts,extract(epoch from (next_attempt_at-clock_timestamp()))::float8*1000 delay from post_event_delivery_jobs where event_id=$1 and target='CACHE'").bind(event).fetch_one(&f.pool).await?;
    assert_eq!(row.get::<i64, _>("retry_base_ms"), 1000);
    assert_eq!(row.get::<i64, _>("retry_cap_ms"), 1500);
    assert_eq!(row.get::<i32, _>("max_attempts"), 3);
    assert!(row.get::<f64, _>("delay") > 0.0 && row.get::<f64, _>("delay") <= 1500.0);
    f.shutdown().await
}

#[tokio::test]
async fn migration_inherits_checkpoints_dead_budgets_and_preserves_history() -> Result<()> {
    let f = Fixture::new(16).await?;
    let dead = f.save(Uuid::new_v4(), Some(1), 1).await?;
    f.checkpoint(dead, DeliveryTarget::Timeline).await?;
    sqlx::query(
        "update post_event_outbox set dead_lettered_at=now(),attempt_count=5 where event_id=$1",
    )
    .bind(dead)
    .execute(&f.pool)
    .await?;
    let published = f.save(Uuid::new_v4(), Some(1), 1).await?;
    f.checkpoint(published, DeliveryTarget::Internal).await?;
    sqlx::query("update post_event_outbox set published=true,published_at=now() where event_id=$1")
        .bind(published)
        .execute(&f.pool)
        .await?;
    sqlx::query("insert into post_event_delivery_attempts(attempt_id,event_id,target,source,phase,occurred_at) values($1,$2,'CACHE','OUTBOX_WORKER','STARTED',now())").bind(Uuid::new_v4()).bind(dead).execute(&f.pool).await?;
    let before: serde_json::Value = sqlx::query_scalar(
        "select jsonb_agg(to_jsonb(a) order by id) from post_event_delivery_attempts a",
    )
    .fetch_one(&f.pool)
    .await?;
    f.upgrade().await?;
    let after: serde_json::Value = sqlx::query_scalar(
        "select jsonb_agg(to_jsonb(a) order by id) from post_event_delivery_attempts a",
    )
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(before, after);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select state from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'"
        )
        .bind(dead)
        .fetch_one(&f.pool)
        .await?,
        "SUCCEEDED"
    );
    assert_eq!(sqlx::query_scalar::<_,i32>("select attempt_count from post_event_delivery_jobs where event_id=$1 and target='CACHE' and state='DEAD'").bind(dead).fetch_one(&f.pool).await?,5);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select state from post_event_delivery_jobs where event_id=$1 and target='EXTERNAL'"
        )
        .bind(published)
        .fetch_one(&f.pool)
        .await?,
        "SKIPPED"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from event_publications")
            .fetch_one(&f.pool)
            .await?,
        2
    );
    f.shutdown().await
}
