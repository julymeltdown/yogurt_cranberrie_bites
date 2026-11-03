use auth_service::application::dto::auth_dto::AuthResponseDto;
use auth_service::domain::user::IdentityProvider;
use uuid::Uuid;

#[test]
fn auth_response_uses_explicit_camel_case_dto_fields() {
    let response = AuthResponseDto {
        user_id: Uuid::nil(),
        access_token: "access".into(),
        refresh_token: "refresh".into(),
        token_type: "Bearer".into(),
        expires_in_seconds: 900,
        linked_providers: vec![IdentityProvider::Email],
    };

    let json = serde_json::to_value(response).expect("serialize");
    assert!(json.get("userId").is_some());
    assert!(json.get("accessToken").is_some());
    assert!(json.get("expiresInSeconds").is_some());
}
