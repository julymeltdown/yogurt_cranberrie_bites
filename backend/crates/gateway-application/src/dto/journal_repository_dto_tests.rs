use super::journal_repository_dto::*;
use super::operation_view_dto::*;
use gateway_domain::operation::{ManualOperationResolution, OperationStatus};

fn record(actor_id: Option<String>) -> OperationJournalRecordDto {
    OperationJournalRecordDto {
        operation_id: uuid::Uuid::from_u128(42),
        actor_id,
        state: "UNKNOWN".into(),
        outcome: OperationOutcomeDto::Unknown,
        action: "POST /api/follows".into(),
        resource_path: "/api/follows".into(),
        response_status: None,
        created_at: "2030-02-03T04:05:06.123456789Z".parse().unwrap(),
        finished_at: None,
        replayable: false,
        manual_resolution: None,
    }
}

#[test]
fn persistence_transfer_preserves_private_actor_before_one_way_http_projection() {
    let actor = uuid::Uuid::from_u128(7).to_string();
    let status = OperationStatus::from(record(Some(actor.clone())));
    assert_eq!(status.actor_identity(), Some(actor.as_str()));
    assert_eq!(status.owner_mutation_scope(), Some("friend.v1.FollowUser"));
    let view = OperationStatusResponseDto::from(status);
    let payload = serde_json::to_string(&view).unwrap();
    assert!(!payload.contains(&actor));
    assert!(!payload.contains("actorId"));
    assert!(!payload.contains("actor_id"));
    assert_eq!(view.operation_id, uuid::Uuid::from_u128(42));
    assert_eq!(view.outcome, OperationOutcomeDto::Unknown);
}

#[test]
fn absent_actor_is_preserved_without_manufacturing_receipt_identity() {
    let status = OperationStatus::from(record(None));
    assert!(status.actor_identity().is_none());
    assert_eq!(status.owner_mutation_scope(), Some("friend.v1.FollowUser"));
}

#[test]
fn manual_persistence_records_preserve_all_selections_and_exact_evidence() {
    for disposition in [
        ManualResolutionDispositionDto::Committed,
        ManualResolutionDispositionDto::NotCommitted,
        ManualResolutionDispositionDto::Unresolved,
    ] {
        for evidence_kind in [
            ManualResolutionEvidenceKindDto::DatabaseAudit,
            ManualResolutionEvidenceKindDto::OwnerConfirmation,
            ManualResolutionEvidenceKindDto::ServiceLog,
            ManualResolutionEvidenceKindDto::IncidentRecord,
            ManualResolutionEvidenceKindDto::Other,
        ] {
            let persisted = ManualResolutionJournalRecordDto {
                resolution_id: uuid::Uuid::from_u128(11),
                operation_id: uuid::Uuid::from_u128(42),
                revision: 9,
                supersedes_resolution_id: Some(uuid::Uuid::from_u128(10)),
                disposition,
                evidence_kind,
                evidence_reference: "DB-AUDIT-2030-0203".into(),
                reason: "Exact protected evidence and reviewer attribution.".into(),
                reviewer_id: "reviewer-private".into(),
                created_at: "2030-02-03T04:05:06.123456789Z".parse().unwrap(),
            };
            let domain = ManualOperationResolution::from(persisted.clone());
            assert_eq!(domain.disposition.as_str(), disposition.as_str());
            assert_eq!(domain.evidence_kind.as_str(), evidence_kind.as_str());
            assert!(ManualResolutionJournalRecordDto::from(domain.clone()) == persisted);
            let mut journal = record(Some("actor-private".into()));
            journal.manual_resolution = Some(persisted);
            assert_eq!(
                OperationStatus::from(journal).manual_resolution,
                Some(domain)
            );
        }
    }
}
