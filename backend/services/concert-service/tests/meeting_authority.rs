//! Common-transaction BE37/community integration. Only remote owner proof is synthetic.
#[path = "support/meeting/mod.rs"]
mod support;
use async_trait::async_trait;
use concert_application::{
    beacon::*, commands::*, community::PartyCommand, place::SelectionOutcome, rally::RallyQuery,
};
use concert_domain::{
    beacon::*, community::party::*, place::PlaceId, relationship::*, AuthUserId, EventId, Revision,
};
use concert_service::{
    adapters::out::repository::place::PgPlaceRepository,
    community::{CommunityAccess, CommunityAuthority, PgCommunity},
    meeting::{
        authority::PgMeetingAuthority,
        beacon::{mutate_in, read_in, PgMeetingRepository},
        commands::BeaconCommands,
        rally::PgRally,
        MeetingAuthority,
    },
    relationship::{FencedRelationshipAuthority, PgRelationshipPolicy},
};
use sqlx::{Acquire, PgConnection, PgPool, Row};
use std::time::Duration;
use support::*;
use uuid::Uuid;

#[derive(Clone, Copy)]
struct SyntheticOwnerFence;
#[async_trait]
impl FencedRelationshipAuthority for SyntheticOwnerFence {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        q: PolicyQuery,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<FreshAuthority, RelationshipError> {
        let row = sqlx::query("select * from meeting_fixture_authority where event=$1 for share")
            .bind(q.event.uuid())
            .fetch_optional(c)
            .await
            .map_err(|_| RelationshipError::Unavailable)?
            .ok_or(RelationshipError::Denied)?;
        if row.get::<bool, _>("unavailable") {
            return Err(RelationshipError::Unavailable);
        }
        let members: Vec<Uuid> = row.get("members");
        let blocked: Vec<Uuid> = row.get("blocked");
        Ok(FreshAuthority {
            query: q,
            checked_at: now,
            valid_until: now + chrono::Duration::seconds(30),
            revision: Revision::initial(),
            actor_allowed: members.contains(&q.actor.uuid()),
            subject_allowed: members.contains(&q.subject.uuid()),
            inherited_block: blocked.contains(&q.actor.uuid())
                || blocked.contains(&q.subject.uuid()),
        })
    }
}
struct ActualCommunityPolicy {
    policy: PgRelationshipPolicy<SyntheticOwnerFence>,
}
#[async_trait]
impl CommunityAuthority for ActualCommunityPolicy {
    async fn actor_in(
        &self,
        c: &mut PgConnection,
        actor: Uuid,
        event: Uuid,
        _access: CommunityAccess,
    ) -> Result<(), CommandError> {
        self.policy
            .authorize_actor_in(
                c,
                concert_domain::ParticipantKey {
                    event_id: EventId::from_uuid(event),
                    user_id: AuthUserId::from_uuid(actor),
                },
            )
            .await
            .map_err(|error| match error {
                RelationshipError::Denied => CommandError::Denied,
                _ => CommandError::Unavailable,
            })?;
        Ok(())
    }
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        actor: Uuid,
        event: Uuid,
        subject: Uuid,
        access: CommunityAccess,
    ) -> Result<(), CommandError> {
        self.policy
            .authorize_in(
                c,
                PolicyQuery {
                    event: EventId::from_uuid(event),
                    actor: AuthUserId::from_uuid(actor),
                    subject: AuthUserId::from_uuid(subject),
                    field: match access {
                        CommunityAccess::ReadParties | CommunityAccess::ReadGifts => {
                            Field::PublicProfile
                        }
                        CommunityAccess::WriteParties | CommunityAccess::WriteGifts => {
                            Field::Interaction
                        }
                        _ if actor == subject => Field::SafetySelf,
                        _ => Field::SafetyInteraction,
                    },
                },
                None,
            )
            .await
            .map_err(|e| match e {
                RelationshipError::Denied | RelationshipError::Stale => CommandError::Denied,
                _ => CommandError::Unavailable,
            })?;
        Ok(())
    }
    async fn place_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        place: Uuid,
    ) -> Result<(), CommandError> {
        let mut tx = c.begin().await.map_err(|_| CommandError::Unavailable)?;
        let row=sqlx::query("select catalog_revision,map_revision from concert_events_map_releases where event_id=$1 for share").bind(event).fetch_one(&mut *tx).await.map_err(|_|CommandError::Unavailable)?;
        let outcome = PgPlaceRepository::select_physical_in_transaction(
            &mut tx,
            EventId::from_uuid(event),
            PlaceId::from_uuid(place).unwrap(),
            Revision::new(row.get("catalog_revision")).unwrap(),
            row.get("map_revision"),
        )
        .await
        .map_err(|_| CommandError::Unavailable)?;
        if !matches!(outcome, SelectionOutcome::Eligible(_)) {
            return Err(CommandError::Denied);
        }
        tx.commit().await.map_err(|_| CommandError::Unavailable)
    }
}
fn keys() -> FingerprintKeys {
    FingerprintKeys::new(
        SigningKey::new("meeting-common-test", &[55; 32]).unwrap(),
        vec![],
    )
    .unwrap()
}
fn command(f: &Fixture, actor: Uuid, party: Uuid, action: PartyAction) -> PartyCommand {
    PartyCommand {
        actor,
        event: f.event,
        key: Uuid::new_v4(),
        party,
        action,
    }
}
async fn execute(
    service: &PgCommunity<ActualCommunityPolicy>,
    c: PartyCommand,
) -> anyhow::Result<SafeReceipt> {
    service
        .execute_party(&c, &keys())
        .await
        .map_err(|e| anyhow::anyhow!("community fixture: {e:?}"))
}
fn bridge(pool: &PgPool, actor: MeetingActor) -> PgMeetingAuthority<SyntheticOwnerFence> {
    PgMeetingAuthority::new(pool.clone(), actor, SyntheticOwnerFence)
}

#[tokio::test]
async fn actual_actor_guard_party_root_pair_order_contends_and_rolls_back() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let f = setup(&pool).await?;
    for user in [f.owner, f.peer] {
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(f.event).bind(user).execute(&pool).await?;
    }
    let service = PgCommunity::new(
        pool.clone(),
        ActualCommunityPolicy {
            policy: PgRelationshipPolicy::new(pool.clone(), SyntheticOwnerFence),
        },
    );
    let now = sqlx::query_scalar::<_, chrono::DateTime<chrono::Utc>>("select clock_timestamp()")
        .fetch_one(&pool)
        .await?;
    let create = command(
        &f,
        f.owner,
        Uuid::nil(),
        PartyAction::Create(PartyFields {
            title: "Common fixture".into(),
            description: "Synthetic only".into(),
            ticket_section: None,
            capacity: 2,
            meeting_starts_at: now,
            meeting_ends_at: now + chrono::Duration::hours(1),
            requirements: String::new(),
            public_place_id: f.meeting.place_id,
        }),
    );
    let party = create.party_id();
    execute(&service, create).await?;
    let apply = command(
        &f,
        f.peer,
        party,
        PartyAction::Apply {
            party_revision: 1,
            note: String::new(),
        },
    );
    let application = apply.target();
    execute(&service, apply).await?;
    execute(
        &service,
        command(
            &f,
            f.owner,
            party,
            PartyAction::Decide {
                application,
                expected: 1,
                party_revision: 1,
                approve: true,
            },
        ),
    )
    .await?;
    let mut m = f.meeting.clone();
    m.audience = Audience::PartyMembers { party_id: party };
    let beacon = Uuid::new_v4();
    let mut tx = pool.begin().await?;
    mutate_in(
        &mut tx,
        &bridge(&pool, f.actor(f.owner)),
        f.actor(f.owner),
        beacon,
        &BeaconMutation::Create(CreateBeacon {
            meeting: m.clone(),
            physical_presence_acknowledged: true,
        }),
    )
    .await?;
    tx.commit().await?;
    assert_eq!(
        PgMeetingRepository::new(pool.clone(), bridge(&pool, f.actor(f.peer)))
            .read(f.actor(f.peer), beacon)
            .await?
            .revision,
        1
    );
    let wrong = PgMeetingRepository::new(pool.clone(), bridge(&pool, f.actor(f.owner)))
        .read(f.actor(f.peer), beacon)
        .await;
    assert_eq!(
        wrong,
        Err(MeetingError::ResourceUnavailable),
        "request-bound actor cannot be substituted"
    );
    // Reproduce the formerly cyclic interleaving using real root and pair locks.
    let mut writer = pool.begin().await?;
    sqlx::query("select event_id from concert_events where event_id=$1 for share")
        .bind(f.event)
        .execute(&mut *writer)
        .await?;
    sqlx::query("select party_id from concert_community_parties where event_id=$1 and party_id=$2 for update").bind(f.event).bind(party).execute(&mut *writer).await?;
    let writer_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *writer)
        .await?;
    let mut reader = pool.begin().await?;
    let reader_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *reader)
        .await?;
    let actor = f.actor(f.owner);
    let reader_authority = bridge(&pool, actor);
    reader_authority.actor_in(&mut reader, actor).await?;
    let pending = tokio::spawn(async move {
        let value = read_in(&mut reader, &reader_authority, actor, beacon).await;
        reader
            .commit()
            .await
            .map_err(|_| MeetingError::StorageUnavailable)?;
        value
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let blocking: Vec<i32> = sqlx::query_scalar("select pg_blocking_pids($1)")
                .bind(reader_pid)
                .fetch_one(&pool)
                .await?;
            if blocking.contains(&writer_pid) {
                break;
            }
            tokio::task::yield_now().await;
        }
        Ok::<(), sqlx::Error>(())
    })
    .await??;
    // A command must wait for the party before taking the shared BE15 budget.
    // Otherwise the root holder below and beacon admission form a second cycle.
    let command_pool = pool.clone();
    let command_meeting = m.clone();
    let pending_command = tokio::spawn(async move {
        BeaconCommands::new(command_pool.clone(), bridge(&command_pool, actor))
            .execute(
                actor,
                Uuid::new_v4(),
                Uuid::new_v4(),
                BeaconMutation::Create(CreateBeacon {
                    meeting: command_meeting,
                    physical_presence_acknowledged: true,
                }),
                &keys(),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "select exists(select 1 from pg_stat_activity where datname=current_database() and pid<>$1 and $2=any(pg_blocking_pids(pid)))")
                .bind(reader_pid).bind(writer_pid).fetch_one(&pool).await?;
            if waiting { break; }
            tokio::task::yield_now().await;
        }
        Ok::<(), sqlx::Error>(())
    }).await??;
    tokio::time::timeout(
        Duration::from_millis(500),
        sqlx::query("select singleton from concert_command_budget where singleton for update")
            .execute(&mut *writer),
    )
    .await??;
    let pair = PgRelationshipPolicy::new(pool.clone(), SyntheticOwnerFence);
    tokio::time::timeout(
        Duration::from_millis(500),
        pair.authorize_in(
            &mut writer,
            PolicyQuery {
                event: EventId::from_uuid(f.event),
                actor: AuthUserId::from_uuid(f.owner),
                subject: AuthUserId::from_uuid(f.owner),
                field: Field::SafetySelf,
            },
            None,
        ),
    )
    .await??;
    let rolled = Uuid::new_v4();
    mutate_in(
        &mut writer,
        &bridge(&pool, actor),
        actor,
        rolled,
        &BeaconMutation::Create(CreateBeacon {
            meeting: m,
            physical_presence_acknowledged: true,
        }),
    )
    .await?;
    writer.rollback().await?;
    let got = tokio::time::timeout(Duration::from_secs(2), pending).await???;
    assert_eq!(got.beacon_id, beacon);
    let command_receipt = tokio::time::timeout(Duration::from_secs(2), pending_command).await???;
    assert_eq!(command_receipt.state, ReceiptState::Succeeded);

    let count: i64 =
        sqlx::query_scalar("select count(*) from concert_meeting_beacons where beacon_id=$1")
            .bind(rolled)
            .fetch_one(&pool)
            .await?;
    assert_eq!(count, 0, "protected effect rolls back");
    // Actual community removal fences the actual meeting adapter before outbox delivery.
    execute(
        &service,
        command(
            &f,
            f.owner,
            party,
            PartyAction::Remove {
                user: f.peer,
                expected: 1,
                party_revision: 2,
            },
        ),
    )
    .await?;
    assert_eq!(
        PgMeetingRepository::new(pool.clone(), bridge(&pool, f.actor(f.peer)))
            .read(f.actor(f.peer), beacon)
            .await,
        Err(MeetingError::ResourceUnavailable)
    );
    assert_eq!(
        PgMeetingRepository::new(pool.clone(), bridge(&pool, f.actor(f.owner)))
            .read(f.actor(f.owner), beacon)
            .await,
        Err(MeetingError::ResourceUnavailable),
        "old audience revision is stale"
    );
    assert_eq!(
        PgRally::new(pool.clone(), bridge(&pool, f.actor(f.peer)))
            .read(RallyQuery {
                actor: f.actor(f.peer),
                party
            })
            .await,
        Err(MeetingError::ResourceUnavailable)
    );
    pool.close().await;
    Ok(())
}
