//! CU01 adapts the pinned Operations parent capture to the actual Participant
//! UpdatePreferences protobuf and policy. Only signed frame capture, not a grant.
use concert_service::participant::{proto, POLICY};
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
impl proto::concert_participant_service_server::ConcertParticipantService for Capture {
    async fn update_preferences(
        &self,
        request: Request<proto::ParticipantRequest>,
    ) -> Result<Response<proto::ParticipantResponse>, Status> {
        let parent = request
            .extensions()
            .get::<VerifiedParentAuthority>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Actual verified Gateway parent required"))?;
        *self.0.lock().map_err(|_| Status::internal("capture"))? = Some(parent);
        Ok(Response::new(proto::ParticipantResponse {
            ..Default::default()
        }))
    }
    async fn get_participant(
        &self,
        request: Request<proto::ParticipantRequest>,
    ) -> Result<Response<proto::ParticipantResponse>, Status> {
        self.update_preferences(request).await
    }
    async fn set_avatar(
        &self,
        request: Request<proto::ParticipantRequest>,
    ) -> Result<Response<proto::ParticipantResponse>, Status> {
        self.update_preferences(request).await
    }
}

pub(super) async fn binding(
    actor: Uuid,
    family: Uuid,
    event: Uuid,
    operation: &str,
    http_method: &str,
    suffix: &str,
    body: Vec<u8>,
    _key: Option<Uuid>,
) -> anyhow::Result<(
    concert_service::adapters::out::relationship_authority::guarded::GuardBinding,
    proto::ParticipantRequest,
)> {
    let request_id = Uuid::new_v4();
    let target = format!("/api/concert/events/{event}/{suffix}");
    let now = chrono::Utc::now().timestamp_micros();
    let deadline =
        AuthorityRpcDeadline::new(now + 4_800_000, std::time::Duration::from_micros(4_800_000))?;
    let message = proto::ParticipantRequest {
        user_id: actor.to_string(),
        event_id: event.to_string(),
        session_family_id: family.to_string(),
        request_id: request_id.to_string(),
        http_method: http_method.into(),
        http_target: target.clone(),
        request_json: body.clone(),
        release_digest: "8".repeat(64),
        authority_expires_unix_micros: deadline.expires_micros(),
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
        ..Default::default()
    };
    let bytes = message.encode_to_vec();
    let mut frame = vec![0];
    frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    frame.extend(bytes);
    let grpc_method = "/concert.participant.v1.ConcertParticipantService/UpdatePreferences";
    let signer = gateway()?;
    let proof = signer.issue(
        Workload::Concert,
        grpc_method,
        actor,
        None,
        &frame,
        chrono::Utc::now().timestamp(),
    );
    let captured = Arc::new(Mutex::new(None));
    let server = guard(
        &POLICY,
        proto::concert_participant_service_server::ConcertParticipantServiceServer::new(Capture(
            captured.clone(),
        )),
        CallerRegistry::new().register(Workload::Gateway, "gateway-fixture")?,
        ActorDelegationVerifier::new(Workload::Concert, &[signer.public_key()])?,
    )?;
    let response = server
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(grpc_method)
                .header("content-type", "application/grpc")
                .header("te", "trailers")
                .header("x-service-caller", "gateway")
                .header("x-service-token", "gateway-fixture")
                .header(HEADER, proof)
                .body(axum::body::Body::from(frame))?,
        )
        .await?;
    let response_body = response
        .into_body()
        .collect()
        .await
        .map_err(|error| anyhow::anyhow!("Parent capture body failed: {error}"))?;
    anyhow::ensure!(
        response_body
            .trailers()
            .and_then(|v| v.get("grpc-status"))
            .map_or(true, |v| v == "0"),
        "Parent capture RPC denied"
    );
    let parent = captured
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| anyhow::anyhow!("No admitted parent"))?;
    Ok((
        concert_service::adapters::out::relationship_authority::guarded::GuardBinding::from_parent(
            &parent,
            actor,
            event,
            family,
            request_id,
            operation,
            http_method,
            &target,
            Sha256::digest(&body).into(),
            deadline,
        )?,
        message,
    ))
}
pub(super) fn gateway() -> anyhow::Result<ActorDelegationSigner> {
    ActorDelegationSigner::from_seed(Workload::Gateway, &[74; 32])
}
