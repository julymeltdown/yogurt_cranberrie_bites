use crate::application::delivery_effect::DeliveryEffectFailure;
use crate::application::ports::{
    FriendClientPort, InternalEventDispatcherPort, NotificationClientPort, PostRepositoryPort,
};
use crate::domain::post::{NotificationRequest, StoredEvent};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct InternalNotificationListener {
    posts: Arc<dyn PostRepositoryPort>,
    friend_client: Arc<dyn FriendClientPort>,
    client: Arc<dyn NotificationClientPort>,
}

impl InternalNotificationListener {
    pub fn new(
        posts: Arc<dyn PostRepositoryPort>,
        friend_client: Arc<dyn FriendClientPort>,
        client: Arc<dyn NotificationClientPort>,
    ) -> Self {
        Self {
            posts,
            friend_client,
            client,
        }
    }
}

#[async_trait]
impl InternalEventDispatcherPort for InternalNotificationListener {
    async fn dispatch(&self, event: &StoredEvent) -> Result<()> {
        match event.event_type.as_str() {
            // A later recipient may fail after earlier deliveries committed.
            "POST_CREATED" => self
                .dispatch_post_created(event)
                .await
                .map_err(DeliveryEffectFailure::uncertain),
            "POST_LIKED" | "POST_COMMENTED" | "COMMENT" => self.dispatch_direct(event).await,
            _ => Ok(()),
        }
    }
}

impl InternalNotificationListener {
    async fn dispatch_post_created(&self, event: &StoredEvent) -> Result<()> {
        // Outbox attributes and payloads describe the creation-time state.
        // Delayed delivery/replay must consult the owner's current state before
        // it copies content to another service. Missing/deleted/moderated posts
        // and unsupported visibility are deliberately not notification sources.
        let snapshots = self
            .posts
            .fetch_post_access_snapshots(&[event.aggregate_id])
            .await?;
        let Some(post) = snapshots.get(&event.aggregate_id) else {
            return Ok(());
        };
        if !matches!(post.visibility.as_str(), "PUBLIC" | "FRIENDS") {
            return Ok(());
        }
        let payload: Value = serde_json::from_str(&event.payload)?;
        let author_id = payload
            .get("authorId")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Missing authorId"))?;
        let author_id = Uuid::parse_str(author_id)?;
        let post_id = payload
            .get("postId")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Missing postId"))?;
        anyhow::ensure!(
            Uuid::parse_str(post_id)? == event.aggregate_id
                && author_id == post.author_id
                && event.actor_id == Some(author_id),
            "Post notification identity mismatch"
        );
        let follower_ids = self.friend_client.fetch_follower_ids(author_id).await?;
        for follower_id in follower_ids {
            self.notify_with_policy(event, follower_id, author_id, post.visibility == "FRIENDS")
                .await?;
        }
        Ok(())
    }

    async fn dispatch_direct(&self, event: &StoredEvent) -> Result<()> {
        let payload: Value = serde_json::from_str(&event.payload)?;
        let recipient_id = payload
            .get("notifyUserId")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Missing notifyUserId"))?;
        let recipient_id = Uuid::parse_str(recipient_id)?;
        let actor_id = event
            .actor_id
            .ok_or_else(|| anyhow::anyhow!("Missing actorId"))?;
        self.notify_if_allowed(event, recipient_id, actor_id).await
    }

    async fn notify_if_allowed(
        &self,
        event: &StoredEvent,
        recipient_id: Uuid,
        actor_id: Uuid,
    ) -> Result<()> {
        self.notify_with_policy(event, recipient_id, actor_id, false)
            .await
    }

    async fn notify_with_policy(
        &self,
        event: &StoredEvent,
        recipient_id: Uuid,
        actor_id: Uuid,
        requires_following: bool,
    ) -> Result<()> {
        if recipient_id == actor_id {
            return Ok(());
        }
        let policy = self
            .friend_client
            .relationship_policy(recipient_id, actor_id, None)
            .await?;
        if policy.is_blocked()
            || policy.muted_by_viewer
            || (requires_following && !policy.following)
        {
            return Ok(());
        }
        // Notification storage is a resource reference, never a second copy of
        // mutable post/comment text. Keep the original outbox/Kafka event intact.
        let payload: Value = serde_json::from_str(&event.payload)?;
        let post_id = payload
            .get("postId")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Missing postId"))?;
        let post_id = Uuid::parse_str(post_id)?;
        self.client
            .create_notification(NotificationRequest {
                user_id: recipient_id,
                notification_type: event.event_type.clone(),
                payload_json: serde_json::json!({"payloadVersion": 1, "postId": post_id})
                    .to_string(),
                event_id: event.event_id,
                actor_id: event.actor_id,
                occurred_at: event.occurred_at.to_rfc3339(),
            })
            .await
    }
}
