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

use concert_application::{
    admin::places::PlaceChange,
    place::{PlaceRepository, SelectionOutcome},
};
use concert_domain::{place::*, EventId};
use concert_service::adapters::out::repository::place::{PgPlaceOperations, PgPlaceRepository};
use sha2::Digest;
fn release_bytes(revision: i64, place: &Place) -> anyhow::Result<Vec<u8>> {
    let value: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../contracts/fixtures/assets/map-manifest.json"
    ))?;
    let mut object = value.as_object().unwrap().clone();
    object.remove("content_hash");
    object.insert(
        "map_id".into(),
        format!("be38_{}", place.id.uuid().simple()).into(),
    );
    object.insert("map_revision".into(), revision.into());
    object.insert(
        "places".into(),
        serde_json::Value::Array(vec![serde_json::json!({
            "place_id": place.id.uuid().to_string(),
            "label": place.label,
            "place_kind": place.kind.as_str(),
            "verification_state": place.verification.as_str(),
            "closure_state": place.closure.as_str(),
            "accessibility_notes": place.accessibility_notes,
            "provenance": serde_json::json!({
                "source_id": place.provenance.source_id,
                "source_kind": place.provenance.source_kind.as_str(),
                "source_revision": place.provenance.source_revision,
                "source_uri": place.provenance.source_uri,
                "author": place.provenance.author,
                "license": place.provenance.license,
                "review_state": place.provenance.review_state.as_str(),
                "reviewed_by": place.provenance.reviewed_by,
                "reviewed_at": place.provenance.reviewed_at.map(|at| at.to_rfc3339()),
                "photo_permission": place.provenance.photo_permission.as_str(),
            })
        })]),
    );
    object["geometry"]["interactions"][0]["place_id"] = place.id.uuid().to_string().into();
    let geometry = canonical_json_bytes(&serde_json::to_vec(&object["geometry"])?)?;
    object.insert(
        "geometry_hash".into(),
        format!("sha256:{:x}", sha2::Sha256::digest(&geometry)).into(),
    );
    let digest = format!(
        "sha256:{}",
        sha2::Sha256::digest(serde_json::to_vec(&serde_json::Value::Object(
            object.clone()
        ))?)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect::<String>()
    );
    object.insert("content_hash".into(), digest.into());
    let bytes = serde_json::to_vec(&serde_json::Value::Object(object))?;
    Ok(bytes)
}
async fn place_fixture() -> anyhow::Result<(PgPool, Actor, Place, CatalogRelease)> {
    let (p, a) = fixture().await?;
    let place = Place::new(
        PlaceId::from_uuid(Uuid::new_v4())?,
        "Synthetic reviewed physical reference".into(),
        PlaceKind::Physical,
        VerificationState::Verified,
        ClosureState::Open,
        "Synthetic accessibility note; no venue qualification".into(),
        Provenance::new(
            "be38_place_fixture".into(),
            SourceKind::PlaceReference,
            "synthetic-1".into(),
            Some("https://example.org/synthetic-place".into()),
            "Synthetic reviewer".into(),
            "Synthetic metadata only".into(),
            ReviewState::Reviewed,
            Some("test-reviewer".into()),
            Some(Utc::now() - chrono::Duration::seconds(1)),
            PhotoPermission::NotApplicable,
        )?,
    )?;
    let bytes = release_bytes(1, &place)?;
    let release = CatalogRelease::new(parse_stored_release(&bytes)?, bytes)?;
    PgPlaceRepository::new(p.clone())
        .publish_release(EventId::from_uuid(a.event), release.clone())
        .await?;
    Ok((p, a, place, release))
}
fn change(place: &Place, revision: i64) -> EventCommand {
    let mut place = place.clone();
    place.closure = ClosureState::Closed;
    EventCommand::Place(PlaceChange {
        place,
        expected: Revision::new(revision).unwrap(),
        reason: "Checked synthetic closure".into(),
    })
}
#[tokio::test]
async fn place_current_revision_cas_immediately_fences_selection_and_preserves_release(
) -> anyhow::Result<()> {
    let (p, a, place, _) = place_fixture().await?;
    let owner = PgPlaceRepository::new(p.clone());
    let repo = PgEventOperations::new(
        p.clone(),
        authority(Role::Operator),
        UnavailableReview,
        PgPlaceOperations::new(authority(Role::Operator)),
    );
    let keys = keys();
    let (x, y) = tokio::join!(
        repo.execute(
            a,
            Uuid::new_v4(),
            Some(place.id.uuid()),
            change(&place, 1),
            &keys
        ),
        repo.execute(
            a,
            Uuid::new_v4(),
            Some(place.id.uuid()),
            change(&place, 1),
            &keys
        )
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
    assert_eq!(
        owner
            .select_physical(
                EventId::from_uuid(a.event),
                place.id,
                Revision::initial(),
                1
            )
            .await?,
        SelectionOutcome::Closed
    );
    let rows = repo.places(a, None, Some(true), 20).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].revision.get(), 2);
    assert_eq!(rows[0].place.provenance, place.provenance);
    let mut changed = place.clone();
    changed.accessibility_notes = "Unpublished replacement".into();
    let response = repo
        .execute(
            a,
            Uuid::new_v4(),
            Some(place.id.uuid()),
            EventCommand::Place(PlaceChange {
                place: changed,
                expected: Revision::new(2)?,
                reason: "Needs reviewed release".into(),
            }),
            &keys,
        )
        .await?;
    assert_eq!(response.failure, Some(SafeFailure::MapVersionMismatch));
    let mut original = place.clone();
    original.closure = ClosureState::Open;
    let key = Uuid::new_v4();
    let command = EventCommand::Place(PlaceChange {
        place: original,
        expected: Revision::new(2)?,
        reason: "Checked reopen".into(),
    });
    let receipt = repo
        .execute(a, key, Some(place.id.uuid()), command.clone(), &keys)
        .await?;
    assert_eq!(receipt.reference.unwrap().revision, 3);
    assert_eq!(
        repo.execute(a, key, Some(place.id.uuid()), command, &keys)
            .await?,
        receipt
    );
    assert!(matches!(
        owner
            .select_physical(
                EventId::from_uuid(a.event),
                place.id,
                Revision::initial(),
                1
            )
            .await?,
        SelectionOutcome::Eligible(_)
    ));
    assert_eq!(count(&p, "concert_outbox", a.event).await, 2);
    let (other_pool, other) = fixture().await?;
    // Even after reopening, a later event cannot inherit this event's operated
    // release-local closure/revision. Owner publication itself must refuse it.
    let bytes = release_bytes(1, &place)?;
    let original_release = CatalogRelease::new(parse_stored_release(&bytes)?, bytes)?;
    assert!(owner
        .publish_release(EventId::from_uuid(other.event), original_release)
        .await
        .is_err());
    let mut closed = place.clone();
    closed.closure = ClosureState::Closed;
    let bytes = release_bytes(2, &closed)?;
    let replacement = CatalogRelease::new(parse_stored_release(&bytes)?, bytes)?;
    owner
        .publish_release(EventId::from_uuid(a.event), replacement)
        .await?;
    // A delayed OPEN at revision1 from the old release must not match revision1
    // of a new CLOSED release. Rebound writes fail closed pending monotonic owner.
    let stale = EventCommand::Place(PlaceChange {
        place: place.clone(),
        expected: Revision::initial(),
        reason: "Stale old release reopen".into(),
    });
    assert_eq!(
        repo.execute(a, Uuid::new_v4(), Some(place.id.uuid()), stale, &keys)
            .await?
            .failure,
        Some(SafeFailure::MapVersionMismatch)
    );
    assert_eq!(
        owner
            .select_physical(EventId::from_uuid(a.event), place.id, Revision::new(2)?, 2)
            .await?,
        SelectionOutcome::Closed
    );
    other_pool.close().await;

    p.close().await;
    Ok(())
}
#[tokio::test]
async fn place_failure_rolls_back_and_shared_release_cannot_mutate_another_event(
) -> anyhow::Result<()> {
    let (p, a, place, release) = place_fixture().await?;
    let mut tx = p.begin().await?;
    let port = PgPlaceOperations::new(authority(Role::Member));
    let EventCommand::Place(input) = change(&place, 1) else {
        unreachable!()
    };
    assert_eq!(
        port.update_in(&mut tx, a, &input).await.unwrap_err(),
        CommandError::Denied
    );
    tx.rollback().await?;
    fail_audit(&p, a).await?;
    let repo = PgEventOperations::new(
        p.clone(),
        authority(Role::Operator),
        UnavailableReview,
        PgPlaceOperations::new(authority(Role::Operator)),
    );
    assert!(repo
        .execute(
            a,
            Uuid::new_v4(),
            Some(place.id.uuid()),
            change(&place, 1),
            &keys()
        )
        .await
        .is_err());
    assert!(matches!(
        PgPlaceRepository::new(p.clone())
            .select_physical(
                EventId::from_uuid(a.event),
                place.id,
                Revision::initial(),
                1
            )
            .await?,
        SelectionOutcome::Eligible(_)
    ));
    for table in [
        "concert_command_receipts",
        "concert_outbox",
        "concert_operations_audit",
    ] {
        assert_eq!(count(&p, table, a.event).await, 0);
    }
    let (p2, a2) = fixture().await?;
    PgPlaceRepository::new(p.clone())
        .publish_release(EventId::from_uuid(a2.event), release)
        .await?;
    let repo = PgEventOperations::new(
        p.clone(),
        authority(Role::Operator),
        UnavailableReview,
        PgPlaceOperations::new(authority(Role::Operator)),
    );
    let receipt = repo
        .execute(
            a2,
            Uuid::new_v4(),
            Some(place.id.uuid()),
            change(&place, 1),
            &keys(),
        )
        .await?;
    assert_eq!(receipt.failure, Some(SafeFailure::MapVersionMismatch));
    for actor in [a, a2] {
        assert!(matches!(
            PgPlaceRepository::new(p.clone())
                .select_physical(
                    EventId::from_uuid(actor.event),
                    place.id,
                    Revision::initial(),
                    1
                )
                .await?,
            SelectionOutcome::Eligible(_)
        ));
    }
    p.close().await;
    p2.close().await;
    Ok(())
}
#[tokio::test]
async fn held_selection_lock_prevents_racing_closure_until_commit() -> anyhow::Result<()> {
    let (p, a, place, _) = place_fixture().await?;
    let mut selected = p.begin().await?;
    assert!(matches!(
        PgPlaceRepository::select_physical_in_transaction(
            &mut selected,
            EventId::from_uuid(a.event),
            place.id,
            Revision::initial(),
            1
        )
        .await?,
        SelectionOutcome::Eligible(_)
    ));
    let mut closing = p.begin().await?;
    sqlx::query("set local lock_timeout='60ms'")
        .execute(&mut *closing)
        .await?;
    let port = PgPlaceOperations::new(authority(Role::Operator));
    let EventCommand::Place(input) = change(&place, 1) else {
        unreachable!()
    };
    assert!(port.update_in(&mut closing, a, &input).await.is_err());
    closing.rollback().await?;
    selected.commit().await?;
    let repo = PgEventOperations::new(
        p.clone(),
        authority(Role::Operator),
        UnavailableReview,
        PgPlaceOperations::new(authority(Role::Operator)),
    );
    assert_eq!(
        repo.execute(
            a,
            Uuid::new_v4(),
            Some(place.id.uuid()),
            change(&place, 1),
            &keys()
        )
        .await?
        .state,
        ReceiptState::Succeeded
    );
    p.close().await;
    Ok(())
}
