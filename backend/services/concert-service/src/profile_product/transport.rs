//! The guarded concert-hosted profile product transport.
//!
//! A verified edge input is not owner authority. This module is the concert
//! service's counterpart of `profile_service::profile_product`: the same
//! request shape, the same parent-frame comparison, the same owner scope and
//! delivery discipline, keyed by `profile_id` and refusing any event path.
//!
//! The reason it exists rather than reusing `concert.product.v1` is that the
//! concert product transport is event-scoped by construction (`event_id` must be
//! non-nil, every target must start with `/api/concert/events/{event}/`), and a
//! profile operation has no event. Inventing one would be the silent product
//! decision the integration forbids.
use super::{
    delivery::{ProfileDeliveryPublisher, ProfileDeliveryService},
    guard::{ProfileOwnerFence, ProfileScopeRegistry},
    ledger::ProfileScopeLedger,
    proto, targets,
};
use platform::{
    actor_delegation::{
        authority_deadline::AuthorityRpcDeadline, exchange::VerifiedParentAuthority,
        read_authority::VerifiedReadAuthority, VerifiedActor,
    },
    grpc_auth::{VerifiedCaller, Workload},
    grpc_method_policy::MethodPolicy,
};
use prost::Message;
use std::{sync::Arc, time::Duration};
use tonic::{Request, Response, Status};
use uuid::Uuid;

/// The same bounded budget the profile and concert product transports use.
pub const BUDGET: Duration = Duration::from_secs(2);
pub const MAX_BODY: usize = 10 * 1024 * 1024;
pub const MAX_FRAME: usize = MAX_BODY + 16 * 1024;

/// Only the Gateway may dispatch, and the method acts for a declared actor.
pub const POLICY: MethodPolicy = MethodPolicy {
    service: "concert.profile_product.v1.ProfileProductService",
    workload: Some(Workload::Concert),
    methods: &[("Dispatch", &[Workload::Gateway])],
    actor_fields: &[("Dispatch", &[&[1]])],
};

/// Settlement is a system operation: no actor, Gateway only.
pub const DELIVERY_POLICY: MethodPolicy = MethodPolicy {
    service: "concert.profile_product.v1.ProfileProductDeliveryService",
    workload: Some(Workload::Concert),
    methods: &[("CompleteResponse", &[Workload::Gateway])],
    actor_fields: &[],
};

/// Canonical lowercase non-nil UUID. Same rule as every other product producer.
pub fn canonical_uuid(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or_else(|| Status::invalid_argument("Invalid profile identity"))
}

/// The event-path guard, applied to the raw target before route parsing so a
/// percent-encoded or nested `/events/` segment cannot slip past a narrower
/// parser.
pub fn rejects_event_path(target: &str) -> bool {
    let lowered = target.to_ascii_lowercase();
    lowered.contains("/events/")
        || lowered.contains("%2fevents%2f")
        || lowered.contains("%2fevents/")
        || lowered.contains("/events%2f")
}

/// Only the guarded RPC can construct this request-bound input. Producers still
/// owe real owner fencing through commit and response, not observations.
pub struct VerifiedProfile {
    trusted_network_bucket: Option<[u8; 32]>,
    actor: Uuid,
    profile: Uuid,
    session: Uuid,
    request: Uuid,
    deadline: AuthorityRpcDeadline,
    operation: targets::ProfileOperation,
    parent: VerifiedParentAuthority,
}

impl VerifiedProfile {
    /// Retain this already authenticated request for the owned producer future.
    /// Copy both original deadline clocks and the sealed parent; never reparse
    /// observations, renew authority, or expose another context constructor.
    pub(crate) fn owned_for_producer(&self) -> Self {
        Self {
            trusted_network_bucket: self.trusted_network_bucket,
            actor: self.actor,
            profile: self.profile,
            session: self.session,
            request: self.request,
            deadline: self.deadline,
            operation: self.operation,
            parent: self.parent.clone(),
        }
    }
    pub fn trusted_network_bucket(&self) -> Option<[u8; 32]> {
        self.trusted_network_bucket
    }
    pub fn actor(&self) -> Uuid {
        self.actor
    }
    pub fn profile(&self) -> Uuid {
        self.profile
    }
    pub fn session(&self) -> Uuid {
        self.session
    }
    pub fn request(&self) -> Uuid {
        self.request
    }
    pub fn operation(&self) -> targets::ProfileOperation {
        self.operation
    }
    pub fn deadline(&self) -> AuthorityRpcDeadline {
        self.deadline
    }
    pub fn parent_authority(&self) -> &VerifiedParentAuthority {
        &self.parent
    }
    pub fn check(&self) -> Result<(), Status> {
        self.deadline
            .remaining(BUDGET, self.deadline.expires_micros())
            .map(|_| ())
    }
}

/// Verify one inbound dispatch.
///
/// Every check here is a guarantee a persistence function called with a UUID
/// does not supply: the caller is the verified Gateway, the actor is the
/// delegated actor and the actor the original deadline proof names, the signed
/// parent frame is byte-identical to the message received, the profile identity
/// is canonical, the target is event-free, and the observation window is fresh
/// and bounded.
pub fn authorize(request: &Request<proto::ProfileRequest>) -> Result<VerifiedProfile, Status> {
    if !matches!(
        request.extensions().get::<VerifiedCaller>(),
        Some(VerifiedCaller(Workload::Gateway))
    ) {
        return Err(Status::unauthenticated("Verified Gateway required"));
    }
    let actor = request
        .extensions()
        .get::<VerifiedActor>()
        .ok_or_else(|| Status::unauthenticated("Verified profile actor required"))?
        .0;
    let proof = request
        .extensions()
        .get::<VerifiedReadAuthority>()
        .ok_or_else(|| Status::unauthenticated("Original profile deadline required"))?;
    let m = request.get_ref();
    let parent = request
        .extensions()
        .get::<VerifiedParentAuthority>()
        .ok_or_else(|| Status::unauthenticated("Verified original profile parent required"))?
        .clone();
    if parent.request().method != "/concert.profile_product.v1.ProfileProductService/Dispatch"
        || parent.actor() != actor
        || proto::ProfileRequest::decode(
            parent
                .message()
                .map_err(|_| Status::permission_denied("Invalid profile parent frame"))?,
        )
        .map_err(|_| Status::permission_denied("Invalid profile parent message"))?
            != *m
    {
        return Err(Status::permission_denied("Profile parent mismatch"));
    }
    let user = canonical_uuid(&m.user_id)?;
    if user != actor || user != proof.actor() {
        return Err(Status::permission_denied("Profile actor mismatch"));
    }
    // A nil profile identity is rejected before any owner work is attempted.
    let profile = canonical_uuid(&m.profile_id)?;
    // Profile operations never carry an event path. Reject before parsing the
    // route so a concert-shaped request can never reach a profile producer.
    if rejects_event_path(&m.http_target) {
        return Err(Status::invalid_argument(
            "Profile operations never carry an event path",
        ));
    }
    if m.http_body.len() > MAX_BODY {
        return Err(Status::resource_exhausted("Profile body too large"));
    }
    if !matches!(
        m.http_method.as_str(),
        "GET" | "POST" | "PATCH" | "PUT" | "DELETE"
    ) {
        return Err(Status::permission_denied("Unsupported profile operation"));
    }
    if m.http_target.len() > 4096
        || m.http_target.chars().any(char::is_control)
        || !m.http_target.starts_with("/api/")
    {
        return Err(Status::permission_denied("Invalid profile target"));
    }
    let mutation = matches!(
        m.http_method.as_str(),
        "POST" | "PATCH" | "PUT" | "DELETE"
    );
    if mutation {
        canonical_uuid(&m.idempotency_key)?;
    } else if !m.http_body.is_empty() || !m.content_type.is_empty() || !m.idempotency_key.is_empty()
    {
        return Err(Status::invalid_argument("Unexpected profile read payload"));
    }
    let now = chrono::Utc::now().timestamp_micros();
    let expires = m
        .authority_expires_unix_micros
        .min(m.access_expires_unix_micros);
    for observed in [
        m.auth_observed_at_unix_micros,
        m.profile_observed_at_unix_micros,
    ] {
        if observed <= 0
            || observed > now
            || expires <= observed
            || expires.saturating_sub(observed) > BUDGET.as_micros() as i64
        {
            return Err(Status::permission_denied(
                "Invalid profile observation bounds",
            ));
        }
    }
    let original = proof.deadline(BUDGET)?;
    let value = VerifiedProfile {
        trusted_network_bucket: match m.trusted_network_bucket.as_slice() {
            [] => None,
            bytes if bytes.len() == 32 && bytes.iter().any(|b| *b != 0) => {
                Some(bytes.try_into().expect("checked bucket size"))
            }
            _ => return Err(Status::invalid_argument("Invalid trusted network bucket")),
        },
        actor,
        profile,
        session: canonical_uuid(&m.session_family_id)?,
        request: canonical_uuid(&m.request_id)?,
        parent,
        deadline: AuthorityRpcDeadline::within_original(
            expires.min(original.expires_micros()),
            original.stop().into_std(),
            BUDGET,
        )?,
        // The closed route set decides whether this operation is served here at
        // all. A profile operation whose tables live in the profile database is
        // refused by this parser, so it can never be answered by this service.
        operation: targets::parse(&m.http_method, &m.http_target)?,
    };
    value.check()?;
    Ok(value)
}

#[async_trait::async_trait]
pub trait ProfileProductOwner: Send + Sync {
    async fn dispatch(
        &self,
        context: &VerifiedProfile,
        request: proto::ProfileRequest,
    ) -> Result<Response<proto::ProfileResponse>, Status>;
}

pub struct ProfileProductRpcService {
    owner: Arc<dyn ProfileProductOwner>,
    /// The registry the owner acquires into. It is held here, not only by the
    /// owner's fence, so a live served transport keeps its scopes reachable for
    /// settlement for as long as it is serving.
    registry: Arc<ProfileScopeRegistry>,
    capacity: Arc<tokio::sync::Semaphore>,
}

impl ProfileProductRpcService {
    pub fn new(owner: Arc<dyn ProfileProductOwner>, registry: Arc<ProfileScopeRegistry>) -> Self {
        Self {
            owner,
            registry,
            capacity: Arc::new(tokio::sync::Semaphore::new(4)),
        }
    }
    /// Number of scopes this process still holds. Diagnostic only.
    pub fn held_scopes(&self) -> usize {
        self.registry.live()
    }
}

#[tonic::async_trait]
impl proto::profile_product_service_server::ProfileProductService for ProfileProductRpcService {
    async fn dispatch(
        &self,
        request: Request<proto::ProfileRequest>,
    ) -> Result<Response<proto::ProfileResponse>, Status> {
        let context = authorize(&request)?;
        let _permit = self
            .capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| Status::unavailable("Profile product capacity unavailable"))?;
        let mut response = context
            .parent
            .clone()
            .scope(async {
                platform::call_deadline::scope(context.deadline.stop().into_std(), async {
                    tokio::time::timeout_at(
                        context.deadline.stop(),
                        self.owner.dispatch(&context, request.into_inner()),
                    )
                    .await
                    .map_err(|_| Status::deadline_exceeded("Profile product deadline expired"))?
                })
                .await
            })
            .await?;
        context.check()?;
        let result = response.get_ref();
        if result.body.is_empty()
            || result.body.len() > MAX_BODY
            || !matches!(
                result.status,
                200 | 201 | 202 | 400 | 401 | 403 | 404 | 409 | 413 | 415 | 429 | 500 | 503 | 504
            )
            || !matches!(
                result.content_type.as_str(),
                "application/json" | "image/jpeg" | "image/png" | "image/webp"
            )
        {
            return Err(Status::internal("Invalid profile product representation"));
        }
        if result.delivery.is_none()
            || response
                .extensions()
                .get::<platform::actor_delegation::response_scope::ResponseScopeToken>()
                .is_none()
        {
            return Err(Status::unavailable(
                "Canonical profile delivery scope required",
            ));
        }
        response.metadata_mut().insert(
            "cache-control",
            "no-store".parse().expect("static metadata"),
        );
        Ok(response)
    }
}

/// Wrap the guarded dispatch service so the response-scope token attached to
/// each response is retained until the gRPC body terminal. Without this layer
/// the token would be dropped with the response and the owner scope released at
/// gRPC EOF, before the Gateway can settle it.
pub fn retain_scopes<S, B>(service: S) -> super::retained::NamedRetainedScope<S, B>
where
    S: tower::Service<http::Request<tonic::body::Body>, Response = http::Response<B>>
        + tonic::server::NamedService
        + Send
        + 'static,
    B: http_body::Body + Send + 'static,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    super::retained::NamedRetainedScope::new(service)
}

pub struct ProfileProductRuntime {
    /// The scope registry and ledger the transport uses. Retained so the process
    /// can observe and reconcile unresolved scopes; it is not a grant.
    pub registry: Arc<ProfileScopeRegistry>,
    pub ledger: ProfileScopeLedger,
    pub dispatch: platform::grpc_method_policy::GuardedService<
        proto::profile_product_service_server::ProfileProductServiceServer<
            ProfileProductRpcService,
        >,
    >,
    pub settlement: platform::grpc_method_policy::GuardedService<
        proto::profile_product_delivery_service_server::ProfileProductDeliveryServiceServer<
            ProfileDeliveryService,
        >,
    >,
}

/// Build the concert-hosted profile product transport over a real database pool.
///
/// `verifier` must accept delegations addressed to the Concert workload. The
/// dispatch method acts for a user, so the method policy refuses to wrap it
/// without a verifier; that is the same requirement the concert product has.
pub fn runtime(
    pool: sqlx::PgPool,
    callers: platform::grpc_auth::CallerRegistry,
    verifier: platform::actor_delegation::ActorDelegationVerifier,
) -> anyhow::Result<ProfileProductRuntime> {
    build_runtime(
        pool.clone(),
        callers,
        verifier,
        targets::ProfileProductProducer::new(pool.clone())
            .with_intro(std::sync::Arc::new(super::intro::IntroProducer::new(pool))),
    )
}

fn build_runtime(
    pool: sqlx::PgPool,
    callers: platform::grpc_auth::CallerRegistry,
    verifier: platform::actor_delegation::ActorDelegationVerifier,
    producer: targets::ProfileProductProducer,
) -> anyhow::Result<ProfileProductRuntime> {
    let registry = ProfileScopeRegistry::new();
    let ledger = ProfileScopeLedger::new(pool.clone());
    let fence = Arc::new(ProfileOwnerFence::new(registry.clone(), ledger.clone()));
    let publisher = ProfileDeliveryPublisher::new(ledger.clone(), registry.clone());
    let producer: Arc<dyn super::scoped::ScopedProfileProducer> = Arc::new(producer);
    let owner = Arc::new(super::scoped::ScopedProfileOwner::new(
        fence,
        publisher.clone(),
        producer,
    ));
    let dispatch = platform::grpc_method_policy::guard(
        &POLICY,
        proto::profile_product_service_server::ProfileProductServiceServer::new(
            ProfileProductRpcService::new(owner, registry.clone()),
        ),
        callers.clone(),
        verifier,
    )?;
    let settlement = platform::grpc_method_policy::guard_without_delegation(
        &DELIVERY_POLICY,
        proto::profile_product_delivery_service_server::ProfileProductDeliveryServiceServer::new(
            ProfileDeliveryService::new(publisher),
        ),
        callers,
    )?;
    Ok(ProfileProductRuntime {
        registry,
        ledger,
        dispatch,
        settlement,
    })
}
