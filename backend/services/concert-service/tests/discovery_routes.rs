//! WP-03 T-803: `GET /api/discover/people` route parsing.
//!
//! The acceptance case the task document names is that an unknown query field
//! is a `400`. The parser is the whole subject of this file: it is closed, it
//! defaults only what the frozen contract defaults, and it refuses everything
//! else rather than ignoring it.
//!
//! These cases are pure: no database is involved, so they run in any
//! environment. The database half of discovery is `tests/discovery.rs`.
use concert_service::discovery::{parse, Category, RouteError, DEFAULT_LIMIT, MAX_LIMIT, PATH};

fn music(target: &str) -> Result<concert_service::discovery::PeopleQuery, RouteError> {
    parse("GET", target, Category::Music)
}

#[test]
fn the_frozen_path_parses_with_defaults() {
    assert_eq!(PATH, "/api/discover/people");
    let query = music(PATH).expect("valid");
    assert_eq!(query.category, Category::Music);
    assert_eq!(query.limit, DEFAULT_LIMIT);
    assert_eq!(query.cursor, None);
}

#[test]
fn an_unknown_query_field_is_a_client_error() {
    // The acceptance case: an unknown field is refused, never ignored.
    for target in [
        "/api/discover/people?sort=score",
        "/api/discover/people?category=MUSIC&sort=score",
        "/api/discover/people?q=mcr",
        "/api/discover/people?offset=20",
        "/api/discover/people?",
        // A near-miss on a real field is still unknown.
        "/api/discover/people?Category=MUSIC",
        "/api/discover/people?categorie=MUSIC",
    ] {
        let result = music(target);
        if target == "/api/discover/people?" {
            // A bare trailing `?` carries no fields at all, so it is valid.
            assert!(result.is_ok(), "{target}");
            continue;
        }
        assert_eq!(result, Err(RouteError::InvalidQuery("query")), "{target}");
    }
}

#[test]
fn a_repeated_field_is_refused() {
    assert_eq!(
        music("/api/discover/people?limit=5&limit=10"),
        Err(RouteError::InvalidQuery("limit"))
    );
    assert_eq!(
        music("/api/discover/people?category=MUSIC&category=ANIME"),
        Err(RouteError::InvalidQuery("category"))
    );
}

#[test]
fn the_category_is_closed() {
    for (value, expected) in [
        ("MUSIC", Category::Music),
        ("ANIME", Category::Anime),
        ("GAME", Category::Game),
    ] {
        let query = music(&format!("/api/discover/people?category={value}")).expect("valid");
        assert_eq!(query.category, expected);
    }
    for value in ["music", "MUSIC%20", "MOVIE", "BOOK", "KPOP", ""] {
        assert_eq!(
            music(&format!("/api/discover/people?category={value}")),
            Err(RouteError::InvalidQuery("category")),
            "{value}"
        );
    }
}

#[test]
fn the_limit_is_bounded_and_strictly_decimal() {
    let query = music("/api/discover/people?limit=1").expect("valid");
    assert_eq!(query.limit, 1);
    let query = music(&format!("/api/discover/people?limit={MAX_LIMIT}")).expect("valid");
    assert_eq!(query.limit, MAX_LIMIT);
    for value in [
        "0",
        "-1",
        "101",
        "1.5",
        "1e2",
        "%2B5",
        " 5",
        "abc",
        "",
        "9999999999999999999999",
    ] {
        assert_eq!(
            music(&format!("/api/discover/people?limit={value}")),
            Err(RouteError::InvalidQuery("limit")),
            "{value}"
        );
    }
}

#[test]
fn the_cursor_is_bounded_and_shaped() {
    let query = music("/api/discover/people?cursor=abcDEF012_-").expect("valid");
    assert_eq!(query.cursor.as_deref(), Some("abcDEF012_-"));
    for value in ["", "abc+def", "abc%20def", "abc.def", "abc%2Fdef"] {
        assert_eq!(
            music(&format!("/api/discover/people?cursor={value}")),
            Err(RouteError::InvalidQuery("cursor")),
            "{value}"
        );
    }
    let long = "a".repeat(1025);
    assert_eq!(
        music(&format!("/api/discover/people?cursor={long}")),
        Err(RouteError::InvalidQuery("cursor"))
    );
}

#[test]
fn only_the_get_method_and_the_exact_path_are_served() {
    for method in ["POST", "PUT", "PATCH", "DELETE", "HEAD"] {
        assert_eq!(
            parse(method, PATH, Category::Music),
            Err(RouteError::NotFound),
            "{method}"
        );
    }
    for path in [
        "/api/discover/people/extra",
        "/api/discover",
        "/api/discover/intro-posts",
        "/api/discover/people/music",
        "/api/profiles/00000000-0000-4000-8000-000000000001",
    ] {
        assert_eq!(
            music(path),
            Err(RouteError::NotFound),
            "{path}"
        );
    }
}

#[test]
fn an_event_path_is_refused() {
    let event = "00000000-0000-4000-8000-000000000001";
    for target in [
        format!("/api/concert/events/{event}/discover/people"),
        format!("/api/discover/events/{event}/people"),
    ] {
        assert_eq!(
            music(&target),
            Err(RouteError::InvalidQuery("target")),
            "{target}"
        );
    }
}

#[test]
fn a_percent_encoded_value_is_decoded_once() {
    // `%41NIME` decodes to `ANIME`; a double encoding would not.
    let query = music("/api/discover/people?category=%41NIME").expect("valid");
    assert_eq!(query.category, Category::Anime);
    // A malformed escape is a client error, not a best guess.
    assert_eq!(
        music("/api/discover/people?category=%zz"),
        Err(RouteError::InvalidQuery("query"))
    );
    assert_eq!(
        music("/api/discover/people?category=%4"),
        Err(RouteError::InvalidQuery("query"))
    );
}

#[test]
fn every_field_combines() {
    let query = music(&format!(
        "/api/discover/people?category=GAME&limit=7&cursor=abc"
    ))
    .expect("valid");
    assert_eq!(query.category, Category::Game);
    assert_eq!(query.limit, 7);
    assert_eq!(query.cursor.as_deref(), Some("abc"));
}

#[test]
fn the_default_category_is_the_callers_choice() {
    // The parser does not hardcode a default; the caller's presentation default
    // is passed in, so the "default tab is music" decision lives in one place.
    for category in [Category::Music, Category::Anime, Category::Game] {
        let query = parse("GET", PATH, category).expect("valid");
        assert_eq!(query.category, category);
    }
}
