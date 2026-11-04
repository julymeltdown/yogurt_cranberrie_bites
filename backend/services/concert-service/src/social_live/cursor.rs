use super::{
    keys::ManagedSocialKeys,
    protocol::{Direction, Page},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tonic::Status;
use uuid::Uuid;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    context: [u8; 32],
    last: Uuid,
    expires: i64,
}
fn invalid() -> Status {
    Status::invalid_argument("Invalid social cursor")
}
fn context(actor: Uuid, event: Uuid, operation: &str, page: &Page) -> [u8; 32] {
    let value = serde_json::json!([
        actor,
        event,
        operation,
        page.limit,
        page.state,
        page.direction.map(|d| if d == Direction::Incoming {
            "INCOMING"
        } else {
            "OUTGOING"
        })
    ]);
    Sha256::digest(value.to_string()).into()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(raw: &str) -> Result<Vec<u8>, Status> {
    if raw.len() % 2 != 0
        || !raw
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid());
    }
    raw.as_bytes()
        .chunks_exact(2)
        .map(|c| {
            u8::from_str_radix(std::str::from_utf8(c).map_err(|_| invalid())?, 16)
                .map_err(|_| invalid())
        })
        .collect()
}
pub(super) fn read(
    keys: &ManagedSocialKeys,
    actor: Uuid,
    event: Uuid,
    operation: &str,
    page: &Page,
) -> Result<Uuid, Status> {
    let Some(raw) = &page.cursor else {
        return Ok(Uuid::nil());
    };
    if raw.len() > 1024 {
        return Err(invalid());
    }
    let (payload, signature) = raw.split_once('_').ok_or_else(invalid)?;
    let payload = unhex(payload)?;
    let signature = unhex(signature)?;
    keys.cursor_check(&payload, &signature)?;
    let value: Cursor = serde_json::from_slice(&payload).map_err(|_| invalid())?;
    if value.context != context(actor, event, operation, page)
        || value.last.is_nil()
        || value.expires <= chrono::Utc::now().timestamp()
    {
        return Err(invalid());
    }
    Ok(value.last)
}
pub(super) fn write(
    keys: &ManagedSocialKeys,
    actor: Uuid,
    event: Uuid,
    operation: &str,
    page: &Page,
    last: Uuid,
) -> Result<String, Status> {
    let value = Cursor {
        context: context(actor, event, operation, page),
        last,
        expires: chrono::Utc::now().timestamp() + 300,
    };
    let payload = serde_json::to_vec(&value).map_err(|_| invalid())?;
    let token = format!("{}_{}", hex(&payload), hex(&keys.cursor_sign(&payload)));
    if token.len() > 1024 {
        return Err(invalid());
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::social_live::keys::KeyMaterial;
    fn keys() -> ManagedSocialKeys {
        ManagedSocialKeys::new(
            KeyMaterial::new("test-handle".into(), vec![3; 32]),
            vec![],
            KeyMaterial::new("test-command".into(), vec![5; 32]),
            vec![],
            &[7; 32],
        )
        .unwrap()
    }
    #[test]
    fn signed_cursor_cannot_cross_actor_event_operation_or_filter() {
        let keys = keys();
        let actor = Uuid::from_u128(1);
        let event = Uuid::from_u128(2);
        let last = Uuid::from_u128(3);
        let mut page = Page {
            limit: 20,
            ..Default::default()
        };
        page.cursor = Some(
            write(
                &keys,
                actor,
                event,
                "listConcertSocialRequests",
                &page,
                last,
            )
            .unwrap(),
        );
        assert_eq!(
            read(&keys, actor, event, "listConcertSocialRequests", &page).unwrap(),
            last
        );
        assert!(read(
            &keys,
            Uuid::from_u128(4),
            event,
            "listConcertSocialRequests",
            &page
        )
        .is_err());
        assert!(read(
            &keys,
            actor,
            Uuid::from_u128(4),
            "listConcertSocialRequests",
            &page
        )
        .is_err());
        assert!(read(&keys, actor, event, "listConcertBlocks", &page).is_err());
        page.direction = Some(Direction::Incoming);
        assert!(read(&keys, actor, event, "listConcertSocialRequests", &page).is_err());
    }
    #[test]
    fn changed_truncated_and_expired_cursor_cannot_advance_private_collection() {
        let keys = keys();
        let a = Uuid::from_u128(1);
        let e = Uuid::from_u128(2);
        let mut p = Page {
            limit: 20,
            ..Default::default()
        };
        let token = write(&keys, a, e, "listConcertBlocks", &p, Uuid::from_u128(3)).unwrap();
        let changed = if token.ends_with('0') { '1' } else { '0' };
        for bad in [
            format!("{}{changed}", &token[..token.len() - 1]),
            token[..token.len() - 2].into(),
            "00_00".into(),
        ] {
            p.cursor = Some(bad);
            assert!(read(&keys, a, e, "listConcertBlocks", &p).is_err());
        }
        let value = Cursor {
            context: context(a, e, "listConcertBlocks", &p),
            last: Uuid::from_u128(3),
            expires: chrono::Utc::now().timestamp() - 1,
        };
        let payload = serde_json::to_vec(&value).unwrap();
        p.cursor = Some(format!(
            "{}_{}",
            hex(&payload),
            hex(&keys.cursor_sign(&payload))
        ));
        assert!(read(&keys, a, e, "listConcertBlocks", &p).is_err());
    }
}
