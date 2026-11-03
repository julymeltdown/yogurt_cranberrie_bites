use crate::support::{instant, post, stored_event, uuid, Harness, SpyNotificationClient, ACTOR_ID};
use post_service::adapters::out::messaging::kafka_publisher::DisabledEventPublisher;
use post_service::adapters::r#in::messaging::external_event_forwarder::ExternalEventForwarder;
use post_service::adapters::r#in::messaging::internal_notification_listener::InternalNotificationListener;
use post_service::adapters::r#in::messaging::outbox_worker::PostOutboxWorker;
use post_service::adapters::r#in::messaging::retention_purge_worker::PostRetentionPurgeWorker;
use post_service::application::ports::ExternalEventPublisherPort;
use post_service::application::use_cases::purge_expired_posts::PurgeExpiredPostsUseCase;
use post_service::config::PostMetrics;
use post_service::domain::post::{
    DeliveryAttemptPhase, DeliveryAttemptSource, DeliveryTarget, OutboxMetricsSnapshot, PostDetail,
    RetentionPurgeResult,
};
use std::sync::Arc;
use uuid::Uuid;

#[test]
fn disabled_external_publisher_stays_disabled_when_wrapped_in_arc() {
    let publisher: Arc<dyn ExternalEventPublisherPort> = Arc::new(DisabledEventPublisher::new());

    assert!(!publisher.is_enabled());
}

#[tokio::test]
async fn external_forwarder_invalidates_comment_event_caches_before_forwarding() {
    let harness = Harness::new();
    let event_id = uuid("33333333-3333-3333-3333-333333333301");
    let post_id = uuid("33333333-3333-3333-3333-333333333302");
    let author_id = uuid("33333333-3333-3333-3333-333333333303");
    let follower_id = uuid("33333333-3333-3333-3333-333333333304");
    *harness.friend.follower_ids.lock().unwrap() = vec![follower_id];
    *harness.outbox.unpublished.lock().unwrap() = vec![stored_event(
        event_id,
        "POST_COMMENTED",
        serde_json::json!({
            "postId": post_id,
            "notifyUserId": author_id
        })
        .to_string(),
    )];
    let forwarder = forwarder(&harness);

    let forwarded = forwarder
        .republish_unpublished(instant(1_700_000_300), 5)
        .await
        .expect("republish unpublished");

    assert_eq!(forwarded, 1);
    assert_eq!(*harness.post_cache.evicted.lock().unwrap(), vec![post_id]);
    assert_eq!(
        *harness.friend.follower_calls.lock().unwrap(),
        vec![author_id]
    );
    assert_eq!(
        *harness.feed_cache.invalidated.lock().unwrap(),
        vec![vec![follower_id, author_id]]
    );
    assert_eq!(*harness.internal.dispatched.lock().unwrap(), vec![event_id]);
    assert_eq!(*harness.publisher.published.lock().unwrap(), vec![event_id]);
    let attempts = harness.outbox.delivery_attempts.lock().unwrap();
    assert_eq!(attempts.len(), 8);
    for (pair, target) in attempts.chunks_exact(2).zip(DeliveryTarget::ALL) {
        assert_eq!(pair[0].phase, DeliveryAttemptPhase::Started);
        assert_eq!(pair[1].phase, DeliveryAttemptPhase::Succeeded);
        assert_eq!(pair[0].attempt_id, pair[1].attempt_id);
        assert_eq!(pair[0].event_id, event_id);
        assert_eq!(pair[0].target, target);
        assert_eq!(pair[1].target, target);
        assert_eq!(pair[0].source, DeliveryAttemptSource::OutboxWorker);
    }
}

#[tokio::test]
async fn external_forwarder_creates_a_child_trace_for_async_outbox_delivery() {
    const ORIGIN: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let harness = Harness::new();
    let event_id = uuid("33333333-3333-3333-3333-333333333306");
    let mut event = stored_event(
        event_id,
        "POST_LIKED",
        serde_json::json!({"postId": "33333333-3333-3333-3333-333333333307"}).to_string(),
    );
    event.origin_traceparent = Some(ORIGIN.to_owned());
    *harness.outbox.unpublished.lock().unwrap() = vec![event];

    let forwarded = forwarder(&harness)
        .republish_unpublished(instant(1_700_000_300), 1)
        .await
        .expect("deliver event with its persisted trace parent");

    assert_eq!(forwarded, 1);
    let traceparents = harness.publisher.traceparents.lock().unwrap();
    assert_eq!(traceparents.len(), 1);
    let child = traceparents[0]
        .as_deref()
        .expect("worker scopes the external delivery");
    assert!(child.starts_with("00-4bf92f3577b34da6a3ce929d0e0e4736-"));
    assert_ne!(&child[36..52], "00f067aa0ba902b7");
    assert!(child.ends_with("-01"));
}

#[tokio::test]
async fn external_forwarder_records_poison_event_and_continues_with_later_events() {
    let harness = Harness::new();
    let invalid_id = uuid("33333333-3333-3333-3333-333333333305");
    let valid_id = uuid("33333333-3333-3333-3333-333333333314");
    let post_id = uuid("33333333-3333-3333-3333-333333333315");
    *harness.outbox.unpublished.lock().unwrap() = vec![
        stored_event(invalid_id, "POST_DELETED", "{}".to_owned()),
        stored_event(
            valid_id,
            "POST_LIKED",
            serde_json::json!({ "postId": post_id }).to_string(),
        ),
    ];

    let forwarded = forwarder(&harness)
        .republish_unpublished(instant(1_700_000_300), 10)
        .await
        .expect("poison event must not stop the batch");

    assert_eq!(forwarded, 1);
    let failures = harness.outbox.failures.lock().unwrap();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].0, invalid_id);
    assert_eq!(failures[0].2, 5);
    // The poison event fails only its cache target, so the targets that do not
    // depend on the cache still run for it. The valid event also reaches them.
    assert_eq!(
        *harness.internal.dispatched.lock().unwrap(),
        vec![invalid_id, valid_id]
    );
    // Publication is likewise unaffected by the cache failure.
    assert_eq!(
        *harness.publisher.published.lock().unwrap(),
        vec![invalid_id, valid_id]
    );
    // Only the valid event is fully delivered, so only it counts as forwarded.
    assert_eq!(forwarded, 1);

    // The failure names the target that failed, so an operator reading the
    // recorded error can tell which destination needs attention.
    assert!(
        failures[0].1.contains("CACHE"),
        "the recorded error must name the failing target, got: {}",
        failures[0].1
    );
}

#[tokio::test]
async fn external_forwarder_delivers_notifications_while_the_cache_is_down() {
    // The regression this guards: targets were processed in a fixed order and
    // the first failure aborted the event, so a Redis outage stopped
    // notifications and external publication as well. Those targets do not
    // depend on the cache and must not be blocked by it.
    let harness = Harness::new();
    let event_id = uuid("33333333-3333-3333-3333-333333333330");
    let post_id = uuid("33333333-3333-3333-3333-333333333331");
    *harness.outbox.unpublished.lock().unwrap() = vec![stored_event(
        event_id,
        "POST_LIKED",
        serde_json::json!({ "postId": post_id }).to_string(),
    )];
    *harness.post_cache.error.lock().unwrap() = Some("redis unavailable".to_owned());

    let first = forwarder(&harness)
        .republish_unpublished(instant(1_700_000_300), 10)
        .await
        .expect("a cache outage must not fail the batch");

    // Given: the cache target failed.
    // Then: the independent targets still ran, and the event is not complete.
    assert_eq!(first, 0);
    assert_eq!(*harness.internal.dispatched.lock().unwrap(), vec![event_id]);
    assert_eq!(*harness.publisher.published.lock().unwrap(), vec![event_id]);
    {
        // Scoped so the guard is released before the retry below awaits.
        let completed = harness.outbox.completed.lock().unwrap();
        assert!(completed.contains(&(event_id, "INTERNAL".to_owned())));
        assert!(completed.contains(&(event_id, "EXTERNAL".to_owned())));
        assert!(!completed.contains(&(event_id, "CACHE".to_owned())));
    }
    assert!(harness.outbox.published.lock().unwrap().is_empty());
    let metrics = harness
        .metrics
        .render_prometheus(&OutboxMetricsSnapshot::default());
    let target_failures: Vec<_> = metrics
        .lines()
        .filter(|sample| sample.starts_with("post_outbox_worker_target_delivery_failures_total"))
        .collect();
    assert_eq!(
        target_failures,
        vec!["post_outbox_worker_target_delivery_failures_total{target=\"CACHE\"} 1"]
    );

    // When: the cache recovers and the event is retried.
    *harness.post_cache.error.lock().unwrap() = None;
    let second = forwarder(&harness)
        .republish_unpublished(instant(1_700_000_301), 10)
        .await
        .expect("retry after recovery");

    // Then: only the outstanding cache target is retried, and the targets that
    // already succeeded are not delivered a second time.
    assert_eq!(second, 1);
    assert_eq!(*harness.internal.dispatched.lock().unwrap(), vec![event_id]);
    assert_eq!(*harness.publisher.published.lock().unwrap(), vec![event_id]);
    assert_eq!(*harness.post_cache.evicted.lock().unwrap(), vec![post_id]);
    assert_eq!(*harness.outbox.published.lock().unwrap(), vec![event_id]);
    let attempts = harness.outbox.delivery_attempts.lock().unwrap();
    let cache_attempts: Vec<_> = attempts
        .iter()
        .filter(|attempt| attempt.target == DeliveryTarget::Cache)
        .collect();
    assert_eq!(cache_attempts.len(), 4);
    assert_eq!(cache_attempts[0].phase, DeliveryAttemptPhase::Started);
    assert_eq!(cache_attempts[1].phase, DeliveryAttemptPhase::Failed);
    assert_eq!(cache_attempts[2].phase, DeliveryAttemptPhase::Started);
    assert_eq!(cache_attempts[3].phase, DeliveryAttemptPhase::Succeeded);
    assert!(cache_attempts.iter().all(|attempt| {
        attempt.event_id == event_id && attempt.source == DeliveryAttemptSource::OutboxWorker
    }));
}

#[tokio::test]
async fn external_forwarder_resumes_from_completed_delivery_checkpoints() {
    let harness = Harness::new();
    let event_id = uuid("33333333-3333-3333-3333-333333333316");
    let post_id = uuid("33333333-3333-3333-3333-333333333317");
    *harness.outbox.unpublished.lock().unwrap() = vec![stored_event(
        event_id,
        "POST_LIKED",
        serde_json::json!({ "postId": post_id }).to_string(),
    )];
    *harness.publisher.error.lock().unwrap() = Some("broker unavailable".to_owned());
    let forwarder = forwarder(&harness);

    let first = forwarder
        .republish_unpublished(instant(1_700_000_300), 10)
        .await
        .expect("failed event is recorded, not returned as a batch failure");
    assert_eq!(first, 0);
    *harness.publisher.error.lock().unwrap() = None;
    let second = forwarder
        .republish_unpublished(instant(1_700_000_301), 10)
        .await
        .expect("retry");

    assert_eq!(second, 1);
    assert_eq!(*harness.post_cache.evicted.lock().unwrap(), vec![post_id]);
    assert_eq!(*harness.internal.dispatched.lock().unwrap(), vec![event_id]);
    assert_eq!(*harness.publisher.published.lock().unwrap(), vec![event_id]);
    let completed = harness.outbox.completed.lock().unwrap();
    assert!(completed.contains(&(event_id, "CACHE".to_owned())));
    assert!(completed.contains(&(event_id, "INTERNAL".to_owned())));
    assert!(completed.contains(&(event_id, "EXTERNAL".to_owned())));
}

#[tokio::test]
async fn internal_notification_listener_suppresses_blocked_muted_and_self_direct_notifications() {
    let harness = Harness::new();
    let notification = Arc::new(SpyNotificationClient::default());
    let actor_id = uuid("33333333-3333-3333-3333-333333333320");
    let blocked_id = uuid("33333333-3333-3333-3333-333333333321");
    let muted_id = uuid("33333333-3333-3333-3333-333333333322");
    harness.friend.policies.lock().unwrap().insert(
        (blocked_id, actor_id),
        post_service::domain::post::RelationshipPolicySnapshot {
            blocked_by_viewer: true,
            ..Default::default()
        },
    );
    harness.friend.policies.lock().unwrap().insert(
        (muted_id, actor_id),
        post_service::domain::post::RelationshipPolicySnapshot {
            muted_by_viewer: true,
            ..Default::default()
        },
    );
    let listener = InternalNotificationListener::new(
        harness.posts.clone(),
        harness.friend.clone(),
        notification.clone(),
    );

    for (index, recipient) in [blocked_id, muted_id, actor_id].into_iter().enumerate() {
        let event = stored_event(
            Uuid::from_u128(0x33333333333333333333333333333330 + index as u128),
            "POST_LIKED",
            serde_json::json!({
                "postId": "33333333-3333-3333-3333-333333333399",
                "notifyUserId": recipient,
            })
            .to_string(),
        );
        let event = post_service::domain::post::StoredEvent {
            actor_id: Some(actor_id),
            ..event
        };
        post_service::application::ports::InternalEventDispatcherPort::dispatch(&listener, &event)
            .await
            .expect("suppressed direct notification");
    }

    assert!(notification.notifications.lock().unwrap().is_empty());
}

#[tokio::test]
async fn internal_notification_listener_routes_like_and_comment_events_to_notify_user() {
    let harness = Harness::new();
    let notification = Arc::new(SpyNotificationClient::default());
    let actor_id = uuid("33333333-3333-3333-3333-333333333323");
    let recipient_id = uuid("33333333-3333-3333-3333-333333333324");
    let listener = InternalNotificationListener::new(
        harness.posts.clone(),
        harness.friend.clone(),
        notification.clone(),
    );

    for (index, event_type) in ["POST_LIKED", "POST_COMMENTED"].into_iter().enumerate() {
        let event = stored_event(
            Uuid::from_u128(0x33333333333333333333333333333340 + index as u128),
            event_type,
            serde_json::json!({
                "postId": "33333333-3333-3333-3333-333333333398",
                "notifyUserId": recipient_id,
            })
            .to_string(),
        );
        let event = post_service::domain::post::StoredEvent {
            actor_id: Some(actor_id),
            ..event
        };
        post_service::application::ports::InternalEventDispatcherPort::dispatch(&listener, &event)
            .await
            .expect("direct notification");
    }

    let notifications = notification.notifications.lock().unwrap();
    assert_eq!(notifications.len(), 2);
    assert_eq!(notifications[0].user_id, recipient_id);
    assert_eq!(notifications[0].notification_type, "POST_LIKED");
    assert_eq!(notifications[1].notification_type, "POST_COMMENTED");
}

#[tokio::test]
async fn internal_notification_listener_fans_out_post_created_to_followers() {
    let harness = Harness::new();
    let notification = Arc::new(SpyNotificationClient::default());
    let author_id = uuid("33333333-3333-3333-3333-333333333306");
    let follower_a = uuid("33333333-3333-3333-3333-333333333307");
    let follower_b = uuid("33333333-3333-3333-3333-333333333308");
    *harness.friend.follower_ids.lock().unwrap() = vec![follower_a, follower_b];
    let event = stored_event(
        uuid("33333333-3333-3333-3333-333333333309"),
        "POST_CREATED",
        serde_json::json!({ "authorId": author_id, "postId": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa" }).to_string(),
    );
    harness.posts.posts_by_id.lock().unwrap().insert(
        event.aggregate_id,
        post(event.aggregate_id, author_id, "public notification"),
    );
    let event = post_service::domain::post::StoredEvent {
        actor_id: Some(author_id),
        ..event
    };
    let listener = InternalNotificationListener::new(
        harness.posts.clone(),
        harness.friend.clone(),
        notification.clone(),
    );

    post_service::application::ports::InternalEventDispatcherPort::dispatch(&listener, &event)
        .await
        .expect("dispatch notification");

    let notifications = notification.notifications.lock().unwrap();
    assert_eq!(notifications.len(), 2);
    assert_eq!(notifications[0].user_id, follower_a);
    assert_eq!(notifications[1].user_id, follower_b);
    assert_eq!(notifications[0].notification_type, "POST_CREATED");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&notifications[0].payload_json).unwrap(),
        serde_json::json!({"payloadVersion": 1, "postId": event.aggregate_id})
    );
}

#[tokio::test]
async fn post_created_notifications_use_current_visibility_and_follow_policy() {
    use post_service::application::ports::InternalEventDispatcherPort;
    use post_service::domain::post::{RelationshipPolicySnapshot, StoredEvent};

    let harness = Harness::new();
    let notifications = Arc::new(SpyNotificationClient::default());
    let author = Uuid::new_v4();
    let follower = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    *harness.friend.follower_ids.lock().unwrap() = vec![follower];
    let listener = InternalNotificationListener::new(
        harness.posts.clone(),
        harness.friend.clone(),
        notifications.clone(),
    );
    // A legacy/public creation-time payload cannot grant access to an absent
    // resource or to its current PRIVATE/unsupported visibility.
    let event = StoredEvent {
        aggregate_id: post_id,
        actor_id: Some(author),
        attributes: vec!["PUBLIC".into()],
        ..stored_event(
            Uuid::new_v4(),
            "POST_CREATED",
            serde_json::json!({
                "postId": post_id, "authorId": author, "content": "private secret"
            })
            .to_string(),
        )
    };
    for visibility in [None, Some("PRIVATE"), Some("unsupported")] {
        if let Some(visibility) = visibility {
            let mut current = post(post_id, author, "private secret");
            current.visibility = visibility.into();
            harness
                .posts
                .posts_by_id
                .lock()
                .unwrap()
                .insert(post_id, current);
        }
        listener.dispatch(&event).await.expect("suppressed event");
    }
    assert!(notifications.notifications.lock().unwrap().is_empty());
    assert!(harness.friend.follower_calls.lock().unwrap().is_empty());

    let mut current = post(post_id, author, "friends only");
    current.visibility = "FRIENDS".into();
    harness
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(post_id, current);
    // A stale follower-list entry is not sufficient after unfollow.
    listener.dispatch(&event).await.expect("revoked follow");
    assert!(notifications.notifications.lock().unwrap().is_empty());
    harness.friend.policies.lock().unwrap().insert(
        (follower, author),
        RelationshipPolicySnapshot {
            following: true,
            ..Default::default()
        },
    );
    listener.dispatch(&event).await.expect("current follower");
    assert_eq!(notifications.notifications.lock().unwrap().len(), 1);
    for policy in [
        RelationshipPolicySnapshot {
            following: true,
            blocked_viewer: true,
            ..Default::default()
        },
        RelationshipPolicySnapshot {
            following: true,
            muted_by_viewer: true,
            ..Default::default()
        },
    ] {
        harness
            .friend
            .policies
            .lock()
            .unwrap()
            .insert((follower, author), policy);
        listener
            .dispatch(&event)
            .await
            .expect("blocked/muted follower");
    }
    assert_eq!(notifications.notifications.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn post_created_notification_rejects_payload_identity_mismatch() {
    use post_service::application::ports::InternalEventDispatcherPort;
    use post_service::domain::post::StoredEvent;

    let harness = Harness::new();
    let notifications = Arc::new(SpyNotificationClient::default());
    let author = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    harness
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(post_id, post(post_id, author, "public"));
    let listener = InternalNotificationListener::new(
        harness.posts.clone(),
        harness.friend.clone(),
        notifications.clone(),
    );
    for (payload_post, payload_author, actor) in [
        (Uuid::new_v4(), author, Some(author)),
        (post_id, Uuid::new_v4(), Some(author)),
        (post_id, author, Some(Uuid::new_v4())),
        (post_id, author, None),
    ] {
        let event = StoredEvent {
            aggregate_id: post_id,
            actor_id: actor,
            ..stored_event(
                Uuid::new_v4(),
                "POST_CREATED",
                serde_json::json!({
                    "postId": payload_post, "authorId": payload_author
                })
                .to_string(),
            )
        };
        assert!(listener.dispatch(&event).await.is_err());
    }
    assert!(harness.friend.follower_calls.lock().unwrap().is_empty());
    assert!(notifications.notifications.lock().unwrap().is_empty());
}

#[tokio::test]
async fn outbox_worker_tick_reports_forwarded_count_and_metrics() {
    let harness = Harness::new();
    let metrics = Arc::new(PostMetrics::default());
    let event_id = uuid("33333333-3333-3333-3333-333333333310");
    let post_id = uuid("33333333-3333-3333-3333-333333333311");
    *harness.outbox.unpublished.lock().unwrap() = vec![stored_event(
        event_id,
        "POST_LIKED",
        serde_json::json!({ "postId": post_id }).to_string(),
    )];
    let worker = PostOutboxWorker::new(forwarder(&harness), metrics.clone());

    let forwarded = worker
        .tick(instant(1_700_000_300), 25)
        .await
        .expect("outbox tick");

    assert_eq!(forwarded, 1);
    assert_eq!(harness.outbox.target_claim_calls.lock().unwrap()[0].2, 1);
    let rendered = metrics.render_prometheus(&OutboxMetricsSnapshot::default());
    assert!(rendered.contains("post_outbox_worker_runs_total{outcome=\"success\"} 1\n"));
    assert!(rendered.contains("post_outbox_worker_forwarded_events_total{outcome=\"success\"} 1\n"));
}

#[tokio::test]
async fn outbox_worker_tick_records_failure_metrics_when_outbox_lookup_fails() {
    let harness = Harness::new();
    *harness.outbox.claim_error.lock().unwrap() = Some("outbox unavailable".into());
    let metrics = Arc::new(PostMetrics::default());
    let worker = PostOutboxWorker::new(forwarder(&harness), metrics.clone());

    let error = worker
        .tick(instant(1_700_000_300), 25)
        .await
        .expect_err("outbox lookup must fail");

    assert!(error.to_string().contains("outbox unavailable"));
    assert_eq!(harness.outbox.target_claim_calls.lock().unwrap().len(), 1);
    let rendered = metrics.render_prometheus(&OutboxMetricsSnapshot::default());
    assert!(rendered.contains("post_outbox_worker_runs_total{outcome=\"failure\"} 1\n"));
    assert!(rendered.contains("post_outbox_worker_forwarded_events_total{outcome=\"failure\"} 0\n"));
}

#[tokio::test]
async fn retention_worker_tick_builds_worker_request_and_records_split_metrics() {
    let harness = Harness::new();
    let actor_id = uuid(ACTOR_ID);
    let post_id = uuid("33333333-3333-3333-3333-333333333312");
    harness.posts.insert_detail(PostDetail {
        post: post(post_id, actor_id, "purge"),
        comments: Vec::new(),
    });
    *harness.posts.purge_result.lock().unwrap() = RetentionPurgeResult {
        post_ids: vec![post_id],
        comment_ids: vec![uuid("33333333-3333-3333-3333-333333333313")],
    };
    let metrics = Arc::new(PostMetrics::default());
    let worker = PostRetentionPurgeWorker::new(
        PurgeExpiredPostsUseCase::new(harness.service.clone()),
        metrics.clone(),
        ACTOR_ID.to_owned(),
    );

    let purged = worker.tick().await.expect("retention tick");

    assert_eq!(purged, 2);
    let calls = harness.posts.purge_calls.lock().unwrap();
    assert_eq!(calls[0].actor_id, actor_id);
    assert_eq!(
        calls[0].request_id.as_deref(),
        Some("post-retention-worker")
    );
    assert_eq!(
        calls[0].user_agent.as_deref(),
        Some("post-retention-worker")
    );
    assert!(!calls[0].dry_run);
    let rendered = metrics.render_prometheus(&OutboxMetricsSnapshot::default());
    assert!(rendered.contains("post_retention_purge_runs_total{outcome=\"success\"} 1\n"));
    assert!(rendered.contains("post_retention_purged_posts_total{outcome=\"success\"} 1\n"));
    assert!(rendered.contains("post_retention_purged_comments_total{outcome=\"success\"} 1\n"));
}

fn forwarder(harness: &Harness) -> ExternalEventForwarder {
    ExternalEventForwarder::new(
        harness.outbox.clone(),
        harness.publisher.clone(),
        harness.internal.clone(),
        harness.feed_cache.clone(),
        harness.post_cache.clone(),
        harness.friend.clone(),
        Arc::new(super::support::RecordingTimelineProjection::default()),
    )
    .with_delivery_metrics(harness.metrics.clone())
}

#[tokio::test]
async fn notification_batch_authorization_checks_current_visibility_and_complete_relationships() {
    use post_service::application::dto::post_dto::AuthorizeNotificationPostsDto;
    use post_service::application::use_cases::authorize_notification_posts::AuthorizeNotificationPostsUseCase;
    use post_service::domain::post::RelationshipPolicySnapshot;
    let harness = Harness::new();
    let viewer = Uuid::new_v4();
    let other = Uuid::new_v4();
    let public_id = Uuid::new_v4();
    let friends_id = Uuid::new_v4();
    let private_id = Uuid::new_v4();
    let own_id = Uuid::new_v4();
    let absent = Uuid::new_v4();
    for (id, author, visibility) in [
        (public_id, other, "PUBLIC"),
        (friends_id, other, "FRIENDS"),
        (private_id, other, "PRIVATE"),
        (own_id, viewer, "PRIVATE"),
    ] {
        let mut value = post(id, author, "must not be returned");
        value.visibility = visibility.into();
        harness.posts.posts_by_id.lock().unwrap().insert(id, value);
    }
    let use_case = AuthorizeNotificationPostsUseCase::new(harness.service.clone());
    for (policy, expected) in [
        (
            RelationshipPolicySnapshot::default(),
            vec![public_id, own_id],
        ),
        (
            RelationshipPolicySnapshot {
                following: true,
                ..Default::default()
            },
            vec![public_id, friends_id, own_id],
        ),
        (
            RelationshipPolicySnapshot {
                following: true,
                blocked_by_viewer: true,
                ..Default::default()
            },
            vec![own_id],
        ),
        (
            RelationshipPolicySnapshot {
                following: true,
                blocked_viewer: true,
                ..Default::default()
            },
            vec![own_id],
        ),
        (
            RelationshipPolicySnapshot {
                following: true,
                muted_by_viewer: true,
                ..Default::default()
            },
            vec![own_id],
        ),
    ] {
        harness
            .friend
            .policies
            .lock()
            .unwrap()
            .insert((viewer, other), policy);
        let result = use_case
            .execute(AuthorizeNotificationPostsDto {
                viewer_id: viewer,
                post_ids: vec![public_id, friends_id, private_id, own_id, absent],
            })
            .await
            .unwrap();
        assert_eq!(result.visible_post_ids, expected);
    }
    *harness.friend.omit_batch_policy_for.lock().unwrap() = Some(other);
    assert!(use_case
        .execute(AuthorizeNotificationPostsDto {
            viewer_id: viewer,
            post_ids: vec![public_id]
        })
        .await
        .is_err());
    let before = harness.posts.access_queries.lock().unwrap().len();
    assert!(use_case
        .execute(AuthorizeNotificationPostsDto {
            viewer_id: viewer,
            post_ids: vec![public_id; 101]
        })
        .await
        .is_err());
    assert_eq!(harness.posts.access_queries.lock().unwrap().len(), before);
}
