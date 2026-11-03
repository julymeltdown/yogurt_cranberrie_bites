#[test]
fn profile_dto_is_distinct_from_domain_module() {
    let dto = profile_service::application::dto::profile_dto::ProfileDto {
        user_id: "user-1".into(),
        username: "user_1".into(),
        nickname: "User One".into(),
        avatar_url: "/avatars/user.svg".into(),
        bio: "bio".into(),
        follower_count: 1,
        post_count: 2,
        following_count: 3,
        account_visibility: "PUBLIC".into(),
        lifecycle_status: "ACTIVE".into(),
    };
    assert_eq!(dto.user_id, "user-1");
    assert_eq!(dto.username, "user_1");
}

#[test]
fn profile_dto_from_domain_preserves_profile_fields() {
    let user_id =
        uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("valid uuid");
    let profile = profile_service::domain::profile::Profile {
        user_id,
        username: "edge_user".into(),
        nickname: "Edge User".into(),
        avatar_url: "/avatars/edge.png".into(),
        bio: "keeps every field".into(),
        follower_count: -1,
        post_count: 0,
        following_count: 42,
        account_visibility: profile_service::domain::profile::AccountVisibility::Private,
        lifecycle_status: "ACTIVE".into(),
    };

    let dto = profile_service::application::dto::profile_dto::ProfileDto::from(profile);

    assert_eq!(dto.user_id, user_id.to_string());
    assert_eq!(dto.username, "edge_user");
    assert_eq!(dto.nickname, "Edge User");
    assert_eq!(dto.avatar_url, "/avatars/edge.png");
    assert_eq!(dto.bio, "keeps every field");
    assert_eq!(dto.follower_count, -1);
    assert_eq!(dto.post_count, 0);
    assert_eq!(dto.following_count, 42);
    assert_eq!(dto.account_visibility, "PRIVATE");
    assert_eq!(dto.lifecycle_status, "ACTIVE");
}

#[test]
fn avatar_dto_from_domain_preserves_binary_metadata() {
    let avatar = profile_service::domain::avatar::Avatar::from_processed(
        crate::support::webp_fixture(),
        " IMAGE/WEBP ",
        " avatar.webp ",
    )
    .expect("avatar");

    let dto = profile_service::application::dto::profile_dto::AvatarDto::from(avatar);

    assert_eq!(dto.content, crate::support::webp_fixture());
    assert_eq!(dto.content_type, "image/webp");
    assert_eq!(dto.filename, "avatar.webp");
}

#[test]
fn delete_profile_dto_uses_camel_case_boundary_fields() {
    let dto = profile_service::application::dto::profile_dto::DeleteProfileDto {
        user_id: "user-id".into(),
        actor_id: "actor-id".into(),
        request_id: Some("req-1".into()),
        ip_address: Some("127.0.0.1".into()),
        user_agent: Some("unit-test".into()),
    };

    let value = serde_json::to_value(dto).expect("serialize dto");

    assert_eq!(value["userId"], "user-id");
    assert_eq!(value["actorId"], "actor-id");
    assert_eq!(value["requestId"], "req-1");
    assert_eq!(value["ipAddress"], "127.0.0.1");
    assert_eq!(value["userAgent"], "unit-test");
}
