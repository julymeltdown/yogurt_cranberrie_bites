//! Venue authorization and privacy matrix on real PostgreSQL with synthetic
//! external-owner evidence; production keeps the same ports fail-closed.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::venue_location::{VenueLocation, VenueLocationPolicyProvider};
use concert_application::venue_meeting::{
    CreateInvitation, DecideInvitation, LandmarkReview, PairFacts, SendMessage,
    VenueMeetingAuthority,
};
use concert_application::venue_notes::{
    CreatePlaceNote, CreatePlaceReply, NoteListQuery, PlaceNoteUseCases,
};
use concert_domain::venue_location::{
    CommandReceipt, LocationSnapshot, PhoneFixInput, SharingCommand, SharingState,
    VenueLocationError, VenueLocationPolicy, VisibleLocation,
};
use concert_domain::venue_meeting::{
    Decision, InvitationState, LandmarkSelection, MeetingError, MeetingPolicy, MeetingSelection,
};
use concert_domain::venue_notes::{
    LandmarkSelection as NoteLandmarkSelection, NoteAudience, NoteAuthor, NoteExpiry,
    NoteMeetingEntry, PlaceNoteError,
};
use concert_domain::{AuthUserId, EventId, ParticipantKey};
use concert_service::venue_location::{PgVenueLocation, VenueLocationAuthority};
use concert_service::venue_meeting::{Direction, PgVenueMeeting};
use concert_service::venue_notes::{
    NoteFriendAuthority, NoteMeetingAuthority, NoteModerationAuthority, NotePlaceAuthority,
    NoteProfileAuthority, PgPlaceNotes,
};
use serde_json::{json, Value};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgConnection, PgPool,
};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use tonic::Code;
use uuid::Uuid;

const LANDMARK: &str = "gate-a";
const CATALOG_REVISION: i64 = 3;
const LANDMARK_REVISION: i64 = 4;
const LATITUDE: f64 = 37.5665;
const LONGITUDE: f64 = 126.9780;
const PRIVATE_NOTE: &str = "friends only secret coordinates 37.5665";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Ok,
    BadRequest,
    Forbidden,
    NotFound,
    Conflict,
    Unavailable,
}

impl Status {
    fn code(self) -> u16 {
        match self {
            Self::Ok => 200,
            Self::BadRequest => 400,
            Self::Forbidden => 403,
            Self::NotFound => 404,
            Self::Conflict => 409,
            Self::Unavailable => 503,
        }
    }
}

fn location_status(error: &VenueLocationError) -> Status {
    match error {
        VenueLocationError::InvalidInput => Status::BadRequest,
        VenueLocationError::Denied | VenueLocationError::NotFound => Status::NotFound,
        VenueLocationError::Conflict => Status::Conflict,
        VenueLocationError::PolicyUnavailable | VenueLocationError::DependencyUnavailable => {
            Status::Unavailable
        }
    }
}

fn meeting_status(error: &MeetingError) -> Status {
    match error {
        MeetingError::InvalidInput => Status::BadRequest,
        MeetingError::ResourceUnavailable => Status::NotFound,
        MeetingError::RevisionConflict | MeetingError::InvalidState => Status::Conflict,
        MeetingError::PolicyUnavailable | MeetingError::StorageUnavailable => Status::Unavailable,
    }
}

fn note_status(error: &PlaceNoteError) -> Status {
    match error {
        PlaceNoteError::InvalidInput => Status::BadRequest,
        PlaceNoteError::Denied | PlaceNoteError::ModerationDenied => Status::Forbidden,
        PlaceNoteError::NotFound => Status::NotFound,
        PlaceNoteError::Conflict | PlaceNoteError::EventReadOnly => Status::Conflict,
        PlaceNoteError::DependencyUnavailable | PlaceNoteError::InvalidState => Status::Unavailable,
    }
}

fn location_error(result: Result<impl std::fmt::Debug, VenueLocationError>) -> VenueLocationError {
    match result {
        Ok(value) => panic!("expected a location denial, got {value:?}"),
        Err(error) => error,
    }
}

fn meeting_error(result: Result<impl std::fmt::Debug, MeetingError>) -> MeetingError {
    match result {
        Ok(value) => panic!("expected a meeting denial, got {value:?}"),
        Err(error) => error,
    }
}

fn note_error(result: Result<impl std::fmt::Debug, PlaceNoteError>) -> PlaceNoteError {
    match result {
        Ok(value) => panic!("expected a note denial, got {value:?}"),
        Err(error) => error,
    }
}

fn assert_no_secret(text: &str, secrets: &[String]) {
    for secret in secrets {
        assert!(
            !text.contains(secret.as_str()),
            "denied response leaked {secret}: {text}"
        );
    }
}

async fn owned_pool() -> anyhow::Result<PgPool> {
    let url = std::env::var("CONCERT_TEST_DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("CONCERT_TEST_DATABASE_URL is required"))?;
    let options = PgConnectOptions::from_str(&url)
        .map_err(|_| anyhow::anyhow!("invalid owned fixture options"))?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_venue_authz") && options.get_username() == "mcr_owner",
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await?;
    if let Ok(root) = std::env::var("CONCERT_TEST_OWNER_ROOT") {
        let actual: String = sqlx::query_scalar("show data_directory")
            .fetch_one(&pool)
            .await?;
        anyhow::ensure!(
            actual == PathBuf::from(root).join("data").to_string_lossy(),
            "foreign data directory refused"
        );
    }
    provision(&pool).await?;
    Ok(pool)
}

const FIXTURE_DDL: &str = "
create table if not exists public.venue_authz_follows(
    event_id uuid not null, a uuid not null, b uuid not null, primary key(event_id,a,b));
create table if not exists public.venue_authz_blocks(
    event_id uuid not null, blocker uuid not null, blocked uuid not null,
    primary key(event_id,blocker,blocked));
create table if not exists public.venue_authz_suspensions(
    event_id uuid not null, user_id uuid not null, primary key(event_id,user_id));
create table if not exists public.venue_authz_landmarks(
    event_id uuid not null, landmark_id text not null, catalog_revision bigint not null,
    landmark_revision bigint not null, can_meet boolean not null, can_note boolean not null,
    valid_until timestamptz not null,
    primary key(event_id,landmark_id,catalog_revision,landmark_revision));
create table if not exists public.venue_authz_profiles(
    event_id uuid not null, user_id uuid not null, display_name text not null,
    primary key(event_id,user_id));
create table if not exists public.venue_authz_moderators(
    event_id uuid not null, user_id uuid not null, primary key(event_id,user_id));";

async fn provision(pool: &PgPool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(7613023461094200117)")
        .execute(&mut *tx)
        .await?;
    sqlx::raw_sql(FIXTURE_DDL).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

struct Authz {
    pool: PgPool,
}

#[async_trait]
impl VenueLocationAuthority for Authz {
    async fn actor_in(
        &self,
        c: &mut PgConnection,
        actor: ParticipantKey,
        _: DateTime<Utc>,
    ) -> Result<(), VenueLocationError> {
        actor_in(c, actor.event_id.uuid(), actor.user_id.uuid()).await
    }

    async fn friend_in(
        &self,
        c: &mut PgConnection,
        event: EventId,
        viewer: AuthUserId,
        owner: AuthUserId,
        _: DateTime<Utc>,
    ) -> Result<bool, VenueLocationError> {
        bilateral(c, event.uuid(), viewer.uuid(), owner.uuid()).await
    }
}

#[async_trait]
impl VenueMeetingAuthority for Authz {
    async fn friend(
        &self,
        event: Uuid,
        actor: Uuid,
        peer: Uuid,
    ) -> Result<PairFacts, MeetingError> {
        if actor == peer {
            return Err(MeetingError::InvalidInput);
        }
        match bilateral_pool(&self.pool, event, actor, peer).await {
            Ok(true) => Ok(PairFacts {
                relationship_revision: 1,
            }),
            Ok(false) => Err(MeetingError::ResourceUnavailable),
            Err(_) => Err(MeetingError::PolicyUnavailable),
        }
    }

    async fn reviewed_landmark(
        &self,
        event: Uuid,
        selection: &LandmarkSelection,
    ) -> Result<LandmarkReview, MeetingError> {
        let row: Option<(bool, DateTime<Utc>)> = sqlx::query_as(
            "select can_meet, valid_until from public.venue_authz_landmarks
             where event_id=$1 and landmark_id=$2 and catalog_revision=$3 and landmark_revision=$4",
        )
        .bind(event)
        .bind(&selection.landmark_id)
        .bind(selection.catalog_revision)
        .bind(selection.landmark_revision)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| MeetingError::PolicyUnavailable)?;
        match row {
            Some((true, valid_until)) if valid_until > Utc::now() => Ok(LandmarkReview {
                catalog_revision: selection.catalog_revision,
                landmark_revision: selection.landmark_revision,
                can_meet: true,
                valid_until,
            }),
            Some(_) => Err(MeetingError::ResourceUnavailable),
            None => Err(MeetingError::ResourceUnavailable),
        }
    }

    async fn policy(&self, event: Uuid) -> Result<MeetingPolicy, MeetingError> {
        if event.is_nil() {
            return Err(MeetingError::PolicyUnavailable);
        }
        let now = Utc::now();
        Ok(MeetingPolicy {
            policy_revision: 1,
            valid_until: now + Duration::hours(1),
            invitation_ms: 600_000,
            content_ms: 600_000,
            purge_ms: 3_600_000,
            event_timezone: "Asia/Seoul".into(),
        })
    }
}

async fn bilateral_pool(
    pool: &PgPool,
    event: Uuid,
    actor: Uuid,
    peer: Uuid,
) -> Result<bool, sqlx::Error> {
    if actor == peer {
        return Ok(false);
    }
    let restricted: bool = sqlx::query_scalar(
        "select exists(select 1 from public.venue_authz_suspensions where event_id=$1 and user_id=$2)
             or exists(select 1 from public.venue_authz_suspensions where event_id=$1 and user_id=$3)",
    )
    .bind(event)
    .bind(actor)
    .bind(peer)
    .fetch_one(pool)
    .await?;
    if restricted {
        return Ok(false);
    }
    let blocked: bool = sqlx::query_scalar(
        "select exists(select 1 from public.venue_authz_blocks
          where event_id=$1 and ((blocker=$2 and blocked=$3) or (blocker=$3 and blocked=$2)))",
    )
    .bind(event)
    .bind(actor)
    .bind(peer)
    .fetch_one(pool)
    .await?;
    if blocked {
        return Ok(false);
    }
    let edges: i64 = sqlx::query_scalar(
        "select count(*) from public.venue_authz_follows
          where event_id=$1 and ((a=$2 and b=$3) or (a=$3 and b=$2))",
    )
    .bind(event)
    .bind(actor)
    .bind(peer)
    .fetch_one(pool)
    .await?;
    Ok(edges == 2)
}

async fn actor_in(c: &mut PgConnection, event: Uuid, user: Uuid) -> Result<(), VenueLocationError> {
    let phase: Option<String> =
        sqlx::query_scalar("select phase from public.concert_events where event_id=$1 for share")
            .bind(event)
            .fetch_optional(&mut *c)
            .await
            .map_err(|_| VenueLocationError::DependencyUnavailable)?;
    if !matches!(phase.as_deref(), Some("OPEN") | Some("LIVE")) {
        return Err(VenueLocationError::Denied);
    }
    let member: Option<Uuid> = sqlx::query_scalar(
        "select user_id from public.concert_participants where event_id=$1 and user_id=$2 for share",
    )
    .bind(event)
    .bind(user)
    .fetch_optional(&mut *c)
    .await
    .map_err(|_| VenueLocationError::DependencyUnavailable)?;
    if member != Some(user) {
        return Err(VenueLocationError::Denied);
    }
    if suspended(c, event, user).await? {
        return Err(VenueLocationError::Denied);
    }
    Ok(())
}

async fn bilateral(
    c: &mut PgConnection,
    event: Uuid,
    a: Uuid,
    b: Uuid,
) -> Result<bool, VenueLocationError> {
    if a == b {
        return Ok(false);
    }
    if suspended(c, event, a).await? || suspended(c, event, b).await? {
        return Ok(false);
    }
    let blocked: bool = sqlx::query_scalar(
        "select exists(select 1 from public.venue_authz_blocks
          where event_id=$1 and ((blocker=$2 and blocked=$3) or (blocker=$3 and blocked=$2)))",
    )
    .bind(event)
    .bind(a)
    .bind(b)
    .fetch_one(&mut *c)
    .await
    .map_err(|_| VenueLocationError::DependencyUnavailable)?;
    if blocked {
        return Ok(false);
    }
    let edges: i64 = sqlx::query_scalar(
        "select count(*) from public.venue_authz_follows
          where event_id=$1 and ((a=$2 and b=$3) or (a=$3 and b=$2))",
    )
    .bind(event)
    .bind(a)
    .bind(b)
    .fetch_one(&mut *c)
    .await
    .map_err(|_| VenueLocationError::DependencyUnavailable)?;
    Ok(edges == 2)
}

async fn suspended(
    c: &mut PgConnection,
    event: Uuid,
    user: Uuid,
) -> Result<bool, VenueLocationError> {
    sqlx::query_scalar(
        "select exists(select 1 from public.venue_authz_suspensions where event_id=$1 and user_id=$2)",
    )
    .bind(event)
    .bind(user)
    .fetch_one(&mut *c)
    .await
    .map_err(|_| VenueLocationError::DependencyUnavailable)
}

struct Policy {
    event: Uuid,
    valid_until: DateTime<Utc>,
}

#[async_trait]
impl VenueLocationPolicyProvider for Policy {
    async fn policy(&self, event: EventId) -> Result<VenueLocationPolicy, VenueLocationError> {
        if event.uuid() != self.event {
            return Err(VenueLocationError::PolicyUnavailable);
        }
        Ok(VenueLocationPolicy {
            event_id: event,
            venue_key: "venue-authz".into(),
            revision: 1,
            timezone: "Asia/Seoul".into(),
            valid_until: self.valid_until,
            fresh_ms: 30_000,
            last_known_ms: 120_000,
            fix_max_age_ms: 120_000,
            future_skew_ms: 5_000,
            sharing_lease_ms: 60_000,
            request_ttl_ms: 60_000,
            check_in_ttl_ms: 60_000,
            coordinate_retention_ms: 300_000,
            command_retention_ms: 60_000,
            max_accuracy_m: 50.0,
            max_viewers: 10,
        })
    }
}

struct Fx {
    pool: PgPool,
    event: Uuid,
    valid_until: DateTime<Utc>,
    owner: Uuid,
    friend: Uuid,
    stranger: Uuid,
    blocked: Uuid,
    suspended: Uuid,
    admin: Uuid,
    outsider: Uuid,
}

impl Fx {
    fn key(&self, user: Uuid) -> ParticipantKey {
        ParticipantKey {
            event_id: EventId::from_uuid(self.event),
            user_id: AuthUserId::from_uuid(user),
        }
    }
    fn location(&self) -> PgVenueLocation<Policy, Authz> {
        PgVenueLocation::new(
            self.pool.clone(),
            Policy {
                event: self.event,
                valid_until: self.valid_until,
            },
            Authz {
                pool: self.pool.clone(),
            },
        )
    }
    fn meeting(&self) -> PgVenueMeeting<Authz> {
        PgVenueMeeting::new(
            self.pool.clone(),
            Authz {
                pool: self.pool.clone(),
            },
        )
    }
    fn notes(&self) -> PgPlaceNotes {
        PgPlaceNotes::new(
            self.pool.clone(),
            Arc::new(NotesPlace),
            Arc::new(NotesFriend),
            Arc::new(NotesProfile),
            Arc::new(NotesMeeting),
            Arc::new(NotesModeration),
        )
    }
    fn selection(&self, peer: Uuid, landmark: bool) -> MeetingSelection {
        MeetingSelection::new(peer, landmark.then(landmark_selection)).unwrap()
    }
}

fn landmark_selection() -> LandmarkSelection {
    LandmarkSelection::new(LANDMARK, CATALOG_REVISION, LANDMARK_REVISION).unwrap()
}

async fn fixture() -> anyhow::Result<Fx> {
    let pool = owned_pool().await?;
    let event = Uuid::new_v4();
    let valid_until = Utc::now() + Duration::hours(1);
    sqlx::query(
        "insert into concert_events(event_id,slug,title,phase) values($1,$2,'Venue authz','OPEN')",
    )
    .bind(event)
    .bind(format!("authz-{}", event.simple()))
    .execute(&pool)
    .await?;
    let owner = Uuid::new_v4();
    let friend = Uuid::new_v4();
    let stranger = Uuid::new_v4();
    let blocked = Uuid::new_v4();
    let suspended = Uuid::new_v4();
    let admin = Uuid::new_v4();
    let outsider = Uuid::new_v4();
    for user in [owner, friend, stranger, blocked, suspended, admin] {
        sqlx::query(
            "insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled)
             values($1,$2,'REVENGE',false,false,false)",
        )
        .bind(event)
        .bind(user)
        .execute(&pool)
        .await?;
    }
    for (a, b) in [
        (owner, friend),
        (friend, owner),
        (owner, blocked),
        (blocked, owner),
        (owner, suspended),
        (suspended, owner),
    ] {
        sqlx::query("insert into venue_authz_follows(event_id,a,b) values($1,$2,$3)")
            .bind(event)
            .bind(a)
            .bind(b)
            .execute(&pool)
            .await?;
    }
    for (user, name) in [
        (owner, "Owner"),
        (friend, "Friend"),
        (stranger, "Stranger"),
        (blocked, "Blocked"),
        (suspended, "Suspended"),
        (admin, "Admin"),
        (outsider, "Outsider"),
    ] {
        sqlx::query(
            "insert into venue_authz_profiles(event_id,user_id,display_name) values($1,$2,$3)",
        )
        .bind(event)
        .bind(user)
        .bind(name)
        .execute(&pool)
        .await?;
    }
    sqlx::query("insert into venue_authz_moderators(event_id,user_id) values($1,$2)")
        .bind(event)
        .bind(admin)
        .execute(&pool)
        .await?;
    sqlx::query(
        "insert into venue_authz_landmarks(event_id,landmark_id,catalog_revision,landmark_revision,can_meet,can_note,valid_until)
         values($1,$2,$3,$4,true,true,$5)",
    )
    .bind(event)
    .bind(LANDMARK)
    .bind(CATALOG_REVISION)
    .bind(LANDMARK_REVISION)
    .bind(valid_until)
    .execute(&pool)
    .await?;
    Ok(Fx {
        pool,
        event,
        valid_until,
        owner,
        friend,
        stranger,
        blocked,
        suspended,
        admin,
        outsider,
    })
}

async fn restriction(fx: &Fx, user: Uuid, blocked: bool, suspended: bool) {
    if blocked {
        sqlx::query("insert into venue_authz_blocks(event_id,blocker,blocked) values($1,$2,$3)")
            .bind(fx.event)
            .bind(fx.owner)
            .bind(user)
            .execute(&fx.pool)
            .await
            .unwrap();
    }
    if suspended {
        sqlx::query("insert into venue_authz_suspensions(event_id,user_id) values($1,$2)")
            .bind(fx.event)
            .bind(user)
            .execute(&fx.pool)
            .await
            .unwrap();
    }
}

async fn share(fx: &Fx, viewers: &[Uuid]) -> Result<CommandReceipt, VenueLocationError> {
    fx.location()
        .set_sharing(
            fx.key(fx.owner),
            Uuid::new_v4(),
            SharingCommand::Active {
                expected_revision: 0,
                viewer_ids: viewers.to_vec(),
            },
        )
        .await
}

async fn publish_fix(fx: &Fx, revision: i64) -> Result<CommandReceipt, VenueLocationError> {
    fx.location()
        .publish_fix(
            fx.key(fx.owner),
            Uuid::new_v4(),
            PhoneFixInput {
                expected_sharing_revision: revision,
                latitude: LATITUDE,
                longitude: LONGITUDE,
                accuracy_m: 8.0,
                captured_at: Utc::now(),
            },
        )
        .await
}

async fn active_fix(fx: &Fx, viewers: &[Uuid]) -> LocationSnapshot {
    share(fx, viewers).await.unwrap();
    publish_fix(fx, 1).await.unwrap();
    fx.location().snapshot(fx.key(fx.owner)).await.unwrap()
}

async fn visible(fx: &Fx, viewer: Uuid) -> Result<VisibleLocation, VenueLocationError> {
    fx.location()
        .visible_for(fx.key(viewer), AuthUserId::from_uuid(fx.owner))
        .await
}

async fn invitation(fx: &Fx, sender: Uuid, recipient: Uuid) -> Value {
    fx.meeting()
        .create_invitation(
            fx.event,
            sender,
            Uuid::new_v4(),
            &CreateInvitation::new(fx.selection(recipient, true)).unwrap(),
        )
        .await
        .unwrap()
}

fn invitation_id(value: &Value) -> Uuid {
    Uuid::parse_str(
        value["result"]["invitation"]["id"]
            .as_str()
            .expect("invitation id"),
    )
    .expect("uuid")
}

async fn send_message(fx: &Fx, sender: Uuid, peer: Uuid, body: &str) -> Value {
    fx.meeting()
        .send(
            fx.event,
            sender,
            Uuid::new_v4(),
            &SendMessage::new(fx.selection(peer, false), body, None).unwrap(),
        )
        .await
        .unwrap()
}

fn note(audience: NoteAudience, text: &str) -> CreatePlaceNote {
    CreatePlaceNote::new(
        text,
        audience,
        NoteExpiry::OneHour,
        NoteLandmarkSelection::new(LANDMARK, CATALOG_REVISION, LANDMARK_REVISION).unwrap(),
    )
    .unwrap()
}

struct NotesPlace;
#[async_trait]
impl NotePlaceAuthority for NotesPlace {
    async fn review(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        selection: &NoteLandmarkSelection,
    ) -> Result<(), PlaceNoteError> {
        let ok: Option<bool> = sqlx::query_scalar(
            "select can_note from public.venue_authz_landmarks
             where event_id=$1 and landmark_id=$2 and catalog_revision=$3 and landmark_revision=$4",
        )
        .bind(event)
        .bind(&selection.landmark_id)
        .bind(selection.catalog_revision)
        .bind(selection.landmark_revision)
        .fetch_optional(&mut *c)
        .await
        .map_err(|_| PlaceNoteError::DependencyUnavailable)?;
        match ok {
            Some(true) => Ok(()),
            _ => Err(PlaceNoteError::NotFound),
        }
    }

    async fn current(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        landmark_id: &str,
    ) -> Result<(), PlaceNoteError> {
        let ok: Option<bool> = sqlx::query_scalar(
            "select bool_or(can_note) from public.venue_authz_landmarks
             where event_id=$1 and landmark_id=$2",
        )
        .bind(event)
        .bind(landmark_id)
        .fetch_optional(&mut *c)
        .await
        .map_err(|_| PlaceNoteError::DependencyUnavailable)?;
        match ok {
            Some(true) => Ok(()),
            _ => Err(PlaceNoteError::NotFound),
        }
    }
}

struct NotesFriend;
#[async_trait]
impl NoteFriendAuthority for NotesFriend {
    async fn available(&self, _: &mut PgConnection) -> Result<(), PlaceNoteError> {
        Ok(())
    }

    async fn bilateral(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        author: Uuid,
        viewer: Uuid,
    ) -> Result<(), PlaceNoteError> {
        match bilateral(c, event, author, viewer).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(PlaceNoteError::Denied),
            Err(_) => Err(PlaceNoteError::DependencyUnavailable),
        }
    }
}

struct NotesProfile;
#[async_trait]
impl NoteProfileAuthority for NotesProfile {
    async fn profile(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        user: Uuid,
    ) -> Result<NoteAuthor, PlaceNoteError> {
        let name: Option<String> = sqlx::query_scalar(
            "select display_name from public.venue_authz_profiles where event_id=$1 and user_id=$2",
        )
        .bind(event)
        .bind(user)
        .fetch_optional(&mut *c)
        .await
        .map_err(|_| PlaceNoteError::DependencyUnavailable)?;
        NoteAuthor::new(
            user,
            name.as_deref().ok_or(PlaceNoteError::NotFound)?,
            None,
            json!({"revision": 1, "config": {"kind": "avatar_config"}}),
        )
    }
}

struct NotesMeeting;
#[async_trait]
impl NoteMeetingAuthority for NotesMeeting {
    async fn composer_entry(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        viewer: Uuid,
        author: Uuid,
        landmark_id: &str,
        landmark_revision: i64,
        access_until: DateTime<Utc>,
    ) -> Result<Option<NoteMeetingEntry>, PlaceNoteError> {
        let friend = matches!(bilateral(c, event, viewer, author).await, Ok(true));
        let landmark: Option<bool> = sqlx::query_scalar(
            "select bool_or(can_meet) from public.venue_authz_landmarks
             where event_id=$1 and landmark_id=$2 and landmark_revision=$3",
        )
        .bind(event)
        .bind(landmark_id)
        .bind(landmark_revision)
        .fetch_optional(&mut *c)
        .await
        .map_err(|_| PlaceNoteError::DependencyUnavailable)?;
        if friend && landmark == Some(true) && viewer != author {
            Ok(Some(NoteMeetingEntry {
                landmark_id: landmark_id.to_owned(),
                recipient_user_id: author,
                landmark_revision,
                access_until,
            }))
        } else {
            Ok(None)
        }
    }
}

struct NotesModeration;
#[async_trait]
impl NoteModerationAuthority for NotesModeration {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        moderator: Uuid,
    ) -> Result<(), PlaceNoteError> {
        let allowed: bool = sqlx::query_scalar(
            "select exists(select 1 from public.venue_authz_moderators where event_id=$1 and user_id=$2)",
        )
        .bind(event)
        .bind(moderator)
        .fetch_one(&mut *c)
        .await
        .map_err(|_| PlaceNoteError::DependencyUnavailable)?;
        allowed.then_some(()).ok_or(PlaceNoteError::Denied)
    }
}

#[tokio::test]
async fn location_owner_reads_own_snapshot() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let snapshot = active_fix(&fx, &[fx.friend]).await;
    assert_eq!(snapshot.sharing.state, SharingState::Active);
    let own = snapshot.self_location.expect("owner position");
    assert_eq!(own.position.latitude, LATITUDE);
    assert!(snapshot.friends.is_empty());
    assert_eq!(Status::Ok.code(), 200);
    Ok(())
}

#[tokio::test]
async fn location_friend_reads_selected_sharing() -> anyhow::Result<()> {
    let fx = fixture().await?;
    active_fix(&fx, &[fx.friend]).await;
    let location = visible(&fx, fx.friend).await.unwrap();
    assert_eq!(location.position.latitude, LATITUDE);
    assert_eq!(location.user_id, fx.owner);
    assert!(!location.physical_arrival_verified);
    Ok(())
}

#[tokio::test]
async fn location_stranger_denied_owner_location() -> anyhow::Result<()> {
    let fx = fixture().await?;
    active_fix(&fx, &[fx.friend]).await;
    let error = location_error(visible(&fx, fx.stranger).await);
    assert_eq!(error, VenueLocationError::Denied);
    assert_eq!(location_status(&error), Status::NotFound);
    let snapshot = fx.location().snapshot(fx.key(fx.stranger)).await.unwrap();
    assert!(snapshot.friends.is_empty());
    assert!(!format!("{snapshot:?}").contains(&LATITUDE.to_string()));
    Ok(())
}

#[tokio::test]
async fn location_blocked_friend_denied_owner_location() -> anyhow::Result<()> {
    let fx = fixture().await?;
    active_fix(&fx, &[fx.blocked]).await;
    restriction(&fx, fx.blocked, true, false).await;
    let error = location_error(visible(&fx, fx.blocked).await);
    assert_eq!(error, VenueLocationError::Denied);
    assert_eq!(location_status(&error), Status::NotFound);
    let snapshot = fx.location().snapshot(fx.key(fx.blocked)).await.unwrap();
    assert!(snapshot.friends.is_empty());
    Ok(())
}

#[tokio::test]
async fn location_suspended_viewer_snapshot_denied() -> anyhow::Result<()> {
    let fx = fixture().await?;
    active_fix(&fx, &[fx.suspended]).await;
    restriction(&fx, fx.suspended, false, true).await;
    let error = location_error(fx.location().snapshot(fx.key(fx.suspended)).await);
    assert_eq!(error, VenueLocationError::Denied);
    assert_eq!(location_status(&error), Status::NotFound);
    let error = location_error(visible(&fx, fx.suspended).await);
    assert_eq!(location_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn location_expired_lease_returns_no_position() -> anyhow::Result<()> {
    let fx = fixture().await?;
    active_fix(&fx, &[fx.friend]).await;
    sqlx::query(
        "update public.concert_venue_location_sharing
         set expires_at = clock_timestamp() - interval '1 second'
         where event_id=$1 and owner_id=$2",
    )
    .bind(fx.event)
    .bind(fx.owner)
    .execute(&fx.pool)
    .await?;
    let snapshot = fx.location().snapshot(fx.key(fx.owner)).await.unwrap();
    assert_eq!(snapshot.sharing.state, SharingState::Expired);
    assert!(snapshot.self_location.is_none());
    let error = location_error(visible(&fx, fx.friend).await);
    assert_eq!(error, VenueLocationError::NotFound);
    assert_eq!(location_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn location_admin_without_selection_denied() -> anyhow::Result<()> {
    let fx = fixture().await?;
    active_fix(&fx, &[fx.friend]).await;
    let error = location_error(visible(&fx, fx.admin).await);
    assert_eq!(error, VenueLocationError::Denied);
    assert_eq!(location_status(&error), Status::NotFound);
    let snapshot = fx.location().snapshot(fx.key(fx.admin)).await.unwrap();
    assert!(snapshot.friends.is_empty());
    Ok(())
}

#[tokio::test]
async fn location_pause_clears_viewers_and_fix() -> anyhow::Result<()> {
    let fx = fixture().await?;
    active_fix(&fx, &[fx.friend]).await;
    fx.location()
        .set_sharing(
            fx.key(fx.owner),
            Uuid::new_v4(),
            SharingCommand::Paused {
                expected_revision: 1,
                viewer_ids: Vec::new(),
            },
        )
        .await
        .unwrap();
    let (viewers, fixes): (i64, i64) = sqlx::query_as(
        "select
           (select count(*) from public.concert_venue_location_viewers where event_id=$1 and owner_id=$2),
           (select count(*) from public.concert_venue_location_fixes where event_id=$1 and owner_id=$2)",
    )
    .bind(fx.event)
    .bind(fx.owner)
    .fetch_one(&fx.pool)
    .await?;
    assert_eq!((viewers, fixes), (0, 0));
    let snapshot = fx.location().snapshot(fx.key(fx.owner)).await.unwrap();
    assert_eq!(snapshot.sharing.state, SharingState::Paused);
    assert!(snapshot.self_location.is_none());
    assert_eq!(
        location_error(visible(&fx, fx.friend).await),
        VenueLocationError::NotFound
    );
    Ok(())
}

#[tokio::test]
async fn location_revoke_removes_immediately() -> anyhow::Result<()> {
    let fx = fixture().await?;
    active_fix(&fx, &[fx.friend]).await;
    assert!(visible(&fx, fx.friend).await.is_ok());
    fx.location()
        .set_sharing(
            fx.key(fx.owner),
            Uuid::new_v4(),
            SharingCommand::Revoked {
                expected_revision: 1,
                viewer_ids: Vec::new(),
            },
        )
        .await
        .unwrap();
    let error = location_error(visible(&fx, fx.friend).await);
    assert_eq!(error, VenueLocationError::NotFound);
    let snapshot = fx.location().snapshot(fx.key(fx.friend)).await.unwrap();
    assert!(snapshot.friends.is_empty());
    Ok(())
}

#[tokio::test]
async fn location_non_participant_denied() -> anyhow::Result<()> {
    let fx = fixture().await?;
    active_fix(&fx, &[fx.friend]).await;
    let error = location_error(visible(&fx, fx.outsider).await);
    assert_eq!(error, VenueLocationError::Denied);
    assert_eq!(location_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn invitation_friend_create_succeeds() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let value = invitation(&fx, fx.owner, fx.friend).await;
    assert_eq!(value["state"], "APPLIED");
    assert_eq!(value["result"]["kind"], "INVITATION");
    assert_eq!(value["result"]["invitation"]["state"], "PENDING");
    assert_eq!(
        value["result"]["invitation"]["senderId"],
        fx.owner.to_string()
    );
    assert_eq!(
        value["result"]["invitation"]["recipientId"],
        fx.friend.to_string()
    );
    assert_eq!(value["result"]["invitation"]["landmarkId"], LANDMARK);
    assert_eq!(Status::Ok.code(), 200);
    Ok(())
}

#[tokio::test]
async fn invitation_stranger_create_denied() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let error = meeting_error(
        fx.meeting()
            .create_invitation(
                fx.event,
                fx.stranger,
                Uuid::new_v4(),
                &CreateInvitation::new(fx.selection(fx.friend, true)).unwrap(),
            )
            .await,
    );
    assert_eq!(error, MeetingError::ResourceUnavailable);
    assert_eq!(meeting_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn invitation_blocked_create_denied() -> anyhow::Result<()> {
    let fx = fixture().await?;
    restriction(&fx, fx.blocked, true, false).await;
    let error = meeting_error(
        fx.meeting()
            .create_invitation(
                fx.event,
                fx.blocked,
                Uuid::new_v4(),
                &CreateInvitation::new(fx.selection(fx.owner, true)).unwrap(),
            )
            .await,
    );
    assert_eq!(error, MeetingError::ResourceUnavailable);
    assert_eq!(meeting_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn invitation_suspended_create_denied() -> anyhow::Result<()> {
    let fx = fixture().await?;
    restriction(&fx, fx.suspended, false, true).await;
    let error = meeting_error(
        fx.meeting()
            .create_invitation(
                fx.event,
                fx.suspended,
                Uuid::new_v4(),
                &CreateInvitation::new(fx.selection(fx.owner, true)).unwrap(),
            )
            .await,
    );
    assert_eq!(error, MeetingError::ResourceUnavailable);
    assert_eq!(meeting_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn invitation_admin_create_denied() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let error = meeting_error(
        fx.meeting()
            .create_invitation(
                fx.event,
                fx.admin,
                Uuid::new_v4(),
                &CreateInvitation::new(fx.selection(fx.owner, true)).unwrap(),
            )
            .await,
    );
    assert_eq!(error, MeetingError::ResourceUnavailable);
    assert_eq!(meeting_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn invitation_stale_landmark_denied() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let stale = LandmarkSelection::new(LANDMARK, CATALOG_REVISION, LANDMARK_REVISION + 1).unwrap();
    let selection = MeetingSelection::new(fx.friend, Some(stale)).unwrap();
    let error = meeting_error(
        fx.meeting()
            .create_invitation(
                fx.event,
                fx.owner,
                Uuid::new_v4(),
                &CreateInvitation::new(selection).unwrap(),
            )
            .await,
    );
    assert_eq!(error, MeetingError::ResourceUnavailable);
    assert_eq!(meeting_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn invitation_recipient_accepts_and_sender_cancels() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let value = invitation(&fx, fx.owner, fx.friend).await;
    let id = invitation_id(&value);
    let accepted = fx
        .meeting()
        .decide(
            fx.event,
            fx.friend,
            id,
            Uuid::new_v4(),
            &DecideInvitation::new(fx.selection(fx.owner, true), 1, Decision::Accept).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(accepted["result"]["invitation"]["state"], "ACCEPTED");
    assert_eq!(accepted["result"]["invitation"]["revision"], 2);
    let cancelled = fx
        .meeting()
        .decide(
            fx.event,
            fx.owner,
            id,
            Uuid::new_v4(),
            &DecideInvitation::new(fx.selection(fx.friend, true), 2, Decision::Cancel).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancelled["result"]["invitation"]["state"], "CANCELLED");
    let error = meeting_error(
        fx.meeting()
            .decide(
                fx.event,
                fx.friend,
                id,
                Uuid::new_v4(),
                &DecideInvitation::new(fx.selection(fx.owner, true), 3, Decision::Accept).unwrap(),
            )
            .await,
    );
    assert_eq!(error, MeetingError::ResourceUnavailable);
    assert_eq!(meeting_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn invitation_expired_decision_conflicts() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let value = invitation(&fx, fx.owner, fx.friend).await;
    let id = invitation_id(&value);
    let mut tx = fx.pool.begin().await?;
    sqlx::query("select set_config('mcr.venue_meeting_authority_until', $1, true)")
        .bind((Utc::now() + Duration::hours(1)).to_rfc3339())
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "update public.concert_venue_meeting_invitations
         set expires_at = created_at + interval '1 microsecond'
         where event_id=$1 and id=$2",
    )
    .bind(fx.event)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let error = meeting_error(
        fx.meeting()
            .decide(
                fx.event,
                fx.friend,
                id,
                Uuid::new_v4(),
                &DecideInvitation::new(fx.selection(fx.owner, true), 1, Decision::Accept).unwrap(),
            )
            .await,
    );
    assert_eq!(error, MeetingError::InvalidState);
    assert_eq!(meeting_status(&error), Status::Conflict);
    let detail = fx
        .meeting()
        .invitation(fx.event, fx.friend, id)
        .await
        .unwrap();
    assert_eq!(detail["state"], "PENDING");
    assert_eq!(
        detail["allowedInvitationActions"].as_array().unwrap().len(),
        0
    );
    Ok(())
}

#[tokio::test]
async fn invitation_stranger_detail_denied_and_inbox_empty() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let value = invitation(&fx, fx.owner, fx.friend).await;
    let id = invitation_id(&value);
    let error = meeting_error(fx.meeting().invitation(fx.event, fx.stranger, id).await);
    assert_eq!(error, MeetingError::ResourceUnavailable);
    assert_eq!(meeting_status(&error), Status::NotFound);
    let inbox = fx
        .meeting()
        .inbox(fx.event, fx.stranger, Direction::Incoming, 0)
        .await
        .unwrap();
    assert_eq!(inbox["items"].as_array().unwrap().len(), 0);
    assert_no_secret(&inbox.to_string(), &[id.to_string(), fx.owner.to_string()]);
    Ok(())
}

#[tokio::test]
async fn conversation_friend_context_allows() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let context = fx
        .meeting()
        .context(fx.event, fx.friend, &fx.selection(fx.owner, false))
        .await
        .unwrap();
    assert_eq!(context["canRead"], true);
    assert_eq!(context["canSend"], true);
    assert_eq!(context["canInvite"], false);
    assert_eq!(context["actorId"], fx.friend.to_string());
    Ok(())
}

#[tokio::test]
async fn conversation_stranger_context_denied() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let error = meeting_error(
        fx.meeting()
            .context(fx.event, fx.stranger, &fx.selection(fx.owner, false))
            .await,
    );
    assert_eq!(error, MeetingError::ResourceUnavailable);
    assert_eq!(meeting_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn conversation_blocked_context_denied() -> anyhow::Result<()> {
    let fx = fixture().await?;
    restriction(&fx, fx.blocked, true, false).await;
    let error = meeting_error(
        fx.meeting()
            .context(fx.event, fx.blocked, &fx.selection(fx.owner, false))
            .await,
    );
    assert_eq!(error, MeetingError::ResourceUnavailable);
    assert_eq!(meeting_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn conversation_suspended_context_denied() -> anyhow::Result<()> {
    let fx = fixture().await?;
    restriction(&fx, fx.suspended, false, true).await;
    let error = meeting_error(
        fx.meeting()
            .context(fx.event, fx.suspended, &fx.selection(fx.owner, false))
            .await,
    );
    assert_eq!(error, MeetingError::ResourceUnavailable);
    assert_eq!(meeting_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn conversation_admin_context_denied() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let error = meeting_error(
        fx.meeting()
            .context(fx.event, fx.admin, &fx.selection(fx.owner, false))
            .await,
    );
    assert_eq!(error, MeetingError::ResourceUnavailable);
    assert_eq!(meeting_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn conversation_friend_round_trip() -> anyhow::Result<()> {
    let fx = fixture().await?;
    send_message(&fx, fx.owner, fx.friend, "Meet at the north gate").await;
    let page = fx
        .meeting()
        .messages(fx.event, fx.friend, &fx.selection(fx.owner, false), 0)
        .await
        .unwrap();
    let items = page["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["body"], "Meet at the north gate");
    assert_eq!(items[0]["senderId"], fx.owner.to_string());
    Ok(())
}

#[tokio::test]
async fn conversation_stranger_cannot_read_messages() -> anyhow::Result<()> {
    let fx = fixture().await?;
    send_message(&fx, fx.owner, fx.friend, "private plan").await;
    let error = meeting_error(
        fx.meeting()
            .messages(fx.event, fx.stranger, &fx.selection(fx.owner, false), 0)
            .await,
    );
    assert_eq!(error, MeetingError::ResourceUnavailable);
    assert_eq!(meeting_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn conversation_expired_message_hidden() -> anyhow::Result<()> {
    let fx = fixture().await?;
    send_message(&fx, fx.owner, fx.friend, "expired plan").await;
    sqlx::query(
        "update public.concert_venue_meeting_messages
         set expires_at = created_at + interval '1 microsecond'
         where event_id=$1",
    )
    .bind(fx.event)
    .execute(&fx.pool)
    .await?;
    let page = fx
        .meeting()
        .messages(fx.event, fx.friend, &fx.selection(fx.owner, false), 0)
        .await
        .unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 0);
    assert_no_secret(&page.to_string(), &["expired plan".to_owned()]);
    Ok(())
}

#[tokio::test]
async fn conversation_block_revokes_read_access() -> anyhow::Result<()> {
    let fx = fixture().await?;
    send_message(&fx, fx.owner, fx.friend, "before the block").await;
    assert!(fx
        .meeting()
        .messages(fx.event, fx.friend, &fx.selection(fx.owner, false), 0)
        .await
        .is_ok());
    restriction(&fx, fx.friend, true, false).await;
    let error = meeting_error(
        fx.meeting()
            .messages(fx.event, fx.friend, &fx.selection(fx.owner, false), 0)
            .await,
    );
    assert_eq!(error, MeetingError::ResourceUnavailable);
    assert_eq!(meeting_status(&error), Status::NotFound);
    let stored: i64 = sqlx::query_scalar(
        "select count(*) from public.concert_venue_meeting_messages where event_id=$1",
    )
    .bind(fx.event)
    .fetch_one(&fx.pool)
    .await?;
    assert_eq!(stored, 1, "history is retained but never disclosed");
    Ok(())
}

#[tokio::test]
async fn conversation_invitation_link_does_not_grant() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let value = invitation(&fx, fx.owner, fx.friend).await;
    let id = invitation_id(&value);
    let command = SendMessage::new(fx.selection(fx.owner, false), "link only", Some(id)).unwrap();
    let error = meeting_error(
        fx.meeting()
            .send(fx.event, fx.stranger, Uuid::new_v4(), &command)
            .await,
    );
    assert_eq!(error, MeetingError::ResourceUnavailable);
    assert_eq!(meeting_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn conversation_context_has_no_write_side_effect() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let before: (i64, i64) = sqlx::query_as(
        "select
           (select count(*) from public.concert_venue_meeting_contexts where event_id=$1),
           (select count(*) from public.concert_venue_meeting_conversations where event_id=$1)",
    )
    .bind(fx.event)
    .fetch_one(&fx.pool)
    .await?;
    fx.meeting()
        .context(fx.event, fx.friend, &fx.selection(fx.owner, true))
        .await
        .unwrap();
    let after: (i64, i64) = sqlx::query_as(
        "select
           (select count(*) from public.concert_venue_meeting_contexts where event_id=$1),
           (select count(*) from public.concert_venue_meeting_conversations where event_id=$1)",
    )
    .bind(fx.event)
    .fetch_one(&fx.pool)
    .await?;
    assert_eq!(before, after);
    assert_eq!(after, (0, 0));
    Ok(())
}

#[tokio::test]
async fn notes_friend_sees_friends_note() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let created = fx
        .notes()
        .create_note(
            fx.owner,
            fx.event,
            note(NoteAudience::Friends, PRIVATE_NOTE),
            Uuid::new_v4(),
        )
        .await
        .unwrap();
    let page = fx
        .notes()
        .list_notes(
            fx.friend,
            fx.event,
            NoteListQuery::new(LANDMARK, None, None)?,
        )
        .await
        .unwrap();
    assert_eq!(page.visible_count, 1);
    assert_eq!(page.items[0].id, created.id);
    assert_eq!(page.items[0].text, PRIVATE_NOTE);
    Ok(())
}

#[tokio::test]
async fn notes_stranger_cannot_see_friends_note() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let created = fx
        .notes()
        .create_note(
            fx.owner,
            fx.event,
            note(NoteAudience::Friends, PRIVATE_NOTE),
            Uuid::new_v4(),
        )
        .await
        .unwrap();
    let page = fx
        .notes()
        .list_notes(
            fx.stranger,
            fx.event,
            NoteListQuery::new(LANDMARK, None, None)?,
        )
        .await
        .unwrap();
    assert_eq!(page.visible_count, 0);
    assert_no_secret(&format!("{page:?}"), &[PRIVATE_NOTE.to_owned()]);
    let error = note_error(
        fx.notes()
            .note_thread(fx.stranger, fx.event, LANDMARK, created.id, None, None)
            .await,
    );
    assert_eq!(error, PlaceNoteError::NotFound);
    assert_eq!(note_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn notes_blocked_friend_cannot_see_friends_note() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let created = fx
        .notes()
        .create_note(
            fx.owner,
            fx.event,
            note(NoteAudience::Friends, PRIVATE_NOTE),
            Uuid::new_v4(),
        )
        .await
        .unwrap();
    restriction(&fx, fx.blocked, true, false).await;
    let page = fx
        .notes()
        .list_notes(
            fx.blocked,
            fx.event,
            NoteListQuery::new(LANDMARK, None, None)?,
        )
        .await
        .unwrap();
    assert_eq!(page.visible_count, 0);
    assert_eq!(
        note_error(
            fx.notes()
                .note_thread(fx.blocked, fx.event, LANDMARK, created.id, None, None)
                .await
        ),
        PlaceNoteError::NotFound
    );
    Ok(())
}

#[tokio::test]
async fn notes_suspended_actor_denied() -> anyhow::Result<()> {
    let fx = fixture().await?;
    sqlx::query("delete from concert_participants where event_id=$1 and user_id=$2")
        .bind(fx.event)
        .bind(fx.suspended)
        .execute(&fx.pool)
        .await?;
    let error = note_error(
        fx.notes()
            .list_notes(
                fx.suspended,
                fx.event,
                NoteListQuery::new(LANDMARK, None, None)?,
            )
            .await,
    );
    assert_eq!(error, PlaceNoteError::Denied);
    assert_eq!(note_status(&error), Status::Forbidden);
    let error = note_error(
        fx.notes()
            .create_note(
                fx.suspended,
                fx.event,
                note(NoteAudience::Everyone, "suspended"),
                Uuid::new_v4(),
            )
            .await,
    );
    assert_eq!(note_status(&error), Status::Forbidden);
    Ok(())
}

#[tokio::test]
async fn notes_admin_cannot_read_friends_note() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let created = fx
        .notes()
        .create_note(
            fx.owner,
            fx.event,
            note(NoteAudience::Friends, PRIVATE_NOTE),
            Uuid::new_v4(),
        )
        .await
        .unwrap();
    let page = fx
        .notes()
        .list_notes(
            fx.admin,
            fx.event,
            NoteListQuery::new(LANDMARK, None, None)?,
        )
        .await
        .unwrap();
    assert_eq!(page.visible_count, 0);
    let error = note_error(
        fx.notes()
            .note_thread(fx.admin, fx.event, LANDMARK, created.id, None, None)
            .await,
    );
    assert_eq!(error, PlaceNoteError::NotFound);
    assert_eq!(note_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn notes_everyone_note_visible_to_participant_stranger() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let created = fx
        .notes()
        .create_note(
            fx.owner,
            fx.event,
            note(NoteAudience::Everyone, "public meeting"),
            Uuid::new_v4(),
        )
        .await
        .unwrap();
    for viewer in [fx.stranger, fx.blocked, fx.admin] {
        let page = fx
            .notes()
            .list_notes(viewer, fx.event, NoteListQuery::new(LANDMARK, None, None)?)
            .await
            .unwrap();
        assert_eq!(page.visible_count, 1);
        assert_eq!(page.items[0].id, created.id);
    }
    Ok(())
}

#[tokio::test]
async fn notes_non_author_withdraw_denied() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let created = fx
        .notes()
        .create_note(
            fx.owner,
            fx.event,
            note(NoteAudience::Everyone, "public"),
            Uuid::new_v4(),
        )
        .await
        .unwrap();
    let error = note_error(
        fx.notes()
            .withdraw_note(
                fx.friend,
                fx.event,
                LANDMARK,
                created.id,
                created.revision,
                Uuid::new_v4(),
            )
            .await,
    );
    assert_eq!(error, PlaceNoteError::Denied);
    assert_eq!(note_status(&error), Status::Forbidden);
    Ok(())
}

#[tokio::test]
async fn notes_friend_replies_and_stranger_reply_denied() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let created = fx
        .notes()
        .create_note(
            fx.owner,
            fx.event,
            note(NoteAudience::Friends, PRIVATE_NOTE),
            Uuid::new_v4(),
        )
        .await
        .unwrap();
    let reply = fx
        .notes()
        .create_reply(
            fx.friend,
            fx.event,
            LANDMARK,
            created.id,
            CreatePlaceReply::new("I am coming", created.revision)?,
            Uuid::new_v4(),
        )
        .await
        .unwrap();
    assert_eq!(reply.parent_id, Some(created.id));
    let thread = fx
        .notes()
        .note_thread(fx.friend, fx.event, LANDMARK, created.id, None, None)
        .await
        .unwrap();
    assert_eq!(thread.replies.len(), 1);
    assert_eq!(thread.note.reply_count, 1);
    let error = note_error(
        fx.notes()
            .create_reply(
                fx.stranger,
                fx.event,
                LANDMARK,
                created.id,
                CreatePlaceReply::new("not my audience", created.revision + 1)?,
                Uuid::new_v4(),
            )
            .await,
    );
    assert_eq!(error, PlaceNoteError::Denied);
    assert_eq!(note_status(&error), Status::Forbidden);
    Ok(())
}

#[tokio::test]
async fn notes_expired_hidden() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let expired = Uuid::new_v4();
    sqlx::query(
        "insert into concert_place_notes
         (note_id,event_id,landmark_id,catalog_revision,landmark_revision,author_id,audience,
          expiry_kind,body,revision,created_at,expires_at)
         values($1,$2,$3,$4,$5,$6,'EVERYONE','ONE_HOUR','expired note',1,
                clock_timestamp()-interval '2 hours',clock_timestamp()-interval '1 hour')",
    )
    .bind(expired)
    .bind(fx.event)
    .bind(LANDMARK)
    .bind(CATALOG_REVISION)
    .bind(LANDMARK_REVISION)
    .bind(fx.owner)
    .execute(&fx.pool)
    .await?;
    let page = fx
        .notes()
        .list_notes(
            fx.stranger,
            fx.event,
            NoteListQuery::new(LANDMARK, None, None)?,
        )
        .await
        .unwrap();
    assert_eq!(page.visible_count, 0);
    let error = note_error(
        fx.notes()
            .note_thread(fx.stranger, fx.event, LANDMARK, expired, None, None)
            .await,
    );
    assert_eq!(error, PlaceNoteError::NotFound);
    assert_eq!(note_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn notes_moderator_hides_and_purges() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let created = fx
        .notes()
        .create_note(
            fx.owner,
            fx.event,
            note(NoteAudience::Everyone, "hide me"),
            Uuid::new_v4(),
        )
        .await
        .unwrap();
    fx.notes()
        .create_reply(
            fx.friend,
            fx.event,
            LANDMARK,
            created.id,
            CreatePlaceReply::new("reply body", created.revision)?,
            Uuid::new_v4(),
        )
        .await
        .unwrap();
    fx.notes()
        .hide_note(fx.admin, fx.event, created.id)
        .await
        .unwrap();
    let page = fx
        .notes()
        .list_notes(
            fx.friend,
            fx.event,
            NoteListQuery::new(LANDMARK, None, None)?,
        )
        .await
        .unwrap();
    assert_eq!(page.visible_count, 0);
    assert_eq!(
        note_error(
            fx.notes()
                .note_thread(fx.friend, fx.event, LANDMARK, created.id, None, None)
                .await
        ),
        PlaceNoteError::NotFound
    );
    let bodies: Vec<String> = sqlx::query_scalar(
        "select body from concert_place_notes where event_id=$1 order by created_at",
    )
    .bind(fx.event)
    .fetch_all(&fx.pool)
    .await?;
    assert_eq!(bodies, vec![String::new(), String::new()]);
    Ok(())
}

#[tokio::test]
async fn notes_meet_here_is_read_only_and_friend_scoped() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let created = fx
        .notes()
        .create_note(
            fx.owner,
            fx.event,
            note(NoteAudience::Everyone, "meet here"),
            Uuid::new_v4(),
        )
        .await
        .unwrap();
    let friend_page = fx
        .notes()
        .list_notes(
            fx.friend,
            fx.event,
            NoteListQuery::new(LANDMARK, None, None)?,
        )
        .await
        .unwrap();
    let entry = friend_page.items[0]
        .meet_here
        .as_ref()
        .expect("friend entry");
    assert_eq!(entry.recipient_user_id, fx.owner);
    assert_eq!(entry.landmark_id, LANDMARK);
    let stranger_page = fx
        .notes()
        .list_notes(
            fx.stranger,
            fx.event,
            NoteListQuery::new(LANDMARK, None, None)?,
        )
        .await
        .unwrap();
    assert!(stranger_page.items[0].meet_here.is_none());
    let writes: (i64, i64) = sqlx::query_as(
        "select
           (select count(*) from public.concert_venue_meeting_invitations where event_id=$1),
           (select count(*) from public.concert_place_notes where event_id=$1 and parent_id is not null)",
    )
    .bind(fx.event)
    .fetch_one(&fx.pool)
    .await?;
    assert_eq!(writes, (0, 0));
    assert_eq!(stranger_page.items[0].id, created.id);
    Ok(())
}

#[tokio::test]
async fn notes_unreviewed_landmark_denied() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let command = CreatePlaceNote::new(
        "unreviewed",
        NoteAudience::Everyone,
        NoteExpiry::OneHour,
        NoteLandmarkSelection::new("gate-unknown", 1, 1).unwrap(),
    )?;
    let error = note_error(
        fx.notes()
            .create_note(fx.owner, fx.event, command, Uuid::new_v4())
            .await,
    );
    assert_eq!(error, PlaceNoteError::NotFound);
    assert_eq!(note_status(&error), Status::NotFound);
    Ok(())
}

#[tokio::test]
async fn notes_block_removes_friends_note_immediately() -> anyhow::Result<()> {
    let fx = fixture().await?;
    let created = fx
        .notes()
        .create_note(
            fx.owner,
            fx.event,
            note(NoteAudience::Friends, PRIVATE_NOTE),
            Uuid::new_v4(),
        )
        .await
        .unwrap();
    let before = fx
        .notes()
        .list_notes(
            fx.friend,
            fx.event,
            NoteListQuery::new(LANDMARK, None, None)?,
        )
        .await
        .unwrap();
    assert_eq!(before.visible_count, 1);
    restriction(&fx, fx.friend, true, false).await;
    let after = fx
        .notes()
        .list_notes(
            fx.friend,
            fx.event,
            NoteListQuery::new(LANDMARK, None, None)?,
        )
        .await
        .unwrap();
    assert_eq!(after.visible_count, 0);
    assert_no_secret(&format!("{after:?}"), &[PRIVATE_NOTE.to_owned()]);
    assert_eq!(
        note_error(
            fx.notes()
                .note_thread(fx.friend, fx.event, LANDMARK, created.id, None, None)
                .await
        ),
        PlaceNoteError::NotFound
    );
    Ok(())
}

fn contract(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/api/paradise-city-v1")
        .join(format!("{name}.openapi.json"));
    serde_json::from_slice(&std::fs::read(path).expect("contract readable")).expect("contract json")
}

fn declared(doc: &Value, method: &str, path: &str) -> Vec<u16> {
    doc["paths"][path][method]["responses"]
        .as_object()
        .expect("responses")
        .keys()
        .filter_map(|key| key.parse::<u16>().ok())
        .collect()
}

#[test]
fn matrix_statuses_are_declared_by_frozen_contracts() {
    let location = contract("location");
    let meeting = contract("meeting");
    let notes = contract("place-notes");
    for (doc, method, path, status) in [
        (
            &location,
            "get",
            "/api/concert/events/{eventId}/venue/location",
            200u16,
        ),
        (
            &location,
            "get",
            "/api/concert/events/{eventId}/venue/location",
            404,
        ),
        (
            &location,
            "put",
            "/api/concert/events/{eventId}/venue/location/sharing",
            200,
        ),
        (
            &location,
            "put",
            "/api/concert/events/{eventId}/venue/location/fix",
            400,
        ),
        (
            &location,
            "put",
            "/api/concert/events/{eventId}/venue/location/fix",
            409,
        ),
        (
            &location,
            "get",
            "/api/concert/events/{eventId}/venue/location",
            503,
        ),
        (
            &meeting,
            "get",
            "/api/concert/events/{eventId}/venue-meeting/context",
            404,
        ),
        (
            &meeting,
            "get",
            "/api/concert/events/{eventId}/venue-meeting/context",
            409,
        ),
        (
            &meeting,
            "post",
            "/api/concert/events/{eventId}/venue-meeting/invitations",
            400,
        ),
        (
            &meeting,
            "post",
            "/api/concert/events/{eventId}/venue-meeting/invitations",
            503,
        ),
        (
            &meeting,
            "get",
            "/api/concert/events/{eventId}/venue-meeting/messages",
            404,
        ),
        (
            &notes,
            "get",
            "/api/concert/events/{eventId}/venue/landmarks/{landmarkId}/notes",
            403,
        ),
        (
            &notes,
            "get",
            "/api/concert/events/{eventId}/venue/landmarks/{landmarkId}/notes",
            404,
        ),
        (
            &notes,
            "post",
            "/api/concert/events/{eventId}/venue/landmarks/{landmarkId}/notes",
            409,
        ),
        (
            &notes,
            "get",
            "/api/concert/events/{eventId}/venue/landmarks/{landmarkId}/notes/{noteId}",
            404,
        ),
        (
            &notes,
            "post",
            "/api/concert/events/{eventId}/venue/landmarks/{landmarkId}/notes/{noteId}/replies",
            403,
        ),
        (
            &notes,
            "delete",
            "/api/concert/events/{eventId}/venue/landmarks/{landmarkId}/notes/{noteId}",
            403,
        ),
    ] {
        assert!(
            declared(doc, method, path).contains(&status),
            "{method} {path} must declare {status}"
        );
    }
    for (schema, keys) in [
        (
            &location["components"]["schemas"]["LocationError"],
            vec!["code", "message", "traceId", "timestamp", "details"],
        ),
        (
            &notes["components"]["schemas"]["NoteError"],
            vec!["code", "message", "traceId", "timestamp", "details"],
        ),
    ] {
        assert_eq!(schema["additionalProperties"], false);
        let required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        assert_eq!(required, keys);
        assert_eq!(
            schema["properties"]["details"]["additionalProperties"],
            false
        );
        assert_eq!(schema["properties"]["details"]["required"][0], "reason");
    }
    let meeting_error = &meeting["components"]["schemas"]["ErrorResponse"];
    assert_eq!(meeting_error["additionalProperties"], false);
    assert_eq!(
        meeting_error["properties"]["details"]["required"][0],
        "reason"
    );
    for (reason, schema) in [
        (
            "RESOURCE_UNAVAILABLE",
            &location["components"]["schemas"]["LocationError"],
        ),
        (
            "PERMISSION_DENIED",
            &notes["components"]["schemas"]["NoteError"],
        ),
        (
            "RESOURCE_UNAVAILABLE",
            &meeting["components"]["schemas"]["ErrorResponse"],
        ),
    ] {
        assert!(
            schema["properties"]["details"]["properties"]["reason"]["enum"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == reason),
            "reason {reason} must stay in the contract"
        );
    }
}

#[test]
fn meeting_route_admission_is_exact_and_closed() {
    use concert_service::product_rpc::{
        operation, venue_meeting_admission::MeetingOperation, Operation,
    };
    let event = Uuid::new_v4();
    let friend = Uuid::new_v4();
    let base = format!("/api/concert/events/{event}");
    assert_eq!(
        operation(
            event,
            "GET",
            &format!("{base}/venue-meeting/context?selectedFriendId={friend}")
        )
        .unwrap(),
        Operation::Meeting(MeetingOperation::Context)
    );
    assert_eq!(
        operation(event, "GET", &format!("{base}/venue/location"))
            .unwrap_err()
            .code(),
        Code::PermissionDenied,
        "product RPC refuses venue location/notes; HTTP authorization is proven at the application layer only"
    );
    assert_eq!(
        operation(
            event,
            "GET",
            &format!("{base}/venue-meeting/invitations/not-a-uuid")
        )
        .unwrap_err()
        .code(),
        Code::PermissionDenied
    );
}

#[test]
fn invitation_state_values_are_the_frozen_ones() {
    for state in [
        InvitationState::Pending,
        InvitationState::Accepted,
        InvitationState::Declined,
        InvitationState::Cancelled,
        InvitationState::Expired,
    ] {
        assert_eq!(InvitationState::parse(state.as_str()).unwrap(), state);
    }
}
