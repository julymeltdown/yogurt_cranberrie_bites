use serde::{Deserialize, Serialize};

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

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUpdateRequest {
    pub username: String,
    pub nickname: String,
    pub avatar_url: String,
    pub bio: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePrivacyUpdateRequest {
    pub account_visibility: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileBatchRequest {
    pub user_ids: Vec<String>,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileResponse {
    pub user_id: String,
    pub username: String,
    pub nickname: String,
    pub avatar_url: String,
    pub bio: String,
    pub follower_count: i64,
    pub following_count: i64,
    pub post_count: i64,
    pub account_visibility: String,
    pub lifecycle_status: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileBatchResponse {
    pub profiles: Vec<ProfileResponse>,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteProfileResponse {
    pub user_id: String,
    pub deleted: bool,
    pub account_deleted: bool,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSearchResponse {
    pub profiles: Vec<ProfileResponse>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AvatarUploadResponse {
    pub avatar_url: String,
}

redacted_debug! {
    ProfileUpdateRequest { username, nickname, avatar_url, bio },
    ProfilePrivacyUpdateRequest { account_visibility },
    ProfileBatchRequest { user_ids },
    ProfileResponse {
        user_id,
        username,
        nickname,
        avatar_url,
        bio,
        follower_count,
        following_count,
        post_count,
        account_visibility,
        lifecycle_status,
    },
    ProfileBatchResponse { profiles },
    DeleteProfileResponse { user_id, deleted, account_deleted },
    ProfileSearchResponse { profiles, next_cursor },
    AvatarUploadResponse { avatar_url },
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AvatarContent {
    pub content: Vec<u8>,
    pub content_type: String,
}

impl std::fmt::Debug for AvatarContent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AvatarContent")
            .field("content", &"[REDACTED]")
            .field("content_len_bytes", &self.content.len())
            .field("content_type", &self.content_type)
            .finish()
    }
}

#[cfg(test)]
mod secret_debug_tests {
    use super::{
        AvatarContent, AvatarUploadResponse, DeleteProfileResponse, ProfileBatchRequest,
        ProfileBatchResponse, ProfilePrivacyUpdateRequest, ProfileResponse, ProfileSearchResponse,
        ProfileUpdateRequest,
    };

    #[test]
    fn avatar_content_debug_does_not_render_uploaded_bytes() {
        let avatar = AvatarContent {
            content: b"private-avatar-payload".to_vec(),
            content_type: "image/png".into(),
        };

        let rendered = format!("{avatar:?}");

        assert!(!rendered.contains("private-avatar-payload"), "{rendered}");
        assert!(rendered.contains("[REDACTED]"), "{rendered}");
        assert!(rendered.contains("image/png"), "{rendered}");
    }

    #[test]
    fn profile_dto_debug_does_not_render_personal_data_or_opaque_identifiers() {
        let profile = ProfileResponse {
            user_id: "private-profile-user-821".into(),
            username: "private-handle-4c10".into(),
            nickname: "private-nickname-935".into(),
            avatar_url: "https://private.example/avatar/secret-object".into(),
            bio: "private-profile-bio-6d7a".into(),
            follower_count: 17,
            following_count: 23,
            post_count: 5,
            account_visibility: "PRIVATE".into(),
            lifecycle_status: "ACTIVE".into(),
        };
        let values = [
            format!(
                "{:?}",
                ProfileUpdateRequest {
                    username: "private-handle-4c10".into(),
                    nickname: "private-nickname-935".into(),
                    avatar_url: "https://private.example/avatar/secret-object".into(),
                    bio: "private-profile-bio-6d7a".into(),
                }
            ),
            format!(
                "{:?}",
                ProfilePrivacyUpdateRequest {
                    account_visibility: "PRIVATE-visibility-sentinel".into(),
                }
            ),
            format!(
                "{:?}",
                ProfileBatchRequest {
                    user_ids: vec!["private-batch-user-415".into()],
                }
            ),
            format!("{profile:?}"),
            format!(
                "{:?}",
                ProfileBatchResponse {
                    profiles: vec![profile.clone()],
                }
            ),
            format!(
                "{:?}",
                DeleteProfileResponse {
                    user_id: "private-delete-user-621".into(),
                    deleted: true,
                    account_deleted: false,
                }
            ),
            format!(
                "{:?}",
                ProfileSearchResponse {
                    profiles: vec![profile],
                    next_cursor: Some("private-profile-cursor-0123".into()),
                }
            ),
            format!(
                "{:?}",
                AvatarUploadResponse {
                    avatar_url: "https://private.example/avatar/uploaded-object".into(),
                }
            ),
        ];
        let rendered = values.join("\n");

        for sentinel in [
            "private-profile-user-821",
            "private-handle-4c10",
            "private-nickname-935",
            "https://private.example/avatar/secret-object",
            "private-profile-bio-6d7a",
            "PRIVATE-visibility-sentinel",
            "private-batch-user-415",
            "private-delete-user-621",
            "private-profile-cursor-0123",
            "https://private.example/avatar/uploaded-object",
        ] {
            assert!(
                !rendered.contains(sentinel),
                "{sentinel} leaked: {rendered}"
            );
        }
        assert!(rendered.matches("[REDACTED]").count() >= 8, "{rendered}");
    }

    #[test]
    fn profile_dto_json_serialization_keeps_the_existing_wire_fields() {
        let update = ProfileUpdateRequest {
            username: "wire-user".into(),
            nickname: "Wire Nickname".into(),
            avatar_url: "https://cdn.example/wire.png".into(),
            bio: "wire bio".into(),
        };

        assert_eq!(
            serde_json::to_value(update).unwrap(),
            serde_json::json!({
                "username": "wire-user",
                "nickname": "Wire Nickname",
                "avatarUrl": "https://cdn.example/wire.png",
                "bio": "wire bio"
            })
        );
    }
}
