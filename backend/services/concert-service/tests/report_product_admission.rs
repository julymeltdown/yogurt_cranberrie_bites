//! Signed admission checks only; the recording owner grants no authority.
#[path = "product_feature_admission_support/mod.rs"]
mod support;
use concert_service::product_rpc::{operation, Operation, ReportOperation};
use serde_json::json;
use support::*;
use tonic::Code;
use uuid::Uuid;

fn body() -> String {
    json!({"target_type":"USER","target_id":Uuid::new_v4(),"reason":"SPAM","text":"reporter text","evidence_media_ids":[]}).to_string()
}
async fn refused(request: concert_service::product_rpc::proto::ProductRequest) {
    let owner = Recorder::new(Mode::Refuse);
    assert_ne!(
        call(server(owner.clone()), wire(&request)).await.0,
        Code::Ok
    );
    assert_eq!(owner.len(), 0, "invalid or unsupported input reached owner");
}
#[tokio::test]
async fn original_frozen_report_reaches_owner_with_original_signed_frame() {
    let request = input("POST", "reports", &body());
    let owner = Recorder::new(Mode::Refuse);
    assert_eq!(
        operation(
            request.event_id.parse().unwrap(),
            "POST",
            &request.http_target
        )
        .unwrap(),
        Operation::Report(ReportOperation::Create)
    );
    assert_eq!(ReportOperation::Create.id(), "createConcertReport");
    assert!(ReportOperation::Create.mutation());
    let contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../contracts/domain/v1/operations.json"
    ))
    .unwrap();
    let frozen = contract
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["operationId"] == "createConcertReport")
        .unwrap();
    assert_eq!(
        frozen["path"]
            .as_str()
            .unwrap()
            .replace("{eventId}", &request.event_id),
        request.http_target
    );
    assert_eq!(
        frozen["method"].as_str().unwrap().to_uppercase(),
        request.http_method
    );
    assert_eq!(frozen["idempotency_key"], true);
    assert_eq!(
        call(server(owner.clone()), wire(&request)).await,
        (Code::Unavailable, REFUSAL.into())
    );
    let seen = owner.admitted.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(
        seen[0].operation,
        Operation::Report(ReportOperation::Create)
    );
    assert_eq!(seen[0].request, request);
}
#[tokio::test]
async fn report_route_is_exact_and_does_not_enable_privileged_or_read_operations() {
    let original = input("POST", "reports", &body());
    for suffix in [
        "reports?",
        "reports?target_type=USER",
        "reports/",
        "reports#x",
        "%72eports",
        "reports/../reports",
        "admin/reports",
        "admin/settings",
        "notices",
        "me/reports",
    ] {
        let mut request = original.clone();
        request.http_target = format!("/api/concert/events/{}/{suffix}", request.event_id);
        refused(request).await;
    }
    for method in ["GET", "PATCH", "PUT", "DELETE", "post", "OPTIONS"] {
        let mut request = original.clone();
        request.http_method = method.into();
        refused(request).await;
    }
    let mut request = original;
    request.http_target = request
        .http_target
        .replace(&request.event_id, &Uuid::new_v4().to_string());
    refused(request).await;
}
#[tokio::test]
async fn original_json_rejects_duplicates_spoofed_fields_noncanonical_ids_and_limits() {
    let original = input("POST", "reports", &body());
    let valid: serde_json::Value = serde_json::from_slice(&original.http_body).unwrap();
    for field in [
        "reporter_id",
        "actor_id",
        "event_id",
        "receipt_id",
        "role",
        "state",
    ] {
        let mut value = valid.clone();
        value[field] = "spoof".into();
        refused(input("POST", "reports", &value.to_string())).await;
    }
    for value in [
        "[]".into(),
        "{}".into(),
        "null".into(),
        format!("{{\"target_type\":\"USER\",{}", &body()[1..]),
        "{bad".into(),
    ] {
        refused(input("POST", "reports", &value)).await;
    }
    for field in [
        "target_type",
        "target_id",
        "reason",
        "text",
        "evidence_media_ids",
    ] {
        let mut value = valid.clone();
        value.as_object_mut().unwrap().remove(field);
        refused(input("POST", "reports", &value.to_string())).await;
    }
    for id in [
        Uuid::nil().to_string(),
        Uuid::new_v4().simple().to_string(),
        "ABCDEFAB-0000-4000-8000-000000000001".into(),
        "bad".into(),
    ] {
        let mut value = valid.clone();
        value["target_id"] = id.clone().into();
        refused(input("POST", "reports", &value.to_string())).await;
        let mut request = original.clone();
        request.idempotency_key = id;
        refused(request).await;
    }
    for text in ["x".repeat(2001), "bad\u{0001}".into()] {
        let mut value = valid.clone();
        value["text"] = text.into();
        refused(input("POST", "reports", &value.to_string())).await;
    }
    for mime in ["", "application/json; charset=utf-8", "text/plain"] {
        let mut request = original.clone();
        request.content_type = mime.into();
        refused(request).await;
    }
    let mut request = original.clone();
    request.idempotency_key.clear();
    refused(request).await;
    let mut request = original;
    request.http_body = vec![b' '; 65537];
    refused(request).await;
    let mut value = valid;
    value["text"] = "한".repeat(2000).into();
    let request = input("POST", "reports", &value.to_string());
    assert!(ReportOperation::Create.validate(&request).is_ok());
}
#[tokio::test]
async fn other_targets_and_ready_media_remain_explicitly_unavailable() {
    let valid: serde_json::Value = serde_json::from_str(&body()).unwrap();
    for target in ["POST", "COMMENT", "PARTY", "GIVEAWAY", "MURAL"] {
        let mut value = valid.clone();
        value["target_type"] = target.into();
        let request = input("POST", "reports", &value.to_string());
        assert_eq!(
            ReportOperation::Create
                .validate(&request)
                .unwrap_err()
                .code(),
            Code::Unavailable
        );
        refused(request).await;
    }
    let mut value = valid;
    value["evidence_media_ids"] = json!([Uuid::new_v4()]);
    let request = input("POST", "reports", &value.to_string());
    assert_eq!(
        ReportOperation::Create
            .validate(&request)
            .unwrap_err()
            .code(),
        Code::Unavailable
    );
    refused(request).await;
}
#[tokio::test]
async fn foreign_actor_parent_and_expired_original_bounds_never_reach_owner() {
    let request = input("POST", "reports", &body());
    let owner = Recorder::new(Mode::Refuse);
    assert_ne!(
        call(
            server(owner.clone()),
            signed(&request, Uuid::new_v4(), None)
        )
        .await
        .0,
        Code::Ok
    );
    assert_eq!(owner.len(), 0);
    let mut expired = request;
    expired.authority_expires_unix_micros = chrono::Utc::now().timestamp_micros() - 1;
    refused(expired).await;
}
