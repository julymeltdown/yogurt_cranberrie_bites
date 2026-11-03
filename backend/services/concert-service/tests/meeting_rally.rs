#[path = "support/meeting/mod.rs"]
mod support;
use concert_application::{beacon::*, commands::*, rally::*};
use concert_domain::rally::*;
use concert_service::meeting::rally::PgRally;
use support::*;
use uuid::Uuid;
fn keys() -> FingerprintKeys {
    FingerprintKeys::new(SigningKey::new("rally-test", &[42; 32]).unwrap(), vec![]).unwrap()
}
fn input(rev: i64, meeting: i64, state: RallyState) -> SetRally {
    SetRally {
        expected_revision: rev,
        meeting_revision: meeting,
        state,
        physical_presence_acknowledged: true,
    }
}
#[tokio::test]
async fn rally_is_explicit_actor_scoped_and_rejects_stale_meeting() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let f = setup(&pool).await?;
    let rally = PgRally::new(pool.clone(), SyntheticAuthority);
    let q = RallyQuery {
        actor: f.actor(f.peer),
        party: f.party,
    };
    let k = keys();
    assert!(rally.read(q).await?.entries.is_empty());
    assert_eq!(
        rally
            .read(RallyQuery {
                actor: f.actor(Uuid::new_v4()),
                party: f.party
            })
            .await,
        Err(MeetingError::ResourceUnavailable)
    );
    let key = Uuid::new_v4();
    let first = input(0, 1, RallyState::OnMyWay);
    let receipt = rally.execute(q, key, first.clone(), &k).await?;
    assert_eq!(receipt.state, ReceiptState::Succeeded);
    assert_eq!(
        receipt.reference,
        Some(SafeReference {
            resource: f.peer,
            revision: 1
        })
    );
    assert_eq!(receipt, rally.execute(q, key, first, &k).await?);
    let snapshot = rally.read(q).await?;
    assert_eq!(snapshot.beacon_id, None);
    assert_eq!(snapshot.entries[0].state, RallyState::OnMyWay);
    assert_eq!(
        snapshot.entries[0].user_id, f.peer,
        "body cannot name another actor"
    );
    // Unknown virtual coordinates and actor fields cannot enter this closed command schema.
    assert!(serde_json::from_value::<SetRally>(serde_json::json!({"expected_revision":1,"meeting_revision":1,"state":"HERE","physical_presence_acknowledged":true,"user_id":f.owner})).is_err());
    assert!(serde_json::from_value::<SetRally>(serde_json::json!({"expected_revision":1,"meeting_revision":1,"state":"HERE","physical_presence_acknowledged":true,"world_position":{"x":12,"y":34}})).is_err());
    let mut no_ack = input(1, 1, RallyState::Here);
    no_ack.physical_presence_acknowledged = false;
    assert_eq!(
        rally.execute(q, Uuid::new_v4(), no_ack, &k).await,
        Err(CommandError::InvalidInput)
    );
    let (a, b) = tokio::join!(
        rally.execute(q, Uuid::new_v4(), input(1, 1, RallyState::Here), &k),
        rally.execute(q, Uuid::new_v4(), input(1, 1, RallyState::NeedHelp), &k)
    );
    let results = [a?, b?];
    assert_eq!(
        results
            .iter()
            .filter(|r| r.state == ReceiptState::Succeeded)
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|r| r.failure == Some(SafeFailure::VersionConflict))
            .count(),
        1
    );
    // Venue/meeting movement fences old guidance without moving anybody physically.
    sqlx::query("update meeting_fixture_authority set meeting_revision=2 where event=$1")
        .bind(f.event)
        .execute(&pool)
        .await?;
    let snapshot = rally.read(q).await?;
    assert_eq!(snapshot.meeting_revision, 2);
    assert_eq!(
        snapshot.entries[0].meeting_revision, 1,
        "old report remains explicitly stale"
    );
    let stale = rally
        .execute(q, Uuid::new_v4(), input(2, 1, RallyState::Met), &k)
        .await?;
    assert_eq!(stale.failure, Some(SafeFailure::VersionConflict));
    let fresh = rally
        .execute(q, Uuid::new_v4(), input(2, 2, RallyState::Here), &k)
        .await?;
    assert_eq!(fresh.reference.unwrap().revision, 3);
    assert_eq!(rally.read(q).await?.entries[0].meeting_revision, 2);
    // Removed membership blocks reads, writes AND safe-receipt replay immediately.
    sqlx::query("update meeting_fixture_authority set members=$2 where event=$1")
        .bind(f.event)
        .bind(vec![f.owner])
        .execute(&pool)
        .await?;
    assert_eq!(rally.read(q).await, Err(MeetingError::ResourceUnavailable));
    assert_eq!(
        rally
            .execute(q, key, input(0, 1, RallyState::OnMyWay), &k)
            .await,
        Err(CommandError::Denied)
    );
    let owner = RallyQuery {
        actor: f.actor(f.owner),
        party: f.party,
    };
    assert!(rally.read(owner).await?.entries.is_empty());
    // Reapproval cannot restore earlier physical reports; new membership starts with absent expected_revision=0.
    sqlx::query(
        "update meeting_fixture_authority set members=$2,member_generation=2 where event=$1",
    )
    .bind(f.event)
    .bind(vec![f.owner, f.peer])
    .execute(&pool)
    .await?;
    assert!(rally.read(q).await?.entries.is_empty());
    let fresh = rally
        .execute(q, Uuid::new_v4(), input(0, 2, RallyState::NeedHelp), &k)
        .await?;
    assert_eq!(fresh.reference.unwrap().revision, 4);
    assert_eq!(rally.read(q).await?.entries[0].state, RallyState::NeedHelp);
    pool.close().await;
    Ok(())
}
#[tokio::test]
async fn rally_filters_blocked_peers_and_checks_database_expiry() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let f = setup(&pool).await?;
    let rally = PgRally::new(pool.clone(), SyntheticAuthority);
    let k = keys();
    let q = RallyQuery {
        actor: f.actor(f.peer),
        party: f.party,
    };
    rally
        .execute(q, Uuid::new_v4(), input(0, 1, RallyState::Here), &k)
        .await?;
    sqlx::query("update meeting_fixture_authority set blocked=$2 where event=$1")
        .bind(f.event)
        .bind(vec![f.peer])
        .execute(&pool)
        .await?;
    let owner = RallyQuery {
        actor: f.actor(f.owner),
        party: f.party,
    };
    assert!(rally.read(owner).await?.entries.is_empty());
    sqlx::query("update meeting_fixture_authority set blocked='{}',ends_at=clock_timestamp()-interval '1 second' where event=$1").bind(f.event).execute(&pool).await?;
    assert_eq!(
        rally.read(owner).await,
        Err(MeetingError::ResourceUnavailable)
    );
    sqlx::query("update meeting_fixture_authority set ends_at=clock_timestamp()+interval '1 hour' where event=$1").bind(f.event).execute(&pool).await?;
    sqlx::query(
        "update concert_events set phase='READ_ONLY',revision=revision+1 where event_id=$1",
    )
    .bind(f.event)
    .execute(&pool)
    .await?;
    let mut tx = pool.begin().await?;
    assert_eq!(
        concert_service::meeting::rally::set_in(
            &mut tx,
            &SyntheticAuthority,
            q,
            &input(1, 1, RallyState::Met)
        )
        .await,
        Err(MeetingError::EventReadOnly)
    );
    tx.rollback().await?;
    pool.close().await;
    Ok(())
}
