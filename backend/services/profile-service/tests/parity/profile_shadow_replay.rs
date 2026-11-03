#[tokio::test]
async fn profile_shadow_fixture_matches_seeded_lookup() {
    let app = crate::support::start_postgres_app(true).await.expect("app");
    let fixture: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../scripts/parity/profile-service/profile-shadow.json"),
        )
        .expect("fixture"),
    )
    .expect("json");

    let target = app
        .state
        .get_profile
        .execute(
            "11111111-1111-1111-1111-111111111111"
                .parse()
                .expect("uuid"),
        )
        .await
        .expect("lookup");

    assert_eq!(
        fixture["getProfile"]["userId"].as_str().unwrap_or_default(),
        target.user_id
    );
    assert_eq!(
        fixture["getProfile"]["username"]
            .as_str()
            .unwrap_or_default(),
        target.username
    );
    assert_eq!(
        fixture["getProfile"]["nickname"]
            .as_str()
            .unwrap_or_default(),
        target.nickname
    );
}
