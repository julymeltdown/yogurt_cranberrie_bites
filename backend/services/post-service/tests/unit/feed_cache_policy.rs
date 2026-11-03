use post_service::adapters::out::cache::feed_cache::{
    feed_cache_generation_key, feed_cache_generation_key_for_epoch, feed_cache_key,
    feed_cache_key_for_epoch,
};
use uuid::Uuid;

const USER_ID: &str = "11111111-1111-1111-1111-111111111111";

#[test]
fn feed_cache_key_is_versioned_and_uses_canonical_cursor_shape() {
    let user_id = Uuid::parse_str(USER_ID).expect("valid uuid");

    assert_eq!(
        feed_cache_key(user_id, 7, Some("cursor|page|2"), 25),
        "feed:1:11111111-1111-1111-1111-111111111111:7:cursor_page_2:25"
    );
    assert_eq!(
        feed_cache_key(user_id, 0, None, 50),
        "feed:1:11111111-1111-1111-1111-111111111111:0:head:50"
    );
    assert_eq!(
        feed_cache_generation_key(user_id),
        "feed-generation:1:11111111-1111-1111-1111-111111111111"
    );
}

#[test]
fn feed_cache_epoch_is_part_of_both_generation_and_page_keys() {
    let user_id = Uuid::parse_str(USER_ID).expect("valid uuid");

    assert_eq!(
        feed_cache_generation_key_for_epoch(2, user_id),
        "feed-generation:2:11111111-1111-1111-1111-111111111111"
    );
    assert_eq!(
        feed_cache_key_for_epoch(2, user_id, 3, None, 20),
        "feed:2:11111111-1111-1111-1111-111111111111:3:head:20"
    );
}

#[test]
fn feed_cache_invalidation_uses_generation_bumps_not_keyspace_scans() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/adapters/out/cache/feed_cache.rs"),
    )
    .expect("read feed cache source");

    assert!(source.contains("INCR"));
    // Checked as command invocations rather than as bare substrings. A plain
    // substring test also matches the KEYS table that a Lua script addresses by
    // position, which is not a keyspace scan and was a false positive here.
    for invocation in ["'SCAN'", "'KEYS'", "\"SCAN\"", "\"KEYS\""] {
        assert!(
            !source.contains(invocation),
            "invalidation must not invoke the {invocation} command",
        );
    }
}

#[test]
fn feed_cache_fill_guard_compares_decimal_generations_without_lua_numbers() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/adapters/out/cache/feed_cache.rs"),
    )
    .expect("read feed cache source");

    assert!(source.contains("observed = '0'"));
    assert!(source.contains("if observed ~= ARGV[1] then"));
    assert!(
        !source.contains("tonumber"),
        "Lua numeric coercion can alias distinct large generations"
    );
}
