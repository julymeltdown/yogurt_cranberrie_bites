use concert_application::commands::*;
use uuid::Uuid;
const EVENT: &str = "11111111-1111-4111-8111-111111111111";
const TARGET: &str = "22222222-2222-4222-8222-222222222222";
fn scope(op: &str) -> Scope {
    Scope {
        actor: Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap(),
        event: Uuid::parse_str(EVENT).unwrap(),
        operation: RegisteredOperation::find(op).unwrap(),
        key: Uuid::parse_str("44444444-4444-4444-8444-444444444444").unwrap(),
    }
}
fn keys() -> FingerprintKeys {
    FingerprintKeys::new(SigningKey::new("test_v1", &[0x0b; 32]).unwrap(), vec![]).unwrap()
}
fn fingerprint(body: &CanonicalValue, query: &[(&str, &str)]) -> Fingerprint {
    keys()
        .sign(
            &CanonicalRequest::json(
                scope("createConcertParty"),
                &[("eventId", EVENT)],
                query,
                Some(body),
            )
            .unwrap(),
        )
        .unwrap()
}
#[test]
fn registry_rejects_arbitrary_reads_and_operations() {
    assert_eq!(OPERATIONS.len(), 69);
    assert_eq!(OPERATIONS.len(), 69);
    for (id, tool) in [
        ("eraseConcertMuralPixel", "erase"),
        ("fillConcertMuralRegion", "fill"),
        ("undoConcertMuralAction", "undo"),
    ] {
        let operation = RegisteredOperation::find(id).unwrap();
        assert_eq!(operation.method(), "POST");
        assert_eq!(
            operation.path(&[("eventId", EVENT)]).unwrap(),
            format!("/api/concert/events/{EVENT}/mural/{tool}")
        );
    }
    for id in [
        "unregistered",
        "getConcertParty",
        "getConcertCommandReceipt",
    ] {
        assert_eq!(
            RegisteredOperation::find(id),
            Err(CommandError::InvalidInput)
        );
    }
}
#[test]
fn canonical_values_and_queries_are_ordered_but_arrays_and_types_are_distinct() {
    let a = CanonicalValue::Object(vec![
        ("z".into(), CanonicalValue::Integer(1)),
        ("a".into(), CanonicalValue::String("secret".into())),
    ]);
    let b = CanonicalValue::Object(vec![
        ("a".into(), CanonicalValue::String("secret".into())),
        ("z".into(), CanonicalValue::Integer(1)),
    ]);
    assert!(
        fingerprint(&a, &[("b", "2"), ("a", "1")]) == fingerprint(&b, &[("a", "1"), ("b", "2")])
    );
    let values = [
        CanonicalValue::Null,
        CanonicalValue::Bool(true),
        CanonicalValue::String("true".into()),
        CanonicalValue::Integer(1),
        CanonicalValue::String("1".into()),
        CanonicalValue::Object(vec![]),
        CanonicalValue::Object(vec![("a".into(), CanonicalValue::Null)]),
        CanonicalValue::Array(vec![CanonicalValue::Integer(1), CanonicalValue::Integer(2)]),
        CanonicalValue::Array(vec![CanonicalValue::Integer(2), CanonicalValue::Integer(1)]),
    ];
    let all: std::collections::BTreeSet<_> = values
        .iter()
        .map(|v| fingerprint(v, &[]).as_storage())
        .collect();
    assert_eq!(all.len(), values.len());
    let absent = keys()
        .sign(
            &CanonicalRequest::json(
                scope("createConcertParty"),
                &[("eventId", EVENT)],
                &[],
                None,
            )
            .unwrap(),
        )
        .unwrap();
    assert!(absent != fingerprint(&CanonicalValue::Null, &[]));
    assert!(
        fingerprint(&CanonicalValue::Null, &[]) != fingerprint(&CanonicalValue::Null, &[("a", "")])
    );
}
#[test]
fn canonical_rejects_duplicate_oversized_unsafe_or_mismatched_scope_input() {
    let s = scope("createConcertParty");
    let params = [("eventId", EVENT)];
    for body in [
        CanonicalValue::Integer(9007199254740992),
        CanonicalValue::Object(vec![
            ("a".into(), CanonicalValue::Null),
            ("a".into(), CanonicalValue::Null),
        ]),
        CanonicalValue::String("x".repeat(65537)),
        CanonicalValue::Array((0..1025).map(|_| CanonicalValue::Null).collect()),
    ] {
        assert!(CanonicalRequest::json(s, &params, &[], Some(&body)).is_err());
    }
    for query in [
        vec![("a", "1"), ("a", "2")],
        vec![("a", "\n")],
        vec![("", "x")],
    ] {
        assert!(CanonicalRequest::json(s, &params, &query, None).is_err());
    }
    for params in [
        vec![],
        vec![("unknown", EVENT)],
        vec![("eventId", "../bad")],
        vec![("eventId", EVENT), ("eventId", EVENT)],
        vec![("eventId", TARGET)],
    ] {
        assert!(CanonicalRequest::json(s, &params, &[], None).is_err());
    }
}
#[test]
fn no_pending_success_or_invalid_safe_reference() {
    assert!(Completion::Succeeded {
        reference: SafeReference {
            resource: Uuid::new_v4(),
            revision: 1
        },
        status: 202
    }
    .validate()
    .is_err());
    assert!(SafeReference {
        resource: Uuid::new_v4(),
        revision: 0
    }
    .validate()
    .is_err());
    assert_eq!(
        ReceiptState::parse("EXPIRED"),
        Err(CommandError::InvalidState)
    );
    assert_eq!(
        SafeFailure::parse("token=secret"),
        Err(CommandError::InvalidState)
    );
}
#[test]
fn binary_body_is_bound_and_cannot_use_json_or_unkeyed_fallback() {
    let s = scope("putConcertUploadBytes");
    let params = [("eventId", EVENT), ("mediaId", TARGET)];
    let ring = keys();
    let a = ring
        .sign(&CanonicalRequest::binary(s, &params, &[], b"a").unwrap())
        .unwrap();
    let b = ring
        .sign(&CanonicalRequest::binary(s, &params, &[], b"b").unwrap())
        .unwrap();
    assert!(a != b);
    assert!(CanonicalRequest::json(s, &params, &[], Some(&CanonicalValue::Null)).is_err());
    assert!(CanonicalRequest::binary(
        scope("createConcertParty"),
        &[("eventId", EVENT)],
        &[],
        b"a"
    )
    .is_err());
    assert!(CanonicalRequest::binary(s, &params, &[], &vec![0; 10 * 1024 * 1024 + 1]).is_err());
    assert!(Fingerprint::from_storage(&format!("sha256:{}", "0".repeat(64))).is_err());
}
#[test]
fn keys_are_injected_bounded_unambiguous_and_rotation_verifies_stored_version() {
    for id in [
        "",
        "bad:key",
        "bad/key",
        "bad key",
        "012345678901234567890123456789012",
    ] {
        assert!(SigningKey::new(id, &[1; 32]).is_err());
    }
    assert!(SigningKey::new("valid", &[1; 31]).is_err());
    assert!(FingerprintKeys::new(
        SigningKey::new("same", &[1; 32]).unwrap(),
        vec![SigningKey::new("same", &[2; 32]).unwrap()]
    )
    .is_err());
    assert!(FingerprintKeys::new(
        SigningKey::new("active", &[1; 32]).unwrap(),
        (0..32)
            .map(|i| SigningKey::new(&format!("old{i}"), &[2; 32]).unwrap())
            .collect()
    )
    .is_err());
    let request = CanonicalRequest::json(
        scope("createConcertParty"),
        &[("eventId", EVENT)],
        &[],
        Some(&CanonicalValue::String("123456".into())),
    )
    .unwrap();
    let old = keys().sign(&request).unwrap();
    let rotated = FingerprintKeys::new(
        SigningKey::new("test_v2", &[2; 32]).unwrap(),
        vec![SigningKey::new("test_v1", &[0x0b; 32]).unwrap()],
    )
    .unwrap();
    assert!(rotated.verify(&request, &old).unwrap());
    assert!(rotated.sign(&request).unwrap() != old);
    let lost = FingerprintKeys::new(SigningKey::new("test_v2", &[2; 32]).unwrap(), vec![]).unwrap();
    assert_eq!(lost.verify(&request, &old), Err(CommandError::Unavailable));
    let stored = old.as_storage();
    assert!(!stored.contains("123456"));
    assert!(Fingerprint::from_storage(&stored).unwrap() == old);
}
#[test]
fn mac_binds_actor_event_operation_key_and_path() {
    let s = scope("createConcertParty");
    let ring = keys();
    let base = ring
        .sign(&CanonicalRequest::json(s, &[("eventId", EVENT)], &[], None).unwrap())
        .unwrap();
    for other in [
        Scope {
            actor: Uuid::new_v4(),
            ..s
        },
        Scope {
            event: Uuid::new_v4(),
            ..s
        },
        Scope {
            key: Uuid::new_v4(),
            ..s
        },
        Scope {
            operation: RegisteredOperation::find("setConcertSettings").unwrap(),
            ..s
        },
    ] {
        let value = ring
            .sign(
                &CanonicalRequest::json(other, &[("eventId", &other.event.to_string())], &[], None)
                    .unwrap(),
            )
            .unwrap();
        assert!(value != base);
    }
    let s = scope("updateConcertParty");
    let a = ring
        .sign(
            &CanonicalRequest::json(s, &[("eventId", EVENT), ("partyId", TARGET)], &[], None)
                .unwrap(),
        )
        .unwrap();
    let b = ring
        .sign(
            &CanonicalRequest::json(s, &[("eventId", EVENT), ("partyId", EVENT)], &[], None)
                .unwrap(),
        )
        .unwrap();
    assert!(a != b);
}
#[test]
fn hmac_sha256_known_answer_and_low_entropy_guess_cannot_match_unkeyed_digest() {
    use hmac::{Hmac, Mac};
    use sha2::{Digest, Sha256};
    // RFC 4231 test case 6 (131-byte key) exercises the retained implementation.
    let mut mac = Hmac::<Sha256>::new_from_slice(&[0xaa; 131]).unwrap();
    mac.update(b"Test Using Larger Than Block-Size Key - Hash Key First");
    assert_eq!(
        format!("{:x}", mac.finalize().into_bytes()),
        "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
    );
    let body = CanonicalValue::Object(vec![(
        "code".into(),
        CanonicalValue::String("123456".into()),
    )]);
    let value = fingerprint(&body, &[]).as_storage();
    // Fixed canonical framing answer independently calculated with Python's hmac.
    assert!(value == "hmac-sha256:v1:test_v1:d0d44e89bdaeecfd82ecc08bb5988aadaed7b95a5060e32fd7aead3a7bd2539c");
    let s = scope("createConcertParty");
    let mut guessed_frame = Vec::new();
    for part in [
        b"MCR-BE15-HMAC-v1".as_slice(),
        s.actor.as_bytes(),
        s.event.as_bytes(),
        b"createConcertParty",
        s.key.as_bytes(),
        b"POST",
        b"/api/concert/events/11111111-1111-4111-8111-111111111111/parties",
        b"{}",
        b"JSON:{\"code\":\"123456\"}",
    ] {
        guessed_frame.extend((part.len() as u64).to_be_bytes());
        guessed_frame.extend(part);
    }
    // The entire canonical input is known to this attacker, but no server key.
    let guess = format!("{:x}", Sha256::digest(&guessed_frame));
    assert!(!value.ends_with(&guess));
}
