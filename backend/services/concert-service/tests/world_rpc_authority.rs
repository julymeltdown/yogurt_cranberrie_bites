//! Generated RPC and workload/signature/actor policy; no database claim.
use concert_application::world_ticket::WorldError;
use concert_domain::{AuthUserId, EventId};
use concert_service::world::rpc::{authorize_world, proto, POLICY};
use concert_service::world_rpc::{WorldCanonicalAuthority, WorldRpcService};
use http_body_util::{BodyExt, Full};
use platform::{
    actor_delegation::{ActorDelegationSigner, ActorDelegationVerifier, HEADER},
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::guard,
};
use prost::Message;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;
use tonic::{Request, Response, Status};
use tower::ServiceExt;
use uuid::Uuid;

const METHOD: &str = "/concert.world.v1.WorldService/ListRooms";

#[derive(Clone)]
struct Probe(Arc<AtomicUsize>);

#[derive(Clone)]
struct Rooms(Arc<AtomicUsize>);

#[async_trait::async_trait]
impl WorldCanonicalAuthority for Rooms {
    async fn rooms_subject(&self, _: AuthUserId, _: EventId, _: Uuid) -> Result<(), WorldError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(WorldError::Denied)
    }
    async fn subject(
        &self,
        _: AuthUserId,
        _: EventId,
        _: Uuid,
        _: &concert_application::world_ticket::ReleasePins,
        _: chrono::DateTime<chrono::Utc>,
        _: chrono::DateTime<chrono::Utc>,
    ) -> Result<concert_application::world_ticket::VerifiedTicketSubject, WorldError> {
        Err(WorldError::Denied)
    }
}

struct Entropy;
impl concert_service::world_rpc::WorldTicketEntropy for Entropy {
    fn generate(&self) -> Result<concert_application::world_ticket::AdmissionTicket, WorldError> {
        Err(WorldError::Unavailable)
    }
}

#[tonic::async_trait]
impl proto::world_service_server::WorldService for Probe {
    async fn list_rooms(
        &self,
        request: Request<proto::WorldRequest>,
    ) -> Result<Response<proto::WorldResponse>, Status> {
        authorize_world(&request, "ListRooms")?.check()?;
        self.0.fetch_add(1, Ordering::SeqCst);
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
        access_expires_unix_micros: now + 60_000_000,
    }
}

fn framed(message: &proto::WorldRequest) -> Vec<u8> {
    let bytes = message.encode_to_vec();
    let mut body = vec![0];
    body.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    body.extend(bytes);
    body
}

async fn invoke(message: proto::WorldRequest, kind: &str) -> (i32, usize) {
    let calls = Arc::new(AtomicUsize::new(0));
    let signer = ActorDelegationSigner::from_seed(Workload::Gateway, &[42; 32]).unwrap();
    let verifier = ActorDelegationVerifier::new(Workload::Concert, &[signer.public_key()]).unwrap();
    let service = guard(
        &POLICY,
        proto::world_service_server::WorldServiceServer::new(Probe(calls.clone())),
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
async fn authentic_signed_request_reaches_world_producer() {
    assert_eq!(invoke(message(), "valid").await, (0, 1));
}

#[tokio::test]
async fn invalid_actor_audience_or_missing_proof_is_refused() {
    for kind in ["unsigned", "audience", "actor", "no-deadline"] {
        assert_eq!(invoke(message(), kind).await.1, 0, "{kind}");
    }
}

#[tokio::test]
async fn invalid_target_body_and_time_bounds_are_refused() {
    for kind in [
        "event",
        "target",
        "body",
        "method",
        "expired",
        "future",
        "stale",
        "access-expired",
        "authority-after-access",
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
            "access-expired" => value.access_expires_unix_micros = 0,
            "authority-after-access" => {
                value.access_expires_unix_micros = value.authority_expires_unix_micros - 1
            }
            _ => unreachable!(),
        }
        assert_eq!(invoke(value, "valid").await.1, 0, "{kind}");
    }
}

#[tokio::test]
async fn rooms_authority_runs_before_catalog_and_denial_has_no_fallback() {
    let calls = Arc::new(AtomicUsize::new(0));
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused@127.0.0.1:1/unused")
        .unwrap();
    let bootstrap = Arc::new(concert_service::bootstrap::BootstrapService::new(
        pool.clone(),
        test_release(),
    ));
    let service = WorldRpcService::new(pool, bootstrap, Rooms(calls.clone()), Arc::new(Entropy));
    let service_calls = Arc::new(AtomicUsize::new(0));
    let service = Counting {
        inner: service,
        calls: service_calls.clone(),
    };
    let service = guard(
        &POLICY,
        proto::world_service_server::WorldServiceServer::new(service),
        CallerRegistry::new()
            .register(Workload::Gateway, "world-gateway")
            .unwrap(),
        ActorDelegationVerifier::new(
            Workload::Concert,
            &[
                ActorDelegationSigner::from_seed(Workload::Gateway, &[42; 32])
                    .unwrap()
                    .public_key(),
            ],
        )
        .unwrap(),
    )
    .unwrap();
    let result = service
        .oneshot(grpc_request(message(), "valid"))
        .await
        .unwrap();
    let headers = result.headers().clone();
    let collected = result.into_body().collect().await.unwrap();
    let status = headers
        .get("grpc-status")
        .or_else(|| {
            collected
                .trailers()
                .and_then(|value| value.get("grpc-status"))
        })
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert_eq!(status, "3");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(service_calls.load(Ordering::SeqCst), 1);
}

struct Counting<A> {
    inner: WorldRpcService<A>,
    calls: Arc<AtomicUsize>,
}

impl<A> Clone for Counting<A>
where
    A: Clone + WorldCanonicalAuthority,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            calls: self.calls.clone(),
        }
    }
}

#[tonic::async_trait]
impl<A: WorldCanonicalAuthority + 'static> proto::world_service_server::WorldService
    for Counting<A>
{
    async fn list_rooms(
        &self,
        request: Request<proto::WorldRequest>,
    ) -> Result<Response<proto::WorldResponse>, Status> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        proto::world_service_server::WorldService::list_rooms(&self.inner, request).await
    }
    async fn issue_ticket(
        &self,
        request: Request<proto::WorldRequest>,
    ) -> Result<Response<proto::WorldResponse>, Status> {
        proto::world_service_server::WorldService::issue_ticket(&self.inner, request).await
    }
    async fn current_membership(
        &self,
        request: Request<proto::CurrentMembershipRequest>,
    ) -> Result<Response<proto::CurrentMembershipResponse>, Status> {
        proto::world_service_server::WorldService::current_membership(&self.inner, request).await
    }
}

fn test_release() -> Arc<concert_application::bootstrap::ValidatedProductRelease> {
    use concert_application::bootstrap::{
        BootstrapDefaults, BootstrapRelease, ValidatedProductRelease,
    };
    use concert_domain::avatar::SetAvatar;
    use serde_json::Value;
    let document: Value = serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/core/bootstrap-open.json"
    ))
    .unwrap();
    let raw = serde_json::to_vec(&serde_json::json!({
        "expected_revision": 1,
        "config": document["avatar"]["config"].clone()
    }))
    .unwrap();
    let avatar = SetAvatar::parse(&raw).unwrap();
    Arc::new(
        ValidatedProductRelease::from_validated(
            BootstrapRelease::new(
                BootstrapDefaults {
                    theme: concert_domain::Theme::Revenge,
                    avatar: avatar.selection,
                },
                serde_json::to_vec(&document).unwrap(),
            )
            .unwrap(),
            "a".repeat(64),
        )
        .unwrap(),
    )
}

fn grpc_request(
    message: proto::WorldRequest,
    kind: &str,
) -> axum::http::Request<tonic::body::Body> {
    let signer = ActorDelegationSigner::from_seed(Workload::Gateway, &[42; 32]).unwrap();
    let actor = if kind == "actor" {
        Uuid::new_v4()
    } else {
        Uuid::parse_str(&message.user_id).unwrap()
    };
    let body = framed(&message);
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
    request
        .body(tonic::body::Body::new(Full::new(
            tonic::codegen::Bytes::from(body),
        )))
        .unwrap()
}
