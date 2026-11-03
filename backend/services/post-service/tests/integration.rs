mod support;

#[path = "integration/post_event_outbox_flow.rs"]
mod post_event_outbox_flow;

#[path = "integration/post_consumer_dedup_flow.rs"]
mod post_consumer_dedup_flow;

#[path = "integration/post_owner_receipts.rs"]
mod post_owner_receipts;

#[path = "integration/post_feed_cache_flow.rs"]
mod post_feed_cache_flow;

#[path = "integration/post_notification_forwarding_flow.rs"]
mod post_notification_forwarding_flow;

#[path = "integration/post_moderation_flow.rs"]
mod post_moderation_flow;

#[path = "integration/post_postgres_flow.rs"]
mod post_postgres_flow;

#[path = "integration/post_ranked_feed_flow.rs"]
mod post_ranked_feed_flow;

#[path = "integration/post_user_deletion_fence.rs"]
mod post_user_deletion_fence;
