use post_service::application::dto::post_dto::{CreatePostDto, ReplayRequestDto};
use post_service::domain::post::ReplayTarget;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn post_shadow_fixture_matches_create_feed_and_replay_summary() {
    let fixture: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../scripts/parity/post-service/post-shadow.json"),
        )
        .expect("fixture"),
    )
    .expect("json");

    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");

    let created = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Shadow parity post".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("create");

    let feed = app
        .state
        .get_feed
        .execute(
            Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid"),
            None,
            10,
        )
        .await
        .expect("feed");

    let replay = app
        .state
        .replay_events
        .execute(ReplayRequestDto {
            actor_id: Some("22222222-2222-2222-2222-222222222222".into()),
            event_type: Some("POST_CREATED".into()),
            attribute: None,
            from: None,
            to: None,
            published: Some(false),
            target: Some(ReplayTarget::External),
            limit: Some(10),
        })
        .await
        .expect("replay");

    let actual = json!({
        "createPost": {
            "status": 200,
            "body": {
                "visibility": created.visibility
            }
        },
        "getFeed": {
            "status": 200,
            "body": {
                "postCount": feed.posts.len()
            }
        },
        "replay": {
            "status": 200,
            "body": {
                "replayedCount": replay.replayed_count
            }
        }
    });

    assert_eq!(
        actual["createPost"]["status"],
        fixture["createPost"]["status"]
    );
    assert_eq!(
        actual["createPost"]["body"]["visibility"],
        fixture["createPost"]["body"]["visibility"]
    );
    assert_eq!(actual["getFeed"], fixture["getFeed"]);
    assert_eq!(actual["replay"], fixture["replay"]);
}
