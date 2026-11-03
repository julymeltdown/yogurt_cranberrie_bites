use crate::dto::notification_dto::{
    NotificationFeedResponse, NotificationMarkAllReadResponse, NotificationResponse,
};
use crate::ports::{GatewayRuntimePort, NotificationClientPort, PostClientPort};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct NotificationGatewayService {
    notification_client: Arc<dyn NotificationClientPort>,
    post_client: Arc<dyn PostClientPort>,
    projection_budget: Duration,
    runtime: Arc<dyn GatewayRuntimePort>,
}

impl NotificationGatewayService {
    pub fn new(
        notification_client: Arc<dyn NotificationClientPort>,
        post_client: Arc<dyn PostClientPort>,
        runtime: Arc<dyn GatewayRuntimePort>,
    ) -> Self {
        Self {
            notification_client,
            post_client,
            runtime,
            projection_budget: Duration::from_millis(900),
        }
    }

    pub fn with_projection_budget(mut self, budget: Duration) -> Self {
        self.projection_budget = budget;
        self
    }

    pub async fn list_notifications(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::service_error::GatewayServiceResult<NotificationFeedResponse> {
        self.list_notifications_with_budget(user_id, cursor, limit, None)
            .await
    }

    pub async fn list_notifications_with_budget(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
        budget: Option<Duration>,
    ) -> crate::service_error::GatewayServiceResult<NotificationFeedResponse> {
        let started = self.runtime.monotonic_now();
        let budget = budget
            .unwrap_or(self.projection_budget)
            .min(self.projection_budget);
        let mut page = self
            .notification_client
            .list_notifications(user_id.clone(), cursor, limit.clamp(1, 100))
            .await?;
        if page.items.len() > 100 {
            return Err(crate::service_error::GatewayServiceError::NotificationPageBound);
        }
        let ids: BTreeSet<_> = page
            .items
            .iter()
            .filter(|item| item.user_id == user_id)
            .filter_map(post_reference)
            .collect();
        let decisions = if ids.is_empty() {
            HashMap::new()
        } else {
            let remaining = budget.saturating_sub(
                self.runtime
                    .monotonic_now()
                    .saturating_duration_since(started),
            );
            self.post_client
                .notification_posts_visible(
                    user_id.clone(),
                    ids.iter().cloned().collect(),
                    remaining,
                )
                .await?
        };
        if !ids.iter().all(|id| decisions.contains_key(id)) {
            return Err(
                crate::service_error::GatewayServiceError::NotificationAuthorizationIncomplete,
            );
        }
        page.items
            .retain(|item| payload_visible(&user_id, item, &decisions));
        for item in &mut page.items {
            normalize_post_reference(item);
        }
        // Do not scan additional pages to fill filtered rows: preserve bounded
        // work and the owner's continuation, including an empty visible page.
        Ok(page)
    }

    pub async fn mark_read(
        &self,
        user_id: String,
        notification_id: String,
    ) -> crate::service_error::GatewayServiceResult<NotificationResponse> {
        self.mark_read_with_budget(user_id, notification_id, None)
            .await
    }

    pub async fn mark_read_with_budget(
        &self,
        user_id: String,
        notification_id: String,
        budget: Option<Duration>,
    ) -> crate::service_error::GatewayServiceResult<NotificationResponse> {
        let started = self.runtime.monotonic_now();
        let budget = budget
            .unwrap_or(self.projection_budget)
            .min(self.projection_budget);
        let mut item = self
            .notification_client
            .mark_read(user_id.clone(), notification_id)
            .await?;
        let visible = if let Some(post_id) = post_reference(&item) {
            let remaining = budget.saturating_sub(
                self.runtime
                    .monotonic_now()
                    .saturating_duration_since(started),
            );
            if remaining.is_zero() {
                false
            } else {
                self.post_client
                    .notification_posts_visible(user_id.clone(), vec![post_id.clone()], remaining)
                    .await
                    .ok()
                    .and_then(|decisions| decisions.get(&post_id).copied())
                    .unwrap_or(false)
            }
        } else {
            !is_post_notification(&item)
        };
        // The read effect has already committed. A denied/unknown projection
        // must redact rather than turn that known success into a failure.
        if item.user_id != user_id || !visible {
            item.payload_json = "{}".into();
        } else {
            normalize_post_reference(&mut item);
        }
        Ok(item)
    }

    pub async fn mark_all_read(
        &self,
        user_id: String,
    ) -> crate::service_error::GatewayServiceResult<NotificationMarkAllReadResponse> {
        self.notification_client
            .mark_all_read(user_id)
            .await
            .map_err(Into::into)
    }
}

fn is_post_notification(item: &NotificationResponse) -> bool {
    matches!(
        item.notification_type.as_str(),
        "POST_CREATED" | "POST_LIKED" | "POST_COMMENTED" | "COMMENT"
    )
}

fn post_reference(item: &NotificationResponse) -> Option<String> {
    if !is_post_notification(item) {
        return None;
    }
    let payload: serde_json::Value = serde_json::from_str(&item.payload_json).ok()?;
    let id = uuid::Uuid::parse_str(payload.get("postId")?.as_str()?).ok()?;
    Some(id.to_string())
}

fn payload_visible(
    user_id: &str,
    item: &NotificationResponse,
    decisions: &HashMap<String, bool>,
) -> bool {
    item.user_id == user_id
        && (!is_post_notification(item)
            || post_reference(item).is_some_and(|id| decisions.get(&id) == Some(&true)))
}

fn normalize_post_reference(item: &mut NotificationResponse) {
    if !matches!(
        item.notification_type.as_str(),
        "POST_CREATED" | "POST_LIKED" | "POST_COMMENTED" | "COMMENT"
    ) {
        return;
    }
    // An allowlist also repairs historical payloads: merely deleting `content`
    // would leave arbitrary old previews or nested copies available to clients.
    let post_id = serde_json::from_str::<serde_json::Value>(&item.payload_json)
        .ok()
        .and_then(|payload| {
            payload
                .get("postId")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| uuid::Uuid::parse_str(value).ok())
        });
    item.payload_json = post_id.map_or_else(
        || "{}".into(),
        |post_id| serde_json::json!({"payloadVersion": 1, "postId": post_id}).to_string(),
    );
}
