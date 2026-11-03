//! Application-owned repository transfer values. These are concrete DTOs,
//! independent of domain entity identity and database/framework row types.
use super::profile_dto::ProfileDto;
use chrono::{DateTime, Utc};
use profile_domain::profile::{AccountVisibility, Profile, ProfileAuditEvent};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProfileVisibilityDto {
    Public,
    Private,
}

impl ProfileVisibilityDto {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Public => "PUBLIC",
            Self::Private => "PRIVATE",
        }
    }
}

impl From<AccountVisibility> for ProfileVisibilityDto {
    fn from(value: AccountVisibility) -> Self {
        match value {
            AccountVisibility::Public => Self::Public,
            AccountVisibility::Private => Self::Private,
        }
    }
}

impl From<ProfileVisibilityDto> for AccountVisibility {
    fn from(value: ProfileVisibilityDto) -> Self {
        match value {
            ProfileVisibilityDto::Public => Self::Public,
            ProfileVisibilityDto::Private => Self::Private,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProfileRecordDto {
    pub user_id: Uuid,
    pub username: String,
    pub nickname: String,
    pub avatar_url: String,
    pub bio: String,
    pub follower_count: i64,
    pub post_count: i64,
    pub following_count: i64,
    pub account_visibility: ProfileVisibilityDto,
    pub lifecycle_status: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProfileAuditEventDto {
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

impl From<Profile> for ProfileRecordDto {
    fn from(value: Profile) -> Self {
        Self {
            user_id: value.user_id,
            username: value.username,
            nickname: value.nickname,
            avatar_url: value.avatar_url,
            bio: value.bio,
            follower_count: value.follower_count,
            post_count: value.post_count,
            following_count: value.following_count,
            account_visibility: value.account_visibility.into(),
            lifecycle_status: value.lifecycle_status,
        }
    }
}

impl From<ProfileRecordDto> for Profile {
    fn from(value: ProfileRecordDto) -> Self {
        Self {
            user_id: value.user_id,
            username: value.username,
            nickname: value.nickname,
            avatar_url: value.avatar_url,
            bio: value.bio,
            follower_count: value.follower_count,
            post_count: value.post_count,
            following_count: value.following_count,
            account_visibility: value.account_visibility.into(),
            lifecycle_status: value.lifecycle_status,
        }
    }
}

impl From<ProfileAuditEvent> for ProfileAuditEventDto {
    fn from(value: ProfileAuditEvent) -> Self {
        Self {
            id: value.id,
            actor_id: value.actor_id,
            profile_user_id: value.profile_user_id,
            action: value.action,
            outcome: value.outcome,
            request_id: value.request_id,
            ip_address: value.ip_address,
            user_agent: value.user_agent,
            metadata_json: value.metadata_json,
            occurred_at: value.occurred_at,
        }
    }
}

impl From<ProfileAuditEventDto> for ProfileAuditEvent {
    fn from(value: ProfileAuditEventDto) -> Self {
        Self {
            id: value.id,
            actor_id: value.actor_id,
            profile_user_id: value.profile_user_id,
            action: value.action,
            outcome: value.outcome,
            request_id: value.request_id,
            ip_address: value.ip_address,
            user_agent: value.user_agent,
            metadata_json: value.metadata_json,
            occurred_at: value.occurred_at,
        }
    }
}

impl From<ProfileRecordDto> for ProfileDto {
    fn from(value: ProfileRecordDto) -> Self {
        Profile::from(value).into()
    }
}

impl std::fmt::Debug for ProfileRecordDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProfileRecordDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}

impl std::fmt::Debug for ProfileVisibilityDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ProfileVisibilityDto")
            .field(&"[REDACTED]")
            .finish()
    }
}

impl std::fmt::Debug for ProfileAuditEventDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProfileAuditEventDto")
            .field("fields", &"[REDACTED]")
            .field("action", &self.action)
            .field("outcome", &self.outcome)
            .field("occurred_at", &self.occurred_at)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn repository_profile_roundtrip_preserves_domain_and_public_snapshot() {
        for visibility in [AccountVisibility::Public, AccountVisibility::Private] {
            let profile = Profile {
                user_id: Uuid::from_u128(0xabc),
                username: "private_username".into(),
                nickname: "Unicode 이름".into(),
                avatar_url: "private-avatar-url".into(),
                bio: "private-bio".into(),
                follower_count: -1,
                post_count: i64::MAX,
                following_count: 0,
                account_visibility: visibility,
                lifecycle_status: "CUSTOM_STATUS".into(),
            };
            let expected = serde_json::to_value(ProfileDto::from(profile.clone())).unwrap();
            let record = ProfileRecordDto::from(profile.clone());
            assert_eq!(Profile::from(record.clone()), profile);
            assert_eq!(
                serde_json::to_value(ProfileDto::from(record.clone())).unwrap(),
                expected
            );
            assert_eq!(record.account_visibility.as_str(), visibility.as_str());
            let rendered = format!("{record:?} {:?}", record.account_visibility);
            for secret in [
                "private_username",
                "Unicode",
                "private-avatar-url",
                "private-bio",
                "CUSTOM_STATUS",
                visibility.as_str(),
            ] {
                assert!(!rendered.contains(secret), "{rendered}");
            }
            assert!(rendered.contains("[REDACTED]"));
        }
    }

    #[test]
    fn repository_audit_roundtrip_preserves_optional_identity_and_redaction() {
        for subject in [None, Some(Uuid::from_u128(0xdef))] {
            let event = ProfileAuditEvent {
                id: Uuid::from_u128(0xabc),
                actor_id: subject,
                profile_user_id: subject,
                action: "PROFILE_DELETED".into(),
                outcome: "FORBIDDEN".into(),
                request_id: Some("private-request".into()),
                ip_address: Some("private-ip".into()),
                user_agent: Some("private-agent".into()),
                metadata_json: "private-metadata".into(),
                occurred_at: Utc.timestamp_opt(1_700_000_000, 123).unwrap(),
            };
            let dto = ProfileAuditEventDto::from(event.clone());
            assert_eq!(ProfileAuditEvent::from(dto.clone()), event);
            let rendered = format!("{dto:?}");
            for secret in [
                "private-request",
                "private-ip",
                "private-agent",
                "private-metadata",
            ] {
                assert!(!rendered.contains(secret), "{rendered}");
            }
            assert!(!rendered.contains(&dto.id.to_string()), "{rendered}");
            if let Some(subject) = subject {
                assert!(!rendered.contains(&subject.to_string()), "{rendered}");
            }
            assert!(rendered.contains("[REDACTED]"));
            assert!(rendered.contains("PROFILE_DELETED"));
            assert!(rendered.contains("FORBIDDEN"));
        }
    }
}
