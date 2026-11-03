//! Owner-validation acceptance uses real isolated PostgreSQL, never a fake job.
use super::*;
use post_service::adapters::out::persistence::{
    fanout_authority_repository::PgFanoutAuthorityRepository,
    feed_recommendation_repository::PgFeedRecommendationRepository,
};
use post_service::application::dto::timeline_fanout_dto::{
    TimelineFanoutClaimDto, TimelineFanoutClaimRequestDto, TimelineFanoutOutcomeDto,
    TimelineFanoutPageDto,
};
use post_service::application::fanout_authority::{
    FanoutAuthorityErrorDto as Error, FanoutAuthorityRepositoryPort, FanoutPageLookupDto,
    FanoutPagePolicyDto,
};
use post_service::application::ports::TimelineFanoutRepositoryPort;
use std::time::Duration as StdDuration;

fn reader(f: &Fixture, max_page_size: i32) -> Result<PgFanoutAuthorityRepository> {
    Ok(PgFanoutAuthorityRepository::new(
        f.pool.clone(),
        FanoutPagePolicyDto::new(max_page_size)?,
    ))
}
async fn seed(
    f: &Fixture,
    lease: StdDuration,
    max_attempts: i32,
) -> Result<(PgFeedRecommendationRepository, TimelineFanoutClaimDto)> {
    let repo = PgFeedRecommendationRepository::new(f.pool.clone());
    let post = Uuid::new_v4();
    let author = Uuid::new_v4();
    let created = sqlx::query_scalar("insert into posts(id,author_id,content,visibility,created_at) values($1,$2,'authority fixture','PUBLIC',clock_timestamp()) returning created_at")
        .bind(post).bind(author).fetch_one(&f.pool).await?;
    repo.enqueue_post(Uuid::new_v4(), post, author, created, 1, 1000, Utc::now())
        .await?;
    let owned = repo
        .claim_jobs(&TimelineFanoutClaimRequestDto {
            lease_timeout: lease,
            limit: 1,
            max_page_attempts: max_attempts,
            retry_base: StdDuration::from_secs(1),
            excluded_job_ids: vec![],
        })
        .await?
        .remove(0);
    Ok((repo, owned))
}
async fn snapshot(f: &Fixture) -> Result<String> {
    // Every table touched by claim/page/publication work. Stable byte equality
    // proves lookup neither renews nor consumes, suspends or publishes a job.
    Ok(sqlx::query_scalar("select jsonb_build_object( \
        'jobs',(select jsonb_agg(to_jsonb(j) order by job_id) from feed_fanout_jobs j), \
        'posts',(select jsonb_agg(to_jsonb(p) order by id) from posts p), \
        'timeline',(select jsonb_agg(to_jsonb(t) order by viewer_id,post_id,source) from home_timeline_entries t), \
        'outbox',(select jsonb_agg(to_jsonb(e) order by event_id) from post_event_outbox e), \
        'deliveries',(select jsonb_agg(to_jsonb(d) order by event_id,target) from post_event_delivery_jobs d), \
        'attempts',(select jsonb_agg(to_jsonb(a) order by attempt_id,phase) from post_event_delivery_attempts a), \
        'publications',(select jsonb_agg(to_jsonb(e) order by event_id,target) from event_publications e))::text")
        .fetch_one(&f.pool).await?)
}
async fn denied(reader: &PgFanoutAuthorityRepository, claim: TimelineFanoutClaimDto) -> Result<()> {
    assert_eq!(
        reader
            .resolve_claimed_page(&FanoutPageLookupDto::new(claim, 10)?)
            .await
            .err(),
        Some(Error::Denied)
    );
    Ok(())
}
async fn wait_for_blocker(f: &Fixture, pid: i32) -> Result<()> {
    tokio::time::timeout(StdDuration::from_secs(5), async {
        loop {
            let blocked: bool = sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and $1=any(pg_blocking_pids(pid)))")
                .bind(pid).fetch_one(&f.pool).await?;
            if blocked { return Ok::<_,anyhow::Error>(()); }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }
    }).await??;
    Ok(())
}

#[tokio::test]
async fn current_final_attempt_is_read_only_and_limit_uses_explicit_owner_policy() -> Result<()> {
    let f = Fixture::new(29).await?;
    let (_, claim) = seed(&f, StdDuration::from_secs(60), 1).await?;
    assert_eq!(claim.page_attempt_count, claim.max_page_attempts);
    let before = snapshot(&f).await?;
    let result = reader(&f, 10)?
        .resolve_claimed_page(&FanoutPageLookupDto::new(claim.clone(), 10)?)
        .await?;
    let stored = result.page().claim();
    assert_eq!(stored.job.job_id, claim.job.job_id);
    assert_eq!(stored.job.event_id, claim.job.event_id);
    assert_eq!(stored.job.post_id, claim.job.post_id);
    assert_eq!(stored.job.author_id, claim.job.author_id);
    assert_eq!(stored.job.post_created_at, claim.job.post_created_at);
    assert_eq!(stored.job.cursor, claim.job.cursor);
    assert_eq!(stored.lease_token, claim.lease_token);
    assert_eq!(stored.job.attempt_count, claim.job.attempt_count);
    assert_eq!(stored.page_attempt_count, 1);
    assert_eq!(stored.max_page_attempts, 1);
    assert_eq!(result.page().page_limit(), 10);
    assert_eq!(result.expires_at(), claim.lease_expires_at);
    assert!(result.checked_at() < result.expires_at());
    assert_eq!(
        reader(&f, 10)?
            .resolve_claimed_page(&FanoutPageLookupDto::new(claim.clone(), 11)?)
            .await
            .err(),
        Some(Error::Denied)
    );
    for limit in [0, 101] {
        assert_eq!(
            FanoutPageLookupDto::new(claim.clone(), limit).err(),
            Some(Error::InvalidBinding)
        );
        assert_eq!(
            FanoutPagePolicyDto::new(limit).err(),
            Some(Error::InvalidBinding)
        );
    }
    assert_eq!(snapshot(&f).await?, before);
    sqlx::query("update posts set visibility='FRIENDS'")
        .execute(&f.pool)
        .await?;
    let before = snapshot(&f).await?;
    reader(&f, 10)?
        .resolve_claimed_page(&FanoutPageLookupDto::new(claim, 10)?)
        .await?;
    assert_eq!(snapshot(&f).await?, before);
    f.shutdown().await
}

#[tokio::test]
async fn each_forged_claim_binding_denies_without_persistent_changes() -> Result<()> {
    let f = Fixture::new(29).await?;
    let (_, claim) = seed(&f, StdDuration::from_secs(60), 8).await?;
    let before = snapshot(&f).await?;
    for case in 0..11 {
        let mut forged = claim.clone();
        match case {
            0 => forged.job.job_id = Uuid::new_v4(),
            1 => forged.job.event_id = Uuid::new_v4(),
            2 => forged.job.post_id = Uuid::new_v4(),
            3 => forged.job.author_id = Uuid::new_v4(),
            4 => forged.lease_token = Uuid::new_v4(),
            5 => forged.job.attempt_count += 1,
            6 => {
                forged.job.cursor = Some(format!("{}|{}", Utc::now().to_rfc3339(), Uuid::new_v4()))
            }
            7 => forged.lease_expires_at += Duration::seconds(1),
            8 => forged.job.post_created_at += Duration::seconds(1),
            9 => forged.page_attempt_count += 1,
            10 => forged.max_page_attempts += 1,
            _ => unreachable!(),
        }
        denied(&reader(&f, 100)?, forged).await?;
    }
    assert_eq!(snapshot(&f).await?, before);
    f.shutdown().await
}

#[tokio::test]
async fn malformed_bindings_and_legacy_provenance_cannot_become_owner_scope() -> Result<()> {
    let f = Fixture::new(29).await?;
    let (_, claim) = seed(&f, StdDuration::from_secs(60), 8).await?;
    for case in 0..12 {
        let mut bad = claim.clone();
        match case {
            0 => bad.job.job_id = Uuid::nil(),
            1 => bad.job.event_id = Uuid::nil(),
            2 => bad.job.post_id = Uuid::nil(),
            3 => bad.job.author_id = Uuid::nil(),
            4 => bad.lease_token = Uuid::nil(),
            5 => bad.job.attempt_count = 0,
            6 => bad.page_attempt_count = 0,
            7 => bad.page_attempt_count = 9,
            8 => bad.max_page_attempts = 101,
            9 => bad.job.cursor = Some(String::new()),
            10 => bad.job.cursor = Some("x".repeat(257)),
            11 => bad.job.cursor = Some(format!("{}|{}", Utc::now().to_rfc3339(), Uuid::nil())),
            _ => unreachable!(),
        }
        assert_eq!(
            FanoutPageLookupDto::new(bad, 10).err(),
            Some(Error::InvalidBinding)
        );
    }
    sqlx::query("update feed_fanout_jobs set page_budget_provenance='LEGACY_UNVERIFIED',page_attempt_count=null,max_page_attempts=null,retry_base_ms=null where job_id=$1")
        .bind(claim.job.job_id).execute(&f.pool).await?;
    let before = snapshot(&f).await?;
    denied(&reader(&f, 100)?, claim).await?;
    assert_eq!(snapshot(&f).await?, before);
    f.shutdown().await
}

#[tokio::test]
async fn current_source_and_terminal_markers_deny_without_changing_job_state() -> Result<()> {
    let f = Fixture::new(29).await?;
    let (_, claim) = seed(&f, StdDuration::from_secs(60), 8).await?;
    for change in [
        "update posts set deleted_at=clock_timestamp()",
        "update posts set moderation_hidden_at=clock_timestamp()",
        "update posts set visibility='PRIVATE'",
        "update posts set author_id=gen_random_uuid()",
        "update feed_fanout_jobs set status='SUSPENDED',lease_token=null,lease_expires_at=null",
        "update feed_fanout_jobs set completed_at=clock_timestamp()",
        "update feed_fanout_jobs set dead_lettered_at=clock_timestamp()",
    ] {
        sqlx::query(change).execute(&f.pool).await?;
        let before = snapshot(&f).await?;
        denied(&reader(&f, 100)?, claim.clone()).await?;
        assert_eq!(snapshot(&f).await?, before);
        sqlx::query("update posts set deleted_at=null,moderation_hidden_at=null,visibility='PUBLIC',author_id=$1")
            .bind(claim.job.author_id).execute(&f.pool).await?;
        sqlx::query("update feed_fanout_jobs set status='PROCESSING',completed_at=null,dead_lettered_at=null,lease_token=$1,lease_expires_at=$2")
            .bind(claim.lease_token).bind(claim.lease_expires_at).execute(&f.pool).await?;
    }
    sqlx::query("delete from posts").execute(&f.pool).await?;
    let before = snapshot(&f).await?;
    denied(&reader(&f, 100)?, claim).await?;
    assert_eq!(snapshot(&f).await?, before);
    f.shutdown().await
}

#[tokio::test]
async fn page_advance_revokes_old_scope_and_preserves_effect_fencing() -> Result<()> {
    let f = Fixture::new(29).await?;
    let (repo, old) = seed(&f, StdDuration::from_secs(60), 8).await?;
    let follower = Uuid::new_v4();
    let cursor = format!("{}|{}", Utc::now().to_rfc3339(), follower);
    let page = TimelineFanoutPageDto {
        follower_ids: vec![follower],
        next_cursor: Some(cursor.clone()),
        expires_at: Utc::now() + Duration::days(1),
    };
    reader(&f, 100)?
        .resolve_claimed_page(&FanoutPageLookupDto::new(old.clone(), 10)?)
        .await?;
    assert_eq!(
        repo.apply_page(&old, &page).await?,
        TimelineFanoutOutcomeDto::Applied
    );
    denied(&reader(&f, 100)?, old.clone()).await?;
    let next = repo
        .claim_jobs(&TimelineFanoutClaimRequestDto {
            lease_timeout: StdDuration::from_secs(60),
            limit: 1,
            max_page_attempts: 8,
            retry_base: StdDuration::from_secs(1),
            excluded_job_ids: vec![],
        })
        .await?
        .remove(0);
    assert_ne!(next.lease_token, old.lease_token);
    assert_eq!(next.job.cursor.as_deref(), Some(cursor.as_str()));
    let before = snapshot(&f).await?;
    let valid = reader(&f, 100)?
        .resolve_claimed_page(&FanoutPageLookupDto::new(next.clone(), 10)?)
        .await?;
    assert_eq!(valid.page().claim().job.cursor, next.job.cursor);
    let mut wrong_cursor = next.clone();
    wrong_cursor.job.cursor = None;
    denied(&reader(&f, 100)?, wrong_cursor).await?;
    let mut normalized = next.clone();
    normalized.job.cursor = Some(cursor.replace("+00:00", "Z"));
    assert_ne!(normalized.job.cursor, next.job.cursor);
    denied(&reader(&f, 100)?, normalized).await?;
    denied(&reader(&f, 100)?, old.clone()).await?;
    assert_eq!(snapshot(&f).await?, before);
    assert_eq!(
        repo.apply_page(&old, &page).await?,
        TimelineFanoutOutcomeDto::Lost
    );
    assert_eq!(snapshot(&f).await?, before);
    f.shutdown().await
}

#[tokio::test]
async fn expired_scope_and_replaced_lease_do_not_renew_or_consume_attempts() -> Result<()> {
    let f = Fixture::new(29).await?;
    let (_, claim) = seed(&f, StdDuration::from_secs(60), 8).await?;
    for change in [
        "update feed_fanout_jobs set lease_expires_at=clock_timestamp()-interval '1 second'",
        "update feed_fanout_jobs set lease_token=gen_random_uuid(),lease_expires_at=clock_timestamp()+interval '60 seconds'",
    ] {
        sqlx::query(change).execute(&f.pool).await?;
        let before = snapshot(&f).await?;
        denied(&reader(&f, 100)?, claim.clone()).await?;
        assert_eq!(snapshot(&f).await?, before);
    }
    f.shutdown().await
}

async fn expiry_wait(lock_kind: u8) -> Result<()> {
    let f = Fixture::new(29).await?;
    let (_, claim) = seed(&f, StdDuration::from_secs(3), 8).await?;
    let before = snapshot(&f).await?;
    let mut blocker = f.pool.begin().await?;
    let pid = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await?;
    match lock_kind {
        0 => {
            sqlx::query("select job_id from feed_fanout_jobs where job_id=$1 for update")
                .bind(claim.job.job_id)
                .fetch_one(&mut *blocker)
                .await?;
        }
        1 => {
            sqlx::query("lock table feed_fanout_jobs in access exclusive mode")
                .execute(&mut *blocker)
                .await?;
        }
        2 => {
            sqlx::query("select id from posts where id=$1 for update")
                .bind(claim.job.post_id)
                .fetch_one(&mut *blocker)
                .await?;
        }
        3 => {
            sqlx::query("select post_lock_subject_ids($1::uuid[])")
                .bind(vec![claim.job.author_id])
                .execute(&mut *blocker)
                .await?;
        }
        _ => unreachable!(),
    }
    let request = FanoutPageLookupDto::new(claim.clone(), 10)?;
    let executing = reader(&f, 100)?;
    let task = tokio::spawn(async move { executing.resolve_claimed_page(&request).await });
    wait_for_blocker(&f, pid).await?;
    // Let the unchanged row's real lease expire while the lookup is demonstrably
    // waiting; no expiry UPDATE can rescue an incorrect pre-wait clock check.
    sqlx::query(
        "select pg_sleep(greatest(0,extract(epoch from ($1::timestamptz-clock_timestamp())))+0.01)",
    )
    .bind(claim.lease_expires_at)
    .execute(&mut *blocker)
    .await?;
    blocker.commit().await?;
    assert_eq!(task.await?.err(), Some(Error::Denied));
    assert_eq!(snapshot(&f).await?, before);
    denied(&reader(&f, 100)?, claim).await?;
    assert_eq!(snapshot(&f).await?, before);
    f.shutdown().await
}
#[tokio::test]
async fn job_row_wait_rechecks_expiry_without_any_expiry_update() -> Result<()> {
    expiry_wait(0).await
}
#[tokio::test]
async fn job_table_wait_rechecks_expiry_without_any_expiry_update() -> Result<()> {
    expiry_wait(1).await
}

#[tokio::test]
async fn post_row_wait_rechecks_expiry_without_any_expiry_update() -> Result<()> {
    expiry_wait(2).await
}
#[tokio::test]
async fn subject_wait_rechecks_expiry_without_any_expiry_update() -> Result<()> {
    expiry_wait(3).await
}

#[tokio::test]
async fn source_wait_rechecks_visibility_and_storage_failure_is_unavailable() -> Result<()> {
    let f = Fixture::new(29).await?;
    let (_, claim) = seed(&f, StdDuration::from_secs(60), 8).await?;
    let mut blocker = f.pool.begin().await?;
    let pid = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await?;
    // A writer must take the subject fence before its Post row. Taking only
    // the row and later UPDATEing would invert the trigger's subject lock.
    sqlx::query("select post_lock_subject_ids($1::uuid[])")
        .bind(vec![claim.job.author_id])
        .execute(&mut *blocker)
        .await?;
    sqlx::query("select id from posts where id=$1 for update")
        .bind(claim.job.post_id)
        .fetch_one(&mut *blocker)
        .await?;
    let request = FanoutPageLookupDto::new(claim.clone(), 10)?;
    let executing = reader(&f, 100)?;
    let task = tokio::spawn(async move { executing.resolve_claimed_page(&request).await });
    wait_for_blocker(&f, pid).await?;
    sqlx::query("update posts set visibility='PRIVATE' where id=$1")
        .bind(claim.job.post_id)
        .execute(&mut *blocker)
        .await?;
    blocker.commit().await?;
    assert_eq!(task.await?.err(), Some(Error::Denied));
    let unavailable = reader(&f, 100)?;
    f.pool.close().await;
    assert_eq!(
        unavailable
            .resolve_claimed_page(&FanoutPageLookupDto::new(claim, 10)?)
            .await
            .err(),
        Some(Error::Unavailable)
    );
    f.shutdown().await
}

#[tokio::test]
async fn owner_change_during_subject_wait_cannot_select_a_different_subject() -> Result<()> {
    let f = Fixture::new(29).await?;
    let (_, claim) = seed(&f, StdDuration::from_secs(60), 8).await?;
    let mut blocker = f.pool.begin().await?;
    let pid = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await?;
    sqlx::query("select post_lock_subject_ids($1::uuid[])")
        .bind(vec![claim.job.author_id])
        .execute(&mut *blocker)
        .await?;
    let request = FanoutPageLookupDto::new(claim.clone(), 10)?;
    let executing = reader(&f, 100)?;
    let task = tokio::spawn(async move { executing.resolve_claimed_page(&request).await });
    wait_for_blocker(&f, pid).await?;
    sqlx::query("update posts set author_id=gen_random_uuid() where id=$1")
        .bind(claim.job.post_id)
        .execute(&mut *blocker)
        .await?;
    blocker.commit().await?;
    assert_eq!(task.await?.err(), Some(Error::Denied));
    let before = snapshot(&f).await?;
    denied(&reader(&f, 100)?, claim).await?;
    assert_eq!(snapshot(&f).await?, before);
    f.shutdown().await
}
