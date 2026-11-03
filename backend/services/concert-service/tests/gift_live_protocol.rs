//! Tests the exact new headless source before root-owned module registration.
#[path = "../src/gift_live/protocol.rs"]
mod protocol;
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use protocol::*;
use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;
fn identity() -> GiftIdentity {
    GiftIdentity {
        actor: Uuid::new_v4(),
        event: Uuid::new_v4(),
        session_family: Uuid::new_v4(),
        request_id: Uuid::new_v4(),
        deadline: AuthorityRpcDeadline::new(
            chrono::Utc::now().timestamp_micros() + 2_000_000,
            Duration::from_secs(2),
        )
        .unwrap(),
    }
}
fn fields() -> Value {
    json!({"title":"Gift","description":"","image_media_id":null,"quantity_total":3,"pickup_starts_at":"2035-11-07T08:00:00Z","pickup_ends_at":"2035-11-07T09:00:00Z","public_place_id":Uuid::new_v4()})
}
fn request(
    identity: GiftIdentity,
    op: GiftOperation,
    suffix: &str,
    body: Option<Value>,
) -> Result<GiftRequest, InputError> {
    let method = match op {
        GiftOperation::Create
        | GiftOperation::Reserve
        | GiftOperation::Claim
        | GiftOperation::IssueCode => "POST",
        GiftOperation::Update => "PATCH",
        GiftOperation::SetMeeting => "PUT",
        GiftOperation::Cancel => "DELETE",
        _ => "GET",
    };
    GiftRequest::from_verified(
        identity,
        op.id(),
        method,
        &format!("/api/concert/events/{}/{}", identity.event, suffix),
        if body.is_some() {
            "application/json"
        } else {
            ""
        },
        if op.mutation() {
            "41414141-4141-4141-8141-414141414141"
        } else {
            ""
        },
        body.map(|v| serde_json::to_vec(&v).unwrap())
            .unwrap_or_default(),
    )
}
#[test]
fn all_thirteen_frozen_operations_have_exact_routes_and_shapes() {
    let i = identity();
    let gift = Uuid::new_v4();
    let reservation = Uuid::new_v4();
    let mut update = fields();
    update["expected_revision"] = json!(1);
    update["state"] = json!("OPEN");
    let cases = vec![
        (
            GiftOperation::List,
            "giveaways?limit=20&in_stock=true".into(),
            None,
        ),
        (GiftOperation::Detail, format!("giveaways/{gift}"), None),
        (GiftOperation::Create, "giveaways".into(), Some(fields())),
        (
            GiftOperation::Update,
            format!("giveaways/{gift}"),
            Some(update),
        ),
        (
            GiftOperation::Management,
            format!("giveaways/{gift}/management?state=READY&cursor=opaque_cursor"),
            None,
        ),
        (
            GiftOperation::SetMeeting,
            format!("giveaways/{gift}/pickup-meeting"),
            Some(
                json!({"expected_revision":1,"physical_presence_acknowledged":true,"meeting":{"place_id":Uuid::new_v4(),"map_revision":1,"place_revision":1,"instructions":"Meet here","identification":"Green bag","expires_at":"2035-11-07T09:00:00Z"}}),
            ),
        ),
        (
            GiftOperation::Reserve,
            format!("giveaways/{gift}/reservations"),
            Some(json!({"giveaway_revision":1})),
        ),
        (
            GiftOperation::Activity,
            "me/giveaways?limit=20".into(),
            None,
        ),
        (
            GiftOperation::Reservation,
            format!("reservations/{reservation}"),
            None,
        ),
        (
            GiftOperation::Cancel,
            format!("reservations/{reservation}?expected_revision=1"),
            None,
        ),
        (
            GiftOperation::Claim,
            format!("reservations/{reservation}/claim"),
            Some(json!({"expected_revision":1,"confirmation_code":"012345"})),
        ),
        (
            GiftOperation::Pickup,
            format!("reservations/{reservation}/pickup"),
            None,
        ),
        (
            GiftOperation::IssueCode,
            format!("reservations/{reservation}/pickup-code"),
            Some(json!({"expected_revision":1})),
        ),
    ];
    for (op, path, body) in cases {
        let r = request(i, op, &path, body);
        assert!(r.is_ok(), "{}", op.id());
        assert_eq!(r.unwrap().operation(), op);
    }
}
#[test]
fn rejects_body_authority_unknown_missing_and_duplicate_fields() {
    let i = identity();
    let mut f = fields();
    f["actor"] = json!(Uuid::new_v4());
    assert!(request(i, GiftOperation::Create, "giveaways", Some(f)).is_err());
    let mut f = fields();
    f.as_object_mut().unwrap().remove("image_media_id");
    assert!(request(i, GiftOperation::Create, "giveaways", Some(f)).is_err());
    let raw = br#"{"giveaway_revision":1,"giveaway_revision":2}"#.to_vec();
    assert!(GiftRequest::from_verified(
        i,
        "reserveConcertGiveaway",
        "POST",
        &format!(
            "/api/concert/events/{}/giveaways/{}/reservations",
            i.event,
            Uuid::new_v4()
        ),
        "application/json",
        "41414141-4141-4141-8141-414141414141",
        raw
    )
    .is_err());
}
#[test]
fn rejects_wrong_method_route_event_and_noncanonical_ids() {
    let i = identity();
    let root = format!("/api/concert/events/{}/giveaways", i.event);
    assert!(
        GiftRequest::from_verified(i, "listConcertGiveaways", "POST", &root, "", "", vec![])
            .is_err()
    );
    assert!(
        GiftRequest::from_verified(i, "getConcertGiveaway", "GET", &root, "", "", vec![]).is_err()
    );
    assert!(GiftRequest::from_verified(
        i,
        "listConcertGiveaways",
        "GET",
        &format!("/api/concert/events/{}/giveaways", Uuid::new_v4()),
        "",
        "",
        vec![]
    )
    .is_err());
    assert!(request(
        i,
        GiftOperation::Detail,
        "giveaways/41414141414141418141414141414141",
        None
    )
    .is_err());
    assert!(request(
        i,
        GiftOperation::Detail,
        "giveaways/00000000-0000-0000-0000-000000000000",
        None
    )
    .is_err());
}
#[test]
fn query_is_closed_single_valued_bounded_and_percent_decoded() {
    let i = identity();
    for suffix in [
        "giveaways?limit=0",
        "giveaways?limit=01",
        "giveaways?limit=101",
        "giveaways?state=DRAFT",
        "giveaways?owner=me",
        "giveaways?limit=1&limit=2",
        "giveaways?cursor=a%00b",
        "giveaways?cursor=%GG",
        "giveaways?cursor=",
        "giveaways?pickup_after=2035-11-08T00:00:00Z&pickup_before=2035-11-07T00:00:00Z",
    ] {
        assert!(
            request(i, GiftOperation::List, suffix, None).is_err(),
            "{suffix}"
        );
    }
    assert!(request(
        i,
        GiftOperation::List,
        "giveaways?pickup_after=2035-11-07T00%3A00%3A00Z",
        None
    )
    .is_ok());
    assert!(request(
        i,
        GiftOperation::Detail,
        &format!("giveaways/{}?limit=20", Uuid::new_v4()),
        None
    )
    .is_err());
}
#[test]
fn mutations_require_keys_and_exact_content_type_and_reads_refuse_payloads() {
    let i = identity();
    let root = format!("/api/concert/events/{}/giveaways", i.event);
    let body = serde_json::to_vec(&fields()).unwrap();
    for key in ["", "not-a-key", "00000000-0000-0000-0000-000000000000"] {
        assert!(GiftRequest::from_verified(
            i,
            "createConcertGiveaway",
            "POST",
            &root,
            "application/json",
            key,
            body.clone()
        )
        .is_err());
    }
    assert!(GiftRequest::from_verified(
        i,
        "createConcertGiveaway",
        "POST",
        &root,
        "text/plain",
        "41414141-4141-4141-8141-414141414141",
        body.clone()
    )
    .is_err());
    assert!(GiftRequest::from_verified(
        i,
        "listConcertGiveaways",
        "GET",
        &root,
        "application/json",
        "",
        body
    )
    .is_err());
    assert!(GiftRequest::from_verified(
        i,
        "listConcertGiveaways",
        "GET",
        &root,
        "",
        "41414141-4141-4141-8141-414141414141",
        vec![]
    )
    .is_err());
}
#[test]
fn code_and_revision_validation_matches_closed_contract() {
    let i = identity();
    let claim = format!("reservations/{}/claim", Uuid::new_v4());
    for code in ["12345", "1234567", "abcdef", "１２３４５６"] {
        assert!(request(
            i,
            GiftOperation::Claim,
            &claim,
            Some(json!({"expected_revision":1,"confirmation_code":code}))
        )
        .is_err());
    }
    for revision in [0, -1, 9007199254740992i64] {
        assert!(request(
            i,
            GiftOperation::Claim,
            &claim,
            Some(json!({"expected_revision":revision,"confirmation_code":"123456"}))
        )
        .is_err());
    }
    let r = request(
        i,
        GiftOperation::Claim,
        &claim,
        Some(json!({"expected_revision":1,"confirmation_code":"012345"})),
    )
    .unwrap();
    match r.input() {
        GiftInput::Write {
            code: Some(code), ..
        } => assert_eq!(code.expose(), "012345"),
        _ => panic!("typed code absent"),
    }
}
#[test]
fn preserves_original_edge_material_for_owner_binding() {
    let i = identity();
    let root = format!(
        "/api/concert/events/{}/giveaways/{}/reservations",
        i.event,
        Uuid::new_v4()
    );
    let body = b"{ \"giveaway_revision\" : 1 }".to_vec();
    let key = Uuid::new_v4();
    let r = GiftRequest::from_verified(
        i,
        "reserveConcertGiveaway",
        "POST",
        &root,
        "application/json",
        &key.to_string(),
        body.clone(),
    )
    .unwrap();
    assert_eq!(r.body(), body);
    assert_eq!(r.target(), root);
    assert_eq!(r.method(), "POST");
    assert_eq!(r.content_type(), "application/json");
    assert_eq!(r.idempotency_key(), Some(key));
    assert_eq!(r.identity().deadline.stop(), i.deadline.stop());
    assert_eq!(r.identity().request_id, i.request_id);
}
#[tokio::test]
async fn original_expired_monotonic_stop_cannot_be_renewed_by_parser() {
    let mut i = identity();
    i.deadline = AuthorityRpcDeadline::within_original(
        chrono::Utc::now().timestamp_micros() + 1_000_000,
        std::time::Instant::now() + Duration::from_millis(1),
        Duration::from_secs(2),
    )
    .unwrap();
    tokio::time::sleep(Duration::from_millis(5)).await;
    assert!(matches!(
        request(i, GiftOperation::List, "giveaways", None),
        Err(InputError::Expired)
    ));
}
