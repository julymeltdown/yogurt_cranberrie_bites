//! Pure source/intent regressions; no network or persistence acceptance claims.
use chrono::Utc;
use post_application::{
    delivery_authority::*, dto::delivery_schedule_dto::TargetDeliveryClaimDto,
    notification_authority::*,
};
use post_domain::post::{DeliveryAttemptSource, DeliveryTarget, StoredEvent};
use serde_json::json;
use uuid::Uuid;

fn event(kind: &str) -> StoredEvent {
    let actor = Uuid::from_u128(1);
    let recipient = Uuid::from_u128(2);
    let post = Uuid::from_u128(3);
    let payload = match kind {
        "POST_CREATED" => json!({"postId":post,"authorId":actor,"content":"private text"}),
        "POST_COMMENTED" => {
            json!({"postId":post,"authorId":actor,"notifyUserId":recipient,"content":"private comment"})
        }
        "POST_LIKED" => {
            json!({"postId":post,"actorId":actor,"authorId":recipient,"notifyUserId":recipient})
        }
        _ => json!({"postId":post,"notifyUserId":recipient}),
    };
    StoredEvent {
        event_id: Uuid::new_v4(),
        event_type: kind.into(),
        aggregate_id: post,
        aggregate_version: Some(1),
        actor_id: Some(actor),
        attributes: vec!["one".into(), "two".into()],
        payload: payload.to_string(),
        occurred_at: Utc::now(),
        created_at: Utc::now(),
        published: false,
        published_at: None,
        replay_resolution_id: None,
        origin_traceparent: Some("retained trace".into()),
    }
}
fn owner(
    e: StoredEvent,
    kind: DeliveryLeaseKindDto,
    count: i32,
    required: bool,
) -> OwnedDeliveryClaimDto {
    OwnedDeliveryClaimDto::from_persisted_claim(
        TargetDeliveryClaimDto {
            event: e,
            target: DeliveryTarget::Internal,
            lease_token: Uuid::new_v4(),
            attempt_id: Uuid::new_v4(),
            attempt_number: count,
            lease_expires_at: Utc::now() + chrono::Duration::minutes(1),
        },
        DeliveryAttemptProvenanceDto {
            lease_kind: kind,
            source: if kind == DeliveryLeaseKindDto::Automatic {
                DeliveryAttemptSource::OutboxWorker
            } else {
                DeliveryAttemptSource::AdminReplay
            },
            resolution_id: None,
            max_attempts: 3,
            progress_credits: 0,
            target_required: required,
        },
        Utc::now(),
    )
    .unwrap()
}
fn reference(e: &StoredEvent, scope: NotificationScopeDto) -> NotificationOwnerReferenceDto {
    NotificationOwnerReferenceDto {
        event_id: e.event_id,
        aggregate_id: e.aggregate_id,
        event_digest: [1; 32],
        attempt_digest: [2; 32],
        lease_token: Uuid::new_v4(),
        attempt_id: Uuid::new_v4(),
        attempt_number: 1,
        lease_expires_at: Utc::now() + chrono::Duration::minutes(1),
        scope,
    }
}
fn recipient() -> NotificationScopeDto {
    NotificationScopeDto::RecipientPolicy {
        recipient_id: Uuid::from_u128(2),
    }
}

#[test]
fn producer_specific_notifications_keep_original_identity_and_reference_only_payload() {
    for kind in ["POST_CREATED", "POST_LIKED", "POST_COMMENTED", "COMMENT"] {
        let e = event(kind);
        let intent = NotificationIntentDto::from_event(&e).unwrap().unwrap();
        let n = intent.notification_for(Uuid::from_u128(2)).unwrap();
        assert_eq!(n.event_id, e.event_id);
        assert_eq!(n.actor_id, e.actor_id);
        assert_eq!(n.notification_type, kind);
        assert_eq!(n.occurred_at, e.occurred_at.to_rfc3339());
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&n.payload_json).unwrap(),
            json!({"payloadVersion":1,"postId":e.aggregate_id})
        );
        assert!(intent.notification_for(Uuid::nil()).is_err());
        assert!(intent.notification_for(e.actor_id.unwrap()).is_err());
        if kind != "POST_CREATED" {
            assert!(intent.notification_for(Uuid::new_v4()).is_err());
        }
    }
}
#[test]
fn minimal_legacy_direct_shapes_and_comment_aggregate_are_not_reinterpreted() {
    for kind in ["POST_LIKED", "POST_COMMENTED", "COMMENT"] {
        let mut e = event(kind);
        e.payload = json!({"postId":e.aggregate_id,"notifyUserId":Uuid::from_u128(2)}).to_string();
        if kind == "COMMENT" {
            e.aggregate_id = Uuid::new_v4();
        }
        let intent = NotificationIntentDto::from_event(&e).unwrap().unwrap();
        assert_eq!(intent.post_id(), Uuid::from_u128(3));
        assert!(intent.validate_scope(&recipient()).is_ok());
    }
}
#[test]
fn unknown_types_are_noop_without_parsing_or_broadening_post_authority() {
    for kind in [
        "POST_UNLIKED",
        "POST_DELETED",
        "POST_COMMENT_DELETED",
        "FRIEND_REQUEST",
        "SYSTEM",
        "FUTURE",
    ] {
        let mut e = event(kind);
        e.payload = "malformed".into();
        e.actor_id = None;
        assert!(NotificationIntentDto::from_event(&e).unwrap().is_none());
    }
}
#[test]
fn conflicting_producer_identity_and_malformed_known_payloads_deny() {
    for kind in ["POST_CREATED", "POST_LIKED", "POST_COMMENTED", "COMMENT"] {
        let e = event(kind);
        for case in 0..5 {
            let mut wrong = e.clone();
            let mut payload: serde_json::Value = serde_json::from_str(&e.payload).unwrap();
            match case {
                0 => wrong.actor_id = None,
                1 => payload["postId"] = json!(Uuid::nil()),
                2 => payload["actorId"] = json!(Uuid::new_v4()),
                3 => wrong.payload = "malformed".into(),
                _ => wrong.event_id = Uuid::nil(),
            }
            if case != 3 {
                wrong.payload = payload.to_string();
            }
            assert!(
                NotificationIntentDto::from_event(&wrong).is_err(),
                "{kind}/{case}"
            );
        }
        if kind != "COMMENT" {
            let mut wrong = e;
            let mut p: serde_json::Value = serde_json::from_str(&wrong.payload).unwrap();
            p["authorId"] = json!(Uuid::new_v4());
            wrong.payload = p.to_string();
            assert!(NotificationIntentDto::from_event(&wrong).is_err());
        }
    }
}
#[test]
fn follower_and_recipient_lookup_scopes_remain_distinct_and_exact() {
    let created = NotificationIntentDto::from_event(&event("POST_CREATED"))
        .unwrap()
        .unwrap();
    let page = NotificationScopeDto::FollowerPage {
        author_id: created.actor_id(),
        cursor: None,
        limit: 2,
    };
    assert!(created.validate_scope(&page).is_ok());
    assert!(created.validate_scope(&recipient()).is_ok());
    let direct = NotificationIntentDto::from_event(&event("POST_COMMENTED"))
        .unwrap()
        .unwrap();
    assert!(direct.validate_scope(&page).is_err());
    assert!(direct.validate_scope(&recipient()).is_ok());
    for bad in [
        NotificationScopeDto::FollowerPage {
            author_id: Uuid::new_v4(),
            cursor: None,
            limit: 2,
        },
        NotificationScopeDto::RecipientPolicy {
            recipient_id: created.actor_id(),
        },
    ] {
        assert!(created.validate_scope(&bad).is_err());
    }
    for cursor in [
        "",
        " bad",
        "2026-10-14T00:00:00Z|00000000-0000-0000-0000-000000000000",
        "not-a-time|00000000-0000-0000-0000-000000000001",
    ] {
        assert!(NotificationScopeDto::FollowerPage {
            author_id: created.actor_id(),
            cursor: Some(cursor.into()),
            limit: 2
        }
        .validate()
        .is_err());
    }
}
#[test]
fn every_replay_restore_state_and_zero_or_exhausted_completed_budget_are_distinct() {
    let e = event("COMMENT");
    let mut identities = std::collections::HashSet::new();
    for (kind, count, required) in [
        (DeliveryLeaseKindDto::Automatic, 1, true),
        (
            DeliveryLeaseKindDto::Replay(DeliveryReplayRestoreDto::Pending),
            1,
            true,
        ),
        (
            DeliveryLeaseKindDto::Replay(DeliveryReplayRestoreDto::Retry),
            3,
            true,
        ),
        (
            DeliveryLeaseKindDto::Replay(DeliveryReplayRestoreDto::Succeeded),
            0,
            true,
        ),
        (
            DeliveryLeaseKindDto::Replay(DeliveryReplayRestoreDto::Skipped),
            0,
            false,
        ),
        (
            DeliveryLeaseKindDto::Replay(DeliveryReplayRestoreDto::Succeeded),
            4,
            true,
        ),
    ] {
        let o = owner(e.clone(), kind, count, required);
        let b = notification_attempt_identity(&o).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&b).unwrap()["target"],
            "INTERNAL"
        );
        // Attempt count is separately bound by the exact claim reference.
        if count != 4 {
            assert!(identities.insert(b));
        }
    }
    assert_eq!(identities.len(), 5);
}
#[test]
fn frozen_provenance_changes_identity_and_sibling_target_is_denied() {
    let original = owner(
        event("POST_CREATED"),
        DeliveryLeaseKindDto::Automatic,
        1,
        true,
    );
    let bytes = notification_attempt_identity(&original).unwrap();
    for case in 0..3 {
        let mut p = original.provenance();
        match case {
            0 => p.max_attempts = 4,
            1 => {
                p.source = DeliveryAttemptSource::AdminReplay;
                p.resolution_id = Some(Uuid::new_v4());
            }
            _ => {
                p.lease_kind = DeliveryLeaseKindDto::Replay(DeliveryReplayRestoreDto::Pending);
                p.source = DeliveryAttemptSource::AdminReplay;
            }
        }
        let o = OwnedDeliveryClaimDto::from_persisted_claim(
            original.lookup().claim().clone(),
            p,
            Utc::now(),
        )
        .unwrap();
        assert_ne!(notification_attempt_identity(&o).unwrap(), bytes);
    }
    let mut sibling = original.lookup().claim().clone();
    sibling.target = DeliveryTarget::Cache;
    let o = OwnedDeliveryClaimDto::from_persisted_claim(sibling, original.provenance(), Utc::now())
        .unwrap();
    assert!(notification_attempt_identity(&o).is_err());
}
#[test]
fn immutable_event_identity_includes_trace_order_and_payload_bytes_but_not_publication() {
    let e = event("POST_CREATED");
    let identity = notification_event_identity(&e).unwrap();
    for case in 0..5 {
        let mut wrong = e.clone();
        match case {
            0 => wrong.payload.push(' '),
            1 => wrong.attributes.reverse(),
            2 => wrong.origin_traceparent = None,
            3 => wrong.occurred_at += chrono::Duration::microseconds(1),
            _ => wrong.actor_id = Some(Uuid::new_v4()),
        }
        assert_ne!(notification_event_identity(&wrong).unwrap(), identity);
    }
    let mut mutable = e;
    mutable.published = true;
    mutable.published_at = Some(Utc::now());
    mutable.replay_resolution_id = Some(Uuid::new_v4());
    assert_eq!(notification_event_identity(&mutable).unwrap(), identity);
}
#[test]
fn created_visibility_is_current_explicit_and_direct_intent_survives_absent_post() {
    let e = event("POST_CREATED");
    let r = reference(&e, recipient());
    assert!(
        OwnedNotificationScopeDto::from_persisted(r.clone(), &e, None, Utc::now())
            .unwrap()
            .is_none()
    );
    for v in ["PUBLIC", "FRIENDS", "PRIVATE", "UNKNOWN"] {
        let p = NotificationPostAccessDto {
            post_id: e.aggregate_id,
            author_id: e.actor_id.unwrap(),
            visibility: v.into(),
        };
        let observed =
            OwnedNotificationScopeDto::from_persisted(r.clone(), &e, Some(&p), Utc::now()).unwrap();
        if v == "PUBLIC" || v == "FRIENDS" {
            let o = observed.unwrap();
            assert_eq!(o.requires_following(), v == "FRIENDS");
            assert!(o.visibility().is_some());
        } else {
            assert!(observed.is_none());
        }
    }
    let direct = event("POST_COMMENTED");
    let o = OwnedNotificationScopeDto::from_persisted(
        reference(&direct, recipient()),
        &direct,
        None,
        Utc::now(),
    )
    .unwrap()
    .unwrap();
    assert!(!o.requires_following());
    assert!(o.visibility().is_none());
}
#[test]
fn owner_observation_rejects_substitution_expiry_and_conflicting_current_author() {
    let e = event("POST_CREATED");
    let r = reference(&e, recipient());
    let mut p = NotificationPostAccessDto {
        post_id: e.aggregate_id,
        author_id: Uuid::new_v4(),
        visibility: "PUBLIC".into(),
    };
    assert!(
        OwnedNotificationScopeDto::from_persisted(r.clone(), &e, Some(&p), Utc::now()).is_err()
    );
    p.author_id = e.actor_id.unwrap();
    p.post_id = Uuid::new_v4();
    assert!(
        OwnedNotificationScopeDto::from_persisted(r.clone(), &e, Some(&p), Utc::now()).is_err()
    );
    assert!(
        OwnedNotificationScopeDto::from_persisted(r.clone(), &e, None, r.lease_expires_at).is_err()
    );
    let mut wrong = r;
    wrong.event_id = Uuid::new_v4();
    assert!(OwnedNotificationScopeDto::from_persisted(wrong, &e, None, Utc::now()).is_err());
    let mut inactive = e;
    inactive.payload = "malformed".into();
    assert!(OwnedNotificationScopeDto::from_persisted(
        reference(&inactive, recipient()),
        &inactive,
        None,
        Utc::now()
    )
    .unwrap()
    .is_none());
}
