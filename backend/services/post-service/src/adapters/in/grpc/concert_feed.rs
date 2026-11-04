//! Isolated transport candidate. Workload authentication does NOT grant actor,
//! relationship, media or linked-object authority. The current platform cannot
//! verify Concert-forwarded user authority or fence remote writes, so all public
//! handlers deliberately return UNAVAILABLE before touching owner persistence.
//! Enabling these handlers requires a separately qualified authority protocol;
//! neither caller-supplied context/digests nor a configuration flag can enable it.
use platform::grpc_auth::{VerifiedCaller, Workload};
use platform::grpc_method_policy::MethodPolicy;
use tonic::{Request, Response, Status};

pub mod proto {
    tonic::include_proto!("post.concert.v1");
}
/// The positive implementation has no public raw constructor. Root composition
/// receives both generated services only behind their exact method policies.
pub mod qualified {
    use super::{proto, read_wire, POST_CONCERT_FEED_READ_POLICY, POST_READ_SETTLEMENT_POLICY};
    use crate::concert_feed::{
        authority_proto as wire,
        read_admission::ReadAdmission,
        read_runtime::{QualifiedReadRuntime, ReadResult},
    };
    use platform::{
        actor_delegation::{
            concert_read::ConcertPostReadVerifier, read_authority::VerifiedReadAuthority,
        },
        grpc_auth::{CallerRegistry, VerifiedCaller, Workload},
        grpc_method_policy::{self, GuardedService},
    };
    use tonic::{Request, Response, Status};

    #[derive(Clone)]
    pub struct QualifiedPostReadGrpcServer {
        runtime: QualifiedReadRuntime,
    }
    #[derive(Clone)]
    pub struct PostReadSettlementGrpcServer {
        runtime: QualifiedReadRuntime,
    }
    pub type Services = (
        GuardedService<
            proto::post_concert_feed_read_service_server::PostConcertFeedReadServiceServer<
                QualifiedPostReadGrpcServer,
            >,
        >,
        GuardedService<
            wire::producer_settlement_service_server::ProducerSettlementServiceServer<
                PostReadSettlementGrpcServer,
            >,
        >,
    );
    pub fn services(
        runtime: QualifiedReadRuntime,
        registry: CallerRegistry,
        verifier: ConcertPostReadVerifier,
    ) -> anyhow::Result<Services> {
        let read =
            proto::post_concert_feed_read_service_server::PostConcertFeedReadServiceServer::new(
                QualifiedPostReadGrpcServer {
                    runtime: runtime.clone(),
                },
            )
            .max_decoding_message_size(65536)
            .max_encoding_message_size(1048576);
        let settlement =
            wire::producer_settlement_service_server::ProducerSettlementServiceServer::new(
                PostReadSettlementGrpcServer { runtime },
            )
            .max_decoding_message_size(65536)
            .max_encoding_message_size(65536);
        Ok((
            grpc_method_policy::guard(
                &POST_CONCERT_FEED_READ_POLICY,
                read,
                registry.clone(),
                verifier.into_inner(),
            )?,
            grpc_method_policy::guard_without_delegation(
                &POST_READ_SETTLEMENT_POLICY,
                settlement,
                registry,
            )?,
        ))
    }
    fn verified<T>(r: &Request<T>) -> Result<(VerifiedCaller, VerifiedReadAuthority), Status> {
        let caller = *r
            .extensions()
            .get::<VerifiedCaller>()
            .ok_or_else(|| Status::unauthenticated("verified caller required"))?;
        if caller.0 != Workload::Concert {
            return Err(Status::permission_denied("read caller denied"));
        }
        let proof = *r
            .extensions()
            .get::<VerifiedReadAuthority>()
            .ok_or_else(|| Status::unauthenticated("verified read authority required"))?;
        Ok((caller, proof))
    }
    fn response<T>(value: T) -> Response<T> {
        let mut r = Response::new(value);
        r.metadata_mut().insert(
            "cache-control",
            "no-store".parse().expect("static metadata"),
        );
        r
    }
    fn missing() -> Status {
        Status::invalid_argument("qualified read envelope required")
    }
    fn mismatch() -> Status {
        Status::unavailable("read response unavailable")
    }
    #[tonic::async_trait]
    impl proto::post_concert_feed_read_service_server::PostConcertFeedReadService
        for QualifiedPostReadGrpcServer
    {
        async fn get_post(
            &self,
            r: Request<proto::QualifiedGetPostRequest>,
        ) -> Result<Response<proto::GetPostResponse>, Status> {
            let (caller, proof) = verified(&r)?;
            let r = r.into_inner();
            let q = read_wire::DecodedRead::try_from(r.query.ok_or_else(missing)?)?;
            let a = ReadAdmission::from_verified(
                caller,
                proof,
                q.context,
                r.authority.ok_or_else(missing)?,
            )
            .map_err(read_wire::status)?;
            match self
                .runtime
                .run(a, q.query)
                .await
                .map_err(read_wire::status)?
            {
                ReadResult::Post(v) => Ok(response(v)),
                _ => Err(mismatch()),
            }
        }
        async fn list_posts(
            &self,
            r: Request<proto::QualifiedListPostsRequest>,
        ) -> Result<Response<proto::ListPostsResponse>, Status> {
            let (caller, proof) = verified(&r)?;
            let r = r.into_inner();
            let q = read_wire::DecodedRead::try_from(r.query.ok_or_else(missing)?)?;
            let a = ReadAdmission::from_verified(
                caller,
                proof,
                q.context,
                r.authority.ok_or_else(missing)?,
            )
            .map_err(read_wire::status)?;
            match self
                .runtime
                .run(a, q.query)
                .await
                .map_err(read_wire::status)?
            {
                ReadResult::Posts(v) => Ok(response(v)),
                _ => Err(mismatch()),
            }
        }
        async fn get_comment(
            &self,
            r: Request<proto::QualifiedGetCommentRequest>,
        ) -> Result<Response<proto::GetCommentResponse>, Status> {
            let (caller, proof) = verified(&r)?;
            let r = r.into_inner();
            let q = read_wire::DecodedRead::try_from(r.query.ok_or_else(missing)?)?;
            let a = ReadAdmission::from_verified(
                caller,
                proof,
                q.context,
                r.authority.ok_or_else(missing)?,
            )
            .map_err(read_wire::status)?;
            match self
                .runtime
                .run(a, q.query)
                .await
                .map_err(read_wire::status)?
            {
                ReadResult::Comment(v) => Ok(response(v)),
                _ => Err(mismatch()),
            }
        }
        async fn list_comments(
            &self,
            r: Request<proto::QualifiedListCommentsRequest>,
        ) -> Result<Response<proto::ListCommentsResponse>, Status> {
            let (caller, proof) = verified(&r)?;
            let r = r.into_inner();
            let q = read_wire::DecodedRead::try_from(r.query.ok_or_else(missing)?)?;
            let a = ReadAdmission::from_verified(
                caller,
                proof,
                q.context,
                r.authority.ok_or_else(missing)?,
            )
            .map_err(read_wire::status)?;
            match self
                .runtime
                .run(a, q.query)
                .await
                .map_err(read_wire::status)?
            {
                ReadResult::Comments(v) => Ok(response(v)),
                _ => Err(mismatch()),
            }
        }
    }
    fn owner<T>(r: &Request<T>) -> Result<Workload, Status> {
        match r.extensions().get::<VerifiedCaller>() {
            Some(VerifiedCaller(w @ (Workload::Auth | Workload::Friend))) => Ok(*w),
            Some(_) => Err(Status::permission_denied("settlement caller denied")),
            None => Err(Status::unauthenticated("verified owner required")),
        }
    }
    #[tonic::async_trait]
    impl wire::producer_settlement_service_server::ProducerSettlementService
        for PostReadSettlementGrpcServer
    {
        async fn resolve_transaction(
            &self,
            r: Request<wire::ResolveTransactionRequest>,
        ) -> Result<Response<wire::ResolveTransactionResponse>, Status> {
            let caller = owner(&r)?;
            let original = platform::call_deadline::current()
                .ok_or_else(|| Status::invalid_argument("settlement deadline required"))?;
            let stop = tokio::time::Instant::from_std(original)
                .min(tokio::time::Instant::now() + std::time::Duration::from_secs(5));
            let work = self
                .runtime
                .tracking()
                .resolve(caller, r.into_inner(), stop);
            let value = tokio::time::timeout_at(stop, work)
                .await
                .map_err(|_| Status::unavailable("settlement unavailable"))?
                .map_err(read_wire::status)?;
            Ok(response(value))
        }
        async fn resolve_scope(
            &self,
            r: Request<wire::ResolveScopeRequest>,
        ) -> Result<Response<wire::ResolveScopeResponse>, Status> {
            owner(&r)?;
            // Acquire.response_scope belongs to Concert, whose actual HTTP
            // completion handler seals it. Absence at Post is never evidence.
            Ok(response(wire::ResolveScopeResponse {
                scope: r.into_inner().scope,
                sealed_and_finished: false,
            }))
        }
    }
}
const CONCERT: &[Workload] = &[Workload::Concert];
pub static POST_CONCERT_FEED_POLICY: MethodPolicy = MethodPolicy {
    service: "post.concert.v1.PostConcertFeedService",
    workload: Some(Workload::Post),
    methods: &[
        ("ExecuteCommand", CONCERT),
        ("GetReceipt", CONCERT),
        ("GetPost", CONCERT),
        ("ListPosts", CONCERT),
        ("ListComments", CONCERT),
        ("GetComment", CONCERT),
    ],
    // No unverified actor field is admitted here. Every handler stops before
    // any read/effect. Adding actor forwarding must use its qualified guard.
    actor_fields: &[],
};
/// Dedicated whole-envelope actor policy. A passing signature still requires
/// both actual owner views and the retained Concert subject callback.
pub static POST_CONCERT_FEED_READ_POLICY: MethodPolicy = MethodPolicy {
    service: "post.concert.v1.PostConcertFeedReadService",
    workload: Some(Workload::Post),
    methods: &[
        ("GetPost", CONCERT),
        ("ListPosts", CONCERT),
        ("GetComment", CONCERT),
        ("ListComments", CONCERT),
    ],
    actor_fields: &[
        ("GetPost", &[&[1, 1, 1]]),
        ("ListPosts", &[&[1, 1, 1]]),
        ("GetComment", &[&[1, 1, 1]]),
        ("ListComments", &[&[1, 1, 1]]),
    ],
};
/// Settlement is owner-to-producer reconciliation, never a user read grant.
pub static POST_READ_SETTLEMENT_POLICY: MethodPolicy = MethodPolicy {
    service: "authority.lease.v1.ProducerSettlementService",
    workload: Some(Workload::Post),
    methods: &[
        ("ResolveTransaction", &[Workload::Auth, Workload::Friend]),
        ("ResolveScope", &[Workload::Auth, Workload::Friend]),
    ],
    actor_fields: &[],
};
#[derive(Clone, Default)]
pub struct PostConcertFeedGrpcServer;
fn admission<T>(request: &Request<T>) -> Status {
    match request.extensions().get::<VerifiedCaller>() {
        None => Status::unauthenticated("verified service identity required"),
        Some(VerifiedCaller(Workload::Concert)) => {
            Status::unavailable("Concert Post owner authority is not qualified")
        }
        Some(_) => Status::permission_denied("Concert Post owner caller denied"),
    }
}
#[tonic::async_trait]
impl proto::post_concert_feed_service_server::PostConcertFeedService for PostConcertFeedGrpcServer {
    async fn execute_command(
        &self,
        r: Request<proto::ExecuteCommandRequest>,
    ) -> Result<Response<proto::Receipt>, Status> {
        Err(admission(&r))
    }
    async fn get_receipt(
        &self,
        r: Request<proto::GetReceiptRequest>,
    ) -> Result<Response<proto::GetReceiptResponse>, Status> {
        Err(admission(&r))
    }
    async fn get_post(
        &self,
        r: Request<proto::GetPostRequest>,
    ) -> Result<Response<proto::GetPostResponse>, Status> {
        Err(admission(&r))
    }
    async fn list_posts(
        &self,
        r: Request<proto::ListPostsRequest>,
    ) -> Result<Response<proto::ListPostsResponse>, Status> {
        Err(admission(&r))
    }
    async fn get_comment(
        &self,
        r: Request<proto::GetCommentRequest>,
    ) -> Result<Response<proto::GetCommentResponse>, Status> {
        Err(admission(&r))
    }
    async fn list_comments(
        &self,
        r: Request<proto::ListCommentsRequest>,
    ) -> Result<Response<proto::ListCommentsResponse>, Status> {
        Err(admission(&r))
    }
}
/// Strict read DTO conversion shared by the eventual qualified read handler.
/// Decoding supplies no authority. The workload-only server above remains closed.
pub mod read_wire {
    use super::proto;
    use chrono::{DateTime, Utc};
    use post_application::concert_feed::{Context, Filter, Page, Position, PostView, Window};
    use post_domain::concert_feed::{Comment, Error, LinkKind, State};
    use tonic::Status;
    use uuid::Uuid;

    pub struct DecodedRead {
        pub context: Context,
        pub query: ReadQuery,
    }
    pub enum ReadQuery {
        Post(Uuid),
        Posts(Window),
        Comment(Uuid),
        Comments {
            post: Uuid,
            upper: DateTime<Utc>,
            after: Option<Position>,
            limit: u32,
        },
    }
    fn id(value: &str) -> Result<Uuid, Status> {
        Uuid::parse_str(value)
            .ok()
            .filter(|id| !id.is_nil() && id.to_string() == value)
            .ok_or_else(|| Status::invalid_argument("invalid canonical read binding"))
    }
    fn time(value: &str) -> Result<DateTime<Utc>, Status> {
        DateTime::parse_from_rfc3339(value)
            .map(|v| v.with_timezone(&Utc))
            .map_err(|_| Status::invalid_argument("invalid read timestamp"))
    }
    fn context(value: Option<proto::Context>) -> Result<Context, Status> {
        let c = value.ok_or_else(|| Status::invalid_argument("read context required"))?;
        let context = Context {
            actor_id: id(&c.actor_id)?,
            event_id: id(&c.event_id)?,
            session_family_id: id(&c.session_family_id)?,
            request_id: id(&c.request_id)?,
            original_authority: c
                .original_authority_digest
                .try_into()
                .map_err(|_| Status::invalid_argument("invalid read binding"))?,
            expires_at: time(&c.expires_at)?,
        };
        context.validate(Utc::now()).map_err(status)?;
        Ok(context)
    }
    fn position(value: Option<proto::Position>) -> Result<Option<Position>, Status> {
        value
            .map(|p| {
                Ok(Position {
                    created_at: time(&p.created_at)?,
                    id: id(&p.id)?,
                })
            })
            .transpose()
    }
    fn window(
        upper: &str,
        after: Option<proto::Position>,
        filter: Filter,
        limit: u32,
    ) -> Result<Window, Status> {
        let w = Window {
            upper: time(upper)?,
            after: position(after)?,
            filter,
            limit,
        };
        w.validate().map_err(status)?;
        if w.upper > Utc::now() {
            return Err(Status::invalid_argument("future read window"));
        }
        Ok(w)
    }
    impl TryFrom<proto::GetPostRequest> for DecodedRead {
        type Error = Status;
        fn try_from(r: proto::GetPostRequest) -> Result<Self, Status> {
            Ok(Self {
                context: context(r.context)?,
                query: ReadQuery::Post(id(&r.post_id)?),
            })
        }
    }
    impl TryFrom<proto::GetCommentRequest> for DecodedRead {
        type Error = Status;
        fn try_from(r: proto::GetCommentRequest) -> Result<Self, Status> {
            Ok(Self {
                context: context(r.context)?,
                query: ReadQuery::Comment(id(&r.comment_id)?),
            })
        }
    }
    impl TryFrom<proto::ListPostsRequest> for DecodedRead {
        type Error = Status;
        fn try_from(r: proto::ListPostsRequest) -> Result<Self, Status> {
            let filter = match r.filter.as_str() {
                "ALL" => Filter::All,
                "WITH_MEDIA" => Filter::WithMedia,
                "PARTY_LINK" => Filter::PartyLink,
                "GIVEAWAY_LINK" => Filter::GiveawayLink,
                _ => return Err(Status::invalid_argument("invalid feed filter")),
            };
            Ok(Self {
                context: context(r.context)?,
                query: ReadQuery::Posts(window(&r.upper, r.after, filter, r.limit)?),
            })
        }
    }
    impl TryFrom<proto::ListCommentsRequest> for DecodedRead {
        type Error = Status;
        fn try_from(r: proto::ListCommentsRequest) -> Result<Self, Status> {
            let w = window(&r.upper, r.after, Filter::All, r.limit)?;
            Ok(Self {
                context: context(r.context)?,
                query: ReadQuery::Comments {
                    post: id(&r.post_id)?,
                    upper: w.upper,
                    after: w.after,
                    limit: w.limit,
                },
            })
        }
    }
    pub fn status(error: Error) -> Status {
        match error {
            Error::Invalid => Status::invalid_argument("invalid feed read"),
            Error::Forbidden | Error::NotFound => Status::not_found("feed resource unavailable"),
            Error::Conflict | Error::Idempotency => Status::aborted("feed resource changed"),
            Error::Authority | Error::Unavailable => {
                Status::unavailable("feed read authority unavailable")
            }
        }
    }
    fn state(value: State) -> String {
        match value {
            State::Published => "PUBLISHED",
            State::Hidden => "HIDDEN",
            State::Deleted => "DELETED",
        }
        .into()
    }
    pub fn post(value: PostView) -> proto::PostRecord {
        let p = value.post;
        proto::PostRecord {
            post_id: p.post_id.to_string(),
            event_id: p.event_id.to_string(),
            author_id: p.author_id.to_string(),
            content: Some(proto::Content {
                text: p.content.text,
                media_ids: p
                    .content
                    .media_ids
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                links: p
                    .content
                    .links
                    .into_iter()
                    .map(|l| proto::Link {
                        target_id: l.target_id.to_string(),
                        target_type: match l.target_type {
                            LinkKind::Party => "PARTY",
                            LinkKind::Giveaway => "GIVEAWAY",
                            LinkKind::Post => "POST",
                            LinkKind::Intro => "INTRO",
                            LinkKind::Mural => "MURAL",
                            LinkKind::Notice => "NOTICE",
                        }
                        .into(),
                    })
                    .collect(),
            }),
            revision: p.revision,
            state: state(p.state),
            created_at: p.created_at.to_rfc3339(),
            edited_at: p.edited_at.map(|v| v.to_rfc3339()),
            comment_count: p.comment_count,
            reaction_count: p.reaction_count,
            reacted_by_me: value.reacted_by_me,
            is_public: true,
        }
    }
    pub fn comment(c: Comment) -> proto::CommentRecord {
        proto::CommentRecord {
            comment_id: c.comment_id.to_string(),
            post_id: c.post_id.to_string(),
            event_id: c.event_id.to_string(),
            author_id: c.author_id.to_string(),
            text: c.text,
            revision: c.revision,
            state: state(c.state),
            created_at: c.created_at.to_rfc3339(),
            edited_at: c.edited_at.map(|v| v.to_rfc3339()),
        }
    }
    fn scan(value: Option<Position>) -> Option<proto::Position> {
        value.map(|p| proto::Position {
            created_at: p.created_at.to_rfc3339(),
            id: p.id.to_string(),
        })
    }
    pub fn posts(page: Page<PostView>) -> proto::ListPostsResponse {
        proto::ListPostsResponse {
            posts: page.items.into_iter().map(post).collect(),
            scanned_through: scan(page.scanned_through),
            has_more: page.has_more,
        }
    }
    pub fn comments(page: Page<Comment>) -> proto::ListCommentsResponse {
        proto::ListCommentsResponse {
            comments: page.items.into_iter().map(comment).collect(),
            scanned_through: scan(page.scanned_through),
            has_more: page.has_more,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use platform::grpc_method_policy::proto_rpc_names;
    use std::collections::BTreeSet;
    #[test]
    fn isolated_method_table_matches_exact_proto_and_only_concert() {
        let declared: BTreeSet<_> = proto_rpc_names(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proto/post/v1/concert_feed.proto"
        )))
        .into_iter()
        .collect();
        let listed: BTreeSet<_> = POST_CONCERT_FEED_POLICY
            .method_names()
            .map(str::to_owned)
            .collect();
        assert_eq!(declared, listed);
        for method in POST_CONCERT_FEED_POLICY.method_names() {
            for caller in Workload::ALL {
                assert_eq!(
                    POST_CONCERT_FEED_POLICY.allows(
                        &format!("/post.concert.v1.PostConcertFeedService/{method}"),
                        caller
                    ),
                    caller == Workload::Concert
                );
            }
        }
    }
    #[test]
    fn caller_context_cannot_activate_production_authority() {
        let mut r = Request::new(proto::ExecuteCommandRequest::default());
        assert_eq!(admission(&r).code(), tonic::Code::Unauthenticated);
        r.extensions_mut().insert(VerifiedCaller(Workload::Gateway));
        assert_eq!(admission(&r).code(), tonic::Code::PermissionDenied);
        r.extensions_mut().insert(VerifiedCaller(Workload::Concert));
        assert_eq!(admission(&r).code(), tonic::Code::Unavailable);
    }
}
