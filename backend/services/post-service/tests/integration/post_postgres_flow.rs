use chrono::{DateTime, Utc};
use post_service::application::dto::post_dto::{
    ApplyContentModerationDto, CreateCommentDto, CreatePostDto, DeleteCommentDto, DeletePostDto,
    PurgeExpiredPostsDto,
};
use post_service::domain::post::RelationshipPolicySnapshot;
use uuid::Uuid;

type SoftDeleteRow = (
    Option<DateTime<Utc>>,
    Option<Uuid>,
    Option<String>,
    Option<DateTime<Utc>>,
);

#[tokio::test]
async fn author_post_count_is_global_active_and_excludes_hidden_or_deleted_posts() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let author_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("uuid");

    let public_post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: author_id.to_string(),
            content: "count public post".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("public post");
    let friends_post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: author_id.to_string(),
            content: "count friends post".into(),
            visibility: "FRIENDS".into(),
        })
        .await
        .expect("friends post");
    app.state
        .create_post
        .execute(CreatePostDto {
            author_id: author_id.to_string(),
            content: "count private post".into(),
            visibility: "PRIVATE".into(),
        })
        .await
        .expect("private post");

    let initial = app
        .state
        .get_author_post_counts
        .execute(vec![author_id])
        .await
        .expect("initial author post count");
    assert_eq!(initial[0].post_count, 3);

    app.state
        .apply_content_moderation
        .execute(ApplyContentModerationDto {
            report_id: Uuid::new_v4().to_string(),
            target_type: "POST".into(),
            target_id: public_post.id,
            actor_id: Uuid::new_v4().to_string(),
            action: "HIDE".into(),
        })
        .await
        .expect("hide public post");
    let after_moderation = app
        .state
        .get_author_post_counts
        .execute(vec![author_id])
        .await
        .expect("count after moderation");
    assert_eq!(after_moderation[0].post_count, 2);

    app.state
        .delete_post
        .execute(DeletePostDto {
            post_id: friends_post.id,
            actor_id: author_id.to_string(),
            request_id: Some("post-count-delete".into()),
            ip_address: None,
            user_agent: None,
        })
        .await
        .expect("delete friends-only post");
    let after_delete = app
        .state
        .get_author_post_counts
        .execute(vec![author_id])
        .await
        .expect("count after deletion");
    assert_eq!(after_delete[0].post_count, 1);
    assert_eq!(
        after_delete[0].author_id,
        author_id.to_string(),
        "the surviving count is the active PRIVATE post, so totals do not vary by post visibility"
    );
}

#[tokio::test]
async fn a_delete_during_feed_fill_is_not_visible_to_subsequent_reads() -> anyhow::Result<()> {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let author_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("uuid");
    let follower_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid");

    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: author_id.to_string(),
            content: "Concurrent delete and feed fill".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");

    let (feed, barrier) = app.chronological_feed_with_write_barrier();
    let in_flight_feed = tokio::spawn(async move { feed.execute(follower_id, None, 10).await });

    // The database page has been read and the page is ready to be cached, but
    // its write has not yet run. Freeze the exact stale-fill window.
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        barrier.wait_until_reached(),
    )
    .await
    .expect("feed request reaches the cache write barrier");

    app.state
        .delete_post
        .execute(DeletePostDto {
            post_id: post.id.clone(),
            actor_id: author_id.to_string(),
            request_id: Some("req-concurrent-feed-delete".into()),
            ip_address: None,
            user_agent: None,
        })
        .await
        .expect("delete commits while the read is in flight");

    // This read began before the delete committed, so it may finish with its
    // earlier snapshot. A new request must validate the cached ID against the
    // current Postgres lifecycle state and suppress the soft-deleted post.
    barrier.release();
    let in_flight = in_flight_feed.await??;
    assert!(in_flight.posts.iter().any(|item| item.id == post.id));

    let after_delete = app
        .chronological_feed
        .execute(follower_id, None, 10)
        .await?;
    assert!(
        !after_delete.posts.iter().any(|item| item.id == post.id),
        "a stale fill completed after deletion must not be visible to a later request"
    );

    Ok(())
}

#[tokio::test]
async fn cached_feed_uses_current_block_and_unblock_policy_before_outbox_eviction(
) -> anyhow::Result<()> {
    let app = crate::support::start_postgres_redis_app(false).await?;
    let viewer_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111")?;
    let author_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222")?;
    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: author_id.to_string(),
            content: "Current relationship policy beats a warm feed cache".into(),
            visibility: "PUBLIC".into(),
        })
        .await?;

    let initial = app.chronological_feed.execute(viewer_id, None, 10).await?;
    assert!(initial.posts.iter().any(|item| item.id == post.id));
    assert!(app.feed_cache_key_exists(viewer_id, None, 10).await?);

    for (blocked, expected_visible) in [(true, false), (false, true)] {
        let (feed, barrier) = app.chronological_feed_with_relationship_policy_barrier();
        let in_flight = tokio::spawn(async move { feed.execute(viewer_id, None, 10).await });
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            barrier.wait_until_reached(),
        )
        .await
        .expect("warm-cache read reaches the relationship authorization boundary");

        // The feed is still warm in Redis and no outbox worker has evicted it.
        // Change the downstream policy while the read is in flight, then let
        // PostService obtain the current snapshot at its service boundary.
        app.set_relationship_policy(
            viewer_id,
            author_id,
            RelationshipPolicySnapshot {
                following: !blocked,
                blocked_viewer: blocked,
                ..RelationshipPolicySnapshot::default()
            },
        );
        barrier.release();

        let response = in_flight.await??;
        assert_eq!(
            response.posts.iter().any(|item| item.id == post.id),
            expected_visible,
            "cached post visibility must follow the current block state"
        );
        assert!(app.feed_cache_key_exists(viewer_id, None, 10).await?);
    }

    Ok(())
}

#[tokio::test]
async fn deleting_post_soft_deletes_row_records_audit_and_hides_cached_reads() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let author_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("uuid");
    let follower_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid");

    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: author_id.to_string(),
            content: "Soft delete retention probe".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");
    let post_id = Uuid::parse_str(&post.id).expect("post id");

    let warm_feed = app
        .state
        .get_feed
        .execute(follower_id, None, 10)
        .await
        .expect("warm feed");
    assert!(warm_feed.posts.iter().any(|item| item.id == post.id));

    let deleted = app
        .state
        .delete_post
        .execute(DeletePostDto {
            post_id: post.id.clone(),
            actor_id: author_id.to_string(),
            request_id: Some("req-soft-delete-1".into()),
            ip_address: Some("203.0.113.10".into()),
            user_agent: Some("post-soft-delete-test".into()),
        })
        .await
        .expect("delete");
    assert!(deleted.deleted);

    let row: SoftDeleteRow = sqlx::query_as(
        r#"
            select deleted_at, deleted_by, delete_reason, retention_until
            from posts
            where id = $1
            "#,
    )
    .bind(post_id)
    .fetch_one(&app.pool)
    .await
    .expect("post row");
    let deleted_at = row.0.expect("deleted_at");
    assert_eq!(row.1, Some(author_id));
    assert_eq!(row.2.as_deref(), Some("user_request"));
    assert!(row.3.expect("retention_until") > deleted_at);

    let audit_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from post_audit_events
        where post_id = $1
          and actor_id = $2
          and action = 'POST_DELETED'
          and outcome = 'SUCCESS'
          and request_id = 'req-soft-delete-1'
          and ip_address = '203.0.113.10'
          and user_agent = 'post-soft-delete-test'
        "#,
    )
    .bind(post_id)
    .bind(author_id)
    .fetch_one(&app.pool)
    .await
    .expect("audit count");
    assert_eq!(audit_count, 1);

    let outbox_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from post_event_outbox
        where aggregate_id = $1
          and actor_id = $2
          and event_type = 'POST_DELETED'
          and published = false
        "#,
    )
    .bind(post_id)
    .bind(author_id)
    .fetch_one(&app.pool)
    .await
    .expect("outbox count");
    assert_eq!(outbox_count, 1);

    let detail_error = app
        .state
        .get_post_detail
        .execute(post_id, follower_id)
        .await
        .expect_err("deleted post detail should be hidden");
    assert_eq!(detail_error.to_string(), "Post not found");

    app.outbox_forwarder()
        .republish_unpublished(Utc::now(), 10)
        .await
        .expect("forward delete event");

    let feed = app
        .state
        .get_feed
        .execute(follower_id, None, 10)
        .await
        .expect("feed");
    assert!(feed.posts.iter().all(|item| item.id != post.id));

    let author_posts = app
        .state
        .get_author_posts
        .execute(author_id, follower_id, None, 10)
        .await
        .expect("author posts");
    assert!(author_posts.posts.iter().all(|item| item.id != post.id));

    let search = app
        .state
        .search_posts
        .execute(follower_id, "soft delete retention", None, 10)
        .await
        .expect("search");
    assert!(search.posts.iter().all(|item| item.id != post.id));

    let trending = app
        .state
        .get_trending
        .execute(follower_id, None, 10)
        .await
        .expect("trending");
    assert!(trending.posts.iter().all(|item| item.id != post.id));
}

#[tokio::test]
async fn search_treats_wildcard_characters_as_literal_text() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let author_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("uuid");

    // One post contains a literal percent sign; another does not.
    let with_wildcard = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: author_id.to_string(),
            content: "alpha beta".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post without wildcard");
    let plain = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: author_id.to_string(),
            content: "gamma a% delta".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post containing a literal percent sign");

    // Searching for a literal percent sign must match only the post containing
    // one. The query is chosen so the two behaviours differ: unescaped, the
    // pattern "%a%%" also matches any post containing the letter a, so the first
    // post would wrongly appear.
    let results = app
        .state
        .search_posts
        .execute(author_id, "a%", None, 10)
        .await
        .expect("search");
    let ids: Vec<&str> = results.posts.iter().map(|post| post.id.as_str()).collect();
    assert!(
        ids.contains(&plain.id.as_str()),
        "the post containing a literal percent sign must match"
    );
    assert!(
        !ids.contains(&with_wildcard.id.as_str()),
        "a post without the literal text must not match"
    );
}

#[tokio::test]
async fn outbox_forwarder_invalidates_explicit_chronological_fallback_cache_keys() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let author_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("uuid");
    let follower_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid");

    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: author_id.to_string(),
            content: "Outbox cache invalidation probe".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");
    let post_id = Uuid::parse_str(&post.id).expect("post id");

    let warm_feed = app
        .chronological_feed
        .execute(follower_id, None, 10)
        .await
        .expect("warm feed");
    assert!(warm_feed.posts.iter().any(|item| item.id == post.id));
    assert!(app
        .feed_cache_key_exists(follower_id, None, 10)
        .await
        .expect("feed cache key"));
    assert!(app
        .post_cache_key_exists(post_id)
        .await
        .expect("post cache key"));

    app.state
        .delete_post
        .execute(DeletePostDto {
            post_id: post.id.clone(),
            actor_id: author_id.to_string(),
            request_id: Some("req-outbox-cache-delete".into()),
            ip_address: Some("203.0.113.60".into()),
            user_agent: Some("post-cache-invalidation-test".into()),
        })
        .await
        .expect("delete");

    assert!(app
        .feed_cache_key_exists(follower_id, None, 10)
        .await
        .expect("feed cache key before worker"));
    assert!(app
        .post_cache_key_exists(post_id)
        .await
        .expect("post cache key before worker"));

    // Both caches still contain the post while the outbox worker is pending.
    // A cache hit must validate current lifecycle state against PostgreSQL and
    // must not resurrect the soft-deleted post during this window.
    let stale_cache_read = app
        .chronological_feed
        .execute(follower_id, None, 10)
        .await
        .expect("feed read while cache invalidation is pending");
    assert!(!stale_cache_read.posts.iter().any(|item| item.id == post.id));

    let forwarded = app
        .outbox_forwarder()
        .republish_unpublished(Utc::now(), 10)
        .await
        .expect("forward unpublished events");
    assert_eq!(forwarded, 2);
    assert!(!app
        .feed_cache_key_exists(follower_id, None, 10)
        .await
        .expect("feed cache key after worker"));
    assert!(!app
        .post_cache_key_exists(post_id)
        .await
        .expect("post cache key after worker"));
}

#[tokio::test]
async fn deleting_comment_soft_deletes_row_records_audit_updates_counter_and_hides_detail() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let author_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("uuid");
    let commenter_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid");

    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: author_id.to_string(),
            content: "Comment soft delete probe".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");
    let post_id = Uuid::parse_str(&post.id).expect("post id");
    let comment = app
        .state
        .create_comment
        .execute(CreateCommentDto {
            post_id: post.id.clone(),
            author_id: commenter_id.to_string(),
            content: "comment to soft delete".into(),
        })
        .await
        .expect("comment");
    let comment_id = Uuid::parse_str(&comment.id).expect("comment id");

    let deleted = app
        .state
        .delete_comment
        .execute(DeleteCommentDto {
            comment_id: comment.id.clone(),
            actor_id: commenter_id.to_string(),
            request_id: Some("req-comment-soft-delete-1".into()),
            ip_address: Some("203.0.113.80".into()),
            user_agent: Some("comment-soft-delete-test".into()),
        })
        .await
        .expect("delete comment");
    assert!(deleted.deleted);
    assert_eq!(deleted.comment_id, comment.id);
    assert_eq!(deleted.post_id, post.id);

    let row: SoftDeleteRow = sqlx::query_as(
        r#"
        select deleted_at, deleted_by, delete_reason, retention_until
        from comments
        where id = $1
        "#,
    )
    .bind(comment_id)
    .fetch_one(&app.pool)
    .await
    .expect("comment row");
    let deleted_at = row.0.expect("deleted_at");
    assert_eq!(row.1, Some(commenter_id));
    assert_eq!(row.2.as_deref(), Some("user_request"));
    assert!(row.3.expect("retention_until") > deleted_at);

    let stored_counter: i64 = sqlx::query_scalar("select comment_count from posts where id = $1")
        .bind(post_id)
        .fetch_one(&app.pool)
        .await
        .expect("comment counter");
    assert_eq!(stored_counter, 0);

    let detail = app
        .state
        .get_post_detail
        .execute(post_id, commenter_id)
        .await
        .expect("detail");
    assert!(detail.comments.iter().all(|item| item.id != comment.id));

    let audit_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from post_audit_events
        where post_id = $1
          and comment_id = $2
          and actor_id = $3
          and action = 'COMMENT_DELETED'
          and outcome = 'SUCCESS'
          and request_id = 'req-comment-soft-delete-1'
          and ip_address = '203.0.113.80'
          and user_agent = 'comment-soft-delete-test'
        "#,
    )
    .bind(post_id)
    .bind(comment_id)
    .bind(commenter_id)
    .fetch_one(&app.pool)
    .await
    .expect("audit count");
    assert_eq!(audit_count, 1);

    let outbox_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from post_event_outbox
        where aggregate_id = $1
          and actor_id = $2
          and event_type = 'POST_COMMENT_DELETED'
          and published = false
        "#,
    )
    .bind(post_id)
    .bind(commenter_id)
    .fetch_one(&app.pool)
    .await
    .expect("outbox count");
    assert_eq!(outbox_count, 1);
}

#[tokio::test]
async fn purging_expired_post_retention_hard_deletes_only_expired_rows_and_records_audit() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let author_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("uuid");
    let commenter_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid");
    let operator_id = Uuid::parse_str("99999999-9999-9999-9999-999999999999").expect("uuid");

    let expired = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: author_id.to_string(),
            content: "Expired retention post".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("expired post");
    let retained = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: author_id.to_string(),
            content: "Retained retention post".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("retained post");
    let expired_id = Uuid::parse_str(&expired.id).expect("expired id");
    let retained_id = Uuid::parse_str(&retained.id).expect("retained id");

    app.state
        .create_comment
        .execute(CreateCommentDto {
            post_id: expired.id.clone(),
            author_id: commenter_id.to_string(),
            content: "comment to purge with post".into(),
        })
        .await
        .expect("comment");
    app.state
        .like_post
        .execute(expired_id, commenter_id)
        .await
        .expect("like");

    for post_id in [expired_id, retained_id] {
        app.state
            .delete_post
            .execute(DeletePostDto {
                post_id: post_id.to_string(),
                actor_id: author_id.to_string(),
                request_id: Some(format!("req-delete-{post_id}")),
                ip_address: Some("203.0.113.50".into()),
                user_agent: Some("post-retention-test".into()),
            })
            .await
            .expect("delete post");
    }

    sqlx::query(
        r#"
        update posts
        set retention_until = now() - interval '1 day'
        where id = $1
        "#,
    )
    .bind(expired_id)
    .execute(&app.pool)
    .await
    .expect("expire post");

    let purged = app
        .state
        .purge_expired_posts
        .execute(PurgeExpiredPostsDto {
            actor_id: operator_id.to_string(),
            request_id: Some("req-post-retention-purge-1".into()),
            ip_address: Some("203.0.113.51".into()),
            user_agent: Some("post-retention-purge-test".into()),
            dry_run: false,
        })
        .await
        .expect("purge posts");
    assert_eq!(purged.purged_count, 1);

    let expired_post_count: i64 = sqlx::query_scalar("select count(*) from posts where id = $1")
        .bind(expired_id)
        .fetch_one(&app.pool)
        .await
        .expect("expired post count");
    assert_eq!(expired_post_count, 0);
    let expired_comment_count: i64 =
        sqlx::query_scalar("select count(*) from comments where post_id = $1")
            .bind(expired_id)
            .fetch_one(&app.pool)
            .await
            .expect("expired comment count");
    assert_eq!(expired_comment_count, 0);
    let expired_like_count: i64 =
        sqlx::query_scalar("select count(*) from post_likes where post_id = $1")
            .bind(expired_id)
            .fetch_one(&app.pool)
            .await
            .expect("expired like count");
    assert_eq!(expired_like_count, 0);

    let retained_post_count: i64 = sqlx::query_scalar("select count(*) from posts where id = $1")
        .bind(retained_id)
        .fetch_one(&app.pool)
        .await
        .expect("retained post count");
    assert_eq!(retained_post_count, 1);

    let audit_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from post_audit_events
        where post_id = $1
          and actor_id = $2
          and action = 'POST_PURGED'
          and outcome = 'SUCCESS'
          and request_id = 'req-post-retention-purge-1'
          and ip_address = '203.0.113.51'
          and user_agent = 'post-retention-purge-test'
        "#,
    )
    .bind(expired_id)
    .bind(operator_id)
    .fetch_one(&app.pool)
    .await
    .expect("purge audit count");
    assert_eq!(audit_count, 1);
}

#[tokio::test]
async fn dry_run_post_retention_purge_counts_expired_rows_without_deleting_and_records_audit() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let author_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("uuid");
    let operator_id = Uuid::parse_str("99999999-9999-9999-9999-999999999999").expect("uuid");

    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: author_id.to_string(),
            content: "Dry-run retention post".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");
    let post_id = Uuid::parse_str(&post.id).expect("post id");
    app.state
        .delete_post
        .execute(DeletePostDto {
            post_id: post.id.clone(),
            actor_id: author_id.to_string(),
            request_id: Some("req-delete-post-dry-run".into()),
            ip_address: Some("203.0.113.52".into()),
            user_agent: Some("post-retention-dry-run-test".into()),
        })
        .await
        .expect("delete post");
    sqlx::query(
        r#"
        update posts
        set retention_until = now() - interval '1 day'
        where id = $1
        "#,
    )
    .bind(post_id)
    .execute(&app.pool)
    .await
    .expect("expire post");

    let purged = app
        .state
        .purge_expired_posts
        .execute(PurgeExpiredPostsDto {
            actor_id: operator_id.to_string(),
            request_id: Some("req-post-retention-dry-run-1".into()),
            ip_address: Some("203.0.113.53".into()),
            user_agent: Some("post-retention-dry-run-test".into()),
            dry_run: true,
        })
        .await
        .expect("dry run purge posts");
    assert_eq!(purged.purged_count, 1);

    let post_count: i64 = sqlx::query_scalar("select count(*) from posts where id = $1")
        .bind(post_id)
        .fetch_one(&app.pool)
        .await
        .expect("post count");
    assert_eq!(post_count, 1);

    let audit_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from post_audit_events
        where post_id = $1
          and actor_id = $2
          and action = 'POST_PURGED'
          and outcome = 'DRY_RUN'
          and request_id = 'req-post-retention-dry-run-1'
          and ip_address = '203.0.113.53'
          and user_agent = 'post-retention-dry-run-test'
        "#,
    )
    .bind(post_id)
    .bind(operator_id)
    .fetch_one(&app.pool)
    .await
    .expect("dry-run audit count");
    assert_eq!(audit_count, 1);
}

#[tokio::test]
async fn retention_purge_worker_hard_deletes_expired_posts_and_records_metrics() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let author_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("uuid");
    let operator_id = Uuid::parse_str("99999999-9999-9999-9999-999999999999").expect("uuid");

    let expired = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: author_id.to_string(),
            content: "Worker retention post".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");
    let expired_id = Uuid::parse_str(&expired.id).expect("expired id");
    app.state
        .delete_post
        .execute(DeletePostDto {
            post_id: expired.id.clone(),
            actor_id: author_id.to_string(),
            request_id: Some("req-worker-retention-delete".into()),
            ip_address: Some("203.0.113.70".into()),
            user_agent: Some("post-retention-worker-test".into()),
        })
        .await
        .expect("delete post");
    sqlx::query(
        r#"
        update posts
        set retention_until = now() - interval '1 day'
        where id = $1
        "#,
    )
    .bind(expired_id)
    .execute(&app.pool)
    .await
    .expect("expire post");

    let purged = app
        .state
        .retention_purge_worker
        .tick()
        .await
        .expect("retention worker tick");

    assert_eq!(purged, 1);
    let post_count: i64 = sqlx::query_scalar("select count(*) from posts where id = $1")
        .bind(expired_id)
        .fetch_one(&app.pool)
        .await
        .expect("post count");
    assert_eq!(post_count, 0);
    let audit_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from post_audit_events
        where post_id = $1
          and actor_id = $2
          and action = 'POST_PURGED'
          and outcome = 'SUCCESS'
          and request_id = 'post-retention-worker'
          and user_agent = 'post-retention-worker'
        "#,
    )
    .bind(expired_id)
    .bind(operator_id)
    .fetch_one(&app.pool)
    .await
    .expect("audit count");
    assert_eq!(audit_count, 1);
    let metrics = app.metrics().await.expect("metrics");
    assert!(metrics.contains("post_retention_purge_runs_total{outcome=\"success\"} 1"));
    assert!(metrics.contains("post_retention_purged_posts_total{outcome=\"success\"} 1"));
}

#[tokio::test]
async fn retention_purge_worker_hard_deletes_expired_comments_and_records_metrics() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let author_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("uuid");
    let commenter_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid");
    let operator_id = Uuid::parse_str("99999999-9999-9999-9999-999999999999").expect("uuid");

    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: author_id.to_string(),
            content: "Comment retention parent".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");
    let post_id = Uuid::parse_str(&post.id).expect("post id");
    let comment = app
        .state
        .create_comment
        .execute(CreateCommentDto {
            post_id: post.id.clone(),
            author_id: commenter_id.to_string(),
            content: "Expired retention comment".into(),
        })
        .await
        .expect("comment");
    let comment_id = Uuid::parse_str(&comment.id).expect("comment id");
    app.state
        .delete_comment
        .execute(DeleteCommentDto {
            comment_id: comment.id.clone(),
            actor_id: commenter_id.to_string(),
            request_id: Some("req-worker-comment-delete".into()),
            ip_address: Some("203.0.113.81".into()),
            user_agent: Some("comment-retention-worker-test".into()),
        })
        .await
        .expect("delete comment");
    sqlx::query(
        r#"
        update comments
        set retention_until = now() - interval '1 day'
        where id = $1
        "#,
    )
    .bind(comment_id)
    .execute(&app.pool)
    .await
    .expect("expire comment");

    let purged = app
        .state
        .retention_purge_worker
        .tick()
        .await
        .expect("retention worker tick");

    assert_eq!(purged, 1);
    let post_count: i64 = sqlx::query_scalar("select count(*) from posts where id = $1")
        .bind(post_id)
        .fetch_one(&app.pool)
        .await
        .expect("post count");
    assert_eq!(post_count, 1);
    let comment_count: i64 = sqlx::query_scalar("select count(*) from comments where id = $1")
        .bind(comment_id)
        .fetch_one(&app.pool)
        .await
        .expect("comment count");
    assert_eq!(comment_count, 0);
    let audit_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from post_audit_events
        where post_id = $1
          and comment_id = $2
          and actor_id = $3
          and action = 'COMMENT_PURGED'
          and outcome = 'SUCCESS'
          and request_id = 'post-retention-worker'
          and user_agent = 'post-retention-worker'
        "#,
    )
    .bind(post_id)
    .bind(comment_id)
    .bind(operator_id)
    .fetch_one(&app.pool)
    .await
    .expect("audit count");
    assert_eq!(audit_count, 1);
    let metrics = app.metrics().await.expect("metrics");
    assert!(metrics.contains("post_retention_purge_runs_total{outcome=\"success\"} 1"));
    assert!(metrics.contains("post_retention_purged_comments_total{outcome=\"success\"} 1"));
}

/// A hidden prefix must not end the scan.
///
/// Search decides visibility in the application, so a database batch is not a
/// response page: the first batch can be mostly (or entirely) filtered out while
/// more matching rows remain behind it. Returning that short page with a cursor
/// is the shape that loses rows, because a client that paginates until an empty
/// page never learns the results continued.
///
/// The setup puts admitted rows *behind* a long run of hidden ones, so a
/// single-batch implementation answers short and the fixed one does not.
#[tokio::test]
async fn search_keeps_scanning_past_hidden_rows_so_a_page_is_full_or_final() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    // The viewer, and an author whose posts the viewer cannot see.
    let viewer_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid");
    let stranger_id = Uuid::parse_str("99999999-9999-9999-9999-999999999998").expect("uuid");

    // The ordering key is (created_at desc, id desc), so the hidden rows are
    // given later timestamps than the visible ones. The first batch therefore
    // contains only rows the viewer must not see.
    let base = Utc::now() - chrono::Duration::hours(1);
    let mut hidden_ids = Vec::new();
    for offset in 0..10 {
        let post_id = Uuid::new_v4();
        sqlx::query(
            "insert into posts (id, author_id, content, visibility, created_at, comment_count, like_count) values ($1, $2, $3, 'PRIVATE', $4, 0, 0)",
        )
        .bind(post_id)
        .bind(stranger_id)
        .bind("pagination probe")
        .bind(base + chrono::Duration::seconds(offset))
        .execute(&app.pool)
        .await
        .expect("hidden post");
        hidden_ids.push(post_id);
    }
    let mut visible_ids = Vec::new();
    for offset in 0..3 {
        let post_id = Uuid::new_v4();
        sqlx::query(
            "insert into posts (id, author_id, content, visibility, created_at, comment_count, like_count) values ($1, $2, $3, 'PUBLIC', $4, 0, 0)",
        )
        .bind(post_id)
        .bind(stranger_id)
        .bind("pagination probe")
        .bind(base - chrono::Duration::seconds(offset + 1))
        .execute(&app.pool)
        .await
        .expect("visible post");
        visible_ids.push(post_id);
    }

    // A page of two, with ten unreadable rows ahead of the readable ones. One
    // batch of two can only return the two newest hidden rows, so a
    // single-batch implementation answers with nothing visible.
    let page = app
        .state
        .search_posts
        .execute(viewer_id, "pagination probe", None, 2)
        .await
        .expect("search");

    let returned: Vec<Uuid> = page
        .posts
        .iter()
        .map(|post| Uuid::parse_str(&post.id).expect("uuid"))
        .collect();
    assert!(
        returned.iter().all(|id| visible_ids.contains(id)),
        "every returned post must be one the viewer may see: {returned:?}"
    );
    assert_eq!(
        returned.len(),
        2,
        "the scan must continue past the hidden rows and fill the page: {returned:?}"
    );
    assert!(
        returned.iter().all(|id| !hidden_ids.contains(id)),
        "a hidden post must never be returned: {returned:?}"
    );
    assert!(
        page.next_cursor.is_some(),
        "a full page must still offer a continuation"
    );

    // The page is full and the scan stopped there, so the continuation it offers is
    // the last returned row. Resuming from it must reach the remaining readable post
    // rather than repeat or skip one.
    let continuation = page.next_cursor.clone().expect("cursor");
    let next_page = app
        .state
        .search_posts
        .execute(viewer_id, "pagination probe", Some(&continuation), 2)
        .await
        .expect("second page");
    let resumed: Vec<Uuid> = next_page
        .posts
        .iter()
        .map(|post| Uuid::parse_str(&post.id).expect("uuid"))
        .collect();
    assert!(
        !resumed.is_empty(),
        "resuming from the continuation must reach the remaining readable posts"
    );
    assert!(
        resumed.iter().all(|id| visible_ids.contains(id)),
        "the second page must only contain readable posts: {resumed:?}"
    );
    assert!(
        resumed.iter().all(|id| !returned.contains(id)),
        "the second page must not repeat the first: {resumed:?}"
    );
}

/// T01.1 ownership: DeletePost requires the post's author and DeleteComment the
/// comment's author (the post's author is not enough). A denied attempt records
/// a FORBIDDEN audit and leaves the row untouched.
#[tokio::test]
async fn only_the_author_can_delete_a_post_or_comment() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let author_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("uuid");
    let commenter_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid");
    let intruder_id = Uuid::parse_str("33333333-3333-3333-3333-333333333333").expect("uuid");
    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: author_id.to_string(),
            content: "Ownership probe".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");
    let post_id = Uuid::parse_str(&post.id).expect("post id");
    let comment = app
        .state
        .create_comment
        .execute(CreateCommentDto {
            post_id: post.id.clone(),
            author_id: commenter_id.to_string(),
            content: "commenter owns this".into(),
        })
        .await
        .expect("comment");
    let comment_id = Uuid::parse_str(&comment.id).expect("comment id");

    for actor in [intruder_id, commenter_id] {
        let error = app
            .state
            .delete_post
            .execute(DeletePostDto {
                post_id: post.id.clone(),
                actor_id: actor.to_string(),
                request_id: None,
                ip_address: None,
                user_agent: None,
            })
            .await
            .expect_err("only the author may delete the post");
        assert_eq!(error.to_string(), "Forbidden", "post delete by {actor}");
    }
    for actor in [intruder_id, author_id] {
        let error = app
            .state
            .delete_comment
            .execute(DeleteCommentDto {
                comment_id: comment.id.clone(),
                actor_id: actor.to_string(),
                request_id: None,
                ip_address: None,
                user_agent: None,
            })
            .await
            .expect_err("only the comment author may delete the comment");
        assert_eq!(error.to_string(), "Forbidden", "comment delete by {actor}");
    }

    let post_deleted: Option<DateTime<Utc>> =
        sqlx::query_scalar("select deleted_at from posts where id = $1")
            .bind(post_id)
            .fetch_one(&app.pool)
            .await
            .expect("post row");
    let comment_deleted: Option<DateTime<Utc>> =
        sqlx::query_scalar("select deleted_at from comments where id = $1")
            .bind(comment_id)
            .fetch_one(&app.pool)
            .await
            .expect("comment row");
    assert!(post_deleted.is_none() && comment_deleted.is_none());
    let forbidden: i64 = sqlx::query_scalar(
        "select count(*) from post_audit_events where post_id = $1 and outcome = 'FORBIDDEN' and action in ('POST_DELETED', 'COMMENT_DELETED')",
    )
    .bind(post_id)
    .fetch_one(&app.pool)
    .await
    .expect("forbidden audits");
    assert_eq!(forbidden, 4);
}

/// T01.1 ownership: reads and engagement on another user's post pass the
/// viewer policy first. A stranger cannot read, comment on, like or unlike a
/// PRIVATE post, and sees the same "Post not found" as for a missing post.
#[tokio::test]
async fn a_stranger_cannot_read_or_engage_with_a_private_post() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let author_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("uuid");
    let stranger_id = Uuid::parse_str("44444444-4444-4444-4444-444444444444").expect("uuid");
    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: author_id.to_string(),
            content: "Private ownership probe".into(),
            visibility: "PRIVATE".into(),
        })
        .await
        .expect("post");
    let post_id = Uuid::parse_str(&post.id).expect("post id");

    let read = app
        .state
        .get_post_detail
        .execute(post_id, stranger_id)
        .await
        .expect_err("stranger cannot read a private post");
    assert_eq!(read.to_string(), "Post not found");
    let comment = app
        .state
        .create_comment
        .execute(CreateCommentDto {
            post_id: post.id.clone(),
            author_id: stranger_id.to_string(),
            content: "should not land".into(),
        })
        .await
        .expect_err("stranger cannot comment on a private post");
    assert_eq!(comment.to_string(), "Post not found");
    let like = app
        .state
        .like_post
        .execute(post_id, stranger_id)
        .await
        .expect_err("stranger cannot like a private post");
    assert_eq!(like.to_string(), "Post not found");
    let unlike = app
        .state
        .unlike_post
        .execute(post_id, stranger_id)
        .await
        .expect_err("stranger cannot unlike a private post");
    assert_eq!(unlike.to_string(), "Post not found");

    let (comments, likes): (i64, i64) = sqlx::query_as(
        "select (select count(*) from comments where post_id = $1), (select count(*) from post_likes where post_id = $1)",
    )
    .bind(post_id)
    .fetch_one(&app.pool)
    .await
    .expect("engagement counts");
    assert_eq!((comments, likes), (0, 0));
    app.state
        .get_post_detail
        .execute(post_id, author_id)
        .await
        .expect("the author still reads it");
}
