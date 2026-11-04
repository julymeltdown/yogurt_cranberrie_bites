//! Producer DTO/consumer fixture tests; no production photo rights or DB claim.

use concert_application::beacon::BeaconMutation;
use concert_service::meeting_photo::wire::*;
use serde_json::{json, Value};
fn fixture() -> Value {
    serde_json::from_str(include_str!("meeting_photo_support/contract-fixtures.json")).unwrap()
}
#[test]
fn contract_fixture_preserves_photo_on_create_update_and_read_projection() {
    let f = fixture();
    let create = CreatePhotoBeacon::parse(&serde_json::to_vec(&f["create"]).unwrap()).unwrap();
    let update = UpdatePhotoBeacon::parse(&serde_json::to_vec(&f["update"]).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(&create).unwrap(), f["create"]);
    assert_eq!(serde_json::to_value(&update).unwrap(), f["update"]);
    let mut text = f["read"].clone();
    text["meeting"].as_object_mut().unwrap().remove("media_id");
    let beacon = serde_json::from_value(text).unwrap();
    let output = PhotoBeacon::from_authorized(beacon, create.meeting.media_id);
    assert_eq!(serde_json::to_value(output).unwrap(), f["read"]);
}
#[test]
fn text_only_typed_fingerprint_shape_is_unchanged_and_null_update_removes_photo() {
    let f = fixture();
    let mut body = f["create"].clone();
    body.as_object_mut()
        .unwrap()
        .remove("photo_permission_acknowledged");
    body["meeting"].as_object_mut().unwrap().remove("media_id");
    let request = CreatePhotoBeacon::parse(&serde_json::to_vec(&body).unwrap()).unwrap();
    let BeaconMutation::Create(old) = request.text_mutation() else {
        panic!("wrong effect")
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::to_value(old).unwrap()
    );
    let mut body = f["update"].clone();
    body["meeting"]["media_id"] = Value::Null;
    body["photo_permission_acknowledged"] = json!(false);
    let request = UpdatePhotoBeacon::parse(&serde_json::to_vec(&body).unwrap()).unwrap();
    assert!(request.meeting.media_id.is_none());
    let BeaconMutation::Update(old) = request.text_mutation() else {
        panic!("wrong effect")
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::to_value(old).unwrap()
    );
}
#[test]
fn missing_permission_noncanonical_identity_and_physical_inference_are_rejected() {
    for (field, value) in [
        ("photo_permission_acknowledged", json!(false)),
        ("physical_presence_acknowledged", json!(false)),
        ("actor_id", json!("22222222-2222-4222-8222-222222222222")),
        ("world_position", json!({"x":1,"y":2})),
    ] {
        let mut b = fixture()["create"].clone();
        b[field] = value;
        assert!(CreatePhotoBeacon::parse(&serde_json::to_vec(&b).unwrap()).is_err());
    }
    for value in [
        "00000000-0000-0000-0000-000000000000",
        "55555555555545558555555555555555",
        "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
    ] {
        let mut b = fixture()["create"].clone();
        b["meeting"]["media_id"] = json!(value);
        assert!(CreatePhotoBeacon::parse(&serde_json::to_vec(&b).unwrap()).is_err());
    }
}
#[test]
fn duplicate_fields_unknown_meeting_fields_and_oversized_inputs_are_rejected() {
    let b = serde_json::to_string(&fixture()["create"]).unwrap();
    let duplicate = b.replacen('{', "{\"photo_permission_acknowledged\":false,", 1);
    assert!(CreatePhotoBeacon::parse(duplicate.as_bytes()).is_err());
    let mut b = fixture()["create"].clone();
    b["meeting"]["photo_url"] = json!("https://invalid.example/private.png");
    assert!(CreatePhotoBeacon::parse(&serde_json::to_vec(&b).unwrap()).is_err());
    assert!(CreatePhotoBeacon::parse(&vec![b' '; 65537]).is_err());
    let mut b = fixture()["update"].clone();
    b["expected_revision"] = json!(0);
    assert!(UpdatePhotoBeacon::parse(&serde_json::to_vec(&b).unwrap()).is_err());
}

#[test]
fn settlement_reference_is_closed_and_full_photo_fingerprint_is_required() {
    use concert_service::meeting_photo::canonical::validate_settlement_reference;
    let f = fixture();
    let guard_digest = vec![3; 32];
    let body_sha256 = vec![4; 32];
    let photo_binding_digest = vec![5; 32];
    let r = json!({"version":1,"purpose":"MEETING_PHOTO","actor":f["actor_id"],"event":f["event_id"],"beacon":f["beacon_id"],"key":"77777777-7777-4777-8777-777777777777","operation":"createConcertBeacon","guard_digest":guard_digest,"body_sha256":body_sha256,"photo_binding_digest":photo_binding_digest});
    let parsed = validate_settlement_reference(&serde_json::to_vec(&r).unwrap()).unwrap();
    assert_eq!(parsed.photo_binding_digest(), [5; 32]);
    assert!(parsed.idempotency_key().is_some());
    for (k, v) in [
        ("version", json!(0)),
        ("purpose", json!("MEDIA")),
        ("key", Value::Null),
        ("photo_binding_digest", json!(vec![0; 32])),
        ("extra", json!(true)),
    ] {
        let mut bad = r.clone();
        bad[k] = v;
        assert!(validate_settlement_reference(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
}

#[test]
fn request_frame_binds_raw_body_content_type_key_and_intent() {
    use concert_application::beacon::MeetingActor;
    use concert_service::meeting_photo::{PhotoFrame, PhotoMutation};
    use uuid::Uuid;
    let f = fixture();
    let actor = MeetingActor {
        actor: serde_json::from_value(f["actor_id"].clone()).unwrap(),
        event: serde_json::from_value(f["event_id"].clone()).unwrap(),
    };
    let beacon = serde_json::from_value(f["beacon_id"].clone()).unwrap();
    let body = serde_json::to_vec(&f["create"]).unwrap();
    let mutation = PhotoMutation::Create(CreatePhotoBeacon::parse(&body).unwrap());
    let key = Uuid::new_v4();
    let frame =
        PhotoFrame::mutation(actor, beacon, key, &mutation, "application/json", &body).unwrap();
    assert!(PhotoFrame::mutation(actor, beacon, key, &mutation, "text/plain", &body).is_err());
    let mut changed = f["create"].clone();
    changed["meeting"]["media_id"] = json!(Uuid::new_v4());
    assert!(PhotoFrame::mutation(
        actor,
        beacon,
        key,
        &mutation,
        "application/json",
        &serde_json::to_vec(&changed).unwrap(),
    )
    .is_err());
    let mut other_key = frame.clone();
    other_key.key = Some(Uuid::new_v4());
    assert_ne!(frame.digest(), other_key.digest());
    let revoke = PhotoMutation::Revoke {
        expected_revision: 1,
    };
    assert!(PhotoFrame::mutation(actor, beacon, key, &revoke, "", b"{}").is_err());
    assert!(PhotoFrame::mutation(actor, beacon, key, &revoke, "", b"").is_ok());
}
