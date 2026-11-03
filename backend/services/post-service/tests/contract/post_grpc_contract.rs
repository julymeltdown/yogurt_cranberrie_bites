use platform::grpc_auth::{VerifiedCaller, Workload};
use post_service::adapters::r#in::grpc::server::proto;
use tonic::{Code, Request};
use uuid::Uuid;

/// Build a request carrying the verified identity that the interceptor attaches.
///
/// These tests call the generated trait method directly, which bypasses the
/// server interceptor. Attaching the extension keeps the authorization path
/// under test instead of skipping it.
fn as_caller<T>(message: T, caller: Workload) -> Request<T> {
    let mut request = Request::new(message);
    request.extensions_mut().insert(VerifiedCaller(caller));
    request
}

fn as_gateway<T>(message: T) -> Request<T> {
    as_caller(message, Workload::Gateway)
}

#[tokio::test]
async fn grpc_post_contract_covers_lifecycle_queries_and_reactions() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let grpc = crate::support::grpc_server(app.state.clone());

    let public_post = proto::post_service_server::PostService::create_post(
        &grpc,
        as_gateway(proto::CreatePostRequest {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Gateway parity public post".into(),
            visibility: "PUBLIC".into(),
        }),
    )
    .await
    .expect("create public")
    .into_inner()
    .post
    .expect("post");

    let friends_post = proto::post_service_server::PostService::create_post(
        &grpc,
        as_gateway(proto::CreatePostRequest {
            author_id: "33333333-3333-3333-3333-333333333333".into(),
            content: "Friends only parity note".into(),
            visibility: "FRIENDS".into(),
        }),
    )
    .await
    .expect("create friends")
    .into_inner()
    .post
    .expect("post");

    let feed_before_worker_ticks = proto::post_service_server::PostService::get_feed(
        &grpc,
        Request::new(proto::FeedRequest {
            user_id: "11111111-1111-1111-1111-111111111111".into(),
            cursor: String::new(),
            limit: 10,
            request_id: "contract-request".into(),
            session_id: String::new(),
        }),
    )
    .await
    .expect("feed")
    .into_inner();
    assert_eq!(feed_before_worker_ticks.rank_version, "feed-linear-v1");
    assert!(!feed_before_worker_ticks.fallback_used);
    assert!(feed_before_worker_ticks
        .posts
        .iter()
        .all(|post| post.id != friends_post.id));

    assert_eq!(
        app.state
            .outbox_worker
            .tick(chrono::Utc::now(), 10)
            .await
            .expect("outbox worker tick"),
        2
    );
    assert_eq!(
        app.state
            .timeline_fanout_worker
            .tick()
            .await
            .expect("timeline fanout worker tick"),
        2
    );

    let feed = proto::post_service_server::PostService::get_feed(
        &grpc,
        Request::new(proto::FeedRequest {
            user_id: "11111111-1111-1111-1111-111111111111".into(),
            cursor: String::new(),
            limit: 10,
            request_id: "contract-request-after-worker-ticks".into(),
            session_id: String::new(),
        }),
    )
    .await
    .expect("ranked feed after worker ticks")
    .into_inner();
    let expected_ranked_ids = [public_post.id.clone(), friends_post.id.clone()];
    assert_eq!(feed.rank_version, "feed-linear-v1");
    assert!(!feed.fallback_used);
    assert_eq!(feed.posts.len(), expected_ranked_ids.len());
    assert!(expected_ranked_ids
        .iter()
        .all(|post_id| feed.posts.iter().any(|post| &post.id == post_id)));
    assert!(expected_ranked_ids.iter().all(|post_id| {
        feed.items
            .iter()
            .any(|item| &item.post_id == post_id && item.source == "MATERIALIZED_TIMELINE")
    }));

    let author_posts = proto::post_service_server::PostService::get_author_posts(
        &grpc,
        Request::new(proto::AuthorPostsRequest {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            viewer_id: "44444444-4444-4444-4444-444444444444".into(),
            cursor: String::new(),
            limit: 10,
        }),
    )
    .await
    .expect("author posts")
    .into_inner();
    assert_eq!(author_posts.posts.len(), 1);
    assert_eq!(author_posts.posts[0].id, public_post.id);

    let search = proto::post_service_server::PostService::search_posts(
        &grpc,
        Request::new(proto::SearchPostsRequest {
            query: "parity".into(),
            viewer_id: "11111111-1111-1111-1111-111111111111".into(),
            cursor: String::new(),
            limit: 10,
        }),
    )
    .await
    .expect("search")
    .into_inner();
    assert_eq!(search.posts.len(), 2);

    let comment = proto::post_service_server::PostService::create_comment(
        &grpc,
        as_gateway(proto::CreateCommentRequest {
            post_id: public_post.id.clone(),
            author_id: "11111111-1111-1111-1111-111111111111".into(),
            content: "first comment".into(),
        }),
    )
    .await
    .expect("comment")
    .into_inner()
    .comment
    .expect("comment");
    assert_eq!(comment.post_id, public_post.id);

    let like = proto::post_service_server::PostService::like_post(
        &grpc,
        as_gateway(proto::LikePostRequest {
            post_id: public_post.id.clone(),
            user_id: "11111111-1111-1111-1111-111111111111".into(),
        }),
    )
    .await
    .expect("like")
    .into_inner();
    assert_eq!(like.like_count, 1);
    assert!(like.liked);

    let detail = proto::post_service_server::PostService::get_post(
        &grpc,
        Request::new(proto::GetPostRequest {
            post_id: public_post.id.clone(),
            viewer_id: "11111111-1111-1111-1111-111111111111".into(),
        }),
    )
    .await
    .expect("detail")
    .into_inner();
    assert_eq!(detail.comments.len(), 1);
    assert_eq!(detail.comments[0].content, "first comment");
    assert!(detail.post.expect("post").liked_by_me);

    let deleted_comment = proto::post_service_server::PostService::delete_comment(
        &grpc,
        Request::new(proto::DeleteCommentRequest {
            comment_id: comment.id.clone(),
            actor_id: "11111111-1111-1111-1111-111111111111".into(),
            request_id: "grpc-delete-comment-1".into(),
            ip_address: "198.51.100.8".into(),
            user_agent: "post-grpc-contract".into(),
        }),
    )
    .await
    .expect("delete comment")
    .into_inner();
    assert!(deleted_comment.deleted);
    assert_eq!(deleted_comment.comment_id, comment.id);

    let detail_after_comment_delete = proto::post_service_server::PostService::get_post(
        &grpc,
        Request::new(proto::GetPostRequest {
            post_id: public_post.id.clone(),
            viewer_id: "11111111-1111-1111-1111-111111111111".into(),
        }),
    )
    .await
    .expect("detail after comment delete")
    .into_inner();
    assert!(detail_after_comment_delete.comments.is_empty());
    assert_eq!(
        detail_after_comment_delete
            .post
            .expect("post")
            .comment_count,
        0
    );

    let unlike = proto::post_service_server::PostService::unlike_post(
        &grpc,
        as_gateway(proto::UnlikePostRequest {
            post_id: public_post.id.clone(),
            user_id: "11111111-1111-1111-1111-111111111111".into(),
        }),
    )
    .await
    .expect("unlike")
    .into_inner();
    assert_eq!(unlike.like_count, 0);
    assert!(!unlike.liked);

    let trending = proto::post_service_server::PostService::get_trending(
        &grpc,
        Request::new(proto::TrendingRequest {
            viewer_id: "11111111-1111-1111-1111-111111111111".into(),
            cursor: String::new(),
            limit: 10,
        }),
    )
    .await
    .expect("trending")
    .into_inner();
    let trending_ids: Vec<Uuid> = trending
        .posts
        .into_iter()
        .map(|post| post.id.parse().expect("uuid"))
        .collect();
    assert!(trending_ids.contains(&Uuid::parse_str(&public_post.id).expect("uuid")));
    assert!(trending_ids.contains(&Uuid::parse_str(&friends_post.id).expect("uuid")));

    let deleted = proto::post_service_server::PostService::delete_post(
        &grpc,
        as_caller(
            proto::DeletePostRequest {
                post_id: public_post.id.clone(),
                actor_id: "22222222-2222-2222-2222-222222222222".into(),
                request_id: "grpc-delete-1".into(),
                ip_address: "198.51.100.7".into(),
                user_agent: "post-grpc-contract".into(),
            },
            Workload::Gateway,
        ),
    )
    .await
    .expect("delete")
    .into_inner();
    assert!(deleted.deleted);

    let deleted_detail = proto::post_service_server::PostService::get_post(
        &grpc,
        Request::new(proto::GetPostRequest {
            post_id: public_post.id,
            viewer_id: "11111111-1111-1111-1111-111111111111".into(),
        }),
    )
    .await
    .expect_err("deleted post should be hidden");
    assert_eq!(deleted_detail.code(), Code::NotFound);
}

#[tokio::test]
async fn deletion_and_moderation_are_denied_to_unauthorized_or_anonymous_callers() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let grpc = crate::support::grpc_server(app.state.clone());

    // Account cleanup is the auth coordinator's job. A sibling service or the
    // edge presenting itself here must be refused, and only after the identity
    // is known, so an anonymous caller is refused differently.
    for caller in [
        Some(Workload::Gateway),
        Some(Workload::Post),
        Some(Workload::Profile),
    ] {
        let message = proto::ApplyUserDeletionRequest {
            saga_id: "saga-1".into(),
            user_id: "33333333-3333-3333-3333-333333333333".into(),
            actor_id: "33333333-3333-3333-3333-333333333333".into(),
            retention_until: "2030-01-01T00:00:00Z".into(),
        };
        let request = match caller {
            Some(caller) => as_caller(message, caller),
            None => Request::new(message),
        };
        let error = proto::post_service_server::PostService::apply_user_deletion(&grpc, request)
            .await
            .expect_err("unauthorized account deletion must fail");
        let expected = if caller.is_some() {
            Code::PermissionDenied
        } else {
            Code::Unauthenticated
        };
        assert_eq!(error.code(), expected, "caller {caller:?}");
    }

    // Moderation is an operator action from the edge; a sibling service may not
    // drive it.
    let denied = proto::post_service_server::PostService::apply_content_moderation(
        &grpc,
        as_caller(
            proto::ApplyContentModerationRequest {
                report_id: "report-1".into(),
                target_type: "POST".into(),
                target_id: "44444444-4444-4444-4444-444444444444".into(),
                actor_id: "55555555-5555-5555-5555-555555555555".into(),
                action: "HIDE".into(),
            },
            Workload::Post,
        ),
    )
    .await
    .expect_err("sibling moderation must be denied");
    assert_eq!(denied.code(), Code::PermissionDenied);

    let denied = proto::post_service_server::PostService::resolve_dead_letter_replay(
        &grpc,
        as_caller(
            proto::ResolveDeadLetterReplayRequest {
                event_id: "44444444-4444-4444-4444-444444444444".into(),
                resolution_id: "55555555-5555-5555-5555-555555555555".into(),
                actor_id: "33333333-3333-3333-3333-333333333333".into(),
                reason: "An operator-approved retry after broker recovery".into(),
                evidence_reference: "OPS-1234".into(),
            },
            Workload::Post,
        ),
    )
    .await
    .expect_err("sibling services cannot replay operator dead letters");
    assert_eq!(denied.code(), Code::PermissionDenied);
}

#[tokio::test]
async fn gateway_can_record_an_audited_dead_letter_resolution_over_grpc() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let grpc = crate::support::grpc_server(app.state.clone());
    let post = proto::post_service_server::PostService::create_post(
        &grpc,
        as_gateway(proto::CreatePostRequest {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Gateway-authorized dead-letter retry".into(),
            visibility: "PUBLIC".into(),
        }),
    )
    .await
    .expect("create post")
    .into_inner()
    .post
    .expect("post");
    let event_id: Uuid = sqlx::query_scalar(
        "select event_id from post_event_outbox where aggregate_id = $1 and event_type = 'POST_CREATED'",
    )
    .bind(Uuid::parse_str(&post.id).expect("post ID"))
    .fetch_one(&app.pool)
    .await
    .expect("outbox event");
    sqlx::query(
        "update post_event_outbox set attempt_count = 5, dead_lettered_at = now() where event_id = $1",
    )
    .bind(event_id)
    .execute(&app.pool)
    .await
    .expect("dead-letter event");

    let request = proto::ResolveDeadLetterReplayRequest {
        event_id: event_id.to_string(),
        resolution_id: Uuid::new_v4().to_string(),
        actor_id: "33333333-3333-3333-3333-333333333333".into(),
        reason: "Broker access was restored and the event is safe to retry".into(),
        evidence_reference: "OPS-incident/INC-2026-1042".into(),
    };
    let response = proto::post_service_server::PostService::resolve_dead_letter_replay(
        &grpc,
        as_gateway(request.clone()),
    )
    .await
    .expect("Gateway is authorized to resolve dead letters")
    .into_inner();
    assert!(response.created);
    assert_eq!(response.event_id, event_id.to_string());

    let retry = proto::post_service_server::PostService::resolve_dead_letter_replay(
        &grpc,
        as_gateway(request.clone()),
    )
    .await
    .expect("identical gRPC retry is idempotent")
    .into_inner();
    assert!(!retry.created);
    assert_eq!(retry.resolution_id, response.resolution_id);

    let audit: (Uuid, String, String, String) = sqlx::query_as(
        "select event_id, actor_id::text, reason, evidence_reference from post_event_dead_letter_resolutions where resolution_id = $1",
    )
    .bind(Uuid::parse_str(&response.resolution_id).expect("resolution ID"))
    .fetch_one(&app.pool)
    .await
    .expect("operator decision audit");
    assert_eq!(audit.0, event_id);
    assert_eq!(audit.1, request.actor_id);
    assert_eq!(audit.2, request.reason);
    assert_eq!(audit.3, request.evidence_reference);
}
