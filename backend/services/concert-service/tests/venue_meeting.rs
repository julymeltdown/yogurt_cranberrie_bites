#[path = "support/meeting/mod.rs"]
mod support;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::venue_meeting::{
    CreateInvitation, DecideInvitation, LandmarkReview, PairFacts, SendMessage,
    VenueMeetingAuthority,
};
use concert_domain::venue_meeting::{
    Decision, InvitationState, LandmarkSelection, MeetingError, MeetingPolicy, MeetingSelection,
};
use concert_service::venue_meeting::PgVenueMeeting;
use sqlx::{PgPool, Row};
use uuid::Uuid;

struct FixtureAuthority {
    pool: PgPool,
}

async fn fixture_tables(pool: &PgPool) -> anyhow::Result<()> {
    for statement in [
        "drop table if exists venue_meeting_fixture_friends",
        "drop table if exists venue_meeting_fixture_blocks",
        "drop table if exists venue_meeting_fixture_landmarks",
        "drop table if exists venue_meeting_fixture_policy",
        "create table venue_meeting_fixture_friends(event_id uuid not null,user_low uuid not null,user_high uuid not null,relationship_revision bigint not null,primary key(event_id,user_low,user_high))",
        "create table venue_meeting_fixture_blocks(event_id uuid not null,blocker uuid not null,blocked uuid not null,primary key(event_id,blocker,blocked))",
        "create table venue_meeting_fixture_landmarks(event_id uuid not null,landmark_id text not null,catalog_revision bigint not null,landmark_revision bigint not null,can_meet boolean not null,valid_until timestamptz not null,primary key(event_id,landmark_id,catalog_revision,landmark_revision))",
        "create table venue_meeting_fixture_policy(event_id uuid primary key,policy_revision bigint not null,valid_until timestamptz not null,invitation_ms bigint not null,content_ms bigint not null,purge_ms bigint not null,timezone text not null)",
    ] {
        sqlx::query(statement).execute(pool).await?;
    }
    Ok(())
}

#[async_trait]
impl VenueMeetingAuthority for FixtureAuthority {
    async fn friend(&self, event: Uuid, actor: Uuid, peer: Uuid) -> Result<PairFacts, MeetingError> {
        if actor == peer {
            return Err(MeetingError::InvalidInput);
        }
        let (low, high) = if actor < peer { (actor, peer) } else { (peer, actor) };
        let blocked: bool = sqlx::query_scalar(
            "select exists(select 1 from venue_meeting_fixture_blocks
              where event_id=$1 and ((blocker=$2 and blocked=$3) or (blocker=$3 and blocked=$2)))",
        )
        .bind(event)
        .bind(actor)
        .bind(peer)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| MeetingError::PolicyUnavailable)?;
        if blocked {
            return Err(MeetingError::ResourceUnavailable);
        }
        let revision: Option<i64> = sqlx::query_scalar(
            "select relationship_revision from venue_meeting_fixture_friends
             where event_id=$1 and user_low=$2 and user_high=$3",
        )
        .bind(event)
        .bind(low)
        .bind(high)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| MeetingError::PolicyUnavailable)?;
        revision
            .map(|relationship_revision| PairFacts {
                relationship_revision,
            })
            .ok_or(MeetingError::ResourceUnavailable)
    }

    async fn reviewed_landmark(
        &self,
        event: Uuid,
        selection: &LandmarkSelection,
    ) -> Result<LandmarkReview, MeetingError> {
        let row = sqlx::query(
            "select can_meet,valid_until from venue_meeting_fixture_landmarks
             where event_id=$1 and landmark_id=$2 and catalog_revision=$3 and landmark_revision=$4",
        )
        .bind(event)
        .bind(&selection.landmark_id)
        .bind(selection.catalog_revision)
        .bind(selection.landmark_revision)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| MeetingError::PolicyUnavailable)?
        .ok_or(MeetingError::ResourceUnavailable)?;
        let can_meet: bool = row.try_get("can_meet").map_err(|_| MeetingError::PolicyUnavailable)?;
        let valid_until: DateTime<Utc> = row
            .try_get("valid_until")
            .map_err(|_| MeetingError::PolicyUnavailable)?;
        if !can_meet || valid_until <= Utc::now() {
            return Err(MeetingError::ResourceUnavailable);
        }
        Ok(LandmarkReview {
            catalog_revision: selection.catalog_revision,
            landmark_revision: selection.landmark_revision,
            can_meet,
            valid_until,
        })
    }

    async fn policy(&self, event: Uuid) -> Result<MeetingPolicy, MeetingError> {
        let row = sqlx::query("select * from venue_meeting_fixture_policy where event_id=$1")
            .bind(event)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| MeetingError::PolicyUnavailable)?
            .ok_or(MeetingError::PolicyUnavailable)?;
        Ok(MeetingPolicy {
            policy_revision: row.try_get("policy_revision").map_err(|_| MeetingError::PolicyUnavailable)?,
            valid_until: row.try_get("valid_until").map_err(|_| MeetingError::PolicyUnavailable)?,
            invitation_ms: row.try_get("invitation_ms").map_err(|_| MeetingError::PolicyUnavailable)?,
            content_ms: row.try_get("content_ms").map_err(|_| MeetingError::PolicyUnavailable)?,
            purge_ms: row.try_get("purge_ms").map_err(|_| MeetingError::PolicyUnavailable)?,
            event_timezone: row.try_get("timezone").map_err(|_| MeetingError::PolicyUnavailable)?,
        })
    }
}

struct Fixture {
    event: Uuid,
    alice: Uuid,
    bob: Uuid,
    outsider: Uuid,
    landmark: LandmarkSelection,
}

async fn setup(pool: &PgPool) -> anyhow::Result<Fixture> {
    fixture_tables(pool).await?;
    let event = Uuid::new_v4();
    let alice = Uuid::new_v4();
    let bob = Uuid::new_v4();
    let outsider = Uuid::new_v4();
    sqlx::query(
        "insert into concert_events(event_id,slug,title,phase) values($1,$2,'Meeting fixture','OPEN')",
    )
    .bind(event)
    .bind(format!("venue-meeting-{event}"))
    .execute(pool)
    .await?;
    let now = Utc::now();
    sqlx::query(
        "insert into venue_meeting_fixture_policy
         (event_id,policy_revision,valid_until,invitation_ms,content_ms,purge_ms,timezone)
         values($1,7,$2,300000,600000,1200000,'Asia/Seoul')",
    )
    .bind(event)
    .bind(now + Duration::hours(1))
    .execute(pool)
    .await?;
    sqlx::query(
        "insert into venue_meeting_fixture_friends(event_id,user_low,user_high,relationship_revision)
         values($1,$2,$3,11)",
    )
    .bind(event)
    .bind(alice.min(bob))
    .bind(alice.max(bob))
    .execute(pool)
    .await?;
    sqlx::query(
        "insert into venue_meeting_fixture_landmarks
         (event_id,landmark_id,catalog_revision,landmark_revision,can_meet,valid_until)
         values($1,'gate-a',3,4,true,$2)",
    )
    .bind(event)
    .bind(now + Duration::minutes(30))
    .execute(pool)
    .await?;
    Ok(Fixture {
        event,
        alice,
        bob,
        outsider,
        landmark: LandmarkSelection::new("gate-a", 3, 4)?,
    })
}

fn actor_selection(f: &Fixture, actor: Uuid, landmark: Option<LandmarkSelection>) -> MeetingSelection {
    MeetingSelection::new(if actor == f.alice { f.bob } else { f.alice }, landmark).unwrap()
}

async fn block(pool: &PgPool, f: &Fixture) -> anyhow::Result<()> {
    sqlx::query("insert into venue_meeting_fixture_blocks(event_id,blocker,blocked) values($1,$2,$3)")
        .bind(f.event)
        .bind(f.bob)
        .bind(f.alice)
        .execute(pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn invitation_accept_message_round_trip_and_idempotent_replay() -> anyhow::Result<()> {
    let pool = support::owned_pool().await?;
    let f = setup(&pool).await?;
    let client = PgVenueMeeting::new(pool.clone(), FixtureAuthority { pool: pool.clone() });
    let create = CreateInvitation::new(actor_selection(&f, f.alice, Some(f.landmark.clone())))?;
    let key = Uuid::new_v4();
    let receipt = client
        .create_invitation(f.event, f.alice, key, &create)
        .await?;
    assert_eq!(receipt["state"], "APPLIED");
    assert_eq!(receipt["result"]["kind"], "INVITATION");
    assert_eq!(receipt["result"]["invitation"]["state"], "PENDING");
    let invitation_id = Uuid::parse_str(
        receipt["result"]["invitation"]["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("invitation id"))?,
    )?;
    let replay = client
        .create_invitation(f.event, f.alice, key, &create)
        .await?;
    assert_eq!(replay["result"], receipt["result"]);
    assert_eq!(replay["replayed"], true);

    let accept = DecideInvitation::new(
        actor_selection(&f, f.bob, Some(f.landmark.clone())),
        1,
        Decision::Accept,
    )?;
    let accept_key = Uuid::new_v4();
    let accepted = client
        .decide(f.event, f.bob, invitation_id, accept_key, &accept)
        .await?;
    assert_eq!(accepted["result"]["invitation"]["state"], "ACCEPTED");
    let accept_replay = client
        .decide(f.event, f.bob, invitation_id, accept_key, &accept)
        .await?;
    assert_eq!(accept_replay["result"], accepted["result"]);
    assert_eq!(accept_replay["replayed"], true);

    let send = SendMessage::new(actor_selection(&f, f.alice, None), "hello\nbob", None)?;
    let sent = client
        .send(f.event, f.alice, Uuid::new_v4(), &send)
        .await?;
    assert_eq!(sent["result"]["kind"], "MESSAGE");
    let page = client
        .messages(f.event, f.bob, &actor_selection(&f, f.bob, None), 0)
        .await?;
    assert_eq!(page["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(page["items"][0]["body"], "hello\nbob");

    block(&pool, &f).await?;
    let blocked = client
        .create_invitation(f.event, f.alice, Uuid::new_v4(), &create)
        .await;
    assert_eq!(blocked, Err(MeetingError::ResourceUnavailable));
    let blocked_send = client
        .send(f.event, f.alice, Uuid::new_v4(), &send)
        .await;
    assert_eq!(blocked_send, Err(MeetingError::ResourceUnavailable));

    let stranger = SendMessage::new(
        MeetingSelection::new(f.alice, None)?,
        "not a friend",
        None,
    )?;
    assert_eq!(
        client
            .send(f.event, f.outsider, Uuid::new_v4(), &stranger)
            .await,
        Err(MeetingError::ResourceUnavailable)
    );
    Ok(())
}

#[tokio::test]
async fn decisions_follow_producer_and_revision_and_stale_landmarks_fail_closed() -> anyhow::Result<()> {
    let pool = support::owned_pool().await?;
    let f = setup(&pool).await?;
    let client = PgVenueMeeting::new(pool.clone(), FixtureAuthority { pool: pool.clone() });
    let create = CreateInvitation::new(actor_selection(&f, f.alice, Some(f.landmark.clone())))?;
    let receipt = client
        .create_invitation(f.event, f.alice, Uuid::new_v4(), &create)
        .await?;
    let invitation_id = Uuid::parse_str(receipt["result"]["invitation"]["id"].as_str().unwrap())?;

    let wrong_actor = DecideInvitation::new(
        actor_selection(&f, f.alice, Some(f.landmark.clone())),
        1,
        Decision::Cancel,
    )?;
    assert_eq!(
        client
            .decide(f.event, f.bob, invitation_id, Uuid::new_v4(), &wrong_actor)
            .await,
        Err(MeetingError::ResourceUnavailable)
    );
    let stale = LandmarkSelection::new("gate-a", 3, 5)?;
    let stale_create = CreateInvitation::new(actor_selection(&f, f.alice, Some(stale)))?;
    assert_eq!(
        client
            .create_invitation(f.event, f.alice, Uuid::new_v4(), &stale_create)
            .await,
        Err(MeetingError::ResourceUnavailable)
    );

    let cancel = DecideInvitation::new(
        actor_selection(&f, f.alice, Some(f.landmark.clone())),
        1,
        Decision::Cancel,
    )?;
    let cancelled = client
        .decide(f.event, f.alice, invitation_id, Uuid::new_v4(), &cancel)
        .await?;
    assert_eq!(cancelled["result"]["invitation"]["state"], "CANCELLED");
    let accept_after = DecideInvitation::new(
        actor_selection(&f, f.bob, Some(f.landmark.clone())),
        2,
        Decision::Accept,
    )?;
    assert_eq!(
        client
            .decide(f.event, f.bob, invitation_id, Uuid::new_v4(), &accept_after)
            .await,
        Err(MeetingError::ResourceUnavailable)
    );
    let decline_create = CreateInvitation::new(actor_selection(&f, f.alice, Some(f.landmark.clone())))?;
    let declined_id = client
        .create_invitation(f.event, f.alice, Uuid::new_v4(), &decline_create)
        .await?["result"]["invitation"]["id"]
        .as_str()
        .map(Uuid::parse_str)
        .transpose()?
        .ok_or_else(|| anyhow::anyhow!("decline invitation id"))?;
    let decline = DecideInvitation::new(
        actor_selection(&f, f.bob, Some(f.landmark.clone())),
        1,
        Decision::Decline,
    )?;
    let declined = client
        .decide(f.event, f.bob, declined_id, Uuid::new_v4(), &decline)
        .await?;
    assert_eq!(declined["result"]["invitation"]["state"], "DECLINED");
    assert_eq!(
        InvitationState::Pending.as_str(),
        "PENDING",
        "terminal decisions never reopen"
    );
    Ok(())
}
