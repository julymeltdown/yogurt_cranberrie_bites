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
use concert_application::report::ReportFilter;
use concert_domain::report::*;
fn input() -> CreateReport {
    CreateReport {
        target_type: TargetType::Post,
        target_id: Uuid::new_v4(),
        reason: Reason::Privacy,
        text: "private safety evidence".into(),
        evidence_media_ids: vec![Uuid::new_v4()],
    }
}
fn filter() -> ReportFilter {
    ReportFilter {
        limit: 100,
        ..Default::default()
    }
}
#[tokio::test]
async fn duplicate_concurrency_replay_and_private_projections() -> anyhow::Result<()> {
    let (p, a) = fixture().await?;
    let repo = PgReports::new(p.clone(), authority(Role::Member), Evidence { mode: 0 });
    let key = Uuid::new_v4();
    let input = input();
    let keys = keys();
    let (x, y) = tokio::join!(
        repo.create(a, key, input.clone(), &keys),
        repo.create(a, key, input.clone(), &keys)
    );
    let receipt = x?;
    assert_eq!(receipt, y?);
    assert_eq!(receipt.state, ReceiptState::Succeeded);
    let rows = repo.own(a, filter()).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(receipt.reference.unwrap().resource, rows[0].report_id);
    assert_ne!(rows[0].target_id, rows[0].report_id);
    let public = serde_json::to_value(&rows[0])?;
    for key in ["reporter_id", "reason", "text", "evidence_media_ids"] {
        assert!(public.get(key).is_none());
    }
    assert_eq!(count(&p, "concert_outbox", a.event).await, 1);
    assert_eq!(count(&p, "concert_operations_audit", a.event).await, 1);
    let dup = repo.create(a, Uuid::new_v4(), input.clone(), &keys).await?;
    assert_eq!(dup.failure, Some(SafeFailure::ResourceUnavailable));
    let mut changed = input;
    changed.text.push('x');
    assert_eq!(
        repo.create(a, key, changed, &keys).await.unwrap_err(),
        CommandError::Conflict
    );
    assert!(repo.detail(a, rows[0].report_id).await.is_err());
    assert!(repo.queue(a, filter()).await.is_err());
    let privileged = PgReports::new(p.clone(), authority(Role::Moderator), Evidence { mode: 0 });
    let detail = privileged.detail(a, rows[0].report_id).await?.unwrap();
    assert_eq!(detail.reporter_id, a.actor);
    assert_eq!(detail.text, "private safety evidence");
    assert_eq!(privileged.queue(a, filter()).await?.len(), 1);
    let other = Actor {
        actor: Uuid::new_v4(),
        event: a.event,
    };
    sqlx::query("insert into be36_test_authority values($1,$2,true,1)")
        .bind(other.actor)
        .bind(other.event)
        .execute(&p)
        .await?;
    assert!(repo.own(other, filter()).await?.is_empty());
    let foreign = Actor {
        event: Uuid::new_v4(),
        ..a
    };
    assert!(privileged.detail(foreign, rows[0].report_id).await.is_err());
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn current_authority_evidence_failure_and_revoked_replay() -> anyhow::Result<()> {
    let (p, a) = fixture().await?;
    let keys = keys();
    for mode in [1, 2, 3, 4] {
        let repo = PgReports::new(
            p.clone(),
            TestAuthority {
                role: Role::Member,
                mode,
            },
            Evidence { mode: 0 },
        );
        assert!(repo
            .create(a, Uuid::new_v4(), input(), &keys)
            .await
            .is_err());
    }
    for mode in [1, 2, 3] {
        let repo = PgReports::new(p.clone(), authority(Role::Member), Evidence { mode });
        assert!(repo
            .create(a, Uuid::new_v4(), input(), &keys)
            .await
            .is_err());
    }
    assert_eq!(count(&p, "concert_reports", a.event).await, 0);
    assert_eq!(count(&p, "concert_command_receipts", a.event).await, 0);
    let unavailable = PgReports::new(p.clone(), UnavailableAuthority, UnavailableEvidence);
    assert_eq!(
        unavailable
            .create(a, Uuid::new_v4(), input(), &keys)
            .await
            .unwrap_err(),
        CommandError::Unavailable
    );
    let repo = PgReports::new(p.clone(), authority(Role::Member), Evidence { mode: 0 });
    let key = Uuid::new_v4();
    let body = input();
    repo.create(a, key, body.clone(), &keys).await?;
    sqlx::query("update be36_test_authority set allowed=false,revision=revision+1 where actor=$1 and event=$2").bind(a.actor).bind(a.event).execute(&p).await?;
    assert_eq!(
        repo.create(a, key, body, &keys).await.unwrap_err(),
        CommandError::Denied
    );
    assert!(repo.own(a, filter()).await.is_err());
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn report_quota_and_fault_roll_back_receipt_effect_audit_outbox() -> anyhow::Result<()> {
    let (p, a) = fixture().await?;
    let repo = PgReports::new(p.clone(), authority(Role::Member), Evidence { mode: 0 });
    let keys = keys();
    for _ in 0..10 {
        assert_eq!(
            repo.create(a, Uuid::new_v4(), input(), &keys).await?.state,
            ReceiptState::Succeeded
        );
    }
    assert_eq!(
        repo.create(a, Uuid::new_v4(), input(), &keys)
            .await
            .unwrap_err(),
        CommandError::Capacity
    );
    assert_eq!(count(&p, "concert_reports", a.event).await, 10);
    assert_eq!(count(&p, "concert_command_receipts", a.event).await, 10);
    let (p2, a2) = fixture().await?;
    fail_audit(&p2, a2).await?;
    let repo = PgReports::new(p2.clone(), authority(Role::Member), Evidence { mode: 0 });
    assert!(repo
        .create(a2, Uuid::new_v4(), input(), &keys)
        .await
        .is_err());
    for table in [
        "concert_reports",
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
async fn authority_revocation_waits_for_held_owner_fence() -> anyhow::Result<()> {
    let (p, a) = fixture().await?;
    let mut tx = p.begin().await?;
    let q = AuthorityQuery {
        actor: a,
        action: Action::CreateReport,
        target: Uuid::new_v4(),
    };
    let now = Utc::now();
    authority(Role::Member).check_in(&mut tx, q, now).await?;
    let mut revoke = p.begin().await?;
    sqlx::query("set local lock_timeout='60ms'")
        .execute(&mut *revoke)
        .await?;
    assert!(sqlx::query(
        "update be36_test_authority set allowed=false where actor=$1 and event=$2"
    )
    .bind(a.actor)
    .bind(a.event)
    .execute(&mut *revoke)
    .await
    .is_err());
    revoke.rollback().await?;
    tx.commit().await?;
    sqlx::query("update be36_test_authority set allowed=false where actor=$1 and event=$2")
        .bind(a.actor)
        .bind(a.event)
        .execute(&p)
        .await?;
    let repo = PgReports::new(p.clone(), authority(Role::Member), Evidence { mode: 0 });
    assert_eq!(
        repo.create(a, Uuid::new_v4(), input(), &keys())
            .await
            .unwrap_err(),
        CommandError::Denied
    );
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn safety_readonly_allowed_archive_denied() -> anyhow::Result<()> {
    let (p, a) = fixture().await?;
    let repo = PgReports::new(p.clone(), authority(Role::Member), Evidence { mode: 0 });
    sqlx::query(
        "update concert_events set phase='READ_ONLY',revision=revision+1 where event_id=$1",
    )
    .bind(a.event)
    .execute(&p)
    .await?;
    assert_eq!(
        repo.create(a, Uuid::new_v4(), input(), &keys())
            .await?
            .state,
        ReceiptState::Succeeded
    );
    sqlx::query("update concert_events set phase='ARCHIVED',revision=revision+1 where event_id=$1")
        .bind(a.event)
        .execute(&p)
        .await?;
    assert_eq!(
        repo.create(a, Uuid::new_v4(), input(), &keys())
            .await?
            .failure,
        Some(SafeFailure::EventReadOnly)
    );
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn completed_receipt_survives_submission_media_loss_but_privileged_audit_failure_withholds_data(
) -> anyhow::Result<()> {
    let (p, a) = fixture().await?;
    let key = Uuid::new_v4();
    let body = input();
    let keys = keys();
    let repo = PgReports::new(p.clone(), authority(Role::Member), Evidence { mode: 0 });
    let receipt = repo.create(a, key, body.clone(), &keys).await?;
    let unavailable = PgReports::new(p.clone(), authority(Role::Member), UnavailableEvidence);
    assert_eq!(unavailable.create(a, key, body, &keys).await?, receipt);
    fail_audit(&p, a).await?;
    let privileged = PgReports::new(p.clone(), authority(Role::Moderator), Evidence { mode: 0 });
    assert!(privileged
        .detail(a, receipt.reference.unwrap().resource)
        .await
        .is_err());
    assert!(privileged.queue(a, filter()).await.is_err());
    assert_eq!(count(&p, "concert_operations_audit", a.event).await, 1);
    p.close().await;
    Ok(())
}
