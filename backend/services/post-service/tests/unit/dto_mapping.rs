use crate::support::{comment, instant, post, uuid};
use post_service::application::dto::post_dto::{
    FeedDto, PostDetailDto, PostDto, PostLikeDto, ReplayRequestDto,
};
use post_service::domain::feed::{FeedView, PostView};
use post_service::domain::post::{PostDetail, PostLikeStatus, ReplayTarget};

#[test]
fn post_dto_is_distinct_from_domain_module() {
    let dto = PostDto {
        id: "post-1".into(),
        author_id: "user-1".into(),
        content: "hello".into(),
        visibility: "PUBLIC".into(),
        created_at: "2026-01-20T09:10:00Z".into(),
        comment_count: 0,
        like_count: 0,
        liked_by_me: false,
    };
    assert_eq!(dto.author_id, "user-1");
}

#[test]
fn feed_view_conversion_preserves_viewer_specific_like_state() {
    let post_id = uuid("11111111-1111-1111-1111-111111111121");
    let author_id = uuid("11111111-1111-1111-1111-111111111122");
    let feed = FeedView {
        posts: vec![PostView {
            id: post_id,
            author_id,
            content: "visible".to_owned(),
            visibility: "FRIENDS".to_owned(),
            created_at: instant(1_700_000_000),
            comment_count: 5,
            like_count: 8,
            liked_by_viewer: true,
        }],
        next_cursor: Some("next-page".to_owned()),
    };

    let dto = FeedDto::from(feed);

    assert_eq!(dto.next_cursor.as_deref(), Some("next-page"));
    assert_eq!(dto.posts[0].id, post_id.to_string());
    assert_eq!(dto.posts[0].author_id, author_id.to_string());
    assert_eq!(dto.posts[0].visibility, "FRIENDS");
    assert_eq!(dto.posts[0].comment_count, 5);
    assert_eq!(dto.posts[0].like_count, 8);
    assert!(dto.posts[0].liked_by_me);
}

#[test]
fn post_detail_conversion_maps_nested_comments_and_defaults_domain_post_like_state() {
    let post_id = uuid("11111111-1111-1111-1111-111111111123");
    let author_id = uuid("11111111-1111-1111-1111-111111111124");
    let comment_id = uuid("11111111-1111-1111-1111-111111111125");
    let detail = PostDetail {
        post: post(post_id, author_id, "detail"),
        comments: vec![comment(comment_id, post_id, author_id, "nested")],
    };

    let dto = PostDetailDto::from(detail);

    assert_eq!(dto.post.id, post_id.to_string());
    assert!(!dto.post.liked_by_me);
    assert_eq!(dto.comments.len(), 1);
    assert_eq!(dto.comments[0].id, comment_id.to_string());
    assert_eq!(dto.comments[0].content, "nested");
}

#[test]
fn replay_request_json_uses_camel_case_fields_and_screaming_snake_target() {
    let payload = serde_json::json!({
        "actorId": "11111111-1111-1111-1111-111111111126",
        "eventType": "POST_CREATED",
        "attribute": "PUBLIC",
        "published": false,
        "target": "BOTH",
        "limit": 25
    });

    let request: ReplayRequestDto =
        serde_json::from_value(payload).expect("deserialize replay request");
    let encoded = serde_json::to_value(&request).expect("serialize replay request");

    assert_eq!(request.target, Some(ReplayTarget::Both));
    assert_eq!(encoded["eventType"], "POST_CREATED");
    assert_eq!(encoded["target"], "BOTH");
}

#[test]
fn post_like_status_conversion_exposes_observable_like_state() {
    let post_id = uuid("11111111-1111-1111-1111-111111111127");

    let dto = PostLikeDto::from(PostLikeStatus {
        post_id,
        like_count: 9,
        liked: false,
    });

    assert_eq!(dto.post_id, post_id.to_string());
    assert_eq!(dto.like_count, 9);
    assert!(!dto.liked);
}
