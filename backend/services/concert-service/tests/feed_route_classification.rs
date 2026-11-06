use concert_service::product_rpc::{operation, Operation};
use uuid::Uuid;

#[test]
fn every_feed_mutation_route_resolves_to_its_typed_command() {
    let event = Uuid::new_v4();
    let post = Uuid::new_v4();
    let comment = Uuid::new_v4();
    let root = format!("/api/concert/events/{event}");
    let cases = [
        (
            "POST",
            format!("{root}/posts"),
            "createConcertPost",
        ),
        (
            "PATCH",
            format!("{root}/posts/{post}"),
            "editConcertPost",
        ),
        (
            "DELETE",
            format!("{root}/posts/{post}?expected_revision=3"),
            "deleteConcertPost",
        ),
        (
            "POST",
            format!("{root}/posts/{post}/comments"),
            "createConcertComment",
        ),
        (
            "PATCH",
            format!("{root}/comments/{comment}"),
            "editConcertComment",
        ),
        (
            "DELETE",
            format!("{root}/comments/{comment}?expected_revision=2"),
            "deleteConcertComment",
        ),
        (
            "PUT",
            format!("{root}/posts/{post}/reaction"),
            "likeConcertPost",
        ),
        (
            "DELETE",
            format!("{root}/posts/{post}/reaction"),
            "unlikeConcertPost",
        ),
    ];
    for (method, target, expected) in cases {
        let parsed = operation(event, method, &target)
            .unwrap_or_else(|error| panic!("{method} {target}: {error:?}"));
        assert!(matches!(parsed, Operation::FeedCommand(_)));
        assert_eq!(parsed.contract_id(), expected);
    }
    for (method, target) in [
        ("POST", format!("{root}/posts/{post}/reaction")),
        ("PATCH", format!("{root}/posts/not-a-uuid")),
        ("DELETE", format!("{root}/posts/{post}")),
        ("PATCH", format!("{root}/posts/{post}?unexpected=1")),
    ] {
        assert!(
            operation(event, method, &target).is_err(),
            "{method} {target} was accepted"
        );
    }
}
