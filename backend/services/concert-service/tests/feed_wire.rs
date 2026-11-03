use concert_service::feed::wire;
use serde_json::{json, Value};
#[test]
fn closed_create_schema_rejects_private_fields_duplicates_and_noncanonical_ids() {
    let good = br#"{"text":"Concert","media_ids":[],"links":[]}"#;
    assert!(wire::create_post(good).is_ok());
    for bad in [
        br#"{"text":"a","text":"b","media_ids":[],"links":[]}"#.as_slice(),
        br#"{"text":"a","author_id":"x","media_ids":[],"links":[]}"#,
        br#"{"text":"a","media_ids":["ABCDEF00-0000-0000-0000-000000000001"],"links":[]}"#,
        br#"{"text":"  ","media_ids":[],"links":[]}"#,
    ] {
        assert!(wire::create_post(bad).is_err());
    }
}
#[test]
fn actual_public_fixture_passes_and_private_author_field_is_rejected() {
    let mut v: Value = serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/domain/public-post.json"
    ))
    .unwrap();
    assert!(wire::validate("Domain_feed_PostPublic", &v).is_ok());
    v["author"]["instagram_handle"] = json!("private");
    assert!(wire::validate("Domain_feed_PostPublic", &v).is_err());
}
