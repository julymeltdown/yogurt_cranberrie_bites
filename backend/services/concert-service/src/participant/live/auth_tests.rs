//! Network contract qualification, not a database or public-route acceptance.
//! The entry uses the actual Gateway guard; the outbound Auth fixture uses its
//! actual Concert-only guard and dedicated read signer. The fixture deliberately
//! violates owner replies to test the receiving adapter independently.
use super::{auth::GrpcCanonicalAuth, *};
use crate::{
    adapters::out::relationship_authority::grpc::proto as auth_proto,
    participant::{ports::CanonicalAuthPort, proto, Operation, POLICY},
};
use platform::{
    actor_delegation::{
        relationship_read::{RelationshipReadSigner, RelationshipReadVerifier},
        request_for, ActorDelegationSigner, ActorDelegationVerifier,
    },
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, ServiceIdentityClientInterceptor, Workload},
    grpc_method_policy::{guard, MethodPolicy},
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tonic::{Request, Response, Status};
use uuid::Uuid;

const TOKEN: &str = "participant-live-auth-test";
const AUTH_POLICY: MethodPolicy = MethodPolicy {
    service: "relationship.v1.AuthConcertRelationshipService",
    workload: Some(Workload::Auth),
    methods: &[("ReadCurrent", &[Workload::Concert])],
    actor_fields: &[("ReadCurrent", &[&[1]])],
};
fn gateway(audience: Workload) -> ActorDelegationVerifier {
    ActorDelegationVerifier::new(audience, &[gateway_signer().public_key()]).unwrap()
}
fn gateway_signer() -> ActorDelegationSigner {
    ActorDelegationSigner::from_seed(Workload::Gateway, &[87; 32]).unwrap()
}
fn read_signer() -> RelationshipReadSigner {
    RelationshipReadSigner::from_seed(&[88; 32], &gateway(Workload::Concert)).unwrap()
}

struct Running {
    endpoint: String,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.task.abort();
    }
}
struct Owner {
    fault: &'static str,
    calls: Arc<AtomicUsize>,
}
#[tonic::async_trait]
impl auth_proto::auth_concert_relationship_service_server::AuthConcertRelationshipService
    for Owner
{
    async fn read_current(
        &self,
        request: Request<auth_proto::RelationshipRead>,
    ) -> Result<Response<auth_proto::AuthRelationshipObservation>, Status> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut query = request.into_inner();
        assert_eq!(query.actor_id, query.subject_id);
        assert_eq!(query.purpose, "SAFETY_SELF");
        assert!(!Uuid::parse_str(&query.event_id).unwrap().is_nil());
        assert!(!Uuid::parse_str(&query.session_family_id).unwrap().is_nil());
        assert!(!Uuid::parse_str(&query.request_id).unwrap().is_nil());
        let now = chrono::Utc::now().timestamp_micros();
        assert!(query.expires_unix_micros > now && query.expires_unix_micros <= now + 2_000_000);
        if self.fault == "slow" {
            tokio::time::sleep(Duration::from_millis(160)).await;
        }
        if self.fault == "owner-unavailable" {
            return Err(Status::unavailable("private-owner-detail"));
        }
        let mut reply = auth_proto::AuthRelationshipObservation {
            query: Some(query.clone()),
            observed_at_unix_micros: chrono::Utc::now().timestamp_micros(),
            valid_until_unix_micros: query.expires_unix_micros,
            owner_epoch: Uuid::new_v4().to_string(),
            revision: 1,
            actor_active: true,
            subject_active: true,
            session_live: true,
        };
        match self.fault {
            "foreign-query" => {
                query.request_id = Uuid::new_v4().to_string();
                reply.query = Some(query);
            }
            "no-query" => reply.query = None,
            "stale" => reply.observed_at_unix_micros = now - 3_000_000,
            "future" => reply.observed_at_unix_micros = now + 3_000_000,
            "extended" => reply.valid_until_unix_micros += 1,
            "expired" => reply.valid_until_unix_micros = now - 1,
            "bad-epoch" => reply.owner_epoch = Uuid::nil().to_string(),
            "zero-revision" => reply.revision = 0,
            "inactive" => reply.actor_active = false,
            "deleted-subject" => reply.subject_active = false,
            "revoked-session" => reply.session_live = false,
            _ => {}
        }
        let mut response = Response::new(reply);
        if self.fault != "cacheable" {
            response
                .metadata_mut()
                .insert("cache-control", "no-store".parse().unwrap());
        }
        Ok(response)
    }
}
async fn start_owner(fault: &'static str) -> (Running, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let verifier = RelationshipReadVerifier::new(
        Workload::Auth,
        &[read_signer().public_key()],
        &gateway(Workload::Auth),
    )
    .unwrap()
    .into_inner();
    let service = guard(&AUTH_POLICY,
        auth_proto::auth_concert_relationship_service_server::AuthConcertRelationshipServiceServer::new(Owner { fault, calls: calls.clone() }),
        CallerRegistry::new().register(Workload::Concert, TOKEN).unwrap(), verifier).unwrap();
    let incoming = futures::stream::unfold(listener, |listener| async move {
        Some((listener.accept().await.map(|(stream, _)| stream), listener))
    });
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .layer(CallDeadlineLayer)
            .add_service(service)
            .serve_with_incoming(incoming)
            .await
            .unwrap();
    });
    (Running { endpoint, task }, calls)
}
#[derive(Clone)]
struct Entry {
    auth: GrpcCanonicalAuth,
}
#[tonic::async_trait]
impl proto::concert_participant_service_server::ConcertParticipantService for Entry {
    async fn get_participant(
        &self,
        request: Request<proto::ParticipantRequest>,
    ) -> Result<Response<proto::ParticipantResponse>, Status> {
        let context = VerifiedParticipantRequest::verify(&request, Operation::GetParticipant)?;
        let auth = self
            .auth
            .read(&context)
            .await
            .map_err(crate::participant::status)?;
        auth.check(&context).map_err(crate::participant::status)?;
        let mut response = Response::new(proto::ParticipantResponse {
            response_json: serde_json::to_vec(&serde_json::json!({"user_id":auth.user_id,"session_family_id":auth.session_family_id})).unwrap()
        });
        response
            .metadata_mut()
            .insert("cache-control", "no-store".parse().unwrap());
        Ok(response)
    }
    async fn set_avatar(
        &self,
        _: Request<proto::ParticipantRequest>,
    ) -> Result<Response<proto::ParticipantResponse>, Status> {
        Err(Status::unimplemented("Read fixture only"))
    }
    async fn update_preferences(
        &self,
        _: Request<proto::ParticipantRequest>,
    ) -> Result<Response<proto::ParticipantResponse>, Status> {
        Err(Status::unimplemented("Read fixture only"))
    }
}
async fn entry(auth: GrpcCanonicalAuth) -> Running {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let service = guard(
        &POLICY,
        proto::concert_participant_service_server::ConcertParticipantServiceServer::new(Entry {
            auth,
        }),
        CallerRegistry::new()
            .register(Workload::Gateway, "participant-gateway")
            .unwrap(),
        gateway(Workload::Concert),
    )
    .unwrap();
    let incoming = futures::stream::unfold(listener, |listener| async move {
        Some((listener.accept().await.map(|(stream, _)| stream), listener))
    });
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .layer(CallDeadlineLayer)
            .add_service(service)
            .serve_with_incoming(incoming)
            .await
            .unwrap();
    });
    Running { endpoint, task }
}
fn message(budget: Duration) -> proto::ParticipantRequest {
    let now = chrono::Utc::now().timestamp_micros();
    let event = Uuid::new_v4();
    proto::ParticipantRequest {
        user_id: Uuid::new_v4().to_string(),
        event_id: event.to_string(),
        session_family_id: Uuid::new_v4().to_string(),
        request_id: Uuid::new_v4().to_string(),
        http_method: "GET".into(),
        http_target: format!("/api/concert/events/{event}/me"),
        request_json: vec![],
        release_digest: "a".repeat(64),
        authority_expires_unix_micros: now + budget.as_micros() as i64,
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
    }
}
async fn invoke(
    endpoint: &str,
    message: proto::ParticipantRequest,
    timeout: Duration,
) -> Result<Response<proto::ParticipantResponse>, Status> {
    let channel = tonic::transport::Endpoint::from_shared(endpoint.to_string())
        .unwrap()
        .connect()
        .await
        .unwrap();
    let mut client = proto::concert_participant_service_client::ConcertParticipantServiceClient::with_interceptor(
        platform::actor_delegation::ActorDelegationClient::new(channel, gateway_signer(), Workload::Concert),
        ServiceIdentityClientInterceptor::new(Workload::Gateway, "participant-gateway").unwrap());
    let mut request = request_for(message.clone(), &message.user_id).unwrap();
    request.set_timeout(timeout);
    platform::call_deadline::scope(
        std::time::Instant::now() + timeout,
        client.get_participant(request),
    )
    .await
}
#[tokio::test]
async fn actual_read_client_requires_current_exact_no_store_owner_reply() {
    for fault in [
        "valid",
        "foreign-query",
        "no-query",
        "stale",
        "future",
        "extended",
        "expired",
        "bad-epoch",
        "zero-revision",
        "inactive",
        "deleted-subject",
        "revoked-session",
        "cacheable",
        "owner-unavailable",
    ] {
        let (owner, calls) = start_owner(fault).await;
        let auth = GrpcCanonicalAuth::connect(&owner.endpoint, TOKEN, None, read_signer()).unwrap();
        let entry = entry(auth).await;
        let message = message(Duration::from_millis(1500));
        let result = invoke(
            &entry.endpoint,
            message.clone(),
            Duration::from_millis(1500),
        )
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 1, "{fault}");
        if fault == "valid" {
            let response = result.unwrap();
            assert_eq!(
                response.metadata().get("cache-control").unwrap(),
                "no-store"
            );
            let body: serde_json::Value =
                serde_json::from_slice(&response.into_inner().response_json).unwrap();
            assert_eq!(body["user_id"], message.user_id);
            assert_eq!(body["session_family_id"], message.session_family_id);
        } else {
            let error = result.expect_err(fault);
            assert!(!error.message().contains("private-owner-detail"));
            let expected = if matches!(fault, "inactive" | "deleted-subject" | "revoked-session") {
                tonic::Code::PermissionDenied
            } else {
                tonic::Code::Unavailable
            };
            assert_eq!(error.code(), expected, "{fault}: {error}");
        }
    }
}
#[tokio::test]
async fn original_deadline_and_real_credentials_bound_auth_call() {
    let (owner, calls) = start_owner("valid").await;
    let auth = GrpcCanonicalAuth::connect(&owner.endpoint, "wrong-credential", None, read_signer())
        .unwrap();
    let server = entry(auth).await;
    assert!(invoke(
        &server.endpoint,
        message(Duration::from_secs(1)),
        Duration::from_secs(1)
    )
    .await
    .is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let (owner, calls) = start_owner("slow").await;
    let auth = GrpcCanonicalAuth::connect(&owner.endpoint, TOKEN, None, read_signer()).unwrap();
    let server = entry(auth).await;
    let started = std::time::Instant::now();
    assert!(invoke(
        &server.endpoint,
        message(Duration::from_millis(90)),
        Duration::from_secs(1)
    )
    .await
    .is_err());
    assert!(started.elapsed() < Duration::from_millis(500));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
