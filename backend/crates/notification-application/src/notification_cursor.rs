use crate::errors::{NotificationError, NotificationInvalidInput, NotificationResult as Result};
use crate::notification_contracts::{NotificationCursor, NotificationPosition};
use chrono::{DateTime, Utc};
use uuid::Uuid;

const VERSION: &str = "v2";
pub const MAX_NOTIFICATION_CURSOR_CHARS: usize = 512;

pub fn parse(recipient_id: Uuid, value: &str) -> Result<Option<NotificationCursor>> {
    // Match the public list-notifications contract and reject oversized input
    // before trimming or allocating a split vector. Count Unicode scalar
    // values to match OpenAPI's maxLength semantics for strings.
    if value
        .chars()
        .take(MAX_NOTIFICATION_CURSOR_CHARS + 1)
        .count()
        > MAX_NOTIFICATION_CURSOR_CHARS
    {
        return Err(NotificationInvalidInput::InvalidCursor.into());
    }

    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.starts_with("v2:") {
        let parts: Vec<&str> = value.split(':').collect();
        if parts.len() != 4 || parts[0] != VERSION {
            return Err(NotificationInvalidInput::InvalidCursor.into());
        }
        let bound_recipient = Uuid::parse_str(parts[1]).map_err(|_| {
            NotificationError::InvalidInput(NotificationInvalidInput::InvalidCursor)
        })?;
        if bound_recipient != recipient_id {
            return Err(NotificationInvalidInput::CursorRecipientMismatch.into());
        }
        let epoch_micros = parts[2].parse::<i64>().map_err(|_| {
            NotificationError::InvalidInput(NotificationInvalidInput::InvalidCursor)
        })?;
        let created_at = DateTime::<Utc>::from_timestamp_micros(epoch_micros).ok_or(
            NotificationError::InvalidInput(NotificationInvalidInput::InvalidCursor),
        )?;
        let id = Uuid::parse_str(parts[3]).map_err(|_| {
            NotificationError::InvalidInput(NotificationInvalidInput::InvalidCursor)
        })?;
        return Ok(Some(NotificationCursor::V2 {
            recipient_id,
            position: NotificationPosition { created_at, id },
        }));
    }

    // Explicit legacy compatibility: preserve the old <millis>:<uuid> shape,
    // but do not pretend multiplying milliseconds restores precision. The
    // persistence adapter must resolve the anchor row under this recipient.
    let (epoch_millis, id) = value
        .split_once(':')
        .ok_or(NotificationError::InvalidInput(
            NotificationInvalidInput::InvalidCursor,
        ))?;
    if id.contains(':') {
        return Err(NotificationInvalidInput::InvalidCursor.into());
    }
    let epoch_millis = epoch_millis
        .parse::<i64>()
        .map_err(|_| NotificationError::InvalidInput(NotificationInvalidInput::InvalidCursor))?;
    let id = Uuid::parse_str(id)
        .map_err(|_| NotificationError::InvalidInput(NotificationInvalidInput::InvalidCursor))?;
    Ok(Some(NotificationCursor::Legacy { epoch_millis, id }))
}

pub fn encode(recipient_id: Uuid, position: NotificationPosition) -> String {
    format!(
        "{VERSION}:{recipient_id}:{}:{}",
        position.created_at.timestamp_micros(),
        position.id
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use proptest::prelude::*;

    fn uuid(value: &str) -> Uuid {
        Uuid::parse_str(value).expect("uuid")
    }

    #[test]
    fn v2_round_trip_preserves_microseconds_and_recipient() {
        let recipient = uuid("11111111-1111-1111-1111-111111111111");
        let position = NotificationPosition {
            created_at: Utc
                .timestamp_micros(1_700_000_000_123_456)
                .single()
                .unwrap(),
            id: uuid("22222222-2222-2222-2222-222222222222"),
        };
        let token = encode(recipient, position);
        assert_eq!(
            parse(recipient, &token).unwrap(),
            Some(NotificationCursor::V2 {
                recipient_id: recipient,
                position,
            })
        );
    }

    #[test]
    fn v2_is_recipient_bound() {
        let first = uuid("11111111-1111-1111-1111-111111111111");
        let second = uuid("22222222-2222-2222-2222-222222222222");
        let token = encode(
            first,
            NotificationPosition {
                created_at: Utc.timestamp_micros(1).single().unwrap(),
                id: first,
            },
        );
        assert_eq!(
            parse(second, &token).unwrap_err().to_string(),
            "Cursor recipient mismatch"
        );
    }

    #[test]
    fn legacy_shape_is_preserved_without_fabricating_precision() {
        let recipient = uuid("11111111-1111-1111-1111-111111111111");
        let id = uuid("22222222-2222-2222-2222-222222222222");
        assert_eq!(
            parse(recipient, &format!("123:{id}")).unwrap(),
            Some(NotificationCursor::Legacy {
                epoch_millis: 123,
                id,
            })
        );
    }

    #[test]
    fn malformed_tokens_are_rejected() {
        let recipient = uuid("11111111-1111-1111-1111-111111111111");
        for token in ["bad", "v2:bad:1:bad", "v2:11111111-1111-1111-1111-111111111111:not-a-number:22222222-2222-2222-2222-222222222222", "1:22222222-2222-2222-2222-222222222222:extra"] {
            assert_eq!(parse(recipient, token).unwrap_err().to_string(), "Invalid cursor format");
        }
    }

    #[test]
    fn cursor_at_the_contract_length_limit_is_accepted() {
        let recipient = uuid("11111111-1111-1111-1111-111111111111");
        let position = NotificationPosition {
            created_at: Utc.timestamp_micros(1).single().unwrap(),
            id: uuid("22222222-2222-2222-2222-222222222222"),
        };
        let token = encode(recipient, position);
        let padded = format!(
            "{}{}",
            token,
            " ".repeat(MAX_NOTIFICATION_CURSOR_CHARS - token.chars().count())
        );

        assert_eq!(padded.chars().count(), MAX_NOTIFICATION_CURSOR_CHARS);
        assert_eq!(
            parse(recipient, &padded).unwrap(),
            Some(NotificationCursor::V2 {
                recipient_id: recipient,
                position,
            })
        );
    }

    #[test]
    fn oversized_unicode_whitespace_is_rejected_before_trimming() {
        let recipient = uuid("11111111-1111-1111-1111-111111111111");
        let value = "\u{2003}".repeat(MAX_NOTIFICATION_CURSOR_CHARS + 1);

        assert_eq!(
            parse(recipient, &value).unwrap_err().to_string(),
            "Invalid cursor format"
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            failure_persistence: Some(Box::new(
                proptest::test_runner::FileFailurePersistence::WithSource("proptest-regressions")
            )),
            ..ProptestConfig::default()
        })]

        #[test]
        fn parser_rejects_every_input_over_the_character_limit(chars in prop::collection::vec(any::<char>(), 0..=MAX_NOTIFICATION_CURSOR_CHARS + 16)) {
            let recipient = uuid("11111111-1111-1111-1111-111111111111");
            let value: String = chars.into_iter().collect();
            let length = value.chars().take(MAX_NOTIFICATION_CURSOR_CHARS + 1).count();

            if length > MAX_NOTIFICATION_CURSOR_CHARS {
                prop_assert_eq!(
                    parse(recipient, &value).unwrap_err().to_string(),
                    "Invalid cursor format"
                );
            } else if let Ok(Some(NotificationCursor::V2 { recipient_id, position })) = parse(recipient, &value) {
                let encoded = encode(recipient_id, position);
                prop_assert_eq!(
                    parse(recipient_id, &encoded).unwrap(),
                    Some(NotificationCursor::V2 { recipient_id, position })
                );
            }
        }
    }
}
