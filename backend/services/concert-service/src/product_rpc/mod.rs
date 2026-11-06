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
mod age_admission;
pub mod child_recovery;
pub mod child_settlement;
pub mod delivery;
mod feature_admission;
mod owner_media;
pub(crate) mod query;
pub mod report_admission;
pub mod venue_meeting_admission;
mod operations_admission;
pub use age_admission::AgeOperation;
pub use operations_admission::OperationsOperation;
pub use feature_admission::IntroOperation;
pub use query::CommandStatusQuery;
pub use report_admission::ReportOperation;
pub mod root_read;
pub mod scope_ledger;
mod scope_lifecycle;
pub mod scoped;
pub mod settlement;
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
    FeedCommand(platform::concert_feed_command::FeedCommandRoute),
    Post(Uuid),
    Comments(Uuid),
    ListParties,
    GetParty(Uuid),
    CreateParty,
    ApplyParty(Uuid),
    DecidePartyApplication(Uuid),
    CancelPartyApplication(Uuid),
    RemovePartyMember(Uuid, Uuid),
    CancelParty(Uuid),
    InitializeUpload,
    MediaMetadata(Uuid),
    UploadBytes(Uuid),
    FinalizeUpload(Uuid),
    PrivateContent(Uuid),
    CommandStatus,
    ServicePolicy,
    Wall(crate::mural_tools_b::protocol::WallOperation),
    Social(crate::social_live::protocol::SocialOperation),
    Intro(IntroOperation),
    Notification(crate::notification_live::NotificationOperation),
    Age(AgeOperation),
    Report(ReportOperation),
    Closure,
    Meeting(venue_meeting_admission::MeetingOperation),
    Operations(OperationsOperation),
}
impl Operation {
    pub fn contract_id(self) -> &'static str {
        match self {
            Self::Feed => "listConcertFeed",
            Self::FeedCommand(route) => route.operation(),
            Self::Post(_) => "getConcertPost",
            Self::Comments(_) => "listConcertComments",
            Self::ListParties => "listConcertParties",
            Self::GetParty(_) => "getConcertParty",
            Self::CreateParty => "createConcertParty",
            Self::ApplyParty(_) => "applyConcertParty",
            Self::DecidePartyApplication(_) => "decideConcertApplication",
            Self::CancelPartyApplication(_) => "cancelConcertApplication",
            Self::RemovePartyMember(_, _) => "removeConcertMember",
            Self::CancelParty(_) => "cancelConcertParty",
            Self::InitializeUpload => "initConcertUpload",
            Self::MediaMetadata(_) => "getConcertMedia",
            Self::UploadBytes(_) => "putConcertUploadBytes",
            Self::FinalizeUpload(_) => "finalizeConcertUpload",
            Self::PrivateContent(_) => "readConcertMedia",
            Self::CommandStatus => "getConcertCommandStatus",
            Self::ServicePolicy => "getConcertServicePolicy",
            Self::Wall(operation) => operation.id(),
            Self::Social(operation) => operation.id(),
            Self::Intro(operation) => operation.id(),
            Self::Notification(operation) => operation.id(),
            Self::Age(operation) => operation.id(),
            Self::Report(operation) => operation.id(),
            Self::Closure => "closeConcertEvent",
            Self::Meeting(operation) => operation.id(),
            Self::Operations(operation) => operation.id(),
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
    if matches!(method, "POST" | "PATCH" | "PUT" | "DELETE")
        && matches!(suffix.split('/').next(), Some("posts" | "comments"))
    {
        return platform::concert_feed_command::FeedCommandRoute::parse(event, method, target)
            .map(Operation::FeedCommand);
    }
    if let Ok(wall) = crate::mural_tools_b::protocol::WallOperation::parse(event, method, target) {
        if wall != crate::mural_tools_b::protocol::WallOperation::Status {
            return Ok(Operation::Wall(wall));
        }
    }
    // The Social parser owns exactly thirteen frozen operations. No unknown
    // route, platform or method falls through to a generic producer.
    if let Ok(social) = crate::social_live::protocol::route(event, method, target) {
        return Ok(Operation::Social(social));
    }
    if let Ok(intro) = feature_admission::intro_route(event, method, target) {
        return Ok(Operation::Intro(intro));
    }
    if let Ok(age) = age_admission::route(event, method, target) {
        return Ok(Operation::Age(age));
    }
    if let Ok(report) = ReportOperation::route(event, method, target) {
        return Ok(Operation::Report(report));
    }
    if let Ok(operations) = OperationsOperation::route(event, method, target) {
        return Ok(Operation::Operations(operations));
    }
    if let Ok(notification) =
        crate::notification_live::NotificationOperation::route(event, method, target)
    {
        return Ok(Operation::Notification(notification));
    }
    if let Ok(meeting) = venue_meeting_admission::route(event, method, target) {
        return Ok(Operation::Meeting(meeting));
    }
    let result = match (method, suffix) {
        ("GET", "feed") => Operation::Feed,
        ("GET", "parties") => Operation::ListParties,
        ("POST", "parties") => Operation::CreateParty,
        ("GET", "commands") => Operation::CommandStatus,
        ("GET", "service-policy") => Operation::ServicePolicy,
        ("POST", "media/uploads") => Operation::InitializeUpload,
        ("POST", "admin/closure") => Operation::Closure,
        _ => {
            let pieces: Vec<_> = suffix.split('/').collect();
            if !(1..=4).contains(&pieces.len()) {
                return Err(Status::permission_denied("Unsupported product operation"));
            }
            match (method, pieces.as_slice()) {
                ("POST", ["parties", party, "applications"]) => {
                    Operation::ApplyParty(canonical_uuid(party)?)
                }
                ("POST", ["party-applications", application, "decision"]) => {
                    Operation::DecidePartyApplication(canonical_uuid(application)?)
                }
                ("DELETE", ["party-applications", application]) => {
                    Operation::CancelPartyApplication(canonical_uuid(application)?)
                }
                ("DELETE", ["parties", party, "members", user]) => {
                    Operation::RemovePartyMember(canonical_uuid(party)?, canonical_uuid(user)?)
                }
                ("POST", ["parties", party, "cancel"]) => {
                    Operation::CancelParty(canonical_uuid(party)?)
                }
                ("GET", ["posts", id]) => Operation::Post(canonical_uuid(id)?),
                ("GET", ["posts", id, "comments"]) => Operation::Comments(canonical_uuid(id)?),
                ("GET", ["parties", id]) => Operation::GetParty(canonical_uuid(id)?),
                ("GET", ["media", id]) => Operation::MediaMetadata(canonical_uuid(id)?),
                ("GET", ["media", id, "content"]) => Operation::PrivateContent(canonical_uuid(id)?),
                ("PUT", ["media", id, "bytes"]) => Operation::UploadBytes(canonical_uuid(id)?),
                ("POST", ["media", id, "finalize"]) => {
                    Operation::FinalizeUpload(canonical_uuid(id)?)
                }
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
                | Operation::CancelPartyApplication(_)
                | Operation::RemovePartyMember(_, _)
                | Operation::ServicePolicy
        )
    {
        return Err(Status::invalid_argument("Unexpected product query"));
    }
    query::party_command(target, result)?;
    Ok(result)
}

/// Only the guarded RPC can construct this request-bound input. Implementations
/// still owe canonical owner fencing through commit/response, not observations.
pub struct VerifiedProduct {
    trusted_network_bucket: Option<[u8; 32]>,
    actor: Uuid,
    event: Uuid,
    session: Uuid,
    request: Uuid,
    deadline: AuthorityRpcDeadline,
    operation: Operation,
    parent: VerifiedParentAuthority,
    feed_query: Option<crate::feed::live_read::FeedQuery>,
    comments_query: Option<crate::feed::live_read::CommentsQuery>,
    command_query: Option<CommandStatusQuery>,
    service_policy_query: Option<crate::service_policy::Query>,
}
impl VerifiedProduct {
    // Retain this already authenticated request for the owned producer future.
    // Copy both original deadline clocks and the sealed parent; never reparse
    // observations, renew authority, or expose another context constructor.
    fn owned_for_producer(&self) -> Self {
        Self {
            trusted_network_bucket: self.trusted_network_bucket,
            actor: self.actor,
            event: self.event,
            session: self.session,
            request: self.request,
            deadline: self.deadline,
            operation: self.operation,
            parent: self.parent.clone(),
            feed_query: self
                .feed_query
                .as_ref()
                .map(|query| crate::feed::live_read::FeedQuery {
                    limit: query.limit,
                    cursor: query.cursor.clone(),
                    filter: query.filter,
                    before: query.before,
                }),
            comments_query: self.comments_query.as_ref().map(|query| {
                crate::feed::live_read::CommentsQuery {
                    limit: query.limit,
                    cursor: query.cursor.clone(),
                }
            }),
            command_query: self.command_query,
            service_policy_query: self.service_policy_query,
        }
    }
    /// Only populated from the exact workload/actor-authenticated Gateway frame.
    /// Absence is unavailable, never a shared/random/default network bucket.
    pub fn trusted_pickup_context(
        &self,
    ) -> Option<crate::community::giveaway::TrustedPickupContext> {
        self.trusted_network_bucket
            .map(crate::community::giveaway::TrustedPickupContext::from_trusted_keyed_ip_bucket)
    }
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
    pub fn command_query(&self) -> Option<CommandStatusQuery> {
        self.command_query
    }
    pub fn service_policy_query(&self) -> Option<crate::service_policy::Query> {
        self.service_policy_query
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
    let command_query = matches!(operation, Operation::CommandStatus)
        .then(|| query::command_status(&m.http_target))
        .transpose()?;
    let service_policy_query = matches!(operation, Operation::ServicePolicy)
        .then(|| crate::service_policy::parse_query(&m.http_target))
        .transpose()?;
    if m.http_body.len() > MAX_BODY {
        return Err(Status::resource_exhausted("Product body too large"));
    }
    if let Operation::Wall(wall) = operation {
        query::wall_request(m, wall)?;
    }
    if let Operation::Social(social) = operation {
        query::social_request(m, social)?;
    }
    if let Operation::Age(age) = operation {
        query::age_request(m, age)?;
    }
    if let Operation::Report(report) = operation {
        query::report_request(m, report)?;
    }
    if let Operation::FeedCommand(route) = operation {
        canonical_uuid(&m.idempotency_key)?;
        route.validate_body(&m.content_type, &m.http_body)?;
        query::feed_command(m, route)?;
    }
    if let Operation::Meeting(_) = operation {
        venue_meeting_admission::request(m, operation)?;
    }
    if let Operation::Operations(operations) = operation {
        query::operations_request(m, operations)?;
    }
    if matches!(operation, Operation::Intro(_) | Operation::Notification(_)) {
        feature_admission::request(m, operation)?;
    }
    // The frozen START/RESUME union is parsed again by the lifecycle boundary;
    // this only bounds the frame before any owner work is acquired.
    if operation == Operation::Closure && m.http_body.len() > 8192 {
        return Err(Status::invalid_argument("Invalid product content type"));
    }
    let mutation = matches!(
        operation,
        Operation::InitializeUpload
            | Operation::UploadBytes(_)
            | Operation::FinalizeUpload(_)
            | Operation::CreateParty
            | Operation::ApplyParty(_)
            | Operation::DecidePartyApplication(_)
            | Operation::CancelPartyApplication(_)
            | Operation::RemovePartyMember(_, _)
            | Operation::CancelParty(_)
            | Operation::Report(_)
            | Operation::Closure
            | Operation::Operations(_)
    );
    if matches!(
        operation,
        Operation::Social(_) | Operation::Intro(_) | Operation::Notification(_) | Operation::Age(_) | Operation::Report(_) | Operation::Operations(_) | Operation::Wall(_) | Operation::Meeting(_) | Operation::FeedCommand(_)
    ) {
        // Each exact feature decoder validates its own read payload and keyed
        // command shape, including Intro PUT/POST and Notification PATCH writes.
        // Age owns its bodyless status and keyed POSTs. Social additionally owns
        // keyed bodyless DELETE; none are generic reads.
    } else if mutation {
        canonical_uuid(&m.idempotency_key)?;
        let bodyless = matches!(
            operation,
            Operation::CancelPartyApplication(_) | Operation::RemovePartyMember(_, _)
        );
        let valid_type = if bodyless {
            m.http_body.is_empty() && m.content_type.is_empty()
        } else if matches!(operation, Operation::UploadBytes(_)) {
            matches!(
                m.content_type.as_str(),
                "image/jpeg" | "image/png" | "image/webp"
            )
        } else {
            !m.http_body.is_empty() && m.content_type == "application/json"
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
        trusted_network_bucket: match m.trusted_network_bucket.as_slice() {
            [] => None,
            bytes if bytes.len() == 32 && bytes.iter().any(|b| *b != 0) => {
                Some(bytes.try_into().expect("checked bucket size"))
            }
            _ => return Err(Status::invalid_argument("Invalid trusted network bucket")),
        },
        actor,
        event,
        session: canonical_uuid(&m.session_family_id)?,
        request: canonical_uuid(&m.request_id)?,
        operation,
        parent,
        feed_query,
        comments_query,
        command_query,
        service_policy_query,
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

/// Wall recovery uses the same closed canonical status parser as admission.
pub(crate) fn wall_command_status_query(target: &str) -> Result<CommandStatusQuery, Status> {
    query::command_status(target)
}
