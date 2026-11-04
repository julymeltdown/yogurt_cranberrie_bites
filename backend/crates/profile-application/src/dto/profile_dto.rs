use chrono::{DateTime, Utc};
use profile_domain::avatar::Avatar;
use profile_domain::profile::Profile;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! redacted_debug {
    ($($type:ident { $($field:ident),+ $(,)? }),+ $(,)?) => {
        $(
            impl std::fmt::Debug for $type {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    let mut debug = formatter.debug_struct(stringify!($type));
                    $(debug.field(stringify!($field), &"[REDACTED]");)+
                    debug.finish()
                }
            }
        )+
    };
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDto {
    pub user_id: String,
    pub username: String,
    pub nickname: String,
    pub avatar_url: String,
    pub bio: String,
    pub follower_count: i64,
    pub post_count: i64,
    pub following_count: i64,
    pub account_visibility: String,
    pub lifecycle_status: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileDto {
    pub user_id: String,
    pub username: String,
    pub nickname: String,
    pub avatar_url: String,
    pub bio: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfilePrivacyDto {
    pub user_id: String,
    pub actor_id: String,
    pub account_visibility: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteProfileDto {
    pub user_id: String,
    pub actor_id: String,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// Required audit metadata for the repository's atomic profile deletion.
/// Actor, subject, action and success outcome are bound by the deletion write.
#[derive(Clone)]
pub struct ProfileDeletionAuditDto {
    pub id: Uuid,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub metadata_json: String,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteProfileResponseDto {
    pub user_id: String,
    pub deleted: bool,
    pub account_deleted: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeExpiredProfilesDto {
    pub actor_id: String,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeExpiredProfilesResponseDto {
    pub purged_count: i64,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSearchPageDto {
    pub profiles: Vec<ProfileDto>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvatarDto {
    pub content: Vec<u8>,
    pub content_type: String,
    pub filename: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvatarUploadResponseDto {
    pub user_id: String,
}

impl From<Profile> for ProfileDto {
    fn from(value: Profile) -> Self {
        Self {
            user_id: value.user_id.to_string(),
            username: value.username,
            nickname: value.nickname,
            avatar_url: value.avatar_url,
            bio: value.bio,
            follower_count: value.follower_count,
            post_count: value.post_count,
            following_count: value.following_count,
            account_visibility: value.account_visibility.as_str().to_owned(),
            lifecycle_status: value.lifecycle_status,
        }
    }
}

impl From<Avatar> for AvatarDto {
    fn from(value: Avatar) -> Self {
        Self {
            content: value.content,
            content_type: value.content_type,
            filename: value.filename,
        }
    }
}

#[cfg(test)]
mod secret_debug_tests {
    use super::*;

    #[test]
    fn profile_and_account_dto_debug_redacts_private_values_without_changing_serde() {
        let profile = ProfileDto {
            user_id: "private-profile-user-id".into(),
            username: "private-profile-username".into(),
            nickname: "private-profile-nickname".into(),
            avatar_url: "private-profile-avatar-url".into(),
            bio: "private-profile-bio".into(),
            follower_count: 11,
            post_count: 12,
            following_count: 13,
            account_visibility: "PRIVATE".into(),
            lifecycle_status: "PRIVATE_LIFECYCLE".into(),
        };
        let update = UpdateProfileDto {
            user_id: "private-update-user-id".into(),
            username: "private-update-username".into(),
            nickname: "private-update-nickname".into(),
            avatar_url: "private-update-avatar-url".into(),
            bio: "private-update-bio".into(),
        };
        let privacy = UpdateProfilePrivacyDto {
            user_id: "private-privacy-user-id".into(),
            actor_id: "private-privacy-actor-id".into(),
            account_visibility: "PRIVATE".into(),
        };
        let deletion = DeleteProfileDto {
            user_id: "private-delete-user-id".into(),
            actor_id: "private-delete-actor-id".into(),
            request_id: Some("private-delete-request-id".into()),
            ip_address: Some("private-delete-ip".into()),
            user_agent: Some("private-delete-user-agent".into()),
        };
        let purge = PurgeExpiredProfilesDto {
            actor_id: "private-purge-actor-id".into(),
            request_id: Some("private-purge-request-id".into()),
            ip_address: Some("private-purge-ip".into()),
            user_agent: Some("private-purge-user-agent".into()),
            dry_run: true,
        };
        let delete_response = DeleteProfileResponseDto {
            user_id: "private-delete-response-user-id".into(),
            deleted: true,
            account_deleted: false,
        };
        let search = ProfileSearchPageDto {
            profiles: vec![profile.clone()],
            next_cursor: Some("private-signed-cursor".into()),
        };
        let avatar = AvatarDto {
            content: vec![222, 173, 190, 239],
            content_type: "image/png".into(),
            filename: "private-avatar-filename".into(),
        };
        let avatar_response = AvatarUploadResponseDto {
            user_id: "private-avatar-response-user-id".into(),
        };
        let apply_deletion = ApplyUserDeletionDto {
            saga_id: "private-deletion-saga-id".into(),
            user_id: "private-deletion-user-id".into(),
            actor_id: "private-deletion-actor-id".into(),
            retention_until: "private-retention-date".into(),
        };

        let rendered = [
            format!("{profile:?}"),
            format!("{update:?}"),
            format!("{privacy:?}"),
            format!("{deletion:?}"),
            format!("{purge:?}"),
            format!("{delete_response:?}"),
            format!("{search:?}"),
            format!("{avatar:?}"),
            format!("{avatar_response:?}"),
            format!("{apply_deletion:?}"),
        ]
        .join("\n");

        for private_value in [
            "private-profile-user-id",
            "private-profile-username",
            "private-profile-nickname",
            "private-profile-avatar-url",
            "private-profile-bio",
            "private-update-user-id",
            "private-update-username",
            "private-update-nickname",
            "private-update-avatar-url",
            "private-update-bio",
            "private-privacy-user-id",
            "private-privacy-actor-id",
            "private-delete-user-id",
            "private-delete-actor-id",
            "private-delete-request-id",
            "private-delete-ip",
            "private-delete-user-agent",
            "private-purge-actor-id",
            "private-purge-request-id",
            "private-purge-ip",
            "private-purge-user-agent",
            "private-delete-response-user-id",
            "private-signed-cursor",
            "222, 173, 190, 239",
            "private-avatar-filename",
            "private-avatar-response-user-id",
            "private-deletion-saga-id",
            "private-deletion-user-id",
            "private-deletion-actor-id",
            "private-retention-date",
        ] {
            assert!(!rendered.contains(private_value), "{rendered}");
        }
        assert!(rendered.contains("[REDACTED]"), "{rendered}");
        assert!(rendered.contains("profiles_len: 1"), "{rendered}");
        assert!(rendered.contains("content_len: 4"), "{rendered}");
        assert!(rendered.contains("image/png"), "{rendered}");

        let serialized_profile = serde_json::to_value(&profile).expect("serialize Profile DTO");
        assert_eq!(serialized_profile["username"], "private-profile-username");
        assert_eq!(serialized_profile["bio"], "private-profile-bio");
        let serialized_update = serde_json::to_value(&update).expect("serialize update DTO");
        assert_eq!(serialized_update["username"], "private-update-username");
        assert_eq!(serialized_update["bio"], "private-update-bio");
        let serialized_avatar = serde_json::to_value(&avatar).expect("serialize Avatar DTO");
        assert_eq!(
            serialized_avatar["content"],
            serde_json::json!([222, 173, 190, 239])
        );
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyUserDeletionDto {
    pub saga_id: String,
    pub user_id: String,
    pub actor_id: String,
    pub retention_until: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyUserDeletionResponseDto {
    pub applied: bool,
}

/// Internal owning-transaction evidence. It is not a public caller capability.
#[derive(Clone, PartialEq, Eq)]
pub struct CommittedDeletionReceiptDto {
    pub version: u32,
    pub saga_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub retention_until: chrono::DateTime<chrono::Utc>,
    pub fence_revision: i64,
    pub effect_xid: String,
    pub recorded_at: chrono::DateTime<chrono::Utc>,
    pub owner: String,
}

redacted_debug! {
    ProfileDto {
        user_id,
        username,
        nickname,
        avatar_url,
        bio,
        follower_count,
        post_count,
        following_count,
        account_visibility,
        lifecycle_status,
    },
    UpdateProfileDto { user_id, username, nickname, avatar_url, bio },
    UpdateProfilePrivacyDto { user_id, actor_id, account_visibility },
    DeleteProfileDto { user_id, actor_id, request_id, ip_address, user_agent },
    ProfileDeletionAuditDto { id, request_id, ip_address, user_agent, metadata_json, occurred_at },
    PurgeExpiredProfilesDto { actor_id, request_id, ip_address, user_agent, dry_run },
    AvatarUploadResponseDto { user_id },
    ApplyUserDeletionDto { saga_id, user_id, actor_id, retention_until },
}

impl std::fmt::Debug for DeleteProfileResponseDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeleteProfileResponseDto")
            .field("user_id", &"[REDACTED]")
            .field("deleted", &self.deleted)
            .field("account_deleted", &self.account_deleted)
            .finish()
    }
}

impl std::fmt::Debug for ProfileSearchPageDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProfileSearchPageDto")
            .field("profiles_len", &self.profiles.len())
            .field("next_cursor", &"[REDACTED]")
            .finish()
    }
}

impl std::fmt::Debug for AvatarDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AvatarDto")
            .field("content", &"[REDACTED]")
            .field("content_len", &self.content.len())
            .field("content_type", &self.content_type)
            .field("filename", &"[REDACTED]")
            .finish()
    }
}
