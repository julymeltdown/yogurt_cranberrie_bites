//! WP-03 T-803: the discovery route parser.
//!
//! `GET /api/discover/people` carries the frozen `PeopleQuery`: an optional
//! `category`, an optional `cursor`, and an optional `limit`. The parser is
//! **closed**: a field that is not one of those three is a `400`, not something
//! silently ignored. That matters because an ignored field is a lie — a client
//! that sends `sort=score` and gets an unsorted page believes it was sorted.
//!
//! The parser is also the only place the raw target is interpreted, so the
//! repository can assume a validated query. It deliberately does not default the
//! category to music in the *parser*: the default belongs to the caller's
//! presentation, and `PeopleQuery` requires an explicit category so no layer can
//! silently discover the wrong tab.
//!
//! The route is a read. It carries no body, no `X-MCR-Intent`, no idempotency
//! key and no event path; a target naming an event is refused here exactly as it
//! is for every other profile-owned operation.
use super::repository::{Category, PeopleQuery, DEFAULT_LIMIT, MAX_LIMIT};
use std::collections::BTreeMap;

/// The frozen path for `discoverProfiles`.
pub const PATH: &str = "/api/discover/people";

#[derive(Debug, PartialEq, Eq)]
pub enum RouteError {
    /// The method or path is not this operation.
    NotFound,
    /// A query field is unknown, repeated, or out of bounds.
    InvalidQuery(&'static str),
}

/// The closed query field set. Anything else is refused.
const FIELDS: [&str; 3] = ["category", "cursor", "limit"];

/// Percent-decode one query component. `+` is a literal plus in a query value
/// for this API (the transport encodes spaces as `%20`), and a malformed escape
/// is a client error rather than a best guess.
fn decode(value: &str) -> Result<String, RouteError> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                let hex = bytes
                    .get(index + 1..index + 3)
                    .ok_or(RouteError::InvalidQuery("query"))?;
                let text = std::str::from_utf8(hex).map_err(|_| RouteError::InvalidQuery("query"))?;
                let byte = u8::from_str_radix(text, 16).map_err(|_| RouteError::InvalidQuery("query"))?;
                out.push(byte);
                index += 3;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| RouteError::InvalidQuery("query"))
}

/// Parse the request target into a validated query.
///
/// `default_category` is the caller's presentation default. It is a parameter
/// rather than a constant so the default is visible at the call site instead of
/// buried in the parser.
pub fn parse(
    method: &str,
    target: &str,
    default_category: Category,
) -> Result<PeopleQuery, RouteError> {
    if method != "GET" {
        return Err(RouteError::NotFound);
    }
    if target.len() > 4096 || target.chars().any(char::is_control) {
        return Err(RouteError::InvalidQuery("target"));
    }
    // Discovery is a profile-owned read. An event path is a caller error.
    if target.contains("/events/") {
        return Err(RouteError::InvalidQuery("target"));
    }
    let (path, query) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| (path, Some(query)));
    if path != PATH {
        return Err(RouteError::NotFound);
    }
    let mut fields: BTreeMap<&'static str, String> = BTreeMap::new();
    if let Some(query) = query {
        for pair in query.split('&') {
            if pair.is_empty() {
                continue;
            }
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            let name = decode(name)?;
            let known = FIELDS
                .iter()
                .copied()
                .find(|field| *field == name)
                .ok_or(RouteError::InvalidQuery("query"))?;
            if fields.insert(known, decode(value)?).is_some() {
                // A repeated field is ambiguous, never "last one wins".
                return Err(RouteError::InvalidQuery(known));
            }
        }
    }
    let category = match fields.get("category") {
        Some(value) => Category::parse(value).ok_or(RouteError::InvalidQuery("category"))?,
        None => default_category,
    };
    let limit = match fields.get("limit") {
        Some(value) => {
            // Strict decimal only: `+5`, ` 5`, `5.0` and `1e2` are all refused.
            if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err(RouteError::InvalidQuery("limit"));
            }
            let parsed: i64 = value
                .parse()
                .map_err(|_| RouteError::InvalidQuery("limit"))?;
            if !(1..=MAX_LIMIT).contains(&parsed) {
                return Err(RouteError::InvalidQuery("limit"));
            }
            parsed
        }
        None => DEFAULT_LIMIT,
    };
    let cursor = fields.get("cursor").cloned();
    if let Some(cursor) = cursor.as_deref() {
        if cursor.is_empty()
            || cursor.len() > 1024
            || !cursor
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(RouteError::InvalidQuery("cursor"));
        }
    }
    Ok(PeopleQuery {
        category,
        cursor,
        limit,
    })
}
