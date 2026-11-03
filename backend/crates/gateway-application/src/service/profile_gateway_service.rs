use crate::dto::friend_dto::FollowCountsResponse;
use crate::dto::post_dto::MutationReceiptStatusDto;
use crate::dto::profile_dto::{
    AvatarContent, DeleteProfileResponse, ProfileBatchResponse, ProfilePrivacyUpdateRequest,
    ProfileResponse, ProfileSearchResponse, ProfileUpdateRequest,
};
use crate::ports::{FriendClientPort, PostClientPort, ProfileClientPort};
use crate::search_cursor::{SearchCursorCodec, SearchSort};
use gateway_domain::profile::ProfileNotVisible;
use std::collections::HashMap;
use std::sync::Arc;

/// Maximum rows requested across batches, including batches filtered to nothing.
const SEARCH_SCAN_ROWS: usize = 500;

/// Avoid one RPC per remaining result when most profiles are hidden.
const MIN_SEARCH_BATCH: usize = 25;

fn normalize_search_limit(limit: i32) -> usize {
    if limit > 0 {
        (limit as usize).min(100)
    } else {
        10
    }
}

#[derive(Clone)]
pub struct ProfileGatewayService {
    profile_client: Arc<dyn ProfileClientPort>,
    friend_client: Arc<dyn FriendClientPort>,
    post_client: Arc<dyn PostClientPort>,
    search_cursor_codec: SearchCursorCodec,
}

impl ProfileGatewayService {
    pub fn new(
        profile_client: Arc<dyn ProfileClientPort>,
        friend_client: Arc<dyn FriendClientPort>,
        post_client: Arc<dyn PostClientPort>,
        search_cursor_codec: SearchCursorCodec,
    ) -> Self {
        Self {
            profile_client,
            friend_client,
            post_client,
            search_cursor_codec,
        }
    }

    pub async fn get_mutation_receipt_status(
        &self,
        operation_id: uuid::Uuid,
    ) -> crate::service_error::GatewayServiceResult<Option<MutationReceiptStatusDto>> {
        self.profile_client
            .get_mutation_receipt_status(operation_id)
            .await
            .map_err(Into::into)
    }

    pub async fn get_profile(
        &self,
        user_id: String,
    ) -> crate::service_error::GatewayServiceResult<ProfileResponse> {
        self.get_profile_for_viewer(None, user_id).await
    }

    pub async fn get_profile_for_viewer(
        &self,
        viewer_id: Option<String>,
        user_id: String,
    ) -> crate::service_error::GatewayServiceResult<ProfileResponse> {
        let profile = self.profile_client.get_profile(user_id).await?;
        self.ensure_directly_visible(viewer_id.as_deref(), &profile)
            .await?;
        let counts = self
            .friend_client
            .get_counts(profile.user_id.clone())
            .await?;
        let post_counts =
            load_post_counts(&self.post_client, std::slice::from_ref(&profile)).await?;
        let post_count = post_counts.get(&profile.user_id).copied();
        Ok(merge_counts(profile, Some(counts), post_count))
    }

    pub async fn get_profile_by_username(
        &self,
        username: String,
    ) -> crate::service_error::GatewayServiceResult<ProfileResponse> {
        self.get_profile_by_username_for_viewer(None, username)
            .await
    }

    pub async fn get_profile_by_username_for_viewer(
        &self,
        viewer_id: Option<String>,
        username: String,
    ) -> crate::service_error::GatewayServiceResult<ProfileResponse> {
        let profile = self
            .profile_client
            .get_profile_by_username(username)
            .await?;
        self.ensure_directly_visible(viewer_id.as_deref(), &profile)
            .await?;
        let counts = self
            .friend_client
            .get_counts(profile.user_id.clone())
            .await?;
        let post_counts =
            load_post_counts(&self.post_client, std::slice::from_ref(&profile)).await?;
        let post_count = post_counts.get(&profile.user_id).copied();
        Ok(merge_counts(profile, Some(counts), post_count))
    }

    pub async fn get_profiles(
        &self,
        user_ids: Vec<String>,
    ) -> crate::service_error::GatewayServiceResult<ProfileBatchResponse> {
        self.get_profiles_for_viewer(None, user_ids).await
    }

    pub async fn get_profiles_for_viewer(
        &self,
        viewer_id: Option<String>,
        user_ids: Vec<String>,
    ) -> crate::service_error::GatewayServiceResult<ProfileBatchResponse> {
        let response = self.profile_client.get_profiles(user_ids).await?;
        let profiles = self
            .filter_profiles(viewer_id.as_deref(), response.profiles, false)
            .await?;
        let counts = self.load_counts(&profiles).await?;
        let post_counts = load_post_counts(&self.post_client, &profiles).await?;
        Ok(ProfileBatchResponse {
            profiles: profiles
                .into_iter()
                .map(|profile| {
                    let user_id = profile.user_id.clone();
                    merge_counts(
                        profile,
                        counts.get(&user_id).copied(),
                        post_counts.get(&user_id).copied(),
                    )
                })
                .collect(),
        })
    }

    pub async fn search_profiles(
        &self,
        query: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::service_error::GatewayServiceResult<ProfileSearchResponse> {
        self.search_profiles_for_viewer(None, query, cursor, limit)
            .await
    }

    pub async fn search_profiles_for_viewer(
        &self,
        viewer_id: Option<String>,
        query: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::service_error::GatewayServiceResult<ProfileSearchResponse> {
        let limit = normalize_search_limit(limit);
        // Relationship filtering happens after the profile service's database
        // limit. Scan past hidden batches until the page is full or work is bounded.
        let mut visible: Vec<ProfileResponse> = Vec::new();
        let sort = SearchSort::ProfilesUsernameAscV1;
        let mut scan_from = self
            .search_cursor_codec
            .decode(cursor.as_deref(), &query, viewer_id.as_deref(), sort)
            .map_err(crate::service_error::GatewayServiceError::from)?;
        let mut requested_rows = 0usize;
        let exhausted;
        loop {
            let remaining = limit - visible.len();
            let batch = remaining
                .max(MIN_SEARCH_BATCH)
                .min(SEARCH_SCAN_ROWS - requested_rows);
            let requested_from = scan_from.clone();
            let response = self
                .profile_client
                .search_profiles(query.clone(), requested_from.clone(), batch as i32)
                .await?;
            let returned_cursor = response.next_cursor;
            // Charge requested capacity so even an empty upstream page with an
            // advancing cursor cannot extend the scan indefinitely.
            requested_rows += batch;
            let admitted = self
                .filter_profiles(viewer_id.as_deref(), response.profiles, true)
                .await?;
            // Do not admit the same upstream page twice if its cursor stalls.
            let repeated = requested_from.is_some() && returned_cursor == requested_from;
            let advanced = returned_cursor.is_some() && !repeated;
            scan_from = returned_cursor;
            if !repeated {
                visible.extend(admitted);
            }
            if visible.len() >= limit || !advanced || requested_rows >= SEARCH_SCAN_ROWS {
                exhausted = !advanced;
                break;
            }
        }
        let overflowed = visible.len() > limit;
        visible.truncate(limit);
        // The continuation is the page boundary, not the batch cursor: the batch
        // cursor names the last profile *fetched*, and one batch can return more
        // admitted profiles than fit on the page.
        let next_position = if overflowed {
            visible.last().map(|profile| profile.username.clone())
        } else if exhausted {
            None
        } else {
            // The row budget stopped the scan before the matching set was read.
            scan_from
        };
        let next_cursor = self.search_cursor_codec.encode(
            next_position.as_deref(),
            &query,
            viewer_id.as_deref(),
            sort,
        )?;
        let counts = self.load_counts(&visible).await?;
        let post_counts = load_post_counts(&self.post_client, &visible).await?;
        Ok(ProfileSearchResponse {
            profiles: visible
                .into_iter()
                .map(|profile| {
                    let user_id = profile.user_id.clone();
                    merge_counts(
                        profile,
                        counts.get(&user_id).copied(),
                        post_counts.get(&user_id).copied(),
                    )
                })
                .collect(),
            next_cursor,
        })
    }

    pub async fn upload_avatar(
        &self,
        user_id: String,
        content: Vec<u8>,
        content_type: String,
        filename: String,
    ) -> crate::service_error::GatewayServiceResult<String> {
        self.profile_client
            .upload_avatar(user_id, content, content_type, filename)
            .await
            .map_err(Into::into)
    }

    pub async fn get_avatar(
        &self,
        user_id: String,
    ) -> crate::service_error::GatewayServiceResult<AvatarContent> {
        self.profile_client
            .get_avatar(user_id)
            .await
            .map_err(Into::into)
    }

    pub async fn update_profile(
        &self,
        user_id: String,
        request: ProfileUpdateRequest,
    ) -> crate::service_error::GatewayServiceResult<ProfileResponse> {
        let profile = self.profile_client.update_profile(user_id, request).await?;
        let counts = self
            .friend_client
            .get_counts(profile.user_id.clone())
            .await?;
        let post_counts =
            load_post_counts(&self.post_client, std::slice::from_ref(&profile)).await?;
        let post_count = post_counts.get(&profile.user_id).copied();
        Ok(merge_counts(profile, Some(counts), post_count))
    }

    pub async fn update_profile_privacy(
        &self,
        user_id: String,
        request: ProfilePrivacyUpdateRequest,
    ) -> crate::service_error::GatewayServiceResult<ProfileResponse> {
        let profile = self
            .profile_client
            .update_profile_privacy(user_id, request)
            .await?;
        let counts = self
            .friend_client
            .get_counts(profile.user_id.clone())
            .await?;
        let post_counts =
            load_post_counts(&self.post_client, std::slice::from_ref(&profile)).await?;
        let post_count = post_counts.get(&profile.user_id).copied();
        Ok(merge_counts(profile, Some(counts), post_count))
    }

    pub async fn delete_profile(
        &self,
        user_id: String,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> crate::service_error::GatewayServiceResult<DeleteProfileResponse> {
        self.profile_client
            .delete_profile(user_id, request_id, ip_address, user_agent)
            .await
            .map_err(Into::into)
    }

    async fn ensure_directly_visible(
        &self,
        viewer_id: Option<&str>,
        profile: &ProfileResponse,
    ) -> crate::service_error::GatewayServiceResult<()> {
        if profile.lifecycle_status != "ACTIVE" {
            return Err(ProfileNotVisible.into());
        }

        match profile.account_visibility.as_str() {
            "PUBLIC" => {}
            "PRIVATE" => {
                if viewer_id == Some(profile.user_id.as_str()) {
                    return Ok(());
                }
                if viewer_id.is_none() {
                    return Err(ProfileNotVisible.into());
                }
            }
            _ => return Err(ProfileNotVisible.into()),
        }

        let Some(viewer_id) = viewer_id.filter(|viewer_id| *viewer_id != profile.user_id) else {
            return Ok(());
        };
        let policy = self
            .friend_client
            .get_relationship(viewer_id.to_owned(), profile.user_id.clone())
            .await?;
        if policy.blocked_by_viewer
            || policy.blocked_viewer
            || (profile.account_visibility == "PRIVATE" && !policy.following)
        {
            return Err(ProfileNotVisible.into());
        }
        Ok(())
    }

    async fn filter_profiles(
        &self,
        viewer_id: Option<&str>,
        profiles: Vec<ProfileResponse>,
        hide_muted: bool,
    ) -> crate::service_error::GatewayServiceResult<Vec<ProfileResponse>> {
        let Some(viewer_id) = viewer_id else {
            return Ok(profiles
                .into_iter()
                .filter(|profile| {
                    profile.lifecycle_status == "ACTIVE" && profile.account_visibility == "PUBLIC"
                })
                .collect());
        };

        let targets = profiles
            .iter()
            .filter(|profile| {
                profile.user_id != viewer_id
                    && profile.lifecycle_status == "ACTIVE"
                    && matches!(profile.account_visibility.as_str(), "PUBLIC" | "PRIVATE")
            })
            .map(|profile| profile.user_id.clone())
            .collect::<Vec<_>>();
        let policies = if targets.is_empty() {
            HashMap::new()
        } else {
            self.friend_client
                .get_relationships_batch(viewer_id.to_owned(), targets.clone())
                .await?
        };

        let expected_targets = targets.iter().collect::<std::collections::HashSet<_>>();
        if targets.iter().any(|target| !policies.contains_key(target))
            || policies
                .keys()
                .any(|target| !expected_targets.contains(target))
        {
            return Err(crate::service_error::GatewayServiceError::ProfileRelationshipIncomplete);
        }

        let mut visible = Vec::with_capacity(profiles.len());
        for profile in profiles {
            if profile.lifecycle_status != "ACTIVE"
                || !matches!(profile.account_visibility.as_str(), "PUBLIC" | "PRIVATE")
            {
                continue;
            }
            if profile.user_id == viewer_id {
                visible.push(profile);
                continue;
            }

            let policy = policies
                .get(&profile.user_id)
                .expect("relationship policy completeness checked above");
            if policy.blocked_by_viewer
                || policy.blocked_viewer
                || (profile.account_visibility == "PRIVATE" && !policy.following)
                || (hide_muted && policy.muted_by_viewer)
            {
                continue;
            }
            visible.push(profile);
        }
        Ok(visible)
    }

    async fn load_counts(
        &self,
        profiles: &[ProfileResponse],
    ) -> crate::service_error::GatewayServiceResult<HashMap<String, FollowCountsResponse>> {
        let user_ids = profiles
            .iter()
            .map(|profile| profile.user_id.clone())
            .collect::<Vec<_>>();
        self.friend_client
            .get_counts_batch(user_ids)
            .await
            .map_err(Into::into)
    }
}

/// Overlay resolved counts onto a profile.
///
/// Each value is only replaced when a real observation is available. A missing
/// observation leaves the stored value in place rather than substituting zero,
/// so a dependency that did not answer cannot be mistaken for a real count of
/// zero.
fn merge_counts(
    profile: ProfileResponse,
    counts: Option<FollowCountsResponse>,
    post_count: Option<i64>,
) -> ProfileResponse {
    let follower_count = counts
        .as_ref()
        .map(|value| value.follower_count)
        .unwrap_or(profile.follower_count);
    let following_count = counts
        .as_ref()
        .map(|value| value.following_count)
        .unwrap_or(profile.following_count);
    let post_count = post_count.unwrap_or(profile.post_count);
    ProfileResponse {
        follower_count,
        following_count,
        post_count,
        ..profile
    }
}

/// Load the authoritative post counts for a set of profiles.
///
/// The profile service stores a post_count column that nothing maintains, so
/// reading it produced a permanent zero that looked like a measured value. The
/// count is answered by the service that owns the posts instead. A lookup failure
/// is returned as an error rather than swallowed into a zero, because a zero is
/// itself a meaningful count and must not double as "unknown".
async fn load_post_counts(
    post_client: &Arc<dyn PostClientPort>,
    profiles: &[ProfileResponse],
) -> crate::service_error::GatewayServiceResult<HashMap<String, i64>> {
    let user_ids = profiles
        .iter()
        .map(|profile| profile.user_id.clone())
        .collect::<Vec<_>>();
    if user_ids.is_empty() {
        return Ok(HashMap::new());
    }
    post_client
        .get_author_post_counts(user_ids)
        .await
        .map_err(Into::into)
}
