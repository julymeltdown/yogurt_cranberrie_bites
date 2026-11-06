//! Closed C04 form query parsing before any owner work is acquired. The raw
//! target remains unchanged in the signed parent and canonical guard binding.
use crate::feed::live_read::{CommentsQuery, FeedQuery};
use chrono::{DateTime, Utc};
use concert_application::commands::RegisteredOperation;
use concert_domain::feed::Filter;
use tonic::Status;
use uuid::Uuid;

pub(super) fn report_request(
    request: &super::proto::ProductRequest,
    expected: super::ReportOperation,
) -> Result<(), Status> {
    expected.validate(request)
}

pub(super) fn operations_request(
    request: &super::proto::ProductRequest,
    expected: super::OperationsOperation,
) -> Result<(), Status> {
    expected.validate(request)
}

pub(super) fn age_request(
    request: &super::proto::ProductRequest,
    expected: super::AgeOperation,
) -> Result<(), Status> {
    super::age_admission::request(request, expected)
}

pub(super) fn social_request(
    request: &super::proto::ProductRequest,
    expected: crate::social_live::protocol::SocialOperation,
) -> Result<(), Status> {
    let decoded = crate::social_live::protocol::SocialRequest::parse(
        super::canonical_uuid(&request.user_id)?,
        super::canonical_uuid(&request.event_id)?,
        &request.http_method,
        &request.http_target,
        &request.http_body,
        &request.content_type,
        &request.idempotency_key,
    )?;
    if decoded.operation() != expected {
        return Err(invalid());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandStatusQuery {
    pub operation: RegisteredOperation,
    pub key: Uuid,
}
pub(super) fn command_status(target: &str) -> Result<CommandStatusQuery, Status> {
    if target.len() > 4096 {
        return Err(invalid());
    }
    let query = target.split_once('?').ok_or_else(invalid)?.1;
    let (mut operation, mut key) = (None, None);
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').ok_or_else(invalid)?;
        let name = decode(name)?;
        let value = decode(value)?;
        match name.as_str() {
            "operation_id" if operation.is_none() => {
                operation = Some(RegisteredOperation::find(&value).map_err(|_| invalid())?)
            }
            "idempotency_key" if key.is_none() => key = Some(super::canonical_uuid(&value)?),
            _ => return Err(invalid()),
        }
    }
    Ok(CommandStatusQuery {
        operation: operation.ok_or_else(invalid)?,
        key: key.ok_or_else(invalid)?,
    })
}

/// Strict C04 form query for the two bodyless DELETE commands. Query presence
/// is required and every expected revision appears exactly once.
pub(super) fn party_command(target: &str, operation: super::Operation) -> Result<(), Status> {
    let fields: &[&str] = match operation {
        super::Operation::CancelPartyApplication(_) => &["expected_revision"],
        super::Operation::RemovePartyMember(_, _) => &["expected_revision", "party_revision"],
        _ => return Ok(()),
    };
    let query = target
        .split_once('?')
        .map(|(_, value)| value)
        .ok_or_else(invalid)?;
    let mut seen = 0u8;
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').ok_or_else(invalid)?;
        let name = decode_without_plus(name)?;
        let value = decode_without_plus(value)?;
        let bit = fields
            .iter()
            .position(|field| *field == name)
            .filter(|position| seen & (1 << position) == 0)
            .ok_or_else(invalid)?;
        seen |= 1 << bit;
        if value.is_empty()
            || (value.len() > 1 && value.starts_with('0'))
            || !value.bytes().all(|byte| byte.is_ascii_digit())
            || value
                .parse::<i64>()
                .ok()
                .is_none_or(|revision| !(1..=9_007_199_254_740_991).contains(&revision))
        {
            return Err(invalid());
        }
    }
    if (seen as usize) == (1usize << fields.len()) - 1 {
        Ok(())
    } else {
        Err(invalid())
    }
}

fn decode_without_plus(value: &str) -> Result<String, Status> {
    if value.contains('+') {
        return Err(invalid());
    }
    decode(value)
}

fn invalid() -> Status {
    Status::invalid_argument("Invalid Concert product query")
}
pub(super) fn comments(target: &str) -> Result<CommentsQuery, Status> {
    let query = feed(target)?;
    if query.filter.is_some() || query.before.is_some() {
        return Err(invalid());
    }
    Ok(CommentsQuery {
        limit: query.limit,
        cursor: query.cursor,
    })
}

pub(super) fn feed(target: &str) -> Result<FeedQuery, Status> {
    if target.len() > 4096 {
        return Err(invalid());
    }
    let mut result = FeedQuery::default();
    let query = target.split_once('?').map_or("", |(_, query)| query);
    if query.is_empty() {
        return Ok(result);
    }
    let mut seen = 0u8;
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').ok_or_else(invalid)?;
        let name = decode(name)?;
        let value = decode(value)?;
        let bit = match name.as_str() {
            "limit" => 1,
            "cursor" => 2,
            "filter" => 4,
            "before" => 8,
            _ => return Err(invalid()),
        };
        if seen & bit != 0 {
            return Err(invalid());
        }
        seen |= bit;
        match name.as_str() {
            "limit" => {
                if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Err(invalid());
                }
                let limit = value.parse::<u16>().map_err(|_| invalid())?;
                if !(1..=100).contains(&limit) {
                    return Err(invalid());
                }
                result.limit = Some(limit);
            }
            "cursor" => {
                if value.is_empty()
                    || value.len() > 1024
                    || !value
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
                {
                    return Err(invalid());
                }
                result.cursor = Some(value);
            }
            "filter" => {
                result.filter = Some(match value.as_str() {
                    "ALL" => Filter::All,
                    "WITH_MEDIA" => Filter::WithMedia,
                    "PARTY_LINK" => Filter::PartyLink,
                    "GIVEAWAY_LINK" => Filter::GiveawayLink,
                    _ => return Err(invalid()),
                });
            }
            "before" => {
                if !value.ends_with('Z') {
                    return Err(invalid());
                }
                result.before = Some(
                    DateTime::parse_from_rfc3339(&value)
                        .map_err(|_| invalid())?
                        .with_timezone(&Utc),
                );
            }
            _ => unreachable!("closed parameter set"),
        }
    }
    Ok(result)
}

pub(super) fn decode(value: &str) -> Result<String, Status> {
    let mut bytes = Vec::with_capacity(value.len());
    let mut input = value.bytes();
    while let Some(byte) = input.next() {
        bytes.push(match byte {
            b'+' => b' ',
            b'%' => {
                let high = input.next().and_then(hex).ok_or_else(invalid)?;
                let low = input.next().and_then(hex).ok_or_else(invalid)?;
                high * 16 + low
            }
            byte => byte,
        });
    }
    let result = String::from_utf8(bytes).map_err(|_| invalid())?;
    if result.chars().any(char::is_control) {
        return Err(invalid());
    }
    Ok(result)
}
fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn social_delete_and_reveal_use_actual_frame_queries_without_key_or_body_fallback() {
        use crate::social_live::protocol::SocialOperation;
        let event = Uuid::new_v4();
        let actor = Uuid::new_v4();
        let id = Uuid::new_v4();
        let mut input = super::super::proto::ProductRequest {
            user_id: actor.to_string(),
            event_id: event.to_string(),
            http_method: "DELETE".into(),
            http_target: format!(
                "/api/concert/events/{event}/social-requests/{id}?expected_revision=1"
            ),
            idempotency_key: Uuid::new_v4().to_string(),
            ..Default::default()
        };
        assert!(social_request(&input, SocialOperation::Cancel(id)).is_ok());
        input.idempotency_key.clear();
        assert!(social_request(&input, SocialOperation::Cancel(id)).is_err());
        input.http_method = "GET".into();
        input.http_target =
            format!("/api/concert/events/{event}/social-requests/{id}/reveal?platform=INSTAGRAM");
        assert!(social_request(&input, SocialOperation::Reveal(id)).is_ok());
        input.http_target.push_str("&platform=X");
        assert!(social_request(&input, SocialOperation::Reveal(id)).is_err());
    }

    #[test]
    fn accepts_frozen_fields_and_single_decoding_without_changing_target() {
        let target =
            "/feed?limit=100&filter=WITH_MEDIA&cursor=Ab_9-x&before=2026-11-04T01%3A02%3A03.123Z";
        let query = feed(target).unwrap();
        assert_eq!(query.limit, Some(100));
        assert_eq!(query.filter, Some(Filter::WithMedia));
        assert_eq!(query.cursor.as_deref(), Some("Ab_9-x"));
        assert_eq!(query.before.unwrap().timestamp_millis(), 1793754123123);
        assert!(target.contains("%3A"));
        assert_eq!(feed("/feed").unwrap().limit, None);
        assert!(feed("/feed?").unwrap().filter.is_none());
        assert!(feed("/feed?cursor=A%5Fb").is_ok());
        assert!(feed(&format!("/feed?cursor={}", "a".repeat(1024))).is_ok());
    }

    #[test]
    fn rejects_ambiguous_unknown_and_malformed_form_queries() {
        for query in [
            "limit=1&limit=2",
            "limit=1&%6cimit=2",
            "limit=1&",
            "&limit=1",
            "limit",
            "limit=",
            "limit=0",
            "limit=101",
            "limit=65536",
            "limit=+1",
            "limit=-1",
            "limit=1.0",
            "limit=%201",
            "unknown=1",
            "filter=all",
            "filter=",
            "cursor=",
            "cursor=a=",
            "cursor=a+b",
            "cursor=%252F",
            "cursor=%FF",
            "cursor=%",
            "cursor=%G0",
            "cursor=a%00",
            "cursor=a%0A",
            "before=2026-11-04T01:02:03+00:00",
            "before=2026-11-04T01:02:03%2B00:00",
            "before=invalidZ",
        ] {
            assert!(feed(&format!("/feed?{query}")).is_err(), "{query}");
        }
        assert!(feed(&format!("/feed?cursor={}", "a".repeat(1025))).is_err());
    }
    #[test]
    fn comments_accepts_only_its_frozen_limit_and_cursor() {
        assert_eq!(
            comments("/comments?limit=20&cursor=A_b9").unwrap().limit,
            Some(20)
        );
        for query in [
            "filter=ALL",
            "before=2026-11-04T01:02:03Z",
            "limit=1&%6cimit=2",
        ] {
            assert!(comments(&format!("/comments?{query}")).is_err());
        }
    }
    #[test]
    fn command_status_binds_only_registered_operation_and_exact_key_without_public_target() {
        let key = Uuid::from_u128(12);
        let valid = format!("/commands?operation_id=createConcertGiveaway&idempotency_key={key}");
        let parsed = command_status(&valid).unwrap();
        assert_eq!(parsed.operation.id(), "createConcertGiveaway");
        assert_eq!(parsed.key, key);
        for bad in [
            "",
            "?",
            "?operation_id=createConcertGiveaway",
            "?operation_id=arbitrary&idempotency_key=bad",
        ] {
            assert!(command_status(&format!("/commands{bad}")).is_err());
        }
        for extra in [
            "&target_id=anything",
            "&operation_id=createConcertGiveaway",
            "&%6fperation_id=createConcertGiveaway",
            "&idempotency_key=bad",
            "&",
            "&unknown=1",
        ] {
            assert!(command_status(&format!("{valid}{extra}")).is_err());
        }
    }
}

/// Wall admission parses closed original bytes; scope acquisition remains Root-owned.
pub(super) fn wall_request(m: &super::proto::ProductRequest, op: crate::mural_tools_b::protocol::WallOperation) -> Result<(), tonic::Status> {
    if op.mutation() {
        super::canonical_uuid(&m.idempotency_key)?;
        if m.content_type != "application/json" { return Err(tonic::Status::invalid_argument("Invalid Wall media type")); }
        crate::mural_tools_b::protocol::body(op, &m.http_body)?;
    } else if !m.http_body.is_empty() || !m.content_type.is_empty() || !m.idempotency_key.is_empty() {
        return Err(tonic::Status::invalid_argument("Unexpected Wall read payload"));
    }
    Ok(())
}
