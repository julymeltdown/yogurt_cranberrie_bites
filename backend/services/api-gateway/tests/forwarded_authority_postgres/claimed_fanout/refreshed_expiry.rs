//! Actual persistence effects: no in-memory repository substitutes.
use super::*;
use post_service::application::{
    delivery_effect::{DeliveryEffectDeadlineDto, DeliveryEffectFailure},
    fanout_authority::ClaimedFanoutApplyPort,
};
fn deadline(duration: Duration) -> DeliveryEffectDeadlineDto {
    DeliveryEffectDeadlineDto::new(
        chrono::Utc::now() + chrono::Duration::from_std(duration).unwrap(),
        std::time::Instant::now() + duration,
    )
    .unwrap()
}
async fn fresh(pool: &PgPool) -> anyhow::Result<TimelineFanoutClaimDto> {
    seed(pool, Uuid::new_v4()).await?;
    claim(pool, Duration::from_secs(60)).await
}
fn page(c: &TimelineFanoutClaimDto) -> TimelineFanoutPageDto {
    TimelineFanoutPageDto {
        follower_ids: vec![Uuid::new_v4()],
        next_cursor: None,
        expires_at: c.job.post_created_at + chrono::Duration::days(14),
    }
}
async fn retire(pool: &PgPool, c: &TimelineFanoutClaimDto) -> anyhow::Result<()> {
    sqlx::query("update feed_fanout_jobs set status='DEAD_LETTERED',lease_token=null,lease_expires_at=null,locked_at=null,dead_lettered_at=clock_timestamp() where job_id=$1")
        .bind(c.job.job_id).execute(pool).await?;
    Ok(())
}
async fn no_durable_effect(pool: &PgPool, c: &TimelineFanoutClaimDto) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    // Wait for the old transaction's row locks too; a hidden uncommitted row is
    // not sufficient rollback evidence. The dedicated session is checked idle.
    sqlx::query("select id from posts where id=$1 for update")
        .bind(c.job.post_id)
        .execute(&mut *tx)
        .await?;
    let row = sqlx::query(
        "select status,cursor,lease_token from feed_fanout_jobs where job_id=$1 for update",
    )
    .bind(c.job.job_id)
    .fetch_one(&mut *tx)
    .await?;
    assert_eq!(row.get::<String, _>("status"), "PROCESSING");
    assert_eq!(row.get::<Option<String>, _>("cursor"), c.job.cursor);
    assert_eq!(row.get::<Uuid, _>("lease_token"), c.lease_token);
    let count:i64=sqlx::query_scalar("select count(*) from home_timeline_entries where post_id=$1 and source='MATERIALIZED_TIMELINE'").bind(c.job.post_id).fetch_one(&mut *tx).await?;
    assert_eq!(count, 0);
    tx.rollback().await?;
    Ok(())
}
#[tokio::test]
async fn refreshed_expiry_rolls_back_pool_subject_post_job_and_insert_waits() -> anyhow::Result<()>
{
    let app = post_support::start_postgres_redis_app(false).await?;
    for mode in 0..5 {
        let c = fresh(&app.pool).await?;
        let p = page(&c);
        let name = format!("refreshed-expiry-{}", Uuid::new_v4());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with(
                app.pool
                    .connect_options()
                    .as_ref()
                    .clone()
                    .application_name(&name),
            )
            .await?;
        let held = if mode == 0 {
            Some(pool.acquire().await?)
        } else {
            None
        };
        let mut blocker = app.pool.begin().await?;
        match mode {
            0 => {}
            1 => {
                sqlx::query("select post_lock_subject_ids($1::uuid[])")
                    .bind(vec![c.job.author_id])
                    .execute(&mut *blocker)
                    .await?;
            }
            2 => {
                sqlx::query("select id from posts where id=$1 for update")
                    .bind(c.job.post_id)
                    .execute(&mut *blocker)
                    .await?;
            }
            3 => {
                sqlx::query("select job_id from feed_fanout_jobs where job_id=$1 for update")
                    .bind(c.job.job_id)
                    .execute(&mut *blocker)
                    .await?;
            }
            _ => {
                // Owned superuser fixture only: avoid taking a parent FK lock so
                // the injected wait is the unique insertion conflict itself.
                sqlx::query("set local session_replication_role='replica'")
                    .execute(&mut *blocker)
                    .await?;
                sqlx::query("insert into home_timeline_entries(viewer_id,post_id,author_id,source,source_score,inserted_at,expires_at) values($1,$2,$3,'MATERIALIZED_TIMELINE',1,$4,$5)")
                    .bind(p.follower_ids[0]).bind(c.job.post_id).bind(c.job.author_id).bind(c.job.post_created_at).bind(p.expires_at).execute(&mut *blocker).await?;
            }
        }
        let repo = PgFeedRecommendationRepository::new(pool.clone());
        let copy = c.clone();
        let budget = deadline(Duration::from_millis(800));
        let task = tokio::spawn(async move { repo.apply_claimed_page(&copy, &p, budget).await });
        if mode > 0 {
            tokio::time::timeout(Duration::from_millis(500),async {
                loop {
                    let waiting:i64=sqlx::query_scalar("select count(*) from pg_stat_activity where application_name=$1 and wait_event_type='Lock'")
                        .bind(&name).fetch_one(&app.pool).await.unwrap();
                    if waiting==1 {break;}
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            }).await?;
        }
        let error = tokio::time::timeout(Duration::from_secs(2), task)
            .await??
            .unwrap_err();
        assert_eq!(
            DeliveryEffectFailure::classify(&error),
            DeliveryEffectFailure::ConfirmedNoEffect,
            "mode {mode}: {error:#}"
        );
        assert!(
            chrono::Utc::now() < c.lease_expires_at,
            "owner lease remains live"
        );
        blocker.rollback().await?;
        drop(held);
        tokio::time::timeout(Duration::from_secs(2),async {
            loop {
                let active:i64=sqlx::query_scalar("select count(*) from pg_stat_activity where application_name=$1 and (xact_start is not null or state='active')")
                    .bind(&name).fetch_one(&app.pool).await.unwrap();
                if active==0 {break;}
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }).await?;
        tokio::time::timeout(Duration::from_secs(2), no_durable_effect(&app.pool, &c)).await??;
        pool.close().await;
        retire(&app.pool, &c).await?;
    }
    println!("REFRESHED_FANOUT_ROLLBACK: actual pool, subject, Post, job and insert-conflict waits; original lease live; independent idle/lock/entry/cursor verification");
    Ok(())
}
#[tokio::test]
async fn bounded_apply_rejects_every_frozen_claim_mutation_and_preserves_entry_retention(
) -> anyhow::Result<()> {
    let app = post_support::start_postgres_redis_app(false).await?;
    let c = fresh(&app.pool).await?;
    let p = page(&c);
    let repo = PgFeedRecommendationRepository::new(app.pool.clone());
    let retry_base: i64 =
        sqlx::query_scalar("select retry_base_ms from feed_fanout_jobs where job_id=$1")
            .bind(c.job.job_id)
            .fetch_one(&app.pool)
            .await?;
    for field in 0..12 {
        let mut forged = c.clone();
        match field {
            0 => forged.job.job_id = Uuid::new_v4(),
            1 => forged.job.event_id = Uuid::new_v4(),
            2 => forged.job.post_id = Uuid::new_v4(),
            3 => forged.job.author_id = Uuid::new_v4(),
            4 => forged.lease_token = Uuid::new_v4(),
            5 => forged.job.attempt_count += 1,
            6 => forged.job.cursor = Some(format!("2026-10-14T00:00:00Z|{}", Uuid::new_v4())),
            7 => forged.lease_expires_at += chrono::Duration::microseconds(1),
            8 => forged.job.post_created_at += chrono::Duration::microseconds(1),
            9 => forged.page_attempt_count += 1,
            10 => forged.max_page_attempts += 1,
            _ => {
                sqlx::query("update feed_fanout_jobs set page_budget_provenance='LEGACY_UNVERIFIED',page_attempt_count=null,max_page_attempts=null,retry_base_ms=null where job_id=$1").bind(c.job.job_id).execute(&app.pool).await?;
            }
        }
        assert_eq!(
            repo.apply_claimed_page(&forged, &p, deadline(Duration::from_secs(2)))
                .await?,
            TimelineFanoutOutcomeDto::Lost,
            "field {field}"
        );
        no_durable_effect(&app.pool, &c).await?;
    }
    sqlx::query("update feed_fanout_jobs set page_budget_provenance='CURRENT_PAGE',page_attempt_count=$2,max_page_attempts=$3,retry_base_ms=$4 where job_id=$1").bind(c.job.job_id).bind(c.page_attempt_count).bind(c.max_page_attempts)
        .bind(retry_base).execute(&app.pool).await?;
    assert_eq!(
        repo.apply_claimed_page(&c, &p, deadline(Duration::from_secs(2)))
            .await?,
        TimelineFanoutOutcomeDto::Applied
    );
    let expiry:chrono::DateTime<chrono::Utc>=sqlx::query_scalar("select expires_at from home_timeline_entries where viewer_id=$1 and post_id=$2 and source='MATERIALIZED_TIMELINE'")
        .bind(p.follower_ids[0]).bind(c.job.post_id).fetch_one(&app.pool).await?;
    assert_eq!(
        expiry, p.expires_at,
        "entry retention must not become authority lifetime"
    );
    println!("REFRESHED_FANOUT_CLAIM: 12 frozen mutations denied; actual bounded commit; independent retention verified");
    Ok(())
}

#[derive(Clone)]
pub(super) struct RefreshingOwner {
    pub pool: PgPool,
    pub lifetimes: Arc<std::sync::Mutex<std::collections::VecDeque<i64>>>,
}
#[tonic::async_trait]
impl owner::fanout_owner_authority_service_server::FanoutOwnerAuthorityService for RefreshingOwner {
    async fn resolve_fanout_page(
        &self,
        request: Request<owner::ResolveFanoutPageRequest>,
    ) -> Result<Response<owner::ResolveFanoutPageResponse>, Status> {
        let lifetime = self
            .lifetimes
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(30_000_000);
        let signer =
            FanoutOwnerSigner::new(&[41; 32], LifetimePolicy::new(lifetime).unwrap()).unwrap();
        FanoutOwnerGrpcServer::new(
            Arc::new(PgFanoutAuthorityRepository::new(
                self.pool.clone(),
                FanoutPagePolicyDto::new(10).unwrap(),
            )),
            signer,
        )
        .resolve_fanout_page(request)
        .await
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn exercise_refresh(
    app: &post_support::TestApp,
    friend_pool: &PgPool,
    post_tls: &TlsMaterial,
    gateway: &str,
    friend: &str,
    key: [u8; 32],
    lifetimes: Arc<std::sync::Mutex<std::collections::VecDeque<i64>>>,
    entries: Arc<AtomicUsize>,
    reply_expiry: Arc<std::sync::atomic::AtomicI64>,
) -> anyhow::Result<()> {
    use post_service::application::fanout_authority::ClaimedFanoutReadPort;
    let c = fresh(&app.pool).await?;
    let lookup = FanoutPageLookupDto::new(c.clone(), 2)?;
    let message = mapping::message(&lookup);
    let message =
        authority::ListFanoutFollowersRequest::decode(message.encode_to_vec().as_slice())?;

    let mut issuer = IssuerClient::new(post_tls.connect(gateway, Workload::Gateway).await?);
    let grant = issue(&mut issuer, &message).await?;
    let friend_tls = tls(Workload::Friend)?;
    let mut validator = IssuerClient::new(friend_tls.connect(gateway, Workload::Gateway).await?);
    lifetimes.lock().unwrap().push_back(800_000);
    let refreshed = validator
        .validate_fanout_grant(identified(
            authority::ValidateFanoutGrantRequest {
                token: grant.token.clone(),
                framed_body: frame(&message),
            },
            Workload::Friend,
            "friend-fixture",
            None,
        ))
        .await?
        .into_inner();
    assert!(refreshed.expires_micros < grant.expires_micros - 10_000_000);
    let mut projection =
        authority::fanout_projection_service_client::FanoutProjectionServiceClient::new(
            post_tls.connect(friend, Workload::Friend).await?,
        );
    lifetimes.lock().unwrap().push_back(800_000);
    let response = projection
        .list_fanout_followers(presented(
            message.clone(),
            Workload::Post,
            "post-fixture",
            Some(&grant.token),
        ))
        .await?
        .into_inner();
    assert!(response.expires_micros < grant.expires_micros - 10_000_000);
    assert!(response.expires_micros > chrono::Utc::now().timestamp_micros());
    // The actual Post presenter must retain the independently refreshed expiry,
    // including an empty page, rather than returning its original issuance cap.
    let reader = GrpcClaimedFanoutClient::connect(
        gateway,
        friend,
        "post-fixture",
        Some(post_tls),
        [budget("refresh issue"), budget("refresh projection")],
        FanoutVerifier::new(&[key], 128)?,
    )?;
    lifetimes.lock().unwrap().extend([30_000_000, 800_000]);
    let page = reader.fetch_claimed_page(&lookup).await?;
    assert!(page.expires_at() <= chrono::Utc::now() + chrono::Duration::milliseconds(800));
    assert!(page.expires_at() < c.lease_expires_at - chrono::Duration::seconds(10));
    // Proto3 zero expiry serializes as an omitted scalar. Corrupt only the
    // response of the actual owning handler, after all real admission/query work.
    for mode in 1..=3 {
        reply_expiry.store(mode, Ordering::SeqCst);
        assert!(
            reader.fetch_claimed_page(&lookup).await.is_err(),
            "malformed projection expiry {mode}"
        );
        no_durable_effect(&app.pool, &c).await?;
    }
    reply_expiry.store(0, Ordering::SeqCst);
    exercise_uncertain_worker(app, &reader).await?;
    // A real Friend query blocks after admission; expiry must deny its result
    // while the owner lease and original grant still have substantial lifetime.
    let before = entries.load(Ordering::SeqCst);
    let mut blocked = friend_pool.begin().await?;
    sqlx::query("lock table follows in access exclusive mode")
        .execute(&mut *blocked)
        .await?;
    lifetimes.lock().unwrap().extend([30_000_000, 800_000]);
    let copy = lookup.clone();
    let task = tokio::spawn(async move { reader.fetch_claimed_page(&copy).await });
    tokio::time::timeout(Duration::from_millis(600),async {
        loop {
            let waiting:i64=sqlx::query_scalar("select count(*) from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like '%follows%'").fetch_one(friend_pool).await.unwrap();
            if waiting>0 {break;}
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await?;
    let result = tokio::time::timeout(Duration::from_secs(2), task).await??;
    blocked.rollback().await?;
    assert!(result.is_err());
    assert_eq!(entries.load(Ordering::SeqCst), before + 1);
    assert!(chrono::Utc::now().timestamp_micros() < grant.expires_micros);
    assert!(chrono::Utc::now() < c.lease_expires_at);
    retire(&app.pool, &c).await?;
    println!("REFRESHED_Fanout_MTLS: actual owner shorter signing policy, Gateway validation, Friend seal/response, Post DTO including empty page; actual blocked Friend query denied while original grant/lease live");
    Ok(())
}

struct CommitAckProxy(tokio::task::JoinHandle<std::io::Result<()>>);
impl Drop for CommitAckProxy {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn drop_commit_ack(
    listener: TcpListener,
    backend_port: u16,
    committed: tokio::sync::oneshot::Sender<()>,
) -> std::io::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (front, _) = listener.accept().await?;
    let backend = tokio::net::TcpStream::connect(("127.0.0.1", backend_port)).await?;
    let (mut fr, mut fw) = front.into_split();
    let (mut br, mut bw) = backend.into_split();
    // With explicit Disable SSL the backend protocol is framed from its first
    // Authentication response. Client startup/extended messages pass untouched.
    let to_backend = tokio::io::copy(&mut fr, &mut bw);
    let to_front = async {
        loop {
            let tag = br.read_u8().await?;
            let len = br.read_u32().await?;
            if !(4..=32 * 1024 * 1024).contains(&len) {
                return Err(std::io::Error::other("invalid owned PostgreSQL frame"));
            }
            let mut body = vec![0; (len - 4) as usize];
            br.read_exact(&mut body).await?;
            if tag == b'C' && body == b"COMMIT\0" {
                // The actual server reported COMMIT, but none of this reply is
                // delivered to SQLx. This is not an injected application error.
                let _ = committed.send(());
                return Ok(());
            }
            fw.write_u8(tag).await?;
            fw.write_u32(len).await?;
            fw.write_all(&body).await?;
        }
    };
    tokio::select! {r=to_backend=>r.map(|_|()),r=to_front=>r}
}
#[tokio::test]
async fn actual_postgres_commit_ack_loss_is_uncertain_with_durable_fanout_page(
) -> anyhow::Result<()> {
    let app = post_support::start_postgres_redis_app(false).await?;
    let c = fresh(&app.pool).await?;
    let p = page(&c);
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let (committed, observed) = tokio::sync::oneshot::channel();
    let backend_port = app.pool.connect_options().get_port();
    let mut proxy = CommitAckProxy(tokio::spawn(drop_commit_ack(
        listener,
        backend_port,
        committed,
    )));
    let options = app
        .pool
        .connect_options()
        .as_ref()
        .clone()
        .host("127.0.0.1")
        .port(port)
        .ssl_mode(sqlx::postgres::PgSslMode::Disable);
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    let repo = PgFeedRecommendationRepository::new(pool.clone());
    let error = tokio::time::timeout(
        Duration::from_secs(3),
        repo.apply_claimed_page(&c, &p, deadline(Duration::from_secs(2))),
    )
    .await?
    .unwrap_err();
    tokio::time::timeout(Duration::from_secs(1), observed).await??;
    assert_eq!(
        DeliveryEffectFailure::classify(&error),
        DeliveryEffectFailure::Uncertain
    );
    tokio::time::timeout(Duration::from_secs(1), &mut proxy.0).await???;
    pool.close().await;
    let row = sqlx::query(
        "select status,cursor,lease_token,page_attempt_count from feed_fanout_jobs where job_id=$1",
    )
    .bind(c.job.job_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(row.get::<String, _>("status"), "COMPLETED");
    assert_eq!(row.get::<Option<String>, _>("cursor"), None);
    assert_eq!(row.get::<Option<Uuid>, _>("lease_token"), None);
    assert_eq!(row.get::<i32, _>("page_attempt_count"), 0);
    let entries:i64=sqlx::query_scalar("select count(*) from home_timeline_entries where post_id=$1 and source='MATERIALIZED_TIMELINE'")
        .bind(c.job.post_id).fetch_one(&app.pool).await?;
    assert_eq!(entries, 1);
    println!("REFRESHED_FANOUT_COMMIT_ACK_LOSS: actual PostgreSQL COMMIT observed at owned loopback protocol proxy, acknowledgement dropped, adapter Uncertain, independently verified durable entry and completed cursor/job");
    Ok(())
}

struct ObservedFanoutRepository {
    pg: PgFeedRecommendationRepository,
    failures: AtomicUsize,
    legacy_apply: AtomicUsize,
}
#[async_trait::async_trait]
impl TimelineFanoutRepositoryPort for ObservedFanoutRepository {
    async fn enqueue_post(
        &self,
        event: Uuid,
        post: Uuid,
        author: Uuid,
        created: chrono::DateTime<chrono::Utc>,
        followers: i64,
        threshold: i64,
        now: chrono::DateTime<chrono::Utc>,
    ) -> anyhow::Result<post_service::domain::ranking::AuthorDeliveryMode> {
        self.pg
            .enqueue_post(event, post, author, created, followers, threshold, now)
            .await
    }
    async fn claim_jobs(
        &self,
        r: &TimelineFanoutClaimRequestDto,
    ) -> anyhow::Result<Vec<TimelineFanoutClaimDto>> {
        self.pg.claim_jobs(r).await
    }
    async fn apply_page(
        &self,
        c: &TimelineFanoutClaimDto,
        p: &TimelineFanoutPageDto,
    ) -> anyhow::Result<TimelineFanoutOutcomeDto> {
        self.legacy_apply.fetch_add(1, Ordering::SeqCst);
        self.pg.apply_page(c, p).await
    }
    async fn record_job_failure(
        &self,
        c: &TimelineFanoutClaimDto,
        f: &post_service::application::dto::timeline_fanout_dto::TimelineFanoutFailureDto,
    ) -> anyhow::Result<TimelineFanoutOutcomeDto> {
        self.failures.fetch_add(1, Ordering::SeqCst);
        self.pg.record_job_failure(c, f).await
    }
    async fn remove_post(&self, id: Uuid) -> anyhow::Result<()> {
        self.pg.remove_post(id).await
    }
}
struct UncertainApply {
    explicit: bool,
    seen: std::sync::Mutex<Option<TimelineFanoutClaimDto>>,
}
#[async_trait::async_trait]
impl ClaimedFanoutApplyPort for UncertainApply {
    async fn apply_claimed_page(
        &self,
        c: &TimelineFanoutClaimDto,
        _: &TimelineFanoutPageDto,
        _: DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<TimelineFanoutOutcomeDto> {
        *self.seen.lock().unwrap() = Some(c.clone());
        let error = anyhow::anyhow!("injected unknown write acknowledgement");
        Err(if self.explicit {
            DeliveryEffectFailure::uncertain(error)
        } else {
            error
        })
    }
}
async fn exercise_uncertain_worker(
    app: &post_support::TestApp,
    reader: &GrpcClaimedFanoutClient,
) -> anyhow::Result<()> {
    for explicit in [true, false] {
        let post = seed(&app.pool, Uuid::new_v4()).await?;
        let observed = Arc::new(ObservedFanoutRepository {
            pg: PgFeedRecommendationRepository::new(app.pool.clone()),
            failures: AtomicUsize::new(0),
            legacy_apply: AtomicUsize::new(0),
        });
        let writer = Arc::new(UncertainApply {
            explicit,
            seen: std::sync::Mutex::new(None),
        });
        let worker = ProcessTimelineFanoutUseCase::new_claimed(
            observed.clone(),
            Arc::new(reader.clone()),
            writer.clone(),
            TimelineFanoutConfig {
                claim_limit: 1,
                follower_page_size: 2,
                ..TimelineFanoutConfig::default()
            },
        )?;
        assert_eq!(worker.execute(chrono::Utc::now()).await?, 0);
        let c = writer
            .seen
            .lock()
            .unwrap()
            .clone()
            .expect("actual selected writer entered");
        assert_eq!(c.job.post_id, post);
        assert_eq!(observed.failures.load(Ordering::SeqCst), 0);
        assert_eq!(observed.legacy_apply.load(Ordering::SeqCst), 0);
        no_durable_effect(&app.pool, &c).await?;
        let retained:(chrono::DateTime<chrono::Utc>,i32,i32)=sqlx::query_as("select lease_expires_at,attempt_count,page_attempt_count from feed_fanout_jobs where job_id=$1")
            .bind(c.job.job_id).fetch_one(&app.pool).await?;
        assert_eq!(
            retained,
            (
                c.lease_expires_at,
                c.job.attempt_count,
                c.page_attempt_count
            )
        );
        retire(&app.pool, &c).await?;
    }
    println!("REFRESHED_FANOUT_UNCERTAINTY: actual worker, PostgreSQL claim and mTLS reader; explicit/unknown writer errors yield zero failure-scheduler and legacy-apply calls; exact original lease and attempts retained");
    Ok(())
}
