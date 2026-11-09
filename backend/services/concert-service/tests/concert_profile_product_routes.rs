//! The concert-hosted profile product transport: the closed route set.
//!
//! This is the transport half of the cross-database producer gap. Fifteen frozen
//! profile operations read tables that live in `mcr_concert`, so the concert
//! service serves them and the Gateway routes them here. The producer code
//! already existed (WP-06, WP-07, WP-08); what this test proves is that the
//! *transport* now reaches it, and that it reaches nothing else.
//!
//! The parser is the security boundary, so it is tested as one:
//!
//! * every one of the fifteen operations resolves to its own variant;
//! * every profile operation whose tables live in `mcr_profile` is refused, so
//!   this service can never answer one from the wrong database;
//! * an event path is refused before any module parser runs;
//! * a concert-product operation is not a profile operation here.
//!
//! No database is needed: this is the pure route table.
use concert_service::profile_product::targets::{parse, ProfileOperation};

/// A concrete path for a contract template.
fn concrete(path: &str) -> String {
    path.replace("{profileId}", "11111111-1111-4111-8111-111111111111")
        .replace("{postId}", "33333333-3333-4333-8333-333333333333")
        .replace("{mediaId}", "44444444-4444-4444-8444-444444444444")
}

/// The fifteen operations this service serves, with their frozen method/path.
const SERVED: &[(&str, &str, &str)] = &[
    ("listProfilePosts", "GET", "/api/profiles/{profileId}/posts"),
    ("createProfilePost", "POST", "/api/me/posts"),
    ("deleteProfilePost", "DELETE", "/api/me/posts/{postId}"),
    ("initProfileMediaUpload", "POST", "/api/me/media/uploads"),
    ("putProfileMediaBytes", "PUT", "/api/me/media/{mediaId}/bytes"),
    ("finalizeProfileMedia", "POST", "/api/me/media/{mediaId}/finalize"),
    ("getProfileMedia", "GET", "/api/media/{mediaId}"),
    ("readProfileMedia", "GET", "/api/media/{mediaId}/content"),
    ("getProfileRoom", "GET", "/api/profiles/{profileId}/room"),
    ("getOwnRoom", "GET", "/api/me/room"),
    ("saveOwnRoom", "PUT", "/api/me/room"),
    ("setOwnRoomVisibility", "PATCH", "/api/me/room/visibility"),
    ("uploadRoomPoster", "POST", "/api/me/room/posters"),
    ("getOwnProfileAvatar", "GET", "/api/me/avatar"),
    ("setOwnProfileAvatar", "PUT", "/api/me/avatar"),
];

#[test]
fn the_transport_serves_exactly_the_fifteen_cross_database_operations() {
    assert_eq!(SERVED.len(), 15);
    for (operation, method, path) in SERVED {
        let mut target = concrete(path);
        // A delete carries its revision in the query, which the module parser
        // requires; every other operation takes no query.
        if *operation == "deleteProfilePost" {
            target.push_str("?expected_revision=1");
        }
        let parsed = parse(method, &target)
            .unwrap_or_else(|status| panic!("{operation} must parse: {status}"));
        assert_eq!(
            parsed.contract_id(),
            *operation,
            "{method} {target} parsed as {}",
            parsed.contract_id()
        );
    }
}

#[test]
fn every_served_operation_is_a_mutation_exactly_when_the_contract_says_so() {
    // The mutation flag decides whether the transport requires an idempotency
    // key, so it must match the frozen method.
    for (operation, method, path) in SERVED {
        let mut target = concrete(path);
        if *operation == "deleteProfilePost" {
            target.push_str("?expected_revision=1");
        }
        let parsed = parse(method, &target).expect("parses");
        let expected = !matches!(*method, "GET");
        assert_eq!(
            parsed.mutation(),
            expected,
            "{operation} ({method}) mutation flag"
        );
    }
}

#[test]
fn a_profile_database_operation_is_refused_by_this_transport() {
    // These read `mcr_profile`. Answering one here would read a database that
    // does not hold its rows, which is exactly the failure this work removes.
    for (method, target) in [
        ("GET", "/api/me/profile"),
        ("PATCH", "/api/me/profile"),
        ("PUT", "/api/me/profile/favorites"),
        ("GET", "/api/profiles/11111111-1111-4111-8111-111111111111"),
        ("GET", "/api/catalog/music/search"),
        ("GET", "/api/discover/people"),
        ("GET", "/api/me/identity/naver"),
        ("POST", "/api/me/identity/naver/verifications"),
        ("DELETE", "/api/me/identity/naver"),
        // Operations that no producer owns yet.
        ("GET", "/api/me/notifications"),
        ("PUT", "/api/profiles/11111111-1111-4111-8111-111111111111/follow"),
    ] {
        let status = parse(method, target)
            .expect_err("a profile-database operation must be refused here");
        assert_eq!(
            status.code(),
            tonic::Code::PermissionDenied,
            "{method} {target} must be refused by the closed route set"
        );
    }
}

#[test]
fn an_event_path_is_refused_before_any_module_parser_runs() {
    let event = "99999999-9999-4999-8999-999999999999";
    for target in [
        format!("/api/concert/events/{event}/posts"),
        format!("/api/me/posts/events/{event}"),
        "/api/me/room/events/1".to_owned(),
        "/api/me/avatar/events/1".to_owned(),
        // A percent-encoded segment must not slip past a narrower parser.
        "/api/me/posts%2Fevents%2F1".to_owned(),
    ] {
        let status = parse("GET", &target).expect_err("an event path must be refused");
        assert_eq!(
            status.code(),
            tonic::Code::InvalidArgument,
            "{target} must be refused as an event path"
        );
    }
}

#[test]
fn a_concert_product_operation_is_not_a_profile_operation() {
    let event = "99999999-9999-4999-8999-999999999999";
    for target in [
        format!("/api/concert/events/{event}/feed"),
        format!("/api/concert/events/{event}/parties"),
    ] {
        assert!(
            parse("GET", &target).is_err(),
            "{target} belongs to the concert product transport, not this one"
        );
    }
}

#[test]
fn the_route_set_is_closed_against_unknown_paths() {
    for (method, target) in [
        ("GET", "/api/me/unknown"),
        ("POST", "/api/me/posts/extra"),
        ("GET", "/api/media"),
        ("PUT", "/api/me/room/unknown"),
        ("GET", "/api/profiles/x/posts"),
        ("GET", "/not/api"),
        ("TRACE", "/api/me/room"),
    ] {
        assert!(
            parse(method, target).is_err(),
            "{method} {target} must not resolve to an operation"
        );
    }
}

#[test]
fn a_query_on_an_operation_that_carries_none_is_refused() {
    // Only `listProfilePosts` (page query) and `deleteProfilePost` (revision)
    // carry a query. A query on any other operation is a caller error, never a
    // silently ignored payload.
    for (method, target) in [
        ("GET", "/api/me/room?limit=1"),
        ("GET", "/api/me/avatar?x=1"),
        ("GET", "/api/media/44444444-4444-4444-8444-444444444444?x=1"),
    ] {
        assert!(
            parse(method, target).is_err(),
            "{method} {target} must refuse an unexpected query"
        );
    }
    // And the two that do carry one accept it.
    assert!(parse("GET", "/api/profiles/11111111-1111-4111-8111-111111111111/posts?limit=5").is_ok());
    assert!(parse("DELETE", "/api/me/posts/33333333-3333-4333-8333-333333333333?expected_revision=2").is_ok());
}

#[test]
fn a_non_canonical_uuid_is_refused() {
    for target in [
        // Upper case.
        "GET /api/media/44444444-4444-4444-8444-44444444444A",
        // Nil.
        "GET /api/media/00000000-0000-0000-0000-000000000000",
        // Not a uuid.
        "GET /api/media/not-a-uuid",
    ] {
        let (method, target) = target.split_once(' ').expect("method target");
        assert!(
            parse(method, target).is_err(),
            "{target} must be refused as a non-canonical id"
        );
    }
}

/// The eleven WP-05 intro operations the transport serves.
const INTRO_SERVED: &[(&str, &str, &str)] = &[
    ("getOwnIntroPost", "GET", "/api/me/intro-posts/{introId}"),
    ("listOwnIntroPosts", "GET", "/api/me/intro-posts"),
    ("saveIntroPostDraft", "PUT", "/api/me/intro-posts/{introId}"),
    ("publishIntroPost", "POST", "/api/me/intro-posts/{introId}/publish"),
    ("deleteIntroPost", "DELETE", "/api/me/intro-posts/{introId}"),
    ("setIntroPostLike", "PUT", "/api/intro-posts/{introId}/like"),
    ("clearIntroPostLike", "DELETE", "/api/intro-posts/{introId}/like"),
    ("listIntroPostLikers", "GET", "/api/intro-posts/{introId}/likes"),
    ("listIntroPostComments", "GET", "/api/intro-posts/{introId}/comments"),
    ("createIntroPostComment", "POST", "/api/intro-posts/{introId}/comments"),
    ("deleteIntroPostComment", "DELETE", "/api/intro-posts/{introId}/comments/{commentId}"),
];

fn intro_concrete(path: &str) -> String {
    path.replace("{introId}", "22222222-2222-4222-8222-222222222222")
        .replace("{commentId}", "55555555-5555-4555-8555-555555555555")
}

#[test]
fn the_transport_serves_the_eleven_wp05_intro_operations() {
    assert_eq!(INTRO_SERVED.len(), 11);
    for (operation, method, path) in INTRO_SERVED {
        let mut target = intro_concrete(path);
        // A bodyless DELETE carries its revision in the query.
        if *operation == "deleteIntroPost" {
            target.push_str("?expected_revision=1");
        }
        let parsed = parse(method, &target)
            .unwrap_or_else(|status| panic!("{operation} must parse: {status}"));
        assert_eq!(
            parsed.contract_id(),
            *operation,
            "{method} {target} parsed as {}",
            parsed.contract_id()
        );
    }
}

#[test]
fn the_two_discovery_reads_are_now_served_by_this_transport() {
    // WP-04's visibility predicate and the page/detail queries exist, so both
    // reads resolve to real operations rather than being refused.
    for (method, target, expected) in [
        ("GET", "/api/discover/intro-posts", "listIntroPosts"),
        (
            "GET",
            "/api/intro-posts/22222222-2222-4222-8222-222222222222",
            "getIntroPost",
        ),
    ] {
        let parsed = parse(method, target).expect("a served intro read must parse");
        assert_eq!(parsed.contract_id(), expected);
    }
    // Every one of the thirteen frozen intro operations is served: no gap
    // remains, and the refused set is empty.
    assert_eq!(concert_service::profile_product::INTRO_SERVED.len(), 13);
    assert!(concert_service::profile_product::INTRO_NOT_SERVED.is_empty());
    for operation in [
        "listIntroPosts",
        "getIntroPost",
        "getOwnIntroPost",
        "listOwnIntroPosts",
        "saveIntroPostDraft",
        "publishIntroPost",
        "deleteIntroPost",
        "setIntroPostLike",
        "clearIntroPostLike",
        "listIntroPostLikers",
        "listIntroPostComments",
        "createIntroPostComment",
        "deleteIntroPostComment",
    ] {
        assert!(
            concert_service::profile_product::intro::is_served(operation),
            "{operation} must be served"
        );
    }
}

#[test]
fn an_intro_operation_never_carries_an_event_path() {
    let event = "99999999-9999-4999-8999-999999999999";
    for target in [
        format!("/api/me/intro-posts/events/{event}"),
        format!("/api/intro-posts/events/{event}/likes"),
        "/api/discover/intro-posts/events/1".to_owned(),
    ] {
        let status = parse("GET", &target).expect_err("an event path must be refused");
        assert_eq!(status.code(), tonic::Code::InvalidArgument, "{target}");
    }
}

#[test]
fn the_operation_variants_are_distinct_and_named_by_the_contract() {
    use std::collections::BTreeSet;
    let mut seen = BTreeSet::new();
    for (operation, method, path) in SERVED {
        let mut target = concrete(path);
        if *operation == "deleteProfilePost" {
            target.push_str("?expected_revision=1");
        }
        let parsed = parse(method, &target).expect("parses");
        assert!(
            seen.insert(parsed.contract_id()),
            "duplicate operation {}",
            parsed.contract_id()
        );
        // The variant is a real one, not a fallback.
        assert!(matches!(
            parsed,
            ProfileOperation::ListProfilePosts { .. }
                | ProfileOperation::CreateProfilePost
                | ProfileOperation::DeleteProfilePost { .. }
                | ProfileOperation::InitProfileMediaUpload
                | ProfileOperation::PutProfileMediaBytes { .. }
                | ProfileOperation::FinalizeProfileMedia { .. }
                | ProfileOperation::GetProfileMedia { .. }
                | ProfileOperation::ReadProfileMedia { .. }
                | ProfileOperation::GetProfileRoom { .. }
                | ProfileOperation::GetOwnRoom
                | ProfileOperation::SaveOwnRoom
                | ProfileOperation::SetOwnRoomVisibility
                | ProfileOperation::UploadRoomPoster
                | ProfileOperation::GetOwnProfileAvatar
                | ProfileOperation::SetOwnProfileAvatar
        ));
    }
    assert_eq!(seen.len(), 15);
}
