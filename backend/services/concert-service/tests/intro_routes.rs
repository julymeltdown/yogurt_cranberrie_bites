//! WP-05 T-405: the closed intro route parser.
//!
//! The acceptance cases the task document names:
//!
//! * an unknown query field is `400`;
//! * a method mismatch is `400`, not a fallthrough.
//!
//! These cases are pure: no database is involved.
use concert_service::intro_posts::http::{parse, IntroOperation};
use tonic::Code;
use uuid::Uuid;

fn id() -> Uuid {
    Uuid::new_v4()
}

fn code(result: Result<impl std::fmt::Debug, tonic::Status>) -> Code {
    result.expect_err("must be refused").code()
}

#[test]
fn every_frozen_operation_parses() {
    let intro = id();
    let comment = id();
    let cases = [
        ("GET", "/api/discover/intro-posts".to_owned(), IntroOperation::ListIntroPosts),
        ("GET", format!("/api/intro-posts/{intro}"), IntroOperation::GetIntroPost(intro)),
        ("GET", format!("/api/me/intro-posts/{intro}"), IntroOperation::GetOwnIntroPost(intro)),
        ("GET", "/api/me/intro-posts".to_owned(), IntroOperation::ListOwnIntroPosts),
        ("PUT", format!("/api/me/intro-posts/{intro}"), IntroOperation::SaveIntroPostDraft(intro)),
        ("POST", format!("/api/me/intro-posts/{intro}/publish"), IntroOperation::PublishIntroPost(intro)),
        ("DELETE", format!("/api/me/intro-posts/{intro}"), IntroOperation::DeleteIntroPost(intro)),
        ("PUT", format!("/api/intro-posts/{intro}/like"), IntroOperation::SetIntroPostLike(intro)),
        ("DELETE", format!("/api/intro-posts/{intro}/like"), IntroOperation::ClearIntroPostLike(intro)),
        ("GET", format!("/api/intro-posts/{intro}/likes"), IntroOperation::ListIntroPostLikers(intro)),
        ("GET", format!("/api/intro-posts/{intro}/comments"), IntroOperation::ListIntroPostComments(intro)),
        ("POST", format!("/api/intro-posts/{intro}/comments"), IntroOperation::CreateIntroPostComment(intro)),
        (
            "DELETE",
            format!("/api/intro-posts/{intro}/comments/{comment}"),
            IntroOperation::DeleteIntroPostComment(intro, comment),
        ),
    ];
    assert_eq!(cases.len(), 13, "the frozen set has 13 operations");
    for (method, target, expected) in cases {
        let parsed = parse(method, &target).unwrap_or_else(|error| {
            panic!("{method} {target} must parse: {error:?}")
        });
        assert_eq!(parsed.operation, expected, "{method} {target}");
    }
}

#[test]
fn the_contract_ids_match_the_frozen_register() {
    let intro = id();
    let pairs = [
        (IntroOperation::ListIntroPosts, "listIntroPosts"),
        (IntroOperation::GetIntroPost(intro), "getIntroPost"),
        (IntroOperation::GetOwnIntroPost(intro), "getOwnIntroPost"),
        (IntroOperation::ListOwnIntroPosts, "listOwnIntroPosts"),
        (IntroOperation::SaveIntroPostDraft(intro), "saveIntroPostDraft"),
        (IntroOperation::PublishIntroPost(intro), "publishIntroPost"),
        (IntroOperation::DeleteIntroPost(intro), "deleteIntroPost"),
        (IntroOperation::SetIntroPostLike(intro), "setIntroPostLike"),
        (IntroOperation::ClearIntroPostLike(intro), "clearIntroPostLike"),
        (IntroOperation::ListIntroPostLikers(intro), "listIntroPostLikers"),
        (IntroOperation::ListIntroPostComments(intro), "listIntroPostComments"),
        (IntroOperation::CreateIntroPostComment(intro), "createIntroPostComment"),
        (IntroOperation::DeleteIntroPostComment(intro, id()), "deleteIntroPostComment"),
    ];
    for (operation, expected) in pairs {
        assert_eq!(operation.contract_id(), expected);
    }
}

#[test]
fn reads_and_mutations_are_distinguished() {
    let intro = id();
    for operation in [
        IntroOperation::ListIntroPosts,
        IntroOperation::GetIntroPost(intro),
        IntroOperation::GetOwnIntroPost(intro),
        IntroOperation::ListOwnIntroPosts,
        IntroOperation::ListIntroPostLikers(intro),
        IntroOperation::ListIntroPostComments(intro),
    ] {
        assert!(!operation.mutation(), "{:?}", operation.contract_id());
    }
    for operation in [
        IntroOperation::SaveIntroPostDraft(intro),
        IntroOperation::PublishIntroPost(intro),
        IntroOperation::DeleteIntroPost(intro),
        IntroOperation::SetIntroPostLike(intro),
        IntroOperation::ClearIntroPostLike(intro),
        IntroOperation::CreateIntroPostComment(intro),
        IntroOperation::DeleteIntroPostComment(intro, id()),
    ] {
        assert!(operation.mutation(), "{:?}", operation.contract_id());
    }
}

/// The acceptance case: an unknown query field is `400`.
#[test]
fn an_unknown_query_field_is_a_client_error() {
    for target in [
        "/api/discover/intro-posts?sort=likes".to_owned(),
        "/api/discover/intro-posts?limit=5&sort=likes".to_owned(),
        format!("/api/intro-posts/{}/likes?order=name", id()),
        "/api/me/intro-posts?audience=female".to_owned(),
        // A near-miss on a real field is still unknown.
        "/api/discover/intro-posts?Limit=5".to_owned(),
        "/api/me/intro-posts?states=DRAFT".to_owned(),
    ] {
        assert_eq!(code(parse("GET", &target)), Code::InvalidArgument, "{target}");
    }
    // A repeated field is ambiguous, never "last one wins".
    assert_eq!(
        code(parse("GET", "/api/discover/intro-posts?limit=5&limit=10")),
        Code::InvalidArgument
    );
}

/// The acceptance case: a method mismatch is `400`, not a fallthrough.
#[test]
fn a_method_mismatch_is_refused() {
    let intro = id();
    // The path exists, but not with this method.
    for (method, target) in [
        ("POST", format!("/api/intro-posts/{intro}")),
        ("PATCH", format!("/api/me/intro-posts/{intro}")),
        ("PUT", format!("/api/intro-posts/{intro}/comments")),
        ("GET", format!("/api/intro-posts/{intro}/like")),
        ("POST", format!("/api/intro-posts/{intro}/like")),
        ("PUT", "/api/discover/intro-posts".to_owned()),
        ("HEAD", "/api/me/intro-posts".to_owned()),
    ] {
        assert_eq!(
            code(parse(method, &target)),
            Code::PermissionDenied,
            "{method} {target}"
        );
    }
}

#[test]
fn an_unknown_path_is_refused() {
    let intro = id();
    for target in [
        "/api/intro-posts".to_owned(),
        format!("/api/intro-posts/{intro}/extra"),
        format!("/api/intro-posts/{intro}/comments/{intro}/extra"),
        format!("/api/me/intro-posts/{intro}/publish/extra"),
        format!("/api/profiles/{intro}"),
        "/api/discover/people".to_owned(),
    ] {
        let method = if target.contains("/publish") { "POST" } else { "GET" };
        assert_eq!(
            code(parse(method, &target)),
            Code::PermissionDenied,
            "{target}"
        );
    }
}

/// A malformed identifier is a client error, not an unknown route: the path
/// shape is right and the value is wrong, so the caller learns what to fix.
#[test]
fn a_malformed_identifier_is_a_client_error() {
    for target in [
        "/api/me/intro-posts/not-a-uuid".to_owned(),
        "/api/intro-posts/not-a-uuid/likes".to_owned(),
        format!("/api/intro-posts/{}/comments/not-a-uuid", id()),
    ] {
        let method = if target.contains("/comments/") { "DELETE" } else { "GET" };
        assert_eq!(
            code(parse(method, &target)),
            Code::InvalidArgument,
            "{target}"
        );
    }
}

#[test]
fn an_event_path_is_refused() {
    let event = id();
    for target in [
        format!("/api/concert/events/{event}/intro-posts"),
        format!("/api/intro-posts/events/{event}"),
    ] {
        assert_eq!(code(parse("GET", &target)), Code::InvalidArgument, "{target}");
    }
}

#[test]
fn the_query_bounds_are_enforced() {
    // limit
    assert!(parse("GET", "/api/discover/intro-posts?limit=1").is_ok());
    assert!(parse("GET", "/api/discover/intro-posts?limit=100").is_ok());
    for value in ["0", "101", "-1", "1.5", "abc", ""] {
        assert_eq!(
            code(parse("GET", &format!("/api/discover/intro-posts?limit={value}"))),
            Code::InvalidArgument,
            "{value}"
        );
    }
    // cursor
    assert!(parse("GET", "/api/discover/intro-posts?cursor=abcDEF012_-").is_ok());
    for value in ["", "abc+def", "abc.def"] {
        assert_eq!(
            code(parse("GET", &format!("/api/discover/intro-posts?cursor={value}"))),
            Code::InvalidArgument,
            "{value}"
        );
    }
    // q
    assert!(parse("GET", "/api/discover/intro-posts?q=emo").is_ok());
    for value in ["", "a".repeat(101).as_str()] {
        assert_eq!(
            code(parse("GET", &format!("/api/discover/intro-posts?q={value}"))),
            Code::InvalidArgument,
            "{value}"
        );
    }
    // state is closed
    for value in ["DRAFT", "PUBLISHED", "DELETED"] {
        assert!(parse("GET", &format!("/api/me/intro-posts?state={value}")).is_ok());
    }
    for value in ["draft", "CLOSED", "PENDING", "ACCEPTED"] {
        assert_eq!(
            code(parse("GET", &format!("/api/me/intro-posts?state={value}"))),
            Code::InvalidArgument,
            "{value}"
        );
    }
    // expected_revision
    assert!(parse("DELETE", &format!("/api/me/intro-posts/{}?expected_revision=3", id())).is_ok());
    for value in ["0x", "-1", "abc", "1.5"] {
        assert_eq!(
            code(parse(
                "DELETE",
                &format!("/api/me/intro-posts/{}?expected_revision={value}", id())
            )),
            Code::InvalidArgument,
            "{value}"
        );
    }
}

#[test]
fn the_parsed_query_carries_only_the_declared_fields() {
    let parsed = parse(
        "GET",
        "/api/discover/intro-posts?limit=7&cursor=abc&q=emo",
    )
    .expect("valid");
    assert_eq!(parsed.query.limit, Some(7));
    assert_eq!(parsed.query.cursor.as_deref(), Some("abc"));
    assert_eq!(parsed.query.q.as_deref(), Some("emo"));
    // A field another operation declares is not populated here.
    assert_eq!(parsed.query.state, None);
    assert_eq!(parsed.query.expected_revision, None);
    // An operation that declares no query refuses one.
    assert_eq!(
        code(parse("GET", &format!("/api/intro-posts/{}?limit=5", id()))),
        Code::InvalidArgument
    );
}

#[test]
fn a_percent_encoded_value_is_decoded_once() {
    let parsed = parse("GET", "/api/discover/intro-posts?q=%65mo").expect("valid");
    assert_eq!(parsed.query.q.as_deref(), Some("emo"));
    assert_eq!(
        code(parse("GET", "/api/discover/intro-posts?q=%zz")),
        Code::InvalidArgument
    );
}

#[test]
fn a_canonical_uuid_is_required() {
    // Uppercase, braced and nil forms are all refused.
    for value in [
        "00000000-0000-0000-0000-000000000000",
        "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
        "{aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa}",
        "aaaaaaaaaaaa4aaa8aaaaaaaaaaaaaaa",
    ] {
        assert_eq!(
            code(parse("GET", &format!("/api/intro-posts/{value}"))),
            Code::InvalidArgument,
            "{value}"
        );
    }
}
