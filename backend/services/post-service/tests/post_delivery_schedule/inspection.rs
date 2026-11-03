use super::*;
use serde_json::Value;
use sqlx::postgres::PgRow;

const QUERY: &str =
    include_str!("../../../../scripts/admin/post-outbox-dead-letter-inspection.sql");

async fn inspect(f: &Fixture) -> Result<Vec<PgRow>> {
    let mut tx = f.pool.begin().await?;
    sqlx::query("set transaction read only")
        .execute(&mut *tx)
        .await?;
    assert_eq!(
        sqlx::query_scalar::<_, String>("show transaction_read_only")
            .fetch_one(&mut *tx)
            .await?,
        "on"
    );
    let rows = sqlx::query(QUERY).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(rows)
}

async fn snapshot(f: &Fixture) -> Result<Value> {
    Ok(sqlx::query_scalar(
        "select jsonb_build_object(
            'events',(select jsonb_agg(to_jsonb(t) order by event_id) from post_event_outbox t),
            'jobs',(select jsonb_agg(to_jsonb(t) order by event_id,target) from post_event_delivery_jobs t),
            'publications',(select jsonb_agg(to_jsonb(t) order by event_id,target) from event_publications t),
            'attempts',(select jsonb_agg(to_jsonb(t) order by attempt_id,phase) from post_event_delivery_attempts t),
            'resolutions',(select jsonb_agg(to_jsonb(t) order by resolution_id) from post_event_dead_letter_resolutions t))",
    ).fetch_one(&f.pool).await?)
}

async fn dead(f: &Fixture, id: Uuid, target: &str) -> Result<()> {
    sqlx::query("update post_event_delivery_jobs set state='DEAD',attempt_count=max_attempts where event_id=$1 and target=$2")
        .bind(id).bind(target).execute(&f.pool).await?;
    Ok(())
}

#[tokio::test]
async fn expired_exhausted_target_is_visible_before_summary_without_mutation() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = f.save(Uuid::new_v4(), Some(1), 1).await?;
    sqlx::query("update post_event_outbox set payload='SECRET_PAYLOAD',last_error='SECRET_ERROR' where event_id=$1")
        .bind(id).execute(&f.pool).await?;
    let claim = f
        .claim(
            DeliveryTarget::Internal,
            &DeliveryScheduleConfigDto {
                max_attempts: 1,
                ..Default::default()
            },
        )
        .await?;
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target='INTERNAL'")
        .bind(id).execute(&f.pool).await?;
    let before = snapshot(&f).await?;
    let rows = inspect(&f).await?;
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.get::<Uuid, _>("event_id"), id);
    assert_eq!(
        row.get::<String, _>("recovery_observation"),
        "OBSERVED_ELIGIBLE_FOR_AUDITED_RECOVERY"
    );
    assert!(row
        .get::<Option<chrono::DateTime<Utc>>, _>("parent_dead_letter_summary")
        .is_none());
    let targets: Value = row.get("target_observations");
    let internal = targets
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["target"] == "INTERNAL")
        .unwrap();
    assert_eq!(internal["state"], "LEASED");
    assert_eq!(internal["leaseObservation"], "EXPIRED");
    assert_eq!(internal["attemptCount"], 1);
    assert_eq!(internal["checkpoint"], false);
    let output = targets.to_string();
    for secret in [
        "SECRET_PAYLOAD",
        "SECRET_ERROR",
        &claim.lease_token.to_string(),
        &claim.attempt_id.to_string(),
    ] {
        assert!(!output.contains(secret));
    }
    assert_eq!(snapshot(&f).await?, before);
    assert!(f.resolve(id, Uuid::new_v4()).await?.created);
    f.shutdown().await
}

#[tokio::test]
async fn excluded_dead_targets_and_completed_checkpoints_do_not_authorize_recovery() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = f.save(Uuid::new_v4(), Some(1), 1).await?;
    sqlx::query("update post_event_outbox set delivery_required_targets=array['INTERNAL'] where event_id=$1")
        .bind(id).execute(&f.pool).await?;
    dead(&f, id, "EXTERNAL").await?;
    assert!(inspect(&f).await?.is_empty());
    dead(&f, id, "INTERNAL").await?;
    f.checkpoint(id, DeliveryTarget::Internal).await?;
    assert!(inspect(&f).await?.is_empty());
    sqlx::query("update post_event_outbox set dead_lettered_at=now() where event_id=$1")
        .bind(id)
        .execute(&f.pool)
        .await?;
    let rows = inspect(&f).await?;
    assert_eq!(
        rows[0].get::<String, _>("recovery_observation"),
        "NO_ELIGIBLE_UNFINISHED_JOB"
    );
    let values: Value = rows[0].get("target_observations");
    let external = values
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["target"] == "EXTERNAL")
        .unwrap();
    assert_eq!(external["required"], false);
    assert_eq!(external["state"], "DEAD");
    sqlx::query("update post_event_outbox set published=true where event_id=$1")
        .bind(id)
        .execute(&f.pool)
        .await?;
    assert!(inspect(&f).await?.is_empty());
    f.shutdown().await
}

#[tokio::test]
async fn any_live_target_lease_and_legacy_parent_lock_are_visible_conflicts() -> Result<()> {
    use post_service::application::ports::ReplaySchedulePort;
    let f = Fixture::new(22).await?;
    let id = f.save(Uuid::new_v4(), Some(1), 1).await?;
    let timeline = f
        .claim(DeliveryTarget::Timeline, &Default::default())
        .await?;
    f.schedule().complete_target(&timeline).await?;
    f.schedule()
        .claim_replay_target(
            id,
            DeliveryTarget::External,
            &DeliveryScheduleConfigDto {
                external_enabled: true,
                ..Default::default()
            },
        )
        .await?;
    dead(&f, id, "INTERNAL").await?;
    let rows = inspect(&f).await?;
    assert_eq!(
        rows[0].get::<String, _>("recovery_observation"),
        "LIVE_TARGET_LEASE"
    );
    assert!(f.resolve(id, Uuid::new_v4()).await.is_err());
    sqlx::query("update post_event_outbox set locked_at=now() where event_id=$1")
        .bind(id)
        .execute(&f.pool)
        .await?;
    assert_eq!(
        inspect(&f).await?[0].get::<String, _>("recovery_observation"),
        "LEGACY_PARENT_LOCK_PRESENT"
    );
    f.shutdown().await
}

#[tokio::test]
async fn missing_required_job_is_visible_and_cannot_be_reported_recoverable() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = f.save(Uuid::new_v4(), Some(1), 1).await?;
    sqlx::query("update post_event_outbox set delivery_required_targets=array['INTERNAL'] where event_id=$1")
        .bind(id).execute(&f.pool).await?;
    sqlx::query("delete from post_event_delivery_jobs where event_id=$1 and target='INTERNAL'")
        .bind(id)
        .execute(&f.pool)
        .await?;
    let rows = inspect(&f).await?;
    assert_eq!(
        rows[0].get::<String, _>("recovery_observation"),
        "JOB_SET_INCOMPLETE"
    );
    let targets: Value = rows[0].get("target_observations");
    assert!(targets
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["target"] == "INTERNAL" && t["state"] == "MISSING"));
    assert!(f.resolve(id, Uuid::new_v4()).await.is_err());
    f.shutdown().await
}

#[tokio::test]
async fn legacy_summary_is_required_and_required_mask_is_unknown() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = f.save(Uuid::new_v4(), None, 1).await?;
    dead(&f, id, "INTERNAL").await?;
    let rows = inspect(&f).await?;
    assert_eq!(
        rows[0].get::<String, _>("recovery_observation"),
        "LEGACY_SUMMARY_MISSING"
    );
    assert!(f.resolve(id, Uuid::new_v4()).await.is_err());
    sqlx::query("update post_event_outbox set dead_lettered_at=now() where event_id=$1")
        .bind(id)
        .execute(&f.pool)
        .await?;
    let rows = inspect(&f).await?;
    assert_eq!(
        rows[0].get::<String, _>("recovery_observation"),
        "OBSERVED_ELIGIBLE_FOR_AUDITED_RECOVERY"
    );
    let targets: Value = rows[0].get("target_observations");
    assert!(targets
        .as_array()
        .unwrap()
        .iter()
        .all(|t| t["required"].is_null()));
    assert!(f.resolve(id, Uuid::new_v4()).await?.created);
    f.shutdown().await
}

#[tokio::test]
async fn aggregate_counts_preserve_legacy_then_version_order() -> Result<()> {
    let f = Fixture::new(22).await?;
    let aggregate = Uuid::new_v4();
    let old = f.save(aggregate, None, 30).await?;
    let newer = f.save(aggregate, None, 20).await?;
    let versioned = f.save(aggregate, Some(3), 10).await?;
    for id in [old, newer, versioned] {
        sqlx::query("update post_event_outbox set dead_lettered_at=now() where event_id=$1")
            .bind(id)
            .execute(&f.pool)
            .await?;
    }
    let rows = inspect(&f).await?;
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.get::<i64, _>("unpublished_predecessors"), i as i64);
        assert_eq!(row.get::<i64, _>("unpublished_successors"), 2 - i as i64);
    }
    f.shutdown().await
}

#[tokio::test]
async fn inspection_returns_at_most_100_candidates() -> Result<()> {
    let f = Fixture::new(22).await?;
    for _ in 0..101 {
        let id = f.save(Uuid::new_v4(), Some(1), 1).await?;
        sqlx::query("update post_event_outbox set dead_lettered_at=now() where event_id=$1")
            .bind(id)
            .execute(&f.pool)
            .await?;
    }
    assert_eq!(inspect(&f).await?.len(), 100);
    f.shutdown().await
}
