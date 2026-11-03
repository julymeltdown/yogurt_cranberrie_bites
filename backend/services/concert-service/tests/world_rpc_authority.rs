//! Generated RPC and workload/signature/actor policy; no database claim.
use concert_service::world::rpc::{authorize_world, proto, POLICY};
use http_body_util::{BodyExt, Full};
use platform::{
    actor_delegation::{ActorDelegationSigner, ActorDelegationVerifier, HEADER},
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::guard,
};
use prost::Message;
use std::time::Duration;
use tonic::{Request, Response, Status};
use tower::ServiceExt;
use uuid::Uuid;

const METHOD: &str = "/concert.world.v1.WorldService/ListRooms";

#[derive(Clone)]
struct Probe;

#[tonic::async_trait]
impl proto::world_service_server::WorldService for Probe {
    async fn list_rooms(
        &self,
        request: Request<proto::WorldRequest>,
    ) -> Result<Response<proto::WorldResponse>, Status> {
        authorize_world(&request, "ListRooms")?.check()?;
        let mut response = Response::new(proto::WorldResponse {
            world_json: b"{}".to_vec(),
        });
        response
            .metadata_mut()
            .insert("cache-control", "no-store".parse().unwrap());
        Ok(response)
    }
    async fn issue_ticket(
        &self,
        _: Request<proto::WorldRequest>,
    ) -> Result<Response<proto::WorldResponse>, Status> {
        Err(Status::unimplemented("fixture"))
    }
    async fn current_membership(
        &self,
        _: Request<proto::CurrentMembershipRequest>,
    ) -> Result<Response<proto::CurrentMembershipResponse>, Status> {
        Err(Status::unimplemented("fixture"))
    }
}

fn message() -> proto::WorldRequest {
    let now = chrono::Utc::now().timestamp_micros();
    let event = Uuid::new_v4();
    proto::WorldRequest {
        user_id: Uuid::new_v4().to_string(),
        event_id: event.to_string(),
        session_family_id: Uuid::new_v4().to_string(),
        request_id: Uuid::new_v4().to_string(),
        http_method: "GET".into(),
        http_target: format!("/api/concert/events/{event}/rooms"),
        http_body: vec![],
        authority_expires_unix_micros: now + 1_000_000,
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
    }
}

fn framed(message: &proto::WorldRequest) -> Vec<u8> {
    let bytes = message.encode_to_vec();
    let mut body = vec![0];
    body.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    body.extend(bytes);
    body
}

async fn invoke(message: proto::WorldRequest, kind: &str) -> i32 {
    let signer = ActorDelegationSigner::from_seed(Workload::Gateway, &[42; 32]).unwrap();
    let verifier = ActorDelegationVerifier::new(Workload::Concert, &[signer.public_key()]).unwrap();
    let service = guard(
        &POLICY,
        proto::world_service_server::WorldServiceServer::new(Probe),
        CallerRegistry::new()
            .register(Workload::Gateway, "world-gateway")
            .unwrap(),
        verifier,
    )
    .unwrap();
    let body = framed(&message);
    let actor = if kind == "actor" {
        Uuid::new_v4()
    } else {
        Uuid::parse_str(&message.user_id).unwrap()
    };
    let token = signer.issue(
        if kind == "audience" {
            Workload::Profile
        } else {
            Workload::Concert
        },
        METHOD,
        actor,
        None,
        &body,
        chrono::Utc::now().timestamp(),
    );
    let mut request = axum::http::Request::builder()
        .method("POST")
        .uri(METHOD)
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .header("x-service-caller", "gateway")
        .header("x-service-token", "world-gateway");
    if kind != "unsigned" {
        request = request.header(HEADER, token);
    }
    let request = request
        .body(tonic::body::Body::new(Full::new(
            tonic::codegen::Bytes::from(body),
        )))
        .unwrap();
    let response = if kind == "no-deadline" {
        service.oneshot(request).await.unwrap()
    } else {
        platform::call_deadline::scope(
            std::time::Instant::now() + Duration::from_millis(900),
            service.oneshot(request),
        )
        .await
        .unwrap()
    };
    let headers = response.headers().clone();
    let collected = response.into_body().collect().await.unwrap();
    headers
        .get("grpc-status")
        .or_else(|| collected.trailers().and_then(|t| t.get("grpc-status")))
        .expect("gRPC completion status")
        .to_str()
        .unwrap()
        .parse()
        .unwrap()
}

#[tokio::test]
async fn authentic_signed_request_reaches_world_producer() {
    assert_eq!(invoke(message(), "valid").await, 0);
}

#[tokio::test]
async fn invalid_actor_audience_or_missing_proof_is_refused() {
    for kind in ["unsigned", "audience", "actor", "no-deadline"] {
        assert_ne!(invoke(message(), kind).await, 0, "{kind}");
    }
}

#[tokio::test]
async fn invalid_target_body_and_time_bounds_are_refused() {
    for kind in [
        "event", "target", "body", "method", "expired", "future", "stale",
    ] {
        let mut value = message();
        match kind {
            "event" => value.event_id = Uuid::nil().to_string(),
            "target" => value.http_target.push_str("?actor=other"),
            "body" => value.http_body = b"{}".to_vec(),
            "method" => value.http_method = "POST".into(),
            "expired" => {
                value.authority_expires_unix_micros = chrono::Utc::now().timestamp_micros() - 1
            }
            "future" => value.auth_observed_at_unix_micros += 3_000_000,
            "stale" => value.auth_observed_at_unix_micros -= 3_000_000,
            _ => unreachable!(),
        }
        assert_ne!(invoke(value, "valid").await, 0, "{kind}");
    }
}
