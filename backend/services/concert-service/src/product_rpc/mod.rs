//! Guarded C04 product transport. A verified edge input is not owner authority.
use async_trait::async_trait;
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

pub mod proto {
    tonic::include_proto!("concert.product.v1");
}
pub mod delivery;
mod owner_media;
mod query;
pub mod scope_ledger;
mod scope_lifecycle;
pub const BUDGET: Duration = Duration::from_secs(2);
pub const MAX_BODY: usize = 10 * 1024 * 1024;
pub const MAX_FRAME: usize = MAX_BODY + 16 * 1024;
pub const POLICY: MethodPolicy = MethodPolicy {
    service: "concert.product.v1.ProductService",
    workload: Some(Workload::Concert),
    methods: &[("Dispatch", &[Workload::Gateway])],
    actor_fields: &[("Dispatch", &[&[1]])],
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Feed,
    Post(Uuid),
    Comments(Uuid),
    ListParties,
    GetParty(Uuid),
    CreateParty,
    InitializeUpload,
    MediaMetadata(Uuid),
    UploadBytes(Uuid),
    FinalizeUpload(Uuid),
    PrivateContent(Uuid),
    CommandStatus,
}
impl Operation {
    pub fn contract_id(self) -> &'static str {
        match self {
            Self::Feed => "listConcertFeed",
            Self::Post(_) => "getConcertPost",
            Self::Comments(_) => "listConcertComments",
            Self::ListParties => "listConcertParties",
            Self::GetParty(_) => "getConcertParty",
            Self::CreateParty => "createConcertParty",
            Self::InitializeUpload => "initConcertUpload",
            Self::MediaMetadata(_) => "getConcertMedia",
            Self::UploadBytes(_) => "putConcertUploadBytes",
            Self::FinalizeUpload(_) => "finalizeConcertUpload",
            Self::PrivateContent(_) => "readConcertMedia",
            Self::CommandStatus => "getConcertCommandStatus",
        }
    }
}
pub fn canonical_uuid(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or_else(|| Status::invalid_argument("Invalid product identity"))
}
/// Closed route set: no inherited Feed, arbitrary forwarding or Feed writes.
pub fn operation(event: Uuid, method: &str, target: &str) -> Result<Operation, Status> {
    if event.is_nil() || target.len() > 4096 || target.chars().any(char::is_control) {
        return Err(Status::invalid_argument("Invalid product target"));
    }
    let (path, query) = target
        .split_once('?')
        .map_or((target, None), |(p, q)| (p, Some(q)));
    let prefix = format!("/api/concert/events/{event}/");
    let suffix = path
        .strip_prefix(&prefix)
        .ok_or_else(|| Status::permission_denied("Product event mismatch"))?;
    let result = match (method, suffix) {
        ("GET", "feed") => Operation::Feed,
        ("GET", "parties") => Operation::ListParties,
        ("POST", "parties") => Operation::CreateParty,
        ("GET", "commands") => Operation::CommandStatus,
        ("POST", "media/uploads") => Operation::InitializeUpload,
        _ => {
            let pieces: Vec<_> = suffix.split('/').collect();
            if !(2..=3).contains(&pieces.len()) {
                return Err(Status::permission_denied("Unsupported product operation"));
            }
            let id = canonical_uuid(pieces[1])?;
            match (method, pieces[0], pieces.get(2).copied()) {
                ("GET", "posts", None) => Operation::Post(id),
                ("GET", "posts", Some("comments")) => Operation::Comments(id),
                ("GET", "parties", None) => Operation::GetParty(id),
                ("GET", "media", None) => Operation::MediaMetadata(id),
                ("GET", "media", Some("content")) => Operation::PrivateContent(id),
                ("PUT", "media", Some("bytes")) => Operation::UploadBytes(id),
                ("POST", "media", Some("finalize")) => Operation::FinalizeUpload(id),
                _ => return Err(Status::permission_denied("Unsupported product operation")),
            }
        }
    };
    if query.is_some()
        && !matches!(
            result,
            Operation::Feed
                | Operation::Comments(_)
                | Operation::ListParties
                | Operation::CommandStatus
        )
    {
        return Err(Status::invalid_argument("Unexpected product query"));
    }
    Ok(result)
}

/// Only the guarded RPC can construct this request-bound input. Implementations
/// still owe canonical owner fencing through commit/response, not observations.
pub struct VerifiedProduct {
    actor: Uuid,
    event: Uuid,
    session: Uuid,
    request: Uuid,
    deadline: AuthorityRpcDeadline,
    operation: Operation,
    parent: VerifiedParentAuthority,
    feed_query: Option<crate::feed::live_read::FeedQuery>,
    comments_query: Option<crate::feed::live_read::CommentsQuery>,
}
impl VerifiedProduct {
    pub fn actor(&self) -> Uuid {
        self.actor
    }
    pub fn event(&self) -> Uuid {
        self.event
    }
    pub fn session(&self) -> Uuid {
        self.session
    }
    pub fn request(&self) -> Uuid {
        self.request
    }
    pub fn deadline(&self) -> AuthorityRpcDeadline {
        self.deadline
    }
    pub fn operation(&self) -> Operation {
        self.operation
    }
    pub fn parent_authority(&self) -> &VerifiedParentAuthority {
        &self.parent
    }
    pub fn feed_query(&self) -> Option<&crate::feed::live_read::FeedQuery> {
        self.feed_query.as_ref()
    }
    pub fn comments_query(&self) -> Option<&crate::feed::live_read::CommentsQuery> {
        self.comments_query.as_ref()
    }
    pub fn check(&self) -> Result<(), Status> {
        self.deadline
            .remaining(BUDGET, self.deadline.expires_micros())
            .map(|_| ())
    }
}
fn authorize(request: &Request<proto::ProductRequest>) -> Result<VerifiedProduct, Status> {
    if !matches!(
        request.extensions().get::<VerifiedCaller>(),
        Some(VerifiedCaller(Workload::Gateway))
    ) {
        return Err(Status::unauthenticated("Verified Gateway required"));
    }
    let actor = request
        .extensions()
        .get::<VerifiedActor>()
        .ok_or_else(|| Status::unauthenticated("Verified product actor required"))?
        .0;
    let proof = request
        .extensions()
        .get::<VerifiedReadAuthority>()
        .ok_or_else(|| Status::unauthenticated("Original product deadline required"))?;
    let m = request.get_ref();
    let parent = request
        .extensions()
        .get::<VerifiedParentAuthority>()
        .ok_or_else(|| Status::unauthenticated("Verified original product parent required"))?
        .clone();
    if parent.request().method != "/concert.product.v1.ProductService/Dispatch"
        || parent.actor() != actor
        || proto::ProductRequest::decode(
            parent
                .message()
                .map_err(|_| Status::permission_denied("Invalid product parent frame"))?,
        )
        .map_err(|_| Status::permission_denied("Invalid product parent message"))?
            != *m
    {
        return Err(Status::permission_denied("Product parent mismatch"));
    }
    let user = canonical_uuid(&m.user_id)?;
    let event = canonical_uuid(&m.event_id)?;
    if user != actor || user != proof.actor() {
        return Err(Status::permission_denied("Product actor mismatch"));
    }
    let operation = operation(event, &m.http_method, &m.http_target)?;
    let feed_query = matches!(operation, Operation::Feed)
        .then(|| query::feed(&m.http_target))
        .transpose()?;
    let comments_query = matches!(operation, Operation::Comments(_))
        .then(|| query::comments(&m.http_target))
        .transpose()?;
    if m.http_body.len() > MAX_BODY {
        return Err(Status::resource_exhausted("Product body too large"));
    }
    let mutation = matches!(
        operation,
        Operation::InitializeUpload
            | Operation::UploadBytes(_)
            | Operation::FinalizeUpload(_)
            | Operation::CreateParty
    );
    if mutation {
        canonical_uuid(&m.idempotency_key)?;
        let valid_type = if matches!(operation, Operation::UploadBytes(_)) {
            matches!(
                m.content_type.as_str(),
                "image/jpeg" | "image/png" | "image/webp"
            )
        } else {
            m.content_type == "application/json"
        };
        if !valid_type {
            return Err(Status::invalid_argument("Invalid product content type"));
        }
    } else if !m.http_body.is_empty() || !m.content_type.is_empty() || !m.idempotency_key.is_empty()
    {
        return Err(Status::invalid_argument("Unexpected product read payload"));
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
                "Invalid product observation bounds",
            ));
        }
    }
    let original = proof.deadline(BUDGET)?;
    let value = VerifiedProduct {
        actor,
        event,
        session: canonical_uuid(&m.session_family_id)?,
        request: canonical_uuid(&m.request_id)?,
        operation,
        parent,
        feed_query,
        comments_query,
        deadline: AuthorityRpcDeadline::within_original(
            expires.min(original.expires_micros()),
            original.stop().into_std(),
            BUDGET,
        )?,
    };
    value.check()?;
    Ok(value)
}

#[async_trait]
pub trait ProductOwner: Send + Sync {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        request: proto::ProductRequest,
    ) -> Result<Response<proto::ProductResponse>, Status>;
}

pub struct ProductRpcService {
    owner: Arc<dyn ProductOwner>,
    capacity: Arc<tokio::sync::Semaphore>,
}
impl ProductRpcService {
    pub fn new(owner: Arc<dyn ProductOwner>) -> Self {
        Self {
            owner,
            capacity: Arc::new(tokio::sync::Semaphore::new(4)),
        }
    }
}
#[tonic::async_trait]
impl proto::product_service_server::ProductService for ProductRpcService {
    async fn dispatch(
        &self,
        request: Request<proto::ProductRequest>,
    ) -> Result<Response<proto::ProductResponse>, Status> {
        let context = authorize(&request)?;
        let _permit = self
            .capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| Status::unavailable("Product capacity unavailable"))?;
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
                    .map_err(|_| Status::deadline_exceeded("Product deadline expired"))?
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
                200 | 202 | 400 | 401 | 403 | 404 | 409 | 413 | 415 | 429 | 500 | 503 | 504
            )
            || !matches!(
                result.content_type.as_str(),
                "application/json" | "image/jpeg" | "image/png" | "image/webp"
            )
            || (result.content_type != "application/json"
                && (!matches!(context.operation, Operation::PrivateContent(_))
                    || result.status != 200))
        {
            return Err(Status::internal("Invalid product representation"));
        }
        if result.delivery.is_none()
            || response
                .extensions()
                .get::<platform::actor_delegation::response_scope::ResponseScopeToken>()
                .is_none()
        {
            return Err(Status::unavailable(
                "Canonical product delivery scope required",
            ));
        }
        response.metadata_mut().insert(
            "cache-control",
            "no-store".parse().expect("static metadata"),
        );
        Ok(response)
    }
}
