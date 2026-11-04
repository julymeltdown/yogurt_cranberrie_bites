// Copied from owner_authority_support/capture.rs at a7f3989; captures arbitrary exact ProductRequest through unchanged actual guard.
//! Fixture entrypoint uses the production Gateway signature/workload guard.
//! It only captures its sealed parent; it cannot construct a positive owner scope.
use concert_service::product_rpc::{proto, POLICY};
use http_body_util::BodyExt;
use platform::{
    actor_delegation::{
        exchange::VerifiedParentAuthority, ActorDelegationSigner, ActorDelegationVerifier, HEADER,
    },
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::guard,
};
use prost::Message;
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

pub(super) async fn parent(
    message: &proto::ProductRequest,
) -> anyhow::Result<VerifiedParentAuthority> {
    let actor = Uuid::parse_str(&message.user_id)?;
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
    Ok(parent)
}

pub(super) fn gateway() -> anyhow::Result<ActorDelegationSigner> {
    ActorDelegationSigner::from_seed(Workload::Gateway, &[74; 32])
}
