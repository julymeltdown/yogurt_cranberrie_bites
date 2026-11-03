use super::*;
use async_trait::async_trait;
use post_service::adapters::r#in::messaging::fenced_replay_delivery::FencedReplayDelivery;
use post_service::application::delivery_effect::DeliveryEffectFailure;
use post_service::application::dto::delivery_schedule_dto::DeliveryExecutionConfigDto;
use post_service::application::dto::post_dto::ReplayRequestDto;
use post_service::application::ports::{
    ExternalEventPublisherPort, InternalEventDispatcherPort, ReplayEventDeliveryPort,
    ReplaySchedulePort,
};
use post_service::application::use_cases::replay_events::ReplayEventsUseCase;
use post_service::domain::post::ReplayTarget;
use std::sync::{Arc, Mutex};

#[derive(Default, Clone, Copy)]
enum Mode {
    #[default]
    Success,
    Confirmed,
    Uncertain,
    Block,
}
#[derive(Default)]
struct Effects {
    events: Mutex<Vec<StoredEvent>>,
    mode: Mode,
    entered: tokio::sync::Notify,
}
impl Effects {
    async fn deliver(&self, event: &StoredEvent) -> Result<()> {
        self.events.lock().unwrap().push(event.clone());
        self.entered.notify_one();
        match self.mode {
            Mode::Success => Ok(()),
            Mode::Confirmed => Err(DeliveryEffectFailure::confirmed(anyhow::anyhow!(
                "confirmed pre-effect rejection"
            ))),
            Mode::Uncertain => Err(DeliveryEffectFailure::uncertain(anyhow::anyhow!(
                "lost acknowledgement"
            ))),
            Mode::Block => std::future::pending().await,
        }
    }
}
#[async_trait]
impl ExternalEventPublisherPort for Effects {
    async fn publish(&self, event: &StoredEvent) -> Result<()> {
        self.deliver(event).await
    }
}
#[async_trait]
impl InternalEventDispatcherPort for Effects {
    async fn dispatch(&self, event: &StoredEvent) -> Result<()> {
        self.deliver(event).await
    }
}

fn config() -> DeliveryScheduleConfigDto {
    DeliveryScheduleConfigDto {
        external_enabled: true,
        ..Default::default()
    }
}
async fn event(f: &Fixture, aggregate: Uuid, age: i64) -> Result<Uuid> {
    let id = f.save(aggregate, None, age).await?;
    sqlx::query("update post_event_outbox set event_type='WORKER_FIXTURE',payload='{\"fresh\":1}' where event_id=$1").bind(id).execute(&f.pool).await?;
    Ok(id)
}
async fn expire(f: &Fixture, id: Uuid, target: DeliveryTarget) -> Result<()> {
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target=$2")
        .bind(id).bind(target.as_str()).execute(&f.pool).await?;
    Ok(())
}
async fn state(f: &Fixture, id: Uuid, target: DeliveryTarget) -> Result<String> {
    Ok(sqlx::query_scalar(
        "select state from post_event_delivery_jobs where event_id=$1 and target=$2",
    )
    .bind(id)
    .bind(target.as_str())
    .fetch_one(&f.pool)
    .await?)
}
async fn complete_all(f: &Fixture) -> Result<()> {
    for target in [
        DeliveryTarget::Timeline,
        DeliveryTarget::Cache,
        DeliveryTarget::Internal,
        DeliveryTarget::External,
    ] {
        let claim = f.claim(target, &config()).await?;
        f.schedule().complete_target(&claim).await?;
    }
    Ok(())
}
fn adapter(f: &Fixture, effects: Arc<Effects>, timeout: u64) -> Result<FencedReplayDelivery> {
    FencedReplayDelivery::new(
        Arc::new(f.schedule()),
        effects.clone(),
        effects,
        DeliveryExecutionConfigDto {
            effect_timeout_ms: timeout,
            completion_reserve_ms: 100,
            ..Default::default()
        },
    )
}
fn request(target: ReplayTarget) -> ReplayRequestDto {
    ReplayRequestDto {
        actor_id: None,
        event_type: Some("WORKER_FIXTURE".into()),
        attribute: None,
        from: None,
        to: None,
        published: None,
        target: Some(target),
        limit: Some(1),
    }
}

#[tokio::test]
async fn automatic_and_replay_claims_share_one_target_lease() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1).await?;
    let automatic = f.claim(DeliveryTarget::Internal, &config()).await?;
    assert!(f
        .schedule()
        .claim_replay_target(id, DeliveryTarget::Internal, &config())
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from post_event_delivery_attempts")
            .fetch_one(&f.pool)
            .await?,
        1
    );
    expire(&f, id, DeliveryTarget::Internal).await?;
    let replay = f
        .schedule()
        .claim_replay_target(id, DeliveryTarget::Internal, &config())
        .await?;
    assert_eq!(
        f.schedule().complete_target(&automatic).await?,
        DeliveryLeaseOutcomeDto::Lost
    );
    assert!(f
        .schedule()
        .claim_target(DeliveryTarget::Internal, Utc::now(), 1, &config())
        .await?
        .claims
        .is_empty());
    f.schedule().complete_target(&replay).await?;
    f.shutdown().await
}

#[tokio::test]
async fn racing_automatic_and_replay_claimers_commit_only_one_started() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1).await?;
    let a = f.schedule();
    let b = f.schedule();
    let policy = config();
    let (auto, replay) = tokio::join!(
        a.claim_target(DeliveryTarget::Internal, Utc::now(), 1, &policy),
        b.claim_replay_target(id, DeliveryTarget::Internal, &policy)
    );
    assert_eq!(auto?.claims.len() + usize::from(replay.is_ok()), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from post_event_delivery_attempts")
            .fetch_one(&f.pool)
            .await?,
        1
    );
    f.shutdown().await
}

#[tokio::test]
async fn completed_replay_failure_preserves_budget_checkpoint_and_parent_time() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1).await?;
    complete_all(&f).await?;
    let before:serde_json::Value=sqlx::query_scalar("select jsonb_build_object('parent',to_jsonb(e),'publication',to_jsonb(p),'budget',j.attempt_count,'due',j.next_attempt_at) from post_event_outbox e join event_publications p on p.event_id=e.event_id and p.target='INTERNAL' join post_event_delivery_jobs j on j.event_id=e.event_id and j.target='INTERNAL' where e.event_id=$1").bind(id).fetch_one(&f.pool).await?;
    let replay = f
        .schedule()
        .claim_replay_target(id, DeliveryTarget::Internal, &config())
        .await?;
    f.schedule().fail_target(&replay).await?;
    let after:serde_json::Value=sqlx::query_scalar("select jsonb_build_object('parent',to_jsonb(e),'publication',to_jsonb(p),'budget',j.attempt_count,'due',j.next_attempt_at) from post_event_outbox e join event_publications p on p.event_id=e.event_id and p.target='INTERNAL' join post_event_delivery_jobs j on j.event_id=e.event_id and j.target='INTERNAL' where e.event_id=$1").bind(id).fetch_one(&f.pool).await?;
    assert_eq!(before, after);
    assert_eq!(state(&f, id, DeliveryTarget::Internal).await?, "SUCCEEDED");
    let success = f
        .schedule()
        .claim_replay_target(id, DeliveryTarget::Internal, &config())
        .await?;
    f.schedule().complete_target(&success).await?;
    assert_eq!(sqlx::query_scalar::<_,i32>("select attempt_count from post_event_delivery_jobs where event_id=$1 and target='INTERNAL'").bind(id).fetch_one(&f.pool).await?,1);
    f.shutdown().await
}

#[tokio::test]
async fn live_completed_replay_survives_checkpoint_reconciliation_and_parent_finalization(
) -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1).await?;
    let done = f.claim(DeliveryTarget::Internal, &config()).await?;
    f.schedule().complete_target(&done).await?;
    let replay = f
        .schedule()
        .claim_replay_target(id, DeliveryTarget::Internal, &config())
        .await?;
    for target in [
        DeliveryTarget::Timeline,
        DeliveryTarget::Cache,
        DeliveryTarget::External,
    ] {
        let claim = f.claim(target, &config()).await?;
        f.schedule().complete_target(&claim).await?;
    }
    assert_eq!(state(&f, id, DeliveryTarget::Internal).await?, "LEASED");
    assert!(
        sqlx::query_scalar::<_, bool>("select published from post_event_outbox where event_id=$1")
            .bind(id)
            .fetch_one(&f.pool)
            .await?
    );
    assert!(matches!(
        f.schedule().complete_target(&replay).await?,
        DeliveryLeaseOutcomeDto::Recorded {
            event_published: false
        }
    ));
    f.shutdown().await
}

#[tokio::test]
async fn live_excluded_replay_survives_skipped_reconciliation_and_expiry() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1).await?;
    let disabled = DeliveryScheduleConfigDto::default();
    let first = f.claim(DeliveryTarget::Internal, &disabled).await?;
    f.schedule().complete_target(&first).await?;
    let replay = f
        .schedule()
        .claim_replay_target(id, DeliveryTarget::External, &config())
        .await?;
    for target in [DeliveryTarget::Timeline, DeliveryTarget::Cache] {
        let claim = f.claim(target, &disabled).await?;
        f.schedule().complete_target(&claim).await?;
    }
    assert_eq!(state(&f, id, DeliveryTarget::External).await?, "LEASED");
    expire(&f, id, DeliveryTarget::External).await?;
    assert!(f
        .schedule()
        .claim_target(DeliveryTarget::External, Utc::now(), 1, &config())
        .await?
        .claims
        .is_empty());
    assert_eq!(state(&f, id, DeliveryTarget::External).await?, "SKIPPED");
    assert_eq!(
        f.schedule().complete_target(&replay).await?,
        DeliveryLeaseOutcomeDto::Lost
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from event_publications where target='EXTERNAL'"
        )
        .fetch_one(&f.pool)
        .await?,
        0
    );
    f.shutdown().await
}

#[tokio::test]
async fn published_parent_expiry_restores_success_without_redelivery_or_false_terminal(
) -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1).await?;
    complete_all(&f).await?;
    let replay = f
        .schedule()
        .claim_replay_target(id, DeliveryTarget::Internal, &config())
        .await?;
    expire(&f, id, DeliveryTarget::Internal).await?;
    let recovered = f
        .schedule()
        .claim_target(DeliveryTarget::Internal, Utc::now(), 1, &config())
        .await?;
    assert!(recovered.claims.is_empty());
    assert_eq!(state(&f, id, DeliveryTarget::Internal).await?, "SUCCEEDED");
    assert_eq!(
        f.schedule().complete_target(&replay).await?,
        DeliveryLeaseOutcomeDto::Lost
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_attempts where attempt_id=$1 and phase<>'STARTED'").bind(replay.attempt_id).fetch_one(&f.pool).await?,0);
    assert_eq!(sqlx::query_scalar::<_,i32>("select attempt_count from post_event_delivery_jobs where event_id=$1 and target='INTERNAL'").bind(id).fetch_one(&f.pool).await?,1);
    f.shutdown().await
}

#[tokio::test]
async fn ordinary_replay_cannot_reset_dead_budget_and_audited_resolution_is_required() -> Result<()>
{
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1).await?;
    let policy = DeliveryScheduleConfigDto {
        max_attempts: 1,
        ..config()
    };
    let replay = f
        .schedule()
        .claim_replay_target(id, DeliveryTarget::Internal, &policy)
        .await?;
    assert_eq!(replay.attempt_number, 1);
    expire(&f, id, DeliveryTarget::Internal).await?;
    assert!(f
        .schedule()
        .claim_replay_target(id, DeliveryTarget::Internal, &policy)
        .await
        .is_err());
    assert!(f
        .schedule()
        .claim_replay_target(id, DeliveryTarget::External, &policy)
        .await
        .is_err());
    let resolution = Uuid::new_v4();
    f.resolve(id, resolution).await?;
    let released = f
        .schedule()
        .claim_replay_target(id, DeliveryTarget::Internal, &policy)
        .await?;
    assert_eq!(released.attempt_number, 1);
    assert_eq!(
        sqlx::query_scalar::<_, Option<Uuid>>(
            "select resolution_id from post_event_delivery_attempts where attempt_id=$1"
        )
        .bind(released.attempt_id)
        .fetch_one(&f.pool)
        .await?,
        Some(resolution)
    );
    f.schedule().complete_target(&released).await?;
    f.shutdown().await
}

#[tokio::test]
async fn unfinished_replay_obeys_head_order_and_frozen_due_time() -> Result<()> {
    let f = Fixture::new(22).await?;
    let aggregate = Uuid::new_v4();
    let first = event(&f, aggregate, 20).await?;
    let later = event(&f, aggregate, 10).await?;
    assert!(f
        .schedule()
        .claim_replay_target(later, DeliveryTarget::Internal, &config())
        .await
        .is_err());
    let head = f
        .schedule()
        .claim_replay_target(first, DeliveryTarget::Internal, &config())
        .await?;
    f.schedule().fail_target(&head).await?;
    assert!(f
        .schedule()
        .claim_replay_target(first, DeliveryTarget::Internal, &config())
        .await
        .is_err());
    f.due(first, DeliveryTarget::Internal).await?;
    let next = f
        .schedule()
        .claim_replay_target(first, DeliveryTarget::Internal, &config())
        .await?;
    assert_eq!(next.attempt_number, 2);
    f.schedule().complete_target(&next).await?;
    assert!(f
        .schedule()
        .claim_replay_target(later, DeliveryTarget::Internal, &config())
        .await
        .is_ok());
    f.shutdown().await
}

#[tokio::test]
async fn replay_started_insert_failure_rolls_back_registration_and_lease() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1).await?;
    sqlx::raw_sql("create function reject_manual_start() returns trigger language plpgsql as $$ begin if new.source='ADMIN_REPLAY' then raise exception 'injected history failure'; end if; return new; end $$; create trigger reject_manual_start before insert on post_event_delivery_attempts for each row execute function reject_manual_start();").execute(&f.pool).await?;
    assert!(f
        .schedule()
        .claim_replay_target(id, DeliveryTarget::Internal, &config())
        .await
        .is_err());
    assert_eq!(state(&f, id, DeliveryTarget::Internal).await?, "PENDING");
    assert_eq!(
        sqlx::query_scalar::<_, Option<Vec<String>>>(
            "select delivery_required_targets from post_event_outbox where event_id=$1"
        )
        .bind(id)
        .fetch_one(&f.pool)
        .await?,
        None
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from post_event_delivery_attempts")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    f.shutdown().await
}

#[tokio::test]
async fn failed_replay_that_finalizes_parent_still_returns_failure_from_use_case() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1).await?;
    complete_all(&f).await?;
    // Controlled already-checkpointed reconciliation state, not a normal write.
    sqlx::query("update post_event_outbox set published=false,published_at=null where event_id=$1")
        .bind(id)
        .execute(&f.pool)
        .await?;
    let effects = Arc::new(Effects {
        mode: Mode::Confirmed,
        ..Default::default()
    });
    let service = ReplayEventsUseCase::new(
        Arc::new(PgEventOutbox::new(f.pool.clone())),
        Arc::new(adapter(&f, effects, 1000)?),
    );
    assert!(service
        .execute(request(ReplayTarget::Internal))
        .await
        .is_err());
    assert!(
        sqlx::query_scalar::<_, bool>("select published from post_event_outbox where event_id=$1")
            .bind(id)
            .fetch_one(&f.pool)
            .await?
    );
    assert_eq!(state(&f, id, DeliveryTarget::Internal).await?, "SUCCEEDED");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase='FAILED'"
        )
        .fetch_one(&f.pool)
        .await?,
        1
    );
    f.shutdown().await
}

#[tokio::test]
async fn focused_replay_use_case_uses_fresh_database_event_and_records_both_targets() -> Result<()>
{
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1).await?;
    let effects = Arc::new(Effects::default());
    let service = ReplayEventsUseCase::new(
        Arc::new(PgEventOutbox::new(f.pool.clone())),
        Arc::new(adapter(&f, effects.clone(), 1000)?),
    );
    assert_eq!(
        service
            .execute(request(ReplayTarget::Both))
            .await?
            .replayed_count,
        1
    );
    {
        let recorded = effects.events.lock().unwrap();
        assert_eq!(recorded.len(), 2);
        assert!(recorded
            .iter()
            .all(|e| e.event_id == id && e.payload == "{\"fresh\":1}"));
    }
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_attempts where source='ADMIN_REPLAY' and phase='SUCCEEDED'").fetch_one(&f.pool).await?,2);
    assert!(
        !sqlx::query_scalar::<_, bool>("select published from post_event_outbox where event_id=$1")
            .bind(id)
            .fetch_one(&f.pool)
            .await?
    );
    f.shutdown().await
}

#[tokio::test]
async fn replay_timeout_and_inner_error_keep_uncertain_lease_until_recovery() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1).await?;
    for mode in [Mode::Block, Mode::Uncertain] {
        let effects = Arc::new(Effects {
            mode,
            ..Default::default()
        });
        let adapter = adapter(&f, effects, 10)?;
        assert!(adapter
            .replay_target(id, DeliveryTarget::Internal)
            .await
            .is_err());
        assert_eq!(state(&f, id, DeliveryTarget::Internal).await?, "LEASED");
        expire(&f, id, DeliveryTarget::Internal).await?;
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase<>'STARTED'"
        )
        .fetch_one(&f.pool)
        .await?,
        0
    );
    let healthy = adapter(&f, Arc::new(Effects::default()), 1000)?;
    healthy.replay_target(id, DeliveryTarget::Internal).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase='STARTED'"
        )
        .fetch_one(&f.pool)
        .await?,
        3
    );
    f.shutdown().await
}

#[tokio::test]
async fn sqlx_17_to_current_upgrade_preserves_existing_automatic_lease() -> Result<()> {
    let f = Fixture::new(17).await?;
    let id = event(&f, Uuid::new_v4(), 1).await?;
    let token = Uuid::new_v4();
    let attempt = Uuid::new_v4();
    sqlx::query("update post_event_delivery_jobs set state='LEASED',attempt_count=1,lease_token=$2,attempt_id=$3,lease_expires_at=clock_timestamp()+interval '5 minutes' where event_id=$1 and target='INTERNAL'").bind(id).bind(token).bind(attempt).execute(&f.pool).await?;
    sqlx::query("insert into post_event_delivery_attempts(attempt_id,event_id,target,source,phase,occurred_at) values($1,$2,'INTERNAL','OUTBOX_WORKER','STARTED',clock_timestamp())").bind(attempt).bind(id).execute(&f.pool).await?;
    f.upgrade().await?;
    let row=sqlx::query("select lease_token,attempt_id,lease_kind,replay_restore_state from post_event_delivery_jobs where event_id=$1 and target='INTERNAL'").bind(id).fetch_one(&f.pool).await?;
    assert_eq!(row.get::<Uuid, _>("lease_token"), token);
    assert_eq!(row.get::<Uuid, _>("attempt_id"), attempt);
    assert_eq!(row.get::<String, _>("lease_kind"), "AUTO");
    assert_eq!(row.get::<Option<String>, _>("replay_restore_state"), None);
    assert!(f
        .schedule()
        .claim_replay_target(id, DeliveryTarget::Internal, &config())
        .await
        .is_err());
    f.shutdown().await
}

async fn wait_for_blocked_backend(f: &Fixture) -> Result<()> {
    tokio::time::timeout(std::time::Duration::from_secs(2),async {
        loop {
            let blocked:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and cardinality(pg_blocking_pids(pid))>0)").fetch_one(&f.pool).await?;
            if blocked {return Ok::<(),anyhow::Error>(());}
            tokio::task::yield_now().await;
        }
    }).await??;
    Ok(())
}

#[tokio::test]
async fn replay_claim_checks_expiry_after_waiting_for_job_lock() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1).await?;
    let old = f.claim(DeliveryTarget::Internal, &config()).await?;
    let mut lock = f.pool.begin().await?;
    sqlx::query("select event_id from post_event_delivery_jobs where event_id=$1 and target='INTERNAL' for update").bind(id).fetch_one(&mut *lock).await?;
    let schedule = f.schedule();
    let task = tokio::spawn(async move {
        schedule
            .claim_replay_target(id, DeliveryTarget::Internal, &config())
            .await
    });
    wait_for_blocked_backend(&f).await?;
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target='INTERNAL'").bind(id).execute(&mut *lock).await?;
    lock.commit().await?;
    let replay = task.await??;
    assert_eq!(replay.attempt_number, 2);
    assert_eq!(
        f.schedule().complete_target(&old).await?,
        DeliveryLeaseOutcomeDto::Lost
    );
    f.schedule().complete_target(&replay).await?;
    f.shutdown().await
}

#[tokio::test]
async fn completed_replay_completion_rechecks_expiry_after_job_lock_wait() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1).await?;
    complete_all(&f).await?;
    let replay = f
        .schedule()
        .claim_replay_target(id, DeliveryTarget::Internal, &config())
        .await?;
    let mut lock = f.pool.begin().await?;
    sqlx::query("select event_id from post_event_delivery_jobs where event_id=$1 and target='INTERNAL' for update").bind(id).fetch_one(&mut *lock).await?;
    let schedule = f.schedule();
    let claim = replay.clone();
    let task = tokio::spawn(async move { schedule.complete_target(&claim).await });
    wait_for_blocked_backend(&f).await?;
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target='INTERNAL'").bind(id).execute(&mut *lock).await?;
    lock.commit().await?;
    assert_eq!(task.await??, DeliveryLeaseOutcomeDto::Lost);
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_attempts where attempt_id=$1 and phase<>'STARTED'").bind(replay.attempt_id).fetch_one(&f.pool).await?,0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from event_publications")
            .fetch_one(&f.pool)
            .await?,
        4
    );
    f.shutdown().await
}

struct ExpireBeforeDispatch {
    schedule: PgDeliverySchedule,
    pool: PgPool,
}
#[async_trait]
impl DeliverySchedulePort for ExpireBeforeDispatch {
    async fn lease_status(
        &self,
        claim: &TargetDeliveryClaimDto,
    ) -> Result<post_service::application::dto::delivery_schedule_dto::DeliveryLeaseStatusDto> {
        sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target=$2").bind(claim.event.event_id).bind(claim.target.as_str()).execute(&self.pool).await?;
        self.schedule.lease_status(claim).await
    }
    async fn claim_target(
        &self,
        target: DeliveryTarget,
        cutoff: chrono::DateTime<Utc>,
        limit: i32,
        config: &DeliveryScheduleConfigDto,
    ) -> Result<post_service::application::dto::delivery_schedule_dto::TargetDeliveryBatchDto> {
        self.schedule
            .claim_target(target, cutoff, limit, config)
            .await
    }
    async fn complete_target(
        &self,
        claim: &TargetDeliveryClaimDto,
    ) -> Result<DeliveryLeaseOutcomeDto> {
        self.schedule.complete_target(claim).await
    }
    async fn fail_target(&self, claim: &TargetDeliveryClaimDto) -> Result<DeliveryLeaseOutcomeDto> {
        self.schedule.fail_target(claim).await
    }
}
#[async_trait]
impl ReplaySchedulePort for ExpireBeforeDispatch {
    async fn claim_replay_target(
        &self,
        id: Uuid,
        target: DeliveryTarget,
        config: &DeliveryScheduleConfigDto,
    ) -> Result<TargetDeliveryClaimDto> {
        self.schedule.claim_replay_target(id, target, config).await
    }
}

#[tokio::test]
async fn replay_lost_database_ownership_prevents_dispatch() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1).await?;
    let effects = Arc::new(Effects::default());
    let replay = FencedReplayDelivery::new(
        Arc::new(ExpireBeforeDispatch {
            schedule: f.schedule(),
            pool: f.pool.clone(),
        }),
        effects.clone(),
        effects.clone(),
        Default::default(),
    )?;
    assert!(replay
        .replay_target(id, DeliveryTarget::Internal)
        .await
        .is_err());
    assert!(effects.events.lock().unwrap().is_empty());
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
async fn aborted_replay_task_recovers_without_erasing_uncertain_attempt() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1).await?;
    let effects = Arc::new(Effects {
        mode: Mode::Block,
        ..Default::default()
    });
    let replay = adapter(&f, effects.clone(), 1000)?;
    let task =
        tokio::spawn(async move { replay.replay_target(id, DeliveryTarget::Internal).await });
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        effects.entered.notified(),
    )
    .await?;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    expire(&f, id, DeliveryTarget::Internal).await?;
    adapter(&f, Arc::new(Effects::default()), 1000)?
        .replay_target(id, DeliveryTarget::Internal)
        .await?;
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
async fn review_expiry_crossing_preparation_wait_does_not_abort_healthy_claim() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1).await?;
    let policy = DeliveryScheduleConfigDto {
        max_attempts: 1,
        ..Default::default()
    };
    f.schedule()
        .claim_replay_target(id, DeliveryTarget::Internal, &policy)
        .await?;
    sqlx::raw_sql("create function pause_skipped_update() returns trigger language plpgsql as $$ begin if new.target='EXTERNAL' then perform pg_advisory_xact_lock(818181); end if; return new; end $$; create trigger pause_skipped_update before update of state on post_event_delivery_jobs for each row execute function pause_skipped_update();").execute(&f.pool).await?;
    let mut barrier = f.pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(818181)")
        .execute(&mut *barrier)
        .await?;
    let schedule = f.schedule();
    let task = tokio::spawn(async move {
        schedule
            .claim_target(DeliveryTarget::Cache, Utc::now(), 1, &policy)
            .await
    });
    wait_for_blocked_backend(&f).await?;
    // The initial replay cleanup saw a live lease. Expire it while preparation
    // is held at the later excluded-target reconciliation statement.
    expire(&f, id, DeliveryTarget::Internal).await?;
    barrier.commit().await?;
    let batch = task.await??;
    assert_eq!(batch.claims.len(), 1);
    assert_eq!(state(&f, id, DeliveryTarget::Internal).await?, "DEAD");
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
async fn review_exhausted_replay_expiry_updates_parent_dead_letter_summary() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1).await?;
    let policy = DeliveryScheduleConfigDto {
        max_attempts: 1,
        ..config()
    };
    f.schedule()
        .claim_replay_target(id, DeliveryTarget::Internal, &policy)
        .await?;
    expire(&f, id, DeliveryTarget::Internal).await?;
    f.schedule()
        .claim_target(DeliveryTarget::Internal, Utc::now(), 1, &policy)
        .await?;
    assert_eq!(state(&f, id, DeliveryTarget::Internal).await?, "DEAD");
    assert!(
        sqlx::query_scalar::<_, bool>(
            "select dead_lettered_at is not null from post_event_outbox where event_id=$1"
        )
        .bind(id)
        .fetch_one(&f.pool)
        .await?
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
async fn review_forced_skipped_replay_does_not_block_new_required_external_heads() -> Result<()> {
    let f = Fixture::new(22).await?;
    let aggregate = Uuid::new_v4();
    let old = event(&f, aggregate, 20).await?;
    let first = f
        .claim(
            DeliveryTarget::Internal,
            &DeliveryScheduleConfigDto::default(),
        )
        .await?;
    f.schedule().complete_target(&first).await?;
    let replay = f
        .schedule()
        .claim_replay_target(old, DeliveryTarget::External, &config())
        .await?;
    let newer = event(&f, aggregate, 10).await?;
    let batch = f
        .schedule()
        .claim_target(DeliveryTarget::External, Utc::now(), 1, &config())
        .await?;
    assert_eq!(batch.claims.len(), 1);
    assert_eq!(batch.claims[0].event.event_id, newer);
    f.schedule().complete_target(&batch.claims[0]).await?;
    let latest = event(&f, aggregate, 1).await?;
    let manual = f
        .schedule()
        .claim_replay_target(latest, DeliveryTarget::External, &config())
        .await?;
    assert_eq!(manual.event.event_id, latest);
    assert!(matches!(
        f.schedule().lease_status(&replay).await?,
        post_service::application::dto::delivery_schedule_dto::DeliveryLeaseStatusDto::Owned { .. }
    ));
    f.shutdown().await
}
