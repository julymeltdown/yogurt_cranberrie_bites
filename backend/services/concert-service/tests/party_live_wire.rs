use chrono::{Duration, Utc};
use concert_domain::community::party::{MemberState, Party, PartyFields, PartyState};
use concert_service::party_live::{
    operation_for_fixture, parse_verified_body_for_fixture, PartyCursorBinding, PartyLiveError,
};
use uuid::Uuid;

#[test]
fn party_fixture_is_closed_and_preserves_member_capacity_and_revision_truth() {
    let now = Utc::now();
    let event = Uuid::new_v4();
    let owner = Uuid::new_v4();
    let party = Party::create(
        Uuid::new_v4(),
        event,
        owner,
        PartyFields {
            title: "Queued together".into(),
            description: "East gate meetup".into(),
            ticket_section: Some("A".into()),
            capacity: 2,
            meeting_starts_at: now + Duration::hours(1),
            meeting_ends_at: now + Duration::hours(2),
            requirements: String::new(),
            public_place_id: Uuid::new_v4(),
        },
        now,
    )
    .unwrap();
    assert_eq!(party.state, PartyState::Open);
    assert_eq!(party.revision, 1);
    assert_eq!(party.member_count(), 1);
    assert!(matches!(party.members[0].state, MemberState::Active));
    assert!(party.applications.is_empty());
}

#[test]
fn query_fixture_binding_preserves_only_canonical_scope_and_filters() {
    let binding = PartyCursorBinding {
        actor: Uuid::new_v4(),
        event: Uuid::new_v4(),
        operation: "listConcertParties".into(),
        collection: "parties".into(),
        ticket_section: None,
        state: None,
        has_space: None,
        starts_after: None,
        starts_before: None,
        limit: 20,
    };
    let serialized = serde_json::to_string(&binding).unwrap();
    assert!(serialized.contains(&binding.actor.to_string()));
    assert!(serialized.contains("listConcertParties"));
}

#[test]
fn errors_map_to_the_closed_contract_statuses() {
    for (error, status, reason) in [
        (PartyLiveError::InvalidInput, 400, "INVALID_INPUT"),
        (PartyLiveError::Denied, 403, "PERMISSION_DENIED"),
        (
            PartyLiveError::ResourceUnavailable,
            404,
            "RESOURCE_UNAVAILABLE",
        ),
        (
            PartyLiveError::DependencyUnavailable,
            503,
            "DEPENDENCY_UNAVAILABLE",
        ),
    ] {
        assert_eq!(error.status(), status);
        assert_eq!(error.reason(), reason);
    }
}

#[test]
fn create_requires_exact_json_key_and_rejects_unknown_and_duplicate_fields() {
    let event = Uuid::new_v4();
    let key = Uuid::new_v4();
    let now = Utc::now() + Duration::hours(1);
    let body = serde_json::json!({
        "title":"Queued together",
        "description":"East gate meetup",
        "ticket_section":"A",
        "capacity":2,
        "meeting_starts_at":now,
        "meeting_ends_at":now+Duration::hours(1),
        "requirements":"",
        "public_place_id":Uuid::new_v4()
    });
    let bytes = serde_json::to_vec(&body).unwrap();
    let target = format!("/api/concert/events/{event}/parties");
    assert!(parse_verified_body_for_fixture(
        event,
        "POST",
        &target,
        &bytes,
        "application/json",
        &key.to_string()
    )
    .is_ok());
    for (method, body, content_type, idempotency) in [
        ("PATCH", bytes.clone(), "application/json", key.to_string()),
        ("POST", bytes.clone(), "text/plain", key.to_string()),
        ("POST", bytes.clone(), "application/json", String::new()),
        (
            "POST",
            br#"{"title":"a","title":"b","description":"","ticket_section":null,"capacity":2,"meeting_starts_at":"2090-01-01T00:00:00Z","meeting_ends_at":"2090-01-02T00:00:00Z","requirements":"","public_place_id":"00000000-0000-4000-8000-000000000001"}"#.to_vec(),
            "application/json",
            key.to_string(),
        ),
        (
            "POST",
            br#"{"title":"a","description":"","ticket_section":null,"capacity":2,"meeting_starts_at":"2090-01-01T00:00:00Z","meeting_ends_at":"2090-01-02T00:00:00Z","requirements":"","public_place_id":"00000000-0000-4000-8000-000000000001","extra":true}"#.to_vec(),
            "application/json",
            key.to_string(),
        ),
    ] {
        assert!(parse_verified_body_for_fixture(
            event,
            method,
            &target,
            &body,
            content_type,
            &idempotency
        )
        .is_err());
    }
}

#[test]
fn list_parses_every_closed_filter_and_rejects_duplicate_or_unknown_query_fields() {
    let event = Uuid::new_v4();
    let target = format!(
        "/api/concert/events/{event}/parties?limit=10&ticket_section=A&state=OPEN&has_space=false&starts_after=2090-01-01T00%3A00%3A00Z&starts_before=2090-01-02T00%3A00%3A00Z"
    );
    assert!(parse_verified_body_for_fixture(event, "GET", &target, &[], "", "").is_ok());
    for query in [
        "limit=10&limit=10",
        "extra=true",
        "state=open",
        "has_space=1",
        "ticket_section=",
        "starts_after=not-a-time",
        "limit=0",
        "limit=101",
    ] {
        let target = format!("/api/concert/events/{event}/parties?{query}");
        assert!(
            parse_verified_body_for_fixture(event, "GET", &target, &[], "", "").is_err(),
            "{query}"
        );
    }
    let detail = format!("/api/concert/events/{event}/parties/{}", Uuid::new_v4());
    assert!(parse_verified_body_for_fixture(event, "GET", &detail, &[], "", "").is_ok());
    assert!(parse_verified_body_for_fixture(
        event,
        "GET",
        &format!("{detail}?limit=1"),
        &[],
        "",
        ""
    )
    .is_err());
}

#[test]
fn detail_binding_rejects_noncanonical_and_foreign_paths() {
    let actor = Uuid::new_v4();
    let event = Uuid::new_v4();
    let party = Uuid::new_v4();
    let deadline = platform::actor_delegation::authority_deadline::AuthorityRpcDeadline::new(
        Utc::now().timestamp_micros() + 2_000_000,
        Duration::seconds(3).to_std().unwrap(),
    )
    .unwrap();
    let valid = format!("/api/concert/events/{event}/parties/{party}");
    assert!(concert_service::party_live::PartyLiveBinding::test_binding(
        actor,
        event,
        Uuid::new_v4(),
        Uuid::new_v4(),
        deadline,
        "getConcertParty",
        "GET",
        valid.clone(),
        &[],
        None,
        Some([1; 32]),
    )
    .check()
    .is_ok());
    for target in [
        format!("{valid}/extra"),
        format!("{valid}?limit=1"),
        format!("{valid}#fragment"),
        format!("/api/concert/events/{event}/parties/not-a-uuid"),
        format!("/api/concert/events/{event}/other/{party}"),
    ] {
        assert!(concert_service::party_live::PartyLiveBinding::test_binding(
            actor,
            event,
            Uuid::new_v4(),
            Uuid::new_v4(),
            deadline,
            "getConcertParty",
            "GET",
            target,
            &[],
            None,
            Some([1; 32]),
        )
        .check()
        .is_err());
    }
}

#[test]
fn closed_route_parser_derives_exact_party_operations_and_rejects_unlisted_routes() {
    let event = Uuid::new_v4();
    let party = Uuid::new_v4();
    let root = format!("/api/concert/events/{event}/parties");
    let detail = format!("{root}/{party}");
    assert_eq!(
        operation_for_fixture(event, "GET", &root).unwrap(),
        "listConcertParties"
    );
    assert_eq!(
        operation_for_fixture(event, "GET", &detail).unwrap(),
        "getConcertParty"
    );
    assert_eq!(
        operation_for_fixture(event, "POST", &root).unwrap(),
        "createConcertParty"
    );
    let application = Uuid::new_v4();
    let user = Uuid::new_v4();
    assert_eq!(
        operation_for_fixture(event, "POST", &format!("{detail}/applications")).unwrap(),
        "applyConcertParty"
    );
    assert_eq!(
        operation_for_fixture(
            event,
            "POST",
            &format!("/api/concert/events/{event}/party-applications/{application}/decision")
        )
        .unwrap(),
        "decideConcertApplication"
    );
    assert_eq!(
        operation_for_fixture(
            event,
            "DELETE",
            &format!(
                "/api/concert/events/{event}/party-applications/{application}?expected_revision=1"
            )
        )
        .unwrap(),
        "cancelConcertApplication"
    );
    assert_eq!(
        operation_for_fixture(
            event,
            "DELETE",
            &format!("{detail}/members/{user}?expected_revision=1&party_revision=1")
        )
        .unwrap(),
        "removeConcertMember"
    );
    assert_eq!(
        operation_for_fixture(event, "POST", &format!("{detail}/cancel")).unwrap(),
        "cancelConcertParty"
    );
    for (method, target) in [
        ("POST", detail.clone()),
        ("GET", format!("{root}/applications")),
        ("PATCH", detail.clone()),
        ("GET", format!("{detail}/members")),
        (
            "POST",
            format!("{detail}/applications?extra=true")
                .as_str()
                .to_owned(),
        ),
        (
            "DELETE",
            format!("/api/concert/events/{event}/party-applications/{application}"),
        ),
        (
            "DELETE",
            format!("{detail}/members/{user}?expected_revision=1"),
        ),
        (
            "GET",
            format!("/api/concert/events/{}/parties", Uuid::new_v4()),
        ),
    ] {
        assert!(operation_for_fixture(event, method, &target).is_err());
    }
}

#[test]
fn mutation_body_parser_requires_exact_closed_c04_shapes() {
    let event = Uuid::new_v4();
    let party = Uuid::new_v4();
    let application = Uuid::new_v4();
    let user = Uuid::new_v4();
    let key = Uuid::new_v4();
    let root = format!("/api/concert/events/{event}");
    let apply = format!("{root}/parties/{party}/applications");
    assert!(parse_verified_body_for_fixture(
        event,
        "POST",
        &apply,
        br#"{"party_revision":1,"note":"See you there"}"#,
        "application/json",
        &key.to_string()
    )
    .is_ok());
    let decision = format!("{root}/party-applications/{application}/decision");
    assert!(parse_verified_body_for_fixture(
        event,
        "POST",
        &decision,
        br#"{"expected_revision":1,"party_revision":2,"decision":"APPROVED"}"#,
        "application/json",
        &key.to_string()
    )
    .is_ok());
    assert!(parse_verified_body_for_fixture(
        event,
        "POST",
        &format!("{root}/parties/{party}/cancel"),
        br#"{"expected_revision":3}"#,
        "application/json",
        &key.to_string()
    )
    .is_ok());
    for body in [
        br#"{"party_revision":1}"#.as_slice(),
        br#"{"party_revision":1,"note":"a","extra":true}"#.as_slice(),
        br#"{"party_revision":1,"note":"a","note":"b"}"#.as_slice(),
    ] {
        assert!(parse_verified_body_for_fixture(
            event,
            "POST",
            &apply,
            body,
            "application/json",
            &key.to_string()
        )
        .is_err());
    }
    assert!(parse_verified_body_for_fixture(
        event,
        "DELETE",
        &format!("{root}/parties/{party}/members/{user}?expected_revision=1&party_revision=2"),
        &[],
        "",
        &key.to_string()
    )
    .is_ok());
    for (body, content_type, idempotency) in [
        (&[][..], "", "".to_owned()),
        (&b"{}"[..], "", key.to_string()),
        (&[][..], "application/json", key.to_string()),
    ] {
        assert!(parse_verified_body_for_fixture(
            event,
            "DELETE",
            &format!("{root}/parties/{party}/members/{user}?expected_revision=1&party_revision=2"),
            body,
            content_type,
            &idempotency,
        )
        .is_err());
    }
    assert!(parse_verified_body_for_fixture(
        event,
        "DELETE",
        &format!("{root}/party-applications/{application}?expected_revision=2"),
        &[],
        "",
        &key.to_string()
    )
    .is_ok());
    assert!(parse_verified_body_for_fixture(
        event,
        "DELETE",
        &format!("{root}/party-applications/{application}?expected_revision=2&expected_revision=2"),
        &[],
        "",
        &key.to_string()
    )
    .is_err());
}
