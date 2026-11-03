//! Actual generated mTLS services, PostgreSQL owners/eligibility, Redis effects.
use super::*;
use api_gateway::adapters::{
    out::cache_authority::GrpcCacheOwnerLookup,
    r#in::grpc::cache_authority::{CacheGrantGrpcServer, CACHE_GRANT_POLICY},
};
use friend_service::adapters::{
    out::{
        grpc::cache_admission::GrpcCacheAdmission,
        persistence::follow_repository::PgFollowRepository,
    },
    r#in::grpc::{
        cache_projection::{CacheProjectionGrpcServer, CACHE_PROJECTION_POLICY},
        fanout_projection::proto as fp,
    },
};
use platform::actor_delegation::cache::{
    self, CacheGuard, CacheIssuer, CacheOwnerSigner, CacheVerifier, LifetimePolicy,
};
use post_service::{
    adapters::{
        out::{
            grpc::claimed_cache_client::GrpcClaimedCacheClient,
            persistence::{
                cache_authority_repository::PgCacheAuthorityRepository,
                delivery_authority_repository::PgDeliveryAuthorityRepository,
                delivery_schedule::PgDeliverySchedule, event_outbox::PgEventOutbox,
            },
        },
        r#in::grpc::cache_authority::{self as mapping, CacheOwnerGrpcServer, CACHE_OWNER_POLICY},
    },
    application::{
        cache_authority::*,
        delivery_authority::{DeliveryAuthorityRepositoryPort, DeliveryClaimLookupDto},
        delivery_effect::DeliveryEffectFailure,
        dto::delivery_schedule_dto::{DeliveryScheduleConfigDto, TargetDeliveryClaimDto},
        ports::{DeliverySchedulePort, EventOutboxPort},
    },
    domain::post::{DeliveryTarget, StoredEvent},
};

#[derive(Clone)]
struct ProjectionEntries {
    server: CacheProjectionGrpcServer,
    entries: Arc<AtomicUsize>,
    reply_expiry: Arc<std::sync::atomic::AtomicI64>,
}
#[tonic::async_trait]
impl fp::cache_projection_service_server::CacheProjectionService for ProjectionEntries {
    async fn list_cache_followers(
        &self,
        r: Request<fp::ListCacheFollowersRequest>,
    ) -> Result<Response<fp::ListCacheFollowersResponse>, Status> {
        self.entries.fetch_add(1, Ordering::SeqCst);
        let mut response = self.server.list_cache_followers(r).await?;
        match self.reply_expiry.load(Ordering::SeqCst) {
            0 => {}
            mode => {
                assert!(
                    response.get_ref().follower_ids.is_empty(),
                    "malformed expiry cases include actual empty pages"
                );
                response.get_mut().expires_micros = match mode {
                    1 => 0,
                    2 => chrono::Utc::now().timestamp_micros() - 1,
                    _ => i64::MAX,
                };
            }
        }
        Ok(response)
    }
}
fn registry_all() -> anyhow::Result<CallerRegistry> {
    registry()?.register(Workload::Friend, "friend-fixture")
}
fn budget(edge: &'static str) -> EdgeBudget {
    EdgeBudget {
        edge,
        expected_latency: Duration::from_millis(50),
        deadline: Duration::from_secs(3),
        connect_deadline: Duration::from_secs(1),
        retry: RetryClass::Never,
        failure_policy: FailurePolicy::FailRequest,
    }
}
fn presented<T>(body: T, token: Option<&str>) -> Request<T> {
    let mut r = identified(body, Workload::Post, "post-fixture", None);
    if let Some(t) = token {
        r.metadata_mut().insert(cache::HEADER, t.parse().unwrap());
    }
    r.set_timeout(Duration::from_secs(3));
    r
}
async fn seed(
    pool: &PgPool,
    author: Uuid,
    kind: &str,
    lease_ms: u64,
) -> anyhow::Result<TargetDeliveryClaimDto> {
    let now = chrono::Utc::now() - chrono::Duration::seconds(1);
    let post = Uuid::new_v4();
    // Deliberately no live Post: deletion cleanup must survive resource purge.
    let e = StoredEvent {
        event_id: Uuid::new_v4(),
        event_type: kind.into(),
        aggregate_id: post,
        aggregate_version: Some(1),
        actor_id: Some(author),
        attributes: vec!["cache fixture".into()],
        payload: serde_json::json!({"postId":post,"authorId":author,"actorId":author}).to_string(),
        occurred_at: now,
        created_at: now,
        published: false,
        published_at: None,
        replay_resolution_id: None,
        origin_traceparent: Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".into()),
    };
    PgEventOutbox::new(pool.clone()).save(&e).await?;
    let mut batch = PgDeliverySchedule::new(pool.clone())
        .claim_target(
            DeliveryTarget::Cache,
            chrono::Utc::now(),
            1,
            &DeliveryScheduleConfigDto {
                lease_duration_ms: lease_ms,
                ..Default::default()
            },
        )
        .await?;
    anyhow::ensure!(batch.claims.len() == 1, "expected one real CACHE claim");
    let c = batch.claims.remove(0);
    anyhow::ensure!(c.event.event_id == e.event_id, "wrong scheduled event");
    Ok(c)
}
async fn page_lookup(
    pool: &PgPool,
    c: &TargetDeliveryClaimDto,
) -> anyhow::Result<CachePageLookupDto> {
    let owned = PgDeliveryAuthorityRepository::new(pool.clone())
        .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(c.clone())?)
        .await?;
    CachePageLookupDto::new(owned, c.event.actor_id.unwrap(), None, 2)
}
async fn wire(
    pool: &PgPool,
    c: &TargetDeliveryClaimDto,
) -> anyhow::Result<authority::ListCacheFollowersRequest> {
    let m = mapping::message(&mapping::reference(&page_lookup(pool, c).await?)?);
    Ok(authority::ListCacheFollowersRequest::decode(
        m.encode_to_vec().as_slice(),
    )?)
}
type IssuerClient = authority::cache_grant_service_client::CacheGrantServiceClient<Channel>;
async fn issue(
    client: &mut IssuerClient,
    m: &authority::ListCacheFollowersRequest,
) -> Result<authority::IssueCacheGrantResponse, Status> {
    client
        .issue_cache_grant(presented(
            authority::IssueCacheGrantRequest {
                framed_body: frame(m),
                deadline_micros: m.claim.as_ref().unwrap().lease_expires_micros,
            },
            None,
        ))
        .await
        .map(Response::into_inner)
}
struct LoseAfterRead {
    client: GrpcClaimedCacheClient,
    pool: PgPool,
}
#[async_trait::async_trait]
impl ClaimedCachePageReadPort for LoseAfterRead {
    async fn fetch_cache_page(
        &self,
        r: &CachePageLookupDto,
    ) -> anyhow::Result<CacheFollowerPageDto> {
        let p = self.client.fetch_cache_page(r).await?;
        sqlx::query("update post_event_delivery_jobs set lease_token=$1 where event_id=$2 and target='CACHE'")
            .bind(Uuid::new_v4()).bind(r.claim().claim().event.event_id).execute(&self.pool).await?;
        Ok(p)
    }
}
struct FailSecondPage {
    client: GrpcClaimedCacheClient,
    calls: AtomicUsize,
}
#[async_trait::async_trait]
impl ClaimedCachePageReadPort for FailSecondPage {
    async fn fetch_cache_page(
        &self,
        r: &CachePageLookupDto,
    ) -> anyhow::Result<CacheFollowerPageDto> {
        // Failure injection surrounds actual network projection and real first Redis write.
        anyhow::ensure!(
            self.calls.fetch_add(1, Ordering::SeqCst) == 0,
            "injected second-page outage"
        );
        self.client.fetch_cache_page(r).await
    }
}
async fn retire(pool: &PgPool, c: &TargetDeliveryClaimDto) -> anyhow::Result<()> {
    // Explicit fixture intervention avoids later seeds reclaiming test-failed work.
    sqlx::query("update post_event_delivery_jobs set state='RETRY',lease_token=null,attempt_id=null,lease_expires_at=null,next_attempt_at=clock_timestamp()+interval '1 day' where event_id=$1 and target='CACHE'")
        .bind(c.event.event_id).execute(pool).await?;
    Ok(())
}
async fn ready_retry(pool: &PgPool, c: &TargetDeliveryClaimDto) -> anyhow::Result<()> {
    PgDeliverySchedule::new(pool.clone()).fail_target(c).await?;
    sqlx::query("select pg_sleep(greatest(0,extract(epoch from (next_attempt_at-clock_timestamp())))+0.01) from post_event_delivery_jobs where event_id=$1 and target='CACHE'")
        .bind(c.event.event_id).execute(pool).await?;
    Ok(())
}
#[tokio::test]
async fn claimed_cache_composes_actual_mtls_pg_redis_with_missing_post_and_partial_effects(
) -> anyhow::Result<()> {
    let app = post_support::start_postgres_redis_app(false).await?;
    let friend_app = friend_support::start_postgres_app(false).await?;
    let author = Uuid::new_v4();
    let ids: Vec<_> = (31..=37).map(Uuid::from_u128).collect();
    for (i, id) in ids.iter().enumerate() {
        sqlx::query("insert into follows(follower_id,followee_id,created_at) values($1,$2,clock_timestamp()-make_interval(secs=>$3))")
            .bind(id).bind(author).bind(i as f64).execute(&friend_app.pool).await?;
    }
    for (a, b) in [(ids[3], author), (author, ids[4])] {
        sqlx::query("insert into user_blocks(blocker_id,blocked_id,created_at) values($1,$2,clock_timestamp())")
            .bind(a).bind(b).execute(&friend_app.pool).await?;
    }
    sqlx::query(
        "insert into user_mutes(muter_id,muted_id,created_at) values($1,$2,clock_timestamp())",
    )
    .bind(ids[5])
    .bind(author)
    .execute(&friend_app.pool)
    .await?;
    sqlx::query("update follows set deleted_at=clock_timestamp() where follower_id=$1")
        .bind(ids[6])
        .execute(&friend_app.pool)
        .await?;
    let c = seed(&app.pool, author, "POST_DELETED", 60_000).await?;
    let post_tls = tls(Workload::Post)?;
    let gateway_tls = tls(Workload::Gateway)?;
    let friend_tls = tls(Workload::Friend)?;
    let policy = LifetimePolicy::new(30_000_000).map_err(|_| anyhow::anyhow!("fixture policy"))?;
    let owner_key = CacheOwnerSigner::new(&[51; 32], policy)?;
    let issuer = CacheIssuer::new(&[52; 32], &[owner_key.public_key()], policy)?;
    let key = issuer.public_key();
    let lookup_pool = PgPool::connect_with(app.pool.connect_options().as_ref().clone()).await?;
    let refresh_lifetimes = Arc::new(std::sync::Mutex::new(
        std::collections::VecDeque::<i64>::new(),
    ));
    let pl = TcpListener::bind("127.0.0.1:0").await?;
    let pu = format!("https://{}", pl.local_addr()?);
    let gl = TcpListener::bind("127.0.0.1:0").await?;
    let gu = format!("https://{}", gl.local_addr()?);
    let fl = TcpListener::bind("127.0.0.1:0").await?;
    let fu = format!("https://{}", fl.local_addr()?);
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let ps = ServerTask::spawn(
        Server::builder()
            .tls_config(post_tls.server_config()?)?
            .add_service(guard_without_delegation(
                &CACHE_OWNER_POLICY,
                owner::cache_owner_authority_service_server::CacheOwnerAuthorityServiceServer::new(
                    refreshed_expiry::RefreshingOwner {
                        pool: lookup_pool.clone(),
                        lifetimes: refresh_lifetimes.clone(),
                    },
                ),
                registry_all()?,
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(pl), async {
                let _ = stopped.await;
            }),
        stop,
    );
    let independent = GrpcCacheOwnerLookup::connect(
        &pu,
        "gateway-fixture",
        Some(&gateway_tls),
        budget("cache owner fixture"),
    )?;
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let gs = ServerTask::spawn(
        Server::builder()
            .tls_config(gateway_tls.server_config()?)?
            .add_service(guard_without_delegation(
                &CACHE_GRANT_POLICY,
                authority::cache_grant_service_server::CacheGrantServiceServer::new(
                    CacheGrantGrpcServer::new(
                        issuer,
                        CacheVerifier::new(&[key], 128)?,
                        Arc::new(independent),
                    ),
                ),
                registry_all()?,
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(gl), async {
                let _ = stopped.await;
            }),
        stop,
    );
    let entries = Arc::new(AtomicUsize::new(0));
    let reply_expiry = Arc::new(std::sync::atomic::AtomicI64::new(0));
    let admission = GrpcCacheAdmission::connect(
        &gu,
        "friend-fixture",
        Some(&friend_tls),
        budget("cache admission fixture"),
    )?;
    let guarded = CacheGuard::new(
        fp::cache_projection_service_server::CacheProjectionServiceServer::new(ProjectionEntries {
            server: CacheProjectionGrpcServer::new(Arc::new(PgFollowRepository::new(
                friend_app.pool.clone(),
            ))),
            entries: entries.clone(),
            reply_expiry: reply_expiry.clone(),
        }),
        CacheVerifier::new(&[key], 128)?,
        Arc::new(admission),
    )?;
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let fs = ServerTask::spawn(
        Server::builder()
            .tls_config(friend_tls.server_config()?)?
            .add_service(guard_without_delegation(
                &CACHE_PROJECTION_POLICY,
                guarded,
                registry_all()?,
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(fl), async {
                let _ = stopped.await;
            }),
        stop,
    );
    let outcome=async {
        let (feeds,posts)=app.claimed_cache_effect_ports();
        let effects=app.claimed_cache_effects().await?;
        let own=Arc::new(PgDeliveryAuthorityRepository::new(app.pool.clone()));
        let reader=GrpcClaimedCacheClient::connect(&gu,&fu,"post-fixture",Some(&post_tls),
            [budget("cache issue fixture"),budget("cache read fixture")],CacheVerifier::new(&[key],128)?)?;
        let usecase=InvalidateClaimedCacheUseCase::new(own.clone(), Arc::new(reader.clone()), effects.clone(), 2)?;
        let mut ic=IssuerClient::new(post_tls.connect(&gu,Workload::Gateway).await?);
        let mut pc=authority::cache_projection_service_client::CacheProjectionServiceClient::new(post_tls.connect(&fu,Workload::Friend).await?);
        let m=wire(&app.pool,&c).await?;
        for variant in 0..10 {
            let mut forged=m.clone();let raw=forged.claim.as_mut().unwrap();
            match variant {
                0=>raw.event_id=Uuid::new_v4().to_string(),1=>raw.post_id=Uuid::new_v4().to_string(),
                2=>raw.event_digest[0]^=1,3=>raw.attempt_digest[0]^=1,4=>raw.lease_token=Uuid::new_v4().to_string(),
                5=>raw.attempt_id=Uuid::new_v4().to_string(),6=>raw.attempt+=1,7=>raw.lease_expires_micros+=1,
                8=>forged.author_id=Uuid::new_v4().to_string(),_=>forged.limit=3,
            }
            assert!(issue(&mut ic,&forged).await.is_err(),"forged cache field {variant}");
            assert_eq!(entries.load(Ordering::SeqCst),0);
        }
        assert!(pc.list_cache_followers(presented(m.clone(),None)).await.is_err());
        assert_eq!(entries.load(Ordering::SeqCst),0);
        let g=issue(&mut ic,&m).await?;
        pc.list_cache_followers(presented(m.clone(),Some(&g.token))).await?;
        let count=entries.load(Ordering::SeqCst);
        assert!(pc.list_cache_followers(presented(m.clone(),Some(&g.token))).await.is_err());
        assert_eq!(entries.load(Ordering::SeqCst),count);
        // A real stale Post key is evicted even though the owning Post is absent.
        // Seed through the actual adapter using a domain Post from existing fixture helper.
        let cached=post_service::domain::post::Post {id:c.event.aggregate_id,author_id:author,content:"cache seed".into(),visibility:"PUBLIC".into(),created_at:chrono::Utc::now(),comment_count:0,like_count:0};
        posts.set_post(&cached).await?;
        assert!(app.post_cache_key_exists(c.event.aggregate_id).await?);
        assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from posts where id=$1").bind(c.event.aggregate_id).fetch_one(&app.pool).await?,0);
        let cached_feed=post_service::domain::feed::FeedCachePage {post_ids:vec![c.event.aggregate_id],next_cursor:None};
        for id in ids.iter().chain(std::iter::once(&author)) {feeds.set_feed(*id,None,2,&cached_feed,0).await?;}
        usecase.invalidate_claimed_cache(&c, test_deadline()).await?;
        for id in ids.iter().take(3).chain(std::iter::once(&author)) {assert_eq!(feeds.current_generation(*id).await?,1);}
        for id in &ids[3..] {assert_eq!(feeds.current_generation(*id).await?,0);assert!(feeds.get_feed(*id,None,2).await?.is_some());}
        for id in ids.iter().take(3).chain(std::iter::once(&author)) {assert!(feeds.get_feed(*id,None,2).await?.is_none());}
        assert!(!app.post_cache_key_exists(c.event.aggregate_id).await?);
        assert_eq!(entries.load(Ordering::SeqCst),count+2,"all eligible pages traversed");
        PgDeliverySchedule::new(app.pool.clone()).complete_target(&c).await?;
        // No followers still invalidates author, while POST_CREATED retains Post cache.
        let alone=Uuid::new_v4();let created=seed(&app.pool,alone,"POST_CREATED",60_000).await?;
        let mut retained=cached.clone();retained.id=created.event.aggregate_id;retained.author_id=alone;
        posts.set_post(&retained).await?;
        usecase.invalidate_claimed_cache(&created, test_deadline()).await?;
        assert!(app.post_cache_key_exists(created.event.aggregate_id).await?);
        assert_eq!(feeds.current_generation(alone).await?,1);
        PgDeliverySchedule::new(app.pool.clone()).complete_target(&created).await?;
        // Lease loss after actual projection blocks a first feed write.
        let lost=seed(&app.pool,alone,"POST_CREATED",60_000).await?;
        let lost_case=InvalidateClaimedCacheUseCase::new(own.clone(), Arc::new(LoseAfterRead{client:reader.clone(),pool:app.pool.clone()}), effects.clone(), 2)?;
        let error=lost_case.invalidate_claimed_cache(&lost, test_deadline()).await.unwrap_err();
        assert_eq!(DeliveryEffectFailure::classify(&error),DeliveryEffectFailure::ConfirmedNoEffect);
        assert_eq!(feeds.current_generation(alone).await?,1); retire(&app.pool,&lost).await?;
        // Later page outage follows an actual acknowledged Redis generation bump.
        let partial=seed(&app.pool,author,"POST_CREATED",60_000).await?;
        let partial_case=InvalidateClaimedCacheUseCase::new(own, Arc::new(FailSecondPage{client:reader,calls:AtomicUsize::new(0)}), effects.clone(), 2)?;
        let error=partial_case.invalidate_claimed_cache(&partial, test_deadline()).await.unwrap_err();
        assert_eq!(DeliveryEffectFailure::classify(&error),DeliveryEffectFailure::Uncertain);
        for id in [ids[0],ids[1],author] {assert_eq!(feeds.current_generation(id).await?,2);}
        assert_eq!(feeds.current_generation(ids[2]).await?,1);retire(&app.pool,&partial).await?;
        // Explicit selection is exercised through the real forwarder and scheduler,
        // not merely by invoking the cache use case directly.
        let worker_author=Uuid::new_v4();let worker_claim=seed(&app.pool,worker_author,"POST_CREATED",60_000).await?;
        ready_retry(&app.pool,&worker_claim).await?;
        let worker=app.outbox_forwarder().with_claimed_cache(Arc::new(usecase));
        let before_entries=entries.load(Ordering::SeqCst);
        worker.republish_target(DeliveryTarget::Cache,chrono::Utc::now(),1).await?;
        assert_eq!(feeds.current_generation(worker_author).await?,1);
        assert_eq!(entries.load(Ordering::SeqCst),before_entries+1);
        let state:(String,i32)=sqlx::query_as("select state,attempt_count from post_event_delivery_jobs where event_id=$1 and target='CACHE'").bind(worker_claim.event.event_id).fetch_one(&app.pool).await?;
        assert_eq!(state,("SUCCEEDED".into(),2));
        assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from event_publications where event_id=$1 and target='CACHE'").bind(worker_claim.event.event_id).fetch_one(&app.pool).await?,1);
        refreshed_expiry::exercise_refresh(&app,&friend_app.pool,&post_tls,&gu,&fu,key,refresh_lifetimes.clone(),entries.clone(),reply_expiry.clone()).await?;
        // Expiry uses natural DB time, without changing stored expiry.
        let short=seed(&app.pool,alone,"POST_CREATED",500).await?;let sm=wire(&app.pool,&short).await?;
        let sg=issue(&mut ic,&sm).await?;let count=entries.load(Ordering::SeqCst);
        sqlx::query("select pg_sleep(greatest(0,extract(epoch from ($1::timestamptz-clock_timestamp())))+0.02)")
            .bind(short.lease_expires_at).execute(&app.pool).await?;
        assert!(pc.list_cache_followers(presented(sm,Some(&sg.token))).await.is_err());
        assert_eq!(entries.load(Ordering::SeqCst),count);retire(&app.pool,&short).await?;
        // Independent owner storage loss prevents admission and issuance.
        let active=seed(&app.pool,alone,"POST_CREATED",60_000).await?;let am=wire(&app.pool,&active).await?;
        let ag=issue(&mut ic,&am).await?;lookup_pool.close().await;
        assert!(pc.list_cache_followers(presented(am.clone(),Some(&ag.token))).await.is_err());
        assert!(issue(&mut ic,&am).await.is_err());assert_eq!(entries.load(Ordering::SeqCst),count);
        // A dedicated-path denial cannot fall back to the legacy Friend path.
        let failed_author=Uuid::new_v4();let failed=seed(&app.pool,failed_author,"POST_CREATED",60_000).await?;
        ready_retry(&app.pool,&failed).await?;
        worker.republish_target(DeliveryTarget::Cache,chrono::Utc::now(),1).await?;
        assert_eq!(feeds.current_generation(failed_author).await?,0);
        assert_eq!(entries.load(Ordering::SeqCst),count);
        let retry_state:(String,i32)=sqlx::query_as("select state,attempt_count from post_event_delivery_jobs where event_id=$1 and target='CACHE'").bind(failed.event.event_id).fetch_one(&app.pool).await?;
        assert_eq!(retry_state,("RETRY".into(),2),"worker must actually reclaim the failed event");
        let attempted:Vec<(Uuid,String,String)>=sqlx::query_as("select attempt_id,phase,source from post_event_delivery_attempts where event_id=$1 and target='CACHE' and attempt_id<>$2 order by phase")
            .bind(failed.event.event_id).bind(failed.attempt_id).fetch_all(&app.pool).await?;
        assert_eq!(attempted.len(),2,"fresh retry needs STARTED and FAILED markers");
        assert_ne!(attempted[0].0,failed.attempt_id);
        assert_eq!(attempted[0].0,attempted[1].0);
        assert_eq!((&*attempted[0].1,&*attempted[1].1),("FAILED","STARTED"));
        assert!(attempted.iter().all(|(_,_,source)|source=="OUTBOX_WORKER"));
        assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from event_publications where event_id=$1 and target='CACHE'").bind(failed.event.event_id).fetch_one(&app.pool).await?,0);
        Ok::<_,anyhow::Error>(())
    }.await;
    if let Err(e) = &outcome {
        eprintln!("CLAIMED_CACHE_PRIMARY_ERROR: {e:#}");
    }
    lookup_pool.close().await;
    ps.stop().await?;
    gs.stop().await?;
    fs.stop().await?;
    friend_app.shutdown().await?;
    outcome?;
    println!("CLAIMED_CACHE_MTLS_PG_REDIS: exact event/provenance, dedicated authority, actual eligibility/pagination, missing Post, author-only, replay denial, lost-generation zero write, partial Redis effects uncertain, natural expiry, unavailable owner; listeners stopped");
    Ok(())
}

fn test_deadline() -> post_service::application::delivery_effect::DeliveryEffectDeadlineDto {
    post_service::application::delivery_effect::DeliveryEffectDeadlineDto::new(
        chrono::Utc::now() + chrono::Duration::seconds(30),
        std::time::Instant::now() + Duration::from_secs(30),
    )
    .unwrap()
}

#[path = "claimed_cache/refreshed_expiry.rs"]
mod refreshed_expiry;
