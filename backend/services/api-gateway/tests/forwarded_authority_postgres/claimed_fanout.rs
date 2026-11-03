//! Real generated services, independent mTLS hops, owner PG and actual worker.
use super::*;
use api_gateway::adapters::out::fanout_authority::GrpcFanoutOwnerLookup;
use api_gateway::adapters::r#in::grpc::fanout_authority::{
    FanoutGrantGrpcServer, FANOUT_GRANT_POLICY,
};
use friend_service::adapters::out::grpc::fanout_admission::GrpcFanoutAdmission;
use friend_service::adapters::out::persistence::follow_repository::PgFollowRepository;
use friend_service::adapters::r#in::grpc::fanout_projection::{
    FanoutProjectionGrpcServer, FANOUT_PROJECTION_POLICY,
};
use platform::actor_delegation::fanout::{
    self, FanoutGuard, FanoutIssuer, FanoutOwnerSigner, FanoutVerifier, LifetimePolicy,
};
use post_service::adapters::out::grpc::claimed_fanout_client::GrpcClaimedFanoutClient;
use post_service::adapters::out::persistence::{
    fanout_authority_repository::PgFanoutAuthorityRepository,
    feed_recommendation_repository::PgFeedRecommendationRepository,
};
use post_service::adapters::r#in::grpc::fanout_authority::{
    self as mapping, FanoutOwnerGrpcServer, FANOUT_OWNER_POLICY,
};
use post_service::application::dto::timeline_fanout_dto::{
    TimelineFanoutClaimDto, TimelineFanoutClaimRequestDto, TimelineFanoutOutcomeDto,
    TimelineFanoutPageDto,
};
use post_service::application::fanout_authority::{
    ClaimedFanoutPageDto, ClaimedFanoutReadPort, FanoutPageLookupDto, FanoutPagePolicyDto,
};
use post_service::application::ports::TimelineFanoutRepositoryPort;
use post_service::application::use_cases::process_timeline_fanout::{
    ProcessTimelineFanoutUseCase, TimelineFanoutConfig,
};

#[derive(Clone)]
struct ProjectionEntries {
    server: FanoutProjectionGrpcServer,
    entries: Arc<AtomicUsize>,
    reply_expiry: Arc<std::sync::atomic::AtomicI64>,
}
#[tonic::async_trait]
impl friend_service::adapters::r#in::grpc::fanout_projection::proto::fanout_projection_service_server::FanoutProjectionService for ProjectionEntries {
    async fn list_fanout_followers(&self, request: Request<friend_service::adapters::r#in::grpc::fanout_projection::proto::ListFanoutFollowersRequest>) -> Result<Response<friend_service::adapters::r#in::grpc::fanout_projection::proto::ListFanoutFollowersResponse>, Status> {
        self.entries.fetch_add(1, Ordering::SeqCst);
        let mut response=self.server.list_fanout_followers(request).await?;
        match self.reply_expiry.load(Ordering::SeqCst) {
            0=>{},
            mode=>{
                assert!(response.get_ref().follower_ids.is_empty(),"malformed expiry cases include actual empty pages");
                response.get_mut().expires_micros=match mode {
                    1=>0,2=>chrono::Utc::now().timestamp_micros()-1,_=>i64::MAX,
                };
            }
        }
        Ok(response)
    }
}
fn registry_all() -> anyhow::Result<CallerRegistry> {
    registry()?.register(Workload::Friend, "friend-fixture")
}
fn budget(name: &'static str) -> EdgeBudget {
    EdgeBudget {
        edge: name,
        expected_latency: Duration::from_millis(50),
        deadline: Duration::from_secs(3),
        connect_deadline: Duration::from_secs(1),
        retry: RetryClass::Never,
        failure_policy: FailurePolicy::FailRequest,
    }
}
fn presented<T>(body: T, caller: Workload, credential: &str, token: Option<&str>) -> Request<T> {
    let mut request = identified(body, caller, credential, None);
    if let Some(token) = token {
        request
            .metadata_mut()
            .insert(fanout::HEADER, token.parse().unwrap());
    }
    request.set_timeout(Duration::from_secs(3));
    request
}
async fn seed(pool: &PgPool, author: Uuid) -> anyhow::Result<Uuid> {
    let post = Uuid::new_v4();
    let created = sqlx::query_scalar("insert into posts(id,author_id,content,visibility,created_at) values($1,$2,'claimed native','PUBLIC',clock_timestamp()) returning created_at").bind(post).bind(author).fetch_one(pool).await?;
    PgFeedRecommendationRepository::new(pool.clone())
        .enqueue_post(
            Uuid::new_v4(),
            post,
            author,
            created,
            7,
            1000,
            chrono::Utc::now(),
        )
        .await?;
    Ok(post)
}
async fn claim(pool: &PgPool, lease: Duration) -> anyhow::Result<TimelineFanoutClaimDto> {
    let mut claimed = PgFeedRecommendationRepository::new(pool.clone())
        .claim_jobs(&TimelineFanoutClaimRequestDto {
            lease_timeout: lease,
            limit: 1,
            max_page_attempts: 3,
            retry_base: Duration::from_millis(100),
            excluded_job_ids: vec![],
        })
        .await?;
    anyhow::ensure!(
        claimed.len() == 1,
        "expected exactly one claimable fanout job, got {}; {}",
        claimed.len(),
        claim_diagnostics(pool).await?.join(" | ")
    );
    Ok(claimed.remove(0))
}
/// Explain why no job was claimable.
///
/// `claim_jobs` filters on status, the page budget, the exclusion set and the
/// due time, and it locks candidates with `for update skip locked`, so a row
/// another transaction holds is skipped silently. An empty result alone cannot
/// say which of those held, so dump the recent rows and probe the pending row
/// locks with NOWAIT.
async fn claim_diagnostics(pool: &PgPool) -> anyhow::Result<Vec<String>> {
    type Row = (
        String,
        bool,
        Option<i64>,
        i32,
        Option<i32>,
        String,
        Option<String>,
    );
    let rows: Vec<Row> =
        sqlx::query_as(
            r#"select status, next_attempt_at <= statement_timestamp(),
                      (extract(epoch from (next_attempt_at - statement_timestamp())) * 1000)::bigint,
                      attempt_count, page_attempt_count, page_budget_provenance, locked_at::text
               from feed_fanout_jobs order by created_at desc limit 5"#,
        )
        .fetch_all(pool)
        .await?;
    let mut report = Vec::new();
    for (status, due, due_in_ms, attempts, page_attempts, provenance, locked) in rows {
        report.push(format!(
            "status={status} due={due} due_in_ms={due_in_ms:?} attempts={attempts} page_attempts={page_attempts:?} provenance={provenance} locked={locked:?}"
        ));
    }
    let mut tx = pool.begin().await?;
    let probe: Result<Vec<Uuid>, sqlx::Error> = sqlx::query_scalar(
        r#"select job_id from feed_fanout_jobs where status = 'PENDING' order by next_attempt_at limit 5 for update nowait"#,
    )
    .fetch_all(&mut *tx)
    .await;
    report.push(match probe {
        Ok(ids) => format!("pending rows lockable now: {}", ids.len()),
        Err(error) => format!("pending row held by another transaction: {error}"),
    });
    tx.rollback().await?;
    Ok(report)
}

async fn snapshot(pool: &PgPool) -> anyhow::Result<String> {
    Ok(sqlx::query_scalar("select jsonb_build_object('jobs',(select jsonb_agg(to_jsonb(j) order by job_id) from feed_fanout_jobs j),'posts',(select jsonb_agg(to_jsonb(p) order by id) from posts p),'timeline',(select jsonb_agg(to_jsonb(h) order by viewer_id,post_id,source) from home_timeline_entries h),'outbox',(select jsonb_agg(to_jsonb(e) order by event_id) from post_event_outbox e),'deliveries',(select jsonb_agg(to_jsonb(d) order by event_id,target) from post_event_delivery_jobs d),'attempts',(select jsonb_agg(to_jsonb(a) order by attempt_id,phase) from post_event_delivery_attempts a),'publications',(select jsonb_agg(to_jsonb(e) order by event_id,target) from event_publications e))::text").fetch_one(pool).await?)
}
type IssuerClient = authority::fanout_grant_service_client::FanoutGrantServiceClient<Channel>;
type ProjectionClient =
    authority::fanout_projection_service_client::FanoutProjectionServiceClient<Channel>;
fn resign_fixture_grant(token: &str, field: &str, value: serde_json::Value) -> String {
    use base64ct::{Base64UrlUnpadded, Encoding};
    let payload = token.split_once('.').unwrap().0;
    let mut claims: serde_json::Value =
        serde_json::from_slice(&Base64UrlUnpadded::decode_vec(payload).unwrap()).unwrap();
    if field == "scope" {
        claims["scope"]["limit"] = value;
    } else {
        claims[field] = value;
    }
    let payload = Base64UrlUnpadded::encode_string(&serde_json::to_vec(&claims).unwrap());
    let key = ring::signature::Ed25519KeyPair::from_seed_unchecked(&[42; 32]).unwrap();
    let signature = key.sign(
        &[
            b"port-rust claimed fanout gateway grant v1\n".as_slice(),
            payload.as_bytes(),
        ]
        .concat(),
    );
    format!(
        "{payload}.{}",
        Base64UrlUnpadded::encode_string(signature.as_ref())
    )
}
async fn issue(
    client: &mut IssuerClient,
    message: &authority::ListFanoutFollowersRequest,
) -> Result<authority::IssueFanoutGrantResponse, Status> {
    client
        .issue_fanout_grant(presented(
            authority::IssueFanoutGrantRequest {
                framed_body: frame(message),
                deadline_micros: message.claim.as_ref().unwrap().lease_expires_micros,
            },
            Workload::Post,
            "post-fixture",
            None,
        ))
        .await
        .map(Response::into_inner)
}

struct LoseAfterProjection {
    client: GrpcClaimedFanoutClient,
    pool: PgPool,
}
#[async_trait::async_trait]
impl ClaimedFanoutReadPort for LoseAfterProjection {
    async fn fetch_claimed_page(
        &self,
        request: &FanoutPageLookupDto,
    ) -> anyhow::Result<ClaimedFanoutPageDto> {
        let page = self.client.fetch_claimed_page(request).await?;
        // Deterministic generation loss after actual projection, before the
        // actual worker calls the actual Pg apply_page fence.
        sqlx::query("update feed_fanout_jobs set lease_token=$1 where job_id=$2")
            .bind(Uuid::new_v4())
            .bind(request.claim().job.job_id)
            .execute(&self.pool)
            .await?;
        Ok(page)
    }
}
#[tokio::test]
async fn claimed_fanout_crosses_actual_owner_gateway_friend_and_worker_with_zero_entry_denials(
) -> anyhow::Result<()> {
    let app = post_support::start_postgres_redis_app(false).await?;
    let friend_app = friend_support::start_postgres_app(false).await?;
    let author = Uuid::new_v4();
    let post = seed(&app.pool, author).await?;
    let stored = claim(&app.pool, Duration::from_secs(60)).await?;
    assert_eq!(stored.job.post_id, post);
    let recipients: Vec<_> = (1..=7).map(Uuid::from_u128).collect();
    for (index, follower) in recipients.iter().enumerate() {
        sqlx::query("insert into follows(follower_id,followee_id,created_at) values($1,$2,clock_timestamp()-make_interval(secs=>$3))").bind(follower).bind(author).bind(index as f64).execute(&friend_app.pool).await?;
    }
    // Either direction block, follower mute, and soft-deleted follow all remain
    // filtered by Friend's owning query. Three eligible recipients remain.
    for (blocker, blocked) in [(recipients[3], author), (author, recipients[4])] {
        sqlx::query("insert into user_blocks(blocker_id,blocked_id,created_at) values($1,$2,clock_timestamp())").bind(blocker).bind(blocked).execute(&friend_app.pool).await?;
    }
    sqlx::query(
        "insert into user_mutes(muter_id,muted_id,created_at) values($1,$2,clock_timestamp())",
    )
    .bind(recipients[5])
    .bind(author)
    .execute(&friend_app.pool)
    .await?;
    sqlx::query("update follows set deleted_at=clock_timestamp() where follower_id=$1")
        .bind(recipients[6])
        .execute(&friend_app.pool)
        .await?;

    let post_tls = tls(Workload::Post)?;
    let gateway_tls = tls(Workload::Gateway)?;
    let friend_tls = tls(Workload::Friend)?;
    let policy =
        LifetimePolicy::new(30_000_000).map_err(|_| anyhow::anyhow!("fixture lifetime"))?;
    let owner_key = FanoutOwnerSigner::new(&[41; 32], policy)?;
    let issuer = FanoutIssuer::new(&[42; 32], &[owner_key.public_key()], policy)?;
    let grant_key = issuer.public_key();
    let lookup_pool = PgPool::connect_with(app.pool.connect_options().as_ref().clone()).await?;
    let refresh_lifetimes = Arc::new(std::sync::Mutex::new(
        std::collections::VecDeque::<i64>::new(),
    ));
    let post_listener = TcpListener::bind("127.0.0.1:0").await?;
    let post_url = format!("https://{}", post_listener.local_addr()?);
    let gateway_listener = TcpListener::bind("127.0.0.1:0").await?;
    let gateway_url = format!("https://{}", gateway_listener.local_addr()?);
    let friend_listener = TcpListener::bind("127.0.0.1:0").await?;
    let friend_url = format!("https://{}", friend_listener.local_addr()?);
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let post_server = ServerTask::spawn(Server::builder().tls_config(post_tls.server_config()?)?
        .add_service(guard_without_delegation(&FANOUT_OWNER_POLICY, owner::fanout_owner_authority_service_server::FanoutOwnerAuthorityServiceServer::new(refreshed_expiry::RefreshingOwner {pool:lookup_pool.clone(),lifetimes:refresh_lifetimes.clone()}), registry_all()?)?)
        .serve_with_incoming_shutdown(TcpListenerStream::new(post_listener), async { let _ = stopped.await; }), stop);
    let independent = GrpcFanoutOwnerLookup::connect(
        &post_url,
        "gateway-fixture",
        Some(&gateway_tls),
        budget("fanout owner fixture"),
    )?;
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let gateway_server = ServerTask::spawn(
        Server::builder()
            .tls_config(gateway_tls.server_config()?)?
            .add_service(guard_without_delegation(
                &FANOUT_GRANT_POLICY,
                authority::fanout_grant_service_server::FanoutGrantServiceServer::new(
                    FanoutGrantGrpcServer::new(
                        issuer,
                        FanoutVerifier::new(&[grant_key], 32)?,
                        Arc::new(independent),
                    ),
                ),
                registry_all()?,
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(gateway_listener), async {
                let _ = stopped.await;
            }),
        stop,
    );
    let entries = Arc::new(AtomicUsize::new(0));
    let reply_expiry = Arc::new(std::sync::atomic::AtomicI64::new(0));
    let admission = GrpcFanoutAdmission::connect(
        &gateway_url,
        "friend-fixture",
        Some(&friend_tls),
        budget("fanout admission fixture"),
    )?;
    let projection =
        FanoutProjectionGrpcServer::new(Arc::new(PgFollowRepository::new(friend_app.pool.clone())));
    let guarded = FanoutGuard::new(friend_service::adapters::r#in::grpc::fanout_projection::proto::fanout_projection_service_server::FanoutProjectionServiceServer::new(ProjectionEntries { server: projection, entries: entries.clone(), reply_expiry:reply_expiry.clone() }), FanoutVerifier::new(&[grant_key], 32)?, Arc::new(admission))?;
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let friend_server = ServerTask::spawn(
        Server::builder()
            .tls_config(friend_tls.server_config()?)?
            .add_service(guard_without_delegation(
                &FANOUT_PROJECTION_POLICY,
                guarded,
                registry_all()?,
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(friend_listener), async {
                let _ = stopped.await;
            }),
        stop,
    );
    let outcome = async {
        let mut issuer = IssuerClient::new(post_tls.connect(&gateway_url, Workload::Gateway).await?);
        let mut projection = ProjectionClient::new(post_tls.connect(&friend_url, Workload::Friend).await?);
        let request = mapping::message(&FanoutPageLookupDto::new(stored.clone(), 2)?);
        // DTO definitions are generated independently by each service.
        let request = authority::ListFanoutFollowersRequest::decode(request.encode_to_vec().as_slice())?;
        let before = snapshot(&app.pool).await?;
        for case in 0..12 {
            let mut forged = request.clone(); let c = forged.claim.as_mut().unwrap();
            match case { 0 => c.job_id = Uuid::new_v4().to_string(), 1 => c.event_id = Uuid::new_v4().to_string(), 2 => c.post_id = Uuid::new_v4().to_string(), 3 => c.author_id = Uuid::new_v4().to_string(), 4 => c.lease_token = Uuid::new_v4().to_string(), 5 => c.post_created_micros += 1, 6 => c.lease_expires_micros += 1, 7 => c.attempt += 1, 8 => c.page_attempt += 1, 9 => c.max_page_attempts += 1, 10 => c.cursor = Some(format!("2026-10-14T00:00:00Z|{}", recipients[0])), _ => forged.limit = 11 }
            assert!(issue(&mut issuer, &forged).await.is_err(), "forged binding case {case}");
            assert_eq!(snapshot(&app.pool).await?, before); assert_eq!(entries.load(Ordering::SeqCst), 0);
        }
        let grant = issue(&mut issuer, &request).await?;
        assert!(grant.expires_micros <= stored.lease_expires_at.timestamp_micros());
        // Workload-only, actor proof, unknown system key, altered body, sibling
        // workload and certificate/header disagreement all stop before handler.
        let actor = ActorDelegationSigner::local_parity(Workload::Gateway).issue(Workload::Friend, fanout::METHOD, author, None, &frame(&request), chrono::Utc::now().timestamp());
        let wrong_owner = FanoutOwnerSigner::new(&[44;32], policy)?;
        let wrong_issuer = FanoutIssuer::new(&[43;32], &[wrong_owner.public_key()], policy)?;
        let now = chrono::Utc::now().timestamp_micros();
        let proposed = wrong_issuer.prepare(Workload::Post, mapping::scope(&FanoutPageLookupDto::new(stored.clone(), 2)?)?, frame(&request), now).map_err(|_| anyhow::anyhow!("fixture forged preparation"))?;
        let evidence = wrong_owner.attest(mapping::scope(&FanoutPageLookupDto::new(stored.clone(), 2)?)?, &frame(&request), proposed.challenge(), now, now).map_err(|_| anyhow::anyhow!("fixture forged owner"))?;
        let forged = wrong_issuer.finish(proposed, &evidence, now).map_err(|_| anyhow::anyhow!("fixture forged issuance"))?;
        for token in [None, Some("invalid"), Some(actor.as_str()), Some(forged.token.as_str())] {
            assert!(projection.list_fanout_followers(presented(request.clone(), Workload::Post, "post-fixture", token)).await.is_err());
        }
        for (field, value) in [("purpose", serde_json::json!("notification_recipients")), ("aud", serde_json::json!("profile")), ("scope", serde_json::json!(1))] {
            let token = resign_fixture_grant(&grant.token, field, value);
            assert!(projection.list_fanout_followers(presented(request.clone(), Workload::Post, "post-fixture", Some(&token))).await.is_err());
        }
        let mut duplicate = presented(request.clone(), Workload::Post, "post-fixture", Some(&grant.token));
        duplicate.metadata_mut().append(fanout::HEADER, grant.token.parse().unwrap());
        assert!(projection.list_fanout_followers(duplicate).await.is_err());
        let mut wrong = request.clone(); wrong.limit = 1;
        assert!(projection.list_fanout_followers(presented(wrong, Workload::Post, "post-fixture", Some(&grant.token))).await.is_err());
        assert!(projection.list_fanout_followers(presented(request.clone(), Workload::Gateway, "gateway-fixture", Some(&grant.token))).await.is_err());
        let mut sibling = ProjectionClient::new(gateway_tls.connect(&friend_url, Workload::Friend).await?);
        assert!(sibling.list_fanout_followers(presented(request.clone(), Workload::Gateway, "gateway-fixture", Some(&grant.token))).await.is_err());
        assert_eq!(entries.load(Ordering::SeqCst), 0); assert_eq!(snapshot(&app.pool).await?, before);

        // Revocation committed before issuance and before consumption are
        // distinct: both must stop before projection and make no owner writes.
        sqlx::query("update posts set visibility='PRIVATE' where id=$1").bind(post).execute(&app.pool).await?;
        let private = snapshot(&app.pool).await?;
        assert!(issue(&mut issuer, &request).await.is_err());
        assert!(projection.list_fanout_followers(presented(request.clone(), Workload::Post, "post-fixture", Some(&grant.token))).await.is_err());
        assert_eq!(entries.load(Ordering::SeqCst), 0); assert_eq!(snapshot(&app.pool).await?, private);
        sqlx::query("update posts set visibility='PUBLIC' where id=$1").bind(post).execute(&app.pool).await?;
        for transition in 0..2 {
            let issued = issue(&mut issuer, &request).await?;
            if transition == 0 { sqlx::query("update feed_fanout_jobs set lease_token=$1 where job_id=$2").bind(Uuid::new_v4()).bind(stored.job.job_id).execute(&app.pool).await?; }
            else { sqlx::query("update posts set author_id=$1 where id=$2").bind(Uuid::new_v4()).bind(post).execute(&app.pool).await?; }
            let revoked = snapshot(&app.pool).await?;
            assert!(projection.list_fanout_followers(presented(request.clone(), Workload::Post, "post-fixture", Some(&issued.token))).await.is_err());
            assert_eq!(entries.load(Ordering::SeqCst), 0); assert_eq!(snapshot(&app.pool).await?, revoked);
            if transition == 0 { sqlx::query("update feed_fanout_jobs set lease_token=$1 where job_id=$2").bind(stored.lease_token).bind(stored.job.job_id).execute(&app.pool).await?; }
            else { sqlx::query("update posts set author_id=$1 where id=$2").bind(author).bind(post).execute(&app.pool).await?; }
        }
        // All concurrent consumers present the same valid proof. Exactly one
        // handler entry wins; no timeline write occurs during projection.
        let grant = issue(&mut issuer, &request).await?;
        let before = snapshot(&app.pool).await?;
        let mut tasks = Vec::new();
        for _ in 0..8 { let mut client = projection.clone(); let request = presented(request.clone(), Workload::Post, "post-fixture", Some(&grant.token)); tasks.push(tokio::spawn(async move { client.list_fanout_followers(request).await })); }
        let mut pages = Vec::new(); for task in tasks { if let Ok(response) = task.await? { pages.push(response.into_inner()); } }
        assert_eq!(pages.len(), 1); assert_eq!(entries.load(Ordering::SeqCst), 1);
        assert_eq!(pages[0].follower_ids, recipients[..2].iter().map(Uuid::to_string).collect::<Vec<_>>());
        assert!(pages[0].next_cursor.is_some()); assert_eq!(snapshot(&app.pool).await?, before);
        // Reissuance is a new grant for the same persisted attempt, not replay.
        let fresh = issue(&mut issuer, &request).await?; assert_ne!(fresh.token, grant.token);
        let page = projection.list_fanout_followers(presented(request.clone(), Workload::Post, "post-fixture", Some(&fresh.token))).await?.into_inner();
        assert_eq!(entries.load(Ordering::SeqCst), 2); assert_eq!(snapshot(&app.pool).await?, before);
        // Current follower mute after issuance is enforced by the owning query.
        let issued = issue(&mut issuer, &request).await?;
        sqlx::query("insert into user_mutes(muter_id,muted_id,created_at) values($1,$2,clock_timestamp())").bind(recipients[0]).bind(author).execute(&friend_app.pool).await?;
        let current = projection.list_fanout_followers(presented(request.clone(), Workload::Post, "post-fixture", Some(&issued.token))).await?.into_inner();
        assert_eq!(current.follower_ids, recipients[1..3].iter().map(Uuid::to_string).collect::<Vec<_>>());
        sqlx::query("delete from user_mutes where muter_id=$1 and muted_id=$2").bind(recipients[0]).bind(author).execute(&friend_app.pool).await?;
        // Actual page advancement invalidates an already-issued old-cursor grant.
        let stale = issue(&mut issuer, &request).await?;
        let repo = PgFeedRecommendationRepository::new(app.pool.clone());
        assert_eq!(repo.apply_page(&stored, &TimelineFanoutPageDto { follower_ids: recipients[..2].to_vec(), next_cursor: page.next_cursor, expires_at: stored.job.post_created_at + chrono::Duration::hours(1) }).await?, TimelineFanoutOutcomeDto::Applied);
        let after_page = snapshot(&app.pool).await?; let count = entries.load(Ordering::SeqCst);
        assert!(projection.list_fanout_followers(presented(request.clone(), Workload::Post, "post-fixture", Some(&stale.token))).await.is_err());
        assert_eq!(entries.load(Ordering::SeqCst), count); assert_eq!(snapshot(&app.pool).await?, after_page);
        let next = claim(&app.pool, Duration::from_secs(60)).await?;
        let next_request = mapping::message(&FanoutPageLookupDto::new(next.clone(), 2)?);
        let next_request = authority::ListFanoutFollowersRequest::decode(next_request.encode_to_vec().as_slice())?;
        let issued = issue(&mut issuer, &next_request).await?;
        let page = projection.list_fanout_followers(presented(next_request, Workload::Post, "post-fixture", Some(&issued.token))).await?.into_inner();
        assert_eq!(page.follower_ids, vec![recipients[2].to_string()]); assert!(page.next_cursor.is_none());
        assert_eq!(repo.apply_page(&next, &TimelineFanoutPageDto { follower_ids: vec![recipients[2]], next_cursor: None, expires_at: next.job.post_created_at + chrono::Duration::hours(1) }).await?, TimelineFanoutOutcomeDto::Applied);

        // Expiry is natural database time, not an UPDATE of the expiry row.
        let short_post = seed(&app.pool, author).await?; let short = claim(&app.pool, Duration::from_millis(250)).await?;
        let short_request = mapping::message(&FanoutPageLookupDto::new(short.clone(), 2)?);
        let short_request = authority::ListFanoutFollowersRequest::decode(short_request.encode_to_vec().as_slice())?;
        let short_grant = issue(&mut issuer, &short_request).await?;
        sqlx::query("select pg_sleep(greatest(0,extract(epoch from ($1::timestamptz-clock_timestamp())))+0.02)").bind(short.lease_expires_at).execute(&app.pool).await?;
        let expired = snapshot(&app.pool).await?; let count = entries.load(Ordering::SeqCst);
        assert!(projection.list_fanout_followers(presented(short_request, Workload::Post, "post-fixture", Some(&short_grant.token))).await.is_err());
        assert_eq!(entries.load(Ordering::SeqCst), count); assert_eq!(snapshot(&app.pool).await?, expired);
        sqlx::query("update posts set visibility='PRIVATE' where id=$1").bind(short_post).execute(&app.pool).await?;
        let reader = GrpcClaimedFanoutClient::connect(&gateway_url, &friend_url, "post-fixture", Some(&post_tls), [budget("worker issue fixture"), budget("worker projection fixture")], FanoutVerifier::new(&[grant_key], 32)?)?;
        let config = TimelineFanoutConfig { claim_limit: 1, follower_page_size: 10, lease_timeout: Duration::from_secs(60), max_attempts: 3, base_retry: Duration::from_millis(100), entry_ttl: Duration::from_secs(3600) };
        let happy = seed(&app.pool, author).await?;
        let count = entries.load(Ordering::SeqCst);
        assert_eq!(ProcessTimelineFanoutUseCase::new_claimed(Arc::new(repo.clone()), Arc::new(reader.clone()), Arc::new(repo.clone()), config.clone())?.execute(chrono::Utc::now()).await?, 1);
        assert_eq!(entries.load(Ordering::SeqCst), count + 1);
        let rows: Vec<(Uuid, bool)> = sqlx::query_as("select viewer_id, expires_at=p.created_at+interval '1 hour' from home_timeline_entries h join posts p on p.id=h.post_id where h.post_id=$1 and source='MATERIALIZED_TIMELINE' order by viewer_id").bind(happy).fetch_all(&app.pool).await?;
        assert_eq!(rows, recipients[..3].iter().map(|id| (*id, true)).collect::<Vec<_>>());
        let lost = seed(&app.pool, author).await?;
        let self_before: String = sqlx::query_scalar("select jsonb_agg(to_jsonb(h) order by viewer_id,source)::text from home_timeline_entries h where post_id=$1").bind(lost).fetch_one(&app.pool).await?;
        assert_eq!(ProcessTimelineFanoutUseCase::new_claimed(Arc::new(repo.clone()), Arc::new(LoseAfterProjection { client: reader, pool: app.pool.clone() }), Arc::new(repo.clone()), config)?.execute(chrono::Utc::now()).await?, 0);
        assert_eq!(entries.load(Ordering::SeqCst), count + 2);
        let writes: i64 = sqlx::query_scalar("select count(*) from home_timeline_entries where post_id=$1 and source='MATERIALIZED_TIMELINE'").bind(lost).fetch_one(&app.pool).await?; assert_eq!(writes, 0);
        let self_after: String = sqlx::query_scalar("select jsonb_agg(to_jsonb(h) order by viewer_id,source)::text from home_timeline_entries h where post_id=$1").bind(lost).fetch_one(&app.pool).await?;
        assert_eq!(self_before, self_after, "SELF_HISTORY remains unchanged after lost application");
        refreshed_expiry::exercise_refresh(&app,&friend_app.pool,&post_tls,&gateway_url,&friend_url,grant_key,refresh_lifetimes.clone(),entries.clone(),reply_expiry.clone()).await?;
        // Existing valid grant + failed independent owner storage is unavailable
        // before another projection entry, with durable rows unchanged.
        seed(&app.pool, author).await?;
        let active = claim(&app.pool, Duration::from_secs(60)).await?;
        let active = mapping::message(&FanoutPageLookupDto::new(active, 2)?);
        let active = authority::ListFanoutFollowersRequest::decode(active.encode_to_vec().as_slice())?;
        let issued = issue(&mut issuer, &active).await?;
        lookup_pool.close().await;
        let count = entries.load(Ordering::SeqCst); let before = snapshot(&app.pool).await?;
        assert!(projection.list_fanout_followers(presented(active.clone(), Workload::Post, "post-fixture", Some(&issued.token))).await.is_err());
        assert!(issue(&mut issuer, &active).await.is_err());
        assert_eq!(entries.load(Ordering::SeqCst), count); assert_eq!(snapshot(&app.pool).await?, before);
        Ok::<_, anyhow::Error>(())
    }.await;
    if let Err(error) = &outcome {
        eprintln!("CLAIMED_FANOUT_PRIMARY_ERROR: {error:#}");
    }
    lookup_pool.close().await;
    post_server.stop().await?;
    gateway_server.stop().await?;
    friend_server.stop().await?;
    friend_app.shutdown().await?;
    outcome?;
    println!("CLAIMED_FANOUT_MTLS_PG: independent owner issuance/admission; exact claims; revocation before issuance/consumption; current Friend filtering/pagination; atomic concurrent replay; explicit actual worker; stale application zero writes; unavailable owner zero entries; listeners stopped");
    Ok(())
}

#[path = "claimed_fanout/refreshed_expiry.rs"]
mod refreshed_expiry;
