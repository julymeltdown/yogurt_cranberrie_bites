//! Closed six-operation protocol, independent of Root operation enumeration.
use chrono::{DateTime, Utc};
use concert_application::intro::{self, IntroError};
use serde::Deserialize;
use uuid::Uuid;

/// Root's exact closed routing predicate; this never verifies or mints authority.
pub fn handles_operation(operation: &str) -> bool {
    matches!(
        operation,
        "listConcertIntros"
            | "getConcertIntro"
            | "getOwnConcertIntro"
            | "setConcertIntro"
            | "publishConcertIntro"
            | "closeConcertIntro"
    )
}
/// Closed service target parser for C04 Intro operations. Root independently
/// verifies the signed operation/target; this parser never grants authority.
pub fn parse_intro_target(
    event: Uuid,
    method: &str,
    target: &str,
) -> Result<(&'static str, Option<Uuid>), intro::IntroError> {
    if event.is_nil()
        || target.len() > 4096
        || target.contains('#')
        || target.chars().any(char::is_control)
    {
        return Err(intro::IntroError::Invalid);
    }
    let (path, query) = target
        .split_once('?')
        .map_or((target, None), |(a, b)| (a, Some(b)));
    let base = format!("/api/concert/events/{event}/");
    let path = path.strip_prefix(&base).ok_or(intro::IntroError::Invalid)?;
    match (method, path, query) {
        ("GET", "intros", _) => Ok(("listConcertIntros", None)),
        ("GET", path, None) if path.starts_with("intros/") => {
            let id = crate::product_rpc::canonical_uuid(&path["intros/".len()..])
                .map_err(|_| intro::IntroError::Invalid)?;
            Ok(("getConcertIntro", Some(id)))
        }
        ("GET", "me/intro", None) => Ok(("getOwnConcertIntro", None)),
        ("PUT", "me/intro", None) => Ok(("setConcertIntro", None)),
        ("POST", "me/intro/publish", None) => Ok(("publishConcertIntro", None)),
        ("POST", "me/intro/close", None) => Ok(("closeConcertIntro", None)),
        _ => Err(intro::IntroError::Invalid),
    }
}
pub fn parse_query(target: &str) -> Result<intro::wire::Query, intro::IntroError> {
    let Some((_, query)) = target.split_once('?') else {
        return intro::wire::Query::validate(None, None, None);
    };
    if query.is_empty() || query.len() > 4096 {
        return Err(IntroError::Invalid);
    }
    let mut values = std::collections::BTreeMap::new();
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').ok_or(IntroError::Invalid)?;
        let key = decode(key)?;
        if !matches!(key.as_str(), "limit" | "cursor" | "q")
            || values.insert(key, decode(value)?).is_some()
        {
            return Err(IntroError::Invalid);
        }
    }
    let limit = values
        .remove("limit")
        .map(|v| {
            if v.is_empty() || !v.bytes().all(|v| v.is_ascii_digit()) {
                return Err(IntroError::Invalid);
            }
            v.parse::<u16>().map_err(|_| IntroError::Invalid)
        })
        .transpose()?;
    let cursor = values.remove("cursor");
    if cursor.as_ref().is_some_and(|v| {
        v.is_empty()
            || !v
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    }) {
        return Err(IntroError::Invalid);
    }
    intro::wire::Query::validate(limit, cursor, values.remove("q"))
}

fn decode(value: &str) -> Result<String, IntroError> {
    let mut out = Vec::with_capacity(value.len());
    let mut source = value.as_bytes().iter().copied();
    while let Some(byte) = source.next() {
        out.push(match byte {
            b'+' => b' ',
            b'%' => {
                let hi = source
                    .next()
                    .and_then(|v| (v as char).to_digit(16))
                    .ok_or(IntroError::Invalid)?;
                let lo = source
                    .next()
                    .and_then(|v| (v as char).to_digit(16))
                    .ok_or(IntroError::Invalid)?;
                ((hi << 4) | lo) as u8
            }
            other => other,
        });
    }
    String::from_utf8(out).map_err(|_| IntroError::Invalid)
}
mod command_wire {
    use super::*;
    use concert_application::intro::{DraftInput, IntroCommand, IntroError};
    use concert_domain::{AuthUserId, EventId, Revision};

    fn required_nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
        d: D,
    ) -> Result<Option<T>, D::Error> {
        Option::<T>::deserialize(d)
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct BoundsBody {
        #[serde(deserialize_with = "required_nullable")]
        minimum_age: Option<i16>,
        #[serde(deserialize_with = "required_nullable")]
        maximum_age_exclusive: Option<i16>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SetBody {
        expected_revision: i64,
        text: String,
        age_bounds: BoundsBody,
        #[serde(deserialize_with = "required_nullable")]
        expires_at: Option<DateTime<Utc>>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RevisionBody {
        expected_revision: i64,
    }

    /// Parse the frozen closed C04 write body; identity and operation come from the
    /// verified root, never the JSON body. Nullable fields must still be present.
    pub fn parse_intro_command(
        actor: AuthUserId,
        event: EventId,
        key: Uuid,
        operation: &str,
        body: &[u8],
    ) -> Result<intro::IntroCommand, intro::IntroError> {
        if body.len() > 16 * 1024 || key.is_nil() {
            return Err(IntroError::Invalid);
        }
        match operation {
            "setConcertIntro" => {
                // Derived closed structs reject duplicate fields before canonicalization.
                let raw: SetBody = serde_json::from_slice(body).map_err(|_| IntroError::Invalid)?;
                let draft = DraftInput {
                    text: raw.text,
                    minimum_age: raw.age_bounds.minimum_age,
                    maximum_age_exclusive: raw.age_bounds.maximum_age_exclusive,
                    expires_at: raw.expires_at,
                }
                .validate_at(Utc::now())?;
                IntroCommand::set(actor, event, key, raw.expected_revision, draft)
            }
            "publishConcertIntro" | "closeConcertIntro" => {
                let raw: RevisionBody =
                    serde_json::from_slice(body).map_err(|_| IntroError::Invalid)?;
                let expected =
                    Revision::new(raw.expected_revision).map_err(|_| IntroError::Invalid)?;
                Ok(if operation == "publishConcertIntro" {
                    IntroCommand::publish(actor, event, key, expected)
                } else {
                    IntroCommand::close(actor, event, key, expected)
                })
            }
            _ => Err(IntroError::Invalid),
        }
    }
}
pub use command_wire::parse_intro_command;
