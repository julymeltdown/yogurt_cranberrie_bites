use auth_service::application::dto::auth_dto::DeleteUserDto;
use auth_service::application::ports::{
    RefreshTokenStorePort, TokenServicePort, UserRepositoryPort,
};
use auth_service::application::use_cases::delete_user::DeleteUserUseCase;
use auth_service::domain::user::UserStatus;

#[tokio::test]
async fn delete_user_soft_deletes_records_audit_and_revokes_refresh_tokens() {
    let users = crate::support::InMemoryUserRepository::default();
    let audits = crate::support::InMemoryAuthAuditRepository::default();
    let refresh_store = crate::support::InMemoryRefreshTokenStore::default();
    let token_service = crate::support::TestTokenService::default();
    let user_id = crate::support::seed_active_user(&users, "hana@example.com")
        .await
        .expect("seed");
    let tokens = token_service
        .issue_token_pair(user_id, vec!["ROLE_USER".into()])
        .expect("tokens");
    let decoded = token_service
        .decode_refresh_token(&tokens.refresh_token)
        .expect("decode");
    refresh_store
        .store(&(&auth_service::domain::session::RefreshTokenRecord {
            jti: decoded.jti.clone(),
            token_hash: "hash".into(),
            user_id,
            family_id: uuid::Uuid::new_v4(),
            expires_at: tokens.refresh_expires_at,
        }).into())
        .await
        .expect("store");
    let use_case = DeleteUserUseCase::new(users.clone(), audits.clone(), refresh_store.clone(), std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime));

    let response = use_case
        .execute(DeleteUserDto {
            user_id: user_id.to_string(),
            actor_id: user_id.to_string(),
            request_id: Some("req-delete-user-1".into()),
            ip_address: Some("127.0.0.1".into()),
            user_agent: Some("delete-user-test".into()),
        })
        .await
        .expect("delete user");

    assert!(response.deleted);
    assert_eq!(
        users
            .find_by_id(user_id)
            .await
            .expect("find").map(auth_service::domain::user::User::from)
            .expect("user")
            .status,
        UserStatus::Deleted
    );
    assert!(refresh_store
        .find(&decoded.jti)
        .await
        .expect("find token")
        .is_none());
    let events = audits.audits.lock().await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].action, "USER_DELETED");
    assert_eq!(events[0].outcome, "SUCCESS");
    assert_eq!(events[0].request_id.as_deref(), Some("req-delete-user-1"));
    assert_eq!(events[0].ip_address.as_deref(), Some("127.0.0.1"));
    assert_eq!(events[0].user_agent.as_deref(), Some("delete-user-test"));
}

#[tokio::test]
async fn delete_user_records_forbidden_audit_for_wrong_actor() {
    let users = crate::support::InMemoryUserRepository::default();
    let audits = crate::support::InMemoryAuthAuditRepository::default();
    let refresh_store = crate::support::InMemoryRefreshTokenStore::default();
    let user_id = crate::support::seed_active_user(&users, "hana@example.com")
        .await
        .expect("seed");
    let actor_id = uuid::Uuid::new_v4();
    let use_case = DeleteUserUseCase::new(users, audits.clone(), refresh_store, std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime));

    let error = use_case
        .execute(DeleteUserDto {
            user_id: user_id.to_string(),
            actor_id: actor_id.to_string(),
            request_id: Some("req-forbidden-delete-user".into()),
            ip_address: None,
            user_agent: Some("delete-user-test".into()),
        })
        .await
        .expect_err("wrong actor should fail");

    assert_eq!(error.to_string(), "Forbidden");
    let events = audits.audits.lock().await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].outcome, "FORBIDDEN");
    assert_eq!(events[0].actor_id, Some(actor_id));
    assert_eq!(events[0].subject_user_id, Some(user_id));
}
