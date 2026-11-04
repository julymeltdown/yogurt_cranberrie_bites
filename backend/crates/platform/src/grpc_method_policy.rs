//! Default-deny authorization of internal gRPC methods by full method path.
//!
//! Every service registers one [`MethodPolicy`] that lists each RPC it serves
//! with the workloads allowed to call it. The policy runs after caller
//! authentication and before the generated service routes the request, so a
//! handler that forgets its own check is still covered, and an RPC missing from
//! the table is denied.
//!
//! Methods that act for a user also declare the request fields naming that
//! user. For those, the guard verifies the Gateway's signed actor delegation
//! (see [crate::actor_delegation]) and requires every declared field to name
//! the delegated user before the handler runs.

use crate::actor_delegation::{self, ActorDelegationVerifier};
use crate::grpc_auth::{
    CallerRegistry, RequireRegisteredCallerInterceptor, VerifiedCaller, Workload,
};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tonic::body::Body;
use tonic::codegen::Bytes;
use tonic::server::NamedService;
use tonic::service::interceptor::InterceptedService;
use tonic::Status;

/// A generated server behind caller authentication and its method policy.
pub type GuardedService<S> =
    InterceptedService<MethodPolicyService<S>, RequireRegisteredCallerInterceptor>;

/// Authenticate the caller, then enforce `policy` before `server` routes the
/// request. Every internal gRPC service is registered through this function.
pub fn guard<S: NamedService>(
    policy: &'static MethodPolicy,
    server: S,
    registry: CallerRegistry,
    verifier: ActorDelegationVerifier,
) -> anyhow::Result<GuardedService<S>> {
    Ok(InterceptedService::new(
        policy.wrap(server, verifier)?,
        RequireRegisteredCallerInterceptor::new(registry)?,
    ))
}

/// Registration with explicit child-proof requirements for sibling reads.
/// Empty actor fields do not remove the proof requirement; they mean the
/// request names a resource rather than the acting user.
pub fn guard_with_forwarding<S: NamedService>(
    policy: &'static MethodPolicy,
    forwarding: &'static [ForwardedMethod],
    server: S,
    registry: CallerRegistry,
    verifier: ActorDelegationVerifier,
) -> anyhow::Result<GuardedService<S>> {
    Ok(InterceptedService::new(
        policy.wrap_with_forwarding(server, verifier, forwarding)?,
        RequireRegisteredCallerInterceptor::new(registry)?,
    ))
}

/// [guard] for a service whose methods do not act for a user, such as a newly
/// generated service. The policy must declare no actor fields, so no method can
/// rely on a delegation that this registration would not verify.
pub fn guard_without_delegation<S: NamedService>(
    policy: &'static MethodPolicy,
    server: S,
    registry: CallerRegistry,
) -> anyhow::Result<GuardedService<S>> {
    Ok(InterceptedService::new(
        policy.wrap_without_delegation(server)?,
        RequireRegisteredCallerInterceptor::new(registry)?,
    ))
}

/// Request fields naming the acting user, as protobuf field-number paths
/// (outermost first). Every listed field that is present must name the
/// delegated user, and at least one must be present. List one path per
/// variant for a oneof command.
pub type ActorFields = &'static [&'static [u32]];

/// One caller/method pair that always requires Gateway-issued child authority.
/// Only Post is currently supported by the child-token protocol. Background
/// work must use a separately authorized method, not omission of this proof.
#[derive(Debug)]
pub struct ForwardedMethod {
    pub method: &'static str,
    pub caller: Workload,
    pub actor_fields: ActorFields,
}

/// One service's complete method table.
#[derive(Debug)]
pub struct MethodPolicy {
    /// Fully qualified gRPC service name, for example `friend.v1.FriendService`.
    pub service: &'static str,
    /// Workload serving this service; a delegation must name it as audience.
    /// None for a service outside the fixed workload set, which can then
    /// register only through [guard_without_delegation].
    pub workload: Option<Workload>,
    /// Every method the service serves, with the workloads allowed to call it.
    /// An empty caller list denies the method to every workload.
    pub methods: &'static [(&'static str, &'static [Workload])],
    /// Methods that act for a user. Each must be Gateway-only, because the
    /// Gateway is the only delegation issuer.
    pub actor_fields: &'static [(&'static str, ActorFields)],
}

impl MethodPolicy {
    fn method_of<'a>(&self, path: &'a str) -> Option<&'a str> {
        path.strip_prefix('/')
            .and_then(|rest| rest.strip_prefix(self.service))
            .and_then(|rest| rest.strip_prefix('/'))
    }

    /// Whether `caller` may invoke the method addressed by an HTTP/2 `path`
    /// of the form `/{service}/{method}`. Unknown services and methods deny.
    pub fn allows(&self, path: &str, caller: Workload) -> bool {
        let Some(method) = self.method_of(path) else {
            return false;
        };
        self.methods
            .iter()
            .any(|(name, callers)| *name == method && callers.contains(&caller))
    }

    /// Actor fields for the method at `path`, when it acts for a user.
    pub fn actor_fields_for(&self, path: &str) -> Option<ActorFields> {
        let method = self.method_of(path)?;
        self.actor_fields
            .iter()
            .find(|(name, _)| *name == method)
            .map(|(_, fields)| *fields)
    }

    /// Method names in table order.
    pub fn method_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.methods.iter().map(|(name, _)| *name)
    }

    /// Workloads allowed to call `method`, or `None` when it is not listed.
    pub fn callers(&self, method: &str) -> Option<&'static [Workload]> {
        self.methods
            .iter()
            .find(|(name, _)| *name == method)
            .map(|(_, callers)| *callers)
    }

    /// Wrap a generated server. The policy must name the same service, and
    /// the verifier must accept delegations addressed to this workload.
    pub fn wrap<S: NamedService>(
        &'static self,
        inner: S,
        verifier: ActorDelegationVerifier,
    ) -> anyhow::Result<MethodPolicyService<S>> {
        anyhow::ensure!(
            self.workload == Some(verifier.audience()),
            "actor delegation verifier for {:?} cannot guard {}",
            verifier.audience(),
            self.service
        );
        self.validate(S::NAME)?;
        Ok(MethodPolicyService {
            inner,
            policy: self,
            verifier: Some(verifier),
            forwarding: &[],
        })
    }

    pub fn wrap_with_forwarding<S: NamedService>(
        &'static self,
        inner: S,
        verifier: ActorDelegationVerifier,
        forwarding: &'static [ForwardedMethod],
    ) -> anyhow::Result<MethodPolicyService<S>> {
        let mut service = self.wrap(inner, verifier)?;
        let mut pairs = std::collections::HashSet::new();
        for entry in forwarding {
            anyhow::ensure!(
                entry.caller == Workload::Post,
                "child authority supports only the Post presenter"
            );
            anyhow::ensure!(
                self.callers(entry.method)
                    .is_some_and(|callers| callers.contains(&entry.caller)),
                "forwarding policy names an unlisted caller or method"
            );
            anyhow::ensure!(
                pairs.insert((entry.method, entry.caller)),
                "forwarding policy lists a caller/method twice"
            );
            anyhow::ensure!(
                entry.actor_fields.iter().all(|path| !path.is_empty()),
                "forwarding policy declares an empty actor field path"
            );
        }
        service.forwarding = forwarding;
        Ok(service)
    }

    /// Wrap a generated server whose methods never act for a user.
    pub fn wrap_without_delegation<S: NamedService>(
        &'static self,
        inner: S,
    ) -> anyhow::Result<MethodPolicyService<S>> {
        anyhow::ensure!(
            self.actor_fields.is_empty(),
            "method policy for {} declares actor fields and needs a delegation verifier",
            self.service
        );
        self.validate(S::NAME)?;
        Ok(MethodPolicyService {
            inner,
            policy: self,
            verifier: None,
            forwarding: &[],
        })
    }

    fn validate(&self, served: &str) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.service == served,
            "method policy for {} cannot guard {}",
            self.service,
            served
        );
        let mut names: Vec<&str> = self.method_names().collect();
        names.sort_unstable();
        let unique = names.len();
        names.dedup();
        anyhow::ensure!(
            names.len() == unique,
            "method policy for {} lists a method twice",
            self.service
        );
        let mut actor_methods: Vec<&str> =
            self.actor_fields.iter().map(|(name, _)| *name).collect();
        actor_methods.sort_unstable();
        let unique = actor_methods.len();
        actor_methods.dedup();
        anyhow::ensure!(
            actor_methods.len() == unique,
            "method policy for {} declares actor fields twice",
            self.service
        );
        // Only exact reserved observation services accept a dedicated Concert
        // issuer. Existing actor-bound methods remain Gateway-only.
        let reserved_relationship = matches!(
            self.service,
            "relationship.v1.AuthConcertRelationshipService"
                | "relationship.v1.FriendConcertRelationshipService"
        );
        let relationship_read = matches!(
            (self.service, self.workload),
            (
                "relationship.v1.AuthConcertRelationshipService",
                Some(Workload::Auth)
            ) | (
                "relationship.v1.FriendConcertRelationshipService",
                Some(Workload::Friend)
            )
        ) && self.methods.len() == 1
            && self.methods[0].0 == "ReadCurrent"
            && self.methods[0].1 == [Workload::Concert]
            && self.actor_fields.len() == 1
            && self.actor_fields[0].0 == "ReadCurrent"
            && self.actor_fields[0].1.len() == 1
            && self.actor_fields[0].1[0] == [1];
        anyhow::ensure!(!reserved_relationship || relationship_read,
            "reserved relationship read policy must match its exact owner, method, caller and actor field");
        let reserved_profile = self.service == "profile.v1.ConcertWorldProfileService";
        let world_profile_read = reserved_profile
            && self.workload == Some(Workload::Profile)
            && self.methods.len() == 1
            && self.methods[0].0 == "ReadCurrent"
            && self.methods[0].1 == [Workload::Concert]
            && self.actor_fields.len() == 1
            && self.actor_fields[0].0 == "ReadCurrent"
            && self.actor_fields[0].1.len() == 1
            && self.actor_fields[0].1[0] == [1];
        anyhow::ensure!(
            !reserved_profile || world_profile_read,
            "reserved world Profile policy must match exact owner, method, caller and actor field"
        );
        let reserved_public_profile =
            self.service == "profile.concert_public.v1.ConcertPublicProfileService";
        let public_profile_read = reserved_public_profile
            && self.workload == Some(Workload::Profile)
            && self.methods
                == [
                    ("ReadCurrent", &[Workload::Concert][..]),
                    ("Check", &[Workload::Concert][..]),
                    ("Complete", &[Workload::Concert][..]),
                ]
            && self.actor_fields.len() == 3
            && self
                .actor_fields
                .iter()
                .zip(["ReadCurrent", "Check", "Complete"])
                .all(|((method, fields), expected)| *method == expected && *fields == [&[1][..]]);
        anyhow::ensure!(!reserved_public_profile || public_profile_read,
            "reserved public Profile policy must match exact owner, methods, caller and actor fields");
        let reserved_post_read = self.service == "post.concert.v1.PostConcertFeedReadService";
        let qualified_post_read = reserved_post_read
            && self.workload == Some(Workload::Post)
            && self.methods.len() == 4
            && self.actor_fields.len() == 4
            && self
                .methods
                .iter()
                .zip(self.actor_fields)
                .zip(["GetPost", "ListPosts", "GetComment", "ListComments"])
                .all(|(((method, callers), (actor_method, fields)), expected)| {
                    *method == expected
                        && *actor_method == expected
                        && *callers == [Workload::Concert]
                        && *fields == [&[1, 1, 1][..]]
                });
        anyhow::ensure!(!reserved_post_read || qualified_post_read,
            "reserved qualified Post read policy must match exact owner, methods, caller and actor fields");
        let reserved_owner = matches!(
            self.service,
            "authority.lease.v1.AuthOwnerAuthorityService"
                | "authority.lease.v1.FriendOwnerAuthorityService"
        );
        let owner_authority = reserved_owner
            && matches!(
                (self.service, self.workload),
                (
                    "authority.lease.v1.AuthOwnerAuthorityService",
                    Some(Workload::Auth)
                ) | (
                    "authority.lease.v1.FriendOwnerAuthorityService",
                    Some(Workload::Friend)
                )
            )
            && self.methods
                == [
                    ("Acquire", &[Workload::Concert][..]),
                    (
                        "Check",
                        &[Workload::Concert, Workload::Post, Workload::Profile][..],
                    ),
                    (
                        "Enroll",
                        &[Workload::Concert, Workload::Post, Workload::Profile][..],
                    ),
                    (
                        "Track",
                        &[Workload::Concert, Workload::Post, Workload::Profile][..],
                    ),
                    ("Complete", &[Workload::Concert][..]),
                ]
            && self.actor_fields == [("Acquire", &[&[1, 1][..]][..])];
        anyhow::ensure!(!reserved_owner || owner_authority,
            "reserved owner authority policy must match exact owner, methods, callers and Acquire actor field");
        let reserved_settlement = self.service == "authority.lease.v1.ProducerSettlementService";
        let settlement = reserved_settlement
            && matches!(
                self.workload,
                Some(Workload::Concert | Workload::Post | Workload::Profile)
            )
            && self.methods
                == [
                    (
                        "ResolveTransaction",
                        &[Workload::Auth, Workload::Friend][..],
                    ),
                    ("ResolveScope", &[Workload::Auth, Workload::Friend][..]),
                ]
            && self.actor_fields.is_empty();
        anyhow::ensure!(
            !reserved_settlement || settlement,
            "reserved producer settlement policy accepts only Auth/Friend system queries"
        );
        let reserved_decision =
            self.service == "concert.participant.v1.ConcertParticipantDecisionService";
        let decision = reserved_decision
            && self.workload == Some(Workload::Concert)
            && self.methods == [("ResolveDecision", &[Workload::Profile][..])]
            && self.actor_fields == [("ResolveDecision", &[&[1][..]][..])];
        anyhow::ensure!(
            !reserved_decision || decision,
            "reserved Participant decision resolver requires exact signed Profile query"
        );
        let reserved_root = self.service == "authority.lease.v1.ConcertRootReadService";
        let root_read = reserved_root
            && self.workload == Some(Workload::Concert)
            && self.methods == [("CheckSubject", &[Workload::Post][..])]
            && self.actor_fields == [("CheckSubject", &[&[1, 3, 1][..]][..])];
        anyhow::ensure!(
            !reserved_root || root_read,
            "reserved root read callback requires exact Post-bound envelope"
        );
        let reserved_delivery = self.service == "concert.product.v1.ProductDeliveryService";
        let delivery = reserved_delivery
            && self.workload == Some(Workload::Concert)
            && self.methods == [("CompleteResponse", &[Workload::Gateway][..])]
            && self.actor_fields.is_empty();
        anyhow::ensure!(
            !reserved_delivery || delivery,
            "reserved delivery completion requires exact Gateway capability method"
        );
        let reserved_self = self.service == "profile.self.v1.ConcertSelfProfileService";
        let self_profile = reserved_self
            && self.workload == Some(Workload::Profile)
            && self.methods
                == [
                    ("Read", &[Workload::Concert][..]),
                    ("PrepareUpdate", &[Workload::Concert][..]),
                    ("ReadOperation", &[Workload::Concert][..]),
                    ("Reconcile", &[Workload::Concert][..]),
                ]
            && self.actor_fields
                == [
                    ("Read", &[&[1, 1, 1][..], &[2][..]][..]),
                    ("PrepareUpdate", &[&[1, 1, 1][..], &[2][..]][..]),
                    ("ReadOperation", &[&[1, 1, 1][..]][..]),
                    ("Reconcile", &[&[1, 1, 1][..]][..]),
                ];
        anyhow::ensure!(
            !reserved_self || self_profile,
            "reserved SELF Profile policy requires exact Concert-signed scope and subjects"
        );
        for (method, fields) in self.actor_fields {
            anyhow::ensure!(
                self.callers(method) == Some(&[Workload::Gateway][..])
                    || relationship_read
                    || world_profile_read
                    || public_profile_read
                    || qualified_post_read
                    || owner_authority
                    || decision
                    || root_read
                    || self_profile,
                "{}/{method} acts for a user and must accept only the Gateway",
                self.service
            );
            anyhow::ensure!(
                !fields.is_empty() && fields.iter().all(|path| !path.is_empty()),
                "{}/{method} declares an empty actor field path",
                self.service
            );
        }
        Ok(())
    }
}

/// Generated-server wrapper that enforces a [`MethodPolicy`].
#[derive(Clone)]
pub struct MethodPolicyService<S> {
    inner: S,
    policy: &'static MethodPolicy,
    verifier: Option<ActorDelegationVerifier>,
    forwarding: &'static [ForwardedMethod],
}

impl<S: NamedService> NamedService for MethodPolicyService<S> {
    const NAME: &'static str = S::NAME;
}

type BoxedResponse<R, E> = Pin<Box<dyn Future<Output = Result<R, E>> + Send>>;

impl<S, B, ResBody> tower::Service<http::Request<B>> for MethodPolicyService<S>
where
    S: tower::Service<http::Request<Body>, Response = http::Response<ResBody>>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    B: http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    ResBody: Default + Send + 'static,
{
    type Response = http::Response<ResBody>;
    type Error = S::Error;
    type Future = BoxedResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, request: http::Request<B>) -> Self::Future {
        let caller = request
            .extensions()
            .get::<VerifiedCaller>()
            .map(|caller| caller.0);
        match caller {
            Some(caller) if self.policy.allows(request.uri().path(), caller) => {
                let forwarded = self.forwarding.iter().find(|entry| {
                    entry.caller == caller
                        && self.policy.method_of(request.uri().path()) == Some(entry.method)
                });
                let fields = forwarded
                    .map(|entry| entry.actor_fields)
                    .or_else(|| self.policy.actor_fields_for(request.uri().path()));
                let Some(fields) = fields else {
                    return Box::pin(self.inner.call(request.map(Body::new)));
                };
                let forwarded = forwarded.is_some();
                // The ready service moves into the future; a fresh clone stays
                // behind for the next poll_ready.
                let clone = self.inner.clone();
                let mut inner = std::mem::replace(&mut self.inner, clone);
                let Some(verifier) = self.verifier.clone() else {
                    // Validation rejects this combination at registration.
                    let response =
                        Status::permission_denied("service has no actor delegation verifier")
                            .into_http();
                    return Box::pin(async move { Ok(response) });
                };
                let service = self.policy.service;
                Box::pin(async move {
                    let path = request.uri().path().to_owned();
                    let checked = if forwarded {
                        actor_delegation::verify_forwarded_request(
                            request, caller, fields, &verifier,
                        )
                        .await
                    } else {
                        actor_delegation::verify_request(request, caller, fields, &verifier).await
                    };
                    match checked {
                        Ok(request) => {
                            let parent = request
                                .extensions()
                                .get::<actor_delegation::exchange::VerifiedParentAuthority>()
                                .cloned();
                            let future = inner.call(request);
                            match parent {
                                Some(parent) => parent.scope(future).await,
                                None => future.await,
                            }
                        }
                        Err(denial) => {
                            tracing::warn!(
                                caller = caller.as_str(),
                                service,
                                path,
                                code = denial.code(),
                                "actor delegation rejected"
                            );
                            Ok(denial.status().into_http())
                        }
                    }
                })
            }
            Some(caller) => {
                tracing::warn!(
                    caller = caller.as_str(),
                    service = self.policy.service,
                    path = request.uri().path(),
                    code = "RPC_METHOD_DENIED",
                    "internal caller is not authorized for this method"
                );
                let response =
                    Status::permission_denied("service is not authorized for this operation")
                        .into_http();
                Box::pin(async move { Ok(response) })
            }
            None => {
                let response =
                    Status::unauthenticated("verified service identity required").into_http();
                Box::pin(async move { Ok(response) })
            }
        }
    }
}

/// RPC names declared by a `.proto` source, for policy completeness tests.
pub fn proto_rpc_names(proto_source: &str) -> Vec<String> {
    proto_source
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("rpc "))
        .filter_map(|rest| rest.split(|c: char| c == '(' || c.is_whitespace()).next())
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Check a policy's actor fields against encoded sample requests built from
/// the service's generated types, for table tests. Each sample names
/// `actor` in every actor field it sets. Every actor-bound method needs at
/// least one sample, must be Gateway-only, and must accept its sample for
/// `actor` and reject it for any other user.
pub fn check_actor_samples(
    policy: &MethodPolicy,
    samples: &[(&str, Vec<u8>)],
    actor: uuid::Uuid,
) -> Result<(), String> {
    let other = uuid::Uuid::new_v4();
    for (method, fields) in policy.actor_fields {
        if !samples.iter().any(|(name, _)| name == method) {
            return Err(format!("{method} has no sample request"));
        }
        if policy.callers(method) != Some(&[Workload::Gateway][..]) {
            return Err(format!("{method} acts for a user but is not Gateway-only"));
        }
        let _ = fields;
    }
    for (method, message) in samples {
        let fields = policy
            .actor_fields
            .iter()
            .find(|(name, _)| name == method)
            .map(|(_, fields)| *fields)
            .ok_or_else(|| format!("{method} has a sample but no actor fields"))?;
        actor_delegation::check_actor_fields(message, fields, actor)
            .map_err(|denial| format!("{method}: {}", denial.code()))?;
        if actor_delegation::check_actor_fields(message, fields, other).is_ok() {
            return Err(format!("{method} accepted a delegation for another user"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod forwarding_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor_delegation::{ActorDelegationSigner, VerifiedActor, HEADER};
    use std::convert::Infallible;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;
    use uuid::Uuid;

    static POLICY: MethodPolicy = MethodPolicy {
        service: "demo.v1.DemoService",
        workload: Some(Workload::Post),
        methods: &[
            ("Read", &[Workload::Gateway, Workload::Post]),
            ("Write", &[Workload::Gateway]),
            ("Retired", &[]),
        ],
        actor_fields: &[("Write", &[&[1], &[2, 1]])],
    };

    #[derive(Clone, Default)]
    struct Demo {
        calls: Arc<AtomicUsize>,
        actors: Arc<Mutex<Vec<Option<Uuid>>>>,
    }

    impl NamedService for Demo {
        const NAME: &'static str = "demo.v1.DemoService";
    }

    impl tower::Service<http::Request<Body>> for Demo {
        type Response = http::Response<Body>;
        type Error = Infallible;
        type Future = std::future::Ready<Result<Self::Response, Self::Error>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, request: http::Request<Body>) -> Self::Future {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.actors.lock().unwrap().push(
                request
                    .extensions()
                    .get::<VerifiedActor>()
                    .map(|actor| actor.0),
            );
            std::future::ready(Ok(http::Response::new(Body::default())))
        }
    }

    fn verifier() -> ActorDelegationVerifier {
        ActorDelegationVerifier::local_parity(Workload::Post)
    }

    fn request(path: &str, caller: Option<Workload>) -> http::Request<Body> {
        request_with(path, caller, Vec::new(), None)
    }

    fn request_with(
        path: &str,
        caller: Option<Workload>,
        body: Vec<u8>,
        delegation: Option<String>,
    ) -> http::Request<Body> {
        let mut builder = http::Request::builder().uri(path);
        if let Some(token) = delegation {
            builder = builder.header(HEADER, token);
        }
        let mut request = builder
            .body(Body::new(http_body_util::Full::new(Bytes::from(body))))
            .unwrap();
        if let Some(caller) = caller {
            request.extensions_mut().insert(VerifiedCaller(caller));
        }
        request
    }

    async fn send(
        service: &MethodPolicyService<Demo>,
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

    async fn grpc_status(path: &str, caller: Option<Workload>) -> (Option<String>, usize) {
        let demo = Demo::default();
        let service = POLICY.wrap(demo.clone(), verifier()).unwrap();
        let status = send(&service, request(path, caller)).await;
        (status, demo.calls.load(Ordering::SeqCst))
    }

    fn frame(message: &[u8]) -> Vec<u8> {
        let mut body = vec![0];
        body.extend_from_slice(&u32::try_from(message.len()).unwrap().to_be_bytes());
        body.extend_from_slice(message);
        body
    }

    fn field(number: u8, value: &[u8]) -> Vec<u8> {
        let mut bytes = vec![(number << 3) | 2, u8::try_from(value.len()).unwrap()];
        bytes.extend_from_slice(value);
        bytes
    }

    #[tokio::test]
    async fn listed_caller_reaches_the_handler() {
        assert_eq!(
            grpc_status("/demo.v1.DemoService/Read", Some(Workload::Post)).await,
            (None, 1)
        );
    }

    #[tokio::test]
    async fn unlisted_callers_and_methods_are_denied_before_the_handler() {
        let denied = Some("7".to_owned());
        for (path, caller) in [
            ("/demo.v1.DemoService/Write", Workload::Post),
            ("/demo.v1.DemoService/Retired", Workload::Gateway),
            ("/demo.v1.DemoService/Unknown", Workload::Gateway),
            ("/other.v1.OtherService/Read", Workload::Gateway),
            ("/demo.v1.DemoService/Read/extra", Workload::Gateway),
        ] {
            assert_eq!(
                grpc_status(path, Some(caller)).await,
                (denied.clone(), 0),
                "{path} for {caller:?}"
            );
        }
    }

    #[tokio::test]
    async fn a_request_without_a_verified_caller_is_unauthenticated() {
        assert_eq!(
            grpc_status("/demo.v1.DemoService/Read", None).await,
            (Some("16".to_owned()), 0)
        );
    }

    #[tokio::test]
    async fn actor_bound_methods_require_a_matching_delegation() {
        const WRITE: &str = "/demo.v1.DemoService/Write";
        let demo = Demo::default();
        let service = POLICY.wrap(demo.clone(), verifier()).unwrap();
        let signer = ActorDelegationSigner::local_parity(Workload::Gateway);
        let now = chrono::Utc::now().timestamp();
        let actor = Uuid::new_v4();
        let body = frame(&field(1, actor.to_string().as_bytes()));
        let token = signer.issue(Workload::Post, WRITE, actor, None, &body, now);

        // Missing delegation, then a delegation for another user's command.
        let gateway = Some(Workload::Gateway);
        assert_eq!(
            send(&service, request_with(WRITE, gateway, body.clone(), None)).await,
            Some("16".to_owned())
        );
        let victim = frame(&field(1, Uuid::new_v4().to_string().as_bytes()));
        let spoof = signer.issue(Workload::Post, WRITE, actor, None, &victim, now);
        let spoofed_body = frame(&[field(1, Uuid::new_v4().to_string().as_bytes())].concat());
        assert_eq!(
            send(
                &service,
                request_with(WRITE, gateway, spoofed_body, Some(spoof))
            )
            .await,
            Some("7".to_owned())
        );
        // A delegation whose subject differs from the request's actor field.
        let other = Uuid::new_v4();
        let mismatched = signer.issue(Workload::Post, WRITE, other, None, &body, now);
        assert_eq!(
            send(
                &service,
                request_with(WRITE, gateway, body.clone(), Some(mismatched))
            )
            .await,
            Some("7".to_owned())
        );
        assert_eq!(demo.calls.load(Ordering::SeqCst), 0);

        // The valid delegation reaches the handler once with the verified actor.
        assert_eq!(
            send(
                &service,
                request_with(WRITE, gateway, body.clone(), Some(token.clone()))
            )
            .await,
            None
        );
        assert_eq!(*demo.actors.lock().unwrap(), vec![Some(actor)]);
        assert_eq!(
            send(&service, request_with(WRITE, gateway, body, Some(token))).await,
            Some("16".to_owned()),
            "replayed token"
        );
        assert_eq!(demo.calls.load(Ordering::SeqCst), 1);

        // A method without actor fields carries no verified actor.
        send(&service, request("/demo.v1.DemoService/Read", gateway)).await;
        assert_eq!(demo.actors.lock().unwrap().last(), Some(&None));
    }

    #[test]
    fn wrapping_rejects_inconsistent_policies() {
        static OTHER: MethodPolicy = MethodPolicy {
            service: "other.v1.OtherService",
            workload: Some(Workload::Post),
            methods: &[],
            actor_fields: &[],
        };
        static DUPLICATE: MethodPolicy = MethodPolicy {
            service: "demo.v1.DemoService",
            workload: Some(Workload::Post),
            methods: &[("Read", &[]), ("Read", &[Workload::Gateway])],
            actor_fields: &[],
        };
        static SIBLING_ACTOR: MethodPolicy = MethodPolicy {
            service: "demo.v1.DemoService",
            workload: Some(Workload::Post),
            methods: &[("Read", &[Workload::Gateway, Workload::Post])],
            actor_fields: &[("Read", &[&[1]])],
        };
        static EMPTY_PATH: MethodPolicy = MethodPolicy {
            service: "demo.v1.DemoService",
            workload: Some(Workload::Post),
            methods: &[("Write", &[Workload::Gateway])],
            actor_fields: &[("Write", &[&[]])],
        };
        for policy in [&OTHER, &DUPLICATE, &SIBLING_ACTOR, &EMPTY_PATH] {
            assert!(policy.wrap(Demo::default(), verifier()).is_err());
        }
        let wrong_audience = ActorDelegationVerifier::local_parity(Workload::Friend);
        assert!(POLICY.wrap(Demo::default(), wrong_audience).is_err());
    }

    #[tokio::test]
    async fn a_service_outside_the_workload_set_registers_without_delegation() {
        static GENERATED: MethodPolicy = MethodPolicy {
            service: "demo.v1.DemoService",
            workload: None,
            methods: &[("Read", &[Workload::Gateway]), ("Write", &[])],
            actor_fields: &[],
        };
        let demo = Demo::default();
        let service = GENERATED
            .wrap_without_delegation(demo.clone())
            .expect("generated service policy");
        let gateway = Some(Workload::Gateway);
        assert_eq!(
            send(&service, request("/demo.v1.DemoService/Read", gateway)).await,
            None
        );
        // Unlisted caller, method with no callers, and unlisted method all deny
        // before the handler.
        for (path, caller) in [
            ("/demo.v1.DemoService/Read", Workload::Post),
            ("/demo.v1.DemoService/Write", Workload::Gateway),
            ("/demo.v1.DemoService/Other", Workload::Gateway),
        ] {
            assert_eq!(
                send(&service, request(path, Some(caller))).await,
                Some("7".to_owned()),
                "{path} for {caller:?}"
            );
        }
        assert_eq!(demo.calls.load(Ordering::SeqCst), 1);

        // A policy with actor fields cannot register without a verifier, and a
        // policy with no workload cannot register with one.
        assert!(POLICY.wrap_without_delegation(Demo::default()).is_err());
        assert!(GENERATED.wrap(Demo::default(), verifier()).is_err());
    }

    #[test]
    fn proto_rpc_names_reads_service_methods() {
        let proto = "service DemoService {\n  rpc Read(ReadRequest) returns (ReadResponse);\n  rpc Write (WriteRequest) returns (WriteResponse) {}\n  // rpc Commented(X) returns (Y);\n}";
        assert_eq!(proto_rpc_names(proto), vec!["Read", "Write"]);
    }
}

#[cfg(test)]
mod relationship_read_tests;
#[cfg(test)]
mod world_profile_tests;

#[cfg(test)]
mod owner_authority_tests;
