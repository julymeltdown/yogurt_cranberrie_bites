use anyhow::Result;
use friend_service::application::ports::ProfilePrivacyPort;
use friend_service::domain::follow_request::AccountVisibility;
use uuid::Uuid;

#[derive(Clone, Copy, Default)]
pub struct PublicProfilePrivacy;

#[async_trait::async_trait]
impl ProfilePrivacyPort for PublicProfilePrivacy {
    async fn account_visibility(&self, _user_id: Uuid) -> Result<AccountVisibility> {
        Ok(AccountVisibility::Public)
    }
}
