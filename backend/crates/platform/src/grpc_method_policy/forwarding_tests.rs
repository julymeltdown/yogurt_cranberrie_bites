use super::*;
use crate::actor_delegation::exchange::{
    ChildRequest, DelegationExchange, ParentRequest, ResourceEvidenceVerifier,
};
use crate::actor_delegation::{ActorDelegationSigner, VerifiedActor, HEADER};
use http_body_util::Full;
use std::convert::Infallible;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use uuid::Uuid;

const ROOT: &str = "/post.v1.PostService/CreatePost";
const CHILD: &str = "/authority.v1.ProfileAuthorAuthorityService/CheckAuthorProfile";
static POLICY: MethodPolicy = MethodPolicy {
    service: "authority.v1.ProfileAuthorAuthorityService",
    workload: Some(Workload::Profile),
    methods: &[
        ("CheckAuthorProfile", &[Workload::Gateway, Workload::Post]),
        ("Other", &[Workload::Post]),
    ],
    actor_fields: &[],
};
static FORWARDING: &[ForwardedMethod] = &[ForwardedMethod {
    method: "CheckAuthorProfile",
    caller: Workload::Post,
    actor_fields: &[], // user_id is a target resource, not a viewer field.
}];

#[derive(Clone, Default)]
struct Owner {
    effects: Arc<AtomicUsize>,
    actors: Arc<Mutex<Vec<Option<Uuid>>>>,
}
impl NamedService for Owner {
    const NAME: &'static str = "authority.v1.ProfileAuthorAuthorityService";
}
impl tower::Service<http::Request<Body>> for Owner {
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = std::future::Ready<Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        self.effects.fetch_add(1, Ordering::SeqCst);
        self.actors.lock().unwrap().push(
            request
                .extensions()
                .get::<VerifiedActor>()
                .map(|actor| actor.0),
        );
        std::future::ready(Ok(http::Response::new(Body::default())))
    }
}
fn frame(actor: Uuid) -> Vec<u8> {
    let mut message = vec![10, 36];
    message.extend(actor.to_string().as_bytes());
    let mut body = vec![0];
    body.extend(u32::try_from(message.len()).unwrap().to_be_bytes());
    body.extend(message);
    body
}
fn issued(actor: Uuid, at: i64) -> String {
    let signer = ActorDelegationSigner::local_parity(Workload::Gateway);
    let body = frame(actor);
    let root = signer.issue(Workload::Post, ROOT, actor, None, &body, at);
    let broker = DelegationExchange::new(
        signer,
        ActorDelegationVerifier::local_parity(Workload::Post),
        ResourceEvidenceVerifier::new(&[[11; 32]]).unwrap(),
    )
    .unwrap();
    let prepared = broker
        .prepare(
            Workload::Post,
            ParentRequest {
                token: &root,
                method: ROOT,
                body: &body,
                operation: None,
            },
            ChildRequest {
                audience: Workload::Profile,
                method: CHILD,
                body: &body,
                operation: None,
            },
            at,
        )
        .unwrap();
    broker.finish(prepared, None, at).unwrap().token
}
fn request(caller: Workload, body: Vec<u8>, token: Option<&str>) -> http::Request<Body> {
    let mut request = http::Request::builder()
        .uri(CHILD)
        .body(Body::new(Full::new(Bytes::from(body))))
        .unwrap();
    request.extensions_mut().insert(VerifiedCaller(caller));
    if let Some(token) = token {
        request.headers_mut().insert(HEADER, token.parse().unwrap());
    }
    request
}
async fn status(
    service: &MethodPolicyService<Owner>,
    request: http::Request<Body>,
) -> Option<String> {
    service
        .clone()
        .oneshot(request)
        .await
        .unwrap()
        .headers()
        .get("grpc-status")
        .map(|value| value.to_str().unwrap().to_owned())
}

#[tokio::test]
async fn proof_requirement_is_independent_of_actor_fields_and_has_no_fallback() {
    let owner = Owner::default();
    let service = POLICY
        .wrap_with_forwarding(
            owner.clone(),
            ActorDelegationVerifier::local_parity(Workload::Profile),
            FORWARDING,
        )
        .unwrap();
    let actor = Uuid::new_v4();
    let now = chrono::Utc::now().timestamp();
    let body = frame(actor);
    let v1 = ActorDelegationSigner::local_parity(Workload::Gateway).issue(
        Workload::Profile,
        CHILD,
        actor,
        None,
        &body,
        now,
    );
    for proof in [None, Some("garbage"), Some(v1.as_str())] {
        assert_eq!(
            status(&service, request(Workload::Post, body.clone(), proof)).await,
            Some("16".into())
        );
        assert_eq!(owner.effects.load(Ordering::SeqCst), 0);
    }
    let valid = issued(actor, now);
    let mut duplicate = request(Workload::Post, body.clone(), Some(&valid));
    duplicate
        .headers_mut()
        .append(HEADER, valid.parse().unwrap());
    assert_eq!(status(&service, duplicate).await, Some("16".into()));
    let modified_body = frame(Uuid::new_v4());
    assert_eq!(
        status(
            &service,
            request(Workload::Post, modified_body, Some(&valid))
        )
        .await,
        Some("7".into())
    );
    let expired = issued(actor, now - 100);
    assert_eq!(
        status(
            &service,
            request(Workload::Post, body.clone(), Some(&expired))
        )
        .await,
        Some("16".into())
    );
    assert_eq!(owner.effects.load(Ordering::SeqCst), 0);
    // Failed validation does not burn the proof's replay identity.
    assert_eq!(
        status(
            &service,
            request(Workload::Post, body.clone(), Some(&valid))
        )
        .await,
        None
    );
    assert_eq!(owner.effects.load(Ordering::SeqCst), 1);
    assert_eq!(owner.actors.lock().unwrap().as_slice(), &[Some(actor)]);
    assert_eq!(
        status(&service, request(Workload::Post, body, Some(&valid))).await,
        Some("16".into())
    );
    assert_eq!(owner.effects.load(Ordering::SeqCst), 1);
    // Direct Gateway read behavior remains governed by its existing policy.
    assert_eq!(
        status(&service, request(Workload::Gateway, frame(actor), None)).await,
        None
    );
}

#[test]
fn registration_rejects_unknown_unlisted_duplicate_and_unsupported_presenters() {
    static BAD: [&[ForwardedMethod]; 4] = [
        &[ForwardedMethod {
            method: "Unknown",
            caller: Workload::Post,
            actor_fields: &[],
        }],
        &[ForwardedMethod {
            method: "CheckAuthorProfile",
            caller: Workload::Friend,
            actor_fields: &[],
        }],
        &[ForwardedMethod {
            method: "CheckAuthorProfile",
            caller: Workload::Post,
            actor_fields: &[&[]],
        }],
        &[
            ForwardedMethod {
                method: "CheckAuthorProfile",
                caller: Workload::Post,
                actor_fields: &[],
            },
            ForwardedMethod {
                method: "CheckAuthorProfile",
                caller: Workload::Post,
                actor_fields: &[],
            },
        ],
    ];
    for entries in BAD {
        assert!(POLICY
            .wrap_with_forwarding(
                Owner::default(),
                ActorDelegationVerifier::local_parity(Workload::Profile),
                entries
            )
            .is_err());
    }
    assert!(POLICY
        .wrap_with_forwarding(
            Owner::default(),
            ActorDelegationVerifier::local_parity(Workload::Friend),
            FORWARDING
        )
        .is_err());
}

#[tokio::test]
async fn admitted_post_handlers_keep_their_sealed_parent_during_every_poll() {
    use crate::actor_delegation::exchange::VerifiedParentAuthority;
    static ROOT_POLICY: MethodPolicy = MethodPolicy {
        service: "post.v1.PostService",
        workload: Some(Workload::Post),
        methods: &[("CreatePost", &[Workload::Gateway])],
        actor_fields: &[("CreatePost", &[&[1]])],
    };
    #[derive(Clone)]
    struct PostHandler;
    impl NamedService for PostHandler {
        const NAME: &'static str = "post.v1.PostService";
    }
    impl tower::Service<http::Request<Body>> for PostHandler {
        type Response = http::Response<Body>;
        type Error = Infallible;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, request: http::Request<Body>) -> Self::Future {
            let sealed = request
                .extensions()
                .get::<VerifiedParentAuthority>()
                .cloned()
                .unwrap();
            let actor = request.extensions().get::<VerifiedActor>().unwrap().0;
            Box::pin(async move {
                for _ in 0..3 {
                    tokio::task::yield_now().await;
                    let current = VerifiedParentAuthority::current().unwrap();
                    assert_eq!(current.actor(), actor);
                    assert_eq!(current.request().token, sealed.request().token);
                    assert_eq!(current.request().method, ROOT);
                }
                Ok(http::Response::new(Body::default()))
            })
        }
    }
    let signer = ActorDelegationSigner::local_parity(Workload::Gateway);
    let service = ROOT_POLICY
        .wrap(
            PostHandler,
            ActorDelegationVerifier::local_parity(Workload::Post),
        )
        .unwrap();
    let make = |actor| {
        let body = frame(actor);
        let token = signer.issue(
            Workload::Post,
            ROOT,
            actor,
            None,
            &body,
            chrono::Utc::now().timestamp(),
        );
        let mut request = request(Workload::Gateway, body, Some(&token));
        *request.uri_mut() = ROOT.parse().unwrap();
        service.clone().oneshot(request)
    };
    let (a, b) = tokio::join!(make(Uuid::new_v4()), make(Uuid::new_v4()));
    assert!(a.unwrap().headers().get("grpc-status").is_none());
    assert!(b.unwrap().headers().get("grpc-status").is_none());
    assert!(VerifiedParentAuthority::current().is_none());
}
