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

#[test]
fn command_decoders_preserve_c04_shapes_and_reject_actor_injection() {
    use concert_domain::feed::FeedCommand;
    use uuid::Uuid;
    let id = Uuid::from_u128(12);
    for (op, json) in [
        (
            "createConcertPost",
            r#"{"text":"Post","media_ids":[],"links":[]}"#,
        ),
        (
            "editConcertPost",
            r#"{"expected_revision":1,"text":"Edit","media_ids":[],"links":[]}"#,
        ),
        ("deleteConcertPost", r#"{"expected_revision":1}"#),
        (
            "createConcertComment",
            r#"{"text":"Comment","post_revision":1}"#,
        ),
        (
            "editConcertComment",
            r#"{"text":"Edit","expected_revision":1}"#,
        ),
        ("deleteConcertComment", r#"{"expected_revision":1}"#),
        ("likeConcertPost", "{}"),
        ("unlikeConcertPost", "{}"),
    ] {
        let c = wire::command(
            op,
            if op == "createConcertPost" {
                None
            } else {
                Some(id)
            },
            json.as_bytes(),
        )
        .unwrap();
        assert_eq!(c.operation(), op);
        assert!(c.validate().is_ok());
    }
    assert!(wire::command(
        "createConcertComment",
        Some(id),
        br#"{"text":"a","post_revision":1,"actor_id":"ignored"}"#
    )
    .is_err());
    assert!(wire::command("likeConcertPost", Some(id), br#"{"actor_id":"another"}"#).is_err());
    assert!(wire::command(
        "deleteConcertComment",
        Some(id),
        br#"{"expected_revision":1,"expected_revision":2}"#
    )
    .is_err());
    assert!(matches!(
        wire::command("unlikeConcertPost", Some(id), b"{}").unwrap(),
        FeedCommand::SetReaction { present: false, .. }
    ));
}
