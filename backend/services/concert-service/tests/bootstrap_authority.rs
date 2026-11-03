//! Real generated RPC and production workload/signature guard; no database claim.
use concert_service::rpc::{authorize_bootstrap, proto, POLICY};
use http_body_util::{BodyExt, Full};
use platform::{
    actor_delegation::{ActorDelegationSigner, ActorDelegationVerifier, HEADER},
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::guard,
};
use prost::Message;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tonic::{Request, Response, Status};
use tower::ServiceExt;
use uuid::Uuid;
const METHOD: &str = "/concert.v1.ConcertService/Bootstrap";
#[derive(Clone)]
struct Probe(Arc<AtomicUsize>);
#[tonic::async_trait]
impl proto::concert_service_server::ConcertService for Probe {
    async fn bootstrap(
        &self,
        request: Request<proto::BootstrapRequest>,
    ) -> Result<Response<proto::BootstrapResponse>, Status> {
        let authority = authorize_bootstrap(&request)?;
        authority.check()?;
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(Response::new(proto::BootstrapResponse {
            bootstrap_json: b"{}".to_vec(),
        }))
    }
}
fn message() -> proto::BootstrapRequest {
    let now = chrono::Utc::now().timestamp_micros();
    let event = Uuid::new_v4();
    proto::BootstrapRequest {
        user_id: Uuid::new_v4().to_string(),
        event_id: event.to_string(),
        session_family_id: Uuid::new_v4().to_string(),
        request_id: Uuid::new_v4().to_string(),
        authority_expires_unix_micros: now + 1_000_000,
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
        display_name: "Current public name".into(),
        bio: None,
        http_method: "GET".into(),
        http_target: format!("/api/concert/events/{event}/bootstrap"),
    }
}
fn framed(m: &proto::BootstrapRequest) -> Vec<u8> {
    let bytes = m.encode_to_vec();
    let mut body = vec![0];
    body.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    body.extend(bytes);
    body
}
async fn invoke(m: proto::BootstrapRequest, kind: &str) -> (i32, usize) {
    let calls = Arc::new(AtomicUsize::new(0));
    let signer = ActorDelegationSigner::from_seed(Workload::Gateway, &[42; 32]).unwrap();
    let verifier = ActorDelegationVerifier::new(Workload::Concert, &[signer.public_key()]).unwrap();
    let service = guard(
        &POLICY,
        proto::concert_service_server::ConcertServiceServer::new(Probe(calls.clone())),
        CallerRegistry::new()
            .register(Workload::Gateway, "test-gateway")
            .unwrap()
            .register(Workload::Post, "test-post")
            .unwrap(),
        verifier,
    )
    .unwrap();
    let body = framed(&m);
    let signed_body = if kind == "tampered" {
        framed(&message())
    } else {
        body.clone()
    };
    let audience = if kind == "audience" {
        Workload::Profile
    } else {
        Workload::Concert
    };
    let actor = if kind == "actor" {
        Uuid::new_v4()
    } else {
        Uuid::parse_str(&m.user_id).unwrap()
    };
    let token = signer.issue(
        audience,
        METHOD,
        actor,
        None,
        &signed_body,
        chrono::Utc::now().timestamp(),
    );
    let sibling = kind == "sibling";
    let mut request = axum::http::Request::builder()
        .method("POST")
        .uri(METHOD)
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .header("x-service-caller", if sibling { "post" } else { "gateway" })
        .header(
            "x-service-token",
            if kind == "credential" {
                "wrong"
            } else if sibling {
                "test-post"
            } else {
                "test-gateway"
            },
        );
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
    let status = headers
        .get("grpc-status")
        .or_else(|| collected.trailers().and_then(|t| t.get("grpc-status")))
        .expect("gRPC completion status")
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    (status, calls.load(Ordering::SeqCst))
}
#[tokio::test]
async fn authentic_current_bound_request_reaches_the_producer() {
    assert_eq!(invoke(message(), "valid").await, (0, 1));
}
#[tokio::test]
async fn invalid_workload_signature_binding_or_budget_never_reaches_the_producer() {
    for kind in [
        "unsigned",
        "credential",
        "sibling",
        "audience",
        "actor",
        "tampered",
        "no-deadline",
    ] {
        let (code, calls) = invoke(message(), kind).await;
        assert_ne!(code, 0, "{kind}");
        assert_eq!(calls, 0, "{kind}");
    }
}
#[tokio::test]
async fn signed_but_invalid_owner_assertions_are_refused() {
    for kind in [
        "event", "target", "method", "name", "bio", "expired", "future", "stale", "family",
    ] {
        let mut m = message();
        match kind {
            "event" => m.event_id = Uuid::nil().to_string(),
            "target" => m.http_target.push_str("?actor=other"),
            "method" => m.http_method = "POST".into(),
            "name" => m.display_name = "\nprivate".into(),
            "bio" => m.bio = Some("x".repeat(161)),
            "expired" => {
                m.auth_observed_at_unix_micros -= 1_000_000;
                m.profile_observed_at_unix_micros -= 1_000_000;
                m.authority_expires_unix_micros = chrono::Utc::now().timestamp_micros() - 1;
            }
            "future" => m.auth_observed_at_unix_micros += 3_000_000,
            "stale" => m.auth_observed_at_unix_micros -= 3_000_000,
            "family" => m.session_family_id = "not-a-family".into(),
            _ => unreachable!(),
        }
        let (code, calls) = invoke(m, "valid").await;
        assert_ne!(code, 0, "{kind}");
        assert_eq!(calls, 0, "{kind}");
    }
}
