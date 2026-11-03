use auth_service::adapters::r#in::grpc::server::proto;
use tonic::Request;

#[tokio::test]
async fn postgres_backed_oauth_flow_links_existing_identities_and_persists_subjects() {
    let app = crate::support::start_postgres_app(true).await.expect("app");
    let grpc = crate::support::grpc_server(app.state.clone());

    let authorize = proto::auth_service_server::AuthService::authorize_o_auth(
        &grpc,
        Request::new(proto::AuthorizeOAuthRequest {
            provider: proto::IdentityProvider::Google as i32,
        }),
    )
    .await
    .expect("authorize")
    .into_inner();
    assert!(authorize.authorization_uri.contains("state="));
    assert!(authorize.authorization_uri.contains("code_challenge="));
    assert!(authorize
        .authorization_uri
        .contains("code_challenge_method=S256"));
    assert!(authorize.authorization_uri.contains("nonce="));
    let state = extract_query_param(&authorize.authorization_uri, "state").expect("state");

    let callback = proto::auth_service_server::AuthService::o_auth_callback(
        &grpc,
        Request::new(proto::OAuthCallbackRequest {
            provider: proto::IdentityProvider::Google as i32,
            code: "google-hana-code".into(),
            state: state.clone(),
            ip_address: "127.0.0.1".into(),
            user_agent: "integration".into(),
        }),
    )
    .await
    .expect("callback")
    .into_inner();
    assert_eq!(callback.user_id, "11111111-1111-1111-1111-111111111111");
    assert!(callback
        .linked_providers
        .contains(&(proto::IdentityProvider::Google as i32)));

    let replayed_callback = proto::auth_service_server::AuthService::o_auth_callback(
        &grpc,
        Request::new(proto::OAuthCallbackRequest {
            provider: proto::IdentityProvider::Google as i32,
            code: "google-hana-code".into(),
            state,
            ip_address: "127.0.0.1".into(),
            user_agent: "integration".into(),
        }),
    )
    .await
    .expect_err("oauth state must be one-time use");
    assert!(replayed_callback
        .message()
        .contains("OAuth authorization state is invalid"));
    assert_eq!(replayed_callback.code(), tonic::Code::InvalidArgument);

    let apple_authorize = proto::auth_service_server::AuthService::authorize_o_auth(
        &grpc,
        Request::new(proto::AuthorizeOAuthRequest {
            provider: proto::IdentityProvider::Apple as i32,
        }),
    )
    .await
    .expect("apple authorize")
    .into_inner();
    let apple_state =
        extract_query_param(&apple_authorize.authorization_uri, "state").expect("apple state");

    let linked = proto::auth_service_server::AuthService::link_o_auth_provider(
        &grpc,
        Request::new(proto::LinkOAuthProviderRequest {
            provider: proto::IdentityProvider::Apple as i32,
            user_id: "11111111-1111-1111-1111-111111111111".into(),
            code: "apple-hana-code".into(),
            state: apple_state,
        }),
    )
    .await
    .expect("link")
    .into_inner();
    assert!(linked
        .providers
        .contains(&(proto::IdentityProvider::Apple as i32)));

    let google_count: (i64,) = sqlx::query_as(
        "select count(*) from user_identities where user_id = $1 and provider_type = 'GOOGLE' and provider_subject = $2",
    )
    .bind(uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid"))
    .bind("google-hana-subject")
    .fetch_one(&app.pool)
    .await
    .expect("google count");
    assert_eq!(google_count.0, 1);

    let apple_count: (i64,) = sqlx::query_as(
        "select count(*) from user_identities where user_id = $1 and provider_type = 'APPLE' and provider_subject = $2",
    )
    .bind(uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid"))
    .bind("apple-hana-subject")
    .fetch_one(&app.pool)
    .await
    .expect("apple count");
    assert_eq!(apple_count.0, 1);
}

/// An identity already owned by another user cannot be attached to a second
/// one through the real handler, and the denial happens after the exchange is
/// audited so the attempt is still recorded.
#[tokio::test]
async fn link_o_auth_provider_refuses_an_identity_owned_by_another_user() {
    let app = crate::support::start_postgres_app(true).await.expect("app");
    let grpc = crate::support::grpc_server(app.state.clone());
    // The Google fixture mailbox belongs to the first seeded user, so that user
    // can link it and the second cannot.
    let first = "11111111-1111-1111-1111-111111111111";
    let second = "22222222-2222-2222-2222-222222222222";

    // The first user links the provider account.
    let first_state = authorize_state(&grpc, proto::IdentityProvider::Google).await;
    let linked = proto::auth_service_server::AuthService::link_o_auth_provider(
        &grpc,
        Request::new(proto::LinkOAuthProviderRequest {
            provider: proto::IdentityProvider::Google as i32,
            user_id: first.into(),
            code: "google-hana-code".into(),
            state: first_state,
        }),
    )
    .await
    .expect("the first user may link the account")
    .into_inner();
    assert_eq!(linked.user_id, first);
    assert!(linked
        .providers
        .contains(&(proto::IdentityProvider::Google as i32)));

    // A second user presenting the same provider account must be refused.
    let second_state = authorize_state(&grpc, proto::IdentityProvider::Google).await;
    let denied = proto::auth_service_server::AuthService::link_o_auth_provider(
        &grpc,
        Request::new(proto::LinkOAuthProviderRequest {
            provider: proto::IdentityProvider::Google as i32,
            user_id: second.into(),
            code: "google-hana-code".into(),
            state: second_state,
        }),
    )
    .await
    .expect_err("a provider account cannot be attached to a second user");
    assert_eq!(denied.code(), tonic::Code::AlreadyExists);
    assert_eq!(denied.message(), "Provider already linked to another user");

    // The subject stays owned by exactly the first user, and the denial did
    // not create a second identity row for the second user.
    let owner: (uuid::Uuid,) =
        sqlx::query_as("select user_id from user_identities where provider_subject = $1")
            .bind("google-hana-subject")
            .fetch_one(&app.pool)
            .await
            .expect("one owning identity");
    assert_eq!(owner.0.to_string(), first);
    // Every seeded user owns its own EMAIL identity, so the check is that no
    // Google identity was created for the second user.
    let second_google: (i64,) = sqlx::query_as(
        "select count(*) from user_identities where user_id = $1 and provider_type = 'GOOGLE'",
    )
    .bind(uuid::Uuid::parse_str(second).expect("uuid"))
    .fetch_one(&app.pool)
    .await
    .expect("second user google identities");
    assert_eq!(second_google.0, 0);

    // Both exchanges were audited for the user that asked, so a denied link
    // still leaves the attempt on the record.
    let audited: (i64,) = sqlx::query_as(
        "select count(*) from auth_audit_events where actor_id = $1 and subject_user_id = $1",
    )
    .bind(uuid::Uuid::parse_str(second).expect("uuid"))
    .fetch_one(&app.pool)
    .await
    .expect("second user audits");
    assert_eq!(audited.0, 1);
}

/// Ask the handler for a fresh one-time authorization state.
async fn authorize_state(
    grpc: &auth_service::adapters::r#in::grpc::server::AuthGrpcServer,
    provider: proto::IdentityProvider,
) -> String {
    let authorize = proto::auth_service_server::AuthService::authorize_o_auth(
        grpc,
        Request::new(proto::AuthorizeOAuthRequest {
            provider: provider as i32,
        }),
    )
    .await
    .expect("authorize")
    .into_inner();
    extract_query_param(&authorize.authorization_uri, "state").expect("state")
}

fn extract_query_param(uri: &str, key: &str) -> Option<String> {
    let query = uri.split('?').nth(1)?;
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=')?;
        if name == key {
            return Some(value.to_owned());
        }
    }
    None
}
