//! Exact dedicated callback capture, through the existing reserved method guard.
use super::*;
use crate::grpc_auth::{CallerRegistry, VerifiedCaller};
use crate::grpc_method_policy::{guard, MethodPolicy};
use std::convert::Infallible;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tonic::server::NamedService;
use tower::ServiceExt;

const METHOD: &str = "/authority.lease.v1.ConcertRootReadService/CheckSubject";
static POLICY: MethodPolicy = MethodPolicy {
    service: "authority.lease.v1.ConcertRootReadService",
    workload: Some(Workload::Concert),
    methods: &[("CheckSubject", &[Workload::Post])],
    actor_fields: &[("CheckSubject", &[&[1, 3, 1]])],
};
fn field(number: u8, value: &[u8]) -> Vec<u8> {
    assert!(value.len() < 128);
    let mut bytes = vec![(number << 3) | 2, value.len() as u8];
    bytes.extend(value);
    bytes
}
fn frame(actor: Uuid) -> (Vec<u8>, Vec<u8>) {
    let binding = field(1, actor.to_string().as_bytes());
    let root = field(3, &binding);
    let mut message = field(1, &root);
    message.extend(field(2, Uuid::new_v4().to_string().as_bytes()));
    let mut body = vec![0];
    body.extend_from_slice(&(message.len() as u32).to_be_bytes());
    body.extend(&message);
    (body, message)
}
fn signer() -> ActorDelegationSigner {
    ActorDelegationSigner::from_seed(Workload::Post, &[77; 32]).unwrap()
}
fn verifier() -> ActorDelegationVerifier {
    let gateway = ActorDelegationSigner::from_seed(Workload::Gateway, &[74; 32]).unwrap();
    concert_read::PostConcertRootReadVerifier::new(
        &[signer().public_key()],
        &ActorDelegationVerifier::new(Workload::Concert, &[gateway.public_key()]).unwrap(),
    )
    .unwrap()
    .into_inner()
}
#[derive(Clone)]
struct Callback {
    calls: Arc<AtomicUsize>,
    actor: Uuid,
    message: Vec<u8>,
    deadline: Instant,
}
impl NamedService for Callback {
    const NAME: &'static str = "authority.lease.v1.ConcertRootReadService";
}
impl tower::Service<http::Request<Body>> for Callback {
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = std::future::Ready<Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        let Some(parent) = request
            .extensions()
            .get::<exchange::VerifiedParentAuthority>()
        else {
            return std::future::ready(Ok(Status::unauthenticated(
                "Verified callback frame required",
            )
            .into_http()));
        };
        let proof = *request
            .extensions()
            .get::<read_authority::VerifiedReadAuthority>()
            .unwrap();
        assert_eq!(
            request.extensions().get::<VerifiedActor>().unwrap().0,
            self.actor
        );
        assert_eq!(parent.actor(), self.actor);
        assert_eq!(parent.request().method, METHOD);
        assert_eq!(parent.message().unwrap(), self.message);
        assert!(
            proof
                .deadline(Duration::from_secs(2))
                .unwrap()
                .stop()
                .into_std()
                <= self.deadline
        );
        self.calls.fetch_add(1, Ordering::SeqCst);
        std::future::ready(Ok(http::Response::new(Body::default())))
    }
}
fn request(method: &str, caller: Workload, body: Vec<u8>, token: String) -> http::Request<Body> {
    http::Request::builder()
        .uri(method)
        .header("x-service-caller", caller.as_str())
        .header("x-service-token", format!("{}-fixture", caller.as_str()))
        .header(HEADER, token)
        .body(Body::new(Full::new(tonic::codegen::Bytes::from(body))))
        .unwrap()
}

#[tokio::test]
async fn signed_dedicated_callback_captures_exact_frame_actor_and_original_deadline() {
    let actor = Uuid::new_v4();
    let (body, message) = frame(actor);
    let deadline = Instant::now() + Duration::from_secs(1);
    let calls = Arc::new(AtomicUsize::new(0));
    let service = guard(
        &POLICY,
        Callback {
            calls: calls.clone(),
            actor,
            message,
            deadline,
        },
        CallerRegistry::new()
            .register(Workload::Post, "post-fixture")
            .unwrap(),
        verifier(),
    )
    .unwrap();
    let token = signer().issue(Workload::Concert, METHOD, actor, None, &body, now());
    crate::call_deadline::scope(
        deadline,
        service.oneshot(request(METHOD, Workload::Post, body, token)),
    )
    .await
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn callback_capture_rejects_other_callers_paths_bodies_and_actors() {
    let actor = Uuid::new_v4();
    let (body, message) = frame(actor);
    let deadline = Instant::now() + Duration::from_secs(1);
    let calls = Arc::new(AtomicUsize::new(0));
    let registry = CallerRegistry::new()
        .register(Workload::Post, "post-fixture")
        .unwrap()
        .register(Workload::Friend, "friend-fixture")
        .unwrap();
    let service = guard(
        &POLICY,
        Callback {
            calls: calls.clone(),
            actor,
            message,
            deadline,
        },
        registry,
        verifier(),
    )
    .unwrap();
    for case in 0..4 {
        let path = if case == 1 {
            "/authority.lease.v1.ConcertRootReadService/Other"
        } else {
            METHOD
        };
        let caller = if case == 0 {
            Workload::Friend
        } else {
            Workload::Post
        };
        let mut sent = body.clone();
        if case == 2 {
            *sent.last_mut().unwrap() ^= 1;
        }
        let signed_actor = if case == 3 { Uuid::new_v4() } else { actor };
        let token = signer().issue(Workload::Concert, path, signed_actor, None, &body, now());
        let response = crate::call_deadline::scope(
            deadline,
            service.clone().oneshot(request(path, caller, sent, token)),
        )
        .await
        .unwrap();
        assert_ne!(response.headers().get("grpc-status").unwrap(), "0");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn callback_capture_does_not_convert_forwarded_or_unbounded_proofs() {
    let actor = Uuid::new_v4();
    let (body, _) = frame(actor);
    let token = signer().issue(Workload::Concert, METHOD, actor, None, &body, now());
    let mut forwarded = request(METHOD, Workload::Post, body.clone(), token);
    forwarded
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Post));
    assert!(
        verify_forwarded_request(forwarded, Workload::Post, &[&[1, 3, 1]], &verifier())
            .await
            .is_err()
    );
    let token = signer().issue(Workload::Concert, METHOD, actor, None, &body, now());
    let unbounded = verify_request(
        request(METHOD, Workload::Post, body, token),
        Workload::Post,
        &[&[1, 3, 1]],
        &verifier(),
    )
    .await
    .unwrap();
    assert!(unbounded
        .extensions()
        .get::<exchange::VerifiedParentAuthority>()
        .is_none());
    let other = "/authority.lease.v1.ConcertRootReadService/Other";
    let (body, _) = frame(actor);
    let token = signer().issue(Workload::Concert, other, actor, None, &body, now());
    let verified_other = crate::call_deadline::scope(
        Instant::now() + Duration::from_secs(1),
        verify_request(
            request(other, Workload::Post, body, token),
            Workload::Post,
            &[&[1, 3, 1]],
            &verifier(),
        ),
    )
    .await
    .unwrap();
    assert!(verified_other
        .extensions()
        .get::<exchange::VerifiedParentAuthority>()
        .is_none());
}
