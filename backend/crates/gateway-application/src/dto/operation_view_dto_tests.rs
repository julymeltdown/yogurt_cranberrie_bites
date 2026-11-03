use super::operation_dto::ManualOperationResolutionRequest;
use super::operation_view_dto::*;
use chrono::{DateTime, Utc};
use gateway_domain::operation as domain;
use serde_json::json;
use uuid::Uuid;

fn timestamp() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2030-02-03T04:05:06.123456789Z")
        .unwrap()
        .with_timezone(&Utc)
}

#[test]
fn admin_view_preserves_wire_evidence_after_private_actor_correlation() {
    let operation_id = Uuid::from_u128(1);
    let manual = domain::ManualOperationResolution {
        resolution_id: Uuid::from_u128(2),
        operation_id,
        revision: 3,
        supersedes_resolution_id: Some(Uuid::from_u128(3)),
        disposition: domain::ManualResolutionDisposition::Committed,
        evidence_kind: domain::ManualResolutionEvidenceKind::OwnerConfirmation,
        evidence_reference: "private-record-reference".into(),
        reason: "a private explanation with enough characters".into(),
        reviewer_id: "private-reviewer-id".into(),
        created_at: timestamp(),
    };
    let receipt = domain::OwnerReceiptStatus {
        lookup_status: domain::OwnerReceiptLookupStatus::Found,
        receipt_state: Some("REPLAYABLE".into()),
        scope: Some("post.v1.CreatePost".into()),
        committed_at: Some(timestamp()),
        replay_until: None,
    };
    let status = domain::OperationStatus::from_persisted(
        domain::OperationStatusFields {
            operation_id,
            state: "UNKNOWN".into(),
            outcome: domain::OperationOutcome::Committed,
            action: "POST /api/posts".into(),
            resource_path: "/api/posts".into(),
            response_status: None,
            created_at: timestamp(),
            finished_at: None,
            replayable: false,
            owner_receipt: receipt.clone(),
            manual_resolution: Some(manual.clone()),
        },
        Some("private-actor-correlation".into()),
    );
    assert_eq!(status.actor_identity(), Some("private-actor-correlation"));
    assert_eq!(status.owner_mutation_scope(), Some("post.v1.CreatePost"));
    let view = OperationStatusResponseDto::from(status.clone());
    assert_eq!(
        serde_json::to_vec(&view).unwrap(),
        serde_json::to_vec(&status).unwrap()
    );
    let json = serde_json::to_value(&view).unwrap();
    assert_eq!(json["state"], "UNKNOWN");
    assert_eq!(json["outcome"], "COMMITTED");
    assert!(json["responseStatus"].is_null());
    assert!(json["finishedAt"].is_null());
    assert_eq!(json["ownerReceipt"]["lookupStatus"], "FOUND");
    assert!(json["ownerReceipt"]["replayUntil"].is_null());
    assert_eq!(json["manualResolution"]["revision"], 3);
    assert_eq!(json["createdAt"], "2030-02-03T04:05:06.123456789Z");
    assert!(json.get("actorId").is_none());
    assert!(!serde_json::to_string(&view)
        .unwrap()
        .contains("private-actor-correlation"));
    assert_eq!(
        serde_json::to_vec(&OwnerReceiptStatusResponseDto::from(receipt)).unwrap(),
        serde_json::to_vec(&status.owner_receipt).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(&ManualOperationResolutionResponseDto::from(manual)).unwrap(),
        serde_json::to_vec(status.manual_resolution.as_ref().unwrap()).unwrap()
    );
    for debug in [
        format!("{view:?}"),
        format!("{:?}", view.owner_receipt),
        format!("{:?}", view.manual_resolution.unwrap()),
    ] {
        assert!(!debug.contains("private-"));
        assert!(!debug.contains("post.v1.CreatePost"));
    }
    // Existing old status documents may omit both evidence additions.
    let mut old_shape = json;
    old_shape.as_object_mut().unwrap().remove("ownerReceipt");
    old_shape
        .as_object_mut()
        .unwrap()
        .remove("manualResolution");
    let restored: OperationStatusResponseDto = serde_json::from_value(old_shape).unwrap();
    assert_eq!(
        restored.owner_receipt.lookup_status,
        OwnerReceiptLookupStatusDto::NotApplicable
    );
    assert!(restored.manual_resolution.is_none());
}

#[test]
fn caller_view_has_exactly_the_seven_public_fields_and_preserves_unknown() {
    let status = domain::CallerOperationStatus {
        operation_id: Uuid::from_u128(1),
        state: "UNKNOWN".into(),
        outcome: domain::OperationOutcome::Unknown,
        response_status: None,
        created_at: timestamp(),
        finished_at: None,
        replayable: false,
    };
    let view = CallerOperationStatusResponseDto::from(status.clone());
    assert_eq!(
        serde_json::to_vec(&view).unwrap(),
        serde_json::to_vec(&status).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&view).unwrap(),
        json!({
            "operationId": Uuid::from_u128(1), "state": "UNKNOWN", "outcome": "UNKNOWN",
            "responseStatus": null, "createdAt": "2030-02-03T04:05:06.123456789Z",
            "finishedAt": null, "replayable": false
        })
    );
}

#[test]
fn manual_request_selections_keep_existing_names_validation_and_unknown_field_rejection() {
    for disposition in [
        domain::ManualResolutionDisposition::Committed,
        domain::ManualResolutionDisposition::NotCommitted,
        domain::ManualResolutionDisposition::Unresolved,
    ] {
        for evidence in [
            domain::ManualResolutionEvidenceKind::DatabaseAudit,
            domain::ManualResolutionEvidenceKind::OwnerConfirmation,
            domain::ManualResolutionEvidenceKind::ServiceLog,
            domain::ManualResolutionEvidenceKind::IncidentRecord,
            domain::ManualResolutionEvidenceKind::Other,
        ] {
            let mut wire = json!({
                "disposition": disposition.as_str(), "evidenceKind": evidence.as_str(),
                "evidenceReference": "incident:123", "reason": "A sufficiently long evidence-backed reason"
            });
            let request: ManualOperationResolutionRequest =
                serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(
                domain::ManualResolutionDisposition::from(request.disposition),
                disposition
            );
            assert_eq!(
                domain::ManualResolutionEvidenceKind::from(request.evidence_kind),
                evidence
            );
            assert_eq!(request.disposition.as_str(), disposition.as_str());
            assert_eq!(request.evidence_kind.as_str(), evidence.as_str());
            assert!(request.supersedes_resolution_id.is_none());
            assert!(request.validate().is_ok());
            wire["actorId"] = json!("injected-actor");
            assert!(serde_json::from_value::<ManualOperationResolutionRequest>(wire).is_err());
        }
    }
    for invalid in ["committed", "DELETE", ""] {
        assert!(
            serde_json::from_value::<ManualOperationResolutionRequest>(json!({
                "disposition": invalid, "evidenceKind": "OTHER", "evidenceReference": "record:123",
                "reason": "A sufficiently long evidence-backed reason"
            }))
            .is_err()
        );
    }
    for outcome in [
        domain::OperationOutcome::Pending,
        domain::OperationOutcome::Committed,
        domain::OperationOutcome::RejectedBeforeCommit,
        domain::OperationOutcome::Unknown,
    ] {
        let view = OperationOutcomeDto::from(outcome);
        assert_eq!(
            serde_json::to_vec(&view).unwrap(),
            serde_json::to_vec(&outcome).unwrap()
        );
        assert_eq!(domain::OperationOutcome::from(view), outcome);
    }
    for lookup in [
        domain::OwnerReceiptLookupStatus::NotApplicable,
        domain::OwnerReceiptLookupStatus::Found,
        domain::OwnerReceiptLookupStatus::NotFound,
        domain::OwnerReceiptLookupStatus::Unavailable,
        domain::OwnerReceiptLookupStatus::Mismatch,
    ] {
        let view = OwnerReceiptLookupStatusDto::from(lookup);
        assert_eq!(
            serde_json::to_vec(&view).unwrap(),
            serde_json::to_vec(&lookup).unwrap()
        );
        assert_eq!(domain::OwnerReceiptLookupStatus::from(view), lookup);
    }
}
