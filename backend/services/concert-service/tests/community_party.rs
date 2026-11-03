//! Native community tests. All authority rows are synthetic and are never production grants.
use async_trait::async_trait;
use chrono::{Duration, Utc};
use concert_application::{commands::*, community::PartyCommand};
use concert_domain::community::party::*;
use concert_service::community::*;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgConnection, PgPool,
};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::{path::PathBuf, str::FromStr};
use uuid::Uuid;
async fn owned_pool() -> anyhow::Result<PgPool> {
    let root = PathBuf::from(
        std::env::var("CONCERT_TEST_OWNER_ROOT")
            .map_err(|_| anyhow::anyhow!("owned fixture root required"))?,
    );
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        info.is_dir() && !info.file_type().is_symlink() && info.permissions().mode() & 0o077 == 0,
        "private owned root required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == std::env::var("CONCERT_TEST_OWNER_IDENTITY")?,
        "fixture inode changed"
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
        marker["token"].as_str() == Some(std::env::var("CONCERT_TEST_OWNER_TOKEN")?.as_str())
            && marker["uid"].as_u64() == Some(info.uid() as u64),
        "fixture owner mismatch"
    );
    let options = PgConnectOptions::from_str(&std::env::var("CONCERT_TEST_DATABASE_URL")?)
        .map_err(|_| anyhow::anyhow!("invalid owned fixture options"))?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_community")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await
        .map_err(|_| anyhow::anyhow!("owned fixture connection failed"))?;
    let actual: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        actual == root.join("data").to_string_lossy(),
        "foreign data directory refused"
    );
    Ok(pool)
}
struct SyntheticAuthority;
#[async_trait]
impl CommunityAuthority for SyntheticAuthority {
    async fn actor_in(
        &self,
        c: &mut PgConnection,
        actor: Uuid,
        event: Uuid,
        access: concert_service::community::CommunityAccess,
    ) -> Result<(), CommandError> {
        self.authorize_in(c, actor, event, actor, access).await
    }

    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        actor: Uuid,
        event: Uuid,
        _: Uuid,
        _: concert_service::community::CommunityAccess,
    ) -> Result<(), CommandError> {
        let allowed: Option<bool> = sqlx::query_scalar(
            "select allowed from community_test_authority where actor=$1 and event=$2 for share",
        )
        .bind(actor)
        .bind(event)
        .fetch_optional(c)
        .await
        .map_err(|_| CommandError::Unavailable)?;
        if allowed == Some(true) {
            Ok(())
        } else {
            Err(CommandError::Denied)
        }
    }
    async fn place_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        place: Uuid,
    ) -> Result<(), CommandError> {
        let allowed: Option<bool> = sqlx::query_scalar(
            "select allowed from community_test_places where event=$1 and place=$2 for share",
        )
        .bind(event)
        .bind(place)
        .fetch_optional(c)
        .await
        .map_err(|_| CommandError::Unavailable)?;
        if allowed == Some(true) {
            Ok(())
        } else {
            Err(CommandError::Denied)
        }
    }
}
fn keys() -> FingerprintKeys {
    FingerprintKeys::new(
        SigningKey::new("owned-test-only", &[82; 32]).unwrap(),
        vec![],
    )
    .unwrap()
}
fn fields(place: Uuid) -> PartyFields {
    let now = Utc::now();
    PartyFields {
        title: "Owned party".into(),
        description: "Public description".into(),
        ticket_section: None,
        capacity: 2,
        meeting_starts_at: now + Duration::hours(1),
        meeting_ends_at: now + Duration::hours(2),
        requirements: String::new(),
        public_place_id: place,
    }
}
async fn fixture() -> anyhow::Result<(PgPool, Uuid, Uuid, Uuid)> {
    let pool = owned_pool().await?;
    let event = Uuid::new_v4();
    let owner = Uuid::new_v4();
    let place = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Owned community','OPEN')").bind(event).bind(format!("community-{event}")).execute(&pool).await?;
    allow(&pool, event, owner).await?;
    sqlx::query("insert into community_test_places(event,place,allowed) values($1,$2,true)")
        .bind(event)
        .bind(place)
        .execute(&pool)
        .await?;
    Ok((pool, event, owner, place))
}
async fn allow(pool: &PgPool, event: Uuid, actor: Uuid) -> anyhow::Result<()> {
    sqlx::query("insert into community_test_authority(event,actor,allowed) values($1,$2,true) on conflict(event,actor) do nothing").bind(event).bind(actor).execute(pool).await?;
    Ok(())
}
fn command(actor: Uuid, event: Uuid, party: Uuid, action: PartyAction) -> PartyCommand {
    PartyCommand {
        actor,
        event,
        party,
        action,
        key: Uuid::new_v4(),
    }
}
async fn create(pool: &PgPool, event: Uuid, owner: Uuid, place: Uuid) -> anyhow::Result<Uuid> {
    let service = PgCommunity::new(pool.clone(), SyntheticAuthority);
    let c = command(
        owner,
        event,
        Uuid::nil(),
        PartyAction::Create(fields(place)),
    );
    let id = c.party_id();
    service
        .execute_party(&c, &keys())
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    Ok(id)
}
async fn apply(pool: &PgPool, event: Uuid, user: Uuid, party: Uuid) -> anyhow::Result<Uuid> {
    allow(pool, event, user).await?;
    let c = command(
        user,
        event,
        party,
        PartyAction::Apply {
            party_revision: 1,
            note: "private application".into(),
        },
    );
    let id = c.target();
    PgCommunity::new(pool.clone(), SyntheticAuthority)
        .execute_party(&c, &keys())
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    Ok(id)
}
#[tokio::test]
#[ignore = "requires marker-owned native cluster"]
async fn community_last_slot_race_and_replay() -> anyhow::Result<()> {
    let (pool, event, owner, place) = fixture().await?;
    let party = create(&pool, event, owner, place).await?;
    let a = apply(&pool, event, Uuid::new_v4(), party).await?;
    let b = apply(&pool, event, Uuid::new_v4(), party).await?;
    let ca = command(
        owner,
        event,
        party,
        PartyAction::Decide {
            application: a,
            expected: 1,
            party_revision: 1,
            approve: true,
        },
    );
    let cb = command(
        owner,
        event,
        party,
        PartyAction::Decide {
            application: b,
            expected: 1,
            party_revision: 1,
            approve: true,
        },
    );
    let service = PgCommunity::new(pool.clone(), SyntheticAuthority);
    let keys = keys();
    let (ra, rb) = tokio::join!(
        service.execute_party(&ca, &keys),
        service.execute_party(&cb, &keys)
    );
    assert_eq!(usize::from(ra.is_ok()) + usize::from(rb.is_ok()), 1);
    let (winner, receipt, loser) = if let Ok(r) = ra {
        (&ca, r, b)
    } else {
        (&cb, rb.unwrap(), a)
    };
    assert_eq!(service.execute_party(winner, &keys).await.unwrap(), receipt);
    let full = command(
        owner,
        event,
        party,
        PartyAction::Decide {
            application: loser,
            expected: 1,
            party_revision: 2,
            approve: true,
        },
    );
    assert_eq!(
        service.execute_party(&full, &keys).await,
        Err(CommunityError::Party(PartyError::Full))
    );
    assert_eq!(
        service.execute_party(&full, &keys).await,
        Err(CommunityError::Party(PartyError::Full))
    );
    let mut changed = full;
    changed.action = PartyAction::Decide {
        application: loser,
        expected: 1,
        party_revision: 999,
        approve: true,
    };
    assert_eq!(
        service.execute_party(&changed, &keys).await,
        Err(CommunityError::Command(CommandError::Conflict))
    );
    let count: i32 = sqlx::query_scalar(
        "select member_count from concert_community_parties where event_id=$1 and party_id=$2",
    )
    .bind(event)
    .bind(party)
    .fetch_one(&pool)
    .await?;
    assert_eq!(count, 2);
    let receipts: i64 =
        sqlx::query_scalar("select count(*) from concert_command_receipts where event_id=$1")
            .bind(event)
            .fetch_one(&pool)
            .await?;
    assert_eq!(receipts, 6); // last-slot VERSION_CONFLICT and PARTY_FULL are durable failed receipts
    let outbox: i64 = sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
        .bind(event)
        .fetch_one(&pool)
        .await?;
    assert_eq!(outbox, 4);
    Ok(())
}
#[tokio::test]
#[ignore = "requires marker-owned native cluster"]
async fn community_apply_duplicate_conflict_isolation_and_policy_outage() -> anyhow::Result<()> {
    let (pool, event, owner, place) = fixture().await?;
    let party = create(&pool, event, owner, place).await?;
    let user = Uuid::new_v4();
    allow(&pool, event, user).await?;
    let mut c = command(
        user,
        event,
        party,
        PartyAction::Apply {
            party_revision: 1,
            note: "secret note".into(),
        },
    );
    let service = PgCommunity::new(pool.clone(), SyntheticAuthority);
    let keys = keys();
    let first = service.execute_party(&c, &keys).await.unwrap();
    assert_eq!(service.execute_party(&c, &keys).await.unwrap(), first);
    c.action = PartyAction::Apply {
        party_revision: 1,
        note: "changed".into(),
    };
    assert_eq!(
        service.execute_party(&c, &keys).await,
        Err(CommunityError::Command(CommandError::Conflict))
    );
    c.key = Uuid::new_v4();
    assert_eq!(
        service.execute_party(&c, &keys).await,
        Err(CommunityError::Party(PartyError::DuplicateApplication))
    );
    c.event = Uuid::new_v4();
    assert_eq!(
        service.execute_party(&c, &keys).await,
        Err(CommunityError::Command(CommandError::Denied))
    );
    let outage = PgCommunity::new(pool.clone(), UnavailableAuthority);
    assert_eq!(
        outage.party(user, event, party).await,
        Err(CommunityError::Command(CommandError::Unavailable))
    );
    sqlx::query("update community_test_authority set allowed=false where actor=$1 and event=$2")
        .bind(user)
        .bind(event)
        .execute(&pool)
        .await?;
    c.event = event;
    assert_eq!(
        service.execute_party(&c, &keys).await,
        Err(CommunityError::Command(CommandError::Denied))
    );
    Ok(())
}
#[tokio::test]
#[ignore = "requires marker-owned native cluster"]
async fn community_removal_revokes_before_outbox_delivery_and_safety_read_only(
) -> anyhow::Result<()> {
    let (pool, event, owner, place) = fixture().await?;
    let party = create(&pool, event, owner, place).await?;
    let user = Uuid::new_v4();
    let a = apply(&pool, event, user, party).await?;
    let service = PgCommunity::new(pool.clone(), SyntheticAuthority);
    let keys = keys();
    let approve = command(
        owner,
        event,
        party,
        PartyAction::Decide {
            application: a,
            expected: 1,
            party_revision: 1,
            approve: true,
        },
    );
    service.execute_party(&approve, &keys).await.unwrap();
    service.party_audience(user, event, party, 1).await.unwrap();
    sqlx::query(
        "update concert_events set phase='READ_ONLY',revision=revision+1 where event_id=$1",
    )
    .bind(event)
    .execute(&pool)
    .await?;
    let leave = command(
        user,
        event,
        party,
        PartyAction::Remove {
            user,
            expected: 1,
            party_revision: 2,
        },
    );
    let receipt = service.execute_party(&leave, &keys).await.unwrap();
    assert_eq!(service.execute_party(&leave, &keys).await.unwrap(), receipt);
    assert_eq!(
        service.party_audience(user, event, party, 1).await,
        Err(CommunityError::Command(CommandError::Denied))
    );
    let cancel = command(owner, event, party, PartyAction::Cancel { expected: 3 });
    service.execute_party(&cancel, &keys).await.unwrap();
    assert_eq!(
        service.party_audience(owner, event, party, 2).await,
        Err(CommunityError::Command(CommandError::Denied))
    );
    Ok(())
}
#[tokio::test]
#[ignore = "requires marker-owned native cluster"]
async fn community_owner_place_and_revision_are_enforced() -> anyhow::Result<()> {
    let (pool, event, owner, place) = fixture().await?;
    let party = create(&pool, event, owner, place).await?;
    let other = Uuid::new_v4();
    allow(&pool, event, other).await?;
    let service = PgCommunity::new(pool.clone(), SyntheticAuthority);
    let keys = keys();
    assert_eq!(
        service
            .execute_party(
                &command(other, event, party, PartyAction::Cancel { expected: 1 }),
                &keys
            )
            .await,
        Err(CommunityError::Command(CommandError::Denied))
    );
    let update = command(
        owner,
        event,
        party,
        PartyAction::Update {
            expected: 1,
            fields: fields(Uuid::new_v4()),
            state: PartyState::Open,
        },
    );
    assert_eq!(
        service.execute_party(&update, &keys).await,
        Err(CommunityError::Command(CommandError::Denied))
    );
    assert_eq!(
        service.party(owner, event, party).await.unwrap().revision,
        1
    );
    let orphan = command(
        owner,
        event,
        party,
        PartyAction::Remove {
            user: owner,
            expected: 1,
            party_revision: 1,
        },
    );
    assert_eq!(
        service.execute_party(&orphan, &keys).await,
        Err(CommunityError::Party(PartyError::InvalidTransition))
    );
    Ok(())
}
#[tokio::test]
#[ignore = "requires marker-owned native cluster"]
async fn community_restart_replay() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let event = Uuid::from_u128(9001);
    let owner = Uuid::from_u128(9002);
    let place = Uuid::from_u128(9003);
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,'community-restart','Restart','OPEN') on conflict(event_id) do nothing").bind(event).execute(&pool).await?;
    allow(&pool, event, owner).await?;
    sqlx::query("insert into community_test_places(event,place,allowed) values($1,$2,true) on conflict do nothing").bind(event).bind(place).execute(&pool).await?;
    let mut f = fields(place);
    f.meeting_starts_at = "2090-01-01T00:00:00Z".parse()?;
    f.meeting_ends_at = "2090-01-02T00:00:00Z".parse()?;
    let c = PartyCommand {
        actor: owner,
        event,
        key: Uuid::from_u128(9004),
        party: Uuid::nil(),
        action: PartyAction::Create(f),
    };
    let service = PgCommunity::new(pool.clone(), SyntheticAuthority);
    let receipt = service.execute_party(&c, &keys()).await.unwrap();
    assert_eq!(receipt.reference.unwrap().revision, 1);
    let count: i64 =
        sqlx::query_scalar("select count(*) from concert_command_receipts where event_id=$1")
            .bind(event)
            .fetch_one(&pool)
            .await?;
    assert_eq!(count, 1);
    Ok(())
}
