//! Transport/privacy and pagination regressions; these grant no owner authority.
use chrono::{Duration, Utc};
use concert_service::operations_live::{
    cursor::{CursorBinding, OperationsCursor},
    wire,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use uuid::Uuid;
fn path(event: Uuid, suffix: &str) -> String {
    format!("/api/concert/events/{event}/{suffix}")
}
#[test]
fn closed_report_input_rejects_spoofed_private_fields_and_duplicate_keys() {
    wire::warm_contracts().unwrap();
    let event = Uuid::new_v4();
    let key = Uuid::new_v4().to_string();
    let target = path(event, "reports");
    let body = json!({"target_type":"USER","target_id":Uuid::new_v4(),"reason":"SPAM","text":"private canary","evidence_media_ids":[]});
    let accept = |value: &Value| {
        wire::validate_body_for_fixture(
            event,
            "POST",
            &target,
            &serde_json::to_vec(value).unwrap(),
            "application/json",
            &key,
        )
        .is_ok()
    };
    assert!(accept(&body));
    for field in ["reporter_id", "event_id", "actor_id", "role", "receipt_id"] {
        let mut bad = body.clone();
        bad[field] = json!(Uuid::new_v4());
        assert!(!accept(&bad));
    }
    let duplicate =
        serde_json::to_string(&body)
            .unwrap()
            .replacen("{", "{\"reason\":\"OTHER\",", 1);
    assert!(wire::validate_body_for_fixture(
        event,
        "POST",
        &target,
        duplicate.as_bytes(),
        "application/json",
        &key
    )
    .is_err());
    let mut duplicate_media = body.clone();
    let id = Uuid::new_v4();
    duplicate_media["evidence_media_ids"] = json!([id, id]);
    assert!(!accept(&duplicate_media));
    assert!(wire::validate_body_for_fixture(
        event,
        "POST",
        &target,
        &serde_json::to_vec(&body).unwrap(),
        "application/json",
        &Uuid::nil().to_string()
    )
    .is_err());
}
#[test]
fn route_and_filter_boundaries_reject_cross_route_values() {
    let event = Uuid::new_v4();
    for route in [
        "me/reports?limit=1&state=OPEN",
        "admin/reports?state=IN_REVIEW&target_type=USER",
        "admin/notices?state=PUBLISHED",
        "notices?limit=100",
        "admin/places?closed=true",
    ] {
        assert!(
            wire::validate_body_for_fixture(event, "GET", &path(event, route), b"", "", "").is_ok(),
            "{route}"
        );
    }
    for route in [
        "me/reports?state=PUBLISHED",
        "admin/notices?state=OPEN",
        "me/reports?limit=0",
        "me/reports?limit=101",
        "me/reports?limit=1&limit=2",
        "me/reports?reporter_id=spoof",
        "admin/settings?cursor=abc",
        "admin/reports/not-a-uuid",
        "notices?closed=true",
    ] {
        assert!(
            wire::validate_body_for_fixture(event, "GET", &path(event, route), b"", "", "")
                .is_err(),
            "{route}"
        );
    }
}
#[test]
fn opaque_cursor_rejects_tamper_expiry_and_every_request_binding_change() {
    let codec = OperationsCursor::new(&[7; 32]).unwrap();
    let binding = CursorBinding {
        actor: Uuid::new_v4(),
        event: Uuid::new_v4(),
        operation: "listOwnConcertReports".into(),
        filters: BTreeMap::from([("state".into(), "OPEN".into())]),
        limit: 20,
    };
    let now = Utc::now();
    let position = Uuid::new_v4();
    let token = codec.seal(&binding, position, now).unwrap();
    assert_eq!(codec.open(&binding, &token, now).unwrap(), position);
    assert!(!token.contains(&position.to_string()));
    for change in 0..5 {
        let mut other = binding.clone();
        match change {
            0 => other.actor = Uuid::new_v4(),
            1 => other.event = Uuid::new_v4(),
            2 => other.operation = "listConcertAdminReports".into(),
            3 => {
                other.filters.insert("state".into(), "IN_REVIEW".into());
            }
            _ => other.limit = 21,
        };
        assert!(codec.open(&other, &token, now).is_err());
    }
    let mut bytes = token.clone().into_bytes();
    bytes[30] = if bytes[30] == b'0' { b'1' } else { b'0' };
    assert!(codec
        .open(&binding, std::str::from_utf8(&bytes).unwrap(), now)
        .is_err());
    assert!(codec
        .open(&binding, &token, now + Duration::minutes(15))
        .is_err());
    assert!(OperationsCursor::new(&[0; 32]).is_err());
}
#[test]
fn public_report_dto_cannot_contain_reporter_text_or_evidence() {
    let report = json!({"report_id":Uuid::new_v4(),"event_id":Uuid::new_v4(),"target_type":"USER","target_id":Uuid::new_v4(),"state":"OPEN","revision":1,"created_at":Utc::now()});
    wire::validate_value("Domain_admin_Report", &report).unwrap();
    for field in ["reporter_id", "reason", "text", "evidence_media_ids"] {
        let mut leaked = report.clone();
        leaked[field] = json!("private-canary");
        assert!(wire::validate_value("Domain_admin_Report", &leaked).is_err());
    }
}

// Captures the real signature-verified parent through the production Gateway
// workload guard. No synthetic GuardBinding and no positive database scope.
#[path = "operations_live_support/capture.rs"]
mod capture;
#[tokio::test]
async fn actual_gateway_parent_frame_is_required_for_request_binding() -> anyhow::Result<()> {
    let (binding, frame) = capture::binding(
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        "listOwnConcertReports",
        "GET",
        "me/reports",
        vec![],
        None,
    )
    .await?;
    assert!(
        concert_service::operations_live::OperationsRequest::from_owner_scope(&binding, &frame)
            .is_ok()
    );
    let mut rebound = frame.clone();
    rebound.access_expires_unix_micros += 1;
    assert!(
        concert_service::operations_live::OperationsRequest::from_owner_scope(&binding, &rebound)
            .is_err(),
        "Full admitted parent frame must bind non-body authority fields too"
    );
    let mut rebound = frame;
    rebound.http_target.push_str("?limit=1");
    assert!(
        concert_service::operations_live::OperationsRequest::from_owner_scope(&binding, &rebound)
            .is_err()
    );
    Ok(())
}
