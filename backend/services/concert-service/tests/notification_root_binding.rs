//! NB01 real PostgreSQL lookup mechanics, not canonical-owner/public acceptance.
#[path = "notification_root_binding_support/mod.rs"]
mod support;
use concert_service as binding_test_types;
use concert_service::{
    adapters::out::relationship_authority::guarded::{proto, wire_binding_digest, ScopeRegistry},
    notification_live::NotificationRecoveryBinding,
    product_rpc::scope_ledger::ScopeKey,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use support::*;
use tonic::Code;
use uuid::Uuid;

#[tokio::test]
async fn expired_original_survives_new_ledger_and_empty_registry_for_all_four_operations() {
    let pool = pool().await;
    for (operation, method, suffix) in [
        (
            "listConcertNotifications",
            "GET",
            "?limit=7&unreadOnly=true".to_string(),
        ),
        (
            "getConcertNotificationUnreadCount",
            "GET",
            "/unread-count".to_string(),
        ),
        (
            "markConcertNotification",
            "PATCH",
            format!("/{}/read", Uuid::new_v4()),
        ),
        (
            "markAllConcertNotifications",
            "PATCH",
            "/read-all".to_string(),
        ),
    ] {
        let mut b = binding();
        b.operation = operation.into();
        b.http_method = method.into();
        b.http_target = format!(
            "/api/concert/events/{}/me/notifications{suffix}",
            b.event_id
        );
        let q = query(&b);
        let instance = Uuid::new_v4();
        store(
            &pool,
            instance,
            q.binding_digest,
            serde_json::to_value(&b).unwrap(),
            "ROOT_FINISHED",
        )
        .await;
        let original = ledger(&pool, instance);
        drop(original);
        let restarted = ledger(&pool, instance);
        let _empty_registry = ScopeRegistry::new(instance, Arc::new(restarted.clone())).unwrap();
        let restored = restarted.load_binding(&q).await.unwrap();
        assert!(b.expires_unix_micros < chrono::Utc::now().timestamp_micros());
        assert_eq!(
            restored,
            proto::GuardBinding {
                actor_id: b.actor_id.to_string(),
                event_id: b.event_id.to_string(),
                session_family_id: b.session_family_id.to_string(),
                request_id: b.request_id.to_string(),
                operation: b.operation,
                subject_ids: vec![],
                expires_unix_micros: b.expires_unix_micros,
                parent_digest: b.parent_digest.to_vec(),
                parent_body_digest: b.parent_body_digest.to_vec(),
                parent_method: b.parent_method,
                http_method: b.http_method,
                http_target: b.http_target,
                body_digest: b.body_digest.to_vec(),
            }
        );
        assert_eq!(wire_binding_digest(&restored).unwrap(), q.binding_digest);
    }
    pool.close().await;
}

#[tokio::test]
async fn actual_lifecycle_is_unavailable_until_durable_root_finished() {
    let pool = pool().await;
    let ledger = ledger(&pool, Uuid::new_v4());
    let mut b = binding();
    b.expires_unix_micros = chrono::Utc::now().timestamp_micros() + 300_000_000;
    let q = query(&b);
    let key = ScopeKey {
        instance: ledger.instance(),
        scope: Uuid::new_v4(),
        binding_digest: q.binding_digest,
    };
    ledger.register(&key, &b).await.unwrap();
    assert_eq!(
        ledger.load_binding(&q).await.unwrap_err().code(),
        Code::Unavailable
    );
    ledger.seal(&key).await.unwrap();
    assert_eq!(
        ledger.load_binding(&q).await.unwrap_err().code(),
        Code::Unavailable
    );
    ledger.finish_root_work(&key).await.unwrap();
    assert_eq!(
        ledger.load_binding(&q).await.unwrap().expires_unix_micros,
        b.expires_unix_micros
    );
    pool.close().await;
}

#[tokio::test]
async fn missing_foreign_instance_and_ambiguous_rows_are_unavailable() {
    let pool = pool().await;
    let b = binding();
    let q = query(&b);
    let instance = Uuid::new_v4();
    let provider = ledger(&pool, instance);
    assert_eq!(
        provider.load_binding(&q).await.unwrap_err().code(),
        Code::Unavailable
    );
    store(
        &pool,
        Uuid::new_v4(),
        q.binding_digest,
        serde_json::to_value(&b).unwrap(),
        "ROOT_FINISHED",
    )
    .await;
    assert_eq!(
        provider.load_binding(&q).await.unwrap_err().code(),
        Code::Unavailable
    );
    for second_phase in ["ACTIVE", "SEALED", "ROOT_FINISHED"] {
        let instance = Uuid::new_v4();
        let provider = ledger(&pool, instance);
        store(
            &pool,
            instance,
            q.binding_digest,
            serde_json::to_value(&b).unwrap(),
            "ROOT_FINISHED",
        )
        .await;
        store(
            &pool,
            instance,
            q.binding_digest,
            serde_json::to_value(&b).unwrap(),
            second_phase,
        )
        .await;
        assert_eq!(
            provider.load_binding(&q).await.unwrap_err().code(),
            Code::Unavailable
        );
    }
    pool.close().await;
}

#[tokio::test]
async fn every_child_query_identity_expiry_digest_and_own_subject_must_match() {
    let pool = pool().await;
    let b = binding();
    let q = query(&b);
    let instance = Uuid::new_v4();
    store(
        &pool,
        instance,
        q.binding_digest,
        serde_json::to_value(&b).unwrap(),
        "ROOT_FINISHED",
    )
    .await;
    let provider = ledger(&pool, instance);
    let mut variants = Vec::new();
    for field in 0..9 {
        let mut v = q.clone();
        match field {
            0 => {
                v.actor = Uuid::new_v4();
                v.subject = v.actor;
            }
            1 => v.event = Uuid::new_v4(),
            2 => v.family = Uuid::new_v4(),
            3 => v.request = Uuid::new_v4(),
            4 => v.subject = Uuid::new_v4(),
            5 => v.expires_unix_micros += 1,
            6 => v.binding_digest[0] ^= 1,
            7 => v.actor = Uuid::nil(),
            _ => v.binding_digest = [0; 32],
        }
        variants.push(v);
    }
    for variant in variants {
        assert_eq!(
            provider.load_binding(&variant).await.unwrap_err().code(),
            Code::Unavailable
        );
    }
    assert!(provider.load_binding(&q).await.is_ok());
    pool.close().await;
}

#[tokio::test]
async fn malformed_or_fingerprint_mismatched_original_fields_are_unavailable() {
    let pool = pool().await;
    let b = binding();
    let q = query(&b);
    let value = serde_json::to_value(&b).unwrap();
    let mut variants = vec![serde_json::json!({})];
    for (field, changed) in [
        (
            "operation",
            serde_json::json!("markAllConcertNotifications"),
        ),
        ("http_method", serde_json::json!("PATCH")),
        (
            "http_target",
            serde_json::json!(format!(
                "/api/concert/events/{}/me/notifications?limit=8",
                b.event_id
            )),
        ),
        (
            "parent_method",
            serde_json::json!("/concert.participant.v1.ConcertParticipantService/GetParticipant"),
        ),
        ("parent_digest", serde_json::json!([44; 32].to_vec())),
        ("parent_body_digest", serde_json::json!([55; 32].to_vec())),
        ("body_digest", serde_json::json!([66; 32].to_vec())),
        (
            "expires_unix_micros",
            serde_json::json!(b.expires_unix_micros + 1),
        ),
        ("unknown_field", serde_json::json!(true)),
    ] {
        let mut v = value.clone();
        v[field] = changed;
        variants.push(v);
    }
    for v in variants {
        let instance = Uuid::new_v4();
        store(&pool, instance, q.binding_digest, v, "ROOT_FINISHED").await;
        assert_eq!(
            ledger(&pool, instance)
                .load_binding(&q)
                .await
                .unwrap_err()
                .code(),
            Code::Unavailable
        );
    }
    pool.close().await;
}

#[tokio::test]
async fn valid_digest_cannot_relabel_a_non_notification_operation_or_path() {
    let pool = pool().await;
    for variant in 0..6 {
        let mut b = binding();
        match variant {
            0 => b.operation = "getConcertPublicProfile".into(),
            1 => b.http_method = "POST".into(),
            2 => {
                b.http_target = format!("/api/concert/events/{}/me/notifications/wrong", b.event_id)
            }
            3 => {
                b.parent_method =
                    "/concert.participant.v1.ConcertParticipantService/GetParticipant".into()
            }
            4 | 5 => {
                b.operation = "markConcertNotification".into();
                b.http_method = "PATCH".into();
                let id = if variant == 4 {
                    Uuid::nil().to_string()
                } else {
                    Uuid::new_v4().simple().to_string()
                };
                b.http_target = format!(
                    "/api/concert/events/{}/me/notifications/{id}/read",
                    b.event_id
                );
            }
            _ => unreachable!(),
        }
        let q = query(&b);
        let instance = Uuid::new_v4();
        store(
            &pool,
            instance,
            q.binding_digest,
            serde_json::to_value(&b).unwrap(),
            "ROOT_FINISHED",
        )
        .await;
        assert_eq!(
            ledger(&pool, instance)
                .load_binding(&q)
                .await
                .unwrap_err()
                .code(),
            Code::Unavailable
        );
    }
    pool.close().await;
}

#[tokio::test]
async fn caller_deadline_pool_starvation_and_locked_table_are_bounded_unavailable() {
    let pool = pool().await;
    let b = binding();
    let q = query(&b);
    let instance = Uuid::new_v4();
    store(
        &pool,
        instance,
        q.binding_digest,
        serde_json::to_value(&b).unwrap(),
        "ROOT_FINISHED",
    )
    .await;
    let provider = ledger(&pool, instance);
    let start = Instant::now();
    let result =
        platform::call_deadline::scope(start - Duration::from_millis(1), provider.load_binding(&q))
            .await;
    assert_eq!(result.unwrap_err().code(), Code::Unavailable);
    let one = pool.acquire().await.unwrap();
    let two = pool.acquire().await.unwrap();
    let start = Instant::now();
    let result = platform::call_deadline::scope(
        start + Duration::from_millis(50),
        provider.load_binding(&q),
    )
    .await;
    assert_eq!(result.unwrap_err().code(), Code::Unavailable);
    assert!(start.elapsed() < Duration::from_secs(2));
    drop(one);
    drop(two);
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE public.concert_product_owner_scopes IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let start = Instant::now();
    assert_eq!(
        provider.load_binding(&q).await.unwrap_err().code(),
        Code::Unavailable
    );
    assert!(start.elapsed() < Duration::from_secs(2));
    lock.rollback().await.unwrap();
    assert!(provider.load_binding(&q).await.is_ok());
    pool.close().await;
}
