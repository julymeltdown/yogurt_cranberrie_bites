use concert_application::commands::*;
const EVENT: &str = "11111111-1111-4111-8111-111111111111";
const TARGET: &str = "22222222-2222-4222-8222-222222222222";
fn hash(body: &CanonicalValue, query: &[(&str, &str)]) -> RequestHash {
    RequestHash::canonical(
        RegisteredOperation::find("createConcertParty").unwrap(),
        &[("eventId", EVENT)],
        query,
        Some(body),
    )
    .unwrap()
}
#[test]
fn registry_rejects_arbitrary_reads_and_operations() {
    assert_eq!(OPERATIONS.len(), 59);
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
fn canonical_digest_binds_values_not_object_or_query_order() {
    let a = CanonicalValue::Object(vec![
        ("z".into(), CanonicalValue::Integer(1)),
        ("a".into(), CanonicalValue::String("secret".into())),
    ]);
    let b = CanonicalValue::Object(vec![
        ("a".into(), CanonicalValue::String("secret".into())),
        ("z".into(), CanonicalValue::Integer(1)),
    ]);
    assert!(hash(&a, &[("b", "2"), ("a", "1")]) == hash(&b, &[("a", "1"), ("b", "2")]));
    assert!(hash(&a, &[]) != hash(&CanonicalValue::Null, &[]));
    assert!(
        RequestHash::canonical(
            RegisteredOperation::find("createConcertParty").unwrap(),
            &[("eventId", EVENT)],
            &[],
            None
        )
        .unwrap()
            != hash(&CanonicalValue::Null, &[])
    );
    assert!(hash(&a, &[]) != hash(&a, &[("a", "1")]));
    let op = RegisteredOperation::find("updateConcertParty").unwrap();
    let first = RequestHash::canonical(
        op,
        &[("eventId", EVENT), ("partyId", TARGET)],
        &[],
        Some(&a),
    )
    .unwrap();
    let second =
        RequestHash::canonical(op, &[("eventId", EVENT), ("partyId", EVENT)], &[], Some(&a))
            .unwrap();
    assert!(first != second);
    assert!(first != hash(&a, &[]));
    let stored = first.as_storage();
    assert_eq!(stored.len(), 71);
    assert!(RequestHash::from_storage(&stored).unwrap() == first);
    assert!(!stored.contains("secret"));
}
#[test]
fn canonical_rejects_duplicate_oversized_unsafe_or_ambiguous_input() {
    let op = RegisteredOperation::find("createConcertParty").unwrap();
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
        assert!(RequestHash::canonical(op, &params, &[], Some(&body)).is_err());
    }
    for query in [
        vec![("a", "1"), ("a", "2")],
        vec![("a", "\n")],
        vec![("", "x")],
    ] {
        assert!(RequestHash::canonical(op, &params, &query, Some(&CanonicalValue::Null)).is_err());
    }
    for params in [
        vec![],
        vec![("unknown", EVENT)],
        vec![("eventId", "../bad")],
        vec![("eventId", EVENT), ("eventId", EVENT)],
    ] {
        assert!(op.path(&params).is_err());
    }
    assert!(RequestHash::from_storage(&format!("sha256:{}", "A".repeat(64))).is_err());
}
#[test]
fn no_pending_success_or_invalid_safe_reference() {
    assert!(Completion::Succeeded {
        reference: SafeReference {
            resource: uuid::Uuid::new_v4(),
            revision: 1
        },
        status: 202
    }
    .validate()
    .is_err());
    assert!(SafeReference {
        resource: uuid::Uuid::new_v4(),
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
fn canonical_types_absence_and_array_order_have_distinct_hashes() {
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
    let hashes: std::collections::BTreeSet<_> =
        values.iter().map(|v| hash(v, &[]).as_storage()).collect();
    assert_eq!(hashes.len(), values.len());
    assert!(hash(&CanonicalValue::Null, &[]) != hash(&CanonicalValue::Null, &[("a", "")]));
}

#[test]
fn binary_digest_binds_exact_bytes_with_separate_body_type() {
    let op = RegisteredOperation::find("putConcertUploadBytes").unwrap();
    let params = [("eventId", EVENT), ("mediaId", TARGET)];
    assert!(op.path(&params).is_ok());
    let a = RequestHash::binary(op, &params, &[], b"a").unwrap();
    let b = RequestHash::binary(op, &params, &[], b"b").unwrap();
    assert!(a != b);
    assert!(
        RequestHash::canonical(op, &params, &[], Some(&CanonicalValue::String("a".into())))
            .is_err()
    );
    assert!(RequestHash::binary(
        RegisteredOperation::find("createConcertParty").unwrap(),
        &[("eventId", EVENT)],
        &[],
        b"a"
    )
    .is_err());
    assert!(RequestHash::binary(op, &params, &[], &vec![0; 10 * 1024 * 1024 + 1]).is_err());
}
