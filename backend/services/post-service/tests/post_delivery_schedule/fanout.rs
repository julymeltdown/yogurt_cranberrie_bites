use super::*;
use post_service::adapters::out::persistence::feed_recommendation_repository::PgFeedRecommendationRepository;
use post_service::application::dto::timeline_fanout_dto::TimelineFanoutFailureDto;
use post_service::application::dto::timeline_fanout_dto::{
    TimelineFanoutClaimDto, TimelineFanoutClaimRequestDto, TimelineFanoutOutcomeDto,
    TimelineFanoutPageDto,
};
use post_service::application::ports::TimelineFanoutRepositoryPort;
use std::time::Duration as StdDuration;

async fn seed(f: &Fixture) -> Result<(PgFeedRecommendationRepository, Uuid)> {
    let repo = PgFeedRecommendationRepository::new(f.pool.clone());
    let post = Uuid::new_v4();
    let author = Uuid::new_v4();
    sqlx::query("insert into posts(id,author_id,content,visibility,created_at) values($1,$2,'fanout fixture','PUBLIC',now())")
        .bind(post).bind(author).execute(&f.pool).await?;
    repo.enqueue_post(
        Uuid::new_v4(),
        post,
        author,
        Utc::now(),
        1,
        1000,
        Utc::now(),
    )
    .await?;
    Ok((repo, post))
}

#[tokio::test]
async fn late_page_cannot_recreate_entries_after_job_removal() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (repo, post) = seed(&f).await?;
    let job = repo
        .claim_jobs(&TimelineFanoutClaimRequestDto {
            lease_timeout: StdDuration::from_secs(60),
            limit: 1,
            max_page_attempts: 8,
            retry_base: StdDuration::from_secs(1),
            excluded_job_ids: vec![],
        })
        .await?
        .remove(0);
    // A follower page is now in flight while the deletion projection removes
    // durable job authority. The retained claim must not recreate its entries.
    repo.remove_post(post).await?;
    assert_eq!(
        repo.apply_page(&job, &page(vec![Uuid::new_v4()])).await?,
        TimelineFanoutOutcomeDto::Lost
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from home_timeline_entries where post_id=$1")
            .bind(post)
            .fetch_one(&f.pool)
            .await?,
        0
    );
    f.shutdown().await
}

fn page(follower_ids: Vec<Uuid>) -> TimelineFanoutPageDto {
    TimelineFanoutPageDto {
        follower_ids,
        next_cursor: None,
        expires_at: Utc::now() + Duration::days(1),
    }
}
async fn claim(repo: &PgFeedRecommendationRepository) -> Result<TimelineFanoutClaimDto> {
    Ok(repo
        .claim_jobs(&TimelineFanoutClaimRequestDto {
            lease_timeout: StdDuration::from_secs(60),
            limit: 1,
            max_page_attempts: 8,
            retry_base: StdDuration::from_secs(1),
            excluded_job_ids: vec![],
        })
        .await?
        .remove(0))
}
async fn expire(f: &Fixture, id: Uuid) -> Result<()> {
    sqlx::query("update feed_fanout_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where job_id=$1")
        .bind(id).execute(&f.pool).await?;
    let repo = PgFeedRecommendationRepository::new(f.pool.clone());
    let _ = repo
        .claim_jobs(&TimelineFanoutClaimRequestDto {
            lease_timeout: StdDuration::from_secs(60),
            limit: 1,
            max_page_attempts: 8,
            retry_base: StdDuration::from_secs(1),
            excluded_job_ids: vec![],
        })
        .await?;
    // The replacement probe controls due time after the separate expiry/backoff
    // transition. Dedicated budget probes verify the real recorded delay.
    sqlx::query("update feed_fanout_jobs set next_attempt_at=clock_timestamp()-interval '1 second' where job_id=$1").bind(id).execute(&f.pool).await?;
    Ok(())
}

async fn wait_source_recheck(f: &Fixture, post: Uuid) -> Result<()> {
    tokio::time::timeout(StdDuration::from_secs(5), async {
        loop {
            let due:bool=sqlx::query_scalar("select next_source_check_at<=clock_timestamp() from feed_fanout_jobs where post_id=$1")
                .bind(post).fetch_one(&f.pool).await?;
            if due { return Ok::<_,anyhow::Error>(()); }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }
    }).await??;
    Ok(())
}
#[tokio::test]
async fn replaced_claim_cannot_insert_advance_or_record_failure() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (repo, post) = seed(&f).await?;
    let old = claim(&repo).await?;
    expire(&f, old.job.job_id).await?;
    let new = claim(&repo).await?;
    assert_ne!(new.lease_token, old.lease_token);
    assert_eq!(
        repo.apply_page(&old, &page(vec![Uuid::new_v4()])).await?,
        TimelineFanoutOutcomeDto::Lost
    );
    assert_eq!(
        repo.record_job_failure(
            &old,
            &TimelineFanoutFailureDto {
                message: "stale failure".into(),
            }
        )
        .await?,
        TimelineFanoutOutcomeDto::Lost
    );
    assert_eq!(
        repo.apply_page(&new, &page(vec![Uuid::new_v4()])).await?,
        TimelineFanoutOutcomeDto::Applied
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from home_timeline_entries where post_id=$1 and source='MATERIALIZED_TIMELINE'").bind(post).fetch_one(&f.pool).await?,1);
    f.shutdown().await
}
#[tokio::test]
async fn failed_cursor_update_rolls_back_all_page_entries() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (repo, post) = seed(&f).await?;
    let owned = claim(&repo).await?;
    sqlx::raw_sql("create function reject_page_advance() returns trigger language plpgsql as $$ begin if new.status='COMPLETED' then raise exception 'injected page advance failure'; end if; return new; end; $$;create trigger reject_page_advance before update on feed_fanout_jobs for each row execute function reject_page_advance();").execute(&f.pool).await?;
    assert!(repo
        .apply_page(&owned, &page(vec![Uuid::new_v4()]))
        .await
        .is_err());
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from home_timeline_entries where post_id=$1 and source='MATERIALIZED_TIMELINE'").bind(post).fetch_one(&f.pool).await?,0);
    let row = sqlx::query(
        "select attempt_count,page_attempt_count from feed_fanout_jobs where post_id=$1",
    )
    .bind(post)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(row.get::<i32, _>("attempt_count"), 1);
    assert_eq!(row.get::<i32, _>("page_attempt_count"), 1);
    sqlx::query("drop trigger reject_page_advance on feed_fanout_jobs")
        .execute(&f.pool)
        .await?;
    assert_eq!(
        repo.apply_page(&owned, &page(vec![Uuid::new_v4()])).await?,
        TimelineFanoutOutcomeDto::Applied
    );
    f.shutdown().await
}
#[tokio::test]
async fn deleted_or_moderated_post_rejects_owned_page() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (repo, post) = seed(&f).await?;
    let owned = claim(&repo).await?;
    sqlx::query("update posts set moderation_hidden_at=now() where id=$1")
        .bind(post)
        .execute(&f.pool)
        .await?;
    assert_eq!(
        repo.apply_page(&owned, &page(vec![Uuid::new_v4()])).await?,
        TimelineFanoutOutcomeDto::Suspended
    );
    sqlx::query("update posts set moderation_hidden_at=null,deleted_at=now() where id=$1")
        .bind(post)
        .execute(&f.pool)
        .await?;
    assert_eq!(
        repo.apply_page(&owned, &page(vec![Uuid::new_v4()])).await?,
        TimelineFanoutOutcomeDto::Lost
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from home_timeline_entries where post_id=$1 and source='MATERIALIZED_TIMELINE'").bind(post).fetch_one(&f.pool).await?,0);
    f.shutdown().await
}
#[tokio::test]
async fn upgrade_preserves_legacy_claim_with_unknown_page_budget() -> Result<()> {
    let f = Fixture::new(18).await?;
    let (repo, post) = seed(&f).await?;
    sqlx::query("update feed_fanout_jobs set status='PROCESSING',locked_at=clock_timestamp()-interval '2 minutes',attempt_count=2 where post_id=$1").bind(post).execute(&f.pool).await?;
    f.upgrade().await?;
    assert!(repo
        .claim_jobs(&TimelineFanoutClaimRequestDto {
            lease_timeout: StdDuration::from_secs(60),
            limit: 1,
            max_page_attempts: 8,
            retry_base: StdDuration::from_secs(1),
            excluded_job_ids: vec![],
        })
        .await?
        .is_empty());
    let row=sqlx::query("select attempt_count,page_attempt_count,max_page_attempts,locked_at,lease_token from feed_fanout_jobs where post_id=$1")
        .bind(post).fetch_one(&f.pool).await?;
    assert_eq!(row.get::<i32, _>("attempt_count"), 2);
    assert!(row.get::<Option<i32>, _>("page_attempt_count").is_none());
    assert!(row.get::<Option<i32>, _>("max_page_attempts").is_none());
    assert!(row
        .get::<Option<chrono::DateTime<Utc>>, _>("locked_at")
        .is_some());
    assert!(row.get::<Option<Uuid>, _>("lease_token").is_none());
    f.shutdown().await
}

async fn wait_for_lock(f: &Fixture, query_pattern: &str) -> Result<()> {
    tokio::time::timeout(StdDuration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like $1)")
                .bind(query_pattern).fetch_one(&f.pool).await?;
            if waiting { return Ok::<_,anyhow::Error>(()); }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }
    }).await??;
    Ok(())
}

#[tokio::test]
async fn completion_waiting_on_job_lock_rechecks_database_expiry() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (repo, post) = seed(&f).await?;
    let owned = claim(&repo).await?;
    let mut blocker = f.pool.begin().await?;
    sqlx::query("select job_id from feed_fanout_jobs where job_id=$1 for update")
        .bind(owned.job.job_id)
        .fetch_one(&mut *blocker)
        .await?;
    let executing = repo.clone();
    let pending = owned.clone();
    let task = tokio::spawn(async move {
        executing
            .apply_page(&pending, &page(vec![Uuid::new_v4()]))
            .await
    });
    wait_for_lock(
        &f,
        "select job_id from feed_fanout_jobs where job_id=$1 and event_id=%",
    )
    .await?;
    sqlx::query("update feed_fanout_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where job_id=$1")
        .bind(owned.job.job_id).execute(&mut *blocker).await?;
    blocker.commit().await?;
    assert_eq!(task.await??, TimelineFanoutOutcomeDto::Lost);
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from home_timeline_entries where post_id=$1 and source='MATERIALIZED_TIMELINE'").bind(post).fetch_one(&f.pool).await?,0);
    f.shutdown().await
}

#[tokio::test]
async fn expiry_during_entry_insert_rolls_back_entries_and_cursor() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (repo, post) = seed(&f).await?;
    let owned = repo
        .claim_jobs(&TimelineFanoutClaimRequestDto {
            lease_timeout: StdDuration::from_millis(1000),
            limit: 1,
            max_page_attempts: 8,
            retry_base: StdDuration::from_secs(1),
            excluded_job_ids: vec![],
        })
        .await?
        .remove(0);
    sqlx::raw_sql("create function pause_page_insert() returns trigger language plpgsql as $$ begin perform pg_advisory_xact_lock(191919); return new; end; $$;create trigger pause_page_insert before insert on home_timeline_entries for each row execute function pause_page_insert();").execute(&f.pool).await?;
    let mut barrier = f.pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(191919)")
        .execute(&mut *barrier)
        .await?;
    let executing = repo.clone();
    let pending = owned.clone();
    let task = tokio::spawn(async move {
        executing
            .apply_page(&pending, &page(vec![Uuid::new_v4()]))
            .await
    });
    wait_for_lock(
        &f,
        "%insert into home_timeline_entries(viewer_id,post_id,author_id,source%",
    )
    .await?;
    tokio::time::timeout(StdDuration::from_secs(5), async {
        loop {
            let expired: bool = sqlx::query_scalar(
                "select lease_expires_at<=clock_timestamp() from feed_fanout_jobs where job_id=$1",
            )
            .bind(owned.job.job_id)
            .fetch_one(&f.pool)
            .await?;
            if expired {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }
    })
    .await??;
    barrier.commit().await?;
    assert_eq!(task.await??, TimelineFanoutOutcomeDto::Lost);
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from home_timeline_entries where post_id=$1 and source='MATERIALIZED_TIMELINE'").bind(post).fetch_one(&f.pool).await?,0);
    assert_eq!(
        sqlx::query_scalar::<_, String>("select status from feed_fanout_jobs where job_id=$1")
            .bind(owned.job.job_id)
            .fetch_one(&f.pool)
            .await?,
        "PROCESSING"
    );
    f.shutdown().await
}

#[derive(Default)]
struct DelayedFriend {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    calls: std::sync::atomic::AtomicUsize,
    failure: bool,
}
#[async_trait::async_trait]
impl post_service::application::ports::FriendClientPort for DelayedFriend {
    async fn fetch_feed_authors(
        &self,
        viewer_id: Uuid,
    ) -> Result<post_service::application::feed_authors::FeedAuthorsDto> {
        let _ = viewer_id;
        anyhow::bail!("unused owner-filtered feed fixture")
    }

    async fn fetch_following_ids(&self, _id: Uuid) -> Result<Vec<Uuid>> {
        anyhow::bail!("unused")
    }
    async fn fetch_follower_ids(&self, _id: Uuid) -> Result<Vec<Uuid>> {
        if self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            self.entered.notify_one();
            self.release.notified().await;
        }
        if self.failure {
            anyhow::bail!("controlled delayed Friend failure");
        }
        Ok(vec![Uuid::new_v4()])
    }
    async fn is_following(&self, _viewer: Uuid, _author: Uuid) -> Result<bool> {
        anyhow::bail!("unused")
    }
}

#[tokio::test]
async fn actual_fanout_use_case_delayed_page_crossing_removal_counts_no_commit() -> Result<()> {
    use post_service::application::use_cases::process_timeline_fanout::{
        ProcessTimelineFanoutUseCase, TimelineFanoutConfig,
    };
    use std::sync::Arc;
    let f = Fixture::new(22).await?;
    let (repo, post) = seed(&f).await?;
    let friend = Arc::new(DelayedFriend::default());
    let use_case = ProcessTimelineFanoutUseCase::new(
        Arc::new(repo.clone()),
        friend.clone(),
        TimelineFanoutConfig::default(),
    );
    let task = tokio::spawn(async move { use_case.execute(Utc::now()).await });
    tokio::time::timeout(StdDuration::from_secs(5), friend.entered.notified()).await?;
    repo.remove_post(post).await?;
    friend.release.notify_one();
    assert_eq!(task.await??, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from home_timeline_entries where post_id=$1")
            .bind(post)
            .fetch_one(&f.pool)
            .await?,
        0
    );
    f.shutdown().await
}

#[tokio::test]
async fn slow_page_does_not_prelease_later_job() -> Result<()> {
    use post_service::application::use_cases::process_timeline_fanout::{
        ProcessTimelineFanoutUseCase, TimelineFanoutConfig,
    };
    use std::sync::Arc;
    let f = Fixture::new(22).await?;
    let (repo, first) = seed(&f).await?;
    let (_, second) = seed(&f).await?;
    let friend = Arc::new(DelayedFriend::default());
    let use_case = ProcessTimelineFanoutUseCase::new(
        Arc::new(repo.clone()),
        friend.clone(),
        TimelineFanoutConfig::default(),
    );
    let task = tokio::spawn(async move { use_case.execute(Utc::now()).await });
    tokio::time::timeout(StdDuration::from_secs(5), friend.entered.notified()).await?;
    let later: (String, i32, Option<Uuid>) = sqlx::query_as(
        "select status,attempt_count,lease_token from feed_fanout_jobs where post_id=$1",
    )
    .bind(second)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(later, ("PENDING".into(), 0, None));
    repo.remove_post(first).await?;
    friend.release.notify_one();
    assert_eq!(task.await??, 1);
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from home_timeline_entries where post_id=$1 and source='MATERIALIZED_TIMELINE'").bind(second).fetch_one(&f.pool).await?,1);
    f.shutdown().await
}

#[tokio::test]
async fn hidden_private_and_deleted_jobs_do_not_starve_healthy_work() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (repo, hidden) = seed(&f).await?;
    let (_, private) = seed(&f).await?;
    let (_, deleted) = seed(&f).await?;
    let (_, healthy) = seed(&f).await?;
    sqlx::query("update posts set moderation_hidden_at=now() where id=$1")
        .bind(hidden)
        .execute(&f.pool)
        .await?;
    sqlx::query("update posts set visibility='PRIVATE' where id=$1")
        .bind(private)
        .execute(&f.pool)
        .await?;
    sqlx::query("update posts set deleted_at=now() where id=$1")
        .bind(deleted)
        .execute(&f.pool)
        .await?;
    let owned = claim(&repo).await?;
    assert_eq!(owned.job.post_id, healthy);
    assert_eq!(
        repo.apply_page(&owned, &page(vec![])).await?,
        TimelineFanoutOutcomeDto::Applied
    );
    assert!(repo
        .claim_jobs(&TimelineFanoutClaimRequestDto {
            lease_timeout: StdDuration::from_secs(60),
            limit: 100,
            max_page_attempts: 8,
            retry_base: StdDuration::from_secs(1),
            excluded_job_ids: vec![],
        })
        .await?
        .is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_fanout_jobs where attempt_count=0")
            .fetch_one(&f.pool)
            .await?,
        3
    );
    // Reversible moderation may resume its original job; suppression did not
    // invent completed history or delete the durable work.
    sqlx::query("update posts set moderation_hidden_at=null where id=$1")
        .bind(hidden)
        .execute(&f.pool)
        .await?;
    wait_source_recheck(&f, hidden).await?;
    assert_eq!(claim(&repo).await?.job.post_id, hidden);
    f.shutdown().await
}

#[tokio::test]
async fn delayed_failure_starts_backoff_at_database_recording_time() -> Result<()> {
    use post_service::application::use_cases::process_timeline_fanout::{
        ProcessTimelineFanoutUseCase, TimelineFanoutConfig,
    };
    use std::sync::Arc;
    let f = Fixture::new(22).await?;
    let (repo, _post) = seed(&f).await?;
    let friend = Arc::new(DelayedFriend {
        failure: true,
        ..Default::default()
    });
    let use_case = ProcessTimelineFanoutUseCase::new(
        Arc::new(repo.clone()),
        friend.clone(),
        TimelineFanoutConfig {
            claim_limit: 1,
            base_retry: StdDuration::from_secs(1),
            ..Default::default()
        },
    );
    let task = tokio::spawn(async move { use_case.execute(Utc::now()).await });
    tokio::time::timeout(StdDuration::from_secs(5), friend.entered.notified()).await?;
    // The cycle has already spent more than the retry duration in a pending
    // network read. Failure recording must still start a fresh delay.
    sqlx::query("select pg_sleep(1.2)").execute(&f.pool).await?;
    friend.release.notify_one();
    assert_eq!(task.await??, 0);
    assert!(repo
        .claim_jobs(&TimelineFanoutClaimRequestDto {
            lease_timeout: StdDuration::from_secs(60),
            limit: 1,
            max_page_attempts: 8,
            retry_base: StdDuration::from_secs(1),
            excluded_job_ids: vec![],
        })
        .await?
        .is_empty());
    assert!(sqlx::query_scalar::<_,bool>("select next_attempt_at>clock_timestamp() and status='PENDING' and lease_token is null from feed_fanout_jobs").fetch_one(&f.pool).await?);
    f.shutdown().await
}

#[tokio::test]
async fn suspended_claim_resumes_after_moderation_restoration() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (repo, post) = seed(&f).await?;
    let old = claim(&repo).await?;
    sqlx::query("update posts set moderation_hidden_at=now() where id=$1")
        .bind(post)
        .execute(&f.pool)
        .await?;
    assert_eq!(
        repo.apply_page(&old, &page(vec![Uuid::new_v4()])).await?,
        TimelineFanoutOutcomeDto::Suspended
    );
    assert!(repo
        .claim_jobs(&TimelineFanoutClaimRequestDto {
            lease_timeout: StdDuration::from_secs(60),
            limit: 1,
            max_page_attempts: 8,
            retry_base: StdDuration::from_secs(1),
            excluded_job_ids: vec![],
        })
        .await?
        .is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, String>("select status from feed_fanout_jobs where post_id=$1")
            .bind(post)
            .fetch_one(&f.pool)
            .await?,
        "SUSPENDED"
    );
    sqlx::query("update posts set moderation_hidden_at=null where id=$1")
        .bind(post)
        .execute(&f.pool)
        .await?;
    wait_source_recheck(&f, post).await?;
    let new = claim(&repo).await?;
    assert_ne!(new.lease_token, old.lease_token);
    assert_eq!(
        repo.apply_page(&old, &page(vec![])).await?,
        TimelineFanoutOutcomeDto::Lost
    );
    assert_eq!(
        repo.apply_page(&new, &page(vec![Uuid::new_v4()])).await?,
        TimelineFanoutOutcomeDto::Applied
    );
    f.shutdown().await
}

#[tokio::test]
async fn author_cleanup_waits_on_subject_before_jobs_and_removes_committed_page() -> Result<()> {
    use post_service::application::ports::FeedFeatureRepositoryPort;
    let f = Fixture::new(22).await?;
    let (repo, post) = seed(&f).await?;
    let owned = claim(&repo).await?;
    let recipient = Uuid::new_v4();
    sqlx::query("insert into home_timeline_entries(viewer_id,post_id,author_id,source,source_score,inserted_at,expires_at) values($1,$2,$3,'MATERIALIZED_TIMELINE',1,now(),now()+interval '1 day')")
        .bind(recipient).bind(post).bind(owned.job.author_id).execute(&f.pool).await?;
    sqlx::raw_sql("create function pause_cleanup_page() returns trigger language plpgsql as $$ begin perform pg_advisory_xact_lock(192020); return new; end; $$;create trigger pause_cleanup_page before insert on home_timeline_entries for each row execute function pause_cleanup_page();").execute(&f.pool).await?;
    let mut barrier = f.pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(192020)")
        .execute(&mut *barrier)
        .await?;
    let writer = repo.clone();
    let writer_claim = owned.clone();
    let page_task = tokio::spawn(async move {
        writer
            .apply_page(&writer_claim, &page(vec![recipient]))
            .await
    });
    wait_for_lock(
        &f,
        "%insert into home_timeline_entries(viewer_id,post_id,author_id,source%",
    )
    .await?;
    let cleaner = repo.clone();
    let author = owned.job.author_id;
    let cleanup =
        tokio::spawn(
            async move { FeedFeatureRepositoryPort::cleanup_user(&cleaner, author).await },
        );
    wait_for_lock(&f, "select post_lock_subject_ids($1::uuid[])%").await?;
    barrier.commit().await?;
    assert_eq!(
        tokio::time::timeout(StdDuration::from_secs(5), page_task).await???,
        TimelineFanoutOutcomeDto::Applied
    );
    tokio::time::timeout(StdDuration::from_secs(5), cleanup).await???;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from home_timeline_entries where post_id=$1")
            .bind(post)
            .fetch_one(&f.pool)
            .await?,
        0
    );
    f.shutdown().await
}

#[tokio::test]
async fn successful_pages_do_not_exhaust_later_page_failure_budget() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (repo, post) = seed(&f).await?;
    for i in 0..9 {
        let owned = claim(&repo).await?;
        let follower = Uuid::new_v4();
        let mut next = page(vec![follower]);
        next.next_cursor = Some(format!(
            "{}|{}",
            (Utc::now() - Duration::days(i + 1)).to_rfc3339(),
            follower
        ));
        assert_eq!(
            repo.apply_page(&owned, &next).await?,
            TimelineFanoutOutcomeDto::Applied
        );
    }
    let owned = claim(&repo).await?;
    repo.record_job_failure(
        &owned,
        &TimelineFanoutFailureDto {
            message: "first failure on tenth page".into(),
        },
    )
    .await?;
    let status: String = sqlx::query_scalar("select status from feed_fanout_jobs where post_id=$1")
        .bind(post)
        .fetch_one(&f.pool)
        .await?;
    assert_eq!(
        status, "PENDING",
        "successful earlier pages must not consume this page's retry budget"
    );
    f.shutdown().await
}

fn budget_request(max: i32, base_ms: u64) -> TimelineFanoutClaimRequestDto {
    TimelineFanoutClaimRequestDto {
        lease_timeout: StdDuration::from_secs(60),
        limit: 1,
        max_page_attempts: max,
        retry_base: StdDuration::from_millis(base_ms),
        excluded_job_ids: vec![],
    }
}

#[tokio::test]
async fn live_final_page_lease_survives_then_inactive_expiry_exhausts() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (repo, post) = seed(&f).await?;
    let owned = repo.claim_jobs(&budget_request(1, 100)).await?.remove(0);
    sqlx::query("update posts set moderation_hidden_at=clock_timestamp() where id=$1")
        .bind(post)
        .execute(&f.pool)
        .await?;
    assert!(repo
        .claim_jobs(&budget_request(100, 3600000))
        .await?
        .is_empty());
    let state: String = sqlx::query_scalar("select status from feed_fanout_jobs where post_id=$1")
        .bind(post)
        .fetch_one(&f.pool)
        .await?;
    assert_eq!(state, "PROCESSING");
    sqlx::query("update feed_fanout_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where post_id=$1")
        .bind(post).execute(&f.pool).await?;
    assert!(repo
        .claim_jobs(&budget_request(100, 3600000))
        .await?
        .is_empty());
    let row = sqlx::query("select status,max_page_attempts,page_attempt_count,lease_token,last_error from feed_fanout_jobs where post_id=$1")
        .bind(post).fetch_one(&f.pool).await?;
    assert_eq!(row.get::<String, _>("status"), "DEAD_LETTERED");
    assert_eq!(row.get::<i32, _>("max_page_attempts"), 1);
    assert_eq!(row.get::<i32, _>("page_attempt_count"), 1);
    assert!(row.get::<Option<Uuid>, _>("lease_token").is_none());
    assert!(row
        .get::<String, _>("last_error")
        .contains("budget exhausted"));
    assert_eq!(
        repo.apply_page(&owned, &page(vec![Uuid::new_v4()])).await?,
        TimelineFanoutOutcomeDto::Lost
    );
    f.shutdown().await
}

#[tokio::test]
async fn expired_page_backoff_is_frozen_and_released_once_before_final_crash() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (repo, post) = seed(&f).await?;
    let first = repo.claim_jobs(&budget_request(2, 1000)).await?.remove(0);
    sqlx::query("update feed_fanout_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where post_id=$1")
        .bind(post).execute(&f.pool).await?;
    assert!(repo
        .claim_jobs(&budget_request(100, 3600000))
        .await?
        .is_empty());
    let row = sqlx::query("select next_attempt_at,clock_timestamp() as observed,page_attempt_count,attempt_count,retry_base_ms,status from feed_fanout_jobs where post_id=$1")
        .bind(post).fetch_one(&f.pool).await?;
    let due = row.get::<chrono::DateTime<Utc>, _>("next_attempt_at");
    assert!(due > row.get::<chrono::DateTime<Utc>, _>("observed"));
    assert_eq!(row.get::<i64, _>("retry_base_ms"), 1000);
    assert_eq!(row.get::<i32, _>("page_attempt_count"), 1);
    assert_eq!(row.get::<i32, _>("attempt_count"), 1);
    assert_eq!(row.get::<String, _>("status"), "PENDING");
    assert!(repo
        .claim_jobs(&budget_request(100, 3600000))
        .await?
        .is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, chrono::DateTime<Utc>>(
            "select next_attempt_at from feed_fanout_jobs where post_id=$1"
        )
        .bind(post)
        .fetch_one(&f.pool)
        .await?,
        due
    );
    assert_eq!(
        repo.apply_page(&first, &page(vec![])).await?,
        TimelineFanoutOutcomeDto::Lost
    );
    sqlx::query("update feed_fanout_jobs set next_attempt_at=clock_timestamp()-interval '1 second' where post_id=$1").bind(post).execute(&f.pool).await?;
    let final_claim = repo
        .claim_jobs(&budget_request(100, 3600000))
        .await?
        .remove(0);
    assert_eq!(final_claim.page_attempt_count, 2);
    assert_eq!(final_claim.max_page_attempts, 2);
    sqlx::query("update feed_fanout_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where post_id=$1").bind(post).execute(&f.pool).await?;
    assert!(repo.claim_jobs(&budget_request(100, 100)).await?.is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, String>("select status from feed_fanout_jobs where post_id=$1")
            .bind(post)
            .fetch_one(&f.pool)
            .await?,
        "DEAD_LETTERED"
    );
    f.shutdown().await
}

#[tokio::test]
async fn suspension_restoration_cannot_replenish_final_page_budget() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (repo, post) = seed(&f).await?;
    let owned = repo.claim_jobs(&budget_request(1, 100)).await?.remove(0);
    sqlx::query("update posts set moderation_hidden_at=clock_timestamp() where id=$1")
        .bind(post)
        .execute(&f.pool)
        .await?;
    assert_eq!(
        repo.apply_page(&owned, &page(vec![])).await?,
        TimelineFanoutOutcomeDto::Suspended
    );
    sqlx::query("update posts set moderation_hidden_at=null where id=$1")
        .bind(post)
        .execute(&f.pool)
        .await?;
    assert!(repo.claim_jobs(&budget_request(100, 100)).await?.is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, String>("select status from feed_fanout_jobs where post_id=$1")
            .bind(post)
            .fetch_one(&f.pool)
            .await?,
        "DEAD_LETTERED"
    );
    f.shutdown().await
}

#[tokio::test]
async fn repeated_equivalent_and_backwards_cursors_cannot_commit_or_reset_budget() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (repo, post) = seed(&f).await?;
    let id = Uuid::new_v4();
    let boundary = Utc::now() - Duration::days(1);
    let mut next = page(vec![id]);
    next.next_cursor = Some(format!("{}|{}", boundary.to_rfc3339(), id));
    let first = repo.claim_jobs(&budget_request(3, 100)).await?.remove(0);
    repo.apply_page(&first, &next).await?;
    let second = repo
        .claim_jobs(&budget_request(100, 3600000))
        .await?
        .remove(0);
    let mut invalid = next;
    assert!(repo.apply_page(&second, &invalid).await.is_err());
    for time in [boundary, boundary + Duration::days(1)] {
        invalid.next_cursor = Some(format!(
            "{}|{}",
            time.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true),
            id
        ));
        assert!(repo.apply_page(&second, &invalid).await.is_err());
    }
    invalid.next_cursor = Some("malformed".into());
    assert!(repo.apply_page(&second, &invalid).await.is_err());
    invalid.next_cursor = Some(String::new());
    assert!(repo.apply_page(&second, &invalid).await.is_err());
    let row=sqlx::query("select page_attempt_count,attempt_count,cursor,max_page_attempts from feed_fanout_jobs where post_id=$1").bind(post).fetch_one(&f.pool).await?;
    assert_eq!(row.get::<i32, _>("page_attempt_count"), 1);
    assert_eq!(row.get::<i32, _>("attempt_count"), 2);
    assert_eq!(row.get::<i32, _>("max_page_attempts"), 3);
    assert_eq!(
        row.get::<String, _>("cursor"),
        format!("{}|{}", boundary.to_rfc3339(), id)
    );
    invalid.next_cursor = None;
    invalid.follower_ids.clear();
    assert_eq!(
        repo.apply_page(&second, &invalid).await?,
        TimelineFanoutOutcomeDto::Applied
    );
    f.shutdown().await
}

#[tokio::test]
async fn page_failure_backoff_saturates_at_one_hour_without_overflow() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (repo, post) = seed(&f).await?;
    let owned = repo
        .claim_jobs(&budget_request(100, 3600000))
        .await?
        .remove(0);
    sqlx::query("update feed_fanout_jobs set page_attempt_count=99 where post_id=$1")
        .bind(post)
        .execute(&f.pool)
        .await?;
    repo.record_job_failure(
        &owned,
        &TimelineFanoutFailureDto {
            message: "confirmed RPC failure".into(),
        },
    )
    .await?;
    let delay:f64=sqlx::query_scalar("select extract(epoch from next_attempt_at-clock_timestamp())::float8 from feed_fanout_jobs where post_id=$1").bind(post).fetch_one(&f.pool).await?;
    assert!(delay > 3590.0 && delay <= 3600.0, "{delay}");
    assert_eq!(
        sqlx::query_scalar::<_, String>("select status from feed_fanout_jobs where post_id=$1")
            .bind(post)
            .fetch_one(&f.pool)
            .await?,
        "PENDING"
    );
    f.shutdown().await
}

#[tokio::test]
async fn bounded_inactive_windows_rotate_then_restore_without_consuming_attempts() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (repo, healthy) = seed(&f).await?;
    let author = Uuid::new_v4();
    sqlx::query("insert into posts(id,author_id,content,visibility,created_at) select gen_random_uuid(),$1,'inactive-window','PRIVATE',clock_timestamp()-interval '1 day' from generate_series(1,250)")
        .bind(author).execute(&f.pool).await?;
    sqlx::query("insert into feed_fanout_jobs(job_id,event_id,post_id,author_id,post_created_at,next_attempt_at,created_at,updated_at) select gen_random_uuid(),gen_random_uuid(),id,author_id,created_at,clock_timestamp()-interval '1 day',created_at,clock_timestamp() from posts where content='inactive-window'")
        .execute(&f.pool).await?;
    assert!(repo.claim_jobs(&budget_request(8, 100)).await?.is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from feed_fanout_jobs where status='SUSPENDED'"
        )
        .fetch_one(&f.pool)
        .await?,
        100
    );
    assert!(repo.claim_jobs(&budget_request(8, 100)).await?.is_empty());
    let owned = repo.claim_jobs(&budget_request(8, 100)).await?.remove(0);
    assert_eq!(owned.job.post_id, healthy);
    repo.apply_page(&owned, &page(vec![])).await?;
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from feed_fanout_jobs where status='SUSPENDED' and attempt_count=0 and page_attempt_count=0 and max_page_attempts is null").fetch_one(&f.pool).await?,250);
    let restored: Uuid =
        sqlx::query_scalar("select post_id from feed_fanout_jobs where status='SUSPENDED' limit 1")
            .fetch_one(&f.pool)
            .await?;
    sqlx::query("update posts set visibility='PUBLIC' where id=$1")
        .bind(restored)
        .execute(&f.pool)
        .await?;
    sqlx::query("update feed_fanout_jobs set next_source_check_at=clock_timestamp()-interval '1 day' where post_id=$1").bind(restored).execute(&f.pool).await?;
    let owned = repo.claim_jobs(&budget_request(8, 100)).await?.remove(0);
    assert_eq!(owned.job.post_id, restored);
    assert_eq!(owned.job.attempt_count, 1);
    f.shutdown().await
}

#[tokio::test]
async fn exhausted_maintenance_is_bounded_skips_locks_and_keeps_healthy_claims_available(
) -> Result<()> {
    let f = Fixture::new(22).await?;
    let (repo, healthy) = seed(&f).await?;
    let author = Uuid::new_v4();
    sqlx::query("insert into posts(id,author_id,content,visibility,created_at) select gen_random_uuid(),$1,'exhausted-window','PRIVATE',clock_timestamp()-interval '1 day' from generate_series(1,205)")
        .bind(author).execute(&f.pool).await?;
    sqlx::query("insert into feed_fanout_jobs(job_id,event_id,post_id,author_id,post_created_at,next_attempt_at,created_at,updated_at,attempt_count,page_attempt_count,max_page_attempts,retry_base_ms,page_budget_provenance) select gen_random_uuid(),gen_random_uuid(),id,author_id,created_at,clock_timestamp()-interval '1 day',created_at,clock_timestamp(),1,1,1,100,'CURRENT_PAGE' from posts where content='exhausted-window'")
        .execute(&f.pool).await?;
    let mut blocker = f.pool.begin().await?;
    let held:Uuid=sqlx::query_scalar("select job_id from feed_fanout_jobs where max_page_attempts=1 order by job_id limit 1 for update").fetch_one(&mut *blocker).await?;
    let owned = tokio::time::timeout(
        StdDuration::from_secs(5),
        repo.claim_jobs(&budget_request(8, 100)),
    )
    .await??
    .remove(0);
    assert_eq!(owned.job.post_id, healthy);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from feed_fanout_jobs where status='DEAD_LETTERED'"
        )
        .fetch_one(&f.pool)
        .await?,
        100
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("select status from feed_fanout_jobs where job_id=$1")
            .bind(held)
            .fetch_one(&f.pool)
            .await?,
        "PENDING"
    );
    assert!(repo.claim_jobs(&budget_request(8, 100)).await?.is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from feed_fanout_jobs where status='DEAD_LETTERED'"
        )
        .fetch_one(&f.pool)
        .await?,
        200
    );
    blocker.commit().await?;
    assert!(repo.claim_jobs(&budget_request(8, 100)).await?.is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from feed_fanout_jobs where status='DEAD_LETTERED'"
        )
        .fetch_one(&f.pool)
        .await?,
        205
    );
    f.shutdown().await
}

struct ProgressFriend;
#[async_trait::async_trait]
impl post_service::application::ports::FriendClientPort for ProgressFriend {
    async fn fetch_feed_authors(
        &self,
        viewer_id: Uuid,
    ) -> Result<post_service::application::feed_authors::FeedAuthorsDto> {
        let _ = viewer_id;
        anyhow::bail!("unused owner-filtered feed fixture")
    }

    async fn fetch_following_ids(&self, _id: Uuid) -> Result<Vec<Uuid>> {
        anyhow::bail!("unused")
    }
    async fn fetch_follower_ids(&self, _id: Uuid) -> Result<Vec<Uuid>> {
        anyhow::bail!("unused")
    }
    async fn is_following(&self, _viewer: Uuid, _author: Uuid) -> Result<bool> {
        anyhow::bail!("unused")
    }
    async fn fetch_follower_page(
        &self,
        _id: Uuid,
        _cursor: Option<&str>,
        _limit: i32,
    ) -> Result<post_service::domain::recommendation::FollowerPage> {
        let follower = Uuid::new_v4();
        Ok(post_service::domain::recommendation::FollowerPage {
            follower_ids: vec![follower],
            next_cursor: Some(format!(
                "{}|{}",
                (Utc::now() - Duration::days(1)).to_rfc3339(),
                follower
            )),
            total_count: 1000,
        })
    }
}

#[tokio::test]
async fn production_use_case_claims_each_job_at_most_once_per_cycle() -> Result<()> {
    use post_service::application::use_cases::process_timeline_fanout::{
        ProcessTimelineFanoutUseCase, TimelineFanoutConfig,
    };
    use std::sync::Arc;
    let f = Fixture::new(22).await?;
    let (repo, _) = seed(&f).await?;
    let _ = seed(&f).await?;
    let use_case = ProcessTimelineFanoutUseCase::new(
        Arc::new(repo),
        Arc::new(ProgressFriend),
        TimelineFanoutConfig {
            claim_limit: 2,
            ..Default::default()
        },
    );
    assert_eq!(
        use_case.execute(Utc::now() - Duration::days(30)).await?,
        2,
        "stale cycle time must not gate database-due claims"
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from feed_fanout_jobs where attempt_count=1 and page_attempt_count=0 and cursor is not null and status='PENDING'").fetch_one(&f.pool).await?,2);
    f.shutdown().await
}

#[tokio::test]
async fn upgrade19_to20_preserves_attempted_legacy_live_and_terminal_state() -> Result<()> {
    let f = Fixture::new(19).await?;
    let (repo, live) = seed(&f).await?;
    let (_, pending) = seed(&f).await?;
    let (_, completed) = seed(&f).await?;
    let (_, dead) = seed(&f).await?;
    let (_, unattempted) = seed(&f).await?;
    let token = Uuid::new_v4();
    let cursor = format!(
        "{}|{}",
        (Utc::now() - Duration::days(1)).to_rfc3339(),
        Uuid::new_v4()
    );
    sqlx::query("update feed_fanout_jobs set status='PROCESSING',attempt_count=1,cursor=$2,locked_at=clock_timestamp(),lease_token=$3,lease_expires_at=clock_timestamp()+interval '1 hour' where post_id=$1")
        .bind(live).bind(&cursor).bind(token).execute(&f.pool).await?;
    sqlx::query("update feed_fanout_jobs set attempt_count=42,cursor=$2 where post_id=$1")
        .bind(pending)
        .bind(&cursor)
        .execute(&f.pool)
        .await?;
    sqlx::query("update feed_fanout_jobs set status='COMPLETED',attempt_count=21,completed_at=clock_timestamp(),cursor=$2 where post_id=$1").bind(completed).bind(&cursor).execute(&f.pool).await?;
    sqlx::query("update feed_fanout_jobs set status='DEAD_LETTERED',attempt_count=7,dead_lettered_at=clock_timestamp(),last_error='retained legacy reason' where post_id=$1").bind(dead).execute(&f.pool).await?;
    let before=sqlx::query("select post_id,cursor,status,attempt_count,locked_at,lease_token,lease_expires_at,completed_at,dead_lettered_at,last_error from feed_fanout_jobs where post_id<>$1 order by post_id").bind(unattempted).fetch_all(&f.pool).await?;
    f.upgrade().await?;
    let after=sqlx::query("select post_id,cursor,status,attempt_count,locked_at,lease_token,lease_expires_at,completed_at,dead_lettered_at,last_error,page_attempt_count,page_budget_provenance from feed_fanout_jobs where post_id<>$1 order by post_id").bind(unattempted).fetch_all(&f.pool).await?;
    for (old, new) in before.iter().zip(after.iter()) {
        assert_eq!(old.get::<Uuid, _>("post_id"), new.get::<Uuid, _>("post_id"));
        for col in ["cursor", "last_error"] {
            assert_eq!(
                old.get::<Option<String>, _>(col),
                new.get::<Option<String>, _>(col)
            );
        }
        assert_eq!(
            old.get::<String, _>("status"),
            new.get::<String, _>("status")
        );
        assert_eq!(
            old.get::<i32, _>("attempt_count"),
            new.get::<i32, _>("attempt_count")
        );
        assert_eq!(
            old.get::<Option<Uuid>, _>("lease_token"),
            new.get::<Option<Uuid>, _>("lease_token")
        );
        for col in [
            "locked_at",
            "lease_expires_at",
            "completed_at",
            "dead_lettered_at",
        ] {
            assert_eq!(
                old.get::<Option<chrono::DateTime<Utc>>, _>(col),
                new.get::<Option<chrono::DateTime<Utc>>, _>(col)
            );
        }
        assert!(new.get::<Option<i32>, _>("page_attempt_count").is_none());
        assert_eq!(
            new.get::<String, _>("page_budget_provenance"),
            "LEGACY_UNVERIFIED"
        );
    }
    let claims = repo.claim_jobs(&budget_request(8, 100)).await?;
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].job.post_id, unattempted);
    assert_eq!(claims[0].page_attempt_count, 1);
    f.shutdown().await
}

#[tokio::test]
async fn unclaimed_unknown_page_count_is_rejected_by_native_constraint() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (_, post) = seed(&f).await?;
    let error = sqlx::query("update feed_fanout_jobs set page_attempt_count=null where post_id=$1")
        .bind(post)
        .execute(&f.pool)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().and_then(|e| e.constraint()),
        Some("feed_fanout_page_provenance_check")
    );
    assert_eq!(
        sqlx::query_scalar::<_, Option<i32>>(
            "select page_attempt_count from feed_fanout_jobs where post_id=$1"
        )
        .bind(post)
        .fetch_one(&f.pool)
        .await?,
        Some(0)
    );
    f.shutdown().await
}

fn scan_plan_stats(value: &serde_json::Value, stats: &mut (bool, i64, i64)) {
    if let Some(node) = value.as_object() {
        let kind = node.get("Node Type").and_then(|v| v.as_str()).unwrap_or("");
        if kind == "Seq Scan"
            && node.get("Relation Name").and_then(|v| v.as_str()) == Some("feed_fanout_jobs")
        {
            stats.0 = true;
        }
        stats.1 += node
            .get("Rows Removed by Filter")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if kind.contains("Scan") {
            stats.2 += node
                .get("Actual Rows")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
        }
        for child in node.values() {
            scan_plan_stats(child, stats);
        }
    } else if let Some(values) = value.as_array() {
        for child in values {
            scan_plan_stats(child, stats);
        }
    }
}

#[tokio::test]
async fn production_statement_plans_skip_large_future_and_exhausted_backlogs() -> Result<()> {
    use post_service::adapters::out::persistence::fanout_budget_sql;
    let f = Fixture::new(22).await?;
    let (_, post) = seed(&f).await?;
    sqlx::query("delete from feed_fanout_jobs where post_id=$1")
        .bind(post)
        .execute(&f.pool)
        .await?;
    sqlx::query("insert into feed_fanout_jobs(job_id,event_id,post_id,author_id,post_created_at,status,attempt_count,page_attempt_count,max_page_attempts,retry_base_ms,page_budget_provenance,next_attempt_at,locked_at,lease_token,lease_expires_at) select gen_random_uuid(),gen_random_uuid(),id,author_id,created_at,'PROCESSING',1,1,1,100,'CURRENT_PAGE',clock_timestamp(),clock_timestamp(),gen_random_uuid(),clock_timestamp()+interval '1 hour' from posts cross join generate_series(1,30000) where id=$1")
        .bind(post).execute(&f.pool).await?;
    // Exhausted released PENDING and SUSPENDED work belongs to maintenance,
    // never the eligibility/source-recheck indexes. Keep their future ownership
    // representative fixture separate from the claim-plan fixture below.
    sqlx::query("analyze feed_fanout_jobs")
        .execute(&f.pool)
        .await?;
    for (name, sql) in [
        ("exhausted", fanout_budget_sql::EXHAUSTED_MAINTENANCE_SQL),
        ("expired", fanout_budget_sql::EXPIRED_MAINTENANCE_SQL),
    ] {
        let plan: serde_json::Value =
            sqlx::query_scalar(&format!("explain (analyze,buffers,format json) {sql}"))
                .fetch_one(&f.pool)
                .await?;
        println!("FANOUT_PLAN {name} {plan}");
        let mut stats = (false, 0, 0);
        scan_plan_stats(&plan, &mut stats);
        assert!(
            !stats.0 && stats.1 < 1000 && stats.2 < 1000,
            "{name} scanned future backlog: {stats:?}"
        );
    }
    // After a batch of final-attempt leases expires, exhaustion maintenance
    // consumes only 100 rows. Its large remainder must not enter the separate
    // below-budget expiry index/pass.
    sqlx::query("update feed_fanout_jobs set lease_expires_at=clock_timestamp()-interval '1 hour'")
        .execute(&f.pool)
        .await?;
    sqlx::query("analyze feed_fanout_jobs")
        .execute(&f.pool)
        .await?;
    for (name, sql) in [
        (
            "exhausted-expired",
            fanout_budget_sql::EXHAUSTED_MAINTENANCE_SQL,
        ),
        (
            "expired-exhausted",
            fanout_budget_sql::EXPIRED_MAINTENANCE_SQL,
        ),
    ] {
        let plan: serde_json::Value =
            sqlx::query_scalar(&format!("explain (analyze,buffers,format json) {sql}"))
                .fetch_one(&f.pool)
                .await?;
        println!("FANOUT_PLAN {name} {plan}");
        let mut stats = (false, 0, 0);
        scan_plan_stats(&plan, &mut stats);
        assert!(
            !stats.0 && stats.1 < 1000 && stats.2 < 1000,
            "{name} scanned expired exhausted backlog: {stats:?}"
        );
    }
    sqlx::query("update feed_fanout_jobs set status=case when job_id::text<'8' then 'PENDING' else 'SUSPENDED' end,locked_at=null,lease_token=null,lease_expires_at=null,next_source_check_at=clock_timestamp()-interval '1 hour'")
        .execute(&f.pool).await?;
    sqlx::query("analyze feed_fanout_jobs")
        .execute(&f.pool)
        .await?;
    let source_plan: serde_json::Value = sqlx::query_scalar(&format!(
        "explain (analyze,buffers,format json) {}",
        fanout_budget_sql::SOURCE_RECHECK_SQL
    ))
    .fetch_one(&f.pool)
    .await?;
    println!("FANOUT_PLAN source {source_plan}");
    let mut stats = (false, 0, 0);
    scan_plan_stats(&source_plan, &mut stats);
    assert!(
        !stats.0 && stats.1 < 1000 && stats.2 < 1000,
        "source scanned exhausted backlog: {stats:?}"
    );
    let claim_plan: serde_json::Value = sqlx::query_scalar(&format!(
        "explain (analyze,buffers,format json) {}",
        fanout_budget_sql::CLAIM_SQL
    ))
    .bind(60000_i64)
    .bind(1_i32)
    .bind(8_i32)
    .bind(100_i64)
    .bind(Vec::<Uuid>::new())
    .fetch_one(&f.pool)
    .await?;
    println!("FANOUT_PLAN claim {claim_plan}");
    let mut stats = (false, 0, 0);
    scan_plan_stats(&claim_plan, &mut stats);
    assert!(
        !stats.0 && stats.1 < 1000 && stats.2 < 1000,
        "claim scanned exhausted backlog: {stats:?}"
    );
    f.shutdown().await
}
