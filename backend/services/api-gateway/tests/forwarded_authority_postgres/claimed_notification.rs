//! Four actual generated mTLS services and owning PostgreSQL repositories.
//! Manual presenters and both selected delivery workers exercise the network.
#[path = "claimed_notification/budget.rs"]
mod budget;
#[path = "claimed_notification/commit_ack.rs"]
mod commit_ack;
#[path = "claimed_notification/workers.rs"]
mod workers;
use super::*;
use api_gateway::adapters::{
    out::notification_authority::GrpcNotificationAuthorityLookup,
    r#in::grpc::notification_authority::{NotificationGrantGrpcServer, NOTIFICATION_GRANT_POLICY},
};
use friend_service::adapters::{
    out::{
        grpc::notification_admission::GrpcNotificationAdmission as FriendAdmission,
        persistence::follow_repository::PgFollowRepository,
    },
    r#in::grpc::{
        fanout_projection::proto as fp,
        notification_authority::{
            NotificationPolicyGrpcServer, NotificationProjectionGrpcServer,
            NOTIFICATION_POLICY_POLICY, NOTIFICATION_PROJECTION_POLICY,
        },
    },
};
use notification_service::adapters::{
    out::notification_admission::GrpcNotificationAdmission as DestinationAdmission,
    r#in::grpc::{
        method_policy::NOTIFICATION_METHOD_POLICY,
        server::{claimed_notification_service, proto as np},
    },
};
use platform::actor_delegation::notification::{self as capability, *};
use post_service::{
    adapters::{
        out::persistence::{
            delivery_authority_repository::PgDeliveryAuthorityRepository,
            delivery_schedule::PgDeliverySchedule,
            event_outbox::PgEventOutbox,
            notification_authority_repository::{
                notification_reference, PgNotificationAuthorityRepository,
            },
        },
        r#in::grpc::notification_authority::{
            page_message, NotificationOwnerGrpcServer, NOTIFICATION_OWNER_POLICY,
        },
    },
    application::{
        delivery_authority::{DeliveryAuthorityRepositoryPort, DeliveryClaimLookupDto},
        dto::delivery_schedule_dto::{DeliveryScheduleConfigDto, TargetDeliveryClaimDto},
        notification_authority::NotificationScopeDto,
        ports::{DeliverySchedulePort, EventOutboxPort},
    },
    domain::post::{DeliveryTarget, StoredEvent},
};
use std::sync::atomic::AtomicU8;

fn all_callers() -> anyhow::Result<CallerRegistry> {
    registry()?
        .register(Workload::Friend, "friend-fixture")?
        .register(Workload::Notification, "notification-fixture")
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
fn presented<T>(value: T, grant: Option<&str>) -> Request<T> {
    let mut r = identified(value, Workload::Post, "post-fixture", None);
    if let Some(grant) = grant {
        r.metadata_mut()
            .insert(capability::HEADER, grant.parse().unwrap());
    }
    r.set_timeout(Duration::from_secs(3));
    r
}
#[track_caller]
fn status<T>(r: Result<T, Status>, expected: Code) {
    match r {
        Err(e) => assert_eq!(e.code(), expected, "{e}"),
        Ok(_) => panic!("expected {expected:?}"),
    }
}

#[derive(Clone)]
struct OwnerControl {
    normal: NotificationOwnerGrpcServer,
    short: NotificationOwnerGrpcServer,
    use_short: Arc<AtomicU8>,
}
#[tonic::async_trait]
impl owner::notification_owner_authority_service_server::NotificationOwnerAuthorityService
    for OwnerControl
{
    async fn resolve_notification_source(
        &self,
        r: Request<owner::ResolveNotificationSourceRequest>,
    ) -> Result<Response<owner::ResolveNotificationSourceResponse>, Status> {
        if self.use_short.load(Ordering::SeqCst) == 0 {
            self.normal.resolve_notification_source(r).await
        } else {
            self.short.resolve_notification_source(r).await
        }
    }
}

/// Faults surround the actual owning query; no source or policy evidence is fabricated.
#[derive(Clone)]
struct PolicyControl {
    server: NotificationPolicyGrpcServer,
    mode: Arc<AtomicU8>,
    calls: Arc<AtomicUsize>,
    post_pool: PgPool,
    signer: NotificationFriendSigner,
    budget_probe: budget::Probe,
}
#[tonic::async_trait]
impl fp::notification_policy_service_server::NotificationPolicyService for PolicyControl {
    async fn check_notification_recipient(
        &self,
        r: Request<fp::CheckNotificationRecipientRequest>,
    ) -> Result<Response<fp::CheckNotificationRecipientResponse>, Status> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mode = self.mode.load(Ordering::SeqCst);
        if mode == 3 {
            return Err(Status::unavailable("injected policy transport failure"));
        }
        let tracked = self
            .signer
            .verify_policy_query(
                &r.get_ref().query,
                Workload::Gateway,
                POLICY_METHOD,
                chrono::Utc::now().timestamp_micros(),
            )
            .ok()
            .and_then(|q| self.budget_probe.policy(q.actor_id(), q.recipient_id()));
        let refreshing = tracked.as_ref().is_some_and(|c| c.refreshing());
        let phase = tracked.as_ref().map(|c| {
            c.enter(if refreshing {
                "refresh_policy"
            } else {
                "issue_policy"
            })
        });
        let result = {
            let query_phase = tracked
                .as_ref()
                .filter(|_| refreshing)
                .map(|c| c.enter("refresh_query"));
            let result = self.server.check_notification_recipient(r).await;
            if let Some(p) = &query_phase {
                p.complete(result.is_ok());
            }
            result
        };
        if refreshing && result.is_ok() {
            tracked.as_ref().unwrap().hold_refresh().await;
        }
        if let Some(p) = &phase {
            p.complete(result.is_ok());
        }
        let response = result?;
        if mode == 1 || mode == 2 {
            sqlx::query("update posts set visibility=$1")
                .bind(if mode == 1 { "PRIVATE" } else { "FRIENDS" })
                .execute(&self.post_pool)
                .await
                .map_err(|_| Status::unavailable("fault injection failed"))?;
        }
        if mode == 4 {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        Ok(response)
    }
}
#[derive(Clone)]
struct ProjectionControl {
    server: NotificationProjectionGrpcServer,
    mutate: Arc<AtomicU8>,
}
#[tonic::async_trait]
impl fp::notification_projection_service_server::NotificationProjectionService
    for ProjectionControl
{
    async fn list_notification_followers(
        &self,
        mut r: Request<fp::ListNotificationFollowersRequest>,
    ) -> Result<Response<fp::ListNotificationFollowersResponse>, Status> {
        if self.mutate.load(Ordering::SeqCst) != 0 {
            r.get_mut().limit += 1;
        }
        self.server.list_notification_followers(r).await
    }
}

async fn seed(
    pool: &PgPool,
    actor: Uuid,
    recipient: Uuid,
    kind: &str,
) -> anyhow::Result<TargetDeliveryClaimDto> {
    let post = Uuid::new_v4();
    sqlx::query("insert into posts(id,author_id,content,visibility,created_at) values($1,$2,'private source text','PUBLIC',clock_timestamp())")
        .bind(post).bind(if kind == "POST_CREATED" {actor} else {recipient}).execute(pool).await?;
    let now = chrono::Utc::now() - chrono::Duration::seconds(1);
    let event = StoredEvent {event_id: Uuid::new_v4(), event_type: kind.into(), aggregate_id: post,
        aggregate_version: Some(1), actor_id: Some(actor), attributes: vec!["network fixture".into()],
        payload: serde_json::json!({"postId":post,"authorId":if kind=="POST_LIKED" {recipient} else {actor},"actorId":actor,"notifyUserId":recipient,"content":"must not copy"}).to_string(),
        occurred_at: now, created_at: now, published: false, published_at: None, replay_resolution_id: None,
        origin_traceparent: Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".into())};
    PgEventOutbox::new(pool.clone()).save(&event).await?;
    let mut batch = PgDeliverySchedule::new(pool.clone())
        .claim_target(
            DeliveryTarget::Internal,
            chrono::Utc::now(),
            1,
            &DeliveryScheduleConfigDto {
                lease_duration_ms: 60_000,
                ..Default::default()
            },
        )
        .await?;
    anyhow::ensure!(
        batch.claims.len() == 1 && batch.claims[0].event.event_id == event.event_id,
        "expected the seeded INTERNAL claim"
    );
    Ok(batch.claims.remove(0))
}
async fn claim(
    pool: &PgPool,
    c: &TargetDeliveryClaimDto,
    scope: NotificationScopeDto,
) -> anyhow::Result<NotificationClaim> {
    let owned = PgDeliveryAuthorityRepository::new(pool.clone())
        .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(c.clone())?)
        .await?;
    let r = notification_reference(&owned, scope)?;
    Ok(NotificationClaim {
        event_id: r.event_id,
        aggregate_id: r.aggregate_id,
        event_digest: NotificationClaim::encode_digest(&r.event_digest),
        attempt_digest: NotificationClaim::encode_digest(&r.attempt_digest),
        lease_digest: NotificationClaim::digest(r.lease_token.as_bytes()),
        attempt_id: r.attempt_id,
        attempt: r.attempt_number,
        lease_expires_micros: r.lease_expires_at.timestamp_micros(),
    })
}
async fn page(pool: &PgPool, c: &TargetDeliveryClaimDto) -> anyhow::Result<NotificationPage> {
    let author_id = c.event.actor_id.unwrap();
    Ok(NotificationPage {
        claim: claim(
            pool,
            c,
            NotificationScopeDto::FollowerPage {
                author_id,
                cursor: None,
                limit: 2,
            },
        )
        .await?,
        author_id,
        cursor: None,
        limit: 2,
    })
}
fn delivery(
    c: &TargetDeliveryClaimDto,
    claim: NotificationClaim,
    recipient_id: Uuid,
    membership: Option<String>,
) -> NotificationDelivery {
    NotificationDelivery {
        claim,
        recipient_id,
        actor_id: c.event.actor_id.unwrap(),
        post_id: c.event.aggregate_id,
        kind: if c.event.event_type == "POST_CREATED" {
            NotificationKind::Created
        } else {
            NotificationKind::Commented
        },
        occurred_at: c.event.occurred_at.to_rfc3339(),
        membership,
    }
}
fn create(d: &NotificationDelivery) -> anyhow::Result<np::CreateNotificationRequest> {
    let body = d.frame().map_err(capability::status)?;
    Ok(np::CreateNotificationRequest::decode(&body[5..])?)
}
type Issuer = authority::notification_grant_service_client::NotificationGrantServiceClient<Channel>;
async fn issue(
    client: &mut Issuer,
    b: &NotificationBinding,
    deadline: Option<i64>,
) -> Result<authority::issue_notification_grant_response::Result, Status> {
    let r = authority::IssueNotificationGrantRequest {
        binding: b.encode().map_err(capability::status)?,
        framed_body: b.frame().map_err(capability::status)?,
        deadline_micros: deadline
            .unwrap_or_else(|| chrono::Utc::now().timestamp_micros() + 30_000_000),
    };
    client
        .issue_notification_grant(presented(r, None))
        .await?
        .into_inner()
        .result
        .ok_or_else(|| Status::internal("missing result"))
}
async fn granted(
    client: &mut Issuer,
    b: &NotificationBinding,
) -> anyhow::Result<authority::NotificationGranted> {
    match issue(client, b, None).await? {
        authority::issue_notification_grant_response::Result::Granted(g) => Ok(g),
        _ => anyhow::bail!("expected grant"),
    }
}
async fn suppressed(client: &mut Issuer, b: &NotificationBinding) -> anyhow::Result<()> {
    anyhow::ensure!(
        matches!(
            issue(client, b, None).await?,
            authority::issue_notification_grant_response::Result::Suppressed(_)
        ),
        "expected suppression"
    );
    Ok(())
}

// Assertions must not unwind past fixtures that own reactor-bound pools.
// Catch the test body so listener shutdown and pool cleanup remain asynchronous.
async fn checked_body<F: Future>(body: F) -> anyhow::Result<F::Output> {
    let mut body = Box::pin(body);
    std::future::poll_fn(move |cx| {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body.as_mut().poll(cx))) {
            Ok(poll) => poll.map(Ok),
            Err(_) => Poll::Ready(Err(anyhow::anyhow!(
                "network assertion failed; see panic output"
            ))),
        }
    })
    .await
}

#[tokio::test]
async fn claimed_notification_uses_actual_four_service_mtls_owners_and_destination(
) -> anyhow::Result<()> {
    let legacy = workers::legacy_events();
    let post_app = post_support::start_postgres_redis_app_with_legacy_events(
        false,
        5,
        Duration::from_secs(30),
        &legacy,
    )
    .await?;
    let friend_app = friend_support::start_postgres_app(false).await?;
    let notification_app = notification_support::start_postgres_app().await?;
    let mut commit_proxy = commit_ack::CommitAckProxy::start(&notification_app.pool).await?;
    let post_tls = tls(Workload::Post)?;
    let gateway_tls = tls(Workload::Gateway)?;
    let friend_tls = tls(Workload::Friend)?;
    let notification_tls = tls(Workload::Notification)?;
    let lifetime = LifetimePolicy::new(30_000_000).map_err(capability::status)?;
    let owner_signer = NotificationOwnerSigner::new(&[81; 32], lifetime)?;
    let provisional = NotificationIssuer::new(
        &[82; 32],
        &[owner_signer.public_key()],
        &[owner_signer.public_key()],
        lifetime,
    )?;
    let friend_signer =
        NotificationFriendSigner::new(&[83; 32], &[provisional.public_key()], lifetime)?;
    let issuer = NotificationIssuer::new(
        &[82; 32],
        &[owner_signer.public_key()],
        &[friend_signer.public_key()],
        lifetime,
    )?;
    let key = issuer.public_key();
    let friend_key = friend_signer.public_key();
    let pl = TcpListener::bind("127.0.0.1:0").await?;
    let pu = format!("https://{}", pl.local_addr()?);
    let gl = TcpListener::bind("127.0.0.1:0").await?;
    let gu = format!("https://{}", gl.local_addr()?);
    let fl = TcpListener::bind("127.0.0.1:0").await?;
    let fu = format!("https://{}", fl.local_addr()?);
    let nl = TcpListener::bind("127.0.0.1:0").await?;
    let nu = format!("https://{}", nl.local_addr()?);
    let owner_pool = PgPool::connect_with(post_app.pool.connect_options().as_ref().clone()).await?;
    let short_owner = Arc::new(AtomicU8::new(0));
    let owner_repository = Arc::new(PgNotificationAuthorityRepository::new(
        owner_pool.clone(),
        2,
    )?);

    let (stop, stopped) = tokio::sync::oneshot::channel();
    let ps=ServerTask::spawn(Server::builder().tls_config(post_tls.server_config()?)?
        .add_service(guard_without_delegation(&NOTIFICATION_OWNER_POLICY,
            owner::notification_owner_authority_service_server::NotificationOwnerAuthorityServiceServer::new(
                OwnerControl {
                    normal: NotificationOwnerGrpcServer::new(owner_repository.clone(), owner_signer),
                    short: NotificationOwnerGrpcServer::new(owner_repository,
                        NotificationOwnerSigner::new(&[81; 32], LifetimePolicy::new(500_000).map_err(capability::status)?)?),
                    use_short: short_owner.clone(),
                }),all_callers()?)?)
        .serve_with_incoming_shutdown(TcpListenerStream::new(pl),async {let _=stopped.await;}),stop);
    let lookup = Arc::new(GrpcNotificationAuthorityLookup::connect(
        &pu,
        &fu,
        "gateway-fixture",
        &gateway_tls,
        [
            budget("notification owner fixture"),
            budget("notification policy fixture"),
        ],
    )?);
    let budget_probe = budget::Probe::default();
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let gs = ServerTask::spawn(
        Server::builder()
            .tls_config(gateway_tls.server_config()?)?
            .add_service(guard_without_delegation(
                &NOTIFICATION_GRANT_POLICY,
                authority::notification_grant_service_server::NotificationGrantServiceServer::new(
                    budget::GrantServer {
                        probe: budget_probe.clone(),
                        inner: NotificationGrantGrpcServer::new(
                            issuer,
                            NotificationVerifier::new(&[key], 128)?,
                            lookup.clone(),
                            lookup,
                            Duration::from_secs(10),
                        )?,
                    },
                ),
                all_callers()?,
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(gl), async {
                let _ = stopped.await;
            }),
        stop,
    );
    let policy_mode = Arc::new(AtomicU8::new(0));
    let policy_calls = Arc::new(AtomicUsize::new(0));
    let mutate = Arc::new(AtomicU8::new(0));
    let repo = Arc::new(PgFollowRepository::new(friend_app.pool.clone()));
    let projection = NotificationGuard::new(
        fp::notification_projection_service_server::NotificationProjectionServiceServer::new(
            ProjectionControl {
                server: NotificationProjectionGrpcServer::new(repo.clone(), friend_signer.clone()),
                mutate: mutate.clone(),
            },
        ),
        NotificationVerifier::new(&[key], 128)?,
        Arc::new(FriendAdmission::connect(
            &gu,
            "friend-fixture",
            &friend_tls,
            budget("notification page admission fixture"),
        )?),
    )?;
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let fs = ServerTask::spawn(
        Server::builder()
            .tls_config(friend_tls.server_config()?)?
            .add_service(guard_without_delegation(
                &NOTIFICATION_POLICY_POLICY,
                fp::notification_policy_service_server::NotificationPolicyServiceServer::new(
                    PolicyControl {
                        server: NotificationPolicyGrpcServer::new(repo, friend_signer.clone()),
                        signer: friend_signer,
                        budget_probe: budget_probe.clone(),
                        mode: policy_mode.clone(),
                        calls: policy_calls.clone(),
                        post_pool: post_app.pool.clone(),
                    },
                ),
                all_callers()?,
            )?)
            .add_service(guard_without_delegation(
                &NOTIFICATION_PROJECTION_POLICY,
                projection,
                all_callers()?,
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(fl), async {
                let _ = stopped.await;
            }),
        stop,
    );
    let destination = claimed_notification_service(
        commit_proxy.destination_state(&notification_app.state),
        NotificationVerifier::new(&[key], 128)?,
        Arc::new(DestinationAdmission::connect(
            &gu,
            "notification-fixture",
            &notification_tls,
            budget("notification destination admission fixture"),
        )?),
    )?;
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let reply_mode = Arc::new(AtomicU8::new(0));
    let reply_calls = Arc::new(AtomicUsize::new(0));
    let ns = ServerTask::spawn(
        Server::builder()
            .tls_config(notification_tls.server_config()?)?
            .add_service(guard(
                &NOTIFICATION_METHOD_POLICY,
                workers::ReplyFault::new(
                    destination,
                    reply_mode.clone(),
                    reply_calls.clone(),
                    budget_probe.clone(),
                ),
                all_callers()?,
                ActorDelegationVerifier::local_parity(Workload::Notification),
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(nl), async {
                let _ = stopped.await;
            }),
        stop,
    );
    let outcome=checked_body(async {
        let mut ic=Issuer::new(post_tls.connect(&gu,Workload::Gateway).await?);
        let mut pc=authority::notification_projection_service_client::NotificationProjectionServiceClient::new(post_tls.connect(&fu,Workload::Friend).await?);
        let mut nc=np::notification_service_client::NotificationServiceClient::new(post_tls.connect(&nu,Workload::Notification).await?);
        let actor=Uuid::new_v4();let recipient=Uuid::new_v4();let outsider=Uuid::new_v4();
        sqlx::query("insert into follows(follower_id,followee_id,created_at) values($1,$2,clock_timestamp())")
            .bind(recipient).bind(actor).execute(&friend_app.pool).await?;
        let c=seed(&post_app.pool,actor,recipient,"POST_CREATED").await?;
        let p=page(&post_app.pool,&c).await?;let b=NotificationBinding::FollowerPage(p.clone());
        let m=authority::ListNotificationFollowersRequest::decode(page_message(&p).map_err(capability::status)?.encode_to_vec().as_slice())?;
        let bare=NotificationProjectionGrpcServer::new(Arc::new(PgFollowRepository::new(friend_app.pool.clone())),
            NotificationFriendSigner::new(&[83;32],&[key],lifetime)?);
        let mut unsealed=Request::new(fp::ListNotificationFollowersRequest::decode(m.encode_to_vec().as_slice())?);
        unsealed.extensions_mut().insert(platform::grpc_auth::VerifiedCaller(Workload::Post));
        status(fp::notification_projection_service_server::NotificationProjectionService::list_notification_followers(&bare,unsealed).await,Code::PermissionDenied);
        status(pc.list_notification_followers(presented(m.clone(),None)).await,Code::PermissionDenied);
        let pg=granted(&mut ic,&b).await?;
        let response=pc.list_notification_followers(presented(m.clone(),Some(&pg.token))).await?.into_inner();
        let selected=match response.result {Some(authority::list_notification_followers_response::Result::Page(p))=>p,_=>anyhow::bail!("expected selected page")};
        assert_eq!(selected.follower_ids,vec![recipient.to_string()]);
        let exp=NotificationMembershipVerifier::new(&[friend_key])?.verify_page_expiry(&p,&[recipient],None,&selected.membership,chrono::Utc::now().timestamp_micros()).map_err(capability::status)?;
        assert_eq!(exp,selected.expires_micros);assert!(exp<=pg.expires_micros);
        status(pc.list_notification_followers(presented(m.clone(),Some(&pg.token))).await,Code::PermissionDenied);
        let pg=granted(&mut ic,&b).await?;mutate.store(1,Ordering::SeqCst);
        status(pc.list_notification_followers(presented(m.clone(),Some(&pg.token))).await,Code::PermissionDenied);mutate.store(0,Ordering::SeqCst);
        let d=delivery(&c,p.claim.clone(),recipient,Some(selected.membership.clone()));let db=NotificationBinding::Delivery(d.clone());
        let arbitrary=NotificationBinding::Delivery(delivery(&c,p.claim.clone(),outsider,Some(selected.membership.clone())));
        status(issue(&mut ic,&arbitrary,None).await,Code::PermissionDenied);
        // PUBLIC selected membership remains valid after unfollow; FRIENDS requires a current follow.
        sqlx::query("update follows set deleted_at=clock_timestamp() where follower_id=$1").bind(recipient).execute(&friend_app.pool).await?;
        let dg=granted(&mut ic,&db).await?;
        status(nc.create_notification(presented(create(&d)?,None)).await,Code::PermissionDenied);
        let created=nc.create_notification(presented(create(&d)?,Some(&dg.token))).await?.into_inner();
        assert!(!created.suppressed);let original=created.notification.unwrap();let id=original.id.clone();
        status(nc.create_notification(presented(create(&d)?,Some(&dg.token))).await,Code::PermissionDenied);
        let fresh=granted(&mut ic,&db).await?;
        let existing=nc.create_notification(presented(create(&d)?,Some(&fresh.token))).await?.into_inner();
        assert!(!existing.suppressed);assert_eq!(existing.notification.unwrap(),original);
        let row:(String,String,chrono::DateTime<chrono::Utc>)=sqlx::query_as("select type,payload_json,occurred_at from notifications where id=$1").bind(Uuid::parse_str(&id)?).fetch_one(&notification_app.pool).await?;
        assert_eq!(row.0,"POST_CREATED");assert_eq!(row.2,c.event.occurred_at);
        assert_eq!(serde_json::from_str::<serde_json::Value>(&row.1)?,serde_json::json!({"postId":c.event.aggregate_id,"payloadVersion":1}));
        sqlx::query("update posts set visibility='FRIENDS' where id=$1").bind(c.event.aggregate_id).execute(&post_app.pool).await?;
        suppressed(&mut ic,&db).await?;
        sqlx::query("update posts set visibility='PUBLIC' where id=$1").bind(c.event.aggregate_id).execute(&post_app.pool).await?;
        // Each block direction and recipient mute are owning policy suppression, not transport errors.
        for (table,a_col,b_col,a,z) in [("user_blocks","blocker_id","blocked_id",actor,recipient),("user_blocks","blocker_id","blocked_id",recipient,actor),("user_mutes","muter_id","muted_id",recipient,actor)] {
            sqlx::query(&format!("insert into {table}({a_col},{b_col},created_at) values($1,$2,clock_timestamp())")).bind(a).bind(z).execute(&friend_app.pool).await?;
            suppressed(&mut ic,&db).await?;
            sqlx::query(&format!("delete from {table} where {a_col}=$1 and {b_col}=$2")).bind(a).bind(z).execute(&friend_app.pool).await?;
        }
        // A post-policy private transition is discovered by the final owning challenge.
        policy_mode.store(1,Ordering::SeqCst);suppressed(&mut ic,&db).await?;policy_mode.store(0,Ordering::SeqCst);
        let before=policy_calls.load(Ordering::SeqCst);
        suppressed(&mut ic,&db).await?;suppressed(&mut ic,&b).await?;
        assert_eq!(policy_calls.load(Ordering::SeqCst),before,"inactive source skips Friend policy");
        // A live original page grant is suppressed on refresh without querying follows.
        sqlx::query("update posts set visibility='PUBLIC' where id=$1").bind(c.event.aggregate_id).execute(&post_app.pool).await?;
        let pg=granted(&mut ic,&b).await?;
        sqlx::query("update posts set visibility='PRIVATE' where id=$1").bind(c.event.aggregate_id).execute(&post_app.pool).await?;
        let mut lock=friend_app.pool.begin().await?;sqlx::query("lock follows in access exclusive mode").execute(&mut *lock).await?;
        let response=pc.list_notification_followers(presented(m.clone(),Some(&pg.token))).await?.into_inner();
        assert!(matches!(response.result,Some(authority::list_notification_followers_response::Result::Suppressed(_))));
        lock.rollback().await?;
        sqlx::query("update posts set visibility='PUBLIC' where id=$1").bind(c.event.aggregate_id).execute(&post_app.pool).await?;
        policy_mode.store(2,Ordering::SeqCst);
        status(issue(&mut ic,&db,None).await,Code::PermissionDenied);
        policy_mode.store(0,Ordering::SeqCst);
        sqlx::query("update posts set visibility='PUBLIC' where id=$1").bind(c.event.aggregate_id).execute(&post_app.pool).await?;
        policy_mode.store(3,Ordering::SeqCst);status(issue(&mut ic,&db,None).await,Code::Unavailable);
        policy_mode.store(4,Ordering::SeqCst);
        let start=tokio::time::Instant::now();
        let error=issue(&mut ic,&db,Some(chrono::Utc::now().timestamp_micros()+100_000)).await.expect_err("budget must stop policy wait");
        // Tonic peer cancellation can win the local deadline timer; both are failures.
        assert!(matches!(error.code(),Code::DeadlineExceeded|Code::Unavailable),"{error}");
        assert!(start.elapsed()<Duration::from_millis(240),"original budget reset across awaits");
        policy_mode.store(0,Ordering::SeqCst);
        let cutoff=chrono::Utc::now().timestamp_micros()+1_000_000;
        let bounded=match issue(&mut ic,&b,Some(cutoff)).await? {authority::issue_notification_grant_response::Result::Granted(g)=>g,_=>anyhow::bail!("expected bounded grant")};
        assert!(bounded.expires_micros<=cutoff);
        // Validation is bound to the exact destination audience; Gateway never consumes it.
        let vr=authority::ValidateNotificationGrantRequest {binding:db.encode().map_err(capability::status)?,framed_body:db.frame().map_err(capability::status)?,token:dg.token.clone(),deadline_micros:dg.expires_micros};
        let mut wrong=Issuer::new(friend_tls.connect(&gu,Workload::Gateway).await?);
        status(wrong.validate_notification_grant(identified(vr.clone(),Workload::Friend,"friend-fixture",None)).await,Code::PermissionDenied);
        status(ic.validate_notification_grant(identified(vr,Workload::Notification,"notification-fixture",None)).await,Code::Unauthenticated);
        // Direct intent survives a purged Post and retains exact stored recipient/actor/time/type.
        let dc=seed(&post_app.pool,actor,recipient,"POST_COMMENTED").await?;
        let raw=claim(&post_app.pool,&dc,NotificationScopeDto::RecipientPolicy {recipient_id:recipient}).await?;
        let direct=delivery(&dc,raw,recipient,None);let direct_b=NotificationBinding::Delivery(direct.clone());
        sqlx::query("delete from posts where id=$1").bind(dc.event.aggregate_id).execute(&post_app.pool).await?;
        for field in 0..4 {
            let mut forged=direct.clone();
            match field {0=>forged.recipient_id=outsider,1=>forged.actor_id=outsider,2=>forged.kind=NotificationKind::Liked,_=>forged.occurred_at=chrono::Utc::now().to_rfc3339()}
            status(issue(&mut ic,&NotificationBinding::Delivery(forged),None).await,Code::PermissionDenied);
        }
        let g=granted(&mut ic,&direct_b).await?;
        let mut mutated=create(&direct)?;mutated.user_id=outsider.to_string();
        status(nc.create_notification(presented(mutated,Some(&g.token))).await,Code::PermissionDenied);
        let result=nc.create_notification(presented(create(&direct)?,Some(&g.token))).await?.into_inner();
        assert!(!result.suppressed);assert_eq!(result.notification.unwrap().event_id,dc.event.event_id.to_string());
        let g=granted(&mut ic,&direct_b).await?;
        sqlx::query("insert into user_mutes(muter_id,muted_id,created_at) values($1,$2,clock_timestamp())").bind(recipient).bind(actor).execute(&friend_app.pool).await?;
        let result=nc.create_notification(presented(create(&direct)?,Some(&g.token))).await?.into_inner();
        assert!(result.suppressed);assert!(result.notification.is_none());
        sqlx::query("delete from user_mutes where muter_id=$1 and muted_id=$2").bind(recipient).bind(actor).execute(&friend_app.pool).await?;
        let count:i64=sqlx::query_scalar("select count(*) from notifications").fetch_one(&notification_app.pool).await?;
        assert_eq!(count,2,"suppression/replay/forgery must not add rows");
        // Both independent clients carry a shorter refreshed owner lifetime.
        let pg=granted(&mut ic,&b).await?;short_owner.store(1,Ordering::SeqCst);
        let selected=pc.list_notification_followers(presented(m.clone(),Some(&pg.token))).await?.into_inner();
        let selected=match selected.result {Some(authority::list_notification_followers_response::Result::Page(p))=>p,_=>anyhow::bail!("expected short page")};
        assert!(selected.expires_micros<pg.expires_micros);
        assert!(selected.expires_micros<=chrono::Utc::now().timestamp_micros()+500_000);
        let short_ids=selected.follower_ids.iter().map(|s|Uuid::parse_str(s)).collect::<Result<Vec<_>,_>>()?;
        assert_eq!(NotificationMembershipVerifier::new(&[friend_key])?.verify_page_expiry(&p,&short_ids,selected.next_cursor.as_deref(),&selected.membership,chrono::Utc::now().timestamp_micros()).map_err(capability::status)?,selected.expires_micros);
        short_owner.store(0,Ordering::SeqCst);
        let waiting_actor=Uuid::new_v4();let waiting_recipient=Uuid::new_v4();
        let wc=seed(&post_app.pool,waiting_actor,waiting_recipient,"POST_COMMENTED").await?;
        let wclaim=claim(&post_app.pool,&wc,NotificationScopeDto::RecipientPolicy {recipient_id:waiting_recipient}).await?;
        let wd=delivery(&wc,wclaim,waiting_recipient,None);let wb=NotificationBinding::Delivery(wd.clone());
        let wg=granted(&mut ic,&wb).await?;
        short_owner.store(1,Ordering::SeqCst);
        let proof=NotificationVerifier::new(&[key],8)?.verify(&wg.token,Workload::Post,WRITE_METHOD,&wd.frame().map_err(capability::status)?,chrono::Utc::now().timestamp_micros()).map_err(capability::status)?;
        let admission=DestinationAdmission::connect(&gu,"notification-fixture",&notification_tls,budget("short destination fixture"))?;
        let refreshed=admission.check(&proof).await.map_err(capability::status)?;
        assert!(refreshed.expires_micros<wg.expires_micros);
        assert!(refreshed.expires_micros<=chrono::Utc::now().timestamp_micros()+500_000);
        for subject in [waiting_recipient,waiting_actor] {
            sqlx::query("insert into notification_subject_states(subject_id,created_at) values($1,clock_timestamp())").bind(subject).execute(&notification_app.pool).await?;
        }
        let mut row_lock=notification_app.pool.begin().await?;
        sqlx::query("select subject_id from notification_subject_states where subject_id=$1 for update").bind(waiting_recipient).execute(&mut *row_lock).await?;
        let request=presented(create(&wd)?,Some(&wg.token));let mut waiting_client=nc.clone();
        let pending=tokio::spawn(async move {waiting_client.create_notification(request).await});
        let mut observed=false;
        for _ in 0..100 {
            observed=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and wait_event_type='Lock' and query like '%notification_subject_states%')").fetch_one(&notification_app.pool).await?;
            if observed {break;}
            anyhow::ensure!(!pending.is_finished(),"write finished without observed subject wait");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        anyhow::ensure!(observed,"owning subject wait was not observed");
        tokio::time::sleep(Duration::from_millis(650)).await;
        row_lock.rollback().await?;
        status(pending.await?,Code::PermissionDenied);
        assert!(chrono::Utc::now().timestamp_micros()<wg.expires_micros);
        assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from notifications where event_id=$1").bind(wc.event.event_id).fetch_one(&notification_app.pool).await?,0);
        short_owner.store(0,Ordering::SeqCst);
        commit_proxy.repository_faults(&notification_app.pool).await?;
        workers::exercise(&post_app,&friend_app,&notification_app,&legacy,
            [&gu,&fu,&nu],&post_tls,key,friend_key,policy_mode.clone(),policy_calls.clone(),
            short_owner.clone(),reply_mode.clone(),reply_calls.clone(),budget_probe.clone(),&mut commit_proxy).await?;
        // Actual owning storage loss is unavailable, never a successful no-write decision.
        let g=granted(&mut ic,&direct_b).await?;owner_pool.close().await;
        status(issue(&mut ic,&direct_b,None).await,Code::Unavailable);
        status(nc.create_notification(presented(create(&direct)?,Some(&g.token))).await,Code::Unavailable);
        Ok::<_,anyhow::Error>(())
    }).await.and_then(|outcome| outcome);
    if let Err(e) = &outcome {
        eprintln!("NOTIFICATION_NETWORK_PRIMARY_ERROR: {e:#}");
    }
    owner_pool.close().await;
    let mut failures = Vec::new();
    if let Err(error) = outcome {
        failures.push(format!("network: {error:#}"));
    }
    // Every teardown runs even if another listener or fixture reports an error.
    for (name, result) in [
        ("Post listener", ps.stop().await),
        ("Gateway listener", gs.stop().await),
        ("Friend listener", fs.stop().await),
        ("Notification listener", ns.stop().await),
        ("Notification COMMIT proxy", commit_proxy.shutdown().await),
        ("Friend fixture", friend_app.shutdown().await),
        ("Notification fixture", notification_app.shutdown().await),
    ] {
        if let Err(error) = result {
            failures.push(format!("{name}: {error:#}"));
        }
    }
    anyhow::ensure!(failures.is_empty(), "{}", failures.join("; "));
    println!("NOTIFICATION_FOUR_SERVICE_MTLS_PG: actual ownership, selected membership, current policy, immutable destination, one-use replay; manual presenter and selected automatic/replay workers");
    Ok(())
}
