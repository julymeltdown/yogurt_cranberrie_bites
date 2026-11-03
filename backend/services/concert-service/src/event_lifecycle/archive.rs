//! Strict C04 anonymous archive projection. No live feed/profile/media fallback.
use super::{invalid, unavailable};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::commands::CommandError;
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;
#[derive(Clone, Debug, Serialize)]
pub struct ArchivePost {
    pub post_id: Uuid,
    pub text: String,
    pub media_ids: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveMural {
    pub snapshot_id: Uuid,
    pub event_id: Uuid,
    pub revision: i64,
    pub state: String,
    pub width: u16,
    pub height: u16,
    pub palette: Vec<String>,
    pub pixels: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub server_time: DateTime<Utc>,
    pub cooldown_ms: u32,
    pub next_allowed_at: Option<DateTime<Utc>>,
}
impl ArchiveMural {
    pub fn validate(&self, event: Uuid) -> Result<(), CommandError> {
        if self.snapshot_id.is_nil()
            || self.event_id != event
            || !(1..=concert_domain::MAX_REVISION).contains(&self.revision)
            || self.state != "FROZEN"
            || self.width != 128
            || self.height != 128
            || self.pixels.len() != 16384
            || self.pixels.iter().any(|v| *v > 15)
            || self.palette.len() != 16
            || self.palette.iter().any(|s| {
                s.len() != 7
                    || !s.starts_with('#')
                    || !s[1..]
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
            })
            || self.next_allowed_at.is_some()
            || self.cooldown_ms > 3600000
            || self.created_at > self.server_time
        {
            return Err(CommandError::InvalidState);
        }
        Ok(())
    }
}
#[derive(Serialize)]
pub struct ArchiveFeed {
    pub items: Vec<ArchivePost>,
    pub next_cursor: Option<String>,
}
#[derive(Serialize)]
pub struct Archive {
    pub event_id: Uuid,
    pub state: &'static str,
    pub feed: ArchiveFeed,
    pub final_mural: ArchiveMural,
    pub revision: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub created_at: DateTime<Utc>,
    pub post_id: Uuid,
}
impl ArchivePost {
    fn position(&self) -> Position {
        Position {
            created_at: self.created_at,
            post_id: self.post_id,
        }
    }
}
pub struct PublicPage {
    pub source_revision: i64,
    pub items: Vec<ArchivePost>,
    pub has_more: bool,
}
#[async_trait]
pub trait PublicArchiveOwner: Send + Sync {
    /// Post/media owners enforce public publication, semantic privacy review/redaction,
    /// deletion/moderation and independent public-image qualification. No user auth
    /// claims are accepted here. Source revision covers EVERY permitted content change.
    /// Missing owners fail closed, even when the requested page would be empty.
    async fn page_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        upper: DateTime<Utc>,
        after: Option<Position>,
        limit: u16,
    ) -> Result<PublicPage, CommandError>;
    /// Must re-read current deletion/moderation/publication/media/review revisions and
    /// retain the owning-service fence through this protected read. Network timeouts
    /// are unavailable, never permission to serve a stored copy.
    async fn fence_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        source_revision: i64,
    ) -> Result<(), CommandError>;
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    event: Uuid,
    revision: i64,
    upper: DateTime<Utc>,
    after: Position,
    limit: u16,
    expires: DateTime<Utc>,
}
pub struct ArchiveCursor {
    key: aead::LessSafeKey,
}
impl ArchiveCursor {
    pub fn new(secret: &[u8; 32]) -> Result<Self, CommandError> {
        if *secret == [0; 32] {
            return Err(CommandError::InvalidInput);
        }
        Ok(Self {
            key: aead::LessSafeKey::new(
                aead::UnboundKey::new(&aead::AES_256_GCM, secret).map_err(invalid)?,
            ),
        })
    }
    fn seal(&self, cursor: Cursor) -> Result<String, CommandError> {
        let mut nonce = [0; 12];
        SystemRandom::new().fill(&mut nonce).map_err(invalid)?;
        let mut bytes = serde_json::to_vec(&cursor).map_err(invalid)?;
        self.key
            .seal_in_place_append_tag(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(b"mcr-public-archive-v1"),
                &mut bytes,
            )
            .map_err(invalid)?;
        let all: Vec<_> = nonce.into_iter().chain(bytes).collect();
        Ok(all.iter().map(|v| format!("{v:02x}")).collect())
    }
    fn open(
        &self,
        token: &str,
        event: Uuid,
        limit: u16,
        at: DateTime<Utc>,
    ) -> Result<Cursor, CommandError> {
        if token.len() > 1024
            || token.len() < 58
            || token.len() % 2 != 0
            || !token
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(CommandError::InvalidInput);
        }
        let mut bytes = (0..token.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&token[i..i + 2], 16).map_err(|_| CommandError::InvalidInput)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let nonce: [u8; 12] = bytes[..12].try_into().map_err(invalid)?;
        let plain = self
            .key
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(b"mcr-public-archive-v1"),
                &mut bytes[12..],
            )
            .map_err(|_| CommandError::InvalidInput)?;
        let c: Cursor = serde_json::from_slice(plain).map_err(|_| CommandError::InvalidInput)?;
        if c.event != event
            || c.limit != limit
            || c.revision < 1
            || c.expires <= at
            || c.expires > at + Duration::minutes(15)
            || c.upper > at
            || c.after.created_at > c.upper
            || c.after.post_id.is_nil()
        {
            return Err(CommandError::InvalidInput);
        }
        Ok(c)
    }
}
pub struct PgPublicArchive<P> {
    pool: PgPool,
    owner: P,
    cursors: ArchiveCursor,
}
impl<P: PublicArchiveOwner> PgPublicArchive<P> {
    pub fn new(pool: PgPool, owner: P, cursors: ArchiveCursor) -> Self {
        Self {
            pool,
            owner,
            cursors,
        }
    }
    pub async fn page(
        &self,
        event: Uuid,
        limit: u16,
        token: Option<&str>,
    ) -> Result<Archive, CommandError> {
        if event.is_nil() || !(1..=100).contains(&limit) {
            return Err(CommandError::InvalidInput);
        }
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        let phase: Option<String> =
            sqlx::query_scalar("select phase from concert_events where event_id=$1 for share")
                .bind(event)
                .fetch_optional(&mut *tx)
                .await
                .map_err(unavailable)?;
        if phase.as_deref() != Some("ARCHIVED") {
            return Err(CommandError::Denied);
        }
        let at: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(unavailable)?;
        let cursor = token
            .map(|t| self.cursors.open(t, event, limit, at))
            .transpose()?;
        let row=sqlx::query("select revision,source_revision,final_mural from concert_closure_archives where event_id=$1 for update").bind(event).fetch_optional(&mut *tx).await.map_err(unavailable)?.ok_or(CommandError::Unavailable)?;
        let mut revision: i64 = row.try_get("revision").map_err(invalid)?;
        if cursor.as_ref().is_some_and(|c| c.revision != revision) {
            return Err(CommandError::Conflict);
        }
        let upper = cursor.as_ref().map_or(at, |c| c.upper);
        let after = cursor.as_ref().map(|c| c.after);
        let page = self
            .owner
            .page_in(&mut tx, event, upper, after, limit)
            .await?;
        let prior: i64 = row.try_get("source_revision").map_err(invalid)?;
        if page.source_revision < prior || page.source_revision < 1 {
            return Err(CommandError::InvalidState);
        }
        if page.source_revision != prior {
            if cursor.is_some() {
                return Err(CommandError::Conflict);
            }
            revision = revision
                .checked_add(1)
                .filter(|r| *r <= concert_domain::MAX_REVISION)
                .ok_or(CommandError::InvalidState)?;
            sqlx::query("update concert_closure_archives set revision=$2,source_revision=$3 where event_id=$1").bind(event).bind(revision).bind(page.source_revision).execute(&mut *tx).await.map_err(unavailable)?;
        }
        if page.items.len() > usize::from(limit)
            || (page.has_more && page.items.is_empty())
            || page
                .items
                .windows(2)
                .any(|p| p[0].position() <= p[1].position())
            || page.items.iter().any(|p| {
                p.post_id.is_nil()
                    || p.text.trim().is_empty()
                    || p.text.chars().count() > 4000
                    || p.media_ids.len() > 4
                    || p.media_ids.iter().any(Uuid::is_nil)
                    || p.created_at > upper
                    || after.is_some_and(|a| p.position() >= a)
            })
        {
            return Err(CommandError::InvalidState);
        }
        let mut mural: ArchiveMural =
            serde_json::from_value(row.try_get("final_mural").map_err(invalid)?)
                .map_err(invalid)?;
        mural.server_time = at;
        mural.validate(event)?;
        let next_cursor = if page.has_more {
            Some(
                self.cursors.seal(Cursor {
                    event,
                    revision,
                    upper,
                    after: page
                        .items
                        .last()
                        .ok_or(CommandError::InvalidState)?
                        .position(),
                    limit,
                    expires: at + Duration::minutes(15),
                })?,
            )
        } else {
            None
        };
        self.owner
            .fence_in(&mut tx, event, page.source_revision)
            .await?;
        // Public reads do not use or mutate operator/identity authorization state.
        tx.commit().await.map_err(unavailable)?;
        Ok(Archive {
            event_id: event,
            state: "ARCHIVED",
            feed: ArchiveFeed {
                items: page.items,
                next_cursor,
            },
            final_mural: mural,
            revision,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archive_cursor_rejects_tampering_foreign_event_limit_and_expiry() {
        let codec = ArchiveCursor::new(&[0x41; 32]).unwrap();
        let at = Utc::now();
        let event = Uuid::new_v4();
        let token = codec
            .seal(Cursor {
                event,
                revision: 7,
                upper: at,
                after: Position {
                    created_at: at,
                    post_id: Uuid::new_v4(),
                },
                limit: 20,
                expires: at + Duration::minutes(15),
            })
            .unwrap();
        assert_eq!(codec.open(&token, event, 20, at).unwrap().revision, 7);
        assert!(codec.open(&token, Uuid::new_v4(), 20, at).is_err());
        assert!(codec.open(&token, event, 21, at).is_err());
        assert!(codec
            .open(&token, event, 20, at + Duration::minutes(15))
            .is_err());
        let mut changed = token.into_bytes();
        changed[30] = if changed[30] == b'a' { b'b' } else { b'a' };
        assert!(codec
            .open(std::str::from_utf8(&changed).unwrap(), event, 20, at)
            .is_err());
    }
    #[test]
    fn mural_projection_rejects_nonfrozen_or_private_unknown_fields() {
        let value = serde_json::json!({"snapshot_id":Uuid::new_v4(),"event_id":Uuid::new_v4(),"revision":1,"state":"FROZEN","width":128,"height":128,"palette":vec!["#000000";16],"pixels":vec![0;16384],"created_at":Utc::now(),"server_time":Utc::now(),"cooldown_ms":0,"next_allowed_at":null,"author_id":Uuid::new_v4()});
        assert!(serde_json::from_value::<ArchiveMural>(value).is_err());
    }
}
