use crate::social_live::protocol::*;
use serde_json::json;
use uuid::Uuid;
fn ids() -> (Uuid, Uuid, Uuid, Uuid) {
    (
        Uuid::from_u128(11),
        Uuid::from_u128(12),
        Uuid::from_u128(13),
        Uuid::from_u128(14),
    )
}
#[test]
fn canonical_contract_request_locations_cover_all_thirteen_operations() {
    let (actor, event, id, key) = ids();
    let key_text = key.to_string();
    let root = format!("/api/concert/events/{event}");
    let cases = [
        (
            "GET",
            "me/social-handles".to_string(),
            None,
            "listOwnConcertHandles",
        ),
        (
            "PUT",
            "me/social-handles/INSTAGRAM".into(),
            Some(json!({"expected_revision":0,"handle":"private.name"})),
            "setConcertHandle",
        ),
        (
            "DELETE",
            "me/social-handles/X?expected_revision=1".into(),
            None,
            "removeConcertHandle",
        ),
        (
            "POST",
            "social-requests".into(),
            Some(
                json!({"recipient_id":id,"intro_id":key,"intro_revision":1,"consents":[{"platform":"INSTAGRAM","requester_handle_version":1,"recipient_handle_version":1,"requester_consents":true}]}),
            ),
            "createConcertSocialRequest",
        ),
        (
            "GET",
            "me/social-requests?direction=INCOMING&state=PENDING&limit=100".into(),
            None,
            "listConcertSocialRequests",
        ),
        (
            "GET",
            format!("social-requests/{id}"),
            None,
            "getConcertSocialRequest",
        ),
        (
            "POST",
            format!("social-requests/{id}/decision"),
            Some(
                json!({"expected_revision":1,"decision":"APPROVED","approved_platforms":["INSTAGRAM"],"recipient_consents":true}),
            ),
            "decideConcertSocialRequest",
        ),
        (
            "DELETE",
            format!("social-requests/{id}?expected_revision=1"),
            None,
            "cancelConcertSocialRequest",
        ),
        (
            "POST",
            format!("social-requests/{id}/revoke"),
            Some(json!({"expected_revision":1})),
            "revokeConcertSocialRequest",
        ),
        (
            "GET",
            format!("social-requests/{id}/reveal?platform=INSTAGRAM"),
            None,
            "revealConcertSocialHandle",
        ),
        ("GET", "me/blocks".into(), None, "listConcertBlocks"),
        (
            "PUT",
            format!("me/blocks/{id}"),
            Some(json!({})),
            "blockConcertUser",
        ),
        (
            "DELETE",
            format!("me/blocks/{id}"),
            None,
            "unblockConcertUser",
        ),
    ];
    let frozen: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../contracts/domain/v1/operations.json"
    ))
    .unwrap();
    for (method, suffix, body, opid) in cases {
        let op = route(event, method, &format!("{root}/{suffix}")).unwrap();
        let body = body
            .map(|b| serde_json::to_vec(&b).unwrap())
            .unwrap_or_default();
        let parsed = SocialRequest::parse(
            actor,
            event,
            method,
            &format!("{root}/{suffix}"),
            &body,
            if body.is_empty() {
                ""
            } else {
                "application/json"
            },
            if op.mutation() { &key_text } else { "" },
        )
        .unwrap();
        assert_eq!(parsed.operation().id(), opid);
        let contract = frozen
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["operationId"] == opid)
            .unwrap();
        assert_eq!(
            contract["method"].as_str().unwrap().to_ascii_uppercase(),
            method
        );
        assert_eq!(contract["request_location"] == "body", !body.is_empty());
    }
}
#[test]
fn private_input_rejects_authority_fields_duplicates_and_nonconsent() {
    let (a, e, r, k) = ids();
    let target = format!("/api/concert/events/{e}/me/social-handles/INSTAGRAM");
    for body in [
        r#"{"expected_revision":0,"handle":"private","actor_id":"forged"}"#,
        r#"{"expected_revision":0,"handle":"first","handle":"second"}"#,
        r#"{"expected_revision":0,"handle":"https://secret.example"}"#,
        r#"{"expected_revision":9007199254740991,"handle":"private"}"#,
    ] {
        assert!(SocialRequest::parse(
            a,
            e,
            "PUT",
            &target,
            body.as_bytes(),
            "application/json",
            &k.to_string()
        )
        .is_err());
    }
    let target = format!("/api/concert/events/{e}/social-requests");
    for recipient in [a, r] {
        let body=serde_json::to_vec(&json!({"recipient_id":recipient,"intro_id":k,"intro_revision":1,"consents":[{"platform":"INSTAGRAM","requester_handle_version":1,"recipient_handle_version":1,"requester_consents":false}]})).unwrap();
        assert!(SocialRequest::parse(
            a,
            e,
            "POST",
            &target,
            &body,
            "application/json",
            &k.to_string()
        )
        .is_err());
    }
}
#[test]
fn malformed_query_path_and_read_identity_cannot_bypass_closed_route() {
    let (a, e, r, k) = ids();
    let base = format!("/api/concert/events/{e}/");
    for suffix in [
        "me/blocks?limit=0",
        "me/blocks?limit=1&limit=2",
        "me/blocks?%6cimit=1&limit=2",
        "me/blocks?cursor=%ff",
        "me/blocks?limit=%0a1",
        "me/blocks?owner=other",
        "me/social-handles/",
        "me/social-requests?state=DELETED",
        "me/social-requests?direction=ADMIN",
    ] {
        assert!(
            SocialRequest::parse(a, e, "GET", &format!("{base}{suffix}"), b"", "", "").is_err()
        );
    }
    assert!(SocialRequest::parse(
        a,
        e,
        "GET",
        &format!("{base}me/blocks"),
        b"",
        "",
        &k.to_string()
    )
    .is_err());
    assert!(route(e, "GET", &format!("{base}social-requests/{}", r.simple())).is_err());
    assert!(route(Uuid::new_v4(), "GET", &format!("{base}me/blocks")).is_err());
    assert!(canonical(&r.to_string().to_uppercase()).is_err());
}
