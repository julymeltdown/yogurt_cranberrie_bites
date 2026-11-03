use chrono::{DateTime, Utc};
use std::fmt::{Debug, Formatter};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AccountVisibility {
    Public,
    Private,
}

impl Debug for AccountVisibility {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("AccountVisibility")
            .field(&"[REDACTED]")
            .finish()
    }
}

impl AccountVisibility {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "PUBLIC" => Ok(Self::Public),
            "PRIVATE" => Ok(Self::Private),
            _ => anyhow::bail!("Account visibility must be PUBLIC or PRIVATE"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Public => "PUBLIC",
            Self::Private => "PRIVATE",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Profile {
    pub user_id: Uuid,
    pub username: String,
    pub nickname: String,
    pub avatar_url: String,
    pub bio: String,
    pub follower_count: i64,
    pub post_count: i64,
    pub following_count: i64,
    pub account_visibility: AccountVisibility,
    pub lifecycle_status: String,
}

impl Debug for Profile {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Profile")
            .field("user_id", &"[REDACTED]")
            .field("username", &"[REDACTED]")
            .field("nickname", &"[REDACTED]")
            .field("avatar_url", &"[REDACTED]")
            .field("bio", &"[REDACTED]")
            .field("counts", &"[REDACTED]")
            .field("account_visibility", &"[REDACTED]")
            .field("lifecycle_status", &"[REDACTED]")
            .finish()
    }
}

impl Profile {
    pub fn default_for(user_id: Uuid) -> Self {
        Self {
            user_id,
            username: String::new(),
            nickname: String::new(),
            avatar_url: String::new(),
            bio: String::new(),
            follower_count: 0,
            post_count: 0,
            following_count: 0,
            account_visibility: AccountVisibility::Public,
            lifecycle_status: "ACTIVE".to_owned(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProfileAuditEvent {
    pub id: Uuid,
    pub actor_id: Option<Uuid>,
    pub profile_user_id: Option<Uuid>,
    pub action: String,
    pub outcome: String,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub metadata_json: String,
    pub occurred_at: DateTime<Utc>,
}

impl Debug for ProfileAuditEvent {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProfileAuditEvent")
            .field("id", &"[REDACTED]")
            .field("actor_id", &"[REDACTED]")
            .field("profile_user_id", &"[REDACTED]")
            .field("action", &self.action)
            .field("outcome", &self.outcome)
            .field("request_id", &"[REDACTED]")
            .field("ip_address", &"[REDACTED]")
            .field("user_agent", &"[REDACTED]")
            .field("metadata_json", &"[REDACTED]")
            .field("occurred_at", &self.occurred_at)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_visibility_debug_redacts_privacy_setting() {
        let rendered = format!("{:?}", AccountVisibility::Private);

        assert!(!rendered.contains("Private"));
        assert!(!rendered.contains("PRIVATE"));
        assert!(rendered.contains("[REDACTED]"));
    }

    #[test]
    fn profile_debug_redacts_private_profile_fields() {
        let profile = Profile {
            user_id: Uuid::parse_str("1b1b1b1b-1b1b-4b1b-8b1b-1b1b1b1b1b1b").unwrap(),
            username: "private-username-marker".into(),
            nickname: "private-nickname-marker".into(),
            avatar_url: "private-avatar-url-marker".into(),
            bio: "private-bio-marker".into(),
            follower_count: 41,
            post_count: 42,
            following_count: 43,
            account_visibility: AccountVisibility::Private,
            lifecycle_status: "DELETED_PRIVATE_MARKER".into(),
        };

        let rendered = format!("{profile:?}");
        for private_value in [
            profile.user_id.to_string(),
            profile.username,
            profile.nickname,
            profile.avatar_url,
            profile.bio,
            "41".to_owned(),
            "42".to_owned(),
            "43".to_owned(),
            "PRIVATE".to_owned(),
            "DELETED_PRIVATE_MARKER".to_owned(),
        ] {
            assert!(!rendered.contains(&private_value), "{rendered}");
        }
        assert!(rendered.contains("Profile"));
        assert!(rendered.contains("[REDACTED]"));
    }

    #[test]
    fn profile_audit_debug_redacts_identifiers_and_request_metadata() {
        let event = ProfileAuditEvent {
            id: Uuid::parse_str("2c2c2c2c-2c2c-4c2c-8c2c-2c2c2c2c2c2c").unwrap(),
            actor_id: Some(Uuid::new_v4()),
            profile_user_id: Some(Uuid::new_v4()),
            action: "UPDATE_PROFILE".into(),
            outcome: "SUCCESS".into(),
            request_id: Some("private-request-id-marker".into()),
            ip_address: Some("198.51.100.42".into()),
            user_agent: Some("private-user-agent-marker".into()),
            metadata_json: r#"{"private":"audit-metadata-marker"}"#.into(),
            occurred_at: Utc::now(),
        };

        let rendered = format!("{event:?}");
        for private_value in [
            event.id.to_string(),
            event.actor_id.unwrap().to_string(),
            event.profile_user_id.unwrap().to_string(),
            "private-request-id-marker".to_owned(),
            "198.51.100.42".to_owned(),
            "private-user-agent-marker".to_owned(),
            "audit-metadata-marker".to_owned(),
        ] {
            assert!(!rendered.contains(&private_value), "{rendered}");
        }
        assert!(rendered.contains("UPDATE_PROFILE"));
        assert!(rendered.contains("SUCCESS"));
        assert!(rendered.contains("[REDACTED]"));
    }
}
