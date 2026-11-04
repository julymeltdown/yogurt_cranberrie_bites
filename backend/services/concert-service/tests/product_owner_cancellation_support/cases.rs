//! Actual guarded Product calls; no public constructor for verified authority.
use super::{capture, environment, fixture::Fixture};
use concert_service::{
    adapters::out::relationship_authority::guarded::{
        OwnerGuardScope, RootReadPermit, ScopeRegistry,
    },
    product_rpc::{
        delivery::{DeliveryPublisher, DeliveryService, POLICY as DELIVERY_POLICY},
        proto,
        scoped::{ProductRepresentation, ScopedProductOwner, ScopedProductProducer},
        Operation, ProductOwner, ProductRpcService, VerifiedProduct, POLICY,
    },
};
use http_body_util::BodyExt;
use platform::{
    actor_delegation::{
        exchange::VerifiedParentAuthority, response_scope::RetainedScopeLayer,
        ActorDelegationVerifier, HEADER,
    },
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::{guard, guard_without_delegation},
};
use prost::Message;
use sqlx::PgPool;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{mpsc, Notify};
use tonic::Status;
use tower::{Layer, ServiceExt};
use uuid::Uuid;

const METHOD: &str = "/concert.product.v1.ProductService/Dispatch";
#[derive(Clone, Copy)]
enum Mode {
    Pending,
    DestructorPanic,
    Callback,
    CommittedUnknown,
    Success,
}
struct Producer {
    mode: Mode,
    pool: PgPool,
    registry: Arc<ScopeRegistry>,
    entered: mpsc::UnboundedSender<Arc<OwnerGuardScope>>,
    dropped: Arc<Notify>,
    callback: Mutex<Option<RootReadPermit>>,
    original_deadline:
        Mutex<Option<platform::actor_delegation::authority_deadline::AuthorityRpcDeadline>>,
}
struct ActualWorkDrop(Arc<Notify>, bool);
impl Drop for ActualWorkDrop {
    fn drop(&mut self) {
        self.0.notify_one();
        assert!(!self.1, "actual product destructor fault");
    }
}
#[async_trait::async_trait]
impl ScopedProductProducer for Producer {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        request: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        let _actual_work = ActualWorkDrop(
            self.dropped.clone(),
            matches!(self.mode, Mode::DestructorPanic),
        );
        // The supervisor's spawned task must preserve the original authenticated
        // parent and both deadline clocks, including nested adapter propagation.
        let parent =
            VerifiedParentAuthority::current().expect("original parent propagated into owned task");
        assert_eq!(
            parent.parent_digest(),
            context.parent_authority().parent_digest()
        );
        assert_eq!(parent.message().unwrap(), request.encode_to_vec());
        assert_eq!(context.actor().to_string(), request.user_id);
        assert_eq!(context.event().to_string(), request.event_id);
        assert_eq!(context.session().to_string(), request.session_family_id);
        assert_eq!(context.request().to_string(), request.request_id);
        assert_eq!(
            context.deadline().expires_micros(),
            request.authority_expires_unix_micros
        );
        let original = self
            .original_deadline
            .lock()
            .unwrap()
            .expect("actual pre-copy deadline captured");
        assert_eq!(
            context.deadline().stop(),
            original.stop(),
            "owned context preserves exact original monotonic stop"
        );
        assert_eq!(
            context.deadline().expires_micros(),
            original.expires_micros()
        );
        assert!(
            scope.binding().deadline().stop() <= original.stop(),
            "guard construction may only shorten the original stop"
        );
        assert_eq!(
            platform::call_deadline::current(),
            Some(scope.binding().deadline().stop().into_std())
        );
        assert!(context.trusted_pickup_context().is_some());
        match context.operation() {
            Operation::Feed => {
                let query = context.feed_query().expect("owned feed query");
                assert_eq!(query.limit, Some(7));
                assert_eq!(query.cursor.as_deref(), Some("feed_cursor"));
                assert_eq!(query.filter, Some(concert_domain::feed::Filter::WithMedia));
                assert_eq!(query.before.unwrap().timestamp(), 1_700_000_000);
                assert!(context.comments_query().is_none() && context.command_query().is_none());
            }
            Operation::Comments(_) => {
                let query = context.comments_query().expect("owned comment query");
                assert_eq!(query.limit, Some(9));
                assert_eq!(query.cursor.as_deref(), Some("comments_cursor"));
                assert!(context.feed_query().is_none() && context.command_query().is_none());
            }
            Operation::CommandStatus => {
                let query = context.command_query().expect("owned command query");
                assert!(request.http_target.contains(&query.key.to_string()));
                assert!(request.http_target.contains(query.operation.id()));
                assert!(context.feed_query().is_none() && context.comments_query().is_none());
            }
            _ => panic!("unexpected request"),
        }
        let key = scope.producer_scope();
        let published:bool=sqlx::query_scalar("select delivery_secret_hash is not null from concert_product_owner_scopes where instance_id=$1 and scope_id=$2")
            .bind(Uuid::parse_str(&key.instance_id).unwrap()).bind(Uuid::parse_str(&key.scope_id).unwrap()).fetch_one(&self.pool).await.map_err(|_|Status::unavailable("fixture ledger"))?;
        assert!(published, "actual producer must follow durable publication");
        if matches!(self.mode, Mode::Callback) {
            *self.callback.lock().unwrap() =
                Some(self.registry.lookup_root_read(&scope.root_read_view())?);
        }
        if matches!(self.mode, Mode::CommittedUnknown) {
            let mut native = self
                .pool
                .begin()
                .await
                .map_err(|_| Status::unavailable("fixture transaction"))?;
            scope
                .register_transaction_in(&mut native, [97; 32], Uuid::new_v4().as_bytes().to_vec())
                .await?;
            native
                .commit()
                .await
                .map_err(|_| Status::unavailable("fixture commit"))?;
        }
        self.entered
            .send(scope)
            .map_err(|_| Status::unavailable("fixture observer"))?;
        if matches!(self.mode, Mode::Success) {
            return Ok(ProductRepresentation {
                status: 200,
                content_type: "application/json".into(),
                body: br#"{"ok":true}"#.to_vec(),
            });
        }
        std::future::pending().await
    }
}
fn frame(message: &impl Message) -> Vec<u8> {
    let bytes = message.encode_to_vec();
    let mut f = vec![0];
    f.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    f.extend(bytes);
    f
}
fn request(method: &str, bytes: Vec<u8>) -> anyhow::Result<axum::http::Request<axum::body::Body>> {
    Ok(axum::http::Request::builder()
        .method("POST")
        .uri(method)
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .header("x-service-caller", "gateway")
        .header("x-service-token", "gateway-fixture")
        .header("grpc-timeout", "1800000u")
        .body(axum::body::Body::from(bytes))?)
}
fn message(actor: Uuid, family: Uuid, suffix: &str) -> proto::ProductRequest {
    let event = Uuid::new_v4();
    let now = chrono::Utc::now().timestamp_micros();
    proto::ProductRequest {
        user_id: actor.to_string(),
        event_id: event.to_string(),
        session_family_id: family.to_string(),
        request_id: Uuid::new_v4().to_string(),
        http_method: "GET".into(),
        http_target: format!("/api/concert/events/{event}/{suffix}"),
        authority_expires_unix_micros: now + 1_800_000,
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
        access_expires_unix_micros: now + 60_000_000,
        trusted_network_bucket: vec![61; 32],
        ..Default::default()
    }
}
const FEED: &str =
    "feed?limit=7&cursor=feed_cursor&filter=WITH_MEDIA&before=2023-11-14T22%3A13%3A20Z";
// Observe only the already authenticated input, then delegate to the real
// production owner. This cannot construct or replace verified authority.
struct ObserveOwner {
    inner: ScopedProductOwner,
    producer: Arc<Producer>,
}
#[async_trait::async_trait]
impl ProductOwner for ObserveOwner {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        request: proto::ProductRequest,
    ) -> Result<tonic::Response<proto::ProductResponse>, Status> {
        *self.producer.original_deadline.lock().unwrap() = Some(context.deadline());
        self.inner.dispatch(context, request).await
    }
}
async fn dispatch(
    f: &Fixture,
    producer: Arc<Producer>,
    message: proto::ProductRequest,
    publisher: Option<DeliveryPublisher>,
) -> anyhow::Result<axum::http::Response<axum::body::Body>> {
    let signer = capture::gateway()?;
    let owner = ScopedProductOwner::new(
        Arc::new(f.fence.clone()),
        publisher.unwrap_or_else(|| f.delivery_publisher()),
        producer.clone(),
    );
    let owner = ObserveOwner {
        inner: owner,
        producer,
    };
    let service = guard(
        &POLICY,
        proto::product_service_server::ProductServiceServer::new(ProductRpcService::new(Arc::new(
            owner,
        ))),
        CallerRegistry::new().register(Workload::Gateway, "gateway-fixture")?,
        ActorDelegationVerifier::new(Workload::Concert, &[signer.public_key()])?,
    )?;
    let service =
        platform::call_deadline::CallDeadlineLayer.layer(RetainedScopeLayer.layer(service));
    let bytes = frame(&message);
    let proof = signer.issue(
        Workload::Concert,
        METHOD,
        Uuid::parse_str(&message.user_id)?,
        None,
        &bytes,
        chrono::Utc::now().timestamp(),
    );
    let mut req = request(METHOD, bytes)?;
    req.headers_mut().insert(HEADER, proof.parse()?);
    Ok(service.oneshot(req).await?.map(axum::body::Body::new))
}
async fn prepared(pool: &PgPool) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "select count(*) from pg_prepared_xacts where database=current_database()",
    )
    .fetch_one(pool)
    .await?)
}
async fn evidence(
    f: &Fixture,
    scope: &OwnerGuardScope,
) -> anyhow::Result<(String, bool, bool, bool, bool)> {
    let key = scope.producer_scope();
    Ok(sqlx::query_as("select phase,response_abandoned_at is not null,cancellation_settled_at is not null,serialized_at is not null,delivery_completed_at is not null from concert_product_owner_scopes where instance_id=$1 and scope_id=$2").bind(Uuid::parse_str(&key.instance_id)?).bind(Uuid::parse_str(&key.scope_id)?).fetch_one(&f.concert).await?)
}
async fn wait_settled(f: &Fixture, scope: &OwnerGuardScope) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(4), async {
        while !evidence(f, scope).await?.2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    Ok(())
}
async fn setup(
    mode: Mode,
) -> anyhow::Result<(
    Arc<Fixture>,
    Arc<Producer>,
    mpsc::UnboundedReceiver<Arc<OwnerGuardScope>>,
    proto::ProductRequest,
)> {
    let f = Arc::new(Fixture::with_native_abort().await?);
    let (actor, family, _) = environment::identities(&f.auth, 0).await?;
    let (tx, rx) = mpsc::unbounded_channel();
    let producer = Arc::new(Producer {
        mode,
        pool: f.concert.clone(),
        registry: f.registry.clone(),
        entered: tx,
        dropped: Arc::new(Notify::new()),
        callback: Mutex::new(None),
        original_deadline: Mutex::new(None),
    });
    Ok((f, producer, rx, message(actor, family, FEED)))
}
async fn entered(
    rx: &mut mpsc::UnboundedReceiver<Arc<OwnerGuardScope>>,
) -> anyhow::Result<Arc<OwnerGuardScope>> {
    tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await?
        .ok_or_else(|| anyhow::anyhow!("actual producer not entered"))
}
#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS CR09 controller"]
async fn caller_drop_destroys_actual_product_before_owner_release() -> anyhow::Result<()> {
    let (f, p, mut rx, m) = setup(Mode::Pending).await?;
    let ff = f.clone();
    let pp = p.clone();
    let task = tokio::spawn(async move { dispatch(&ff, pp, m, None).await });
    let scope = entered(&mut rx).await?;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(1), p.dropped.notified()).await?;
    wait_settled(&f, &scope).await?;
    assert_eq!(
        evidence(&f, &scope).await?,
        ("ROOT_FINISHED".into(), true, true, false, false)
    );
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (0, 0)
    );
    f.assert_no_recovery_calls();
    Ok(())
}
#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS CR09 controller"]
async fn original_deadline_stops_actual_product_and_settles_without_delivery() -> anyhow::Result<()>
{
    let (f, p, mut rx, m) = setup(Mode::Pending).await?;
    let ff = f.clone();
    let pp = p.clone();
    let task = tokio::spawn(async move { dispatch(&ff, pp, m, None).await });
    let scope = entered(&mut rx).await?;
    let _transport = tokio::time::timeout(Duration::from_secs(3), task).await??;
    tokio::time::timeout(Duration::from_secs(1), p.dropped.notified()).await?;
    wait_settled(&f, &scope).await?;
    assert_eq!(
        evidence(&f, &scope).await?,
        ("ROOT_FINISHED".into(), true, true, false, false)
    );
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (0, 0)
    );
    Ok(())
}
#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS CR09 controller"]
async fn admitted_callback_past_expiry_retains_actual_product_fences() -> anyhow::Result<()> {
    let (f, p, mut rx, m) = setup(Mode::Callback).await?;
    let ff = f.clone();
    let pp = p.clone();
    let task = tokio::spawn(async move { dispatch(&ff, pp, m, None).await });
    let scope = entered(&mut rx).await?;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(1), p.dropped.notified()).await?;
    tokio::time::sleep(Duration::from_millis(2250)).await;
    assert_eq!(
        evidence(&f, &scope).await?,
        ("SEALED".into(), false, false, false, false)
    );
    assert!(f
        .registry
        .lookup_root_read(&scope.root_read_view())
        .is_err());
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (1, 1)
    );
    drop(p.callback.lock().unwrap().take());
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(
        evidence(&f, &scope).await?,
        ("SEALED".into(), false, false, false, false)
    );
    Ok(())
}
#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS CR09 controller"]
async fn committed_native_without_receipt_keeps_actual_product_quarantined() -> anyhow::Result<()> {
    let (f, p, mut rx, m) = setup(Mode::CommittedUnknown).await?;
    let ff = f.clone();
    let pp = p.clone();
    let task = tokio::spawn(async move { dispatch(&ff, pp, m, None).await });
    let scope = entered(&mut rx).await?;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(1), p.dropped.notified()).await?;
    tokio::time::sleep(Duration::from_millis(2250)).await;
    assert_eq!(
        evidence(&f, &scope).await?,
        ("ROOT_FINISHED".into(), true, false, false, false)
    );
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (1, 1)
    );
    Ok(())
}
#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS CR09 controller"]
async fn successful_handoff_lost_before_eof_never_becomes_cancellation() -> anyhow::Result<()> {
    let (f, p, mut rx, m) = setup(Mode::Success).await?;
    let response = dispatch(&f, p, m, None).await?;
    let scope = entered(&mut rx).await?;
    drop(response);
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert_eq!(
        evidence(&f, &scope).await?,
        ("ACTIVE".into(), false, false, false, false)
    );
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (1, 1)
    );
    Ok(())
}
fn decode<M: Message + Default>(bytes: &[u8]) -> anyhow::Result<M> {
    anyhow::ensure!(bytes.len() >= 5 && bytes[0] == 0, "one plain gRPC frame");
    let size = u32::from_be_bytes(bytes[1..5].try_into()?) as usize;
    anyhow::ensure!(size + 5 == bytes.len(), "exact gRPC frame");
    Ok(M::decode(&bytes[5..])?)
}
#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS CR09 controller"]
async fn original_context_queries_parent_and_actual_success_eof_are_preserved() -> anyhow::Result<()>
{
    let (f, p, mut rx, initial) = setup(Mode::Success).await?;
    let actor = Uuid::parse_str(&initial.user_id)?;
    let family = Uuid::parse_str(&initial.session_family_id)?;
    let delivery = guard_without_delegation(
        &DELIVERY_POLICY,
        proto::product_delivery_service_server::ProductDeliveryServiceServer::new(
            DeliveryService::new(f.delivery_publisher()),
        ),
        CallerRegistry::new().register(Workload::Gateway, "gateway-fixture")?,
    )?;
    let delivery = platform::call_deadline::CallDeadlineLayer.layer(delivery);
    for suffix in [
        FEED.to_string(),
        format!(
            "posts/{}/comments?limit=9&cursor=comments_cursor",
            Uuid::new_v4()
        ),
        format!(
            "commands?operation_id=createConcertParty&idempotency_key={}",
            Uuid::new_v4()
        ),
    ] {
        let response = dispatch(&f, p.clone(), message(actor, family, &suffix), None).await?;
        let scope = entered(&mut rx).await?;
        assert_eq!(
            (prepared(&f.auth).await?, prepared(&f.friend).await?),
            (1, 1)
        );
        assert!(scope.finish_response().await.is_err());
        let body = response.into_body().collect().await?;
        let output: proto::ProductResponse = decode(&body.to_bytes())?;
        assert_eq!(output.status, 200);
        assert_eq!(
            evidence(&f, &scope).await?,
            ("ACTIVE".into(), false, false, true, false)
        );
        assert_eq!(
            (prepared(&f.auth).await?, prepared(&f.friend).await?),
            (1, 1)
        );
        let capability = output
            .delivery
            .ok_or_else(|| anyhow::anyhow!("actual delivery missing"))?;
        let done = delivery
            .clone()
            .oneshot(request(
                "/concert.product.v1.ProductDeliveryService/CompleteResponse",
                frame(&capability),
            )?)
            .await?;
        let body = done.into_body().collect().await?;
        let complete: proto::CompleteResponseResult = decode(&body.to_bytes())?;
        assert!(complete.completed);
        assert_eq!(
            evidence(&f, &scope).await?,
            ("ROOT_FINISHED".into(), false, false, true, true)
        );
        assert_eq!(
            (prepared(&f.auth).await?, prepared(&f.friend).await?),
            (0, 0)
        );
    }
    Ok(())
}
#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS CR09 controller"]
async fn actual_publication_failure_is_supervised_before_producer_dispatch() -> anyhow::Result<()> {
    let (f, p, mut rx, m) = setup(Mode::Pending).await?;
    let unrelated = Fixture::with_native_abort().await?;
    let response = dispatch(&f, p, m, Some(unrelated.delivery_publisher())).await?;
    assert_ne!(
        response
            .headers()
            .get("grpc-status")
            .and_then(|v| v.to_str().ok()),
        Some("0")
    );
    drop(response);
    assert!(
        rx.try_recv().is_err(),
        "producer must never enter after failed actual registry lookup"
    );
    let row:(String,bool,bool,bool,bool)=sqlx::query_as("select phase,response_abandoned_at is not null,cancellation_settled_at is not null,serialized_at is not null,delivery_completed_at is not null from concert_product_owner_scopes").fetch_one(&f.concert).await?;
    assert_eq!(row, ("ROOT_FINISHED".into(), true, true, false, false));
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (0, 0)
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS CR09 controller"]
async fn actual_product_destructor_fault_prevents_terminal_proof() -> anyhow::Result<()> {
    let (f, p, mut rx, m) = setup(Mode::DestructorPanic).await?;
    let ff = f.clone();
    let pp = p.clone();
    let task = tokio::spawn(async move { dispatch(&ff, pp, m, None).await });
    let scope = entered(&mut rx).await?;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(1), p.dropped.notified()).await?;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        evidence(&f, &scope).await?,
        ("ACTIVE".into(), false, false, false, false)
    );
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (1, 1)
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS CR09 controller"]
async fn caller_drop_during_actual_awaited_publication_never_dispatches_product(
) -> anyhow::Result<()> {
    let (f, p, mut rx, m) = setup(Mode::Pending).await?;
    // An owned-fixture-only SQL gate blocks the actual publication UPDATE.
    // It creates no scope/authority, response EOF, native result or proof.
    sqlx::raw_sql("CREATE FUNCTION cr09_publication_gate() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(8247209); RETURN NEW; END $$; CREATE TRIGGER cr09_publication_gate BEFORE UPDATE OF delivery_secret_hash ON concert_product_owner_scopes FOR EACH ROW EXECUTE FUNCTION cr09_publication_gate();").execute(&f.concert).await?;
    let mut gate = f.concert.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(8247209)")
        .execute(&mut *gate)
        .await?;
    let ff = f.clone();
    let task = tokio::spawn(async move { dispatch(&ff, p, m, None).await });
    tokio::time::timeout(Duration::from_secs(2),async {
        loop {
            let blocked:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and wait_event='advisory' and query like 'UPDATE concert_product_owner_scopes%')").fetch_one(&f.concert).await?;
            if blocked { return Ok::<_,anyhow::Error>(()); }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }).await??;
    let (instance, scope, digest): (Uuid, Uuid, Vec<u8>) = sqlx::query_as(
        "select instance_id,scope_id,binding_digest from concert_product_owner_scopes",
    )
    .fetch_one(&f.concert)
    .await?;
    let actual = f.registry.lookup_for_settlement(
        &concert_service::adapters::out::relationship_authority::guarded::proto::ProducerScope {
            instance_id: instance.to_string(),
            scope_id: scope.to_string(),
            binding_digest: digest,
        },
    )?;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(
        rx.try_recv().is_err(),
        "no producer can run before publication completes"
    );
    gate.rollback().await?;
    wait_settled(&f, &actual).await?;
    assert_eq!(
        evidence(&f, &actual).await?,
        ("ROOT_FINISHED".into(), true, true, false, false)
    );
    assert!(
        rx.try_recv().is_err(),
        "cancelled publication must never later dispatch producer"
    );
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (0, 0)
    );
    Ok(())
}
