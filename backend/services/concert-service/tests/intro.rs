//! BE27 native PostgreSQL introduction persistence, authority and pagination.
//! Every authority and profile row below is a synthetic fixture, not production.
#[path = "intro_support/owned.rs"]
mod owned;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::commands::{FingerprintKeys, SigningKey};
use concert_application::intro::{self as app, cursor};
use concert_domain::intro_policy::IntroState;
use concert_domain::{AuthUserId, EventId, Revision};
use concert_service::intro_adapter::{self as repo, IntroPolicy};
use repo::{IntroProfileProjection, IntroProjectionExecutor, PgIntro};
use serde_json::{json, Value};
use sqlx::{PgConnection, PgPool};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use uuid::Uuid;

fn keys() -> std::sync::Arc<FingerprintKeys> {
    std::sync::Arc::new(
        FingerprintKeys::new(SigningKey::new("be27-native", &[73; 32]).unwrap(), vec![]).unwrap(),
    )
}

#[derive(Clone, Copy)]
struct Fixture {
    event: Uuid,
    actor: Uuid,
    other: Uuid,
    foreign_event: Uuid,
    foreign_actor: Uuid,
}
impl Fixture {
    fn event_id(self) -> EventId {
        EventId::from_uuid(self.event)
    }
    fn actor_id(self) -> AuthUserId {
        AuthUserId::from_uuid(self.actor)
    }
    fn context(self) -> app::IntroReadContext {
        app::IntroReadContext {
            actor: self.actor_id(),
            event: self.event_id(),
            session: Uuid::new_v4(),
            request: Uuid::new_v4(),
            expires_at: Utc::now() + Duration::minutes(5),
        }
    }
}

fn profile(user: Uuid, name: &str) -> Value {
    json!({
        "user_id": user,
        "display_name": name,
        "bio": null,
        "avatar": {
            "revision": 1,
            "config": {
                "contract_version": "1.0.0",
                "kind": "avatar_config",
                "manifest_id": "avatar_fixture",
                "manifest_version": 1,
                "item_ids": {
                    "body": "body_fixture",
                    "eyes": "eyes_fixture",
                    "hair": "hair_fixture",
                    "outfit": "outfit_fixture",
                    "accessory": "accessory_fixture"
                },
                "skin_tone_id": "tone_fixture"
            }
        }
    })
}

async fn fixture(pool: &PgPool) -> anyhow::Result<Fixture> {
    for table in [
        "create table if not exists intro_test_authority(
           event_id uuid not null, actor_id uuid not null, operation_id text not null,
           allowed boolean not null, primary key(event_id,actor_id,operation_id))",
        "create table if not exists intro_test_profiles(
           event_id uuid not null, user_id uuid not null, profile jsonb not null,
           primary key(event_id,user_id))",
        "create table if not exists intro_test_deadline(
           event_id uuid not null primary key, valid_until timestamptz not null)",
    ] {
        sqlx::query(table).execute(pool).await?;
    }
    let f = Fixture {
        event: Uuid::new_v4(),
        actor: Uuid::new_v4(),
        other: Uuid::new_v4(),
        foreign_event: Uuid::new_v4(),
        foreign_actor: Uuid::new_v4(),
    };
    for (event, label) in [(f.event, "intro-main"), (f.foreign_event, "intro-foreign")] {
        let slug = format!("{}-{}", label, event.simple());
        sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Introduction test','LIVE')")
            .bind(event)
            .bind(slug)
            .execute(pool)
            .await?;
        sqlx::query(
            "insert into concert_meeting_settings(event_id,event_cutoff) values($1,clock_timestamp()+interval '1 day')",
        )
        .bind(event)
        .execute(pool)
        .await?;
        sqlx::query(
            "insert into intro_test_deadline values($1,clock_timestamp()+interval '5 minutes')",
        )
        .bind(event)
        .execute(pool)
        .await?;
    }
    for (event, user, name) in [
        (f.event, f.actor, "Alice fixture"),
        (f.event, f.other, "Briar fixture"),
        (f.event, f.foreign_actor, "Third fixture"),
        (f.foreign_event, f.foreign_actor, "Foreign fixture"),
    ] {
        sqlx::query("insert into intro_test_profiles values($1,$2,$3)")
            .bind(event)
            .bind(user)
            .bind(profile(user, name))
            .execute(pool)
            .await?;
    }
    for operation in [
        "setConcertIntro",
        "publishConcertIntro",
        "closeConcertIntro",
        "listConcertIntros",
        "getOwnConcertIntro",
    ] {
        for (event, actor) in [
            (f.event, f.actor),
            (f.event, f.other),
            (f.foreign_event, f.foreign_actor),
        ] {
            sqlx::query("insert into intro_test_authority values($1,$2,$3,true)")
                .bind(event)
                .bind(actor)
                .bind(String::from(operation))
                .execute(pool)
                .await?;
        }
    }
    Ok(f)
}

async fn seed_cutoff_event(pool: &PgPool, event: Uuid, actor: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        "insert into concert_events(event_id,slug,title,phase) values($1,$2,'Cutoff test','LIVE')",
    )
    .bind(event)
    .bind(format!("intro-cutoff-{}", event.simple()))
    .execute(pool)
    .await?;
    sqlx::query(
        "insert into concert_meeting_settings(event_id,event_cutoff) values($1,clock_timestamp()+interval '1 day')",
    )
    .bind(event)
    .execute(pool)
    .await?;
    for operation in [
        "setConcertIntro",
        "publishConcertIntro",
        "closeConcertIntro",
        "listConcertIntros",
        "getOwnConcertIntro",
    ] {
        sqlx::query("insert into intro_test_authority values($1,$2,$3,true)")
            .bind(event)
            .bind(actor)
            .bind(operation)
            .execute(pool)
            .await?;
    }
    Ok(())
}

struct Policy {
    fail_edit: AtomicBool,
}
#[async_trait]
impl IntroPolicy for Policy {
    async fn check(
        &self,
        c: &mut PgConnection,
        command: &app::IntroCommand,
    ) -> Result<(), app::IntroError> {
        let allowed: Option<bool> = sqlx::query_scalar(
            "select allowed from intro_test_authority where event_id=$1 and actor_id=$2 and operation_id=$3 for share",
        )
        .bind(command.event().uuid())
        .bind(command.actor().uuid())
        .bind(command.operation())
        .fetch_optional(&mut *c)
        .await
        .map_err(|_| app::IntroError::Unavailable)?;
        if allowed != Some(true) {
            return Err(app::IntroError::Denied);
        }
        sqlx::query("select set_config('mcr.intro_authority_until',$1,true)")
            .bind("2030-01-01T00:00:00Z")
            .execute(&mut *c)
            .await
            .map_err(|_| app::IntroError::Unavailable)?;
        Ok(())
    }
    async fn edit_consent(
        &self,
        _: &mut PgConnection,
        _: &app::IntroCommand,
        _: &app::OwnerIntro,
    ) -> Result<(), app::IntroError> {
        if self.fail_edit.load(Ordering::SeqCst) {
            Err(app::IntroError::Unavailable)
        } else {
            Ok(())
        }
    }
}

struct Profiles;
impl IntroProfileProjection for Profiles {
    async fn author(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        owner: Uuid,
    ) -> Result<app::PublicAuthor, app::IntroError> {
        let value: Option<Value> = sqlx::query_scalar(
            "select profile from intro_test_profiles where event_id=$1 and user_id=$2 for share",
        )
        .bind(event)
        .bind(owner)
        .fetch_optional(&mut *c)
        .await
        .map_err(|_| app::IntroError::Unavailable)?;
        Ok(app::PublicAuthor {
            user_id: AuthUserId::from_uuid(owner),
            json: serde_json::to_vec(&value.ok_or(app::IntroError::Unavailable)?)
                .map_err(|_| app::IntroError::Unavailable)?,
        })
    }
}

struct Projection {
    blocked: Mutex<HashSet<Uuid>>,
    fail: AtomicBool,
    collections: AtomicUsize,
    acknowledgements: AtomicUsize,
}
impl Projection {
    fn new() -> Self {
        Self {
            blocked: Mutex::new(HashSet::new()),
            fail: AtomicBool::new(false),
            collections: AtomicUsize::new(0),
            acknowledgements: AtomicUsize::new(0),
        }
    }
    fn block(&self, user: Uuid) {
        self.blocked.lock().unwrap().insert(user);
    }
}
impl IntroProjectionExecutor for Projection {
    async fn check_collection(
        &self,
        c: &mut PgConnection,
        context: app::IntroReadContext,
        _: DateTime<Utc>,
    ) -> Result<(), app::IntroError> {
        self.collections.fetch_add(1, Ordering::SeqCst);
        let allowed: Option<bool> = sqlx::query_scalar(
            "select allowed from intro_test_authority where event_id=$1 and actor_id=$2 and operation_id='listConcertIntros' for share",
        )
        .bind(context.event.uuid())
        .bind(context.actor.uuid())
        .fetch_optional(&mut *c)
        .await
        .map_err(|_| app::IntroError::Unavailable)?;
        if allowed == Some(true) {
            Ok(())
        } else {
            Err(app::IntroError::Denied)
        }
    }
    async fn check_owner(
        &self,
        c: &mut PgConnection,
        context: app::IntroReadContext,
        _: DateTime<Utc>,
    ) -> Result<(), app::IntroError> {
        self.check_collection(c, context, Utc::now()).await
    }
    async fn project_public(
        &self,
        _: &mut PgConnection,
        _: app::IntroReadContext,
        row: &app::PublicForumIntro,
        _: DateTime<Utc>,
    ) -> Result<(), app::IntroError> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(app::IntroError::Unavailable);
        }
        if self.blocked.lock().unwrap().contains(&row.author.uuid()) {
            Err(app::IntroError::Denied)
        } else {
            Ok(())
        }
    }
    async fn acknowledge(
        &self,
        _: &mut PgConnection,
        _: app::IntroReadContext,
        _: DateTime<Utc>,
    ) -> Result<(), app::IntroError> {
        self.acknowledgements.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) {
            Err(app::IntroError::Unavailable)
        } else {
            Ok(())
        }
    }
}

struct Codec([u8; 32]);
impl cursor::CursorCodec for Codec {
    fn seal(
        &self,
        binding: &cursor::CursorBinding,
        window: cursor::CursorWindow,
        now: DateTime<Utc>,
    ) -> Result<String, app::IntroError> {
        cursor::encode_authenticated(&self.0, binding, window, now)
    }
    fn open(
        &self,
        binding: &cursor::CursorBinding,
        token: &str,
        now: DateTime<Utc>,
    ) -> Result<cursor::CursorWindow, app::IntroError> {
        cursor::decode_authenticated(&self.0, binding, token, now)
    }
}

type Service = PgIntro<Policy, Profiles, Codec, Projection>;
fn service(pool: PgPool) -> Service {
    PgIntro::with_executor(
        pool,
        Policy {
            fail_edit: AtomicBool::new(false),
        },
        Profiles,
        Codec([81; 32]),
        Projection::new(),
        keys(),
    )
}
fn draft(text: &str) -> app::IntroDraft {
    app::IntroDraft {
        text: text.into(),
        age_bounds: "null:null".into(),
        expires_at: None,
    }
}
async fn counts(pool: &PgPool, event: Uuid) -> anyhow::Result<(i64, i64, i64)> {
    Ok((
        sqlx::query_scalar("select count(*) from concert_intros where event_id=$1")
            .bind(event)
            .fetch_one(pool)
            .await?,
        sqlx::query_scalar("select count(*) from concert_command_receipts where event_id=$1")
            .bind(event)
            .fetch_one(pool)
            .await?,
        sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
            .bind(event)
            .fetch_one(pool)
            .await?,
    ))
}
async fn publish(
    pool: &PgPool,
    event: Uuid,
    user: Uuid,
    id: Uuid,
    text: &str,
    created_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
) -> anyhow::Result<()> {
    sqlx::query_scalar::<_, Uuid>(
        "insert into concert_intros(event_id,intro_id,owner_id,revision,state,text,created_at,updated_at,expires_at) values($1,$2,$3,1,'PUBLISHED',$4,$5,clock_timestamp(),$6) returning owner_id",
    )
    .bind(event)
    .bind(id)
    .bind(user)
    .bind(text)
    .bind(created_at)
    .bind(expires_at)
    .fetch_one(pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn receipts_revision_conflicts_and_invalidation_failure_roll_back() -> anyhow::Result<()> {
    let pool = owned::owned_pool().await?;
    let f = fixture(&pool).await?;
    let s = service(pool.clone());
    let key = Uuid::new_v4();
    let command = app::IntroCommand::set(f.actor_id(), f.event_id(), key, 0, draft(""))?;
    let first = s.execute(&command).await?;
    let duplicate = s.execute(&command).await?;
    assert_eq!(first.receipt_id, duplicate.receipt_id);
    assert_eq!(first.revision, Some(Revision::initial()));
    assert_eq!(counts(&pool, f.event).await?, (1, 1, 1));
    let changed = app::IntroCommand::set(f.actor_id(), f.event_id(), key, 0, draft("changed"))?;
    assert_eq!(
        s.execute(&changed).await.err(),
        Some(app::IntroError::Conflict)
    );
    let stale_key = Uuid::new_v4();
    assert_eq!(
        s.execute(&app::IntroCommand::set(
            f.actor_id(),
            f.event_id(),
            stale_key,
            0,
            draft("stale"),
        )?)
        .await
        .err(),
        Some(app::IntroError::Conflict)
    );

    sqlx::query("insert into intro_test_authority values($1,$2,'setConcertIntro',true) on conflict do nothing")
        .bind(f.event)
        .bind(f.actor)
        .execute(&pool)
        .await?;
    let failing = PgIntro::with_executor(
        pool.clone(),
        Policy {
            fail_edit: AtomicBool::new(true),
        },
        Profiles,
        Codec([81; 32]),
        Projection::new(),
        keys(),
    );
    let before = counts(&pool, f.event).await?;
    sqlx::query("insert into intro_test_authority values($1,$2,'setConcertIntro',true) on conflict do nothing")
        .bind(f.event)
        .bind(f.actor)
        .execute(&pool)
        .await?;
    assert_eq!(
        failing
            .execute(&app::IntroCommand::set(
                f.actor_id(),
                f.event_id(),
                Uuid::new_v4(),
                1,
                draft("must rollback")
            )?)
            .await
            .err(),
        Some(app::IntroError::Unavailable)
    );
    assert_eq!(counts(&pool, f.event).await?, before);
    Ok(())
}

#[tokio::test]
async fn empty_publish_close_expiry_and_event_isolation() -> anyhow::Result<()> {
    let pool = owned::owned_pool().await?;
    let f = fixture(&pool).await?;
    let s = service(pool.clone());
    let created = s
        .execute(&app::IntroCommand::set(
            f.actor_id(),
            f.event_id(),
            Uuid::new_v4(),
            0,
            draft("   "),
        )?)
        .await?;
    let publish_key = Uuid::new_v4();
    assert_eq!(
        s.execute(&app::IntroCommand::publish(
            f.actor_id(),
            f.event_id(),
            publish_key,
            Revision::initial(),
        ))
        .await
        .err(),
        Some(app::IntroError::Conflict)
    );
    let valid = s
        .execute(&app::IntroCommand::set(
            f.actor_id(),
            f.event_id(),
            Uuid::new_v4(),
            1,
            draft("Alice is here"),
        )?)
        .await?;
    assert!(valid.owner.state == IntroState::Draft);
    let published = s
        .execute(&app::IntroCommand::publish(
            f.actor_id(),
            f.event_id(),
            publish_key,
            Revision::new(2)?,
        ))
        .await?;
    assert!(published.owner.state == IntroState::Published);
    let (_, next) = s
        .discover(f.context(), app::wire::Query::validate(None, None, None)?)
        .await?;
    assert!(next.is_none());

    let expired = Uuid::new_v4();
    publish(
        &pool,
        f.event,
        f.other,
        expired,
        "Already expired",
        Utc::now() - Duration::seconds(2),
        Some(Utc::now() - Duration::seconds(1)),
    )
    .await?;
    let (items, _) = s
        .discover(f.context(), app::wire::Query::validate(None, None, None)?)
        .await?;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0.intro_id, created.owner.intro_id);
    assert_eq!(
        s.direct(f.context(), expired).await.err(),
        Some(app::IntroError::NotFound)
    );
    let closed = s
        .execute(&app::IntroCommand::close(
            f.actor_id(),
            f.event_id(),
            Uuid::new_v4(),
            Revision::new(3)?,
        ))
        .await?;
    assert!(closed.owner.state == IntroState::Closed);
    assert_eq!(
        s.direct(f.context(), closed.owner.intro_id).await.err(),
        Some(app::IntroError::NotFound)
    );

    let cutoff_event = Uuid::new_v4();
    let cutoff_actor = Uuid::new_v4();
    seed_cutoff_event(&pool, cutoff_event, cutoff_actor).await?;
    let cutoff: DateTime<Utc> =
        sqlx::query_scalar("select event_cutoff from concert_meeting_settings where event_id=$1")
            .bind(cutoff_event)
            .fetch_one(&pool)
            .await?;
    let accepted = s
        .execute(&app::IntroCommand::set(
            AuthUserId::from_uuid(cutoff_actor),
            EventId::from_uuid(cutoff_event),
            Uuid::new_v4(),
            0,
            app::IntroDraft {
                text: "Within cutoff".into(),
                age_bounds: "null:null".into(),
                expires_at: Some(cutoff - Duration::seconds(1)),
            },
        )?)
        .await?;
    assert_eq!(
        accepted.owner.expires_at,
        Some(cutoff - Duration::seconds(1))
    );
    assert_eq!(
        s.execute(&app::IntroCommand::set(
            AuthUserId::from_uuid(cutoff_actor),
            EventId::from_uuid(cutoff_event),
            Uuid::new_v4(),
            1,
            app::IntroDraft {
                text: "Beyond cutoff".into(),
                age_bounds: "null:null".into(),
                expires_at: Some(cutoff + Duration::seconds(1)),
            },
        )?)
        .await
        .err(),
        Some(app::IntroError::Invalid)
    );
    sqlx::query(
        "update concert_meeting_settings set event_cutoff=clock_timestamp() where event_id=$1",
    )
    .bind(cutoff_event)
    .execute(&pool)
    .await?;
    assert_eq!(
        s.execute(&app::IntroCommand::set(
            AuthUserId::from_uuid(cutoff_actor),
            EventId::from_uuid(cutoff_event),
            Uuid::new_v4(),
            1,
            draft("Cutoff closed"),
        )?)
        .await
        .err(),
        Some(app::IntroError::Denied)
    );
    let closed_after_cutoff = s
        .execute(&app::IntroCommand::close(
            AuthUserId::from_uuid(cutoff_actor),
            EventId::from_uuid(cutoff_event),
            Uuid::new_v4(),
            Revision::initial(),
        ))
        .await?;
    assert!(closed_after_cutoff.owner.state == IntroState::Closed);

    let foreign_context = app::IntroReadContext {
        actor: AuthUserId::from_uuid(f.foreign_actor),
        event: EventId::from_uuid(f.foreign_event),
        ..f.context()
    };
    let (foreign, _) = s
        .discover(
            foreign_context,
            app::wire::Query::validate(None, None, None)?,
        )
        .await?;
    assert!(foreign.is_empty());
    let mut denied = f.context();
    denied.event = EventId::from_uuid(f.foreign_event);
    assert_eq!(
        s.discover(denied, app::wire::Query::validate(None, None, None)?)
            .await
            .err(),
        Some(app::IntroError::Denied)
    );
    Ok(())
}

#[tokio::test]
async fn pagination_filter_profile_and_projection_fail_closed() -> anyhow::Result<()> {
    let pool = owned::owned_pool().await?;
    let f = fixture(&pool).await?;
    let s = service(pool.clone());
    let base = Utc::now() - Duration::minutes(2);
    let actor_id = app::IntroCommand::set(
        AuthUserId::from_uuid(f.actor),
        EventId::from_uuid(f.event),
        Uuid::new_v4(),
        0,
        draft("Alice one"),
    )?
    .intro();
    let other_id = app::IntroCommand::set(
        AuthUserId::from_uuid(f.other),
        EventId::from_uuid(f.event),
        Uuid::new_v4(),
        0,
        draft("Briar three"),
    )?
    .intro();
    let third_id = app::IntroCommand::set(
        AuthUserId::from_uuid(f.foreign_actor),
        EventId::from_uuid(f.event),
        Uuid::new_v4(),
        0,
        draft("Briar three"),
    )?
    .intro();
    let ids = [actor_id, other_id, third_id];
    let texts = ["Alice one", "Alice two", "Briar three"];
    for (i, id) in ids.into_iter().enumerate() {
        publish(
            &pool,
            f.event,
            match i {
                1 => f.other,
                2 => f.foreign_actor,
                _ => f.actor,
            },
            id,
            texts[i],
            base + Duration::seconds(i as i64),
            None,
        )
        .await?;
    }
    let (first, next) = s
        .discover(
            f.context(),
            app::wire::Query::validate(Some(1), None, None)?,
        )
        .await
        .map_err(|error| anyhow::anyhow!("first discover: {error:?}"))?;
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].0.intro_id, ids[2]);
    let token = next.expect("continuation over the same scan bound");
    let (second, next) = s
        .discover(
            f.context(),
            app::wire::Query::validate(Some(1), Some(token.clone()), None)?,
        )
        .await
        .map_err(|error| anyhow::anyhow!("second discover: {error:?}"))?;
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].0.intro_id, ids[1]);
    assert!(next.is_some());
    let (third, next) = s
        .discover(
            f.context(),
            app::wire::Query::validate(Some(1), next, None)?,
        )
        .await?;
    assert_eq!(third[0].0.intro_id, ids[0]);
    assert!(next.is_none());
    assert_eq!(
        s.discover(
            app::IntroReadContext {
                actor: AuthUserId::from_uuid(f.other),
                ..f.context()
            },
            app::wire::Query::validate(Some(1), Some(token.clone()), None)?,
        )
        .await
        .err(),
        Some(app::IntroError::Invalid)
    );
    let mismatched_cursor = s
        .discover(
            f.context(),
            app::wire::Query::validate(Some(1), Some(token), Some("alice".into()))?,
        )
        .await;
    assert_eq!(mismatched_cursor.err(), Some(app::IntroError::Invalid));
    let (filtered, next) = s
        .discover(
            f.context(),
            app::wire::Query::validate(Some(1), None, Some("ALICE".into()))?,
        )
        .await?;
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].0.intro_id, ids[1]);
    assert!(
        next.is_some(),
        "a filtered page must continue at its last scan position"
    );
    let (filtered_rest, _) = s
        .discover(
            f.context(),
            app::wire::Query::validate(Some(10), next.clone(), Some("ALICE".into()))?,
        )
        .await?;
    assert_eq!(filtered_rest.len(), 1);
    assert_eq!(filtered_rest[0].0.intro_id, ids[0]);
    let first_length = first.len();
    let page = repo::public_page_dto(first, None)?;
    assert!(page["items"][0]["author"]["display_name"] == "Third fixture");
    for forbidden in [
        "minimum_age",
        "maximum_age_exclusive",
        "age_at_verification",
        "next_birthday_at",
        "verifier_id",
        "source_revision",
        "date_of_birth",
        "token",
    ] {
        assert!(!page.to_string().contains(forbidden));
    }

    let blocked = Projection::new();
    blocked.block(f.other);
    let blocked_service = PgIntro::with_executor(
        pool.clone(),
        Policy {
            fail_edit: AtomicBool::new(false),
        },
        Profiles,
        Codec([81; 32]),
        blocked,
        keys(),
    );
    let (visible, _) = blocked_service
        .discover(f.context(), app::wire::Query::validate(None, None, None)?)
        .await?;
    assert_eq!(visible.len(), 2);
    assert_eq!(
        blocked_service.direct(f.context(), ids[1]).await.err(),
        Some(app::IntroError::NotFound)
    );
    let relation_block_if_public = repo::public_dto(&visible[0].0, visible[0].1.clone())?;
    assert!(relation_block_if_public.get("minimum_age").is_none());
    assert_eq!(first_length, 1);
    Ok(())
}

#[tokio::test]
async fn foreign_profile_and_projection_failure_return_no_value() -> anyhow::Result<()> {
    let pool = owned::owned_pool().await?;
    let f = fixture(&pool).await?;
    let id = Uuid::new_v4();
    publish(
        &pool,
        f.event,
        f.actor,
        id,
        "Profile failure",
        Utc::now() - Duration::seconds(1),
        None,
    )
    .await?;
    sqlx::query("update intro_test_profiles set profile=$3 where event_id=$1 and user_id=$2")
        .bind(f.event)
        .bind(f.actor)
        .bind(profile(f.other, "Foreign"))
        .execute(&pool)
        .await?;
    let s = service(pool.clone());
    assert_eq!(
        s.direct(f.context(), id).await.err(),
        Some(app::IntroError::Unavailable)
    );
    sqlx::query("update intro_test_profiles set profile=$3 where event_id=$1 and user_id=$2")
        .bind(f.event)
        .bind(f.actor)
        .bind(profile(f.actor, "Alice fixture"))
        .execute(&pool)
        .await?;
    let projection = Projection::new();
    projection.fail.store(true, Ordering::SeqCst);
    let failing = PgIntro::with_executor(
        pool,
        Policy {
            fail_edit: AtomicBool::new(false),
        },
        Profiles,
        Codec([81; 32]),
        projection,
        keys(),
    );
    assert_eq!(
        failing.direct(f.context(), id).await.err(),
        Some(app::IntroError::Unavailable)
    );
    assert_eq!(
        failing
            .discover(f.context(), app::wire::Query::validate(None, None, None)?)
            .await
            .err(),
        Some(app::IntroError::Unavailable)
    );
    Ok(())
}
