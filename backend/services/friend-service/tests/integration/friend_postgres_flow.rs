use crate::support::ProfilePrivacyResponse;

#[tokio::test]
async fn postgres_backed_friend_flow_supports_follow_lists_counts_and_unfollow() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");

    let first = app
        .state
        .follow
        .execute(
            "11111111-1111-1111-1111-111111111111"
                .parse()
                .expect("uuid"),
            "22222222-2222-2222-2222-222222222222"
                .parse()
                .expect("uuid"),
        )
        .await
        .expect("follow");
    assert!(first.following);
    assert_eq!(first.status, "ACTIVE");

    let second = app
        .state
        .follow
        .execute(
            "11111111-1111-1111-1111-111111111111"
                .parse()
                .expect("uuid"),
            "33333333-3333-3333-3333-333333333333"
                .parse()
                .expect("uuid"),
        )
        .await
        .expect("follow");
    assert!(second.following);

    let following = app
        .state
        .list_following
        .execute(
            "11111111-1111-1111-1111-111111111111"
                .parse()
                .expect("uuid"),
            None,
            10,
        )
        .await
        .expect("following");
    assert_eq!(following.total_count, 2);

    let follower_id = "11111111-1111-1111-1111-111111111111"
        .parse::<uuid::Uuid>()
        .expect("uuid");
    let followee_id = "22222222-2222-2222-2222-222222222222"
        .parse::<uuid::Uuid>()
        .expect("uuid");
    let follower_counts = app
        .state
        .get_counts
        .execute(follower_id)
        .await
        .expect("follower counts");
    assert_eq!(follower_counts.following_count, 2);

    // Mute changes what the following list shows to this viewer; it does not
    // remove the active graph edge from the profile's viewer-independent count.
    app.state
        .mute_user
        .execute(follower_id, followee_id)
        .await
        .expect("mute existing followee");
    let counts_after_mute = app
        .state
        .get_counts
        .execute(follower_id)
        .await
        .expect("counts after mute");
    assert_eq!(counts_after_mute.following_count, 2);
    let following_after_mute = app
        .state
        .list_following
        .execute(follower_id, None, 10)
        .await
        .expect("visible following after mute");
    assert_eq!(following_after_mute.total_count, 1);

    let counts = app
        .state
        .get_counts
        .execute(
            "22222222-2222-2222-2222-222222222222"
                .parse()
                .expect("uuid"),
        )
        .await
        .expect("counts");
    assert_eq!(counts.follower_count, 1);

    let stats = app
        .state
        .get_stats_batch
        .execute(&[
            "11111111-1111-1111-1111-111111111111"
                .parse()
                .expect("uuid"),
            "22222222-2222-2222-2222-222222222222"
                .parse()
                .expect("uuid"),
        ])
        .await
        .expect("stats");
    assert_eq!(stats.len(), 2);

    let following_status = app
        .state
        .check_follow
        .execute(
            "11111111-1111-1111-1111-111111111111"
                .parse()
                .expect("uuid"),
            "22222222-2222-2222-2222-222222222222"
                .parse()
                .expect("uuid"),
        )
        .await
        .expect("status");
    assert!(following_status);

    let unfollowed = app
        .state
        .unfollow
        .execute(
            "11111111-1111-1111-1111-111111111111"
                .parse()
                .expect("uuid"),
            "22222222-2222-2222-2222-222222222222"
                .parse()
                .expect("uuid"),
        )
        .await
        .expect("unfollow");
    assert!(!unfollowed.following);
    let counts_after_unfollow = app
        .state
        .get_counts
        .execute(follower_id)
        .await
        .expect("counts after unfollow");
    assert_eq!(counts_after_unfollow.following_count, 1);
    let followee_counts_after_unfollow = app
        .state
        .get_counts
        .execute(followee_id)
        .await
        .expect("followee counts after unfollow");
    assert_eq!(followee_counts_after_unfollow.follower_count, 0);

    let tombstone: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "select deleted_at from follows where follower_id = $1 and followee_id = $2",
    )
    .bind(
        "11111111-1111-1111-1111-111111111111"
            .parse::<uuid::Uuid>()
            .expect("uuid"),
    )
    .bind(
        "22222222-2222-2222-2222-222222222222"
            .parse::<uuid::Uuid>()
            .expect("uuid"),
    )
    .fetch_one(&app.pool)
    .await
    .expect("soft deleted row");
    assert!(tombstone.is_some());

    let following_after_unfollow = app
        .state
        .list_following
        .execute(
            "11111111-1111-1111-1111-111111111111"
                .parse()
                .expect("uuid"),
            None,
            10,
        )
        .await
        .expect("following");
    assert_eq!(following_after_unfollow.total_count, 1);

    let following_status_after_unfollow = app
        .state
        .check_follow
        .execute(
            "11111111-1111-1111-1111-111111111111"
                .parse()
                .expect("uuid"),
            "22222222-2222-2222-2222-222222222222"
                .parse()
                .expect("uuid"),
        )
        .await
        .expect("status");
    assert!(!following_status_after_unfollow);

    let refollowed = app
        .state
        .follow
        .execute(
            "11111111-1111-1111-1111-111111111111"
                .parse()
                .expect("uuid"),
            "22222222-2222-2222-2222-222222222222"
                .parse()
                .expect("uuid"),
        )
        .await
        .expect("refollow");
    assert!(refollowed.following);

    let active_duplicate_count: i64 = sqlx::query_scalar(
        "select count(*) from follows where follower_id = $1 and followee_id = $2 and deleted_at is null",
    )
    .bind(
        "11111111-1111-1111-1111-111111111111"
            .parse::<uuid::Uuid>()
            .expect("uuid"),
    )
    .bind(
        "22222222-2222-2222-2222-222222222222"
            .parse::<uuid::Uuid>()
            .expect("uuid"),
    )
    .fetch_one(&app.pool)
    .await
    .expect("active row count");
    assert_eq!(active_duplicate_count, 1);

    let block = app
        .state
        .block_user
        .execute(followee_id, follower_id)
        .await
        .expect("block removes the relationship edge");
    assert!(block.blocked_by_viewer);
    let follower_counts_after_block = app
        .state
        .get_counts
        .execute(follower_id)
        .await
        .expect("counts after block");
    let followee_counts_after_block = app
        .state
        .get_counts
        .execute(followee_id)
        .await
        .expect("counts after block");
    assert_eq!(follower_counts_after_block.following_count, 1);
    assert_eq!(followee_counts_after_block.follower_count, 0);
    app.shutdown().await.expect("fixture shutdown");
}

#[tokio::test]
async fn postgres_backed_friend_flow_creates_pending_request_for_private_profiles() {
    let app =
        crate::support::start_postgres_app_with_privacy(false, ProfilePrivacyResponse::Private)
            .await
            .expect("app");
    let follower_id = "11111111-1111-1111-1111-111111111111"
        .parse::<uuid::Uuid>()
        .expect("uuid");
    let followee_id = "22222222-2222-2222-2222-222222222222"
        .parse::<uuid::Uuid>()
        .expect("uuid");

    let follow = app
        .state
        .follow
        .execute(follower_id, followee_id)
        .await
        .expect("pending follow request");

    assert!(!follow.following);
    assert_eq!(follow.status, "PENDING");
    assert!(follow.request_id.is_some());

    let pending_follower_counts = app
        .state
        .get_counts
        .execute(follower_id)
        .await
        .expect("pending follower counts");
    let pending_followee_counts = app
        .state
        .get_counts
        .execute(followee_id)
        .await
        .expect("pending followee counts");
    assert_eq!(pending_follower_counts.following_count, 0);
    assert_eq!(pending_followee_counts.follower_count, 0);

    let follow_edges: i64 = sqlx::query_scalar(
        "select count(*) from follows where follower_id = $1 and followee_id = $2 and deleted_at is null",
    )
    .bind(follower_id)
    .bind(followee_id)
    .fetch_one(&app.pool)
    .await
    .expect("follow edge count");
    assert_eq!(follow_edges, 0);

    let accepted = app
        .state
        .accept_follow_request
        .execute(
            follow
                .request_id
                .expect("pending request ID")
                .parse()
                .expect("request UUID"),
            followee_id,
        )
        .await
        .expect("accept private follow request");
    assert!(accepted.following);
    let accepted_follower_counts = app
        .state
        .get_counts
        .execute(follower_id)
        .await
        .expect("accepted follower counts");
    let accepted_followee_counts = app
        .state
        .get_counts
        .execute(followee_id)
        .await
        .expect("accepted followee counts");
    assert_eq!(accepted_follower_counts.following_count, 1);
    assert_eq!(accepted_followee_counts.follower_count, 1);
    app.shutdown().await.expect("fixture shutdown");
}

#[tokio::test]
async fn postgres_backed_friend_flow_rejects_unavailable_privacy_without_follow_edge() {
    let app =
        crate::support::start_postgres_app_with_privacy(false, ProfilePrivacyResponse::Unavailable)
            .await
            .expect("app");
    let follower_id = "11111111-1111-1111-1111-111111111111"
        .parse::<uuid::Uuid>()
        .expect("uuid");
    let followee_id = "22222222-2222-2222-2222-222222222222"
        .parse::<uuid::Uuid>()
        .expect("uuid");

    let error = app
        .state
        .follow
        .execute(follower_id, followee_id)
        .await
        .expect_err("unavailable privacy must fail");

    assert!(error.to_string().contains("profile privacy unavailable"));

    let follow_edges: i64 = sqlx::query_scalar(
        "select count(*) from follows where follower_id = $1 and followee_id = $2 and deleted_at is null",
    )
    .bind(follower_id)
    .bind(followee_id)
    .fetch_one(&app.pool)
    .await
    .expect("follow edge count");
    assert_eq!(follow_edges, 0);
    app.shutdown().await.expect("fixture shutdown");
}
