use chrono::{Duration, Utc};
use concert_application::{commands::*, community::giveaway::PickupCode};
use concert_domain::community::giveaway::*;
use concert_service::{
    community::giveaway::{GiftServiceError, IssuedCode},
    gift_live::{protocol::GiftReply, response::*},
};
use serde_json::{json, Value};
use uuid::Uuid;

fn validate(schema: &str, value: &Value) {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../contracts/generated/mcr-domain.openapi.json"
    ))
    .unwrap();
    let schema =
        json!({"$ref":format!("#/components/schemas/{schema}"),"components":source["components"]});
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = validator
        .iter_errors(value)
        .map(|v| v.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
}
fn fixture() -> Reservation {
    Reservation {
        id: Uuid::new_v4(),
        recipient: Uuid::new_v4(),
        state: ReservationState::Ready,
        revision: 5,
        expires_at: Utc::now() + Duration::minutes(10),
        created_at: Utc::now() - Duration::minutes(1),
        claim_receipt_id: None,
        claimed_at: None,
        code_expires_at: Some(Utc::now() + Duration::minutes(2)),
        code_generation: 42,
    }
}
#[test]
fn all_reservation_states_conform_without_internal_code_metadata() {
    let mut r = fixture();
    let event = Uuid::new_v4();
    let gift = Uuid::new_v4();
    for state in [
        ReservationState::Held,
        ReservationState::Ready,
        ReservationState::Claimed,
        ReservationState::Cancelled,
        ReservationState::Expired,
    ] {
        r.state = state;
        let v = reservation(event, gift, &r).unwrap();
        validate("Domain_giveaway_Reservation", &v);
        assert_eq!(v.as_object().unwrap().len(), 9);
        for private in [
            "claimed_at",
            "code_generation",
            "code_expires_at",
            "confirmation_code",
            "meeting",
            "keyed_hash",
        ] {
            assert!(v.get(private).is_none());
        }
    }
}
#[test]
fn claimed_requires_terminal_state_receipt_and_time() {
    let mut r = fixture();
    let gift = Uuid::new_v4();
    assert!(claimed(gift, &r).is_err());
    r.claim_receipt_id = Some(Uuid::new_v4());
    r.claimed_at = Some(Utc::now());
    assert!(claimed(gift, &r).is_err());
    r.state = ReservationState::Claimed;
    validate("Domain_giveaway_PickupReceipt", &claimed(gift, &r).unwrap());
    r.claim_receipt_id = Some(Uuid::nil());
    assert!(claimed(gift, &r).is_err());
}
#[test]
fn one_shot_code_preserves_leading_zero_and_only_c04_fields() {
    let r = fixture();
    let v = issued(IssuedCode {
        code: PickupCode::parse("000123".into()).unwrap(),
        reservation: r.id,
        revision: r.revision,
        expires_at: r.code_expires_at.unwrap(),
    })
    .unwrap();
    validate("Domain_giveaway_PickupCode", &v);
    assert_eq!(v["confirmation_code"], "000123");
    assert_eq!(v.as_object().unwrap().len(), 4);
}
#[test]
fn safe_errors_match_frozen_status_policy_and_do_not_include_input() {
    let source: Value =
        serde_json::from_str(include_str!("../../../../contracts/domain/v1/policy.json")).unwrap();
    let errors = [
        GiftError::InvalidInput,
        GiftError::Unavailable,
        GiftError::Denied,
        GiftError::VersionConflict,
        GiftError::InvalidTransition,
        GiftError::SoldOut,
        GiftError::DuplicateReservation,
        GiftError::Expired,
        GiftError::CodeInvalid,
    ];
    for e in errors {
        let f: Failure = GiftServiceError::Gift(e).into();
        assert_eq!(source["http_error_status"][f.reason], f.status);
        let GiftReply::Json { status, body } = f.reply(Uuid::new_v4());
        assert_eq!(status, f.status);
        assert_eq!(body["details"], json!({"reason":f.reason}));
        assert!(body.get("body").is_none());
        assert!(body.get("confirmation_code").is_none());
    }
    for e in [
        CommandError::InvalidInput,
        CommandError::Denied,
        CommandError::Unavailable,
        CommandError::Conflict,
        CommandError::Expired,
        CommandError::Capacity,
        CommandError::InvalidState,
    ] {
        let f: Failure = e.into();
        assert_eq!(source["http_error_status"][f.reason], f.status);
    }
}

#[test]
fn completed_receipt_cannot_be_reported_as_pending() {
    use concert_service::gift_live::protocol::GiftOperation;
    assert!(checked_reply(GiftOperation::IssueCode, 202, json!({"state":"SUCCEEDED"})).is_err());
    assert!(checked_reply(GiftOperation::Create, 202, json!({"state":"FAILED"})).is_err());
}
