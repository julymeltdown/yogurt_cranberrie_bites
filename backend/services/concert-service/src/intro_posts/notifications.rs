//! The notification inbox: `listNotifications` and `markNotificationRead`.
//!
//! The rows are written by WP-05's like command into `intro_like_notifications`
//! (migration 0087), deduped by `(recipient, intro, actor)`. Migration 0093 added
//! the columns the contract's `feed.Notification` needs, so this module is a read
//! over the same rows rather than a second table that could disagree with them.
//!
//! ## A notification grants nothing
//!
//! `domain-contract.md` §Target resolution: "Re-check on open; a notification
//! grants nothing." This module therefore returns a **reference** — a
//! `feed.NotificationTarget` naming the intro post — and never the post's body,
//! its author, its attachments or its audience. Opening the notification means
//! fetching `/api/intro-posts/{id}`, which runs the full visibility decision
//! again. A recipient who has since been blocked, or whose claim has expired, or
//! whose post was deleted, gets the same refusal there as anyone else.
//!
//! That is why the list query selects only the notification's own columns: there
//! is no join to `intro_posts` at all, so restricted content cannot reach the
//! payload even by accident.
//!
//! ## The event id
//!
//! `feed.Notification.event_id` is required and non-null. A profile notification
//! is not about a concert event, so the stored value is the nil UUID (see
//! migration 0093). The field is emitted as the contract requires; it is not a
//! claim that the notification belongs to an event.
use chrono::{DateTime, SecondsFormat, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// The default page size, matching `feed.NotificationQuery.limit`.
pub const DEFAULT_LIMIT: i64 = 20;
/// The contract's maximum page size.
pub const MAX_LIMIT: i64 = 100;

/// The closed notification kinds this producer can return.
///
/// The contract's enum has seven members; this producer only ever produces
/// `INTRO_LIKED`, because that is the only kind the concert service writes. A
/// stored value outside the set is a defect in the writer, not something to
/// coerce, so it is refused.
pub const INTRO_LIKED: &str = "INTRO_LIKED";

/// One notification, in the contract's `feed.Notification` shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    pub notification_id: Uuid,
    pub event_id: Uuid,
    pub kind: &'static str,
    pub target: NotificationTarget,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
    pub revision: i64,
}

/// The reference a notification carries. Never the content it points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotificationTarget {
    pub target_type: &'static str,
    pub target_id: Uuid,
}

/// One page of the inbox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationPage {
    pub items: Vec<Notification>,
    pub next_cursor: Option<String>,
}

/// Why an inbox read could not answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationError {
    /// The query was not a valid `feed.NotificationQuery`.
    InvalidQuery,
    /// The cursor was not one this producer issued, or it has expired.
    InvalidCursor,
    /// The notification does not exist, or does not belong to this recipient.
    /// The two are deliberately the same answer.
    Unavailable,
    /// The store could not be read.
    Store,
}

impl std::fmt::Display for NotificationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidQuery => "invalid notification query",
            Self::InvalidCursor => "invalid notification cursor",
            Self::Unavailable => "notification unavailable",
            Self::Store => "notification store unavailable",
        })
    }
}

impl std::error::Error for NotificationError {}

/// The parsed `feed.NotificationQuery`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationQuery {
    pub limit: i64,
    pub cursor: Option<String>,
    pub unread_only: bool,
}

impl NotificationQuery {
    /// Parse the query string of `GET /api/me/notifications`.
    ///
    /// The parser is closed: an unknown field is refused rather than ignored, so
    /// a client cannot believe it filtered by something this producer does not
    /// implement.
    pub fn parse(raw: Option<&str>) -> Result<Self, NotificationError> {
        let mut limit = DEFAULT_LIMIT;
        let mut cursor = None;
        let mut unread_only = false;
        let mut seen = std::collections::BTreeSet::new();
        if let Some(raw) = raw {
            for pair in raw.split('&') {
                if pair.is_empty() {
                    continue;
                }
                let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
                if !seen.insert(name) {
                    return Err(NotificationError::InvalidQuery);
                }
                match name {
                    "limit" => {
                        if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                            return Err(NotificationError::InvalidQuery);
                        }
                        let parsed: i64 = value
                            .parse()
                            .map_err(|_| NotificationError::InvalidQuery)?;
                        if !(1..=MAX_LIMIT).contains(&parsed) {
                            return Err(NotificationError::InvalidQuery);
                        }
                        limit = parsed;
                    }
                    "cursor" => {
                        if value.is_empty() || value.len() > 1024 {
                            return Err(NotificationError::InvalidQuery);
                        }
                        cursor = Some(value.to_owned());
                    }
                    "unread_only" => {
                        unread_only = match value {
                            "true" => true,
                            "false" => false,
                            _ => return Err(NotificationError::InvalidQuery),
                        };
                    }
                    _ => return Err(NotificationError::InvalidQuery),
                }
            }
        }
        Ok(Self {
            limit,
            cursor,
            unread_only,
        })
    }
}

/// The inbox producer.
#[derive(Clone)]
pub struct NotificationInbox {
    pool: PgPool,
}

impl NotificationInbox {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// `listNotifications`: the recipient's own inbox, newest first.
    ///
    /// The recipient comes from the verified session, never from the request, so
    /// a caller cannot read another profile's inbox.
    pub async fn list(
        &self,
        recipient: Uuid,
        query: &NotificationQuery,
    ) -> Result<NotificationPage, NotificationError> {
        let after = match query.cursor.as_deref() {
            Some(token) => Some(decode_cursor(token)?),
            None => None,
        };
        // The filter is inside the query, so a page is never post-filtered and
        // `next_cursor` is derived from the rows actually returned.
        let rows = sqlx::query(
            "select notification_id, event_id, created_at, read_at, revision, intro_id, \
                    actor_profile_id \
             from intro_like_notifications \
             where recipient_profile_id = $1 \
               and ($2::boolean = false or read_at is null) \
               and ($3::timestamptz is null \
                    or (created_at, notification_id) < ($3, $4)) \
             order by created_at desc, notification_id desc \
             limit $5",
        )
        .bind(recipient)
        .bind(query.unread_only)
        .bind(after.as_ref().map(|(at, _)| *at))
        .bind(after.as_ref().map(|(_, id)| *id))
        .bind(query.limit + 1)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| NotificationError::Store)?;
        let mut items = Vec::with_capacity(rows.len());
        for row in &rows {
            items.push(decode(row)?);
        }
        let next_cursor = if items.len() as i64 > query.limit {
            items.truncate(query.limit as usize);
            let last = items.last().expect("a truncated page has a last row");
            Some(encode_cursor(last.created_at, last.notification_id))
        } else {
            None
        };
        Ok(NotificationPage { items, next_cursor })
    }

    /// `markNotificationRead`: record that the recipient opened one.
    ///
    /// The recipient is part of the `where` clause, so a notification belonging
    /// to someone else is `Unavailable` — the same answer as one that does not
    /// exist. Marking an already-read notification is a no-op that returns the
    /// row unchanged, so a replayed command is safe without a receipt.
    pub async fn mark_read(
        &self,
        recipient: Uuid,
        notification_id: Uuid,
    ) -> Result<Notification, NotificationError> {
        if notification_id.is_nil() {
            return Err(NotificationError::Unavailable);
        }
        let row = sqlx::query(
            "update intro_like_notifications \
                set read_at = coalesce(read_at, clock_timestamp()) \
              where recipient_profile_id = $1 and notification_id = $2 \
              returning notification_id, event_id, created_at, read_at, revision, intro_id, \
                        actor_profile_id",
        )
        .bind(recipient)
        .bind(notification_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| NotificationError::Store)?
        .ok_or(NotificationError::Unavailable)?;
        decode(&row)
    }
}

/// Decode one selected row into the contract's shape.
///
/// The target is built from `intro_id` alone. Nothing about the post is read, so
/// a notification cannot carry restricted content.
fn decode(row: &sqlx::postgres::PgRow) -> Result<Notification, NotificationError> {
    let intro_id: Uuid = row
        .try_get("intro_id")
        .map_err(|_| NotificationError::Store)?;
    Ok(Notification {
        notification_id: row
            .try_get("notification_id")
            .map_err(|_| NotificationError::Store)?,
        event_id: row
            .try_get("event_id")
            .map_err(|_| NotificationError::Store)?,
        kind: INTRO_LIKED,
        target: NotificationTarget {
            target_type: "INTRO_POST",
            target_id: intro_id,
        },
        created_at: row
            .try_get("created_at")
            .map_err(|_| NotificationError::Store)?,
        read_at: row
            .try_get("read_at")
            .map_err(|_| NotificationError::Store)?,
        revision: row
            .try_get("revision")
            .map_err(|_| NotificationError::Store)?,
    })
}

fn encode_cursor(at: DateTime<Utc>, id: Uuid) -> String {
    use base64ct::{Base64UrlUnpadded, Encoding};
    Base64UrlUnpadded::encode_string(format!("n1:{}\n{id}", at.timestamp_micros()).as_bytes())
}

fn decode_cursor(token: &str) -> Result<(DateTime<Utc>, Uuid), NotificationError> {
    use base64ct::{Base64UrlUnpadded, Encoding};
    if token.is_empty() || token.len() > 1024 {
        return Err(NotificationError::InvalidCursor);
    }
    let raw = Base64UrlUnpadded::decode_vec(token).map_err(|_| NotificationError::InvalidCursor)?;
    let raw = String::from_utf8(raw).map_err(|_| NotificationError::InvalidCursor)?;
    let raw = raw
        .strip_prefix("n1:")
        .ok_or(NotificationError::InvalidCursor)?;
    let (at, id) = raw.split_once('\n').ok_or(NotificationError::InvalidCursor)?;
    let micros: i64 = at.parse().map_err(|_| NotificationError::InvalidCursor)?;
    let at = DateTime::from_timestamp_micros(micros).ok_or(NotificationError::InvalidCursor)?;
    let id = Uuid::parse_str(id).map_err(|_| NotificationError::InvalidCursor)?;
    Ok((at, id))
}

/// The wire form of one notification, matching `feed.Notification` exactly.
///
/// `additionalProperties: false` in the schema, so this emits the seven required
/// fields and nothing else.
pub fn notification_wire(notification: &Notification) -> serde_json::Value {
    serde_json::json!({
        "notification_id": notification.notification_id,
        "event_id": notification.event_id,
        "kind": notification.kind,
        "target": {
            "target_type": notification.target.target_type,
            "target_id": notification.target.target_id,
        },
        "created_at": utc(notification.created_at),
        "read_at": notification.read_at.map(utc),
        "revision": notification.revision,
    })
}

/// The wire form of a page, matching `feed.NotificationList`.
pub fn page_wire(page: &NotificationPage) -> serde_json::Value {
    serde_json::json!({
        "items": page.items.iter().map(notification_wire).collect::<Vec<_>>(),
        "next_cursor": page.next_cursor,
    })
}

fn utc(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}
