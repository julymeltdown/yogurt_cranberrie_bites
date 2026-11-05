//! Run only through world_admission_smoke.py against a marker/inode-owned PostgreSQL cluster.
use chrono::{DateTime, Duration, Utc};
use concert_application::world_ticket::*;
use concert_domain::{AssetId, AuthUserId, EventId, ParticipantKey, Revision};
use concert_service::world::PgWorldRepository;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    str::FromStr,
    time::Duration as StdDuration,
};
use uuid::Uuid;

async fn owned_pool() -> anyhow::Result<PgPool> {
    owned_pool_with_limit(20).await
}

async fn owned_pool_with_limit(max_connections: u32) -> anyhow::Result<PgPool> {
    let root = PathBuf::from(std::env::var("WORLD_TEST_OWNER_ROOT")?);
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        info.is_dir() && !info.file_type().is_symlink() && info.permissions().mode() & 0o077 == 0,
        "private owned root required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == std::env::var("WORLD_TEST_OWNER_IDENTITY")?,
        "fixture identity changed"
    );
    let marker_path = root.join("mcr-owner.json");
    let marker_info = std::fs::symlink_metadata(&marker_path)?;
    anyhow::ensure!(
        marker_info.is_file()
            && !marker_info.file_type().is_symlink()
            && marker_info.permissions().mode() & 0o077 == 0,
        "private marker required"
    );
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(marker_path)?)?;
    anyhow::ensure!(
        marker["token"].as_str() == Some(&std::env::var("WORLD_TEST_OWNER_TOKEN")?),
        "fixture owner mismatch"
    );
    let options = PgConnectOptions::from_str(&std::env::var("WORLD_TEST_DATABASE_URL")?)
        .map_err(|_| anyhow::anyhow!("invalid owned fixture options"))?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_world")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(StdDuration::from_secs(3))
        .connect_with(options)
        .await?;
    let actual: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        actual == root.join("data").to_string_lossy(),
        "foreign data directory refused"
    );
    Ok(pool)
}

#[derive(Clone, Copy)]
struct Fixture {
    event: EventId,
    user: AuthUserId,
    room_a: RoomId,
    room_b: RoomId,
}

async fn fixture(pool: &PgPool, capacity: i32) -> anyhow::Result<Fixture> {
    let event = Uuid::new_v4();
    let user = Uuid::new_v4();
    let room_a = Uuid::new_v4();
    let room_b = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'World test event','OPEN')")
        .bind(event).bind(format!("world-{}", event.simple())).execute(pool).await?;
    sqlx::query("insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,is_default) values($1,$2,'Room A',$3,'map_fixture',1,'avatar_manifest',1,true),($1,$4,'Room B',$3,'map_fixture',1,'avatar_manifest',1,false)")
        .bind(event).bind(room_a).bind(capacity).bind(room_b).execute(pool).await?;
    Ok(Fixture {
        event: EventId::from_uuid(event),
        user: AuthUserId::from_uuid(user),
        room_a: RoomId::from_uuid(room_a),
        room_b: RoomId::from_uuid(room_b),
    })
}

fn session() -> concert_application::identity::SessionId {
    concert_application::identity::SessionId::new(Uuid::new_v4().to_string()).unwrap()
}

fn owner(name: &str) -> OwnerIdentity {
    OwnerIdentity::new(name.into(), Uuid::new_v4()).unwrap()
}

fn pin(f: Fixture, room: RoomId) -> ResourcePin {
    ResourcePin {
        event_id: f.event,
        room_id: room,
        map: MapPin::new(
            AssetId::new("map_fixture".into()).unwrap(),
            Revision::initial(),
        ),
        avatar: AvatarPin::new(
            AssetId::new("avatar_manifest".into()).unwrap(),
            Revision::initial(),
        ),
    }
}

fn record(
    f: Fixture,
    room: RoomId,
    seed: u8,
    now: DateTime<Utc>,
) -> (TicketRecord, AdmissionTicket) {
    // The digest is globally unique in the test database even though tests create independent
    // events. Derive fixture entropy from those event/room identities so a caller's local seed
    // cannot collide with a ticket issued by another test.
    let mut entropy = [0_u8; TICKET_BYTES];
    entropy[..16].copy_from_slice(f.event.uuid().as_bytes());
    entropy[16..].copy_from_slice(room.uuid().as_bytes());
    entropy[0] ^= seed;
    let ticket = AdmissionTicket::from_entropy(&entropy).unwrap();
    let session = session();
    (
        TicketRecord {
            digest: ticket.digest(),
            subject: WorldSubject {
                key: ParticipantKey {
                    event_id: f.event,
                    user_id: f.user,
                },
                event_id: f.event,
                session_id: session,
            },
            pin: pin(f, room),
            issued_at: now,
            expires_at: now + Duration::seconds(30),
            access_expires_at: now + Duration::seconds(90),
        },
        ticket,
    )
}

fn with_identity(
    record: TicketRecord,
    user_id: AuthUserId,
    session_id: concert_application::identity::SessionId,
) -> TicketRecord {
    let mut value = record;
    value.subject.key.user_id = user_id;
    value.subject.session_id = session_id;
    value
}

async fn admission(
    repo: &PgWorldRepository,
    ticket: AdmissionTicket,
    pin: ResourcePin,
    connection: ConnectionId,
) -> anyhow::Result<ConsumeOutcome> {
    let now = Utc::now();
    Ok(repo
        .consume(ConsumeClaim {
            digest: ticket.digest(),
            connection_id: connection,
            requested: pin,
            now,
        })
        .await?)
}

#[tokio::test]
async fn ticket_digest_is_single_use_under_competing_connections() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let repo = PgWorldRepository::new(pool.clone());
    let f = fixture(&pool, 2).await?;
    let now = Utc::now();
    let (rec, ticket) = record(f, f.room_a, 1, now);
    let p = pin(f, f.room_a);
    let original_digest = ticket.digest();
    assert_eq!(repo.issue(rec).await?, IssueOutcome::Issued);
    let one = repo.clone();
    let two = repo.clone();
    let first_connection = ConnectionId::from_uuid(Uuid::new_v4())?;
    let second_connection = ConnectionId::from_uuid(Uuid::new_v4())?;
    let (left, right) = tokio::join!(
        admission(&one, ticket.clone(), p.clone(), first_connection),
        admission(&two, ticket.clone(), p.clone(), second_connection)
    );
    // A conflict is a real infrastructure/admission failure; never erase it into a retry.
    let outcomes = match (left, right) {
        (Ok(one), Ok(two)) => [one, two],
        (Err(error), _) | (_, Err(error)) => {
            anyhow::bail!("competing consume failed: {error:?}")
        }
    };
    assert_eq!(
        outcomes
            .iter()
            .filter(|v| matches!(v, ConsumeOutcome::Consumed(_)))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|v| matches!(v, ConsumeOutcome::AlreadyConsumed))
            .count(),
        1
    );
    let digest = original_digest;
    let count: i64 =
        sqlx::query_scalar("select count(*) from concert_world_tickets where ticket_digest=$1")
            .bind(digest.as_bytes().as_slice())
            .fetch_one(&pool)
            .await?;
    assert_eq!(count, 1);
    let persisted: Vec<u8> = sqlx::query_scalar(
        "select ticket_digest from concert_world_tickets where ticket_digest=$1",
    )
    .bind(original_digest.as_bytes().as_slice())
    .fetch_one(&pool)
    .await?;
    assert_eq!(persisted, original_digest.as_bytes().as_slice());
    assert_eq!(
        admission(&repo, ticket, p, first_connection).await?,
        ConsumeOutcome::AlreadyConsumed
    );
    Ok(())
}

#[tokio::test]
async fn persisted_capacity_and_switch_are_atomic_for_concurrent_users() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let repo = PgWorldRepository::new(pool.clone());
    let f = fixture(&pool, 2).await?;
    let room_owner = owner("room-owner-a");
    repo.claim_owner(
        f.event,
        f.room_a,
        &room_owner,
        Utc::now(),
        Duration::seconds(30),
    )
    .await?;
    assert_eq!(
        repo.renew_owner(
            f.event,
            f.room_a,
            &room_owner,
            Utc::now(),
            Duration::seconds(30),
        )
        .await?
        .event_id,
        f.event
    );
    let now = Utc::now();
    let mut seeds = 0_u8;
    let mut futures = Vec::new();
    for _ in 0..3 {
        seeds += 1;
        let (mut rec, ticket) = record(f, f.room_a, seeds, now);
        // All tickets target the same canonical event; their actors differ.
        rec.subject.key = ParticipantKey {
            event_id: f.event,
            user_id: AuthUserId::from_uuid(Uuid::new_v4()),
        };
        rec.subject.session_id = session();
        rec.digest = ticket.digest();
        let request = rec.pin.clone();
        repo.issue(rec).await?;
        let store = repo.clone();
        let owner = room_owner.clone();
        let connection = ConnectionId::from_uuid(Uuid::new_v4())?;
        futures.push(tokio::spawn(async move {
            let outcome = store
                .consume(ConsumeClaim {
                    digest: ticket.digest(),
                    connection_id: connection,
                    requested: request,
                    now: Utc::now(),
                })
                .await?;
            let ConsumeOutcome::Consumed(claim) = outcome else {
                anyhow::bail!("ticket not consumed")
            };
            Ok(store.admit(&owner, &claim, Utc::now()).await?)
        }));
    }
    let mut admitted = 0;
    let mut full = 0;
    for future in futures {
        match future.await?? {
            AdmissionOutcome::Admitted(_) => admitted += 1,
            AdmissionOutcome::Full => full += 1,
            other => anyhow::bail!("unexpected admission: {other:?}"),
        }
    }
    assert_eq!((admitted, full), (2, 1));
    let rows: i64 = sqlx::query_scalar(
        "select count(*) from concert_world_memberships where event_id=$1 and room_id=$2",
    )
    .bind(f.event.uuid())
    .bind(f.room_a.uuid())
    .fetch_one(&pool)
    .await?;
    assert_eq!(rows, 2);
    // Keep a third slot for the independent switch/takeover probe below.
    sqlx::query("update concert_world_rooms set capacity=3 where event_id=$1 and room_id=$2")
        .bind(f.event.uuid())
        .bind(f.room_a.uuid())
        .execute(&pool)
        .await?;
    repo.claim_owner(
        f.event,
        f.room_b,
        &room_owner,
        Utc::now(),
        Duration::seconds(30),
    )
    .await?;
    let active: i64 = sqlx::query_scalar(
        "select count(*) from concert_world_owner_leases where event_id=$1 and room_id=$2",
    )
    .bind(f.event.uuid())
    .bind(f.room_a.uuid())
    .fetch_one(&pool)
    .await?;
    assert_eq!(active, 1);
    let switch_time = Utc::now();
    let (rec_a, ticket_initial) = record(f, f.room_a, 22, switch_time);
    let (rec_b, ticket_a) = record(f, f.room_b, 20, switch_time);
    let initial_digest = ticket_initial.digest();
    let switch_digest = ticket_a.digest();
    let switch_user = AuthUserId::from_uuid(Uuid::new_v4());
    let switch_session = session();
    let rec_a = with_identity(rec_a, switch_user, switch_session.clone());
    let rec_b = with_identity(rec_b, switch_user, switch_session);
    let issued_initial_pin = pin(f, f.room_a);
    let issued_switch_pin = pin(f, f.room_b);
    let switch_expiry = rec_b.expires_at;
    assert_eq!(repo.issue(rec_a).await?, IssueOutcome::Issued);
    assert_eq!(repo.issue(rec_b).await?, IssueOutcome::Issued);
    let outcome_a = repo
        .consume(ConsumeClaim {
            digest: initial_digest,
            connection_id: ConnectionId::from_uuid(Uuid::new_v4())?,
            requested: issued_initial_pin,
            now: switch_time,
        })
        .await?;
    let ConsumeOutcome::Consumed(claim_a) = outcome_a else {
        anyhow::bail!("first switch ticket not consumed: {outcome_a:?}")
    };
    let AdmissionOutcome::Admitted(first_room) =
        repo.admit(&room_owner, &claim_a, switch_time).await?
    else {
        anyhow::bail!("first switch admission failed")
    };
    assert!(switch_time < switch_expiry);
    let ConsumeOutcome::Consumed(claim_b) = repo
        .consume(ConsumeClaim {
            digest: switch_digest,
            connection_id: first_room.membership.connection_id,
            requested: issued_switch_pin,
            now: switch_time,
        })
        .await?
    else {
        anyhow::bail!("switch ticket not consumed")
    };
    let SwitchOutcome::Switched(switched) = repo
        .switch(&room_owner, &first_room.membership, &claim_b, switch_time)
        .await?
    else {
        anyhow::bail!("room switch failed")
    };
    assert_eq!(switched.membership.room_id, f.room_b);
    assert_eq!(
        switched.membership.connection_id,
        first_room.membership.connection_id
    );
    let memberships: i64 = sqlx::query_scalar(
        "select count(*) from concert_world_memberships where event_id=$1 and user_id=$2",
    )
    .bind(f.event.uuid())
    .bind(switch_user.uuid())
    .fetch_one(&pool)
    .await?;
    assert_eq!(memberships, 1);
    let persisted_owner: String = sqlx::query_scalar(
        "select owner_process_id from concert_world_owner_leases where event_id=$1 and room_id=$2",
    )
    .bind(f.event.uuid())
    .bind(f.room_b.uuid())
    .fetch_one(&pool)
    .await?;
    assert_eq!(persisted_owner, room_owner.process_id);
    Ok(())
}

#[tokio::test]
async fn owner_lease_fencing_and_ghost_prune_are_persisted() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let repo = PgWorldRepository::new(pool.clone());
    let f = fixture(&pool, 1).await?;
    let first = owner("owner-a");
    let second = owner("owner-b");
    repo.claim_owner(f.event, f.room_a, &first, Utc::now(), Duration::seconds(20))
        .await?;
    assert_eq!(
        repo.claim_owner(
            f.event,
            f.room_a,
            &second,
            Utc::now(),
            Duration::seconds(20)
        )
        .await,
        Err(WorldError::OwnerFenced)
    );
    assert_eq!(
        repo.renew_owner(
            f.event,
            f.room_a,
            &second,
            Utc::now(),
            Duration::seconds(20)
        )
        .await,
        Err(WorldError::OwnerFenced)
    );
    let other = fixture(&pool, 1).await?;
    assert_eq!(
        repo.claim_owner(
            other.event,
            other.room_a,
            &second,
            Utc::now(),
            Duration::seconds(20)
        )
        .await?
        .event_id,
        other.event
    );
    let pruned = repo.prune(Utc::now() + Duration::seconds(120)).await?;
    assert_eq!(pruned.tickets, 0);
    let now = Utc::now();
    let (rec, ticket) = record(f, f.room_a, 3, now);
    repo.issue(rec).await?;
    let digest = ticket.digest();
    sqlx::query("update concert_world_tickets set expires_at=issued_at+interval '1 second',access_expires_at=issued_at+interval '2 seconds' where ticket_digest=$1")
        .bind(digest.as_bytes().as_slice()).execute(&pool).await?;
    let pruned = repo.prune(now + Duration::seconds(3)).await?;
    assert_eq!(pruned.tickets, 1);
    assert_eq!(
        repo.consume(ConsumeClaim {
            digest,
            connection_id: ConnectionId::from_uuid(Uuid::new_v4())?,
            requested: pin(f, f.room_a),
            now: now + Duration::seconds(3),
        })
        .await?,
        ConsumeOutcome::Unknown
    );
    Ok(())
}

#[tokio::test]
async fn owner_takeover_does_not_relabel_an_existing_membership() -> anyhow::Result<()> {
    let pool = owned_pool_with_limit(2).await?;
    let repo = PgWorldRepository::new(pool.clone());
    // Independently fence an epoch change and a process change. Keep the old
    // membership live while the shorter owner lease expires; no wall-clock sleep.
    for change_epoch in [true, false] {
        let f = fixture(&pool, 1).await?;
        let now = DateTime::<Utc>::from_timestamp_micros(Utc::now().timestamp_micros()).unwrap();
        let first = owner("takeover-a");
        let second = OwnerIdentity::new(
            if change_epoch { first.process_id.clone() } else { "takeover-b".into() },
            if change_epoch { Uuid::new_v4() } else { first.epoch },
        )?;
        repo.claim_owner(f.event, f.room_a, &first, now, Duration::seconds(2)).await?;
        let (rec, ticket) = record(f, f.room_a, 17, now);
        let digest = ticket.digest();
        repo.issue(rec).await?;
        let ConsumeOutcome::Consumed(claim) = repo.consume(ConsumeClaim {
            digest,
            connection_id: ConnectionId::from_uuid(Uuid::new_v4())?,
            requested: pin(f, f.room_a),
            now,
        }).await? else { anyhow::bail!("initial ticket not consumed") };
        let AdmissionOutcome::Admitted(admitted) = repo.admit(&first, &claim, now).await?
        else { anyhow::bail!("initial admission failed") };
        let key = admitted.membership.key;
        assert_eq!(repo.current_owned_membership(key, now).await?.unwrap().owner, first);
        let subscription = SubscriptionLease {
            key,
            session_id: admitted.membership.session_id.clone(),
            room_id: f.room_a,
            connection_id: admitted.membership.connection_id,
            owner: first.clone(),
        };
        assert!(matches!(repo.reauthorize(&subscription, now).await?, LeaseState::Active(_)));
        let later = now + Duration::seconds(3);
        assert!(admitted.membership.lease_until > later);
        repo.claim_owner(f.event, f.room_a, &second, later, Duration::seconds(20)).await?;
        assert!(repo.current_owned_membership(key, later).await?.is_none());
        assert_eq!(repo.reauthorize(&subscription, later).await?, LeaseState::Gone);
        let relabeled = SubscriptionLease { owner: second.clone(), ..subscription };
        assert_eq!(repo.reauthorize(&relabeled, later).await?, LeaseState::Gone);

        // Only an actual new ticket/admission may write the replacement owner.
        let (rec, ticket) = record(f, f.room_a, 18, later);
        let digest = ticket.digest();
        repo.issue(rec).await?;
        let ConsumeOutcome::Consumed(claim) = repo.consume(ConsumeClaim {
            digest,
            connection_id: ConnectionId::from_uuid(Uuid::new_v4())?,
            requested: pin(f, f.room_a),
            now: later,
        }).await? else { anyhow::bail!("replacement ticket not consumed") };
        let AdmissionOutcome::Replaced(replacement) = repo.admit(&second, &claim, later).await?
        else { anyhow::bail!("replacement admission failed") };
        let current = repo.current_owned_membership(key, later).await?.unwrap();
        assert_eq!(current.owner, second);
        assert_eq!(current.membership, replacement.membership);
        let fresh = SubscriptionLease {
            session_id: replacement.membership.session_id.clone(),
            connection_id: replacement.membership.connection_id,
            ..relabeled
        };
        assert!(matches!(repo.reauthorize(&fresh, later).await?, LeaseState::Active(_)));
    }
    Ok(())
}

#[tokio::test]
async fn membership_leave_and_disconnect_grace_expire_canonical_rows() -> anyhow::Result<()> {
    let pool = owned_pool_with_limit(2).await?;
    let repo = PgWorldRepository::new(pool.clone());
    for disconnect in [false, true] {
        let f = fixture(&pool, 1).await?;
        let now = DateTime::<Utc>::from_timestamp_micros(Utc::now().timestamp_micros()).unwrap();
        let owner = owner("lifecycle");
        repo.claim_owner(f.event, f.room_a, &owner, now, Duration::seconds(20)).await?;
        let (rec, ticket) = record(f, f.room_a, 19, now);
        let digest = ticket.digest();
        repo.issue(rec).await?;
        let ConsumeOutcome::Consumed(claim) = repo.consume(ConsumeClaim {
            digest,
            connection_id: ConnectionId::from_uuid(Uuid::new_v4())?,
            requested: pin(f, f.room_a),
            now,
        }).await? else { anyhow::bail!("lifecycle ticket not consumed") };
        let AdmissionOutcome::Admitted(admission) = repo.admit(&owner, &claim, now).await?
        else { anyhow::bail!("lifecycle admission failed") };
        let member = admission.membership;
        let subscription = SubscriptionLease {
            key: member.key,
            session_id: member.session_id.clone(),
            room_id: member.room_id,
            connection_id: member.connection_id,
            owner,
        };
        if disconnect {
            let at = now + Duration::seconds(1);
            let state = repo.disconnect(member.key, f.event, member.connection_id,
                &member.session_id, at, Duration::seconds(1)).await?;
            let LeaseState::Disconnected(disconnected) = state
            else { anyhow::bail!("disconnect did not retain grace lease") };
            assert_eq!(disconnected.lease_until, now + Duration::seconds(2));
            assert!(repo.current_owned_membership(member.key, at).await?.is_none());
            assert!(matches!(repo.reauthorize(&subscription, at).await?, LeaseState::Disconnected(_)));
            let expiry = disconnected.lease_until;
            assert_eq!(repo.reauthorize(&subscription, expiry).await?, LeaseState::Gone);
            repo.prune(expiry).await?;
        } else {
            assert_eq!(repo.leave(member.key, &subscription, &session()).await, Err(WorldError::Denied));
            assert!(repo.current_owned_membership(member.key, now).await?.is_some());
            repo.leave(member.key, &subscription, &member.session_id).await?;
            assert_eq!(repo.reauthorize(&subscription, now).await?, LeaseState::Gone);
        }
        let count: i64 = sqlx::query_scalar("select count(*) from public.concert_world_memberships where event_id=$1 and user_id=$2")
            .bind(f.event.uuid()).bind(f.user.uuid()).fetch_one(&pool).await?;
        assert_eq!(count, 0, "leave or expired ghost must remove its canonical row");
    }
    Ok(())
}

#[tokio::test]
async fn catalog_is_bounded_and_redacts_exact_occupancy() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let repo = PgWorldRepository::new(pool.clone());
    let f = fixture(&pool, 50).await?;
    let catalog = repo.room_catalog(f.event).await?;
    assert_eq!(catalog.rooms.len(), 2);
    assert_eq!(catalog.default_room_id, f.room_a);
    assert!(catalog.rooms.iter().all(|room| room.capacity <= 50));
    assert!(catalog.rooms.iter().all(|room| matches!(
        room.availability,
        RoomAvailability::Available | RoomAvailability::Full | RoomAvailability::Unavailable
    )));
    Ok(())
}
