use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::venue_notes::{
    CreatePlaceNote, CreatePlaceReply, NoteListQuery, PlaceNoteUseCases,
};
use concert_domain::venue_notes::{
    LandmarkSelection, NoteAudience, NoteExpiry, NoteMeetingEntry, PlaceNoteError,
};
use concert_service::venue_notes::{
    NoteFriendAuthority, NoteMeetingAuthority, NoteModerationAuthority, NotePlaceAuthority,
    NoteProfileAuthority, PgPlaceNotes, UnavailableFriendAuthority,
};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgConnection, PgPool};
use std::{path::Path, sync::Arc};
use uuid::Uuid;

struct Place;
#[async_trait]
impl NotePlaceAuthority for Place {
    async fn review(
        &self,
        _: &mut PgConnection,
        _event: Uuid,
        selection: &LandmarkSelection,
    ) -> Result<(), PlaceNoteError> {
        if selection.landmark_id == "lm-main"
            && selection.catalog_revision == 1
            && selection.landmark_revision == 1
        {
            Ok(())
        } else {
            Err(PlaceNoteError::NotFound)
        }
    }
    async fn current(
        &self,
        _: &mut PgConnection,
        _: Uuid,
        landmark_id: &str,
    ) -> Result<(), PlaceNoteError> {
        (landmark_id == "lm-main")
            .then_some(())
            .ok_or(PlaceNoteError::NotFound)
    }
}

struct Profile;
#[async_trait]
impl NoteProfileAuthority for Profile {
    async fn profile(
        &self,
        connection: &mut PgConnection,
        _: Uuid,
        user: Uuid,
    ) -> Result<concert_domain::venue_notes::NoteAuthor, PlaceNoteError> {
        let name: String = sqlx::query_scalar(
            "select display_name from place_note_test_profiles where user_id=$1",
        )
        .bind(user)
        .fetch_optional(&mut *connection)
        .await
        .map_err(|_| PlaceNoteError::DependencyUnavailable)?
        .ok_or(PlaceNoteError::NotFound)?;
        concert_domain::venue_notes::NoteAuthor::new(
            user,
            &name,
            None,
            json!({"revision":1,"config":{"kind":"avatar_config"}}),
        )
    }
}

struct Friends;
#[async_trait]
impl NoteFriendAuthority for Friends {
    async fn available(&self, _: &mut PgConnection) -> Result<(), PlaceNoteError> {
        Ok(())
    }
    async fn bilateral(
        &self,
        connection: &mut PgConnection,
        event: Uuid,
        author: Uuid,
        viewer: Uuid,
    ) -> Result<(), PlaceNoteError> {
        let exists: bool = sqlx::query_scalar(
            "select exists(select 1 from place_note_test_friends
             where event_id=$1 and author_id=$2 and viewer_id=$3)",
        )
        .bind(event)
        .bind(author)
        .bind(viewer)
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| PlaceNoteError::DependencyUnavailable)?;
        if exists {
            Ok(())
        } else {
            Err(PlaceNoteError::Denied)
        }
    }
}

struct Meeting {
    recipient: Uuid,
}
#[async_trait]
impl NoteMeetingAuthority for Meeting {
    async fn composer_entry(
        &self,
        _: &mut PgConnection,
        _: Uuid,
        viewer: Uuid,
        author: Uuid,
        landmark_id: &str,
        landmark_revision: i64,
        access_until: DateTime<Utc>,
    ) -> Result<Option<NoteMeetingEntry>, PlaceNoteError> {
        if author == self.recipient && viewer != author {
            return Ok(Some(NoteMeetingEntry {
                landmark_id: landmark_id.to_owned(),
                recipient_user_id: author,
                landmark_revision,
                access_until,
            }));
        }
        Ok(None)
    }
}

struct Moderation;
#[async_trait]
impl NoteModerationAuthority for Moderation {
    async fn authorize(
        &self,
        connection: &mut PgConnection,
        _: Uuid,
        moderator: Uuid,
    ) -> Result<(), PlaceNoteError> {
        let allowed: bool = sqlx::query_scalar(
            "select exists(select 1 from place_note_test_moderators where user_id=$1)",
        )
        .bind(moderator)
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| PlaceNoteError::DependencyUnavailable)?;
        allowed.then_some(()).ok_or(PlaceNoteError::Denied)
    }
}

async fn pool() -> anyhow::Result<PgPool> {
    let url = std::env::var("PLACE_NOTES_TEST_DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("PLACE_NOTES_TEST_DATABASE_URL is required"))?;
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect(&url)
        .await?;
    let migrations =
        sqlx::migrate::Migrator::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations"))
            .await?;
    migrations.run(&pool).await?;
    Ok(pool)
}

fn service(
    pool: PgPool,
    friend: Arc<dyn NoteFriendAuthority>,
    meeting: Arc<dyn NoteMeetingAuthority>,
    moderation: Arc<dyn NoteModerationAuthority>,
) -> PgPlaceNotes {
    PgPlaceNotes::new(
        pool,
        Arc::new(Place),
        friend,
        Arc::new(Profile),
        meeting,
        moderation,
    )
}

fn note(audience: NoteAudience) -> CreatePlaceNote {
    CreatePlaceNote::new(
        "Meet at the north gate",
        audience,
        NoteExpiry::OneHour,
        LandmarkSelection::new("lm-main", 1, 1).unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn place_notes_audience_expiry_reply_moderation_and_meeting() -> anyhow::Result<()> {
    let pool = pool().await?;
    let event = Uuid::new_v4();
    let author = Uuid::new_v4();
    let stranger = Uuid::new_v4();
    let friend = Uuid::new_v4();
    let moderator = Uuid::new_v4();
    sqlx::query("truncate concert_place_notes cascade")
        .execute(&pool)
        .await?;
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,$3,'OPEN')")
        .bind(event)
        .bind(format!("notes-{}", event.simple()))
        .bind("Place notes test event")
        .execute(&pool)
        .await?;
    for user in [author, stranger, friend, moderator] {
        sqlx::query(
            "insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled)
             values($1,$2,'PARADE',false,false,true)",
        )
        .bind(event)
        .bind(user)
        .execute(&pool)
        .await?;
    }
    sqlx::query("insert into concert_meeting_settings(event_id,event_cutoff) values($1,$2)")
        .bind(event)
        .bind(Utc::now() + Duration::days(1))
        .execute(&pool)
        .await?;
    sqlx::query("create table if not exists place_note_test_profiles(user_id uuid primary key, display_name text not null)")
        .execute(&pool)
        .await?;
    sqlx::query("create table if not exists place_note_test_friends(event_id uuid, author_id uuid, viewer_id uuid)")
        .execute(&pool)
        .await?;
    sqlx::query("create table if not exists place_note_test_moderators(user_id uuid primary key)")
        .execute(&pool)
        .await?;
    sqlx::query(
        "truncate place_note_test_profiles,place_note_test_friends,place_note_test_moderators",
    )
    .execute(&pool)
    .await?;
    for (user, name) in [
        (author, "Author"),
        (stranger, "Stranger"),
        (friend, "Friend"),
        (moderator, "Moderator"),
    ] {
        sqlx::query("insert into place_note_test_profiles(user_id,display_name) values($1,$2)")
            .bind(user)
            .bind(name)
            .execute(&pool)
            .await?;
    }
    sqlx::query(
        "insert into place_note_test_friends(event_id,author_id,viewer_id) values($1,$2,$3)",
    )
    .bind(event)
    .bind(author)
    .bind(friend)
    .execute(&pool)
    .await?;
    sqlx::query("insert into place_note_test_moderators(user_id) values($1)")
        .bind(moderator)
        .execute(&pool)
        .await?;

    let notes = service(
        pool.clone(),
        Arc::new(Friends),
        Arc::new(Meeting { recipient: author }),
        Arc::new(Moderation),
    );

    let everyone = notes
        .create_note(author, event, note(NoteAudience::Everyone), Uuid::new_v4())
        .await?;
    assert_eq!(everyone.audience, NoteAudience::Everyone);
    assert_eq!(
        everyone.expires_at - everyone.created_at,
        Duration::hours(1)
    );
    let endday = notes
        .create_note(
            author,
            event,
            CreatePlaceNote::new(
                "Until venue close",
                NoteAudience::Everyone,
                NoteExpiry::VenueEndDay,
                LandmarkSelection::new("lm-main", 1, 1).unwrap(),
            )
            .unwrap(),
            Uuid::new_v4(),
        )
        .await?;
    let expected_end: DateTime<Utc> = sqlx::query_scalar(
        "select ((date_trunc('day', clock_timestamp() at time zone 'Asia/Seoul') + interval '1 day') at time zone 'Asia/Seoul')",
    )
    .fetch_one(&pool)
    .await?;
    let cutoff: DateTime<Utc> =
        sqlx::query_scalar("select event_cutoff from concert_meeting_settings where event_id=$1")
            .bind(event)
            .fetch_one(&pool)
            .await?;
    assert_eq!(endday.expires_at, expected_end.min(cutoff));
    notes
        .withdraw_note(
            author,
            event,
            "lm-main",
            endday.id,
            endday.revision,
            Uuid::new_v4(),
        )
        .await?;
    let page = notes
        .list_notes(stranger, event, NoteListQuery::new("lm-main", None, None)?)
        .await?;
    assert_eq!(page.visible_count, 1);
    assert_eq!(page.items[0].text, "Meet at the north gate");
    assert_eq!(page.items[0].author.display_name, "Author");

    let meeting = page.items[0].meet_here.as_ref().expect("meeting entry");
    assert_eq!(meeting.recipient_user_id, author);
    assert_eq!(meeting.landmark_id, "lm-main");
    let invitations: i64 =
        sqlx::query_scalar("select count(*) from concert_place_notes where parent_id is not null")
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        invitations, 0,
        "Meet Here must not write an invitation or reply"
    );

    let reply = notes
        .create_reply(
            stranger,
            event,
            "lm-main",
            everyone.id,
            CreatePlaceReply::new("I am coming", everyone.revision)?,
            Uuid::new_v4(),
        )
        .await?;
    assert_eq!(reply.parent_id, Some(everyone.id));
    let replies = notes
        .list_replies(
            author,
            event,
            "lm-main",
            everyone.id,
            NoteListQuery::new("lm-main", None, None)?,
        )
        .await?;
    assert_eq!(replies.items.len(), 1);
    assert_eq!(replies.items[0].text, "I am coming");
    assert_eq!(replies.parent.as_ref().unwrap().reply_count, 1);

    let friends_row = Uuid::new_v4();
    sqlx::query(
        "insert into concert_place_notes
         (note_id,event_id,landmark_id,catalog_revision,landmark_revision,author_id,audience,
          expiry_kind,body,revision,created_at,expires_at)
         values($1,$2,'lm-main',1,1,$3,'FRIENDS','ONE_HOUR',$4,1,clock_timestamp(),clock_timestamp()+interval '1 hour')",
    )
    .bind(friends_row)
    .bind(event)
    .bind(author)
    .bind("Friends only note")
    .execute(&pool)
    .await?;
    let fail_closed = service(
        pool.clone(),
        Arc::new(UnavailableFriendAuthority),
        Arc::new(Meeting { recipient: author }),
        Arc::new(Moderation),
    );
    assert_eq!(
        fail_closed
            .list_notes(stranger, event, NoteListQuery::new("lm-main", None, None)?)
            .await,
        Err(PlaceNoteError::DependencyUnavailable)
    );
    assert_eq!(
        fail_closed
            .note_thread(stranger, event, "lm-main", friends_row, None, None)
            .await,
        Err(PlaceNoteError::DependencyUnavailable)
    );
    assert_eq!(
        fail_closed
            .create_note(author, event, note(NoteAudience::Friends), Uuid::new_v4())
            .await,
        Err(PlaceNoteError::DependencyUnavailable)
    );
    let friend_page = notes
        .list_notes(friend, event, NoteListQuery::new("lm-main", None, None)?)
        .await?;
    assert_eq!(friend_page.visible_count, 2);
    assert!(friend_page
        .items
        .iter()
        .any(|n| n.text == "Friends only note"));
    let stranger_page = notes
        .list_notes(stranger, event, NoteListQuery::new("lm-main", None, None)?)
        .await?;
    assert_eq!(stranger_page.visible_count, 1);
    assert!(!stranger_page
        .items
        .iter()
        .any(|n| n.text == "Friends only note"));

    let _expired = Uuid::new_v4();
    sqlx::query(
        "insert into concert_place_notes
         (note_id,event_id,landmark_id,catalog_revision,landmark_revision,author_id,audience,
          expiry_kind,body,revision,created_at,expires_at)
         values($1,$2,'lm-main',1,1,$3,'EVERYONE','ONE_HOUR',$4,1,clock_timestamp()-interval '2 hours',clock_timestamp()-interval '1 hour')",
    )
    .bind(_expired)
    .bind(event)
    .bind(author)
    .bind("Expired note")
    .execute(&pool)
    .await?;
    let after_expiry = notes
        .list_notes(stranger, event, NoteListQuery::new("lm-main", None, None)?)
        .await?;
    assert_eq!(after_expiry.visible_count, 1);
    assert!(!after_expiry.items.iter().any(|n| n.text == "Expired note"));

    notes.hide_note(moderator, event, everyone.id).await?;
    let hidden = notes
        .list_notes(stranger, event, NoteListQuery::new("lm-main", None, None)?)
        .await?;
    assert!(hidden.items.iter().all(|n| n.id != everyone.id));
    assert_eq!(
        notes
            .list_replies(
                author,
                event,
                "lm-main",
                everyone.id,
                NoteListQuery::new("lm-main", None, None)?
            )
            .await,
        Err(PlaceNoteError::NotFound)
    );
    let purged: String =
        sqlx::query_scalar("select body from concert_place_notes where note_id=$1")
            .bind(everyone.id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(purged, "");

    let mut tx = pool.begin().await?;
    sqlx::query("drop table place_note_test_profiles")
        .execute(&mut *tx)
        .await?;
    sqlx::query("drop table place_note_test_friends")
        .execute(&mut *tx)
        .await?;
    sqlx::query("drop table place_note_test_moderators")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
