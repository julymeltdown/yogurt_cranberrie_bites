//! Public repository phase guards against an actual owned database.
//! Direct phase updates here isolate the guard; full closure is a separate native case.
use chrono::{Duration, Utc};
use concert_application::{identity::SessionId, world_ticket::*};
use concert_domain::{AssetId, AuthUserId, EventId, ParticipantKey, Revision};
use concert_service::world::PgWorldRepository;
use sqlx::Row;
use uuid::Uuid;
#[path = "world_read_only_support/owned.rs"]
mod owned;
fn record(
    event: Uuid,
    room: Uuid,
    user: Uuid,
    session: Uuid,
    entropy: u8,
) -> anyhow::Result<TicketRecord> {
    let t = AdmissionTicket::from_entropy(&[entropy; 32])?;
    let now = Utc::now();
    Ok(TicketRecord {
        digest: t.digest(),
        subject: WorldSubject {
            key: ParticipantKey {
                event_id: EventId::from_uuid(event),
                user_id: AuthUserId::from_uuid(user),
            },
            event_id: EventId::from_uuid(event),
            session_id: SessionId::new(session.to_string())?,
        },
        pin: ResourcePin {
            event_id: EventId::from_uuid(event),
            room_id: RoomId::from_uuid(room),
            map: MapPin::new(AssetId::new("map_fixture".into())?, Revision::initial()),
            avatar: AvatarPin::new(AssetId::new("avatar_fixture".into())?, Revision::initial()),
        },
        issued_at: now,
        expires_at: now + Duration::seconds(20),
        access_expires_at: now + Duration::seconds(60),
    })
}
async fn consume(repo: &PgWorldRepository, r: &TicketRecord) -> anyhow::Result<TicketClaim> {
    let outcome = repo
        .consume(ConsumeClaim {
            digest: r.digest,
            connection_id: ConnectionId::from_uuid(Uuid::new_v4())?,
            requested: r.pin.clone(),
            now: Utc::now(),
        })
        .await?;
    let ConsumeOutcome::Consumed(c) = outcome else {
        anyhow::bail!("owned ticket not consumed")
    };
    Ok(c)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires marker-owned world_read_only_support/native_smoke.py"]
async fn native_event_first_guards_deny_all_authority_mutations() -> anyhow::Result<()> {
    let pool = owned::pool("concert").await?;
    let repo = PgWorldRepository::new(pool.clone());
    let (event, user, session, room, target) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let event_id = EventId::from_uuid(event);
    let owner = OwnerIdentity::new(Uuid::new_v4().to_string(), Uuid::new_v4())?;
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'World phase guard','OPEN')").bind(event).bind(format!("phase-{event}")).execute(&pool).await?;
    sqlx::query("insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,is_default) values($1,$2,'One',4,'map_fixture',1,'avatar_fixture',1,true),($1,$3,'Two',4,'map_fixture',1,'avatar_fixture',1,false)").bind(event).bind(room).bind(target).execute(&pool).await?;
    for r in [room, target] {
        repo.claim_owner(
            event_id,
            RoomId::from_uuid(r),
            &owner,
            Utc::now(),
            Duration::seconds(5),
        )
        .await?;
    }
    let first = record(event, room, user, session, 111)?;
    assert_eq!(repo.issue(first.clone()).await?, IssueOutcome::Issued);
    let first = consume(&repo, &first).await?;
    let admitted = repo.admit(&owner, &first, Utc::now()).await?;
    let member = match admitted {
        AdmissionOutcome::Admitted(a) | AdmissionOutcome::Replaced(a) => a.membership,
        _ => anyhow::bail!("owned admission denied"),
    };
    let second = record(event, target, user, session, 112)?;
    repo.issue(second.clone()).await?;
    let second = consume(&repo, &second).await?;
    let spare = record(event, room, user, session, 113)?;
    repo.issue(spare.clone()).await?;
    let probe = record(event, room, user, session, 114)?;
    let mut close = pool.begin().await?;
    sqlx::query("select event_id from concert_events where event_id=$1 for update")
        .bind(event)
        .fetch_one(&mut *close)
        .await?;
    let task = {
        let repo = repo.clone();
        tokio::spawn(async move { repo.issue(probe).await })
    };
    let began = std::time::Instant::now();
    loop {
        let waiting:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like '%select phase from public.concert_events%')").fetch_one(&pool).await?;
        if waiting {
            break;
        }
        anyhow::ensure!(
            began.elapsed() < std::time::Duration::from_secs(1),
            "guard not blocked at event first"
        );
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let mut probe = pool.begin().await?;
    sqlx::query("select room_id from concert_world_rooms where event_id=$1 and room_id=$2 for update nowait").bind(event).bind(room).fetch_one(&mut *probe).await?;
    probe.rollback().await?;
    sqlx::query(
        "update concert_events set phase='READ_ONLY',revision=revision+1 where event_id=$1",
    )
    .bind(event)
    .execute(&mut *close)
    .await?;
    close.commit().await?;
    assert!(matches!(task.await?, Err(WorldError::Denied)));
    assert!(matches!(
        repo.issue(record(event, room, user, session, 115)?).await,
        Err(WorldError::Denied)
    ));
    assert!(matches!(
        repo.consume(ConsumeClaim {
            digest: spare.digest,
            connection_id: ConnectionId::from_uuid(Uuid::new_v4())?,
            requested: spare.pin,
            now: Utc::now()
        })
        .await,
        Err(WorldError::Denied)
    ));
    assert!(matches!(
        repo.admit(&owner, &first, Utc::now()).await,
        Err(WorldError::Denied)
    ));
    assert!(matches!(
        repo.switch(&owner, &member, &second, Utc::now()).await,
        Err(WorldError::Denied)
    ));
    assert!(matches!(
        repo.claim_owner(
            event_id,
            RoomId::from_uuid(room),
            &owner,
            Utc::now(),
            Duration::seconds(5)
        )
        .await,
        Err(WorldError::Denied)
    ));
    assert!(matches!(
        repo.renew_owner(
            event_id,
            RoomId::from_uuid(room),
            &owner,
            Utc::now(),
            Duration::seconds(5)
        )
        .await,
        Err(WorldError::Denied)
    ));
    let row=sqlx::query("select (select count(*) from concert_world_tickets where event_id=$1) as tickets,(select count(*) from concert_world_tickets where event_id=$1 and consumed_at is not null) as consumed").bind(event).fetch_one(&pool).await?;
    assert_eq!(row.get::<i64, _>("tickets"), 3);
    assert_eq!(row.get::<i64, _>("consumed"), 2);
    pool.close().await;
    Ok(())
}
