//! Frozen C04 parser cases only. These do not certify owner or HTTP acceptance.
use concert_service::{
    notification_live::{NotificationOperation as Op, NotificationRequest},
    product_rpc::proto::ProductRequest,
};
use tonic::Code;
use uuid::Uuid;

fn request(method: &str, suffix: &str, body: &[u8]) -> ProductRequest {
    let event = Uuid::new_v4();
    ProductRequest {
        user_id: Uuid::new_v4().to_string(),
        event_id: event.to_string(),
        session_family_id: Uuid::new_v4().to_string(),
        request_id: Uuid::new_v4().to_string(),
        http_method: method.into(),
        http_target: format!("/api/concert/events/{event}/me/notifications{suffix}"),
        http_body: body.into(),
        content_type: if method == "PATCH" {
            "application/json".into()
        } else {
            String::new()
        },
        idempotency_key: if method == "PATCH" {
            Uuid::new_v4().to_string()
        } else {
            String::new()
        },
        ..Default::default()
    }
}
fn invalid(r: &ProductRequest) {
    assert!(
        matches!(NotificationRequest::parse(r), Err(e) if e.code() == Code::InvalidArgument),
        "request must be refused locally"
    );
}
#[test]
fn all_four_frozen_routes_and_query_bounds() {
    let id = Uuid::new_v4();
    for (method, suffix, body, expected) in [
        ("GET", "".to_owned(), b"".as_slice(), Op::List),
        (
            "GET",
            "?limit=100&unread_only=true&cursor=opaque_Ab9-".into(),
            b"".as_slice(),
            Op::List,
        ),
        (
            "GET",
            "/unread-count".into(),
            b"".as_slice(),
            Op::UnreadCount,
        ),
        (
            "PATCH",
            format!("/{id}/read"),
            b"{\"expected_revision\":1}".as_slice(),
            Op::MarkRead(id),
        ),
        (
            "PATCH",
            format!("/{id}/read"),
            b"{\"expected_revision\":9007199254740991}".as_slice(),
            Op::MarkRead(id),
        ),
        (
            "PATCH",
            "/read-all".into(),
            b"{}".as_slice(),
            Op::MarkAllRead,
        ),
    ] {
        assert_eq!(
            NotificationRequest::parse(&request(method, &suffix, body))
                .unwrap()
                .operation(),
            expected
        );
    }
    assert_eq!(Op::List.id(), "listConcertNotifications");
    assert_eq!(Op::UnreadCount.id(), "getConcertNotificationUnreadCount");
    assert_eq!(Op::MarkRead(id).id(), "markConcertNotification");
    assert_eq!(Op::MarkAllRead.id(), "markAllConcertNotifications");
}
#[test]
fn duplicate_encoded_unknown_and_malformed_query_are_closed() {
    for query in [
        "limit=0",
        "limit=101",
        "limit=-1",
        "limit=1.0",
        "limit=+1",
        "limit=4294967296",
        "limit=1&limit=2",
        "limit=1&%6cimit=2",
        "unreadOnly=true",
        "unread_only=1",
        "unread_only=True",
        "cursor=",
        "cursor=%252f",
        "cursor=%2f",
        "cursor=%00",
        "cursor=%ff",
        "cursor=%G0",
        "cursor=%",
        "limit",
        "limit=1&",
        "limit=1#fragment",
        "actor_id=arbitrary",
    ] {
        invalid(&request("GET", &format!("?{query}"), b""));
    }
    invalid(&request(
        "GET",
        &format!("?cursor={}", "a".repeat(1025)),
        b"",
    ));
    invalid(&request("GET", "/unread-count?limit=1", b""));
    invalid(&request("PATCH", "/read-all?limit=1", b"{}"));
    assert!(
        NotificationRequest::parse(&request("GET", "?%6cimit=%32&unread_only=false", b"")).is_ok()
    );
}
#[test]
fn closed_mutation_body_revision_type_and_idempotency() {
    let suffix = format!("/{}/read", Uuid::new_v4());
    for body in [
        r#"{}"#,
        r#"{"expected_revision":0}"#,
        r#"{"expected_revision":-1}"#,
        r#"{"expected_revision":9007199254740992}"#,
        r#"{"expected_revision":1.0}"#,
        r#"{"expected_revision":"1"}"#,
        r#"{"expected_revision":1,"payload":"secret"}"#,
        r#"{"expected_revision":1,"expected_revision":2}"#,
        "null",
        "[]",
        "{}{}",
    ] {
        invalid(&request("PATCH", &suffix, body.as_bytes()));
    }
    for body in [
        b"null".as_slice(),
        b"[]",
        b"{\"expected_revision\":1}",
        b"{\"actor_id\":null}",
        b"",
    ] {
        invalid(&request("PATCH", "/read-all", body));
    }
    for field in 0..5 {
        let mut r = request("PATCH", "/read-all", b"{}");
        match field {
            0 => r.idempotency_key.clear(),
            1 => r.idempotency_key = Uuid::nil().to_string(),
            2 => r.idempotency_key = Uuid::new_v4().simple().to_string(),
            3 => r.content_type = "application/json; charset=utf-8".into(),
            _ => r.http_body = vec![b' '; 8193],
        }
        invalid(&r);
    }
}
#[test]
fn event_actor_family_request_path_and_method_cannot_be_relabelled() {
    for field in 0..11 {
        let mut r = request("GET", "", b"");
        match field {
            0 => r.user_id = Uuid::nil().to_string(),
            1 => r.event_id = Uuid::new_v4().to_string(),
            2 => r.session_family_id = Uuid::nil().to_string(),
            3 => r.request_id = Uuid::new_v4().simple().to_string(),
            4 => r.http_method = "POST".into(),
            5 => r.http_target.push('/'),
            6 => r.http_target.push_str("#fragment"),
            7 => r.http_target = r.http_target.replace("/events/", "/events/%2f"),
            8 => r.http_body = b"{}".to_vec(),
            9 => r.content_type = "application/json".into(),
            _ => r.idempotency_key = Uuid::new_v4().to_string(),
        }
        invalid(&r);
    }
    invalid(&request(
        "PATCH",
        &format!("/{}/read", Uuid::nil()),
        b"{\"expected_revision\":1}",
    ));
}

#[path = "notification_product_support/mod.rs"]
mod support;
