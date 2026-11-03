use crate::support::start_postgres_redis_app;
use post_service::application::dto::post_dto::{
    ApplyContentModerationDto, CreateCommentDto, CreatePostDto,
};
use uuid::Uuid;

const AUTHOR_ID: &str = "11111111-1111-1111-1111-111111111111";
const COMMENTER_ID: &str = "22222222-2222-2222-2222-222222222222";
const ACTOR_ID: &str = "99999999-9999-9999-9999-999999999999";

#[tokio::test]
async fn hide_and_restore_post_and_comment_are_idempotent_and_preserve_soft_delete_state() {
    let app = start_postgres_redis_app(false).await.expect("post app");
    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: AUTHOR_ID.into(),
            content: "moderation target".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");
    let comment = app
        .state
        .create_comment
        .execute(CreateCommentDto {
            post_id: post.id.clone(),
            author_id: COMMENTER_ID.into(),
            content: "reported comment".into(),
        })
        .await
        .expect("comment");
    let viewer_id = Uuid::parse_str(AUTHOR_ID).unwrap();
    let post_id = Uuid::parse_str(&post.id).unwrap();
    let comment_id = Uuid::parse_str(&comment.id).unwrap();
    let post_report_id = Uuid::new_v4();
    let comment_report_id = Uuid::new_v4();
    let (delayed_feed, fill_barrier) = app.chronological_feed_with_write_barrier();
    let feed_read = tokio::spawn(async move { delayed_feed.execute(viewer_id, None, 10).await });
    fill_barrier.wait_until_reached().await;

    for _ in 0..2 {
        app.state
            .apply_content_moderation
            .execute(ApplyContentModerationDto {
                report_id: post_report_id.to_string(),
                target_type: "POST".into(),
                target_id: post.id.clone(),
                actor_id: ACTOR_ID.into(),
                action: "HIDE".into(),
            })
            .await
            .expect("hide post");
    }
    fill_barrier.release();
    feed_read
        .await
        .expect("delayed feed task")
        .expect("in-flight feed read can finish from its earlier snapshot");
    assert!(app
        .feed_cache_key_exists(viewer_id, None, 10)
        .await
        .expect("the raced fill leaves its candidate page cached"));

    let hidden_feed = app
        .chronological_feed
        .execute(viewer_id, None, 10)
        .await
        .expect("current moderation state filters the warm cache");
    assert!(!hidden_feed.posts.iter().any(|item| item.id == post.id));
    assert!(app
        .state
        .get_post_detail
        .execute(post_id, viewer_id)
        .await
        .is_err());
    app.state
        .apply_content_moderation
        .execute(ApplyContentModerationDto {
            report_id: post_report_id.to_string(),
            target_type: "POST".into(),
            target_id: post.id.clone(),
            actor_id: ACTOR_ID.into(),
            action: "RESTORE".into(),
        })
        .await
        .expect("restore post");
    assert!(app
        .state
        .get_post_detail
        .execute(post_id, viewer_id)
        .await
        .is_ok());
    let restored_feed = app
        .chronological_feed
        .execute(viewer_id, None, 10)
        .await
        .expect("restored post is visible through the still-cached feed candidate page");
    assert!(restored_feed
        .posts
        .iter()
        .any(|item| item.id == post.id && item.content == "moderation target"));

    for _ in 0..2 {
        app.state
            .apply_content_moderation
            .execute(ApplyContentModerationDto {
                report_id: comment_report_id.to_string(),
                target_type: "COMMENT".into(),
                target_id: comment.id.clone(),
                actor_id: ACTOR_ID.into(),
                action: "HIDE".into(),
            })
            .await
            .expect("hide comment");
    }
    let hidden_detail = app
        .state
        .get_post_detail
        .execute(post_id, viewer_id)
        .await
        .expect("hidden comment detail");
    assert!(hidden_detail.comments.is_empty());
    assert_eq!(hidden_detail.post.comment_count, 0);

    app.state
        .apply_content_moderation
        .execute(ApplyContentModerationDto {
            report_id: comment_report_id.to_string(),
            target_type: "COMMENT".into(),
            target_id: comment.id,
            actor_id: ACTOR_ID.into(),
            action: "RESTORE".into(),
        })
        .await
        .expect("restore comment");
    let restored_detail = app
        .state
        .get_post_detail
        .execute(post_id, viewer_id)
        .await
        .expect("restored detail");
    assert_eq!(restored_detail.comments.len(), 1);
    assert_eq!(restored_detail.post.comment_count, 1);

    let deleted_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("select deleted_at from comments where id = $1")
            .bind(comment_id)
            .fetch_one(&app.pool)
            .await
            .expect("comment state");
    assert!(deleted_at.is_none());
}
