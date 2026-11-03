//! The real scheduling workers, owning PostgreSQL and all four mTLS services.
use super::*;
use post_service::{
    adapters::out::{
        grpc::claimed_notification_client::GrpcClaimedNotificationClient,
        persistence::post_repository::PgPostRepository,
    },
    application::{
        claimed_notification::*,
        delivery_authority::{DeliveryAuthorityErrorDto, OwnedDeliveryClaimDto},
        dto::delivery_schedule_dto::DeliveryExecutionConfigDto,
        ports::ReplayEventDeliveryPort,
    },
};

/// Replace only the acknowledgement after the actual destination future has
/// completed. The owning row count independently proves the commit occurred.
#[derive(Clone)]
pub(super) struct ReplyFault<S> {
    inner: S,
    mode: Arc<AtomicU8>,
    calls: Arc<AtomicUsize>,
    budget_probe: budget::Probe,
}
impl<S> ReplyFault<S> {
    pub(super) fn new(
        inner: S,
        mode: Arc<AtomicU8>,
        calls: Arc<AtomicUsize>,
        budget_probe: budget::Probe,
    ) -> Self {
        Self {
            inner,
            mode,
            calls,
            budget_probe,
        }
    }
}
impl<S: tonic::server::NamedService> tonic::server::NamedService for ReplyFault<S> {
    const NAME: &'static str = S::NAME;
}
impl<S> tower::Service<http::Request<Body>> for ReplyFault<S>
where
    S: tower::Service<http::Request<Body>, Response = http::Response<Body>> + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
{
    type Response = http::Response<Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, r: http::Request<Body>) -> Self::Future {
        let write = r.uri().path() == WRITE_METHOD;
        let mut budget_phase = None;
        if write {
            if let Some(raw) = r
                .headers()
                .get("grpc-timeout")
                .and_then(|h| h.to_str().ok())
            {
                let (value, unit) = raw.split_at(raw.len() - 1);
                let n = value.parse::<u64>().unwrap();
                let timeout = match unit {
                    "H" => Duration::from_secs(n * 3600),
                    "M" => Duration::from_secs(n * 60),
                    "S" => Duration::from_secs(n),
                    "m" => Duration::from_millis(n),
                    "u" => Duration::from_micros(n),
                    "n" => Duration::from_nanos(n),
                    _ => panic!("invalid gRPC timeout"),
                };
                budget_phase = self.budget_probe.ingress(
                    r.headers()
                        .get(capability::HEADER)
                        .and_then(|v| v.to_str().ok()),
                    timeout,
                );
            }
        }
        let future = self.inner.call(r);
        let mode = self.mode.clone();
        let calls = self.calls.clone();
        Box::pin(async move {
            let result = future.await;
            if let Some(p) = &budget_phase {
                p.complete(result.is_ok());
            }
            let response = result?;
            let mode = mode.load(Ordering::SeqCst);
            if !write || mode == 0 {
                return Ok(response);
            }
            calls.fetch_add(1, Ordering::SeqCst);
            if mode == 1 {
                return Ok(Status::unavailable("injected acknowledgement loss").into_http());
            }
            let payload = np::CreateNotificationResponse {
                notification: None,
                suppressed: false,
            }
            .encode_to_vec();
            let mut frame = vec![0];
            frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            frame.extend(payload);
            let mut response = http::Response::new(Body::new(http_body_util::Full::new(
                tonic::codegen::Bytes::from(frame),
            )));
            response
                .headers_mut()
                .insert("content-type", "application/grpc".parse().unwrap());
            response
                .headers_mut()
                .insert("grpc-status", "0".parse().unwrap());
            Ok(response)
        })
    }
}

pub(super) fn legacy_events() -> Vec<(StoredEvent, bool)> {
    [false, true]
        .into_iter()
        .map(|checkpoint| {
            let post = Uuid::new_v4();
            let actor = Uuid::new_v4();
            let recipient = Uuid::new_v4();
            let now = chrono::DateTime::from_timestamp_micros(
                chrono::Utc::now().timestamp_micros() - 1_000_000,
            )
            .unwrap();
            (
                StoredEvent {
                    event_id: Uuid::new_v4(),
                    event_type: "COMMENT".into(),
                    aggregate_id: Uuid::new_v4(),
                    aggregate_version: None,
                    actor_id: Some(actor),
                    attributes: vec!["retained legacy".into()],
                    payload: serde_json::json!({"postId":post,"notifyUserId":recipient})
                        .to_string(),
                    occurred_at: now,
                    created_at: now,
                    published: true,
                    published_at: Some(now),
                    replay_resolution_id: None,
                    origin_traceparent: None,
                },
                checkpoint,
            )
        })
        .collect()
}

// Faults are applied only after a real result; no owner/page/write evidence is
// fabricated. Persistence and cryptographic admission always use actual services.
struct FaultTransport {
    inner: Arc<dyn ClaimedNotificationTransportPort>,
    pool: PgPool,
    mode: u8,
    pages: AtomicUsize,
    writes: AtomicUsize,
    policy: Arc<AtomicU8>,
}
#[tonic::async_trait]
impl ClaimedNotificationTransportPort for FaultTransport {
    async fn select_page(
        &self,
        r: &NotificationPageRequestDto,
    ) -> anyhow::Result<NotificationPageResultDto> {
        let page = self.inner.select_page(r).await?;
        self.pages.fetch_add(1, Ordering::SeqCst);
        if self.mode == 1 {
            PgDeliverySchedule::new(self.pool.clone())
                .fail_target(r.owner().lookup().claim())
                .await?;
        } else if self.mode == 2 {
            sqlx::query("update post_event_delivery_jobs set max_attempts=max_attempts+1 where event_id=$1 and target='INTERNAL'")
                .bind(r.owner().lookup().claim().event.event_id).execute(&self.pool).await?;
        } else if self.mode == 4 {
            if let NotificationPageResultDto::Selected(ref p) = page {
                let remaining = (p.expires_at() - chrono::Utc::now())
                    .to_std()
                    .unwrap_or_default();
                tokio::time::sleep(remaining + Duration::from_millis(20)).await;
            }
        }
        Ok(page)
    }
    async fn write(
        &self,
        r: &ClaimedNotificationWriteDto,
    ) -> anyhow::Result<ClaimedNotificationAckDto> {
        let result = self.inner.write(r).await?;
        self.writes.fetch_add(1, Ordering::SeqCst);
        if self.mode == 3 {
            self.policy.store(3, Ordering::SeqCst);
        }
        if self.mode == 5 {
            anyhow::bail!("injected unclassified post-write failure");
        }
        Ok(result)
    }
}
struct DelayedOwner {
    inner: PgDeliveryAuthorityRepository,
    calls: AtomicUsize,
    delay: Duration,
}
#[tonic::async_trait]
impl DeliveryAuthorityRepositoryPort for DelayedOwner {
    async fn resolve_claimed_delivery(
        &self,
        r: &DeliveryClaimLookupDto,
    ) -> Result<OwnedDeliveryClaimDto, DeliveryAuthorityErrorDto> {
        let result = self.inner.resolve_claimed_delivery(r).await;
        self.calls.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(self.delay).await;
        result
    }
}
fn delivery(
    app: &post_support::TestApp,
    transport: Arc<dyn ClaimedNotificationTransportPort>,
) -> anyhow::Result<Arc<dyn ClaimedNotificationDeliveryPort>> {
    Ok(Arc::new(DeliverClaimedNotificationsUseCase::new(
        Arc::new(PgDeliveryAuthorityRepository::new(app.pool.clone())),
        Arc::new(PgPostRepository::new(app.pool.clone())),
        transport,
        1,
    )?))
}
async fn job(pool: &PgPool, id: Uuid) -> anyhow::Result<(String, i32, i32)> {
    Ok(sqlx::query_as("select state,attempt_count,max_attempts from post_event_delivery_jobs where event_id=$1 and target='INTERNAL'")
        .bind(id).fetch_one(pool).await?)
}
async fn count(pool: &PgPool, id: Uuid) -> anyhow::Result<i64> {
    Ok(
        sqlx::query_scalar("select count(*) from notifications where event_id=$1")
            .bind(id)
            .fetch_one(pool)
            .await?,
    )
}
pub(super) async fn attempt(
    pool: &PgPool,
    id: Uuid,
) -> anyhow::Result<(Uuid, String, Vec<String>)> {
    let (attempt,source): (Uuid,String)=sqlx::query_as("select attempt_id,source from post_event_delivery_attempts where event_id=$1 and target='INTERNAL' and phase='STARTED' order by id desc limit 1").bind(id).fetch_one(pool).await?;
    let phases = sqlx::query_scalar(
        "select phase from post_event_delivery_attempts where attempt_id=$1 order by id",
    )
    .bind(attempt)
    .fetch_all(pool)
    .await?;
    Ok((attempt, source, phases))
}
pub(super) async fn ready(
    app: &post_support::TestApp,
    c: &TargetDeliveryClaimDto,
) -> anyhow::Result<()> {
    PgDeliverySchedule::new(app.pool.clone())
        .fail_target(c)
        .await?;
    sqlx::query("update post_event_delivery_jobs set next_attempt_at=clock_timestamp()-interval '1 second' where event_id=$1 and target='INTERNAL'")
        .bind(c.event.event_id).execute(&app.pool).await?;
    Ok(())
}
async fn park(app: &post_support::TestApp, id: Uuid) -> anyhow::Result<()> {
    // Isolate this failed fixture from later automatic scenarios. It is not
    // presented as scheduler evidence; the observed state is asserted first.
    sqlx::query("update post_event_delivery_jobs set next_attempt_at=clock_timestamp()+interval '1 hour' where event_id=$1 and target='INTERNAL'")
        .bind(id).execute(&app.pool).await?;
    Ok(())
}
pub(super) async fn automatic(
    app: &post_support::TestApp,
    port: Arc<dyn ClaimedNotificationDeliveryPort>,
) -> anyhow::Result<()> {
    app.outbox_forwarder()
        .with_claimed_notifications(port)
        .republish_target(DeliveryTarget::Internal, chrono::Utc::now(), 1)
        .await?;
    Ok(())
}
async fn followers(pool: &PgPool, author: Uuid) -> anyhow::Result<()> {
    for _ in 0..2 {
        sqlx::query("insert into follows(follower_id,followee_id,created_at) values($1,$2,clock_timestamp())")
            .bind(Uuid::new_v4()).bind(author).execute(pool).await?;
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(super) async fn exercise(
    post: &post_support::TestApp,
    friend: &friend_support::TestApp,
    destination: &notification_support::TestApp,
    legacy: &[(StoredEvent, bool)],
    endpoints: [&str; 3],
    tls: &TlsMaterial,
    key: [u8; 32],
    membership_key: [u8; 32],
    policy: Arc<AtomicU8>,
    policy_calls: Arc<AtomicUsize>,
    short_owner: Arc<AtomicU8>,
    reply_mode: Arc<AtomicU8>,
    reply_calls: Arc<AtomicUsize>,
    budget_probe: budget::Probe,
    commit_proxy: &mut commit_ack::CommitAckProxy,
) -> anyhow::Result<()> {
    let real: Arc<dyn ClaimedNotificationTransportPort> =
        Arc::new(GrpcClaimedNotificationClient::connect(
            endpoints[0],
            endpoints[1],
            endpoints[2],
            "post-fixture",
            tls,
            [
                budget("worker issue"),
                budget("worker page"),
                budget("worker write"),
            ],
            NotificationVerifier::new(&[key], 128)?,
            NotificationMembershipVerifier::new(&[membership_key])?,
        )?);
    let selected = delivery(post, real.clone())?;
    let legacy_before = post.notification_requests().await.len();

    // Both genuine migration outcomes retain zero automatic attempts during
    // replay, including original COMMENT aggregate/payload-post divergence.
    for (event, checkpoint) in legacy {
        let expected = if *checkpoint { "SUCCEEDED" } else { "SKIPPED" };
        assert_eq!(
            job(&post.pool, event.event_id).await?,
            (expected.into(), 0, 5)
        );
        let replay = post
            .fenced_replay_delivery(Default::default())?
            .with_claimed_notifications(selected.clone());
        replay
            .replay_target(event.event_id, DeliveryTarget::Internal)
            .await?;
        let original: (Uuid, String, Uuid, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
            "select id,payload_json,actor_id,occurred_at from notifications where event_id=$1",
        )
        .bind(event.event_id)
        .fetch_one(&destination.pool)
        .await?;
        assert_eq!(original.2, event.actor_id.unwrap());
        assert_eq!(original.3, event.occurred_at);
        let payload: serde_json::Value = serde_json::from_str(&event.payload)?;
        assert_ne!(payload["postId"], event.aggregate_id.to_string());
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&original.1)?,
            serde_json::json!({"postId":payload["postId"],"payloadVersion":1})
        );
        replay
            .replay_target(event.event_id, DeliveryTarget::Internal)
            .await?;
        assert_eq!(count(&destination.pool, event.event_id).await?, 1);
        assert_eq!(
            sqlx::query_scalar::<_, Uuid>("select id from notifications where event_id=$1")
                .bind(event.event_id)
                .fetch_one(&destination.pool)
                .await?,
            original.0
        );
        assert_eq!(
            job(&post.pool, event.event_id).await?,
            ("SUCCEEDED".into(), 0, 5)
        );
        assert!(
            sqlx::query_scalar::<_, bool>(
                "select published from post_event_outbox where event_id=$1"
            )
            .bind(event.event_id)
            .fetch_one(&post.pool)
            .await?
        );
        assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_attempts where event_id=$1 and target='INTERNAL' and source='ADMIN_REPLAY' and phase='SUCCEEDED'").bind(event.event_id).fetch_one(&post.pool).await?,2);
    }

    // A real automatic claim delivers direct intent after its Post is purged.
    let c = seed(&post.pool, Uuid::new_v4(), Uuid::new_v4(), "POST_COMMENTED").await?;
    sqlx::query("delete from posts where id=$1")
        .bind(c.event.aggregate_id)
        .execute(&post.pool)
        .await?;
    ready(post, &c).await?;
    automatic(post, selected.clone()).await?;
    assert_eq!(
        job(&post.pool, c.event.event_id).await?,
        ("SUCCEEDED".into(), 2, 5)
    );
    assert_eq!(count(&destination.pool, c.event.event_id).await?, 1);

    // Actual selected membership spans two pages under the same worker fence.
    let actor = Uuid::new_v4();
    followers(&friend.pool, actor).await?;
    let c = seed(&post.pool, actor, Uuid::new_v4(), "POST_CREATED").await?;
    ready(post, &c).await?;
    automatic(post, selected.clone()).await?;
    assert_eq!(job(&post.pool, c.event.event_id).await?.0, "SUCCEEDED");
    assert_eq!(count(&destination.pool, c.event.event_id).await?, 2);

    // Lifecycle suppression precedes malformed retained payload/actor parsing.
    for kind in ["POST_CREATED", "UNRELATED_EVENT"] {
        let c = seed(&post.pool, Uuid::new_v4(), Uuid::new_v4(), kind).await?;
        ready(post, &c).await?;
        sqlx::query(
            "update post_event_outbox set payload='malformed',actor_id=null where event_id=$1",
        )
        .bind(c.event.event_id)
        .execute(&post.pool)
        .await?;
        sqlx::query("update posts set visibility='PRIVATE' where id=$1")
            .bind(c.event.aggregate_id)
            .execute(&post.pool)
            .await?;
        automatic(post, selected.clone()).await?;
        assert_eq!(job(&post.pool, c.event.event_id).await?.0, "SUCCEEDED");
        assert_eq!(count(&destination.pool, c.event.event_id).await?, 0);
    }

    // Required selected-path outages cannot fall back to the legacy dispatcher.
    for replay in [false, true] {
        let c = seed(&post.pool, Uuid::new_v4(), Uuid::new_v4(), "POST_COMMENTED").await?;
        ready(post, &c).await?;
        let before_calls = policy_calls.load(Ordering::SeqCst);
        policy.store(3, Ordering::SeqCst);
        if replay {
            assert!(post
                .fenced_replay_delivery(Default::default())?
                .with_claimed_notifications(selected.clone())
                .replay_target(c.event.event_id, DeliveryTarget::Internal)
                .await
                .is_err());
        } else {
            automatic(post, selected.clone()).await?;
        }
        policy.store(0, Ordering::SeqCst);
        assert_eq!(count(&destination.pool, c.event.event_id).await?, 0);
        assert_eq!(job(&post.pool, c.event.event_id).await?.0, "RETRY");
        assert_eq!(job(&post.pool, c.event.event_id).await?.1, 2);
        let (id, source, phases) = attempt(&post.pool, c.event.event_id).await?;
        assert_ne!(id, c.attempt_id);
        assert_eq!(
            source,
            if replay {
                "ADMIN_REPLAY"
            } else {
                "OUTBOX_WORKER"
            }
        );
        assert_eq!(phases, vec!["STARTED", "FAILED"]);
        assert!(policy_calls.load(Ordering::SeqCst) > before_calls);
        park(post, c.event.event_id).await?;
        assert_eq!(post.notification_requests().await.len(), legacy_before);
    }

    // Lost actual ownership and changed frozen budgets after a genuine page
    // must both deny the first recipient without replacing the frozen claim.
    for (mode, replay) in [
        (1, false),
        (1, true),
        (2, false),
        (2, true),
        (4, false),
        (4, true),
    ] {
        let actor = Uuid::new_v4();
        followers(&friend.pool, actor).await?;
        let c = seed(&post.pool, actor, Uuid::new_v4(), "POST_CREATED").await?;
        let fault = Arc::new(FaultTransport {
            inner: real.clone(),
            pool: post.pool.clone(),
            mode,
            pages: AtomicUsize::new(0),
            writes: AtomicUsize::new(0),
            policy: policy.clone(),
        });
        ready(post, &c).await?;
        if mode == 4 {
            short_owner.store(1, Ordering::SeqCst);
        }
        let port = delivery(post, fault.clone())?;
        if replay {
            assert!(post
                .fenced_replay_delivery(Default::default())?
                .with_claimed_notifications(port)
                .replay_target(c.event.event_id, DeliveryTarget::Internal)
                .await
                .is_err());
        } else {
            automatic(post, port).await?;
        }
        short_owner.store(0, Ordering::SeqCst);
        assert_eq!(fault.pages.load(Ordering::SeqCst), 1);
        assert_eq!(fault.writes.load(Ordering::SeqCst), 0);
        assert_eq!(count(&destination.pool, c.event.event_id).await?, 0);
        assert_eq!(job(&post.pool, c.event.event_id).await?.0, "RETRY");
        park(post, c.event.event_id).await?;
    }

    // One acknowledged real recipient followed by policy outage is uncertain:
    // keep STARTED/LEASED, never immediately retry or report no effect.
    for replay in [false, true] {
        let actor = Uuid::new_v4();
        followers(&friend.pool, actor).await?;
        let c = seed(&post.pool, actor, Uuid::new_v4(), "POST_CREATED").await?;
        let fault = Arc::new(FaultTransport {
            inner: real.clone(),
            pool: post.pool.clone(),
            mode: 3,
            pages: AtomicUsize::new(0),
            writes: AtomicUsize::new(0),
            policy: policy.clone(),
        });
        let port = delivery(post, fault.clone())?;
        ready(post, &c).await?;
        if replay {
            assert!(post
                .fenced_replay_delivery(Default::default())?
                .with_claimed_notifications(port)
                .replay_target(c.event.event_id, DeliveryTarget::Internal)
                .await
                .is_err());
        } else {
            automatic(post, port).await?;
        }
        policy.store(0, Ordering::SeqCst);
        assert_eq!(fault.writes.load(Ordering::SeqCst), 1);
        assert_eq!(count(&destination.pool, c.event.event_id).await?, 1);
        assert_eq!(job(&post.pool, c.event.event_id).await?.0, "LEASED");
        let phases: Vec<String>=sqlx::query_scalar("select phase from post_event_delivery_attempts where event_id=$1 and target='INTERNAL' and attempt_id=(select attempt_id from post_event_delivery_jobs where event_id=$1 and target='INTERNAL') order by id").bind(c.event.event_id).fetch_all(&post.pool).await?;
        assert_eq!(phases, vec!["STARTED"]);
    }

    // A local owner wait consumes the original worker effect budget. A fresh
    // transport constructor must never grant a new budget after this await.
    for replay in [false, true] {
        let c = seed(&post.pool, Uuid::new_v4(), Uuid::new_v4(), "POST_COMMENTED").await?;
        ready(post, &c).await?;
        let owner = Arc::new(DelayedOwner {
            inner: PgDeliveryAuthorityRepository::new(post.pool.clone()),
            calls: AtomicUsize::new(0),
            delay: Duration::from_secs(1),
        });
        let port = Arc::new(DeliverClaimedNotificationsUseCase::new(
            owner.clone(),
            Arc::new(PgPostRepository::new(post.pool.clone())),
            real.clone(),
            1,
        )?);
        let config = DeliveryExecutionConfigDto {
            effect_timeout_ms: 200,
            ..Default::default()
        };
        if replay {
            assert!(post
                .fenced_replay_delivery(config)?
                .with_claimed_notifications(port)
                .replay_target(c.event.event_id, DeliveryTarget::Internal)
                .await
                .is_err());
        } else {
            post.outbox_forwarder()
                .with_execution_config(config)?
                .with_claimed_notifications(port)
                .republish_target(DeliveryTarget::Internal, chrono::Utc::now(), 1)
                .await?;
        }
        assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
        assert_eq!(count(&destination.pool, c.event.event_id).await?, 0);
        assert_eq!(job(&post.pool, c.event.event_id).await?.0, "LEASED");
    }
    // Complete the remaining original direct producer and self-recipient paths.
    for replay in [false, true] {
        for same in [false, true] {
            let actor = Uuid::new_v4();
            let recipient = if same { actor } else { Uuid::new_v4() };
            let c = seed(&post.pool, actor, recipient, "POST_LIKED").await?;
            ready(post, &c).await?;
            if replay {
                post.fenced_replay_delivery(Default::default())?
                    .with_claimed_notifications(selected.clone())
                    .replay_target(c.event.event_id, DeliveryTarget::Internal)
                    .await?;
            } else {
                automatic(post, selected.clone()).await?;
            }
            assert_eq!(job(&post.pool, c.event.event_id).await?.0, "SUCCEEDED");
            assert_eq!(
                count(&destination.pool, c.event.event_id).await?,
                if same { 0 } else { 1 }
            );
        }
        let actor = Uuid::new_v4();
        let recipient = Uuid::new_v4();
        let c = seed(&post.pool, actor, recipient, "POST_COMMENTED").await?;
        ready(post, &c).await?;
        sqlx::query(
            "insert into user_mutes(muter_id,muted_id,created_at) values($1,$2,clock_timestamp())",
        )
        .bind(recipient)
        .bind(actor)
        .execute(&friend.pool)
        .await?;
        if replay {
            post.fenced_replay_delivery(Default::default())?
                .with_claimed_notifications(selected.clone())
                .replay_target(c.event.event_id, DeliveryTarget::Internal)
                .await?;
        } else {
            automatic(post, selected.clone()).await?;
        }
        assert_eq!(job(&post.pool, c.event.event_id).await?.0, "SUCCEEDED");
        assert_eq!(count(&destination.pool, c.event.event_id).await?, 0);
    }

    // Actual first destination commit with lost wire reply, malformed successful
    // reply, or an unclassified port error must never become ConfirmedNoEffect.
    for replay in [false, true] {
        for mode in [1, 2, 5] {
            let c = seed(&post.pool, Uuid::new_v4(), Uuid::new_v4(), "POST_COMMENTED").await?;
            ready(post, &c).await?;
            let before = reply_calls.load(Ordering::SeqCst);
            reply_mode.store(if mode == 5 { 0 } else { mode }, Ordering::SeqCst);
            let port = if mode == 5 {
                delivery(
                    post,
                    Arc::new(FaultTransport {
                        inner: real.clone(),
                        pool: post.pool.clone(),
                        mode,
                        pages: AtomicUsize::new(0),
                        writes: AtomicUsize::new(0),
                        policy: policy.clone(),
                    }),
                )?
            } else {
                selected.clone()
            };
            if replay {
                assert!(post
                    .fenced_replay_delivery(Default::default())?
                    .with_claimed_notifications(port)
                    .replay_target(c.event.event_id, DeliveryTarget::Internal)
                    .await
                    .is_err());
            } else {
                automatic(post, port).await?;
            }
            reply_mode.store(0, Ordering::SeqCst);
            assert_eq!(
                reply_calls.load(Ordering::SeqCst) - before,
                if mode == 5 { 0 } else { 1 }
            );
            assert_eq!(count(&destination.pool, c.event.event_id).await?, 1);
            assert_eq!(job(&post.pool, c.event.event_id).await?.0, "LEASED");
            let (id, source, phases) = attempt(&post.pool, c.event.event_id).await?;
            assert_ne!(id, c.attempt_id);
            assert_eq!(
                source,
                if replay {
                    "ADMIN_REPLAY"
                } else {
                    "OUTBOX_WORKER"
                }
            );
            assert_eq!(phases, vec!["STARTED"]);
        }
    }

    commit_proxy
        .worker_faults(post, destination, selected.clone())
        .await?;

    // Keep the original 480 ms effect deadline. Issue completes normally, then
    // the actual destination's policy refresh is held until the worker returns.
    // Compare its wire timeout with the original remaining budget, rather than
    // spending 250 ms at Issue merely to fit a fixed <200 ms assertion.
    for replay in [false, true] {
        let actor = Uuid::new_v4();
        let recipient = Uuid::new_v4();
        let c = seed(&post.pool, actor, recipient, "POST_COMMENTED").await?;
        ready(post, &c).await?;
        let armed = budget_probe.arm(c.event.event_id, actor, recipient);
        let owner = Arc::new(DelayedOwner {
            inner: PgDeliveryAuthorityRepository::new(post.pool.clone()),
            calls: AtomicUsize::new(0),
            delay: Duration::from_millis(40),
        });
        let port = Arc::new(DeliverClaimedNotificationsUseCase::new(
            owner.clone(),
            Arc::new(PgPostRepository::new(post.pool.clone())),
            Arc::new(budget::Transport {
                inner: real.clone(),
                probe: budget_probe.clone(),
            }),
            1,
        )?);
        let port = Arc::new(budget::Delivery {
            inner: port,
            probe: budget_probe.clone(),
        });
        let config = DeliveryExecutionConfigDto {
            effect_timeout_ms: 480,
            ..Default::default()
        };
        let outcome = if replay {
            post.fenced_replay_delivery(config)?
                .with_claimed_notifications(port)
                .replay_target(c.event.event_id, DeliveryTarget::Internal)
                .await
                .map(|_| ())
        } else {
            post.outbox_forwarder()
                .with_execution_config(config)?
                .with_claimed_notifications(port)
                .republish_target(DeliveryTarget::Internal, chrono::Utc::now(), 1)
                .await
                .map(|_| ())
        };
        let returned = std::time::Instant::now();
        let wall_returned = chrono::Utc::now().timestamp_micros();
        // Release and drain before assertions, including on an early error.
        // The cleanup budget is not additional time for the effect to succeed.
        let checked = armed.case.finish(returned, wall_returned).await;
        eprintln!(
            "NOTIFICATION_BUDGET_WORKER: replay={replay} operation_ok={}",
            outcome.is_ok()
        );
        checked?;
        if replay {
            assert!(outcome.is_err());
        } else {
            outcome?;
        }
        assert!(owner.calls.load(Ordering::SeqCst) >= 2);
        assert_eq!(count(&destination.pool, c.event.event_id).await?, 0);
        assert_eq!(job(&post.pool, c.event.event_id).await?.0, "LEASED");
        assert_eq!(
            attempt(&post.pool, c.event.event_id).await?.2,
            vec!["STARTED"]
        );
    }
    assert_eq!(post.notification_requests().await.len(), legacy_before);
    println!("NOTIFICATION_WORKERS_MTLS_PG: automatic + replay, actual migration16 count-zero SKIPPED/SUCCEEDED, paginated membership, suppression, no fallback, lost owner, frozen budget, partial/first-write/wire-loss/malformed-ACK uncertainty, original deadline during presenter calls");
    Ok(())
}
