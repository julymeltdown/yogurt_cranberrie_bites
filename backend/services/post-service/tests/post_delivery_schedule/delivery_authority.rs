//! Real current-schema PG owner lookups, not network capability acceptance.
use super::*;
use post_service::adapters::out::persistence::{
    delivery_authority_repository::PgDeliveryAuthorityRepository,
    timeline_projection_repository::PgTimelineProjectionRepository,
};
use post_service::application::delivery_authority::{
    DeliveryAuthorityErrorDto as Error, DeliveryAuthorityRepositoryPort, DeliveryClaimLookupDto,
    DeliveryLeaseKindDto, DeliveryReplayRestoreDto,
};
use post_service::application::dto::timeline_projection_dto::{
    TimelineProjectionApplyDto, TimelineProjectionWorkDto,
};
use post_service::application::ports::{ReplaySchedulePort, TimelineProjectionRepositoryPort};
use post_service::domain::post::DeliveryAttemptSource;
use std::time::Duration as StdDuration;

fn reader(f: &Fixture) -> PgDeliveryAuthorityRepository {
    PgDeliveryAuthorityRepository::new(f.pool.clone())
}
async fn snapshot(f: &Fixture) -> Result<String> {
    Ok(sqlx::query_scalar("select jsonb_build_object( \
        'outbox',(select jsonb_agg(to_jsonb(e) order by event_id) from post_event_outbox e), \
        'jobs',(select jsonb_agg(to_jsonb(j) order by event_id,target) from post_event_delivery_jobs j), \
        'attempts',(select jsonb_agg(to_jsonb(a) order by id) from post_event_delivery_attempts a), \
        'resolutions',(select jsonb_agg(to_jsonb(r) order by resolution_id) from post_event_dead_letter_resolutions r), \
        'publications',(select jsonb_agg(to_jsonb(p) order by event_id,target) from event_publications p), \
        'posts',(select jsonb_agg(to_jsonb(p) order by id) from posts p), \
        'timeline',(select jsonb_agg(to_jsonb(t) order by viewer_id,post_id,source) from home_timeline_entries t), \
        'receipts',(select jsonb_agg(to_jsonb(r) order by event_id) from timeline_projection_receipts r))::text")
        .fetch_one(&f.pool).await?)
}
async fn denied(f: &Fixture, claim: &TargetDeliveryClaimDto) -> Result<()> {
    let before = snapshot(f).await?;
    assert_eq!(
        reader(f)
            .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(claim.clone())?)
            .await
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(snapshot(f).await?, before);
    Ok(())
}
async fn seed(f: &Fixture, target: DeliveryTarget, ms: u64) -> Result<TargetDeliveryClaimDto> {
    f.save(Uuid::new_v4(), Some(1), 1).await?;
    f.claim(
        target,
        &DeliveryScheduleConfigDto {
            external_enabled: true,
            max_attempts: 1,
            lease_duration_ms: ms,
            ..Default::default()
        },
    )
    .await
}

#[tokio::test]
async fn all_automatic_targets_accept_the_final_attempt_without_changes() -> Result<()> {
    let f = Fixture::new(29).await?;
    // The same parent retains independently owned target claims.
    f.save(Uuid::new_v4(), Some(1), 1).await?;
    for target in DeliveryTarget::ALL {
        let claim = f
            .claim(
                target,
                &DeliveryScheduleConfigDto {
                    external_enabled: true,
                    max_attempts: 1,
                    ..Default::default()
                },
            )
            .await?;
        let before = snapshot(&f).await?;
        let owned = reader(&f)
            .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(claim.clone())?)
            .await?;
        let actual = owned.lookup().claim();
        assert_eq!(actual.event.event_id, claim.event.event_id);
        assert_eq!(actual.target, target);
        assert_eq!(actual.lease_token, claim.lease_token);
        assert_eq!(actual.attempt_id, claim.attempt_id);
        assert_eq!(actual.attempt_number, 1);
        assert_eq!(actual.lease_expires_at, claim.lease_expires_at);
        let p = owned.provenance();
        assert_eq!(p.lease_kind, DeliveryLeaseKindDto::Automatic);
        assert_eq!(p.source, DeliveryAttemptSource::OutboxWorker);
        assert_eq!(p.resolution_id, None);
        assert!(p.target_required);
        assert_eq!((p.max_attempts, p.progress_credits), (1, 0));
        assert!(owned.checked_at() < owned.expires_at());
        assert_eq!(snapshot(&f).await?, before);
    }
    f.shutdown().await
}

#[tokio::test]
async fn forged_immutable_source_and_claim_bindings_deny_without_effects() -> Result<()> {
    let f = Fixture::new(29).await?;
    let claim = seed(&f, DeliveryTarget::Internal, 60_000).await?;
    for case in 0..16 {
        let mut bad = claim.clone();
        match case {
            0 => bad.event.event_id = Uuid::new_v4(),
            1 => bad.event.aggregate_id = Uuid::new_v4(),
            2 => bad.event.event_type = "POST_DELETED".into(),
            3 => bad.event.aggregate_version = Some(2),
            4 => bad.event.actor_id = Some(Uuid::new_v4()),
            5 => bad.event.attributes.push("forged".into()),
            6 => bad.event.payload.push(' '), // exact bytes, even equal JSON
            7 => bad.event.occurred_at += Duration::microseconds(1),
            8 => bad.event.created_at += Duration::microseconds(1),
            9 => {
                bad.event.origin_traceparent =
                    Some("00-11111111111111111111111111111111-2222222222222222-01".into())
            }
            10 => bad.target = DeliveryTarget::External,
            11 => bad.lease_token = Uuid::new_v4(),
            12 => bad.attempt_id = Uuid::new_v4(),
            13 => bad.attempt_number += 1,
            14 => bad.lease_expires_at += Duration::microseconds(1),
            _ => bad.event.aggregate_version = None,
        }
        denied(&f, &bad).await?;
    }
    for case in 0..7 {
        let mut bad = claim.clone();
        match case {
            0 => bad.event.event_id = Uuid::nil(),
            1 => bad.event.aggregate_id = Uuid::nil(),
            2 => bad.lease_token = Uuid::nil(),
            3 => bad.attempt_id = Uuid::nil(),
            4 => bad.attempt_number = -1,
            5 => bad.event.aggregate_version = Some(0),
            _ => bad.event.event_type.clear(),
        }
        assert_eq!(
            DeliveryClaimLookupDto::new(bad).err(),
            Some(Error::InvalidBinding)
        );
    }
    f.shutdown().await
}

#[tokio::test]
async fn mutable_caller_metadata_cannot_replace_stored_publication_or_resolution() -> Result<()> {
    let f = Fixture::new(29).await?;
    let mut claim = seed(&f, DeliveryTarget::Internal, 60_000).await?;
    claim.event.published = true;
    claim.event.published_at = Some(Utc::now());
    claim.event.replay_resolution_id = Some(Uuid::new_v4());
    let before = snapshot(&f).await?;
    let owned = reader(&f)
        .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(claim)?)
        .await?;
    assert!(!owned.lookup().claim().event.published);
    assert!(owned.lookup().claim().event.published_at.is_none());
    assert!(owned.lookup().claim().event.replay_resolution_id.is_none());
    assert_eq!(snapshot(&f).await?, before);
    f.shutdown().await
}

#[tokio::test]
async fn completed_and_excluded_replays_preserve_published_parent_and_zero_budget() -> Result<()> {
    let f = Fixture::new(29).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let cfg = DeliveryScheduleConfigDto::default();
    for target in [
        DeliveryTarget::Timeline,
        DeliveryTarget::Cache,
        DeliveryTarget::Internal,
    ] {
        let claim = f.claim(target, &cfg).await?;
        f.schedule().complete_target(&claim).await?;
    }
    // INTERNAL completed with a checkpoint; EXTERNAL was excluded and SKIPPED.
    for (target, restore, attempt, required) in [
        (
            DeliveryTarget::Internal,
            DeliveryReplayRestoreDto::Succeeded,
            1,
            true,
        ),
        (
            DeliveryTarget::External,
            DeliveryReplayRestoreDto::Skipped,
            0,
            false,
        ),
    ] {
        let mut claim = f
            .schedule()
            .claim_replay_target(
                event,
                target,
                &DeliveryScheduleConfigDto {
                    external_enabled: true,
                    ..cfg.clone()
                },
            )
            .await?;
        // Simulate stale caller metadata during otherwise valid published replay.
        claim.event.published = false;
        claim.event.published_at = None;
        let before = snapshot(&f).await?;
        let owned = reader(&f)
            .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(claim.clone())?)
            .await?;
        assert!(owned.lookup().claim().event.published);
        assert_eq!(owned.lookup().claim().attempt_number, attempt);
        let p = owned.provenance();
        assert_eq!(p.lease_kind, DeliveryLeaseKindDto::Replay(restore));
        assert_eq!(p.source, DeliveryAttemptSource::AdminReplay);
        assert_eq!(p.resolution_id, None);
        assert_eq!(p.target_required, required);
        assert_eq!(snapshot(&f).await?, before);
        f.schedule().complete_target(&claim).await?;
    }
    f.shutdown().await
}

#[tokio::test]
async fn ordinary_pending_and_retry_replays_accept_resolution_free_final_attempts() -> Result<()> {
    let f = Fixture::new(29).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let cfg = DeliveryScheduleConfigDto {
        max_attempts: 2,
        ..Default::default()
    };
    let first = f
        .schedule()
        .claim_replay_target(event, DeliveryTarget::Internal, &cfg)
        .await?;
    let owned = reader(&f)
        .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(first.clone())?)
        .await?;
    assert_eq!(
        owned.provenance().lease_kind,
        DeliveryLeaseKindDto::Replay(DeliveryReplayRestoreDto::Pending)
    );
    f.schedule().fail_target(&first).await?;
    denied(&f, &first).await?;
    f.due(event, DeliveryTarget::Internal).await?;
    let second = f
        .schedule()
        .claim_replay_target(event, DeliveryTarget::Internal, &cfg)
        .await?;
    let before = snapshot(&f).await?;
    let owned = reader(&f)
        .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(second)?)
        .await?;
    assert_eq!(
        owned.provenance().lease_kind,
        DeliveryLeaseKindDto::Replay(DeliveryReplayRestoreDto::Retry)
    );
    assert_eq!(
        owned.lookup().claim().attempt_number,
        owned.provenance().max_attempts
    );
    assert_eq!(owned.provenance().resolution_id, None);
    assert_eq!(snapshot(&f).await?, before);
    f.shutdown().await
}

#[tokio::test]
async fn audited_recovery_keeps_auto_lease_with_admin_attempt_provenance() -> Result<()> {
    let f = Fixture::new(29).await?;
    let first = seed(&f, DeliveryTarget::Internal, 60_000).await?;
    f.schedule().fail_target(&first).await?;
    let resolution = Uuid::new_v4();
    f.resolve(first.event.event_id, resolution).await?;
    let second = f
        .claim(DeliveryTarget::Internal, &Default::default())
        .await?;
    let before = snapshot(&f).await?;
    let owned = reader(&f)
        .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(second)?)
        .await?;
    let p = owned.provenance();
    assert_eq!(p.lease_kind, DeliveryLeaseKindDto::Automatic);
    assert_eq!(p.source, DeliveryAttemptSource::AdminReplay);
    assert_eq!(p.resolution_id, Some(resolution));
    assert_eq!(p.max_attempts, 1);
    assert_eq!(snapshot(&f).await?, before);
    denied(&f, &first).await?;
    f.shutdown().await
}

#[tokio::test]
async fn real_projection_progress_credit_allows_lifetime_attempt_above_frozen_max() -> Result<()> {
    let f = Fixture::new(29).await?;
    let (post, author) = super::projection::seed(&f).await?;
    let event = super::projection::event(&f, post, author, author, "POST_CREATED").await?;
    let first = f
        .claim(
            DeliveryTarget::Timeline,
            &DeliveryScheduleConfigDto {
                max_attempts: 1,
                ..Default::default()
            },
        )
        .await?;
    PgTimelineProjectionRepository::new(f.pool.clone())
        .apply(&TimelineProjectionApplyDto {
            authority: Some(first.clone()),
            work: TimelineProjectionWorkDto {
                event: event.clone(),
                effect: event.effect()?,
            },
            follower_count: Some(2),
            high_fanout_threshold: 1000,
        })
        .await?;
    // Explicit expiry injection isolates real scheduler progress-credit recovery.
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target='TIMELINE'").bind(event.event_id).execute(&f.pool).await?;
    let next = f
        .claim(DeliveryTarget::Timeline, &Default::default())
        .await?;
    let before = snapshot(&f).await?;
    let owned = reader(&f)
        .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(next)?)
        .await?;
    assert_eq!(owned.lookup().claim().attempt_number, 2);
    assert_eq!(
        (
            owned.provenance().max_attempts,
            owned.provenance().progress_credits
        ),
        (1, 1)
    );
    assert_eq!(snapshot(&f).await?, before);
    denied(&f, &first).await?;
    f.shutdown().await
}

#[tokio::test]
async fn missing_or_wrong_started_and_all_terminal_phases_deny() -> Result<()> {
    for case in 0..6 {
        let f = Fixture::new(29).await?;
        let mut claim = seed(&f, DeliveryTarget::Internal, 60_000).await?;
        match case {
            0..=2 => {
                let phase = ["SUCCEEDED", "FAILED", "PROGRESSED"][case];
                sqlx::query("insert into post_event_delivery_attempts(attempt_id,event_id,target,source,phase,occurred_at) values($1,$2,'INTERNAL','OUTBOX_WORKER',$3,clock_timestamp())")
                    .bind(claim.attempt_id).bind(claim.event.event_id).bind(phase).execute(&f.pool).await?;
            }
            _ => {
                claim.attempt_id = Uuid::new_v4();
                sqlx::query("update post_event_delivery_jobs set attempt_id=$2 where event_id=$1 and target='INTERNAL'").bind(claim.event.event_id).bind(claim.attempt_id).execute(&f.pool).await?;
                if case >= 4 {
                    let other = f.save(Uuid::new_v4(), Some(1), 1).await?;
                    let resolution = if case == 5 {
                        Some(f.resolution(other).await?)
                    } else {
                        None
                    };
                    sqlx::query("insert into post_event_delivery_attempts(attempt_id,event_id,target,source,phase,occurred_at,resolution_id) values($1,$2,'INTERNAL','ADMIN_REPLAY','STARTED',clock_timestamp(),$3)")
                        .bind(claim.attempt_id).bind(if case==4 {other} else {claim.event.event_id}).bind(resolution).execute(&f.pool).await?;
                }
            }
        }
        denied(&f, &claim).await?;
        f.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn current_parent_mask_legacy_lock_budget_and_checkpoint_are_not_caller_policy() -> Result<()>
{
    for case in 0..6 {
        let f = Fixture::new(29).await?;
        let mut claim = seed(&f, DeliveryTarget::Internal, 60_000).await?;
        match case {
            0 => {
                sqlx::query(
                    "update post_event_outbox set delivery_required_targets=null where event_id=$1",
                )
                .bind(claim.event.event_id)
                .execute(&f.pool)
                .await?;
            }
            1 => {
                sqlx::query("update post_event_outbox set delivery_required_targets=array['CACHE'] where event_id=$1").bind(claim.event.event_id).execute(&f.pool).await?;
            }
            2 => {
                sqlx::query(
                    "update post_event_outbox set locked_at=clock_timestamp() where event_id=$1",
                )
                .bind(claim.event.event_id)
                .execute(&f.pool)
                .await?;
            }
            3 => {
                claim.attempt_number = 2;
                sqlx::query("update post_event_delivery_jobs set attempt_count=2 where event_id=$1 and target='INTERNAL'").bind(claim.event.event_id).execute(&f.pool).await?;
            }
            4 => {
                f.checkpoint(claim.event.event_id, DeliveryTarget::Internal)
                    .await?;
            }
            _ => {
                sqlx::query("update post_event_outbox set published=true,published_at=clock_timestamp() where event_id=$1").bind(claim.event.event_id).execute(&f.pool).await?;
            }
        }
        denied(&f, &claim).await?;
        f.shutdown().await?;
    }
    Ok(())
}

async fn wait_for_blocker(f: &Fixture, pid: i32) -> Result<()> {
    tokio::time::timeout(StdDuration::from_secs(5),async {
        loop {
            let blocked:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and $1=any(pg_blocking_pids(pid)))").bind(pid).fetch_one(&f.pool).await?;
            if blocked{return Ok::<_,anyhow::Error>(());}
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }
    }).await??;
    Ok(())
}
async fn expire_naturally(f: &Fixture, claim: &TargetDeliveryClaimDto) -> Result<()> {
    sqlx::query("select pg_sleep(greatest(0,extract(epoch from ($1::timestamptz-clock_timestamp())))::double precision+0.03)").bind(claim.lease_expires_at).execute(&f.pool).await?;
    Ok(())
}
async fn expiry_after_wait(kind: usize) -> Result<()> {
    let f = Fixture::new(29).await?;
    let claim = seed(&f, DeliveryTarget::Internal, 1500).await?;
    let before = snapshot(&f).await?;
    let mut locker = f.pool.begin().await?;
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *locker)
        .await?;
    match kind {
        0 => {
            sqlx::query("select event_id from post_event_outbox where event_id=$1 for update")
                .bind(claim.event.event_id)
                .execute(&mut *locker)
                .await?;
        }
        1 => {
            sqlx::query("select event_id from post_event_delivery_jobs where event_id=$1 and target='INTERNAL' for update").bind(claim.event.event_id).execute(&mut *locker).await?;
        }
        _ => {
            sqlx::query("lock table post_event_delivery_attempts in access exclusive mode")
                .execute(&mut *locker)
                .await?;
        }
    }
    let repo = reader(&f);
    let lookup = DeliveryClaimLookupDto::new(claim.clone())?;
    let pending = tokio::spawn(async move { repo.resolve_claimed_delivery(&lookup).await });
    wait_for_blocker(&f, pid).await?;
    expire_naturally(&f, &claim).await?;
    locker.commit().await?;
    assert_eq!(pending.await?.err(), Some(Error::Denied));
    assert_eq!(snapshot(&f).await?, before);
    f.shutdown().await
}
#[tokio::test]
async fn unchanged_lease_expiry_after_parent_row_wait_denies() -> Result<()> {
    expiry_after_wait(0).await
}
#[tokio::test]
async fn unchanged_lease_expiry_after_job_row_wait_denies() -> Result<()> {
    expiry_after_wait(1).await
}
#[tokio::test]
async fn unchanged_lease_expiry_after_attempt_table_wait_denies() -> Result<()> {
    expiry_after_wait(2).await
}

#[tokio::test]
async fn completed_and_replaced_leases_deny_and_missing_post_does_not_erase_event_owner(
) -> Result<()> {
    let f = Fixture::new(29).await?;
    let claim = seed(&f, DeliveryTarget::Cache, 60_000).await?;
    // No Post exists. A retained deletion event remains owner-resolvable; this
    // does not yet derive deletion/cache recipients from its payload.
    sqlx::query("update post_event_outbox set event_type='POST_DELETED' where event_id=$1")
        .bind(claim.event.event_id)
        .execute(&f.pool)
        .await?;
    let mut deleted = claim.clone();
    deleted.event.event_type = "POST_DELETED".into();
    let before = snapshot(&f).await?;
    reader(&f)
        .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(deleted.clone())?)
        .await?;
    assert_eq!(snapshot(&f).await?, before);
    sqlx::query(
        "update post_event_delivery_jobs set lease_token=$2 where event_id=$1 and target='CACHE'",
    )
    .bind(deleted.event.event_id)
    .bind(Uuid::new_v4())
    .execute(&f.pool)
    .await?;
    denied(&f, &deleted).await?;
    sqlx::query(
        "update post_event_delivery_jobs set lease_token=$2 where event_id=$1 and target='CACHE'",
    )
    .bind(deleted.event.event_id)
    .bind(deleted.lease_token)
    .execute(&f.pool)
    .await?;
    f.schedule().complete_target(&deleted).await?;
    denied(&f, &deleted).await?;
    f.shutdown().await
}

#[tokio::test]
async fn closed_owner_storage_is_unavailable_not_authority() -> Result<()> {
    let f = Fixture::new(29).await?;
    let claim = seed(&f, DeliveryTarget::Internal, 60_000).await?;
    let pool = PgPoolOptions::new()
        .connect_with(f.pool.connect_options().as_ref().clone())
        .await?;
    let repo = PgDeliveryAuthorityRepository::new(pool.clone());
    pool.close().await;
    let before = snapshot(&f).await?;
    assert_eq!(
        repo.resolve_claimed_delivery(&DeliveryClaimLookupDto::new(claim)?)
            .await
            .err(),
        Some(Error::Unavailable)
    );
    assert_eq!(snapshot(&f).await?, before);
    f.shutdown().await
}

#[tokio::test]
async fn valid_replay_observes_publication_and_parent_resolution_changes_from_storage() -> Result<()>
{
    let f = Fixture::new(29).await?;
    let event = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let claim = f
        .schedule()
        .claim_replay_target(event, DeliveryTarget::Internal, &Default::default())
        .await?;
    let resolution = f.resolution(event).await?;
    sqlx::query("update post_event_outbox set published=true,published_at=clock_timestamp() where event_id=$1").bind(event).execute(&f.pool).await?;
    let before = snapshot(&f).await?;
    let owned = reader(&f)
        .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(claim)?)
        .await?;
    assert!(owned.lookup().claim().event.published);
    assert_eq!(
        owned.lookup().claim().event.replay_resolution_id,
        Some(resolution)
    );
    // Parent resolution is not this existing target attempt's provenance.
    assert_eq!(owned.provenance().resolution_id, None);
    assert_eq!(snapshot(&f).await?, before);
    f.shutdown().await
}

#[tokio::test]
async fn unsupported_replay_targets_and_unproven_auto_admin_source_deny() -> Result<()> {
    for target in [DeliveryTarget::Cache, DeliveryTarget::Internal] {
        let f = Fixture::new(29).await?;
        let mut claim = seed(&f, target, 60_000).await?;
        claim.attempt_id = Uuid::new_v4();
        sqlx::query("update post_event_delivery_jobs set attempt_id=$3,lease_kind=$4,replay_restore_state=$5 where event_id=$1 and target=$2")
            .bind(claim.event.event_id).bind(target.as_str()).bind(claim.attempt_id)
            .bind(if target==DeliveryTarget::Cache {"REPLAY"}else{"AUTO"})
            .bind(if target==DeliveryTarget::Cache {Some("SUCCEEDED")}else{None})
            .execute(&f.pool).await?;
        sqlx::query("insert into post_event_delivery_attempts(attempt_id,event_id,target,source,phase,occurred_at) values($1,$2,$3,'ADMIN_REPLAY','STARTED',clock_timestamp())")
            .bind(claim.attempt_id).bind(claim.event.event_id).bind(target.as_str()).execute(&f.pool).await?;
        denied(&f, &claim).await?;
        f.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn legitimate_legacy_event_and_retained_trace_remain_exact_owner_data() -> Result<()> {
    let f = Fixture::new(29).await?;
    let event = f.save(Uuid::new_v4(), None, 1).await?;
    let trace = "00-11111111111111111111111111111111-2222222222222222-01";
    sqlx::query("update post_event_outbox set origin_traceparent=$2 where event_id=$1")
        .bind(event)
        .bind(trace)
        .execute(&f.pool)
        .await?;
    let claim = f
        .claim(DeliveryTarget::Internal, &Default::default())
        .await?;
    assert!(claim.event.aggregate_version.is_none());
    let before = snapshot(&f).await?;
    let owned = reader(&f)
        .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(claim.clone())?)
        .await?;
    assert!(owned.lookup().claim().event.aggregate_version.is_none());
    assert_eq!(
        owned.lookup().claim().event.origin_traceparent.as_deref(),
        Some(trace)
    );
    assert_eq!(snapshot(&f).await?, before);
    let mut invented_sequence = claim.clone();
    invented_sequence.event.aggregate_version = Some(1);
    denied(&f, &invented_sequence).await?;
    let mut discarded_trace = claim;
    discarded_trace.event.origin_traceparent = None;
    denied(&f, &discarded_trace).await?;
    // Owner data does not establish timeline adoption or delivery recipients.
    f.shutdown().await
}
