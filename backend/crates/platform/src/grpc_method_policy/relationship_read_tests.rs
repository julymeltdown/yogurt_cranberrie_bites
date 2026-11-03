//! Exact reserved registration and independent relationship/Gateway key domains.
use super::*;
use crate::actor_delegation::{
    relationship_read::{RelationshipReadSigner, RelationshipReadVerifier},
    ActorDelegationSigner, Expected, VerifiedActor, HEADER,
};
use std::{
    convert::Infallible,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tower::ServiceExt;
use uuid::Uuid;
const AUTH: &str = "relationship.v1.AuthConcertRelationshipService";
const FRIEND: &str = "relationship.v1.FriendConcertRelationshipService";
const METHOD: &str = "/relationship.v1.AuthConcertRelationshipService/ReadCurrent";
fn exact(service: &'static str, workload: Workload) -> MethodPolicy {
    MethodPolicy {
        service,
        workload: Some(workload),
        methods: &[("ReadCurrent", &[Workload::Concert])],
        actor_fields: &[("ReadCurrent", &[&[1]])],
    }
}
#[test]
fn registration_exception_is_exact_and_keeps_other_actor_methods_gateway_only() {
    for (service, workload) in [(AUTH, Workload::Auth), (FRIEND, Workload::Friend)] {
        assert!(exact(service, workload).validate(service).is_ok());
        let cases = [
            MethodPolicy {
                workload: Some(Workload::Concert),
                ..exact(service, workload)
            },
            MethodPolicy {
                methods: &[("Other", &[Workload::Concert])],
                ..exact(service, workload)
            },
            MethodPolicy {
                methods: &[("ReadCurrent", &[Workload::Gateway])],
                ..exact(service, workload)
            },
            MethodPolicy {
                methods: &[("ReadCurrent", &[Workload::Concert, Workload::Gateway])],
                ..exact(service, workload)
            },
            MethodPolicy {
                methods: &[("ReadCurrent", &[Workload::Concert]), ("Other", &[])],
                ..exact(service, workload)
            },
            MethodPolicy {
                actor_fields: &[("ReadCurrent", &[&[2]])],
                ..exact(service, workload)
            },
            MethodPolicy {
                actor_fields: &[("ReadCurrent", &[&[1], &[2]])],
                ..exact(service, workload)
            },
            MethodPolicy {
                actor_fields: &[],
                ..exact(service, workload)
            },
        ];
        for policy in cases {
            assert!(policy.validate(service).is_err(), "{policy:?}");
        }
    }
    for old in [
        "auth.v1.AuthService",
        "friend.v1.FriendService",
        "demo.v1.DemoService",
    ] {
        assert!(exact(old, Workload::Auth).validate(old).is_err());
    }
    let old = MethodPolicy {
        methods: &[("ReadCurrent", &[Workload::Gateway])],
        ..exact("auth.v1.AuthService", Workload::Auth)
    };
    assert!(old.validate(old.service).is_ok());
}
fn gateway() -> ActorDelegationVerifier {
    let signer = ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32]).unwrap();
    ActorDelegationVerifier::new(Workload::Auth, &[signer.public_key()]).unwrap()
}
fn read_signer() -> ActorDelegationSigner {
    ActorDelegationSigner::from_seed(Workload::Concert, &[37; 32]).unwrap()
}
fn dedicated() -> ActorDelegationVerifier {
    RelationshipReadVerifier::new(Workload::Auth, &[read_signer().public_key()], &gateway())
        .unwrap()
        .into_inner()
}
fn frame(actor: Uuid) -> Vec<u8> {
    let text = actor.to_string();
    let mut message = vec![10, text.len() as u8];
    message.extend(text.bytes());
    let mut body = vec![0];
    body.extend_from_slice(&(message.len() as u32).to_be_bytes());
    body.extend(message);
    body
}
#[derive(Clone, Default)]
struct Owner(Arc<AtomicUsize>);
impl NamedService for Owner {
    const NAME: &'static str = AUTH;
}
impl tower::Service<http::Request<Body>> for Owner {
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = std::future::Ready<Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, r: http::Request<Body>) -> Self::Future {
        assert!(r.extensions().get::<VerifiedActor>().is_some());
        assert!(r
            .extensions()
            .get::<crate::actor_delegation::read_authority::VerifiedReadAuthority>()
            .is_some());
        self.0.fetch_add(1, Ordering::SeqCst);
        std::future::ready(Ok(http::Response::new(Body::default())))
    }
}
fn request(body: Vec<u8>, token: Option<String>) -> http::Request<Body> {
    let mut b = http::Request::builder().uri(METHOD);
    if let Some(token) = token {
        b = b.header(HEADER, token);
    }
    let mut r = b
        .body(Body::new(http_body_util::Full::new(Bytes::from(body))))
        .unwrap();
    r.extensions_mut().insert(VerifiedCaller(Workload::Concert));
    r
}
static POLICY: MethodPolicy = MethodPolicy {
    service: AUTH,
    workload: Some(Workload::Auth),
    methods: &[("ReadCurrent", &[Workload::Concert])],
    actor_fields: &[("ReadCurrent", &[&[1]])],
};
#[tokio::test]
async fn missing_wrong_tampered_replayed_and_expired_read_proofs_never_dispatch() {
    let owner = Owner::default();
    let count = owner.0.clone();
    let service = POLICY.wrap(owner, dedicated()).unwrap();
    let actor = Uuid::new_v4();
    let body = frame(actor);
    let now = chrono::Utc::now().timestamp();
    let token = read_signer().issue(Workload::Auth, METHOD, actor, None, &body, now);
    let call = |r| {
        crate::call_deadline::scope(
            std::time::Instant::now() + Duration::from_secs(1),
            service.clone().oneshot(r),
        )
    };
    let result = call(request(body.clone(), Some(token.clone())))
        .await
        .unwrap();
    assert!(!result.headers().contains_key("grpc-status"));
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let wrong = ActorDelegationSigner::from_seed(Workload::Concert, &[17; 32]).unwrap();
    for r in [
        request(body.clone(), None),
        request(body.clone(), Some(token)),
        request(
            body.clone(),
            Some(wrong.issue(Workload::Auth, METHOD, actor, None, &body, now)),
        ),
        request(
            frame(Uuid::new_v4()),
            Some(read_signer().issue(Workload::Auth, METHOD, actor, None, &body, now)),
        ),
        request(
            body.clone(),
            Some(read_signer().issue(Workload::Auth, METHOD, Uuid::new_v4(), None, &body, now)),
        ),
        request(
            body.clone(),
            Some(read_signer().issue(Workload::Friend, METHOD, actor, None, &body, now)),
        ),
        request(
            body.clone(),
            Some(read_signer().issue(
                Workload::Auth,
                "/wrong/ReadCurrent",
                actor,
                None,
                &body,
                now,
            )),
        ),
        request(
            body.clone(),
            Some(read_signer().issue(Workload::Auth, METHOD, actor, None, &body, now - 120)),
        ),
    ] {
        let result = call(r).await.unwrap();
        assert!(result.headers().contains_key("grpc-status"));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}
#[test]
fn relationship_key_cannot_authorize_gateway_rpc_and_gateway_key_cannot_authorize_read() {
    let actor = Uuid::new_v4();
    let body = frame(actor);
    let now = chrono::Utc::now().timestamp();
    let old_method = "/auth.v1.AuthService/GetCurrentAdmission";
    // Deliberately forge the other issuer using each key: signature-domain trust,
    // not just the issuer string, must prevent cross-domain authorization.
    let read_key_as_gateway =
        ActorDelegationSigner::from_seed(Workload::Gateway, &[37; 32]).unwrap();
    let token = read_key_as_gateway.issue(Workload::Auth, old_method, actor, None, &body, now);
    assert!(gateway()
        .verify(
            &token,
            &Expected {
                caller: Workload::Gateway,
                method: old_method,
                body: &body,
                operation: None
            },
            now
        )
        .is_err());
    let gateway_key_as_concert =
        ActorDelegationSigner::from_seed(Workload::Concert, &[41; 32]).unwrap();
    let token = gateway_key_as_concert.issue(Workload::Auth, METHOD, actor, None, &body, now);
    assert!(dedicated()
        .verify(
            &token,
            &Expected {
                caller: Workload::Concert,
                method: METHOD,
                body: &body,
                operation: None
            },
            now
        )
        .is_err());
    assert!(RelationshipReadSigner::from_seed(&[41; 32], &gateway()).is_err());
}
