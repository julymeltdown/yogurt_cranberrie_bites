//! VENU-BE-N01 PostgreSQL adapter. External owners stay behind fail-closed ports.
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::venue_notes::*;
use concert_domain::venue_notes::*;
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait NotePlaceAuthority: Send + Sync {
    async fn review(
        &self,
        connection: &mut PgConnection,
        event: Uuid,
        selection: &LandmarkSelection,
    ) -> Result<(), PlaceNoteError>;
    async fn current(
        &self,
        connection: &mut PgConnection,
        event: Uuid,
        landmark_id: &str,
    ) -> Result<(), PlaceNoteError>;
}
#[async_trait]
pub trait NoteFriendAuthority: Send + Sync {
    async fn available(&self, connection: &mut PgConnection) -> Result<(), PlaceNoteError>;
    async fn bilateral(
        &self,
        connection: &mut PgConnection,
        event: Uuid,
        author: Uuid,
        viewer: Uuid,
    ) -> Result<(), PlaceNoteError>;
}
#[async_trait]
pub trait NoteProfileAuthority: Send + Sync {
    async fn profile(
        &self,
        connection: &mut PgConnection,
        event: Uuid,
        user: Uuid,
    ) -> Result<NoteAuthor, PlaceNoteError>;
}
#[async_trait]
pub trait NoteMeetingAuthority: Send + Sync {
    async fn composer_entry(
        &self,
        connection: &mut PgConnection,
        event: Uuid,
        viewer: Uuid,
        author: Uuid,
        landmark_id: &str,
        landmark_revision: i64,
        access_until: DateTime<Utc>,
    ) -> Result<Option<NoteMeetingEntry>, PlaceNoteError>;
}
#[async_trait]
pub trait NoteModerationAuthority: Send + Sync {
    async fn authorize(
        &self,
        connection: &mut PgConnection,
        event: Uuid,
        moderator: Uuid,
    ) -> Result<(), PlaceNoteError>;
}

pub struct UnavailablePlaceAuthority;
#[async_trait]
impl NotePlaceAuthority for UnavailablePlaceAuthority {
    async fn review(
        &self,
        _: &mut PgConnection,
        _: Uuid,
        _: &LandmarkSelection,
    ) -> Result<(), PlaceNoteError> {
        Err(PlaceNoteError::DependencyUnavailable)
    }
    async fn current(&self, _: &mut PgConnection, _: Uuid, _: &str) -> Result<(), PlaceNoteError> {
        Err(PlaceNoteError::DependencyUnavailable)
    }
}
pub struct UnavailableFriendAuthority;
#[async_trait]
impl NoteFriendAuthority for UnavailableFriendAuthority {
    async fn available(&self, _: &mut PgConnection) -> Result<(), PlaceNoteError> {
        Err(PlaceNoteError::DependencyUnavailable)
    }
    async fn bilateral(
        &self,
        _: &mut PgConnection,
        _: Uuid,
        _: Uuid,
        _: Uuid,
    ) -> Result<(), PlaceNoteError> {
        Err(PlaceNoteError::DependencyUnavailable)
    }
}
pub struct UnavailableProfileAuthority;
#[async_trait]
impl NoteProfileAuthority for UnavailableProfileAuthority {
    async fn profile(
        &self,
        _: &mut PgConnection,
        _: Uuid,
        _: Uuid,
    ) -> Result<NoteAuthor, PlaceNoteError> {
        Err(PlaceNoteError::DependencyUnavailable)
    }
}
pub struct UnavailableMeetingAuthority;
#[async_trait]
impl NoteMeetingAuthority for UnavailableMeetingAuthority {
    async fn composer_entry(
        &self,
        _: &mut PgConnection,
        _: Uuid,
        _: Uuid,
        _: Uuid,
        _: &str,
        _: i64,
        _: DateTime<Utc>,
    ) -> Result<Option<NoteMeetingEntry>, PlaceNoteError> {
        Ok(None)
    }
}
pub struct UnavailableModerationAuthority;
#[async_trait]
impl NoteModerationAuthority for UnavailableModerationAuthority {
    async fn authorize(
        &self,
        _: &mut PgConnection,
        _: Uuid,
        _: Uuid,
    ) -> Result<(), PlaceNoteError> {
        Err(PlaceNoteError::DependencyUnavailable)
    }
}

pub struct PgPlaceNotes {
    pool: PgPool,
    place: Arc<dyn NotePlaceAuthority>,
    friend: Arc<dyn NoteFriendAuthority>,
    profile: Arc<dyn NoteProfileAuthority>,
    meeting: Arc<dyn NoteMeetingAuthority>,
    moderation: Arc<dyn NoteModerationAuthority>,
}
impl PgPlaceNotes {
    pub fn new(
        pool: PgPool,
        place: Arc<dyn NotePlaceAuthority>,
        friend: Arc<dyn NoteFriendAuthority>,
        profile: Arc<dyn NoteProfileAuthority>,
        meeting: Arc<dyn NoteMeetingAuthority>,
        moderation: Arc<dyn NoteModerationAuthority>,
    ) -> Self {
        Self {
            pool,
            place,
            friend,
            profile,
            meeting,
            moderation,
        }
    }
}

#[derive(Clone, Debug)]
struct StoredNote {
    id: Uuid,
    landmark_id: String,
    landmark_revision: i64,
    author_id: Uuid,
    parent_id: Option<Uuid>,
    audience: NoteAudience,
    expiry: NoteExpiry,
    body: String,
    revision: i64,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    withdrawn: bool,
    hidden: bool,
}
const COLUMNS: &str = "note_id,landmark_id,landmark_revision,author_id,parent_id,audience,\
     expiry_kind,body,revision,created_at,expires_at,withdrawn_at,hidden_at";
fn stored(row: &sqlx::postgres::PgRow) -> Result<StoredNote, PlaceNoteError> {
    let audience: String = row
        .try_get("audience")
        .map_err(|_| PlaceNoteError::InvalidState)?;
    let expiry: String = row
        .try_get("expiry_kind")
        .map_err(|_| PlaceNoteError::InvalidState)?;
    let audience = audience.parse().map_err(|_| PlaceNoteError::InvalidState)?;
    let expiry = expiry.parse().map_err(|_| PlaceNoteError::InvalidState)?;
    Ok(StoredNote {
        id: row
            .try_get("note_id")
            .map_err(|_| PlaceNoteError::InvalidState)?,
        landmark_id: row
            .try_get("landmark_id")
            .map_err(|_| PlaceNoteError::InvalidState)?,
        landmark_revision: row
            .try_get("landmark_revision")
            .map_err(|_| PlaceNoteError::InvalidState)?,
        author_id: row
            .try_get("author_id")
            .map_err(|_| PlaceNoteError::InvalidState)?,
        parent_id: row
            .try_get("parent_id")
            .map_err(|_| PlaceNoteError::InvalidState)?,
        audience,
        expiry,
        body: row
            .try_get("body")
            .map_err(|_| PlaceNoteError::InvalidState)?,
        revision: row
            .try_get("revision")
            .map_err(|_| PlaceNoteError::InvalidState)?,
        created_at: row
            .try_get("created_at")
            .map_err(|_| PlaceNoteError::InvalidState)?,
        expires_at: row
            .try_get("expires_at")
            .map_err(|_| PlaceNoteError::InvalidState)?,
        withdrawn: row
            .try_get::<Option<DateTime<Utc>>, _>("withdrawn_at")
            .map_err(|_| PlaceNoteError::InvalidState)?
            .is_some(),
        hidden: row
            .try_get::<Option<DateTime<Utc>>, _>("hidden_at")
            .map_err(|_| PlaceNoteError::InvalidState)?
            .is_some(),
    })
}

fn db(error: sqlx::Error) -> PlaceNoteError {
    match error {
        sqlx::Error::RowNotFound => PlaceNoteError::NotFound,
        _ => PlaceNoteError::DependencyUnavailable,
    }
}
fn cursor(id: Uuid, created_at: DateTime<Utc>) -> String {
    NoteCursor { created_at, id }.encode()
}

impl PgPlaceNotes {
    async fn event_guard(
        &self,
        conn: &mut PgConnection,
        event: Uuid,
        actor: Uuid,
        write: bool,
    ) -> Result<(), PlaceNoteError> {
        let phase: Option<String> =
            sqlx::query_scalar("select phase from concert_events where event_id=$1 for share")
                .bind(event)
                .fetch_optional(&mut *conn)
                .await
                .map_err(db)?;
        match phase.as_deref() {
            Some("OPEN" | "LIVE") => {}
            Some(_) if write => return Err(PlaceNoteError::EventReadOnly),
            Some(_) => return Err(PlaceNoteError::Denied),
            None => return Err(PlaceNoteError::NotFound),
        }
        let member: Option<i32> = sqlx::query_scalar(
            "select 1 from concert_participants where event_id=$1 and user_id=$2",
        )
        .bind(event)
        .bind(actor)
        .fetch_optional(&mut *conn)
        .await
        .map_err(db)?;
        member.ok_or(PlaceNoteError::Denied)?;
        Ok(())
    }

    async fn now(&self, conn: &mut PgConnection) -> Result<DateTime<Utc>, PlaceNoteError> {
        sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *conn)
            .await
            .map_err(db)
    }

    async fn end_of_day(
        &self,
        conn: &mut PgConnection,
        event: Uuid,
        now: DateTime<Utc>,
    ) -> Result<DateTime<Utc>, PlaceNoteError> {
        let end: Option<DateTime<Utc>> = sqlx::query_scalar(
            "select ((date_trunc('day', $2 at time zone e.timezone) + interval '1 day') at time zone e.timezone)
             from concert_events e where e.event_id=$1",
        )
        .bind(event)
        .bind(now)
        .fetch_optional(&mut *conn)
        .await
        .map_err(db)?;
        let end = end.ok_or(PlaceNoteError::DependencyUnavailable)?;
        let cutoff: Option<DateTime<Utc>> = sqlx::query_scalar(
            "select event_cutoff from concert_meeting_settings where event_id=$1",
        )
        .bind(event)
        .fetch_optional(&mut *conn)
        .await
        .map_err(db)?;
        let cutoff = cutoff.ok_or(PlaceNoteError::DependencyUnavailable)?;
        let end = end.min(cutoff);
        if end <= now {
            return Err(PlaceNoteError::EventReadOnly);
        }
        Ok(end)
    }

    async fn audience_ok(
        &self,
        conn: &mut PgConnection,
        event: Uuid,
        viewer: Uuid,
        note: &StoredNote,
    ) -> Result<bool, PlaceNoteError> {
        if note.audience == NoteAudience::Everyone || note.author_id == viewer {
            return Ok(true);
        }
        match self
            .friend
            .bilateral(&mut *conn, event, note.author_id, viewer)
            .await
        {
            Ok(()) => Ok(true),
            Err(PlaceNoteError::Denied | PlaceNoteError::NotFound) => Ok(false),
            Err(error) => Err(error),
        }
    }

    async fn load(&self, conn: &mut PgConnection, id: Uuid) -> Result<StoredNote, PlaceNoteError> {
        let row = sqlx::query(&format!(
            "select {COLUMNS} from concert_place_notes where note_id=$1"
        ))
        .bind(id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(db)?
        .ok_or(PlaceNoteError::NotFound)?;
        stored(&row)
    }

    async fn reply_rows(
        &self,
        conn: &mut PgConnection,
        parent: Uuid,
    ) -> Result<Vec<StoredNote>, PlaceNoteError> {
        let rows = sqlx::query(&format!(
            "select {COLUMNS} from concert_place_notes where parent_id=$1
             order by created_at desc, note_id desc limit 1000"
        ))
        .bind(parent)
        .fetch_all(&mut *conn)
        .await
        .map_err(db)?;
        rows.iter().map(stored).collect()
    }

    async fn visible_replies(
        &self,
        conn: &mut PgConnection,
        event: Uuid,
        viewer: Uuid,
        now: DateTime<Utc>,
        parent: &StoredNote,
    ) -> Result<Vec<StoredNote>, PlaceNoteError> {
        if parent.withdrawn || parent.hidden || parent.expires_at <= now {
            return Ok(Vec::new());
        }
        if !self.audience_ok(&mut *conn, event, viewer, parent).await? {
            return Ok(Vec::new());
        }
        let mut visible = Vec::new();
        for reply in self.reply_rows(&mut *conn, parent.id).await? {
            if reply.withdrawn || reply.hidden || reply.expires_at <= now {
                continue;
            }
            if self.audience_ok(&mut *conn, event, viewer, &reply).await? {
                visible.push(reply);
            }
        }
        Ok(visible)
    }

    async fn project(
        &self,
        conn: &mut PgConnection,
        event: Uuid,
        viewer: Uuid,
        now: DateTime<Utc>,
        note: StoredNote,
        meeting: bool,
    ) -> Result<Option<PlaceNote>, PlaceNoteError> {
        if note.withdrawn || note.hidden || note.expires_at <= now {
            return Ok(None);
        }
        if !self.audience_ok(&mut *conn, event, viewer, &note).await? {
            return Ok(None);
        }
        let author = self
            .profile
            .profile(&mut *conn, event, note.author_id)
            .await?;
        let reply_count = if note.parent_id.is_none() {
            self.visible_replies(&mut *conn, event, viewer, now, &note)
                .await?
                .len()
        } else {
            0
        };
        let meet_here = if meeting && note.parent_id.is_none() && viewer != note.author_id {
            self.meeting
                .composer_entry(
                    &mut *conn,
                    event,
                    viewer,
                    note.author_id,
                    &note.landmark_id,
                    note.landmark_revision,
                    note.expires_at,
                )
                .await
                .ok()
                .flatten()
        } else {
            None
        };
        Ok(Some(PlaceNote {
            id: note.id,
            landmark_id: note.landmark_id,
            landmark_revision: note.landmark_revision,
            author,
            text: note.body,
            created_at: note.created_at,
            expires_at: note.expires_at,
            revision: note.revision,
            audience: note.audience,
            expiry: note.expiry,
            parent_id: note.parent_id,
            reply_count,
            can_reply: note.parent_id.is_none() && viewer != note.author_id,
            can_withdraw: note.author_id == viewer,
            meet_here,
        }))
    }

    fn access_until(now: DateTime<Utc>, items: &[PlaceNote]) -> DateTime<Utc> {
        items
            .iter()
            .flat_map(|item| {
                std::iter::once(item.expires_at)
                    .chain(item.meet_here.as_ref().map(|m| m.access_until))
            })
            .min()
            .unwrap_or(now)
    }

    async fn project_page(
        &self,
        conn: &mut PgConnection,
        event: Uuid,
        viewer: Uuid,
        now: DateTime<Utc>,
        notes: Vec<StoredNote>,
        limit: usize,
        before: Option<NoteCursor>,
    ) -> Result<NotePage, PlaceNoteError> {
        let mut visible = Vec::new();
        for note in notes {
            if let Some(view) = self
                .project(&mut *conn, event, viewer, now, note.clone(), true)
                .await?
            {
                visible.push(view);
            }
        }
        let count = visible.len();
        let mut visible = visible
            .into_iter()
            .filter(|view| {
                before.is_none_or(|cursor| {
                    NoteCursor {
                        created_at: view.created_at,
                        id: view.id,
                    }
                    .is_before(cursor)
                })
            })
            .collect::<Vec<_>>();
        let has_more = visible.len() > limit;
        visible.truncate(limit);
        let next_before = has_more
            .then(|| visible.last().map(|n| cursor(n.id, n.created_at)))
            .flatten();
        let access_until = Self::access_until(now, &visible);
        Ok(NotePage {
            items: visible,
            visible_count: count,
            parent: None,
            next_before,
            server_now: now,
            access_until,
        })
    }
}

#[async_trait]
impl PlaceNoteUseCases for PgPlaceNotes {
    async fn create_note(
        &self,
        actor: Uuid,
        event: Uuid,
        command: CreatePlaceNote,
        idempotency_key: Uuid,
    ) -> Result<PlaceNote, PlaceNoteError> {
        let _ = idempotency_key;
        let mut tx = self.pool.begin().await.map_err(db)?;
        self.event_guard(&mut tx, event, actor, true).await?;
        self.place
            .review(&mut tx, event, &command.landmark_selection)
            .await?;
        if command.audience == NoteAudience::Friends {
            self.friend.available(&mut tx).await?;
        }
        let now = self.now(&mut tx).await?;
        let expires_at = match command.expiry {
            NoteExpiry::OneHour => now + Duration::seconds(ONE_HOUR_SECONDS),
            NoteExpiry::VenueEndDay => self.end_of_day(&mut tx, event, now).await?,
        };
        let id: Uuid = sqlx::query_scalar(
            "insert into concert_place_notes
             (event_id,landmark_id,catalog_revision,landmark_revision,author_id,audience,
              expiry_kind,body,revision,created_at,expires_at)
             values($1,$2,$3,$4,$5,$6,$7,$8,1,$9,$10) returning note_id",
        )
        .bind(event)
        .bind(&command.landmark_selection.landmark_id)
        .bind(command.landmark_selection.catalog_revision)
        .bind(command.landmark_selection.landmark_revision)
        .bind(actor)
        .bind(command.audience.as_str())
        .bind(command.expiry.as_str())
        .bind(command.text.as_str())
        .bind(now)
        .bind(expires_at)
        .fetch_one(&mut *tx)
        .await
        .map_err(db)?;
        let note = self.load(&mut tx, id).await?;
        let view = self
            .project(&mut tx, event, actor, now, note, false)
            .await?
            .ok_or(PlaceNoteError::InvalidState)?;
        tx.commit().await.map_err(db)?;
        Ok(view)
    }

    async fn list_notes(
        &self,
        viewer: Uuid,
        event: Uuid,
        query: NoteListQuery,
    ) -> Result<NotePage, PlaceNoteError> {
        if !valid_landmark_id(&query.landmark_id) {
            return Err(PlaceNoteError::InvalidInput);
        }
        let mut tx = self.pool.begin().await.map_err(db)?;
        self.event_guard(&mut tx, event, viewer, false).await?;
        self.place
            .current(&mut tx, event, &query.landmark_id)
            .await?;
        let now = self.now(&mut tx).await?;
        let rows = sqlx::query(&format!(
            "select {COLUMNS} from concert_place_notes
             where event_id=$1 and landmark_id=$2 and parent_id is null
             order by created_at desc, note_id desc limit 1000"
        ))
        .bind(event)
        .bind(&query.landmark_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(db)?;
        let notes = rows.iter().map(stored).collect::<Result<Vec<_>, _>>()?;
        let page = self
            .project_page(
                &mut tx,
                event,
                viewer,
                now,
                notes,
                query.limit,
                query.before,
            )
            .await?;
        tx.commit().await.map_err(db)?;
        Ok(page)
    }

    async fn note_thread(
        &self,
        viewer: Uuid,
        event: Uuid,
        landmark_id: &str,
        note_id: Uuid,
        limit: Option<usize>,
        before: Option<&str>,
    ) -> Result<NoteThread, PlaceNoteError> {
        let query = NoteListQuery::new(landmark_id, limit, before)?;
        let mut tx = self.pool.begin().await.map_err(db)?;
        self.event_guard(&mut tx, event, viewer, false).await?;
        self.place.current(&mut tx, event, landmark_id).await?;
        let now = self.now(&mut tx).await?;
        let root = self.load(&mut tx, note_id).await?;
        if root.parent_id.is_some() || root.landmark_id != landmark_id {
            return Err(PlaceNoteError::NotFound);
        }
        let parent = self
            .project(&mut tx, event, viewer, now, root.clone(), true)
            .await?
            .ok_or(PlaceNoteError::NotFound)?;
        let mut replies = Vec::new();
        for reply in self
            .visible_replies(&mut tx, event, viewer, now, &root)
            .await?
        {
            if query.before.is_none_or(|cursor| {
                NoteCursor {
                    created_at: reply.created_at,
                    id: reply.id,
                }
                .is_before(cursor)
            }) {
                if let Some(view) = self
                    .project(&mut tx, event, viewer, now, reply, true)
                    .await?
                {
                    replies.push(view);
                }
            }
        }
        let has_more = replies.len() > query.limit;
        replies.truncate(query.limit);
        let next_before = has_more
            .then(|| replies.last().map(|r| cursor(r.id, r.created_at)))
            .flatten();
        let access_until = Self::access_until(now, &replies).min(parent.expires_at);
        tx.commit().await.map_err(db)?;
        Ok(NoteThread {
            note: parent,
            replies,
            next_before,
            server_now: now,
            access_until,
        })
    }

    async fn list_replies(
        &self,
        viewer: Uuid,
        event: Uuid,
        landmark_id: &str,
        note_id: Uuid,
        mut query: NoteListQuery,
    ) -> Result<NotePage, PlaceNoteError> {
        query.landmark_id = landmark_id.to_owned();
        let mut tx = self.pool.begin().await.map_err(db)?;
        self.event_guard(&mut tx, event, viewer, false).await?;
        self.place.current(&mut tx, event, landmark_id).await?;
        let now = self.now(&mut tx).await?;
        let root = self.load(&mut tx, note_id).await?;
        if root.parent_id.is_some() || root.landmark_id != landmark_id {
            return Err(PlaceNoteError::NotFound);
        }
        let parent = self
            .project(&mut tx, event, viewer, now, root.clone(), true)
            .await?
            .ok_or(PlaceNoteError::NotFound)?;
        let replies = self
            .visible_replies(&mut tx, event, viewer, now, &root)
            .await?;
        let mut page = self
            .project_page(
                &mut tx,
                event,
                viewer,
                now,
                replies,
                query.limit,
                query.before,
            )
            .await?;
        page.parent = Some(parent);
        tx.commit().await.map_err(db)?;
        Ok(page)
    }

    async fn create_reply(
        &self,
        actor: Uuid,
        event: Uuid,
        landmark_id: &str,
        parent_id: Uuid,
        command: CreatePlaceReply,
        idempotency_key: Uuid,
    ) -> Result<PlaceNote, PlaceNoteError> {
        let _ = idempotency_key;
        let mut tx = self.pool.begin().await.map_err(db)?;
        self.event_guard(&mut tx, event, actor, true).await?;
        self.place.current(&mut tx, event, landmark_id).await?;
        let parent = sqlx::query(&format!(
            "select {COLUMNS} from concert_place_notes
             where note_id=$1 and event_id=$2 and landmark_id=$3 for share"
        ))
        .bind(parent_id)
        .bind(event)
        .bind(landmark_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?
        .ok_or(PlaceNoteError::NotFound)?;
        let parent = stored(&parent)?;
        let now = self.now(&mut tx).await?;
        if parent.parent_id.is_some()
            || parent.withdrawn
            || parent.hidden
            || parent.expires_at <= now
        {
            return Err(PlaceNoteError::NotFound);
        }
        if parent.revision != command.expected_parent_revision {
            return Err(PlaceNoteError::Conflict);
        }
        if parent.audience == NoteAudience::Friends {
            self.friend.available(&mut tx).await?;
            self.friend
                .bilateral(&mut tx, event, parent.author_id, actor)
                .await?;
        }
        let id: Uuid = sqlx::query_scalar(
            "insert into concert_place_notes
             (event_id,landmark_id,catalog_revision,landmark_revision,author_id,parent_id,
              audience,expiry_kind,body,revision,created_at,expires_at)
             select event_id,landmark_id,catalog_revision,landmark_revision,$3,note_id,
                    audience,expiry_kind,$4,1,$5,expires_at
             from concert_place_notes where note_id=$1 and event_id=$2
             returning note_id",
        )
        .bind(parent_id)
        .bind(event)
        .bind(actor)
        .bind(command.text.as_str())
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(db)?;
        sqlx::query("update concert_place_notes set revision=revision+1 where note_id=$1")
            .bind(parent_id)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        let reply = self.load(&mut tx, id).await?;
        let view = self
            .project(&mut tx, event, actor, now, reply, true)
            .await?
            .ok_or(PlaceNoteError::InvalidState)?;
        tx.commit().await.map_err(db)?;
        Ok(view)
    }

    async fn withdraw_note(
        &self,
        actor: Uuid,
        event: Uuid,
        landmark_id: &str,
        note_id: Uuid,
        expected_revision: i64,
        idempotency_key: Uuid,
    ) -> Result<NoteMutation, PlaceNoteError> {
        let _ = idempotency_key;
        let mut tx = self.pool.begin().await.map_err(db)?;
        self.event_guard(&mut tx, event, actor, true).await?;
        let note = sqlx::query(&format!(
            "select {COLUMNS} from concert_place_notes
             where note_id=$1 and event_id=$2 and landmark_id=$3 for update"
        ))
        .bind(note_id)
        .bind(event)
        .bind(landmark_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?
        .ok_or(PlaceNoteError::NotFound)?;
        let note = stored(&note)?;
        if note.author_id != actor {
            return Err(PlaceNoteError::Denied);
        }
        if note.withdrawn {
            return Ok(NoteMutation {
                resource_id: note_id,
                revision: note.revision,
            });
        }
        if note.revision != expected_revision {
            return Err(PlaceNoteError::Conflict);
        }
        let now = self.now(&mut tx).await?;
        let revision: i64 = sqlx::query_scalar(
            "update concert_place_notes
             set withdrawn_at=$2,body='',revision=revision+1
             where note_id=$1 and withdrawn_at is null returning revision",
        )
        .bind(note_id)
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(db)?;
        if note.parent_id.is_none() {
            sqlx::query(
                "update concert_place_notes
                 set withdrawn_at=$2,body='',revision=revision+1
                 where parent_id=$1 and withdrawn_at is null",
            )
            .bind(note_id)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        }
        tx.commit().await.map_err(db)?;
        Ok(NoteMutation {
            resource_id: note_id,
            revision,
        })
    }

    async fn hide_note(
        &self,
        moderator: Uuid,
        event: Uuid,
        note_id: Uuid,
    ) -> Result<(), PlaceNoteError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        self.event_guard(&mut tx, event, moderator, true).await?;
        self.moderation.authorize(&mut tx, event, moderator).await?;
        let note = sqlx::query(&format!(
            "select {COLUMNS} from concert_place_notes where note_id=$1 and event_id=$2 for update"
        ))
        .bind(note_id)
        .bind(event)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?
        .ok_or(PlaceNoteError::NotFound)?;
        let note = stored(&note)?;
        if note.hidden {
            tx.commit().await.map_err(db)?;
            return Ok(());
        }
        let now = self.now(&mut tx).await?;
        sqlx::query(
            "update concert_place_notes
             set hidden_at=$2,hidden_by=$3,body='',revision=revision+1
             where note_id=$1 and hidden_at is null",
        )
        .bind(note_id)
        .bind(now)
        .bind(moderator)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
        sqlx::query(
            "update concert_place_notes
             set hidden_at=$2,hidden_by=$3,body='',revision=revision+1
             where parent_id=$1 and hidden_at is null",
        )
        .bind(note_id)
        .bind(now)
        .bind(moderator)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
        tx.commit().await.map_err(db)?;
        Ok(())
    }
}
