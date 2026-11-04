//! Real owner/ledger composition, signed production Product/Delivery guards and
//! actual retained gRPC body EOF. Gateway's outer network HTTP is a separate gate.
use super::{capture, environment, fixture::Fixture};
use concert_service::{
    adapters::out::relationship_authority::guarded::OwnerGuardScope,
    product_rpc::{
        delivery::{DeliveryService, POLICY as DELIVERY_POLICY},
        proto,
        scoped::{ProductRepresentation, ScopedProductOwner, ScopedProductProducer},
        ProductRpcService, VerifiedProduct, POLICY,
    },
};
use http_body_util::BodyExt;
use platform::{
    actor_delegation::{response_scope::RetainedScopeLayer, ActorDelegationVerifier, HEADER},
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::{guard, guard_without_delegation},
};
use prost::Message;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use tonic::Status;
use tower::{Layer, ServiceExt};
use uuid::Uuid;

struct Refuse {
    pool: PgPool,
    scope: Mutex<Option<Arc<OwnerGuardScope>>>,
}
#[async_trait::async_trait]
impl ScopedProductProducer for Refuse {
    async fn dispatch(
        &self,
        _: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        _: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        let producer = scope.producer_scope();
        let bound: bool = sqlx::query_scalar("select exists(select 1 from concert_product_owner_scopes where instance_id=$1 and scope_id=$2 and delivery_secret_hash is not null)")
            .bind(Uuid::parse_str(&producer.instance_id).map_err(|_| Status::internal("fixture identity"))?)
            .bind(Uuid::parse_str(&producer.scope_id).map_err(|_| Status::internal("fixture scope"))?)
            .fetch_one(&self.pool).await.map_err(|_| Status::unavailable("fixture ledger"))?;
        if !bound {
            return Err(Status::internal(
                "producer dispatched before durable delivery",
            ));
        }
        *self.scope.lock().expect("fixture capture") = Some(scope);
        Err(Status::permission_denied("private dependency diagnostic"))
    }
}
fn frame(message: &impl Message) -> Vec<u8> {
    let bytes = message.encode_to_vec();
    let mut frame = vec![0];
    frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    frame.extend(bytes);
    frame
}
fn request(method: &str, body: Vec<u8>) -> anyhow::Result<axum::http::Request<axum::body::Body>> {
    Ok(axum::http::Request::builder()
        .method("POST")
        .uri(method)
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .header("x-service-caller", "gateway")
        .header("x-service-token", "gateway-fixture")
        .header("grpc-timeout", "1800000u")
        .body(axum::body::Body::from(body))?)
}
fn decode<M: Message + Default>(bytes: &[u8]) -> anyhow::Result<M> {
    anyhow::ensure!(
        bytes.len() >= 5 && bytes[0] == 0,
        "one uncompressed gRPC frame required"
    );
    let size = u32::from_be_bytes(bytes[1..5].try_into()?) as usize;
    anyhow::ensure!(size + 5 == bytes.len(), "exact one response frame required");
    Ok(M::decode(&bytes[5..])?)
}
async fn prepared(pool: &PgPool) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "select count(*) from pg_prepared_xacts where database=current_database()",
    )
    .fetch_one(pool)
    .await?)
}

#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS canonical controller"]
async fn produced_errors_keep_real_guards_until_terminal_delivery() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let (actor, family, _) = environment::identities(&f.auth, 0).await?;
    let producer = Arc::new(Refuse {
        pool: f.concert.clone(),
        scope: Mutex::new(None),
    });
    let owner = ScopedProductOwner::new(
        Arc::new(f.fence.clone()),
        f.delivery_publisher(),
        producer.clone(),
    );
    let signer = capture::gateway()?;
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
    let delivery = guard_without_delegation(
        &DELIVERY_POLICY,
        proto::product_delivery_service_server::ProductDeliveryServiceServer::new(
            DeliveryService::new(f.delivery_publisher()),
        ),
        CallerRegistry::new().register(Workload::Gateway, "gateway-fixture")?,
    )?;
    let delivery = platform::call_deadline::CallDeadlineLayer.layer(delivery);
    // Exceed the actual 16-scope ceiling sequentially: safe produced refusals
    // must retire their real root/owner slots rather than silently leaking them.
    for _ in 0..17 {
        let event = Uuid::new_v4();
        let now = chrono::Utc::now().timestamp_micros();
        let message = proto::ProductRequest {
            user_id: actor.to_string(),
            event_id: event.to_string(),
            session_family_id: family.to_string(),
            request_id: Uuid::new_v4().to_string(),
            http_method: "GET".into(),
            http_target: format!("/api/concert/events/{event}/feed"),
            authority_expires_unix_micros: now + 1_800_000,
            auth_observed_at_unix_micros: now,
            profile_observed_at_unix_micros: now,
            access_expires_unix_micros: now + 60_000_000,
            ..Default::default()
        };
        let method = "/concert.product.v1.ProductService/Dispatch";
        let bytes = frame(&message);
        let proof = signer.issue(
            Workload::Concert,
            method,
            actor,
            None,
            &bytes,
            chrono::Utc::now().timestamp(),
        );
        let mut req = request(method, bytes)?;
        req.headers_mut().insert(HEADER, proof.parse()?);
        let response = service.clone().oneshot(req).await?;
        let scope = producer
            .scope
            .lock()
            .expect("fixture capture")
            .take()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "actual producer never dispatched: grpc-status={:?}, grpc-message={:?}",
                    response.headers().get("grpc-status"),
                    response.headers().get("grpc-message")
                )
            })?;
        assert_eq!(
            (prepared(&f.auth).await?, prepared(&f.friend).await?),
            (1, 1)
        );
        assert!(
            scope.finish_response().await.is_err(),
            "unconsumed body cannot certify serialization"
        );
        let body = response
            .into_body()
            .collect()
            .await
            .map_err(|error| anyhow::anyhow!("production body failed: {error}"))?;
        anyhow::ensure!(
            body.trailers()
                .and_then(|h| h.get("grpc-status"))
                .map_or(true, |v| v == "0"),
            "product denied"
        );
        let output: proto::ProductResponse = decode(&body.to_bytes())?;
        assert_eq!(output.status, 403);
        let safe: serde_json::Value = serde_json::from_slice(&output.body)?;
        assert_eq!(safe["details"]["reason"], "PERMISSION_DENIED");
        assert!(!String::from_utf8(output.body)?.contains("private dependency diagnostic"));
        let capability = output
            .delivery
            .ok_or_else(|| anyhow::anyhow!("produced error lost delivery"))?;
        assert_eq!(
            (prepared(&f.auth).await?, prepared(&f.friend).await?),
            (1, 1),
            "Concert EOF cannot release owner guards"
        );
        for _ in 0..2 {
            let response = delivery
                .clone()
                .oneshot(request(
                    "/concert.product.v1.ProductDeliveryService/CompleteResponse",
                    frame(&capability),
                )?)
                .await?;
            let body = response.into_body().collect().await?;
            anyhow::ensure!(
                body.trailers()
                    .and_then(|h| h.get("grpc-status"))
                    .map_or(true, |v| v == "0"),
                "delivery denied"
            );
            let done: proto::CompleteResponseResult = decode(&body.to_bytes())?;
            assert!(done.completed);
        }
        assert_eq!(
            (prepared(&f.auth).await?, prepared(&f.friend).await?),
            (0, 0)
        );
    }
    f.assert_no_recovery_calls();
    println!("17 actual scopes: durable publish before produced refusal, safe error envelope, production gRPC body EOF only marks serialized, authenticated terminal delivery settles real Auth/Friend, duplicate completion PASS; no outer Gateway HTTP claim");
    Ok(())
}
