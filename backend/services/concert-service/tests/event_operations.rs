//! Synthetic local authority is test-only; all data lives in the marker/inode-owned harness.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{admin::event::*, admin::notices::*, admin::*, commands::*};
use concert_domain::{EventPhase, Revision};
use concert_service::adapters::out::repository::{event_operations::*, report::*};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgConnection, PgPool, Row,
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
        options.get_database() == Some("mcr_concert_be_operations")
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

#[derive(Clone, Copy)]
struct TestAuthority {
    role: Role,
    mode: u8,
}
#[async_trait]
impl FencedOperationsAuthority for TestAuthority {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        q: AuthorityQuery,
        time: DateTime<Utc>,
    ) -> Result<RoleLease, CommandError> {
        let row=sqlx::query("select allowed,revision from be36_test_authority where actor=$1 and event=$2 for share").bind(q.actor.actor).bind(q.actor.event).fetch_optional(c).await.map_err(|_|CommandError::Unavailable)?.ok_or(CommandError::Denied)?;
        if !row
            .try_get::<bool, _>("allowed")
            .map_err(|_| CommandError::Unavailable)?
        {
            return Err(CommandError::Denied);
        }
        let mut lease = RoleLease {
            query: q,
            role: self.role,
            revision: row
                .try_get("revision")
                .map_err(|_| CommandError::Unavailable)?,
            checked_at: time,
            expires_at: time + chrono::Duration::seconds(30),
        };
        match self.mode {
            1 => lease.query.actor.event = Uuid::new_v4(),
            2 => lease.expires_at = time,
            3 => lease.revision = 0,
            4 => return Err(CommandError::Unavailable),
            5 => lease.expires_at = time + chrono::Duration::milliseconds(80),
            _ => {}
        }
        Ok(lease)
    }
}
struct Evidence {
    mode: u8,
}
#[async_trait]
impl FencedReportEvidence for Evidence {
    async fn check_in(
        &self,
        _: &mut PgConnection,
        actor: Actor,
        input: &concert_domain::report::CreateReport,
        time: DateTime<Utc>,
    ) -> Result<EvidenceLease, CommandError> {
        match self.mode {
            1 => Err(CommandError::Denied),
            2 => Err(CommandError::Unavailable),
            3 => Ok(EvidenceLease {
                actor,
                input_digest: evidence_digest(input)?,
                revision: 1,
                checked_at: time,
                expires_at: time,
            }),
            _ => Ok(EvidenceLease {
                actor,
                input_digest: evidence_digest(input)?,
                revision: 1,
                checked_at: time,
                expires_at: time + chrono::Duration::seconds(30),
            }),
        }
    }
}
struct Review {
    mode: u8,
}
#[async_trait]
impl FencedNoticeReview for Review {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        actor: Actor,
        id: Uuid,
        input: &NoticeInput,
        time: DateTime<Utc>,
    ) -> Result<NoticeReviewLease, CommandError> {
        match self.mode {
            1 => Err(CommandError::Unavailable),
            2 => {
                sqlx::query("select pg_sleep(0.12)")
                    .execute(c)
                    .await
                    .map_err(|_| CommandError::Unavailable)?;
                Ok(NoticeReviewLease {
                    actor,
                    notice_id: id,
                    input_digest: notice_review_digest(input),
                    revision: 1,
                    checked_at: time,
                    expires_at: time + chrono::Duration::milliseconds(20),
                })
            }
            _ => Ok(NoticeReviewLease {
                actor,
                notice_id: id,
                input_digest: notice_review_digest(input),
                revision: 1,
                checked_at: time,
                expires_at: time + chrono::Duration::seconds(30),
            }),
        }
    }
}
async fn fixture() -> anyhow::Result<(PgPool, Actor)> {
    let pool = owned_pool().await?;
    sqlx::query("create table if not exists be36_test_authority(actor uuid,event uuid,allowed boolean not null,revision bigint not null,primary key(actor,event))").execute(&pool).await?;
    let actor = Actor {
        actor: Uuid::new_v4(),
        event: Uuid::new_v4(),
    };
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Synthetic operations fixture','OPEN')").bind(actor.event).bind(format!("be36-{}",actor.event.simple())).execute(&pool).await?;
    sqlx::query("insert into be36_test_authority values($1,$2,true,1)")
        .bind(actor.actor)
        .bind(actor.event)
        .execute(&pool)
        .await?;
    Ok((pool, actor))
}
fn keys() -> FingerprintKeys {
    FingerprintKeys::new(SigningKey::new("be36-test", &[17; 32]).unwrap(), vec![]).unwrap()
}
fn settings(enabled: bool) -> Settings {
    Settings {
        features: Features {
            parties: enabled,
            giveaways: enabled,
            intros: enabled,
            social: enabled,
            mural: enabled,
        },
        beacon_ttl_seconds: 300,
        pickup_grace_seconds: 120,
        social_grant_ttl_seconds: 600,
        mural_cooldown_ms: 250,
    }
}
fn authority(role: Role) -> TestAuthority {
    TestAuthority { role, mode: 0 }
}
async fn count(pool: &PgPool, table: &str, event: Uuid) -> i64 {
    sqlx::query_scalar(&format!("select count(*) from {table} where event_id=$1"))
        .bind(event)
        .fetch_one(pool)
        .await
        .unwrap()
}
async fn fail_audit(pool: &PgPool, actor: Actor) -> anyhow::Result<()> {
    // A session-local function/trigger is installed only on this disposable DB and
    // targets one random event; other tests' rows are untouched.
    sqlx::query(&format!("create function be36_fault_{}() returns trigger language plpgsql as $$ begin if new.event_id='{}'::uuid then raise exception 'synthetic fault'; end if; return new; end $$",actor.event.simple(),actor.event)).execute(pool).await?;
    sqlx::query(&format!("create trigger be36_fault_{} before insert on concert_operations_audit for each row execute function be36_fault_{}()",actor.event.simple(),actor.event.simple())).execute(pool).await?;
    Ok(())
}
fn command(expected: i64, enabled: bool) -> EventCommand {
    EventCommand::Settings {
        expected: Revision::new(expected).unwrap(),
        settings: settings(enabled),
        reason: "Synthetic checked change".into(),
    }
}
fn notice_input() -> NoticeInput {
    NoticeInput {
        title: "Synthetic reviewed notice".into(),
        text: "Source-grounded fixture content".into(),
        source_uri: "https://example.org/fixture".into(),
        confirmed_at: Utc::now() - chrono::Duration::seconds(1),
        state: NoticeState::Published,
    }
}
#[tokio::test]
async fn settings_concurrent_cas_receipts_and_disabled_server_gate() -> anyhow::Result<()> {
    let (p, a) = fixture().await?;
    let repo = PgEventOperations::new(
        p.clone(),
        authority(Role::Operator),
        Review { mode: 0 },
        UnavailablePlaces,
    );
    let keys = keys();
    let key = Uuid::new_v4();
    let (x, y) = tokio::join!(
        repo.execute(a, key, None, command(1, true), &keys),
        repo.execute(a, key, None, command(1, true), &keys)
    );
    let receipt = x?;
    assert_eq!(receipt, y?);
    assert_eq!(receipt.state, ReceiptState::Succeeded);
    assert_eq!(repo.settings(a).await?.event.revision.get(), 2);
    let (x, y) = tokio::join!(
        repo.execute(a, Uuid::new_v4(), None, command(2, false), &keys),
        repo.execute(a, Uuid::new_v4(), None, command(2, false), &keys)
    );
    let values = [x?, y?];
    assert_eq!(
        values
            .iter()
            .filter(|r| r.state == ReceiptState::Succeeded)
            .count(),
        1
    );
    assert_eq!(
        values
            .iter()
            .filter(|r| r.failure == Some(SafeFailure::VersionConflict))
            .count(),
        1
    );
    let mut tx = p.begin().await?;
    assert_eq!(
        require_feature_in(&mut tx, a.event, Feature::Parties, None).await?,
        Err(SafeFailure::EventReadOnly)
    );
    tx.rollback().await?;
    assert_eq!(count(&p, "concert_outbox", a.event).await, 2);
    assert_eq!(count(&p, "concert_operations_audit", a.event).await, 2);
    assert_eq!(repo.settings(a).await?.event.revision.get(), 3);
    assert_eq!(
        repo.execute(a, key, None, command(1, false), &keys)
            .await
            .unwrap_err(),
        CommandError::Conflict
    );
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn operator_required_current_lease_and_missing_feature_policy_failclosed(
) -> anyhow::Result<()> {
    let (p, a) = fixture().await?;
    let keys = keys();
    for role in [Role::Member, Role::Moderator] {
        let repo = PgEventOperations::new(
            p.clone(),
            authority(role),
            UnavailableReview,
            UnavailablePlaces,
        );
        assert_eq!(
            repo.execute(a, Uuid::new_v4(), None, command(1, true), &keys)
                .await
                .unwrap_err(),
            CommandError::Denied
        );
        assert!(repo.settings(a).await.is_err());
    }
    for mode in [1, 2, 3, 4] {
        let repo = PgEventOperations::new(
            p.clone(),
            TestAuthority {
                role: Role::Operator,
                mode,
            },
            UnavailableReview,
            UnavailablePlaces,
        );
        assert!(repo
            .execute(a, Uuid::new_v4(), None, command(1, true), &keys)
            .await
            .is_err());
    }
    let mut tx = p.begin().await?;
    assert_eq!(
        require_feature_in(&mut tx, a.event, Feature::Social, None)
            .await
            .unwrap_err(),
        CommandError::Unavailable
    );
    tx.rollback().await?;
    assert_eq!(count(&p, "concert_command_receipts", a.event).await, 0);
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn notice_publication_withdrawal_review_and_replay() -> anyhow::Result<()> {
    let (p, a) = fixture().await?;
    let repo = PgEventOperations::new(
        p.clone(),
        authority(Role::Operator),
        Review { mode: 0 },
        UnavailablePlaces,
    );
    let input = notice_input();
    let key = Uuid::new_v4();
    let keys = keys();
    let receipt = repo
        .execute(
            a,
            key,
            None,
            EventCommand::NoticeCreate(input.clone()),
            &keys,
        )
        .await?;
    let id = receipt.reference.unwrap().resource;
    assert_eq!(repo.notices(a, None, 20).await?.len(), 1);
    assert_eq!(
        repo.execute(
            a,
            key,
            None,
            EventCommand::NoticeCreate(input.clone()),
            &keys
        )
        .await?,
        receipt
    );
    let mut withdrawn = input.clone();
    withdrawn.state = NoticeState::Withdrawn;
    assert_eq!(
        repo.execute(
            a,
            Uuid::new_v4(),
            Some(id),
            EventCommand::NoticeUpdate {
                expected: Revision::initial(),
                input: withdrawn
            },
            &keys
        )
        .await?
        .state,
        ReceiptState::Succeeded
    );
    assert!(repo.notices(a, None, 20).await?.is_empty());
    assert_eq!(count(&p, "concert_outbox", a.event).await, 2);
    assert_eq!(count(&p, "concert_operations_audit", a.event).await, 2);
    let denied = PgEventOperations::new(
        p.clone(),
        authority(Role::Operator),
        UnavailableReview,
        UnavailablePlaces,
    );
    assert_eq!(
        denied
            .execute(
                a,
                Uuid::new_v4(),
                None,
                EventCommand::NoticeCreate(input.clone()),
                &keys
            )
            .await
            .unwrap_err(),
        CommandError::Unavailable
    );
    let slow = PgEventOperations::new(
        p.clone(),
        authority(Role::Operator),
        Review { mode: 2 },
        UnavailablePlaces,
    );
    assert_eq!(
        slow.execute(
            a,
            Uuid::new_v4(),
            None,
            EventCommand::NoticeCreate(input),
            &keys
        )
        .await
        .unwrap_err(),
        CommandError::Unavailable
    );
    assert_eq!(count(&p, "concert_operation_notices", a.event).await, 1);
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn settings_phase_audit_failure_is_atomic_and_phase_scope_bounded() -> anyhow::Result<()> {
    let (p, a) = fixture().await?;
    let repo = PgEventOperations::new(
        p.clone(),
        authority(Role::Operator),
        UnavailableReview,
        UnavailablePlaces,
    );
    let keys = keys();
    let receipt = repo
        .execute(
            a,
            Uuid::new_v4(),
            None,
            EventCommand::Phase {
                expected: Revision::initial(),
                phase: EventPhase::Live,
                reason: "Checked phase".into(),
            },
            &keys,
        )
        .await?;
    assert_eq!(receipt.state, ReceiptState::Succeeded);
    assert_eq!(
        repo.execute(
            a,
            Uuid::new_v4(),
            None,
            EventCommand::Phase {
                expected: Revision::new(2)?,
                phase: EventPhase::Open,
                reason: "Cannot reverse".into()
            },
            &keys
        )
        .await?
        .failure,
        Some(SafeFailure::InvalidTransition)
    );
    assert_eq!(
        repo.execute(
            a,
            Uuid::new_v4(),
            None,
            EventCommand::Phase {
                expected: Revision::new(2)?,
                phase: EventPhase::ReadOnly,
                reason: "Outside scope".into()
            },
            &keys
        )
        .await
        .unwrap_err(),
        CommandError::InvalidInput
    );
    let (p2, a2) = fixture().await?;
    fail_audit(&p2, a2).await?;
    let repo = PgEventOperations::new(
        p2.clone(),
        authority(Role::Operator),
        UnavailableReview,
        UnavailablePlaces,
    );
    assert!(repo
        .execute(a2, Uuid::new_v4(), None, command(1, true), &keys)
        .await
        .is_err());
    let revision: i64 = sqlx::query_scalar("select revision from concert_events where event_id=$1")
        .bind(a2.event)
        .fetch_one(&p2)
        .await?;
    assert_eq!(revision, 1);
    for table in [
        "concert_event_settings",
        "concert_command_receipts",
        "concert_operations_audit",
        "concert_outbox",
    ] {
        assert_eq!(count(&p2, table, a2.event).await, 0);
    }
    p.close().await;
    p2.close().await;
    Ok(())
}
#[tokio::test]
async fn expired_authority_at_commit_rolls_back_every_effect() -> anyhow::Result<()> {
    let (p, a) = fixture().await?;
    sqlx::query(&format!("create function be38_delay_{}() returns trigger language plpgsql as $$ begin if new.event_id='{}'::uuid then perform pg_sleep(0.16); end if; return new; end $$",a.event.simple(),a.event)).execute(&p).await?;
    sqlx::query(&format!("create trigger be38_delay_{} before insert on concert_operations_audit for each row execute function be38_delay_{}()",a.event.simple(),a.event.simple())).execute(&p).await?;
    let repo = PgEventOperations::new(
        p.clone(),
        TestAuthority {
            role: Role::Operator,
            mode: 5,
        },
        UnavailableReview,
        UnavailablePlaces,
    );
    assert!(repo
        .execute(a, Uuid::new_v4(), None, command(1, true), &keys())
        .await
        .is_err());
    for table in [
        "concert_event_settings",
        "concert_command_receipts",
        "concert_operations_audit",
        "concert_outbox",
    ] {
        assert_eq!(count(&p, table, a.event).await, 0);
    }
    let revision: i64 = sqlx::query_scalar("select revision from concert_events where event_id=$1")
        .bind(a.event)
        .fetch_one(&p)
        .await?;
    assert_eq!(revision, 1);
    p.close().await;
    Ok(())
}
