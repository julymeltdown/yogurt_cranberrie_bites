#[path = "support/meeting/mod.rs"]
mod support;
use concert_application::{beacon::*, commands::*};
use concert_domain::beacon::*;
use concert_service::meeting::{
    beacon::{mutate_in, PgMeetingRepository},
    commands::BeaconCommands,
};
use support::*;
use uuid::Uuid;
fn keys() -> FingerprintKeys {
    FingerprintKeys::new(SigningKey::new("meeting-test", &[23; 32]).unwrap(), vec![]).unwrap()
}
fn create(f: &Fixture) -> BeaconMutation {
    BeaconMutation::Create(CreateBeacon {
        meeting: f.meeting.clone(),
        physical_presence_acknowledged: true,
    })
}

#[tokio::test]
async fn beacon_privacy_expiry_revisions_and_receipt_races() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let f = setup(&pool).await?;
    let commands = BeaconCommands::new(pool.clone(), SyntheticAuthority);
    let reads = PgMeetingRepository::new(pool.clone(), SyntheticAuthority);
    let id = Uuid::new_v4();
    let key = Uuid::new_v4();
    let receipt = commands
        .execute(f.actor(f.owner), id, key, create(&f), &keys())
        .await?;
    assert_eq!(receipt.state, ReceiptState::Succeeded);
    assert_eq!(receipt.reference.unwrap().revision, 1);
    let replay = commands
        .execute(f.actor(f.owner), id, key, create(&f), &keys())
        .await?;
    assert_eq!(
        receipt, replay,
        "exact command replay has one receipt/effect"
    );
    let mut changed = f.meeting.clone();
    changed.instructions = "Changed request".into();
    assert_eq!(
        commands
            .execute(
                f.actor(f.owner),
                id,
                key,
                BeaconMutation::Create(CreateBeacon {
                    meeting: changed,
                    physical_presence_acknowledged: true
                }),
                &keys()
            )
            .await,
        Err(CommandError::Conflict)
    );
    assert_eq!(
        reads.read(f.actor(f.peer), id).await?.meeting.instructions,
        f.meeting.instructions
    );
    assert_eq!(
        reads.read(f.actor(Uuid::new_v4()), id).await,
        Err(MeetingError::ResourceUnavailable)
    );
    assert_eq!(
        reads
            .read(
                MeetingActor {
                    event: Uuid::new_v4(),
                    actor: f.peer
                },
                id
            )
            .await,
        Err(MeetingError::ResourceUnavailable)
    );
    sqlx::query("update meeting_fixture_authority set blocked=$2 where event=$1")
        .bind(f.event)
        .bind(vec![f.peer])
        .execute(&pool)
        .await?;
    assert_eq!(
        reads.read(f.actor(f.peer), id).await,
        Err(MeetingError::ResourceUnavailable)
    );
    sqlx::query(
        "update meeting_fixture_authority set blocked='{}',unavailable=true where event=$1",
    )
    .bind(f.event)
    .execute(&pool)
    .await?;
    assert_eq!(
        reads.read(f.actor(f.peer), id).await,
        Err(MeetingError::PolicyUnavailable)
    );
    assert_eq!(
        commands
            .execute(f.actor(f.owner), id, key, create(&f), &keys())
            .await,
        Err(CommandError::Unavailable),
        "receipt replay reauthorizes"
    );
    sqlx::query("update meeting_fixture_authority set unavailable=false where event=$1")
        .bind(f.event)
        .execute(&pool)
        .await?;
    // Two competing different-key edits produce exactly one new revision.
    let mut moved = f.meeting.clone();
    moved.instructions = "Moved synthetic guidance".into();
    moved.audience = Audience::ExplicitUsers {
        user_ids: vec![f.owner],
    };
    let edit = BeaconMutation::Update(UpdateBeacon {
        expected_revision: 1,
        meeting: moved,
        physical_presence_acknowledged: true,
    });
    let k = keys();
    let (a, b) = tokio::join!(
        commands.execute(f.actor(f.owner), id, Uuid::new_v4(), edit.clone(), &k),
        commands.execute(f.actor(f.owner), id, Uuid::new_v4(), edit, &k)
    );
    let outcomes = [a?, b?];
    assert_eq!(
        outcomes
            .iter()
            .filter(|r| r.state == ReceiptState::Succeeded)
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|r| r.failure == Some(SafeFailure::VersionConflict))
            .count(),
        1
    );
    assert_eq!(reads.read(f.actor(f.owner), id).await?.revision, 2);
    assert_eq!(
        reads.read(f.actor(f.peer), id).await,
        Err(MeetingError::ResourceUnavailable),
        "old audience cannot fetch new guidance"
    );
    let revoke_key = Uuid::new_v4();
    let revoke = BeaconMutation::Revoke {
        expected_revision: 2,
    };
    let revoked = commands
        .execute(f.actor(f.owner), id, revoke_key, revoke.clone(), &k)
        .await?;
    assert_eq!(
        revoked,
        commands
            .execute(f.actor(f.owner), id, revoke_key, revoke, &k)
            .await?
    );
    assert_eq!(
        reads.read(f.actor(f.owner), id).await,
        Err(MeetingError::ResourceUnavailable)
    );
    // Native DB time, not a supplied application clock, controls terminal expiry.
    let short_id = Uuid::new_v4();
    let mut short = f.meeting.clone();
    short.expires_at = sqlx::query_scalar("select clock_timestamp()+interval '350 milliseconds'")
        .fetch_one(&pool)
        .await?;
    commands
        .execute(
            f.actor(f.owner),
            short_id,
            Uuid::new_v4(),
            BeaconMutation::Create(CreateBeacon {
                meeting: short,
                physical_presence_acknowledged: true,
            }),
            &k,
        )
        .await?;
    sqlx::query("select pg_sleep(0.4)").execute(&pool).await?;
    assert_eq!(
        reads.read(f.actor(f.peer), short_id).await,
        Err(MeetingError::ResourceUnavailable)
    );
    let mut tx = pool.begin().await?;
    assert_eq!(
        mutate_in(
            &mut tx,
            &SyntheticAuthority,
            f.actor(f.owner),
            short_id,
            &BeaconMutation::Update(UpdateBeacon {
                expected_revision: 1,
                meeting: f.meeting.clone(),
                physical_presence_acknowledged: true
            })
        )
        .await,
        Err(MeetingError::ResourceUnavailable)
    );
    tx.rollback().await?;
    // Place closure invalidates details without rewriting immutable map bytes.
    let closed = Uuid::new_v4();
    commands
        .execute(f.actor(f.owner), closed, Uuid::new_v4(), create(&f), &k)
        .await?;
    sqlx::query("update concert_place_current_state set closure_state='CLOSED',revision=revision+1 where place_id=$1").bind(f.meeting.place_id).execute(&pool).await?;
    assert_eq!(
        reads.read(f.actor(f.peer), closed).await,
        Err(MeetingError::ResourceUnavailable)
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn database_guards_ack_cutoff_place_and_removal_fence() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let f = setup(&pool).await?;
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await?;
    let mut bad = CreateBeacon {
        meeting: f.meeting.clone(),
        physical_presence_acknowledged: false,
    };
    assert_eq!(
        mutate_in(
            &mut tx,
            &SyntheticAuthority,
            f.actor(f.owner),
            id,
            &BeaconMutation::Create(bad.clone())
        )
        .await,
        Err(MeetingError::InvalidInput)
    );
    tx.rollback().await?;
    bad.physical_presence_acknowledged = true;
    bad.meeting.map_revision = 99;
    let mut tx = pool.begin().await?;
    assert_eq!(
        mutate_in(
            &mut tx,
            &SyntheticAuthority,
            f.actor(f.owner),
            id,
            &BeaconMutation::Create(bad)
        )
        .await,
        Err(MeetingError::MapMismatch)
    );
    tx.rollback().await?;
    // Reader authority lock fences member removal until the read transaction completes.
    let mut tx = pool.begin().await?;
    mutate_in(
        &mut tx,
        &SyntheticAuthority,
        f.actor(f.owner),
        id,
        &create(&f),
    )
    .await?;
    tx.commit().await?;
    let mut read_tx = pool.begin().await?;
    concert_service::meeting::beacon::read_in(
        &mut read_tx,
        &SyntheticAuthority,
        f.actor(f.peer),
        id,
    )
    .await?;
    let mut remove = pool.begin().await?;
    sqlx::query("set local lock_timeout='100ms'")
        .execute(&mut *remove)
        .await?;
    assert!(
        sqlx::query("update meeting_fixture_authority set members=$2 where event=$1")
            .bind(f.event)
            .bind(vec![f.owner])
            .execute(&mut *remove)
            .await
            .is_err()
    );
    remove.rollback().await?;
    read_tx.commit().await?;
    sqlx::query("update meeting_fixture_authority set members=$2 where event=$1")
        .bind(f.event)
        .bind(vec![f.owner])
        .execute(&pool)
        .await?;
    assert_eq!(
        PgMeetingRepository::new(pool.clone(), SyntheticAuthority)
            .read(f.actor(f.peer), id)
            .await,
        Err(MeetingError::ResourceUnavailable)
    );
    // TTL and event cutoff are checked even without the application path.
    let bad=sqlx::query("update concert_meeting_beacons set expires_at=clock_timestamp()+interval '1 hour',revision=revision+1 where beacon_id=$1").bind(id).execute(&pool).await;
    assert!(bad.is_err());
    sqlx::query("update concert_meeting_settings set event_cutoff=clock_timestamp()+interval '2 seconds' where event_id=$1").bind(f.event).execute(&pool).await?;
    let mut tx = pool.begin().await?;
    let mut m = f.meeting.clone();
    m.audience = Audience::ExplicitUsers {
        user_ids: vec![f.owner],
    };
    assert_eq!(
        mutate_in(
            &mut tx,
            &SyntheticAuthority,
            f.actor(f.owner),
            Uuid::new_v4(),
            &BeaconMutation::Create(CreateBeacon {
                meeting: m,
                physical_presence_acknowledged: true
            })
        )
        .await,
        Err(MeetingError::InvalidInput)
    );
    tx.rollback().await?;
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn party_beacon_fences_movement_and_audience_changes() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let f = setup(&pool).await?;
    let commands = BeaconCommands::new(pool.clone(), SyntheticAuthority);
    let reads = PgMeetingRepository::new(pool.clone(), SyntheticAuthority);
    let mut m = f.meeting.clone();
    m.audience = Audience::PartyMembers { party_id: f.party };
    let id = Uuid::new_v4();
    commands
        .execute(
            f.actor(f.owner),
            id,
            Uuid::new_v4(),
            BeaconMutation::Create(CreateBeacon {
                meeting: m.clone(),
                physical_presence_acknowledged: true,
            }),
            &keys(),
        )
        .await?;
    assert_eq!(reads.read(f.actor(f.peer), id).await?.revision, 1);
    sqlx::query("update meeting_fixture_authority set meeting_revision=2 where event=$1")
        .bind(f.event)
        .execute(&pool)
        .await?;
    assert_eq!(
        reads.read(f.actor(f.peer), id).await,
        Err(MeetingError::ResourceUnavailable)
    );
    let fresh = Uuid::new_v4();
    commands
        .execute(
            f.actor(f.owner),
            fresh,
            Uuid::new_v4(),
            BeaconMutation::Create(CreateBeacon {
                meeting: m,
                physical_presence_acknowledged: true,
            }),
            &keys(),
        )
        .await?;
    sqlx::query("update meeting_fixture_authority set audience_revision=2 where event=$1")
        .bind(f.event)
        .execute(&pool)
        .await?;
    assert_eq!(
        reads.read(f.actor(f.peer), fresh).await,
        Err(MeetingError::ResourceUnavailable)
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn restart_replays_persisted_receipt_and_authorized_projection() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    sqlx::query("create table if not exists meeting_fixture_restart(event uuid not null,owner uuid not null,peer uuid not null,beacon uuid not null,key uuid not null,body jsonb not null,receipt uuid not null)").execute(&pool).await?;
    if std::env::var("MEETING_RESTART_VERIFY").as_deref() == Ok("1") {
        use sqlx::Row;
        let rows = sqlx::query("select * from meeting_fixture_restart")
            .fetch_all(&pool)
            .await?;
        assert!(!rows.is_empty(), "restart must verify earlier effects");
        let commands = BeaconCommands::new(pool.clone(), SyntheticAuthority);
        let reads = PgMeetingRepository::new(pool.clone(), SyntheticAuthority);
        for row in rows {
            let actor = MeetingActor {
                event: row.get("event"),
                actor: row.get("owner"),
            };
            let id: Uuid = row.get("beacon");
            let body: CreateBeacon = serde_json::from_value(row.get("body"))?;
            let replay = commands
                .execute(
                    actor,
                    id,
                    row.get("key"),
                    BeaconMutation::Create(body),
                    &keys(),
                )
                .await?;
            assert_eq!(replay.id, row.get::<Uuid, _>("receipt"));
            assert_eq!(
                reads
                    .read(
                        MeetingActor {
                            event: actor.event,
                            actor: row.get("peer")
                        },
                        id
                    )
                    .await?
                    .revision,
                1
            );
            let count: i64 = sqlx::query_scalar(
                "select count(*) from concert_command_receipts where receipt_id=$1",
            )
            .bind(replay.id)
            .fetch_one(&pool)
            .await?;
            assert_eq!(count, 1);
        }
    } else {
        let f = setup(&pool).await?;
        let id = Uuid::new_v4();
        let key = Uuid::new_v4();
        let body = CreateBeacon {
            meeting: f.meeting.clone(),
            physical_presence_acknowledged: true,
        };
        let receipt = BeaconCommands::new(pool.clone(), SyntheticAuthority)
            .execute(
                f.actor(f.owner),
                id,
                key,
                BeaconMutation::Create(body.clone()),
                &keys(),
            )
            .await?;
        sqlx::query("insert into meeting_fixture_restart(event,owner,peer,beacon,key,body,receipt) values($1,$2,$3,$4,$5,$6,$7)").bind(f.event).bind(f.owner).bind(f.peer).bind(id).bind(key).bind(serde_json::to_value(body)?).bind(receipt.id).execute(&pool).await?;
    }
    pool.close().await;
    Ok(())
}
