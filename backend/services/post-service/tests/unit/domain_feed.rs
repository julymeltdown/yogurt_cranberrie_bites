use crate::support::{instant, post, uuid};
use post_service::domain::feed::{FeedCachePage, PostCursor, TrendingCursor};
use proptest::prelude::*;

#[test]
fn post_cursor_round_trips_post_ordering_fields_and_rejects_invalid_values() {
    let post_id = uuid("11111111-1111-1111-1111-111111111111");
    let author_id = uuid("22222222-2222-2222-2222-222222222222");
    let post = post(post_id, author_id, "cursor source");

    let cursor = PostCursor::from(&post);
    let encoded = cursor.encode();
    let parsed = PostCursor::parse(&encoded)
        .expect("valid cursor")
        .expect("present cursor");

    assert_eq!(parsed, cursor);
    assert_eq!(PostCursor::parse("  ").expect("blank cursor"), None);
    assert!(PostCursor::parse("not-a-date|11111111-1111-1111-1111-111111111111").is_err());
    assert!(PostCursor::parse("2026-01-20T09:10:00Z|not-a-uuid").is_err());
    assert!(PostCursor::parse("2026-01-20T09:10:00Z").is_err());
    assert!(PostCursor::parse(&"x".repeat(257)).is_err());
    let padded = format!("{}{}", " ".repeat(257), cursor.encode());
    assert!(PostCursor::parse(&padded).is_err());
}

#[test]
fn trending_cursor_round_trips_rank_fields_and_rejects_invalid_values() {
    let cursor = TrendingCursor {
        like_count: 42,
        created_at: instant(1_700_000_000),
        post_id: uuid("33333333-3333-3333-3333-333333333333"),
    };

    let parsed = TrendingCursor::parse(&cursor.encode())
        .expect("valid cursor")
        .expect("present cursor");

    assert_eq!(parsed, cursor);
    assert_eq!(TrendingCursor::parse("").expect("blank cursor"), None);
    assert!(TrendingCursor::parse(
        "many|2026-01-20T09:10:00Z|33333333-3333-3333-3333-333333333333"
    )
    .is_err());
    assert!(TrendingCursor::parse("42|not-a-date|33333333-3333-3333-3333-333333333333").is_err());
    assert!(TrendingCursor::parse("42|2026-01-20T09:10:00Z|not-a-uuid").is_err());
    assert!(TrendingCursor::parse(
        "42|2026-01-20T09:10:00Z|33333333-3333-3333-3333-333333333333|extra"
    )
    .is_err());
    let padded = format!("{}{}", " ".repeat(257), cursor.encode());
    assert!(TrendingCursor::parse(&padded).is_err());
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(proptest::test_runner::FileFailurePersistence::WithSource("proptest-regressions"))),
        ..ProptestConfig::default()
    })]

    #[test]
    fn post_and_trending_cursor_parsers_are_bounded_and_canonical(input in prop::collection::vec(any::<u8>(), 0..=1024)) {
        let candidate = String::from_utf8_lossy(&input);
        let post_cursor = PostCursor::parse(&candidate);
        let trending_cursor = TrendingCursor::parse(&candidate);
        if candidate.len() > 256 {
            prop_assert!(post_cursor.is_err());
            prop_assert!(trending_cursor.is_err());
        }
        if let Ok(Some(cursor)) = post_cursor {
            prop_assert_eq!(PostCursor::parse(&cursor.encode()).expect("encoded post cursor remains valid"), Some(cursor));
        }
        if let Ok(Some(cursor)) = trending_cursor {
            prop_assert_eq!(TrendingCursor::parse(&cursor.encode()).expect("encoded trending cursor remains valid"), Some(cursor));
        }
    }
}

#[test]
fn feed_cache_page_serializes_post_ids_and_absent_cursor_without_loss() {
    let page = FeedCachePage {
        post_ids: vec![
            uuid("44444444-4444-4444-4444-444444444444"),
            uuid("55555555-5555-5555-5555-555555555555"),
        ],
        next_cursor: None,
    };

    let json = serde_json::to_string(&page).expect("serialize feed cache page");
    let decoded: FeedCachePage = serde_json::from_str(&json).expect("deserialize feed cache page");

    assert_eq!(decoded, page);
}
