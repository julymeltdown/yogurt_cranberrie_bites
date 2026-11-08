//! WP-06 T-504: the profile post and media route parser.
//!
//! These cases prove the route boundary, not the producer: the parser is a pure
//! function, so a refusal is proved without a database and without any producer
//! work having been possible. The acceptance statement is "unknown routes are
//! rejected before producer work", and that is exactly what a pure parser makes
//! structural.
//!
//! The eight operations are the frozen §2.3 register. A test compares the closed
//! table with `contracts/domain/v1/operations.json`, so the table and the
//! contract cannot drift.
use concert_service::profile_posts::http::{
    delete_revision, names_event_path, parse_bytes_content_type, parse_finalize_upload,
    parse_init_upload, protects, route, BodyLimit, RouteRefusal, MAX_POST_BODY, ROUTES,
};
use concert_service::profile_media::UploadPurpose;
use serde_json::json;

const PROFILE: &str = "0f0e0d0c-0b0a-4908-8706-050403020100";
const MEDIA: &str = "1a1b1c1d-1e1f-4a2b-8c3d-4e5f60718293";
const POST: &str = "2a2b2c2d-2e2f-4a3b-8c4d-5e6f708192a3";

fn concrete(template: &str) -> String {
    template
        .replace(":profile_id", PROFILE)
        .replace(":media_id", MEDIA)
        .replace(":post_id", POST)
}

// ---------------------------------------------------------------------------
// The closed table
// ---------------------------------------------------------------------------

#[test]
fn the_route_table_is_exactly_the_frozen_operation_register() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap();
    let rows: Vec<serde_json::Value> = serde_json::from_slice::<Vec<serde_json::Value>>(
        &std::fs::read(root.join("contracts/domain/v1/operations.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(ROUTES.len(), 8);
    for (method, template, operation) in ROUTES {
        let row = rows
            .iter()
            .find(|row| row["operationId"] == *operation)
            .unwrap_or_else(|| panic!("{operation} is not a frozen operation"));
        assert_eq!(
            row["method"].as_str().unwrap().to_ascii_uppercase(),
            *method,
            "{operation} method"
        );
        let contract: Vec<&str> = row["path"].as_str().unwrap().split('/').collect();
        let local: Vec<&str> = template.split('/').collect();
        assert_eq!(contract.len(), local.len(), "{operation} path");
        for (want, got) in contract.iter().zip(local.iter()) {
            if want.starts_with('{') {
                assert!(got.starts_with(':'), "{operation} path");
            } else {
                assert_eq!(want, got, "{operation} path");
            }
        }
        // No profile post or media operation carries an event segment.
        assert!(!template.contains("events"), "{operation} must not name an event");
    }
}

#[test]
fn every_frozen_operation_parses_and_nothing_else_does() {
    for (method, template, operation) in ROUTES {
        let path = concrete(template);
        let (parsed, _) = route(method, &path).unwrap_or_else(|error| panic!("{path}: {error:?}"));
        assert_eq!(parsed.operation(), *operation, "{path}");
        // A wrong method on a real path is not this operation.
        let other = if *method == "GET" { "POST" } else { "GET" };
        assert_eq!(route(other, &path), Err(RouteRefusal {
            status: 404,
            reason: "RESOURCE_UNAVAILABLE",
        }), "{other} {path}");
        // A suffix is never matched.
        assert_eq!(
            route(method, &format!("{path}/extra")),
            Err(RouteRefusal {
                status: 404,
                reason: "RESOURCE_UNAVAILABLE",
            }),
            "{path}/extra"
        );
    }
}

#[test]
fn an_unknown_route_is_rejected_before_any_producer_work() {
    for (method, path) in [
        ("GET", "/api/me/posts"),
        ("POST", "/api/me/posts/anything"),
        ("PUT", "/api/me/media/uploads"),
        ("GET", "/api/me/media/uploads"),
        ("DELETE", &*format!("/api/media/{MEDIA}")),
        ("POST", &*format!("/api/media/{MEDIA}/content")),
        ("GET", "/api/profiles/posts"),
        ("GET", &*format!("/api/profiles/{PROFILE}/posts/{POST}")),
        ("POST", &*format!("/api/me/media/{MEDIA}/bytes")),
        ("GET", &*format!("/api/me/media/{MEDIA}/bytes")),
        ("PATCH", &*format!("/api/me/posts/{POST}")),
    ] {
        assert_eq!(
            route(method, path),
            Err(RouteRefusal {
                status: 404,
                reason: "RESOURCE_UNAVAILABLE",
            }),
            "{method} {path}"
        );
    }
}

#[test]
fn an_event_path_is_refused_before_the_table_is_consulted() {
    let event = "3a3b3c3d-3e3f-4a4b-8c5d-6e7f8091a2b3";
    for target in [
        format!("/api/profiles/events/{event}/posts"),
        format!("/api/me/posts/events/{event}"),
        format!("/api/media/events%2F{event}/content"),
        format!("/api/media/%2Fevents%2F{event}/content"),
        format!("/api/me/media/events/{event}/bytes"),
        format!("/api/me/media/events/{event}/finalize"),
    ] {
        assert!(names_event_path(&target), "{target}");
        assert_eq!(
            route("GET", &target),
            Err(RouteRefusal {
                status: 400,
                reason: "INVALID_INPUT",
            }),
            "{target}"
        );
    }
    // A path that merely contains the word is not an event path.
    assert!(!names_event_path("/api/me/media/events"));
}

#[test]
fn a_non_canonical_or_nil_uuid_is_refused() {
    for target in [
        format!("/api/media/{}", MEDIA.to_ascii_uppercase()),
        format!("/api/media/{{{MEDIA}}}"),
        "/api/media/00000000-0000-0000-0000-000000000000".to_owned(),
        "/api/media/not-a-uuid".to_owned(),
        format!("/api/profiles/{PROFILE}x/posts"),
    ] {
        assert!(
            route("GET", &target).is_err(),
            "{target} must be refused"
        );
    }
}

#[test]
fn the_body_limits_are_declared_per_operation() {
    let (bytes, _) = route("PUT", &format!("/api/me/media/{MEDIA}/bytes")).unwrap();
    assert_eq!(
        bytes.body_limit(),
        BodyLimit::Bytes(concert_application::media::MAX_BYTES)
    );
    assert!(bytes.body_limit().is_raw());
    let (create, _) = route("POST", "/api/me/posts").unwrap();
    assert_eq!(create.body_limit(), BodyLimit::Json(MAX_POST_BODY));
    for (method, path) in [
        ("GET", format!("/api/profiles/{PROFILE}/posts")),
        ("GET", format!("/api/media/{MEDIA}")),
        ("GET", format!("/api/media/{MEDIA}/content")),
    ] {
        let (read, _) = route(method, &path).unwrap();
        assert_eq!(read.body_limit(), BodyLimit::None, "{path}");
    }
    let (delete, _) = route("DELETE", &format!("/api/me/posts/{POST}?expected_revision=1")).unwrap();
    assert_eq!(delete.body_limit(), BodyLimit::None);
    // A delete with no revision is still a route, whose revision read fails.
    let (delete, _) = route("DELETE", &format!("/api/me/posts/{POST}")).unwrap();
    assert_eq!(delete.operation(), "deleteProfilePost");
    assert!(delete_revision(&format!("/api/me/posts/{POST}")).is_err());
}

#[test]
fn the_profile_upload_path_admits_only_profile_post() {
    let body = |purpose: &str| {
        serde_json::to_vec(&json!({
            "purpose": purpose,
            "content_type": "image/png",
            "byte_length": 12,
            "sha256": "aa".repeat(32),
        }))
        .unwrap()
    };
    let parsed = parse_init_upload(&body("PROFILE_POST"), UploadPurpose::ProfilePost).unwrap();
    assert_eq!(parsed.purpose, concert_application::media::Purpose::ProfilePost);
    assert_eq!(parsed.mime, concert_application::media::Mime::Png);
    assert_eq!(parsed.byte_length, 12);
    for purpose in ["ROOM_POSTER", "INTRO_POST", "PROFILE_IMAGE", "POST", "GIVEAWAY", "BEACON"] {
        assert_eq!(
            parse_init_upload(&body(purpose), UploadPurpose::ProfilePost),
            Err(RouteRefusal {
                status: 400,
                reason: "INVALID_PURPOSE",
            }),
            "{purpose}"
        );
    }
    // A GIF or a video content type is 415, before any byte is stored.
    for content_type in ["image/gif", "video/mp4"] {
        let body = serde_json::to_vec(&json!({
            "purpose": "PROFILE_POST",
            "content_type": content_type,
            "byte_length": 12,
            "sha256": "aa".repeat(32),
        }))
        .unwrap();
        assert_eq!(
            parse_init_upload(&body, UploadPurpose::ProfilePost),
            Err(RouteRefusal {
                status: 415,
                reason: "INVALID_MEDIA_TYPE",
            }),
            "{content_type}"
        );
    }
    // A declared length over 10 MiB is 413.
    let body = serde_json::to_vec(&json!({
        "purpose": "PROFILE_POST",
        "content_type": "image/png",
        "byte_length": 10 * 1024 * 1024 + 1,
        "sha256": "aa".repeat(32),
    }))
    .unwrap();
    assert_eq!(
        parse_init_upload(&body, UploadPurpose::ProfilePost),
        Err(RouteRefusal {
            status: 413,
            reason: "PAYLOAD_TOO_LARGE",
        })
    );
    // The body is closed.
    let body = serde_json::to_vec(&json!({
        "purpose": "PROFILE_POST",
        "content_type": "image/png",
        "byte_length": 12,
        "sha256": "aa".repeat(32),
        "event_id": null,
    }))
    .unwrap();
    assert!(parse_init_upload(&body, UploadPurpose::ProfilePost).is_err());
}

#[test]
fn only_the_three_image_types_are_accepted_for_raw_bytes() {
    for accepted in ["image/jpeg", "image/png", "image/webp"] {
        assert!(parse_bytes_content_type(accepted).is_ok(), "{accepted}");
    }
    for refused in ["image/gif", "video/mp4", "image/avif", "application/octet-stream"] {
        assert_eq!(
            parse_bytes_content_type(refused),
            Err(RouteRefusal {
                status: 415,
                reason: "INVALID_MEDIA_TYPE",
            }),
            "{refused}"
        );
    }
}

#[test]
fn the_finalize_body_is_exactly_one_revision() {
    assert_eq!(
        parse_finalize_upload(br#"{"expected_revision":3}"#),
        Ok(3)
    );
    for body in [
        &b"{}"[..],
        br#"{"expected_revision":0}"#,
        br#"{"expected_revision":"3"}"#,
        br#"{"expected_revision":3,"extra":true}"#,
        br#"{"expected_revision":3,"expected_revision":4}"#,
    ] {
        assert!(parse_finalize_upload(body).is_err(), "{}", String::from_utf8_lossy(body));
    }
}

#[test]
fn the_page_query_is_closed_and_bounded() {
    let (_, query) = route("GET", &format!("/api/profiles/{PROFILE}/posts?limit=5")).unwrap();
    assert_eq!(query.limit, 5);
    assert_eq!(query.cursor, None);
    let (_, query) = route(
        "GET",
        &format!("/api/profiles/{PROFILE}/posts?cursor=abc_-123&limit=100"),
    )
    .unwrap();
    assert_eq!(query.cursor.as_deref(), Some("abc_-123"));
    for bad in [
        "limit=0",
        "limit=101",
        "limit=+5",
        "limit=",
        "limit=5.0",
        "sort=score",
        "limit=5&limit=6",
        "cursor=a&cursor=b",
    ] {
        assert_eq!(
            route("GET", &format!("/api/profiles/{PROFILE}/posts?{bad}")),
            Err(RouteRefusal {
                status: 400,
                reason: "INVALID_INPUT",
            }),
            "{bad}"
        );
    }
}

#[test]
fn only_the_list_operation_accepts_a_query() {
    for (method, path) in [
        ("GET", format!("/api/media/{MEDIA}?limit=5")),
        ("GET", format!("/api/media/{MEDIA}/content?x=1")),
        ("POST", "/api/me/media/uploads?x=1".to_owned()),
        ("POST", "/api/me/posts?x=1".to_owned()),
        ("POST", format!("/api/me/media/{MEDIA}/finalize?x=1")),
    ] {
        assert_eq!(
            route(method, &path),
            Err(RouteRefusal {
                status: 400,
                reason: "INVALID_INPUT",
            }),
            "{method} {path}"
        );
    }
}

#[test]
fn a_delete_revision_is_required_and_bounded() {
    let target = format!("/api/me/posts/{POST}");
    assert!(delete_revision(&target).is_err());
    assert_eq!(
        delete_revision(&format!("{target}?expected_revision=3")),
        Ok(3)
    );
    for bad in [
        "expected_revision=0",
        "expected_revision=x",
        "expected_revision=",
        "expected_revision=1&expected_revision=2",
        "other=1",
    ] {
        assert!(
            delete_revision(&format!("{target}?{bad}")).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn the_covered_paths_are_the_profile_post_and_media_surface() {
    for covered in [
        "/api/me/posts",
        "/api/me/posts/anything",
        "/api/me/media/uploads",
        "/api/media/anything",
        "/api/profiles/anything/posts",
    ] {
        assert!(protects(covered), "{covered}");
    }
    for uncovered in ["/api/me/profile", "/api/profiles/anything", "/api/concert/events/x"] {
        assert!(!protects(uncovered), "{uncovered}");
    }
}

#[test]
fn the_upload_context_of_each_upload_operation_is_the_profile_post_context() {
    for (method, path) in [
        ("POST", "/api/me/media/uploads".to_owned()),
        ("PUT", format!("/api/me/media/{MEDIA}/bytes")),
        ("POST", format!("/api/me/media/{MEDIA}/finalize")),
    ] {
        let (parsed, _) = route(method, &path).unwrap();
        assert_eq!(
            parsed.upload_context(),
            Some(UploadPurpose::ProfilePost),
            "{path}"
        );
    }
    let (read, _) = route("GET", &format!("/api/media/{MEDIA}/content")).unwrap();
    assert_eq!(read.upload_context(), None);
}
