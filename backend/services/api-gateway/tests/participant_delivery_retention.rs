//! CR10 regressions; controlled peers here do not certify native owner settlement.
include!("participant_delivery_retention_support/component.rs");

#[tokio::test]
async fn validated_expired_reply_retains_delivery_until_closed_error_eof() {
    for mode in [5, 6] {
        let h = Box::pin(Fixture::new()).await;
        h.seen.mode.store(mode, Ordering::SeqCst);
        let response = h.router.clone().oneshot(h.request("GET", "", "")).await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(h.seen.completions.load(Ordering::SeqCst), 0);
        let mut body = response.into_body();
        let data = body.frame().await.unwrap().unwrap().into_data().unwrap();
        let value: serde_json::Value = serde_json::from_slice(&data).unwrap();
        assert_eq!(value["details"]["reason"], "DEPENDENCY_UNAVAILABLE");
        for hidden in ["Mira", identity::USER, EVENT, "completion_secret", "binding_digest"] {
            assert!(!String::from_utf8_lossy(&data).contains(hidden));
        }
        assert_eq!(h.seen.completions.load(Ordering::SeqCst), 0, "data is not EOF");
        assert!(body.frame().await.is_none(), "closed safe error must reach actual EOF");
        assert_eq!(h.seen.completions.load(Ordering::SeqCst), 1, "validated capability lost after gRPC");
        h.close().await;
    }
}

#[tokio::test]
async fn closed_error_drop_partial_and_invalid_reply_never_complete() {
    for (mode, partial) in [(5,false),(5,true),(6,false),(6,true),(7,false),(8,false),(9,false),(4,false)] {
        let h = Box::pin(Fixture::new()).await;
        h.seen.mode.store(mode, Ordering::SeqCst);
        let response = h.router.clone().oneshot(h.request("GET", "", "")).await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let mut body = response.into_body();
        if mode >= 7 || mode == 4 {
            // Unknown/mismatched replies never acquire a retained capability,
            // even if the public closed error is fully consumed.
            let _ = to_bytes(body, 16384).await.unwrap();
        } else {
            if partial { assert!(body.frame().await.unwrap().unwrap().is_data()); }
            drop(body);
        }
        tokio::task::yield_now().await;
        assert_eq!(h.seen.completions.load(Ordering::SeqCst), 0, "mode={mode} partial={partial}");
        h.close().await;
    }
}
