#[test]
fn friend_dto_is_distinct_from_domain_module() {
    let dto = friend_service::application::dto::friend_dto::FollowEdgeDto {
        follower_id: "user-1".into(),
        followee_id: "user-2".into(),
        created_at: "2026-01-01T00:00:00Z".into(),
    };
    assert_eq!(dto.follower_id, "user-1");
}
