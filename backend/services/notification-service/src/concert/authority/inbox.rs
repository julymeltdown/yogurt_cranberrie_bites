//! Typed internal inbox endpoint. Missing real root target policy is unavailable,
//! including for empty inboxes and unread counts; no default positive provider.
use super::{
    canonical, settlement::no_store, AuthorityStore, CheckedScope, Intent, NativeOutcome,
    NotificationSettlement, Owners,
};
use crate::concert::{
    transport::{lease, proto},
    AuthorityFence, Authorization, FencedInboxAuthority, PgConcertInbox,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::notification::{Candidate, Context, Query};
use concert_domain::notification::{self as domain, NotificationError};
use platform::{
    actor_delegation::authority_deadline::AuthorityRpcDeadline,
    grpc_auth::{VerifiedCaller, Workload},
    grpc_method_policy::MethodPolicy,
};
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use std::{sync::Arc, time::Duration};
use tonic::{Request, Response, Status};

pub static INBOX_POLICY: MethodPolicy = MethodPolicy {
    service: "notification.concert.v1.ConcertInboxService",
    workload: Some(Workload::Notification),
    methods: &[
        ("Execute", &[Workload::Concert]),
        ("Check", &[Workload::Concert]),
        ("Complete", &[Workload::Concert]),
    ],
    actor_fields: &[],
};
/// Root-owned real policy port. Implementations must resolve the exact private
/// RootReadView at the current root, fence event/target/current source policy,
/// and RETAIN those exclusions through child completion/root response delivery.
/// The returned fence is retained through the actual local inbox commit. A
/// remote callback must retain exclusions at Root after its reply as well.
#[async_trait]
pub trait RootTargetSnapshotOwner: Send + Sync {
    async fn authorize(
        &self,
        parent: &proto::ParentScope,
        context: Context,
        candidates: &[Candidate],
        now: DateTime<Utc>,
    ) -> domain::Result<Authorization>;
}
#[derive(Clone)]
pub struct NotificationInboxService {
    pool: PgPool,
    owners: Owners,
    store: AuthorityStore,
    targets: Option<Arc<dyn RootTargetSnapshotOwner>>,
}
impl NotificationInboxService {
    pub async fn connect(
        pool: PgPool,
        auth_endpoint: &str,
        friend_endpoint: &str,
        token: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        targets: Option<Arc<dyn RootTargetSnapshotOwner>>,
    ) -> anyhow::Result<Self> {
        let deadline = AuthorityRpcDeadline::new(
            Utc::now().timestamp_micros() + 5_000_000,
            Duration::from_secs(5),
        )?;
        let store = AuthorityStore::open(pool.clone(), deadline).await?;
        Ok(Self {
            pool,
            owners: Owners::connect(auth_endpoint, friend_endpoint, token, tls)?,
            store,
            targets,
        })
    }
    pub fn inbox_server(
        self,
    ) -> proto::concert_inbox_service_server::ConcertInboxServiceServer<Self> {
        proto::concert_inbox_service_server::ConcertInboxServiceServer::new(self)
            .max_decoding_message_size(65536)
            .max_encoding_message_size(1024 * 1024)
    }
    pub fn settlement_server(
        &self,
    ) -> lease::producer_settlement_service_server::ProducerSettlementServiceServer<
        NotificationSettlement,
    > {
        NotificationSettlement {
            store: self.store.clone(),
        }
        .server()
    }
    async fn guard(
        &self,
        message: proto::GuardRequest,
        deadline: AuthorityRpcDeadline,
        seal: bool,
    ) -> Result<proto::GuardReply, Status> {
        // Recovery validates the original immutable binding/capability but does
        // not extend or reacquire an expired owner scope.
        let parent = message.scope.ok_or_else(denied)?;
        let binding = parent.binding.as_ref().ok_or_else(denied)?;
        let wire = message.intent.ok_or_else(denied)?;
        let id = canonical(&wire.child_id)?;
        let intent = if seal {
            self.store
                .close_intent(id, &wire.completion_secret, binding, deadline)
                .await?
        } else {
            self.store
                .checked_intent(id, &wire.completion_secret, binding, deadline)
                .await?
        };
        if seal {
            let state = self.store.seal(&intent, deadline).await?;
            return Ok(proto::GuardReply {
                held: false,
                settled: matches!(
                    state,
                    NativeOutcome::Committed | NativeOutcome::Aborted | NativeOutcome::NoEffect
                ),
            });
        }
        if intent.sealed {
            return Ok(proto::GuardReply {
                held: false,
                settled: false,
            });
        }
        let scope = self.owners.inspect(parent, deadline).await?;
        scope.check()?;
        Ok(proto::GuardReply {
            held: true,
            settled: false,
        })
    }
}
#[tonic::async_trait]
impl proto::concert_inbox_service_server::ConcertInboxService for NotificationInboxService {
    async fn execute(
        &self,
        request: Request<proto::ExecuteRequest>,
    ) -> Result<Response<proto::ExecuteResponse>, Status> {
        let deadline = caller_deadline(&request)?;
        let message = request.into_inner();
        let operation = parse(&message)?;
        let scope = Arc::new(
            self.owners
                .inspect(message.scope.clone().ok_or_else(denied)?, deadline)
                .await?,
        );
        assert_operation(&scope, &operation)?;
        let targets = self
            .targets
            .clone()
            .ok_or_else(|| Status::unavailable("Notification root target policy unavailable"))?;
        let wire = message.intent.as_ref().ok_or_else(denied)?;
        // Bind operation and canonical typed input, excluding private capabilities
        // and enrolled subject lists (which legitimately change at Root).
        let mut canonical_request = message.clone();
        canonical_request.scope = None;
        canonical_request.intent = None;
        let mut hash = Sha256::new();
        hash.update(b"mcr notification typed inbox request v1\0");
        hash.update(canonical_request.encode_to_vec());
        let intent = self
            .store
            .create_intent(
                canonical(&wire.child_id)?,
                &wire.completion_secret,
                scope.binding().clone(),
                hash.finalize().into(),
                scope.deadline(),
            )
            .await?;
        let now = Utc::now();
        let context = Context {
            actor_id: scope.actor(),
            event_id: canonical(&scope.binding().event_id)?,
            session_family_id: canonical(&scope.binding().session_family_id)?,
            request_id: canonical(&scope.binding().request_id)?,
            authority_digest: scope.digest(),
            observed_at: now,
            expires_at: DateTime::from_timestamp_micros(scope.deadline().expires_micros())
                .ok_or_else(denied)?,
        };
        let authority = OperationAuthority {
            scope: scope.clone(),
            owners: self.owners.clone(),
            store: self.store.clone(),
            intent: intent.clone(),
            targets,
        };
        let inbox = PgConcertInbox::new(self.pool.clone(), authority);
        let work = async {
            use proto::execute_response::Output;
            Ok::<_, Status>(match operation {
                Operation::List(q) => {
                    let result = inbox.list(context, q).await.map_err(status)?;
                    Output::List(proto::NotificationList {
                        items: result.items.into_iter().map(notification).collect(),
                        next_cursor: result.next_cursor,
                    })
                }
                Operation::Count => {
                    let result = inbox.unread_count(context).await.map_err(status)?;
                    Output::Count(proto::UnreadCount {
                        actor_id: result.actor_id.to_string(),
                        event_id: result.event_id.to_string(),
                        unread_total: result.unread_total,
                        as_of_unix_micros: result.as_of.timestamp_micros(),
                    })
                }
                Operation::MarkRead { id, revision, key } => Output::MarkRead(notification(
                    inbox
                        .mark_read(context, id, revision, key)
                        .await
                        .map_err(status)?,
                )),
                Operation::MarkAll(key) => {
                    let result = inbox.mark_all_read(context, key).await.map_err(status)?;
                    Output::MarkAll(proto::ReadAllResult {
                        read_count: result.read_count,
                        read_at_unix_micros: result.read_at.timestamp_micros(),
                    })
                }
            })
        };
        let output = tokio::time::timeout_at(scope.deadline().stop(), work)
            .await
            .map_err(|_| Status::deadline_exceeded("Notification inbox deadline"))??;
        self.owners.check(&scope, &[]).await?;
        scope.check()?;
        Ok(no_store(proto::ExecuteResponse {
            child_id: intent.id.to_string(),
            binding_digest: intent.binding_digest.to_vec(),
            expires_unix_micros: scope.deadline().expires_micros(),
            output: Some(output),
        }))
    }
    async fn check(
        &self,
        request: Request<proto::GuardRequest>,
    ) -> Result<Response<proto::GuardReply>, Status> {
        let deadline = caller_deadline(&request)?;
        Ok(no_store(
            self.guard(request.into_inner(), deadline, false).await?,
        ))
    }
    async fn complete(
        &self,
        request: Request<proto::GuardRequest>,
    ) -> Result<Response<proto::GuardReply>, Status> {
        let deadline = caller_deadline(&request)?;
        Ok(no_store(
            self.guard(request.into_inner(), deadline, true).await?,
        ))
    }
}
struct OperationAuthority {
    scope: Arc<CheckedScope>,
    owners: Owners,
    store: AuthorityStore,
    intent: Intent,
    targets: Arc<dyn RootTargetSnapshotOwner>,
}
struct RetainedFence {
    scope: Arc<CheckedScope>,
    target: Box<dyn AuthorityFence>,
}
impl AuthorityFence for RetainedFence {
    fn check(&self, now: DateTime<Utc>) -> domain::Result<()> {
        self.scope
            .check()
            .map_err(|_| NotificationError::Unavailable)?;
        self.target.check(now)
    }
}
#[async_trait]
impl FencedInboxAuthority for OperationAuthority {
    async fn authorize_in(
        &self,
        connection: &mut PgConnection,
        context: Context,
        candidates: &[Candidate],
        now: DateTime<Utc>,
    ) -> domain::Result<Authorization> {
        let binding = self.scope.binding();
        if context.actor_id != self.scope.actor()
            || context.authority_digest != self.scope.digest()
            || context.event_id.to_string() != binding.event_id
            || context.session_family_id.to_string() != binding.session_family_id
            || context.request_id.to_string() != binding.request_id
            || context.expires_at.timestamp_micros() != self.scope.deadline().expires_micros()
        {
            return Err(NotificationError::PermissionDenied);
        }
        let subjects: Vec<_> = candidates
            .iter()
            .filter_map(|c| c.source_actor_id)
            .collect();
        let checked = self
            .owners
            .check(&self.scope, &subjects)
            .await
            .map_err(|_| NotificationError::Unavailable)?;
        let mut proof = self
            .targets
            .authorize(self.scope.wire(), context, candidates, now)
            .await?;
        proof.validate(context, candidates, Utc::now())?;
        for (candidate, visibility) in &mut proof.decisions {
            if candidate
                .source_actor_id
                .is_some_and(|id| checked.get(&id) != Some(&true))
            {
                *visibility = domain::Visibility::Hidden;
            }
        }
        let identity = self
            .store
            .capture(connection, &self.intent, self.scope.deadline())
            .await
            .map_err(|_| NotificationError::Unavailable)?;
        self.owners
            .track(&self.scope, &self.store, &self.intent, &identity)
            .await
            .map_err(|_| NotificationError::Unavailable)?;
        self.store
            .witness(connection, &self.intent, &identity, self.scope.deadline())
            .await
            .map_err(|_| NotificationError::Unavailable)?;
        proof.fence = Box::new(RetainedFence {
            scope: self.scope.clone(),
            target: proof.fence,
        });
        proof.validate(context, candidates, Utc::now())?;
        Ok(proof)
    }
}
enum Operation {
    List(Query),
    Count,
    MarkRead {
        id: uuid::Uuid,
        revision: i64,
        key: uuid::Uuid,
    },
    MarkAll(uuid::Uuid),
}
fn parse(message: &proto::ExecuteRequest) -> Result<Operation, Status> {
    use proto::{execute_request::Input, InboxOperation};
    Ok(
        match (
            InboxOperation::try_from(message.operation).map_err(|_| invalid())?,
            message.input.as_ref(),
        ) {
            (InboxOperation::List, Some(Input::List(v)))
                if (1..=100).contains(&v.limit)
                    && v.cursor.as_ref().is_none_or(|v| canonical(v).is_ok()) =>
            {
                Operation::List(Query {
                    limit: v.limit as u16,
                    unread_only: v.unread_only,
                    cursor: v.cursor.clone(),
                })
            }
            (InboxOperation::UnreadCount, Some(Input::Count(_))) => Operation::Count,
            (InboxOperation::MarkRead, Some(Input::MarkRead(v)))
                if (1..=concert_domain::MAX_REVISION).contains(&v.expected_revision) =>
            {
                Operation::MarkRead {
                    id: canonical(&v.notification_id)?,
                    revision: v.expected_revision,
                    key: canonical(&v.idempotency_key)?,
                }
            }
            (InboxOperation::MarkAllRead, Some(Input::MarkAll(v))) => {
                Operation::MarkAll(canonical(&v.idempotency_key)?)
            }
            _ => return Err(invalid()),
        },
    )
}
fn assert_operation(scope: &CheckedScope, operation: &Operation) -> Result<(), Status> {
    let b = scope.binding();
    let base = format!("/api/concert/events/{}/me/notifications", b.event_id);
    let (id, method, path) = match operation {
        Operation::List(_) => ("listConcertNotifications", "GET", base),
        Operation::Count => (
            "getConcertNotificationUnreadCount",
            "GET",
            format!("{base}/unread-count"),
        ),
        Operation::MarkRead { id, .. } => (
            "markConcertNotification",
            "PATCH",
            format!("{base}/{id}/read"),
        ),
        Operation::MarkAll(_) => (
            "markAllConcertNotifications",
            "PATCH",
            format!("{base}/read-all"),
        ),
    };
    // Root retains exact original body/query admission. Endpoint binding here
    // rejects relabeling one operation/path as another under the same parent.
    let target = b
        .http_target
        .split_once('?')
        .map_or(b.http_target.as_str(), |v| v.0);
    if b.operation != id || b.http_method != method || target != path {
        return Err(denied());
    }
    scope.check()
}
fn notification(v: domain::Notification) -> proto::Notification {
    use domain::{Kind, TargetType};
    let kind = match v.kind {
        Kind::Party => 1,
        Kind::Giveaway => 2,
        Kind::Social => 3,
        Kind::Moderation => 4,
        Kind::Notice => 5,
        Kind::Feed => 6,
    };
    let target = v.target.map(|t| proto::Target {
        target_type: match t.target_type {
            TargetType::Party => 1,
            TargetType::Application => 2,
            TargetType::Giveaway => 3,
            TargetType::Reservation => 4,
            TargetType::SocialRequest => 5,
            TargetType::Post => 6,
            TargetType::Notice => 7,
            TargetType::Report => 8,
        },
        target_id: t.target_id.to_string(),
    });
    proto::Notification {
        notification_id: v.notification_id.to_string(),
        event_id: v.event_id.to_string(),
        kind,
        target,
        created_at_unix_micros: v.created_at.timestamp_micros(),
        read_at_unix_micros: v.read_at.map(|t| t.timestamp_micros()),
        revision: v.revision,
    }
}
fn caller_deadline<T>(request: &Request<T>) -> Result<AuthorityRpcDeadline, Status> {
    if !matches!(
        request.extensions().get::<VerifiedCaller>(),
        Some(VerifiedCaller(Workload::Concert))
    ) {
        return Err(denied());
    }
    let original = platform::call_deadline::current()
        .ok_or_else(|| Status::invalid_argument("Original Notification inbox deadline required"))?;
    AuthorityRpcDeadline::within_original(
        Utc::now().timestamp_micros() + 5_000_000,
        original,
        Duration::from_secs(5),
    )
}
fn status(error: NotificationError) -> Status {
    match error {
        NotificationError::InvalidInput => invalid(),
        NotificationError::PermissionDenied => denied(),
        NotificationError::ResourceUnavailable => Status::not_found("Notification unavailable"),
        NotificationError::VersionConflict => Status::aborted("Notification version conflict"),
        NotificationError::IdempotencyConflict => {
            Status::already_exists("Notification idempotency conflict")
        }
        NotificationError::IdempotencyExpired => {
            Status::failed_precondition("Notification idempotency expired")
        }
        NotificationError::Unavailable => Status::unavailable("Notification authority unavailable"),
    }
}
fn denied() -> Status {
    Status::permission_denied("Notification inbox binding denied")
}
fn invalid() -> Status {
    Status::invalid_argument("Invalid Notification inbox operation")
}
