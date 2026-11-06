//! Event-scoped Post mutations inside Root's existing canonical scope. Root owns
//! scope acquisition, delivery and response completion; this producer only maps
//! one admitted command to the shared Feed queue and the actual Post owner.
use super::{
    command_transport::{OriginalIntent, QualifiedCommandTransport},
    owner::{self, proto as post_proto, PostOwner, PostTransport},
    policy::GuardedPolicy,
    product::{bound_context, FeedProductConfig},
    queue::{FeedQueue, FencedFeedAuthority},
    resources::GuardedResources,
    wire,
};
use crate::{
    adapters::out::{
        public_profile::PublicProfileReader, relationship_authority::guarded::OwnerGuardScope,
        SystemClock,
    },
    product_rpc::{
        canonical_uuid, proto as rpc, scoped::ProductRepresentation, Operation, VerifiedProduct,
    },
};
use async_trait::async_trait;
use chrono::Utc;
use concert_application::{
    commands::{CommandError, FingerprintKeys, ReceiptState, RegisteredOperation, Scope},
    feed::{
        CommentReader, Context, Feed, FeedResources, PostReader, PreparedCommand, ProjectedComment,
        ProjectedPost, ScannedComments, ScannedPosts,
    },
};
use concert_domain::{
    feed::{Comment, FeedCommand, FeedError, Filter, Post, Window},
    EventPhase,
};
use platform::{
    concert_feed_command::{FeedCommandRoute, FeedCommandSigner},
    grpc_tls::TlsMaterial,
};
use serde_json::{json, Value};
use sqlx::{PgConnection, PgPool};
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

pub struct FeedCommandProducer {
    pool: PgPool,
    config: FeedProductConfig,
    tls: TlsMaterial,
    write: FeedCommandSigner,
    profiles: Arc<PublicProfileReader>,
    keys: Arc<FingerprintKeys>,
}
impl FeedCommandProducer {
    pub fn new(
        pool: PgPool,
        config: FeedProductConfig,
        tls: TlsMaterial,
        write: FeedCommandSigner,
        profiles: Arc<PublicProfileReader>,
        keys: Arc<FingerprintKeys>,
    ) -> anyhow::Result<Self> {
        tls.endpoint(&config.endpoint, platform::grpc_auth::Workload::Post)?;
        Ok(Self {
            pool,
            config,
            tls,
            write,
            profiles,
            keys,
        })
    }
    pub fn handles(context: &VerifiedProduct) -> bool {
        matches!(context.operation(), Operation::FeedCommand(_))
    }
    pub async fn dispatch(
        &self,
        verified: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        request: rpc::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        match self.dispatch_inner(verified, scope, request).await {
            Ok(value) => Ok(value),
            Err(error) => Ok(error_representation(error.code())),
        }
    }
    async fn dispatch_inner(
        &self,
        verified: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        request: rpc::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        verified.check()?;
        let context = bound_context(verified, &scope, &request)?;
        if !self.writable(context.event).await? {
            return Ok(representation(409, "EVENT_READ_ONLY"));
        }
        let Operation::FeedCommand(route) = verified.operation() else {
            return Err(Status::permission_denied("Unsupported Feed command"));
        };
        let key = canonical_uuid(&request.idempotency_key)?;
        let command = crate::product_rpc::query::feed_command(&request, route)?;
        let target = PreparedCommand::target_for(context, key, &command).map_err(feed_status)?;
        let operation = RegisteredOperation::find(route.operation())
            .map_err(|_| Status::invalid_argument("Unsupported Feed operation"))?;
        let stored_scope = Scope {
            actor: context.actor,
            event: context.event,
            operation,
            key,
        };
        let queue = FeedQueue::new(self.pool.clone());
        let authority = GuardedFeedAuthority {
            scope: scope.clone(),
        };
        let transport = QualifiedCommandTransport::connect(
            &self.config.endpoint,
            &self.config.credential,
            &self.tls,
            self.write.clone(),
            scope.clone(),
            OriginalIntent {
                method: request.http_method.clone(),
                target: request.http_target.clone(),
                body: request.http_body.clone(),
                content_type: request.content_type.clone(),
                key: request.idempotency_key.clone(),
            },
        )
        .map_err(|_| Status::unavailable("Feed command owner unavailable"))?;
        let reader = CommandReader {
            transport: transport.clone(),
            context,
            key,
            command: command.clone(),
            target,
        };
        let prepared = match queue.load(stored_scope).await.map_err(status)? {
            Some(stored) => {
                if stored.command != command
                    || PreparedCommand::target_for(context, key, &stored.command)
                        .map_err(feed_status)?
                        != stored.target
                {
                    return Ok(error_representation(
                        Status::already_exists("Feed idempotency conflict").code(),
                    ));
                }
                let revision = stored.receipt.reference.map(|reference| reference.revision);
                let prepared = PreparedCommand::restore(
                    context,
                    key,
                    stored.command,
                    stored.parent,
                    stored.parent_revision,
                )
                .map_err(feed_status)?;
                if stored.receipt.state == ReceiptState::Succeeded {
                    let revision = revision.ok_or_else(|| {
                        Status::unavailable("Feed receipt unavailable")
                    })?;
                    let value = self
                        .project(context, scope.clone(), route, &reader, target, revision)
                        .await?;
                    return Ok(ProductRepresentation {
                        status: 200,
                        content_type: "application/json".into(),
                        body: serde_json::to_vec(&value)
                            .map_err(|_| Status::unavailable("Feed representation unavailable"))?,
                    });
                }
                prepared
            }
            None => {
                let resources = GuardedResources::new(scope.clone(), self.profiles.clone())
                    .map_err(feed_status)?;
                let policy = GuardedPolicy::new(scope.clone(), self.profiles.clone())
                    .map_err(feed_status)?;
                Feed {
                    reader: reader.clone(),
                    policy,
                    resources,
                    clock: SystemClock,
                }
                .prepare(context, key, command.clone())
                .await
                .map_err(feed_status)?
            }
        };
        let receipt = queue
            .enqueue(&prepared, &self.keys, &authority)
            .await
            .map_err(status)?;
        let owner = PostOwner { transport };
        let commit = owner
            .execute(&prepared, &receipt)
            .await
            .map_err(feed_status)?;
        queue
            .acknowledge(&prepared, commit, &authority)
            .await
            .map_err(status)?;
        verified.check()?;
        scope.binding().check()?;
        let value = self
            .project(
                context,
                scope.clone(),
                route,
                &reader,
                target,
                commit.revision,
            )
            .await?;
        Ok(ProductRepresentation {
            status: 200,
            content_type: "application/json".into(),
            body: serde_json::to_vec(&value)
                .map_err(|_| Status::unavailable("Feed representation unavailable"))?,
        })
    }
    async fn writable(&self, event: Uuid) -> Result<bool, Status> {
        let phase: Option<String> =
            sqlx::query_scalar("select phase from public.concert_events where event_id=$1")
                .bind(event)
                .fetch_optional(&self.pool)
                .await
                .map_err(|_| Status::unavailable("Feed event unavailable"))?;
        let phase = phase.ok_or_else(|| Status::not_found("Feed event unavailable"))?;
        let phase: EventPhase = phase
            .parse()
            .map_err(|_| Status::unavailable("Feed event unavailable"))?;
        Ok(phase.permits_participant_writes())
    }
    async fn project(
        &self,
        context: Context,
        scope: Arc<OwnerGuardScope>,
        route: FeedCommandRoute,
        reader: &CommandReader,
        target: Uuid,
        revision: i64,
    ) -> Result<Value, Status> {
        match route {
            FeedCommandRoute::CreatePost | FeedCommandRoute::EditPost(_) => {
                let record = reader
                    .preview()
                    .await
                    .map_err(feed_status)?
                    .post
                    .ok_or_else(|| Status::unavailable("Feed post unavailable"))?;
                let post = owner::post(context, record).map_err(feed_status)?;
                let author = GuardedResources::new(scope, self.profiles.clone())
                    .map_err(feed_status)?
                    .author(context, post.author)
                    .await
                    .map_err(feed_status)?;
                wire::post(ProjectedPost { post, author }).map_err(feed_status)
            }
            FeedCommandRoute::CreateComment(_) | FeedCommandRoute::EditComment(_) => {
                let record = reader
                    .preview()
                    .await
                    .map_err(feed_status)?
                    .comment
                    .ok_or_else(|| Status::unavailable("Feed comment unavailable"))?;
                let comment = owner::comment(context, record).map_err(feed_status)?;
                let author = GuardedResources::new(scope, self.profiles.clone())
                    .map_err(feed_status)?
                    .author(context, comment.author)
                    .await
                    .map_err(feed_status)?;
                wire::comment(ProjectedComment { comment, author }).map_err(feed_status)
            }
            FeedCommandRoute::DeletePost(..) | FeedCommandRoute::DeleteComment(..) => Ok(json!({
                "resource_id": target,
                "revision": revision,
                "state": "DELETED"
            })),
            FeedCommandRoute::Like(_) | FeedCommandRoute::Unlike(_) => {
                let record = reader
                    .preview()
                    .await
                    .map_err(feed_status)?
                    .post
                    .ok_or_else(|| Status::unavailable("Feed post unavailable"))?;
                let post = owner::post(context, record).map_err(feed_status)?;
                Ok(json!({
                    "post_id": post.id,
                    "reacted_by_me": post.reacted_by_me,
                    "reaction_count": post.reaction_count,
                    "revision": post.revision
                }))
            }
        }
    }
}

/// The exact command target/parent read from Post under the same whole-envelope
/// authority. It uses the dedicated preview RPC and never opens the public read
/// service with a mutation binding.
#[derive(Clone)]
struct CommandReader {
    transport: QualifiedCommandTransport,
    context: Context,
    key: Uuid,
    command: FeedCommand,
    target: Uuid,
}
impl CommandReader {
    async fn preview(&self) -> Result<post_proto::PreviewCommandResponse, FeedError> {
        let command =
            owner::preview_command_message(self.context, self.key, self.target, &self.command)?;
        self.transport
            .preview(post_proto::ExecuteCommandRequest {
                context: Some(owner::wire_context(self.context)?),
                command: Some(command),
            })
            .await
            .map_err(owner::failure)
    }
}
#[async_trait]
impl PostReader for CommandReader {
    async fn page(
        &self,
        _: Context,
        _: Window,
        _: Filter,
        _: u16,
    ) -> Result<ScannedPosts, FeedError> {
        Err(FeedError::DependencyUnavailable)
    }
    async fn post(&self, context: Context, id: Uuid) -> Result<Option<Post>, FeedError> {
        let record = self
            .preview()
            .await?
            .post
            .ok_or(FeedError::ResourceUnavailable)?;
        let post = owner::post(context, record)?;
        if post.id != id {
            return Err(FeedError::DependencyUnavailable);
        }
        Ok(Some(post))
    }
}
#[async_trait]
impl CommentReader for CommandReader {
    async fn comment(&self, context: Context, id: Uuid) -> Result<Option<Comment>, FeedError> {
        let record = self
            .preview()
            .await?
            .comment
            .ok_or(FeedError::ResourceUnavailable)?;
        let comment = owner::comment(context, record)?;
        if comment.id != id {
            return Err(FeedError::DependencyUnavailable);
        }
        Ok(Some(comment))
    }
    async fn comments(
        &self,
        _: Context,
        _: Uuid,
        _: Window,
        _: u16,
    ) -> Result<ScannedComments, FeedError> {
        Err(FeedError::DependencyUnavailable)
    }
}

fn feed_status(error: FeedError) -> Status {
    let (status, _) = wire::error(error);
    match status {
        400 => Status::invalid_argument("Invalid Feed request"),
        403 => Status::permission_denied("Feed permission denied"),
        404 => Status::not_found("Feed resource unavailable"),
        409 => Status::aborted("Feed version conflict"),
        _ => Status::unavailable("Feed owner unavailable"),
    }
}
fn status(error: CommandError) -> Status {
    match error {
        CommandError::Conflict => Status::already_exists("Feed idempotency conflict"),
        CommandError::Expired => Status::out_of_range("Feed idempotency expired"),
        CommandError::Denied => Status::permission_denied("Feed command denied"),
        CommandError::InvalidInput => Status::invalid_argument("Invalid Feed command"),
        CommandError::Capacity => Status::resource_exhausted("Feed command rate limited"),
        CommandError::InvalidState => Status::aborted("Feed command state conflict"),
        CommandError::Unavailable => Status::unavailable("Feed command unavailable"),
    }
}
fn error_representation(code: tonic::Code) -> ProductRepresentation {
    let (status, reason) = match code {
        tonic::Code::InvalidArgument => (400u32, "INVALID_INPUT"),
        tonic::Code::Unauthenticated => (401, "AUTHENTICATION_REQUIRED"),
        tonic::Code::PermissionDenied => (403, "PERMISSION_DENIED"),
        tonic::Code::NotFound => (404, "RESOURCE_UNAVAILABLE"),
        tonic::Code::AlreadyExists => (409, "IDEMPOTENCY_CONFLICT"),
        tonic::Code::OutOfRange => (409, "IDEMPOTENCY_EXPIRED"),
        tonic::Code::Aborted => (409, "VERSION_CONFLICT"),
        tonic::Code::ResourceExhausted => (429, "RATE_LIMITED"),
        tonic::Code::DeadlineExceeded => (504, "DEADLINE_EXCEEDED"),
        _ => (503, "DEPENDENCY_UNAVAILABLE"),
    };
    representation(status, reason)
}
fn representation(status: u32, reason: &'static str) -> ProductRepresentation {
    ProductRepresentation {
        status,
        content_type: "application/json".into(),
        body: serde_json::to_vec(&json!({
            "code": format!("CONCERT_{status}"),
            "message": "Concert request unavailable",
            "traceId": Uuid::new_v4().to_string(),
            "timestamp": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "details": { "reason": reason }
        }))
        .expect("fixed JSON projection"),
    }
}
struct GuardedFeedAuthority {
    scope: Arc<OwnerGuardScope>,
}
impl GuardedFeedAuthority {
    async fn local_write(
        &self,
        c: &mut PgConnection,
        context: Context,
    ) -> Result<(), CommandError> {
        let binding = self.scope.binding();
        binding.check().map_err(|_| CommandError::Denied)?;
        binding
            .assert_context(
                context.actor,
                context.event,
                context.session_family,
                context.request,
                context.authority_digest,
                context.expires_at,
            )
            .map_err(|_| CommandError::Denied)?;
        let phase: String = sqlx::query_scalar(
            "select phase from public.concert_events where event_id=$1 for share",
        )
        .bind(context.event)
        .fetch_optional(&mut *c)
        .await
        .map_err(|_| CommandError::Unavailable)?
        .ok_or(CommandError::Denied)?;
        let phase: EventPhase = phase.parse().map_err(|_| CommandError::Unavailable)?;
        if !phase.permits_participant_writes() {
            return Err(CommandError::InvalidState);
        }
        let active: bool = sqlx::query_scalar(
            "select state='ACTIVE' from public.concert_event_admissions where event_id=$1 and user_id=$2 for share",
        )
        .bind(context.event)
        .bind(context.actor)
        .fetch_optional(&mut *c)
        .await
        .map_err(|_| CommandError::Unavailable)?
        .ok_or(CommandError::Denied)?;
        if !active {
            return Err(CommandError::Denied);
        }
        sqlx::query(
            "select pg_advisory_xact_lock(hashtextextended('mcr-world-account-deletion:' || $1::uuid::text,0))",
        )
        .bind(context.actor)
        .execute(&mut *c)
        .await
        .map_err(|_| CommandError::Unavailable)?;
        let fenced: bool = sqlx::query_scalar(
            "select exists(select 1 from public.concert_world_account_deletion_fences where user_id=$1)",
        )
        .bind(context.actor)
        .fetch_one(&mut *c)
        .await
        .map_err(|_| CommandError::Unavailable)?;
        if fenced {
            return Err(CommandError::Denied);
        }
        Ok(())
    }
}
#[async_trait]
impl FencedFeedAuthority for GuardedFeedAuthority {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        p: &PreparedCommand,
    ) -> Result<(), CommandError> {
        p.context()
            .check(Utc::now())
            .map_err(|_| CommandError::Denied)?;
        self.local_write(c, p.context()).await
    }
    async fn stored_in(
        &self,
        c: &mut PgConnection,
        context: Context,
        scope: Scope,
        target: Uuid,
        parent: Option<Uuid>,
    ) -> Result<(), CommandError> {
        if target.is_nil()
            || scope.actor != context.actor
            || scope.event != context.event
            || parent.is_some_and(|id| id.is_nil())
        {
            return Err(CommandError::Denied);
        }
        self.local_write(c, context).await
    }
    async fn first_execution_in(
        &self,
        c: &mut PgConnection,
        p: &PreparedCommand,
    ) -> Result<(), CommandError> {
        self.check_in(c, p).await?;
        if let FeedCommand::CreatePost { content } | FeedCommand::EditPost { content, .. } =
            p.command()
        {
            if !content.media_ids.is_empty() || !content.links.is_empty() {
                return Err(CommandError::Unavailable);
            }
        }
        Ok(())
    }
}
