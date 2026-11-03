use platform::grpc_auth::{VerifiedCaller, Workload};
use profile_service::adapters::r#in::grpc::server::proto;
use tonic::{Code, Request};

/// Build a request carrying the verified identity the interceptor attaches.
///
/// These tests call the generated trait method directly, which bypasses the
/// server interceptor. Attaching the extension keeps the authorization path
/// under test instead of skipping it.
fn as_caller<T>(message: T, caller: Workload) -> Request<T> {
    let mut request = Request::new(message);
    request.extensions_mut().insert(VerifiedCaller(caller));
    request
}

#[tokio::test]
async fn grpc_profile_contract_covers_profile_lookup_and_avatar_flow() {
    let app = crate::support::start_postgres_app(true).await.expect("app");
    let grpc = crate::support::grpc_server(app.state.clone());

    let seeded = proto::profile_service_server::ProfileService::get_profile(
        &grpc,
        Request::new(proto::GetProfileRequest {
            user_id: "11111111-1111-1111-1111-111111111111".into(),
        }),
    )
    .await
    .expect("get profile")
    .into_inner();
    assert_eq!(seeded.profile.expect("profile").username, "hana_ani");

    let by_username = proto::profile_service_server::ProfileService::get_profile_by_username(
        &grpc,
        Request::new(proto::GetProfileByUsernameRequest {
            username: "hana_ani".into(),
        }),
    )
    .await
    .expect("get by username")
    .into_inner();
    assert_eq!(
        by_username.profile.expect("profile").nickname,
        "Hana Akiyama"
    );

    let deleted = proto::profile_service_server::ProfileService::delete_profile(
        &grpc,
        as_caller(
            proto::DeleteProfileRequest {
                user_id: "33333333-3333-3333-3333-333333333333".into(),
                actor_id: "33333333-3333-3333-3333-333333333333".into(),
                request_id: "grpc-profile-batch-delete-1".into(),
                ip_address: "198.51.100.31".into(),
                user_agent: "profile-grpc-contract".into(),
            },
            Workload::Gateway,
        ),
    )
    .await
    .expect("delete batch omission profile")
    .into_inner();
    assert!(deleted.deleted);
    assert!(!deleted.account_deleted);

    let hidden_lookup = proto::profile_service_server::ProfileService::get_profile(
        &grpc,
        Request::new(proto::GetProfileRequest {
            user_id: "33333333-3333-3333-3333-333333333333".into(),
        }),
    )
    .await
    .expect_err("soft-deleted profile must not be returned");
    assert_eq!(hidden_lookup.code(), Code::NotFound);

    let batch = proto::profile_service_server::ProfileService::get_profiles(
        &grpc,
        Request::new(proto::GetProfilesRequest {
            user_ids: vec![
                "22222222-2222-2222-2222-222222222222".into(),
                "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb".into(),
                "11111111-1111-1111-1111-111111111111".into(),
                "22222222-2222-2222-2222-222222222222".into(),
            ],
        }),
    )
    .await
    .expect("batch")
    .into_inner();
    assert_eq!(batch.profiles.len(), 2);
    assert_eq!(
        batch
            .profiles
            .iter()
            .map(|profile| profile.user_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "22222222-2222-2222-2222-222222222222",
            "11111111-1111-1111-1111-111111111111",
        ]
    );

    let deleted_batch = proto::profile_service_server::ProfileService::get_profiles(
        &grpc,
        Request::new(proto::GetProfilesRequest {
            user_ids: vec!["33333333-3333-3333-3333-333333333333".into()],
        }),
    )
    .await
    .expect("soft-deleted batch")
    .into_inner();
    assert!(deleted_batch.profiles.is_empty());

    let updated = proto::profile_service_server::ProfileService::update_profile(
        &grpc,
        as_caller(
            proto::UpdateProfileRequest {
                user_id: "11111111-1111-1111-1111-111111111111".into(),
                nickname: "Hana Rust".into(),
                avatar_url: "/avatars/new.svg".into(),
                bio: "Rust parity profile".into(),
                username: "hana_rust".into(),
            },
            Workload::Gateway,
        ),
    )
    .await
    .expect("update")
    .into_inner();
    assert_eq!(updated.profile.expect("profile").username, "hana_rust");

    let search = proto::profile_service_server::ProfileService::search_profiles(
        &grpc,
        Request::new(proto::SearchProfilesRequest {
            query: "rust".into(),
            cursor: String::new(),
            limit: 10,
        }),
    )
    .await
    .expect("search")
    .into_inner();
    assert_eq!(search.profiles.len(), 1);

    let upload = proto::profile_service_server::ProfileService::upload_avatar(
        &grpc,
        Request::new(proto::UploadAvatarRequest {
            user_id: "11111111-1111-1111-1111-111111111111".into(),
            content: crate::support::png_fixture(),
            content_type: "image/png".into(),
            filename: "avatar.png".into(),
        }),
    )
    .await
    .expect("upload")
    .into_inner();
    assert_eq!(
        upload.avatar_path,
        "/api/profile/avatar/11111111-1111-1111-1111-111111111111"
    );

    let avatar = proto::profile_service_server::ProfileService::get_avatar(
        &grpc,
        Request::new(proto::GetAvatarRequest {
            user_id: "11111111-1111-1111-1111-111111111111".into(),
        }),
    )
    .await
    .expect("avatar")
    .into_inner();
    assert_eq!(avatar.content_type, "image/png");
    assert_eq!(avatar.content, crate::support::png_fixture());

    let deleted = proto::profile_service_server::ProfileService::delete_profile(
        &grpc,
        as_caller(
            proto::DeleteProfileRequest {
                user_id: "11111111-1111-1111-1111-111111111111".into(),
                actor_id: "11111111-1111-1111-1111-111111111111".into(),
                request_id: "grpc-profile-delete-1".into(),
                ip_address: "198.51.100.31".into(),
                user_agent: "profile-grpc-contract".into(),
            },
            Workload::Gateway,
        ),
    )
    .await
    .expect("delete profile")
    .into_inner();
    assert!(deleted.deleted);

    let deleted_username = proto::profile_service_server::ProfileService::get_profile_by_username(
        &grpc,
        Request::new(proto::GetProfileByUsernameRequest {
            username: "hana_rust".into(),
        }),
    )
    .await
    .expect_err("deleted profile should be hidden by username");
    assert_eq!(deleted_username.code(), Code::NotFound);
}
