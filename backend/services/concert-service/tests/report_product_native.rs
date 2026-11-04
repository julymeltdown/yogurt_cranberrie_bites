//! OPS-PRODUCT-01: actual signed Product RPC, owner stores and Root terminal.
//! Helpers are read-only at cb944f7210f6c55fdf1d0e897c96ea7ef0ba8325.
#[path = "operations_live_support/capture.rs"]
mod capture;
#[path = "operations_live_support/environment.rs"]
mod environment;
#[path = "operations_live_support/owners.rs"]
mod owners;
#[path = "product_feature_admission_support/mod.rs"]
mod support;
use concert_application::commands::{FingerprintKeys, SigningKey};
use concert_service::{
    adapters::out::relationship_authority::guarded::{ChildRecoveryReference, OwnerGuardScope},
    config::{canonical_runtime::CanonicalRuntimeCore, native_resolver::NativeResolverUnion},
    operations_live::{report_product::ReportProduct, OperationsOwners, OperationsRuntime},
    product_rpc::{
        delivery::{DeliveryService, POLICY as DELIVERY_POLICY},
        proto,
        scope_ledger::{ChildIntentSettler, ChildRecoveryKey, ChildRecoveryKeys},
        scoped::{ProductRepresentation, ScopedProductOwner, ScopedProductProducer},
        settlement::POLICY as SETTLEMENT_POLICY,
        ProductRpcService, VerifiedProduct, POLICY,
    },
};
use http_body_util::BodyExt;
use platform::{
    actor_delegation::{
        exchange::VerifiedParentAuthority, response_scope::RetainedScopeLayer,
        ActorDelegationVerifier,
    },
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::{guard, guard_without_delegation},
};
use prost::Message;
use serde_json::json;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool, Row,
};
use std::{
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tonic::{Code, Status};
use tower::{Layer, ServiceExt};
use uuid::Uuid;

struct NoChildren;
#[async_trait::async_trait]
impl ChildIntentSettler for NoChildren {
    async fn recover_one(&self, _: &ChildRecoveryReference) -> Result<(), Status> {
        Err(Status::unavailable("Report has no child producer"))
    }
}
// Adapted identity checks from operations_live_support/environment.rs; only a
// dedicated test database under the already marked private Unix-socket root.
async fn concert_pool() -> anyhow::Result<PgPool> {
    let root = environment::root()?;
    let options = PgConnectOptions::from_str(&std::env::var("MCR_CANONICAL_CONCERT_URL")?)?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_concert_report_product")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "Foreign report fixture options"
    );
    let pool = PgPoolOptions::new()
        .max_connections(12)
        .connect_with(options)
        .await?;
    let row=sqlx::query("select current_database() db,current_setting('data_directory') data,current_setting('listen_addresses') listeners,(select system_identifier::text from pg_control_system()) system").fetch_one(&pool).await?;
    anyhow::ensure!(
        row.get::<String, _>("db") == "mcr_concert_report_product"
            && row.get::<String, _>("data") == root.join("data").to_string_lossy()
            && row.get::<String, _>("listeners").is_empty()
            && row.get::<String, _>("system") == std::env::var("MCR_CANONICAL_SYSTEM")?,
        "Foreign report PostgreSQL identity"
    );
    sqlx::migrate::Migrator::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations")
            .as_path(),
    )
    .await?
    .run(&pool)
    .await?;
    Ok(pool)
}
struct Observed {
    leaf: ReportProduct,
    scopes: Arc<Mutex<Vec<Arc<OwnerGuardScope>>>>,
    rebind: bool,
}
#[async_trait::async_trait]
impl ScopedProductProducer for Observed {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        mut frame: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        let parent = VerifiedParentAuthority::current().expect("Original parent propagated");
        assert_eq!(
            parent.parent_digest(),
            context.parent_authority().parent_digest()
        );
        assert_eq!(parent.message().unwrap(), frame.encode_to_vec());
        assert_eq!(scope.binding().actor(), context.actor());
        assert_eq!(scope.binding().event(), context.event());
        assert_eq!(scope.binding().session_family(), context.session());
        assert_eq!(scope.binding().request_id(), context.request());
        assert_eq!(
            scope.binding().owner_binding().parent_body_digest,
            context.parent_authority().request_digest()
        );
        assert!(scope.binding().deadline().stop() <= context.deadline().stop());
        self.scopes.lock().unwrap().push(scope.clone());
        if self.rebind {
            let mut body: serde_json::Value = serde_json::from_slice(&frame.http_body).unwrap();
            body["text"] = "tampered after admission".into();
            frame.http_body = serde_json::to_vec(&body).unwrap();
        }
        self.leaf.dispatch(context, scope, frame).await
    }
}
struct Fixture {
    auth: PgPool,
    friend: PgPool,
    concert: PgPool,
    actor: Uuid,
    family: Uuid,
    subject: Uuid,
    subjects: Vec<Uuid>,
    event: Uuid,
    core: CanonicalRuntimeCore,
    owner: Arc<ScopedProductOwner>,
    scopes: Arc<Mutex<Vec<Arc<OwnerGuardScope>>>>,
    _auth: owners::Running,
    _friend: owners::Running,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Fixture {
    async fn new(rebind: bool) -> anyhow::Result<Self> {
        Self::with_targets(rebind, 1).await
    }
    async fn with_targets(rebind: bool, count: usize) -> anyhow::Result<Self> {
        let auth = environment::pool("auth").await?;
        let friend = environment::pool("friend").await?;
        let concert = concert_pool().await?;
        let (actor, family, subjects) = environment::identities(&auth, count).await?;
        let subject = subjects[0];
        let event = Uuid::new_v4();
        sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Report product native','OPEN')").bind(event).bind(format!("report-{}",event.simple())).execute(&concert).await?;
        for user in std::iter::once(&actor).chain(subjects.iter()) {
            sqlx::query("insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')").bind(event).bind(user).execute(&concert).await?;
        }
        let instance = Uuid::new_v4();
        let core = CanonicalRuntimeCore::new(
            concert.clone(),
            instance,
            Arc::new(ChildRecoveryKeys::new(
                ChildRecoveryKey::new("report-product-v1", &[89; 32])?,
                vec![],
            )?),
            Arc::new(NoChildren),
        )?;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let root_url = format!("https://{}", listener.local_addr()?);
        let running_auth = owners::auth(auth.clone(), &root_url).await?;
        let running_friend = owners::friend(friend.clone(), &root_url).await?;
        let fence = core.owner_fence(
            &running_auth.url,
            &running_friend.url,
            "concert-fixture",
            &environment::tls(Workload::Concert)?,
            owners::signer()?,
        )?;
        let service = guard_without_delegation(
            &SETTLEMENT_POLICY,
            core.settlement_service(Arc::new(NativeResolverUnion::new(
                concert.clone(),
                instance,
            )?))
            .server(),
            CallerRegistry::new()
                .register(Workload::Auth, "auth-fixture")?
                .register(Workload::Friend, "friend-fixture")?,
        )?;
        let server = tonic::transport::Server::builder()
            .tls_config(environment::tls(Workload::Concert)?.server_config()?)?
            .add_service(service);
        let incoming = futures::stream::unfold(listener, |l| async {
            Some((l.accept().await.map(|(s, _)| s), l))
        });
        let task = tokio::spawn(async move {
            server
                .serve_with_incoming(incoming)
                .await
                .expect("Actual report settlement server")
        });
        let runtime = Arc::new(OperationsRuntime::new(
            concert.clone(),
            OperationsOwners::default(),
            Arc::new(FingerprintKeys::new(
                SigningKey::new("report-product-v1", &[91; 32])?,
                vec![],
            )?),
            &[92; 32],
        )?);
        let scopes = Arc::new(Mutex::new(vec![]));
        let observed = Arc::new(Observed {
            leaf: ReportProduct::new(runtime),
            scopes: scopes.clone(),
            rebind,
        });
        let owner = Arc::new(ScopedProductOwner::new(
            Arc::new(fence),
            core.delivery(),
            observed,
        ));
        Ok(Self {
            auth,
            friend,
            concert,
            actor,
            family,
            subject,
            subjects,
            event,
            core,
            owner,
            scopes,
            _auth: running_auth,
            _friend: running_friend,
            task,
        })
    }
    fn input(&self, key: Uuid) -> proto::ProductRequest {
        self.input_target(key, self.subject)
    }
    fn input_target(&self, key: Uuid, target: Uuid) -> proto::ProductRequest {
        let body = json!({"target_type":"USER","target_id":target,"reason":"SPAM","text":"private reporter canary","evidence_media_ids":[]});
        let mut request = support::input("POST", "reports", &body.to_string());
        request.user_id = self.actor.to_string();
        request.event_id = self.event.to_string();
        request.session_family_id = self.family.to_string();
        request.http_target = format!("/api/concert/events/{}/reports", self.event);
        request.idempotency_key = key.to_string();
        request
    }
    async fn call(
        &self,
        request: &proto::ProductRequest,
        foreign: bool,
    ) -> anyhow::Result<(Code, Option<proto::ProductResponse>)> {
        let service = guard(
            &POLICY,
            proto::product_service_server::ProductServiceServer::new(ProductRpcService::new(
                self.owner.clone(),
            )),
            CallerRegistry::new().register(Workload::Gateway, "gateway-feature-fixture")?,
            ActorDelegationVerifier::new(Workload::Concert, &[support::signer().public_key()])?,
        )?;
        let service = CallDeadlineLayer.layer(RetainedScopeLayer.layer(service));
        let wire = if foreign {
            support::signed(request, Uuid::new_v4(), Some("1800000u"))
        } else {
            support::wire(request)
        };
        let response = service.oneshot(wire).await?;
        let headers = response.headers().clone();
        let body = response
            .into_body()
            .collect()
            .await
            .map_err(|_| anyhow::anyhow!("Root gRPC body collection failed"))?;
        let status = Status::from_header_map(&headers)
            .or_else(|| body.trailers().and_then(Status::from_header_map))
            .ok_or_else(|| anyhow::anyhow!("Missing terminal status"))?;
        if status.code() != Code::Ok {
            return Ok((status.code(), None));
        }
        let bytes = body.to_bytes();
        anyhow::ensure!(bytes.len() >= 5, "Missing real product frame");
        Ok((Code::Ok, Some(proto::ProductResponse::decode(&bytes[5..])?)))
    }
    async fn finish(&self, cap: proto::ResponseDelivery) -> anyhow::Result<()> {
        self.finish_delivery(cap).await?;
        self.assert_settled().await
    }
    async fn finish_delivery(&self, cap: proto::ResponseDelivery) -> anyhow::Result<()> {
        use proto::product_delivery_service_server::ProductDeliveryServiceServer;
        let server = guard_without_delegation(
            &DELIVERY_POLICY,
            ProductDeliveryServiceServer::new(DeliveryService::new(self.core.delivery())),
            CallerRegistry::new().register(Workload::Gateway, "gateway-feature-fixture")?,
        )?;
        let request = tonic::codegen::http::Request::builder()
            .uri("/concert.product.v1.ProductDeliveryService/CompleteResponse")
            .header("content-type", "application/grpc")
            .header("te", "trailers")
            .header("x-service-caller", "gateway")
            .header("x-service-token", "gateway-feature-fixture")
            .body(tonic::body::Body::new(http_body_util::Full::new(
                tonic::codegen::Bytes::from(frame(&cap)),
            )))?;
        let response = CallDeadlineLayer.layer(server).oneshot(request).await?;
        let headers = response.headers().clone();
        let body = response.into_body().collect().await?;
        let status = Status::from_header_map(&headers)
            .or_else(|| body.trailers().and_then(Status::from_header_map))
            .ok_or_else(|| anyhow::anyhow!("Missing delivery status"))?;
        anyhow::ensure!(
            status.code() == Code::Ok,
            "Root delivery refused: {}",
            status.message()
        );
        let bytes = body.to_bytes();
        anyhow::ensure!(
            proto::CompleteResponseResult::decode(&bytes[5..])?.completed,
            "Root delivery incomplete"
        );
        Ok(())
    }
    async fn assert_settled(&self) -> anyhow::Result<()> {
        for pool in [&self.auth, &self.friend, &self.concert] {
            let count: i64 = sqlx::query_scalar(
                "select count(*) from pg_prepared_xacts where database=current_database()",
            )
            .fetch_one(pool)
            .await?;
            assert_eq!(count, 0, "Actual owner prepared work must settle");
        }
        Ok(())
    }
    async fn effect_counts(&self) -> anyhow::Result<(i64, i64, i64, i64)> {
        Ok(sqlx::query_as("select (select count(*) from concert_reports where event_id=$1),(select count(*) from concert_command_receipts where event_id=$1),(select count(*) from concert_outbox where event_id=$1),(select count(*) from concert_operations_audit where event_id=$1)")
            .bind(self.event).fetch_one(&self.concert).await?)
    }
    async fn no_effects(&self) -> anyhow::Result<()> {
        for table in [
            "concert_reports",
            "concert_command_receipts",
            "concert_outbox",
        ] {
            let count: i64 =
                sqlx::query_scalar(&format!("select count(*) from {table} where event_id=$1"))
                    .bind(self.event)
                    .fetch_one(&self.concert)
                    .await?;
            assert_eq!(count, 0);
        }
        for pool in [&self.auth, &self.friend, &self.concert] {
            let count: i64 = sqlx::query_scalar(
                "select count(*) from pg_prepared_xacts where database=current_database()",
            )
            .fetch_one(pool)
            .await?;
            assert_eq!(count, 0, "Refused report cannot retain prepared owner work");
        }
        Ok(())
    }
}
fn frame<T: Message>(message: &T) -> Vec<u8> {
    let bytes = message.encode_to_vec();
    let mut result = vec![0];
    result.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    result.extend(bytes);
    result
}

#[tokio::test]
#[ignore = "frozen owned PG16 three-database report-product controller only"]
async fn concurrent_duplicate_reports_persist_one_effect_and_replay_the_safe_failed_receipt(
) -> anyhow::Result<()> {
    let f = Fixture::new(false).await?;
    let requests = [f.input(Uuid::new_v4()), f.input(Uuid::new_v4())];
    let (left, right) = tokio::join!(f.call(&requests[0], false), f.call(&requests[1], false));
    let mut receipts = Vec::new();
    // Both responses remain retained until their own actual Root terminals.
    // Completing one cannot assert that the other scope has already settled.
    for result in [left, right] {
        let (code, output) = result?;
        assert_eq!(code, Code::Ok);
        let output = output.unwrap();
        assert_eq!(output.status, 202);
        assert!(!String::from_utf8_lossy(&output.body).contains("private reporter canary"));
        assert!(!String::from_utf8_lossy(&output.body).contains(&f.subject.to_string()));
        receipts.push(serde_json::from_slice::<serde_json::Value>(&output.body)?);
        f.finish_delivery(output.delivery.unwrap()).await?;
    }
    f.assert_settled().await?;
    assert_eq!(
        receipts
            .iter()
            .filter(|r| r["state"] == "SUCCEEDED")
            .count(),
        1
    );
    let failed = receipts
        .iter()
        .position(|r| r["state"] == "FAILED")
        .unwrap();
    assert_eq!(receipts[failed]["failure_reason"], "RESOURCE_UNAVAILABLE");
    assert!(receipts[failed]["resource_id"].is_null());
    assert_eq!(f.effect_counts().await?, (1, 2, 1, 1));

    let key = Uuid::parse_str(receipts[failed]["idempotency_key"].as_str().unwrap())?;
    let (code, replay) = f.call(&f.input(key), false).await?;
    assert_eq!(code, Code::Ok);
    let replay = replay.unwrap();
    assert_eq!(replay.status, 202);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&replay.body)?,
        receipts[failed]
    );
    f.finish(replay.delivery.unwrap()).await?;
    assert_eq!(f.effect_counts().await?, (1, 2, 1, 1));
    Ok(())
}

#[tokio::test]
#[ignore = "frozen owned PG16 three-database report-product controller only"]
async fn actual_hourly_quota_preserves_replay_and_conflict_without_new_effects(
) -> anyhow::Result<()> {
    let f = Fixture::with_targets(false, 11).await?;
    let first_key = Uuid::new_v4();
    let mut first_receipt = None;
    let budget: i64 = sqlx::query_scalar("select receipts_used from concert_command_budget")
        .fetch_one(&f.concert)
        .await?;
    // These are ten actual signed, authorized Product commands, not seeded reports.
    for (index, target) in f.subjects.iter().take(10).enumerate() {
        let key = if index == 0 {
            first_key
        } else {
            Uuid::new_v4()
        };
        let (code, output) = f.call(&f.input_target(key, *target), false).await?;
        assert_eq!(code, Code::Ok);
        let output = output.unwrap();
        assert_eq!(output.status, 202);
        let receipt: serde_json::Value = serde_json::from_slice(&output.body)?;
        assert_eq!(receipt["state"], "SUCCEEDED");
        if index == 0 {
            first_receipt = Some(receipt);
        }
        f.finish(output.delivery.unwrap()).await?;
    }
    assert_eq!(f.effect_counts().await?, (10, 10, 10, 10));
    let (code, output) = f
        .call(&f.input_target(Uuid::new_v4(), f.subjects[10]), false)
        .await?;
    assert_eq!(code, Code::Ok);
    let output = output.unwrap();
    assert_eq!(output.status, 429);
    let error: serde_json::Value = serde_json::from_slice(&output.body)?;
    assert_eq!(error["details"]["reason"], "CAPACITY_EXCEEDED");
    assert!(!String::from_utf8_lossy(&output.body).contains("private reporter canary"));
    f.finish(output.delivery.unwrap()).await?;

    let (code, replay) = f.call(&f.input(first_key), false).await?;
    assert_eq!(code, Code::Ok);
    let replay = replay.unwrap();
    assert_eq!(replay.status, 202);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&replay.body)?,
        first_receipt.unwrap()
    );
    f.finish(replay.delivery.unwrap()).await?;

    // The changed body is signed as the original request, not tampered post-admission.
    let mut changed = f.input(first_key);
    let mut body: serde_json::Value = serde_json::from_slice(&changed.http_body)?;
    body["text"] = "different private reporter canary".into();
    changed.http_body = serde_json::to_vec(&body)?;
    let (code, conflict) = f.call(&changed, false).await?;
    assert_eq!(code, Code::Ok);
    let conflict = conflict.unwrap();
    assert_eq!(conflict.status, 409);
    let error: serde_json::Value = serde_json::from_slice(&conflict.body)?;
    assert_eq!(error["details"]["reason"], "IDEMPOTENCY_CONFLICT");
    assert!(!String::from_utf8_lossy(&conflict.body).contains("private reporter canary"));
    f.finish(conflict.delivery.unwrap()).await?;
    assert_eq!(f.effect_counts().await?, (10, 10, 10, 10));
    let after: i64 = sqlx::query_scalar("select receipts_used from concert_command_budget")
        .fetch_one(&f.concert)
        .await?;
    assert_eq!(
        after,
        budget + 10,
        "Quota/conflict/replay cannot consume extra receipt budget"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "frozen owned PG16 three-database report-product controller only"]
async fn outbox_failure_rolls_back_report_audit_receipt_and_budget_then_same_key_retries(
) -> anyhow::Result<()> {
    let f = Fixture::new(false).await?;
    let key = Uuid::new_v4();
    let budget: i64 = sqlx::query_scalar("select receipts_used from concert_command_budget")
        .fetch_one(&f.concert)
        .await?;
    // A test-local constraint in the identity-checked disposable DB fails only
    // this event's outbox insert, after the real report and audit mutations.
    let constraint = format!("mcr_report_fault_{}", f.event.simple());
    sqlx::query(&format!("alter table concert_outbox add constraint {constraint} check (event_id <> '{}'::uuid) not valid", f.event))
        .execute(&f.concert).await?;
    let response = f.call(&f.input(key), false).await;
    sqlx::query(&format!(
        "alter table concert_outbox drop constraint {constraint}"
    ))
    .execute(&f.concert)
    .await?;
    let (code, output) = response?;
    assert_eq!(code, Code::Ok);
    let output = output.unwrap();
    assert_eq!(output.status, 503);
    let error: serde_json::Value = serde_json::from_slice(&output.body)?;
    assert_eq!(error["details"]["reason"], "DEPENDENCY_UNAVAILABLE");
    let safe = String::from_utf8_lossy(&output.body);
    assert!(!safe.contains(&constraint));
    assert!(!safe.contains("private reporter canary"));
    assert!(!safe.contains(&f.subject.to_string()));
    f.finish(output.delivery.unwrap()).await?;
    assert_eq!(f.effect_counts().await?, (0, 0, 0, 0));
    let after: i64 = sqlx::query_scalar("select receipts_used from concert_command_budget")
        .fetch_one(&f.concert)
        .await?;
    assert_eq!(after, budget);

    let (code, retry) = f.call(&f.input(key), false).await?;
    assert_eq!(code, Code::Ok);
    let retry = retry.unwrap();
    assert_eq!(retry.status, 202);
    let receipt: serde_json::Value = serde_json::from_slice(&retry.body)?;
    assert_eq!(receipt["state"], "SUCCEEDED");
    f.finish(retry.delivery.unwrap()).await?;
    assert_eq!(f.effect_counts().await?, (1, 1, 1, 1));
    let after: i64 = sqlx::query_scalar("select receipts_used from concert_command_budget")
        .fetch_one(&f.concert)
        .await?;
    assert_eq!(after, budget + 1);
    Ok(())
}

#[tokio::test]
#[ignore = "frozen owned PG16 three-database report-product controller only"]
async fn original_report_runs_actual_product_rpc_receipt_outbox_and_delivery_then_replays(
) -> anyhow::Result<()> {
    let f = Fixture::new(false).await?;
    let key = Uuid::new_v4();
    let request = f.input(key);
    let (code, output) = f.call(&request, false).await?;
    assert_eq!(code, Code::Ok);
    let output = output.unwrap();
    assert_eq!(output.status, 202);
    let receipt: serde_json::Value = serde_json::from_slice(&output.body)?;
    assert_eq!(receipt["state"], "SUCCEEDED");
    assert_eq!(receipt["actor_id"], f.actor.to_string());
    assert_eq!(receipt["event_id"], f.event.to_string());
    assert!(!String::from_utf8_lossy(&output.body).contains("private reporter canary"));
    let cap = output.delivery.unwrap();
    let row:(String,bool,bool)=sqlx::query_as("select phase,serialized_at is not null,delivery_completed_at is not null from concert_product_owner_scopes where instance_id=$1 and scope_id=$2").bind(f.core.ledger().instance()).bind(Uuid::parse_str(&cap.scope_id)?).fetch_one(&f.concert).await?;
    assert_eq!(
        row,
        ("ACTIVE".into(), true, false),
        "Internal serialization does not complete Root delivery"
    );
    let scopes = f.scopes.lock().unwrap().clone();
    assert_eq!(scopes.len(), 1);
    assert!(Arc::ptr_eq(
        &scopes[0],
        &f.core
            .registry()
            .lookup_for_settlement(&scopes[0].producer_scope())?
    ));
    f.finish(cap).await?;
    let (code, replay) = f.call(&f.input(key), false).await?;
    assert_eq!(code, Code::Ok);
    let replay = replay.unwrap();
    assert_eq!(replay.status, 202);
    let replay_receipt: serde_json::Value = serde_json::from_slice(&replay.body)?;
    assert_eq!(replay_receipt["receipt_id"], receipt["receipt_id"]);
    f.finish(replay.delivery.unwrap()).await?;
    let counts:(i64,i64,i64)=sqlx::query_as("select (select count(*) from concert_reports where event_id=$1),(select count(*) from concert_command_receipts where event_id=$1),(select count(*) from concert_outbox where event_id=$1)").bind(f.event).fetch_one(&f.concert).await?;
    assert_eq!(counts, (1, 1, 1));
    let witnesses: i64 = sqlx::query_scalar(
        "select count(*) from concert_operations_native_witnesses where instance_id=$1",
    )
    .bind(f.core.ledger().instance())
    .fetch_one(&f.concert)
    .await?;
    assert_eq!(witnesses, 2);
    Ok(())
}
#[tokio::test]
#[ignore = "frozen owned PG16 three-database report-product controller only"]
async fn canonical_revocation_waits_for_actual_root_terminal_and_replay_is_denied(
) -> anyhow::Result<()> {
    let f = Fixture::new(false).await?;
    let key = Uuid::new_v4();
    let (_, output) = f.call(&f.input(key), false).await?;
    let output = output.unwrap();
    assert_eq!(output.status, 202);
    let mut connection = f.auth.acquire().await?;
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *connection)
        .await?;
    let actor = f.actor;
    let writer = tokio::spawn(async move {
        let mut tx = sqlx::Acquire::begin(&mut *connection).await?;
        sqlx::query("set local lock_timeout='2s'")
            .execute(&mut *tx)
            .await?;
        sqlx::query("update users set status='SUSPENDED' where id=$1")
            .bind(actor)
            .execute(&mut *tx)
            .await?;
        tx.commit().await
    });
    tokio::time::timeout(Duration::from_millis(750), async {
        loop {
            let blocked: bool = sqlx::query_scalar("select cardinality(pg_blocking_pids($1))>0")
                .bind(pid)
                .fetch_one(&f.auth)
                .await?;
            if blocked {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        Ok::<_, sqlx::Error>(())
    })
    .await??;
    assert!(!writer.is_finished());
    f.finish(output.delivery.unwrap()).await?;
    tokio::time::timeout(Duration::from_secs(2), writer).await???;
    assert_ne!(f.call(&f.input(key), false).await?.0, Code::Ok);
    let count: i64 = sqlx::query_scalar("select count(*) from concert_reports where event_id=$1")
        .bind(f.event)
        .fetch_one(&f.concert)
        .await?;
    assert_eq!(count, 1);
    Ok(())
}
#[tokio::test]
#[ignore = "frozen owned PG16 three-database report-product controller only"]
async fn foreign_parent_and_rebound_body_never_create_report_effects() -> anyhow::Result<()> {
    let f = Fixture::new(true).await?;
    let request = f.input(Uuid::new_v4());
    assert_ne!(f.call(&request, true).await?.0, Code::Ok);
    assert!(f.scopes.lock().unwrap().is_empty());
    f.no_effects().await?;
    let mut foreign_session = f.input(Uuid::new_v4());
    foreign_session.session_family_id = Uuid::new_v4().to_string();
    assert_ne!(f.call(&foreign_session, false).await?.0, Code::Ok);
    f.no_effects().await?;
    let mut expired = f.input(Uuid::new_v4());
    expired.authority_expires_unix_micros = chrono::Utc::now().timestamp_micros() - 1;
    assert_ne!(f.call(&expired, false).await?.0, Code::Ok);
    f.no_effects().await?;
    let (code, output) = f.call(&request, false).await?;
    assert_eq!(code, Code::Ok);
    let output = output.unwrap();
    assert_eq!(output.status, 403);
    f.finish(output.delivery.unwrap()).await?;
    f.no_effects().await?;
    let registered: i64 = sqlx::query_scalar(
        "select count(*) from concert_product_owner_transactions where instance_id=$1",
    )
    .bind(f.core.ledger().instance())
    .fetch_one(&f.concert)
    .await?;
    assert_eq!(registered, 0);
    Ok(())
}
#[tokio::test]
#[ignore = "frozen owned PG16 three-database report-product controller only"]
async fn safety_target_policy_and_unsupported_routes_preserve_original_guards() -> anyhow::Result<()>
{
    // Published canonical SafetyInteraction intentionally permits safety reports
    // about an ineligible subject; actor/session eligibility still applies.
    let suspended = Fixture::new(false).await?;
    sqlx::query("update users set status='SUSPENDED' where id=$1")
        .bind(suspended.subject)
        .execute(&suspended.auth)
        .await?;
    let (code, output) = suspended
        .call(&suspended.input(Uuid::new_v4()), false)
        .await?;
    assert_eq!(code, Code::Ok);
    let output = output.unwrap();
    suspended.finish(output.delivery.clone().unwrap()).await?;
    assert_eq!(output.status, 202);
    let receipt: serde_json::Value = serde_json::from_slice(&output.body)?;
    assert_eq!(receipt["state"], "SUCCEEDED");
    let reports: i64 = sqlx::query_scalar("select count(*) from concert_reports where event_id=$1")
        .bind(suspended.event)
        .fetch_one(&suspended.concert)
        .await?;
    assert_eq!(reports, 1);

    // The built-in evidence owner still requires an actual event target. No
    // admission or substitute/recent-target evidence is invented by this leaf.
    let f = Fixture::new(false).await?;
    sqlx::query("delete from concert_event_admissions where event_id=$1 and user_id=$2")
        .bind(f.event)
        .bind(f.subject)
        .execute(&f.concert)
        .await?;
    let (code, output) = f.call(&f.input(Uuid::new_v4()), false).await?;
    assert_eq!(code, Code::Ok);
    let output = output.unwrap();
    f.finish(output.delivery.clone().unwrap()).await?;
    assert_eq!(output.status, 403);
    f.no_effects().await?;
    let scopes = f.scopes.lock().unwrap().len();
    for suffix in ["admin/reports", "me/reports", "admin/settings"] {
        let mut request = f.input(Uuid::new_v4());
        request.http_target = format!("/api/concert/events/{}/{suffix}", f.event);
        assert_ne!(f.call(&request, false).await?.0, Code::Ok);
    }
    let mut request = f.input(Uuid::new_v4());
    let mut body: serde_json::Value = serde_json::from_slice(&request.http_body)?;
    body["evidence_media_ids"] = json!([Uuid::new_v4()]);
    request.http_body = serde_json::to_vec(&body)?;
    assert_ne!(f.call(&request, false).await?.0, Code::Ok);
    assert_eq!(f.scopes.lock().unwrap().len(), scopes);
    f.no_effects().await?;
    Ok(())
}
