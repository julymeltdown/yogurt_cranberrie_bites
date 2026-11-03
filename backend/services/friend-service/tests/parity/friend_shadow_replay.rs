#[tokio::test]
async fn friend_shadow_fixture_matches_seeded_counts() {
    let app = crate::support::start_postgres_app(true).await.expect("app");
    let fixture: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../scripts/parity/friend-service/friend-shadow.json"),
        )
        .expect("fixture"),
    )
    .expect("json");

    let counts = app
        .state
        .get_counts
        .execute(
            fixture["userId"]
                .as_str()
                .expect("user id")
                .parse()
                .expect("uuid"),
        )
        .await
        .expect("counts");

    assert_eq!(
        fixture["followerCount"].as_i64().unwrap_or_default(),
        counts.follower_count
    );
    assert_eq!(
        fixture["followingCount"].as_i64().unwrap_or_default(),
        counts.following_count
    );
    app.shutdown().await.expect("fixture shutdown");
}
