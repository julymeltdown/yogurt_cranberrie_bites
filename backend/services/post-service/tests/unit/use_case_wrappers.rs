use crate::support::{comment, post, uuid, Harness, ACTOR_ID};
use post_service::application::dto::post_dto::{
    CreateCommentDto, CreatePostDto, DeleteCommentDto, PurgeExpiredPostsDto, ReplayRequestDto,
};
use post_service::application::use_cases::{
    create_comment::CreateCommentUseCase, create_post::CreatePostUseCase,
    delete_comment::DeleteCommentUseCase, delete_post::DeletePostUseCase,
    get_author_posts::GetAuthorPostsUseCase, get_feed::GetFeedUseCase,
    get_post_detail::GetPostDetailUseCase, get_trending::GetTrendingUseCase,
    like_post::LikePostUseCase, purge_expired_posts::PurgeExpiredPostsUseCase,
    replay_events::ReplayEventsUseCase, search_posts::SearchPostsUseCase,
    unlike_post::UnlikePostUseCase,
};
use post_service::domain::feed::FeedPage;
use post_service::domain::feed::MIN_SEARCH_BATCH;
use post_service::domain::post::{PostDetail, ReplayTarget, RetentionPurgeResult};

#[tokio::test]
async fn create_post_wrapper_rejects_blank_content_before_profile_lookup() {
    let harness = Harness::new();
    let use_case = CreatePostUseCase::new(harness.service.clone());

    let error = use_case
        .execute(CreatePostDto {
            author_id: ACTOR_ID.to_owned(),
            content: "  ".to_owned(),
            visibility: "PUBLIC".to_owned(),
        })
        .await
        .expect_err("blank content");

    assert_eq!(error.to_string(), "Content is required");
    assert!(harness.profile.fetch_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn comment_wrappers_create_and_soft_delete_with_events() {
    let harness = Harness::new();
    let post_id = uuid("22222222-2222-2222-2222-222222222201");
    let author_id = uuid("22222222-2222-2222-2222-222222222202");
    let comment_id = uuid("22222222-2222-2222-2222-222222222203");
    harness.posts.insert_detail(PostDetail {
        post: post(post_id, author_id, "commented"),
        comments: Vec::new(),
    });
    harness
        .posts
        .insert_comment(comment(comment_id, post_id, author_id, "old comment"));

    let created = CreateCommentUseCase::new(harness.service.clone())
        .execute(CreateCommentDto {
            post_id: post_id.to_string(),
            author_id: author_id.to_string(),
            content: "  new comment  ".to_owned(),
        })
        .await
        .expect("create comment");
    let deleted = DeleteCommentUseCase::new(harness.service.clone())
        .execute(DeleteCommentDto {
            comment_id: comment_id.to_string(),
            actor_id: author_id.to_string(),
            request_id: Some(" req ".to_owned()),
            ip_address: None,
            user_agent: Some(" unit ".to_owned()),
        })
        .await
        .expect("delete comment");

    assert_eq!(created.content, "new comment");
    assert_eq!(deleted.comment_id, comment_id.to_string());
    assert_eq!(
        harness.posts.saved_comment_events.lock().unwrap()[0].event_type,
        "POST_COMMENTED"
    );
    assert_eq!(
        harness.posts.deleted_comment_events.lock().unwrap()[0].event_type,
        "POST_COMMENT_DELETED"
    );
}

#[tokio::test]
async fn feed_wrappers_filter_visibility_trim_queries_and_normalize_limits() {
    let harness = Harness::new();
    let viewer_id = uuid("22222222-2222-2222-2222-222222222204");
    let author_id = uuid("22222222-2222-2222-2222-222222222205");
    let visible_post = post(
        uuid("22222222-2222-2222-2222-222222222206"),
        author_id,
        "visible",
    );
    let hidden_post = post_service::domain::post::Post {
        visibility: "PRIVATE".to_owned(),
        ..post(
            uuid("22222222-2222-2222-2222-222222222207"),
            author_id,
            "hidden",
        )
    };
    *harness.posts.author_page.lock().unwrap() = Some(FeedPage {
        posts: vec![visible_post.clone(), hidden_post],
        next_cursor: Some("author-next".to_owned()),
    });
    // Search scans past filtered rows, so its fixture is a scripted sequence: one
    // batch that ends the matching set. A single short batch is now the end of the
    // results rather than a page with a continuation, which is what the next
    // assertion pins down.
    harness
        .posts
        .search_pages
        .lock()
        .unwrap()
        .extend([FeedPage {
            posts: vec![visible_post.clone()],
            next_cursor: None,
        }]);
    *harness.posts.trending_page.lock().unwrap() = Some(FeedPage {
        posts: vec![visible_post.clone()],
        next_cursor: Some("trend-next".to_owned()),
    });
    *harness.posts.feed_page.lock().unwrap() = Some(FeedPage {
        posts: vec![visible_post],
        next_cursor: Some("feed-next".to_owned()),
    });

    let author_feed = GetAuthorPostsUseCase::new(harness.service.clone())
        .execute(
            author_id,
            viewer_id,
            Some("2026-01-20T09:10:00+00:00|22222222-2222-2222-2222-222222222206"),
            0,
        )
        .await
        .expect("author posts");
    let search_feed = SearchPostsUseCase::new(harness.service.clone())
        .execute(viewer_id, "  rust  ", None, -1)
        .await
        .expect("search posts");
    // A wide page is asked for exactly, which is where limit normalization is
    // observable: the batch equals the page size when it exceeds the minimum scan
    // width.
    SearchPostsUseCase::new(harness.service.clone())
        .execute(viewer_id, "rust", None, 4_000)
        .await
        .expect("wide search posts");
    let trending_feed = GetTrendingUseCase::new(harness.service.clone())
        .execute(viewer_id, None, 2)
        .await
        .expect("trending posts");
    let home_feed = GetFeedUseCase::new(harness.service.clone())
        .execute(viewer_id, None, 3)
        .await
        .expect("home feed");

    assert_eq!(author_feed.posts.len(), 1);
    assert_eq!(author_feed.next_cursor.as_deref(), Some("author-next"));
    // The only batch ended, so there is nothing more to fetch and the page says so
    // rather than handing back a cursor that would return the same rows.
    assert_eq!(search_feed.next_cursor, None);
    assert_eq!(search_feed.posts.len(), 1);
    assert_eq!(trending_feed.next_cursor.as_deref(), Some("trend-next"));
    assert_eq!(home_feed.next_cursor.as_deref(), Some("feed-next"));
    assert_eq!(
        harness.posts.author_calls.lock().unwrap()[0],
        (
            author_id,
            Some("2026-01-20T09:10:00+00:00|22222222-2222-2222-2222-222222222206".to_owned()),
            20,
        )
    );
    // The batch is at least the minimum scan width, not exactly the page size: a
    // mostly-hidden result set would otherwise spend the row budget one page at a
    // time.
    assert_eq!(
        harness.posts.search_calls.lock().unwrap()[0],
        ("rust".to_owned(), None, MIN_SEARCH_BATCH as i32)
    );
    assert_eq!(
        harness.posts.search_calls.lock().unwrap()[1],
        ("rust".to_owned(), None, 100),
        "a page wider than the minimum scan width is requested at its own size, and \
         the limit stays bounded"
    );
}

/// A hidden batch must not end the scan while readable rows follow it.
#[tokio::test]
async fn search_scans_past_a_fully_hidden_batch_and_names_the_page_boundary() {
    let harness = Harness::new();
    let viewer_id = uuid("22222222-2222-2222-2222-222222222210");
    let author_id = uuid("22222222-2222-2222-2222-222222222211");
    let hidden = post_service::domain::post::Post {
        visibility: "PRIVATE".to_owned(),
        ..post(
            uuid("22222222-2222-2222-2222-222222222212"),
            author_id,
            "hidden",
        )
    };
    let first = post(
        uuid("22222222-2222-2222-2222-222222222213"),
        author_id,
        "first",
    );
    let second = post(
        uuid("22222222-2222-2222-2222-222222222214"),
        author_id,
        "second",
    );
    // The first batch is entirely unreadable and still has rows behind it, so a
    // single-batch implementation would answer with an empty page.
    harness.posts.search_pages.lock().unwrap().extend([
        FeedPage {
            posts: vec![hidden],
            next_cursor: Some("batch-two".to_owned()),
        },
        FeedPage {
            posts: vec![first.clone(), second.clone()],
            next_cursor: None,
        },
    ]);

    let page = SearchPostsUseCase::new(harness.service.clone())
        .execute(viewer_id, "rust", None, 2)
        .await
        .expect("search posts");

    assert_eq!(
        page.posts
            .iter()
            .map(|post| post.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            first.id.to_string().as_str(),
            second.id.to_string().as_str()
        ],
        "the scan must continue past the hidden batch and return the readable rows"
    );
    assert_eq!(
        page.next_cursor, None,
        "the matching set ended, so there is no continuation to offer"
    );
    // Both batches were requested, which is the behaviour under test.
    assert_eq!(harness.posts.search_calls.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn post_detail_and_like_wrappers_return_viewer_specific_counts() {
    let harness = Harness::new();
    let post_id = uuid("22222222-2222-2222-2222-222222222208");
    let author_id = uuid("22222222-2222-2222-2222-222222222209");
    let viewer_id = uuid("22222222-2222-2222-2222-222222222210");
    harness.posts.insert_detail(PostDetail {
        post: post(post_id, author_id, "detail"),
        comments: vec![comment(
            uuid("22222222-2222-2222-2222-222222222211"),
            post_id,
            author_id,
            "reply",
        )],
    });
    harness
        .likes
        .like_counts
        .lock()
        .unwrap()
        .insert(post_id, 12);
    harness.likes.liked_ids.lock().unwrap().insert(post_id);
    *harness.likes.add_like_count.lock().unwrap() = 13;
    *harness.likes.remove_like_count.lock().unwrap() = 11;

    let detail = GetPostDetailUseCase::new(harness.service.clone())
        .execute(post_id, viewer_id)
        .await
        .expect("post detail");
    let liked = LikePostUseCase::new(harness.service.clone())
        .execute(post_id, viewer_id)
        .await
        .expect("like post");
    let unliked = UnlikePostUseCase::new(harness.service.clone())
        .execute(post_id, viewer_id)
        .await
        .expect("unlike post");

    assert_eq!(detail.post.like_count, 12);
    assert!(detail.post.liked_by_me);
    assert_eq!(detail.comments.len(), 1);
    assert_eq!(liked.like_count, 13);
    assert!(!unliked.liked);
    assert_eq!(
        harness.likes.add_events.lock().unwrap()[0].event_type,
        "POST_LIKED"
    );
    assert_eq!(
        harness.likes.remove_events.lock().unwrap()[0].event_type,
        "POST_UNLIKED"
    );
}

#[tokio::test]
async fn delete_post_replay_and_purge_wrappers_surface_service_effects() {
    let harness = Harness::new();
    let post_id = uuid("22222222-2222-2222-2222-222222222212");
    let event_id = uuid("22222222-2222-2222-2222-222222222213");
    let actor_id = uuid(ACTOR_ID);
    harness.posts.insert_detail(PostDetail {
        post: post(post_id, actor_id, "owned"),
        comments: Vec::new(),
    });
    *harness.posts.purge_result.lock().unwrap() = RetentionPurgeResult {
        post_ids: vec![post_id],
        comment_ids: vec![uuid("22222222-2222-2222-2222-222222222214")],
    };
    *harness.outbox.events.lock().unwrap() = vec![crate::support::stored_event(
        event_id,
        "POST_CREATED",
        serde_json::json!({ "authorId": actor_id }).to_string(),
    )];

    let deleted = DeletePostUseCase::new(harness.service.clone())
        .execute(post_service::application::dto::post_dto::DeletePostDto {
            post_id: post_id.to_string(),
            actor_id: actor_id.to_string(),
            request_id: None,
            ip_address: None,
            user_agent: None,
        })
        .await
        .expect("delete post");
    let replayed = ReplayEventsUseCase::new(
        harness.outbox.clone(),
        harness.service.replay_delivery.clone(),
    )
    .execute(ReplayRequestDto {
        actor_id: None,
        event_type: Some("POST_CREATED".to_owned()),
        attribute: None,
        from: None,
        to: None,
        published: None,
        target: Some(ReplayTarget::External),
        limit: None,
    })
    .await
    .expect("replay events");
    let purged = PurgeExpiredPostsUseCase::new(harness.service.clone())
        .execute(PurgeExpiredPostsDto {
            actor_id: actor_id.to_string(),
            request_id: Some(" purge ".to_owned()),
            ip_address: None,
            user_agent: None,
            dry_run: true,
        })
        .await
        .expect("purge expired posts");

    assert!(deleted.deleted);
    assert_eq!(replayed.replayed_count, 1);
    assert_eq!(*harness.publisher.published.lock().unwrap(), vec![event_id]);
    assert_eq!(purged.purged_count, 2);
    assert_eq!(
        harness.posts.purge_calls.lock().unwrap()[0]
            .request_id
            .as_deref(),
        Some("purge")
    );
}
