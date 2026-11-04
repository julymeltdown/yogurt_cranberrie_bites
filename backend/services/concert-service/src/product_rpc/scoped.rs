//! The actual root acquisition/delivery wrapper. Producers receive one held
//! scope and cannot substitute observations, mint delivery tokens or finish it.
use super::{delivery::DeliveryPublisher, proto, ProductOwner, VerifiedProduct};
use crate::adapters::out::relationship_authority::guarded::{
    CanonicalOwnerFence, GuardBinding, OwnerGuardScope,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tonic::{Response, Status};

/// Producer representation only; this contains no owner/completion authority.
/// ProductRpcService enforces the closed status/content/body boundary.
pub struct ProductRepresentation {
    pub status: u32,
    pub content_type: String,
    pub body: Vec<u8>,
}
#[async_trait::async_trait]
pub trait ScopedProductProducer: Send + Sync {
    /// Implementations must perform real local eligibility/projection and native
    /// transaction registration under this exact scope. No second acquisition,
    /// release-on-return, detached work, or successful default is permitted.
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        request: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status>;
}
pub struct ScopedProductOwner {
    fence: Arc<CanonicalOwnerFence>,
    delivery: DeliveryPublisher,
    producer: Arc<dyn ScopedProductProducer>,
}
impl ScopedProductOwner {
    pub fn new(
        fence: Arc<CanonicalOwnerFence>,
        delivery: DeliveryPublisher,
        producer: Arc<dyn ScopedProductProducer>,
    ) -> Self {
        Self {
            fence,
            delivery,
            producer,
        }
    }
}
#[async_trait::async_trait]
impl ProductOwner for ScopedProductOwner {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        request: proto::ProductRequest,
    ) -> Result<Response<proto::ProductResponse>, Status> {
        context.check()?;
        let binding = GuardBinding::from_parent(
            context.parent_authority(),
            context.actor(),
            context.event(),
            context.session(),
            context.request(),
            context.operation().contract_id(),
            &request.http_method,
            &request.http_target,
            Sha256::digest(&request.http_body).into(),
            context.deadline(),
        )?;
        let context = context.owned_for_producer();
        let parent = context.parent_authority().clone();
        let publisher = self.delivery.clone();
        let producer = self.producer.clone();
        let scope = self.fence.acquire(binding).await?;
        let owned = scope.clone();
        // No await separates acquisition from supervised ownership. The actual
        // publication, producer and response work all belong to this future.
        scope
            .run_owned(async move {
                // Tokio task locals do not cross spawn. Reinstall only the
                // original sealed parent; run_owned retains the original stop.
                parent
                    .scope(async move {
                        context.check()?;
                        // Do not even construct protected producer work until its
                        // exact durable terminal capability has been published.
                        let (delivery, token) = publisher.publish(owned.clone()).await?;
                        let representation =
                            match producer.dispatch(&context, owned.clone(), request).await {
                                Ok(value) => value,
                                Err(error) => produced_error(error.code(), context.request()),
                            };
                        // A normal refusal remains a response, not cancellation.
                        // Its actual EOF must follow the same guarded delivery path
                        // as success. Unknown native work still prevents settlement.
                        context.check()?;
                        let mut response = Response::new(proto::ProductResponse {
                            status: representation.status,
                            content_type: representation.content_type,
                            body: representation.body,
                            delivery: Some(delivery),
                            content_expires_unix_micros: owned
                                .disclosure_ceiling()?
                                .expires_micros(),
                        });
                        response.extensions_mut().insert(token);
                        response.metadata_mut().insert(
                            "cache-control",
                            "no-store".parse().expect("static metadata"),
                        );
                        Ok(response)
                    })
                    .await
            })
            .await
    }
}
fn produced_error(code: tonic::Code, request: uuid::Uuid) -> ProductRepresentation {
    // Match the existing fixed Gateway transport error projection. Never expose
    // dependency diagnostics, arbitrary metadata or internal completion fields.
    let (status, reason) = match code {
        tonic::Code::InvalidArgument => (400, "INVALID_INPUT"),
        tonic::Code::Unauthenticated => (401, "AUTHENTICATION_REQUIRED"),
        tonic::Code::PermissionDenied => (403, "PERMISSION_DENIED"),
        tonic::Code::NotFound => (404, "RESOURCE_UNAVAILABLE"),
        tonic::Code::DeadlineExceeded => (504, "DEADLINE_EXCEEDED"),
        _ => (503, "DEPENDENCY_UNAVAILABLE"),
    };
    ProductRepresentation{status,content_type:"application/json".into(),body:serde_json::to_vec(&serde_json::json!({
        "code":format!("CONCERT_{status}"),"message":"Concert request unavailable","traceId":request.to_string(),
        "timestamp":chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis,true),"details":{"reason":reason},
    })).expect("fixed JSON projection")}
}
