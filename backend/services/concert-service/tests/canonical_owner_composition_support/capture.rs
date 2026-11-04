//! CF01 adapted from owner_authority_support/capture.rs at 591d908a85a01fe0a494e04c0109bb0bec64a3e1; original SHA256 d8582d7ee9724f96592faff8e66cf3f16bec494c7f46b72746aac4373d15bada.
//! Fixture entrypoint uses the production Gateway signature/workload guard.
//! It only captures its sealed parent; it cannot construct a positive owner scope.
use concert_service::product_rpc::{proto, POLICY};
use http_body_util::BodyExt;
use platform::{
    actor_delegation::{
        authority_deadline::AuthorityRpcDeadline, exchange::VerifiedParentAuthority,
        ActorDelegationSigner, ActorDelegationVerifier, HEADER,
    },
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::guard,
};
use prost::Message;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use tonic::{Request, Response, Status};
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Clone)]
struct Capture(Arc<Mutex<Option<VerifiedParentAuthority>>>);
#[tonic::async_trait]
impl proto::product_service_server::ProductService for Capture {
    async fn dispatch(
        &self,
        request: Request<proto::ProductRequest>,
    ) -> Result<Response<proto::ProductResponse>, Status> {
        let parent = request
            .extensions()
            .get::<VerifiedParentAuthority>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Actual verified Gateway parent required"))?;
        *self.0.lock().map_err(|_| Status::internal("capture"))? = Some(parent);
        Ok(Response::new(proto::ProductResponse {
            status: 204,
            ..Default::default()
        }))
    }
}

pub(super) async fn binding(
    actor: Uuid,
    family: Uuid,
    event: Uuid,
    notification: bool,
) -> anyhow::Result<concert_service::adapters::out::relationship_authority::guarded::GuardBinding> {
    let request_id = Uuid::new_v4();
    let (operation, target) = if notification {
        (
            "getConcertNotificationUnreadCount",
            format!("/api/concert/events/{event}/me/notifications/unread-count"),
        )
    } else {
        (
            "listConcertFeed",
            format!("/api/concert/events/{event}/feed"),
        )
    };
    let now = chrono::Utc::now().timestamp_micros();
    let deadline =
        AuthorityRpcDeadline::new(now + 4_800_000, std::time::Duration::from_micros(4_800_000))?;
    let message = proto::ProductRequest {
        user_id: actor.to_string(),
        event_id: event.to_string(),
        session_family_id: family.to_string(),
        request_id: request_id.to_string(),
        http_method: "GET".into(),
        http_target: target.clone(),
        http_body: b"{}".to_vec(),
        authority_expires_unix_micros: deadline.expires_micros(),
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
        access_expires_unix_micros: now + 60_000_000,
        ..Default::default()
    };
    let bytes = message.encode_to_vec();
    let mut frame = vec![0];
    frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    frame.extend(bytes);
    let method = "/concert.product.v1.ProductService/Dispatch";
    let signer = gateway()?;
    let proof = signer.issue(
        Workload::Concert,
        method,
        actor,
        None,
        &frame,
        chrono::Utc::now().timestamp(),
    );
    let captured = Arc::new(Mutex::new(None));
    let server = guard(
        &POLICY,
        proto::product_service_server::ProductServiceServer::new(Capture(captured.clone())),
        CallerRegistry::new().register(Workload::Gateway, "gateway-fixture")?,
        ActorDelegationVerifier::new(Workload::Concert, &[signer.public_key()])?,
    )?;
    let response = server
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(method)
                .header("content-type", "application/grpc")
                .header("te", "trailers")
                .header("x-service-caller", "gateway")
                .header("x-service-token", "gateway-fixture")
                .header(HEADER, proof)
                .body(axum::body::Body::from(frame))?,
        )
        .await?;
    let body = response
        .into_body()
        .collect()
        .await
        .map_err(|error| anyhow::anyhow!("Parent capture body failed: {error}"))?;
    anyhow::ensure!(
        body.trailers()
            .and_then(|v| v.get("grpc-status"))
            .map_or(true, |v| v == "0"),
        "Parent capture RPC denied"
    );
    let parent = captured
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| anyhow::anyhow!("No admitted parent"))?;
    Ok(
        concert_service::adapters::out::relationship_authority::guarded::GuardBinding::from_parent(
            &parent,
            actor,
            event,
            family,
            request_id,
            operation,
            "GET",
            &target,
            Sha256::digest(b"{}").into(),
            deadline,
        )?,
    )
}
pub(super) fn gateway() -> anyhow::Result<ActorDelegationSigner> {
    ActorDelegationSigner::from_seed(Workload::Gateway, &[74; 32])
}
