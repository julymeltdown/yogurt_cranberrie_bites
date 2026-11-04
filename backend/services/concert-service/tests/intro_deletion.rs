//! BE42 Intro erasure regressions; reuses the unchanged canonical Intro fixture.
//! Auth deletion-source and sealed Sharing rows below are explicit test fixtures;
//! they do not certify production Auth transport or whole BE42/IN03.
//! Actual canonical mTLS owner/SQL coverage. Only the private test verifier and
//! immutable fixture records are synthetic; no fixture can construct a grant.
#[path = "intro_product_support/capture.rs"]
mod capture;
#[path = "intro_product_support/environment.rs"]
mod environment;
#[path = "intro_product_support/fixture.rs"]
mod fixture;
#[path = "intro_product_support/owners.rs"]
mod owners;
use chrono::{DateTime, Duration, Utc};
use concert_application::{
    commands::{FingerprintKeys, SigningKey},
    Clock,
};
use concert_service::{
    adapters::out::{
        age_provider::{
            runtime::{AgeRuntime, ProviderMode},
            trust::ClaimTrust,
        },
        relationship_authority::guarded::{proto::TransactionStatus, OwnerGuardScope},
    },
    config::native_resolver::{NativeResolverDomain, NativeResolverUnion},
    intro_live::{protocol, IntroCursor, IntroRuntime},
    product_rpc::{
        proto,
        scope_ledger::{ProducerTransaction, ScopeKey},
        settlement::NativeTransactionResolver,
    },
};
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;
struct RealClock;
impl Clock for RealClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
fn runtime(f: &fixture::Fixture, trust: bool) -> IntroRuntime {
    let keys = Arc::new(
        FingerprintKeys::new(
            SigningKey::new("intro-native-fixture-v1", &[92; 32]).unwrap(),
            vec![],
        )
        .unwrap(),
    );
    let age = AgeRuntime::new(
        f.concert.clone(),
        Arc::new(RealClock),
        keys.clone(),
        ProviderMode::Unavailable,
    )
    .with_claim_trust(if trust {
        ClaimTrust::new(&[("fixture", "fixture-v1")]).unwrap()
    } else {
        ClaimTrust::none()
    });
    IntroRuntime::new(
        f.concert.clone(),
        f.reader.clone(),
        IntroCursor::new([93; 32]).unwrap(),
        keys,
        Arc::new(age),
    )
}
fn raw(
    f: &fixture::Fixture,
    method: &str,
    suffix: &str,
    body: Value,
    key: Option<Uuid>,
) -> proto::ProductRequest {
    let now = Utc::now().timestamp_micros();
    proto::ProductRequest {
        user_id: f.actor.to_string(),
        event_id: f.event.to_string(),
        session_family_id: f.family.to_string(),
        request_id: Uuid::new_v4().to_string(),
        http_method: method.into(),
        http_target: format!("/api/concert/events/{}/{}", f.event, suffix),
        http_body: if method == "GET" {
            vec![]
        } else {
            serde_json::to_vec(&body).unwrap()
        },
        content_type: if method == "GET" {
            ""
        } else {
            "application/json"
        }
        .into(),
        idempotency_key: key.map(|k| k.to_string()).unwrap_or_default(),
        authority_expires_unix_micros: now + 4_800_000,
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
        access_expires_unix_micros: now + 60_000_000,
        ..Default::default()
    }
}
async fn acquire(
    f: &fixture::Fixture,
    op: &str,
    raw: proto::ProductRequest,
) -> anyhow::Result<(
    Arc<OwnerGuardScope>,
    platform::actor_delegation::exchange::VerifiedParentAuthority,
    proto::ProductRequest,
)> {
    let (b, parent, raw) = capture::capture(raw, op).await?;
    let scope = f.fence.acquire(b).await?;
    let p = scope.producer_scope();
    f.ledger
        .bind_delivery(
            &ScopeKey {
                instance: f.instance,
                scope: Uuid::parse_str(&p.scope_id)?,
                binding_digest: p.binding_digest.as_slice().try_into()?,
            },
            &[90; 32],
        )
        .await?;
    Ok((scope, parent, raw))
}
async fn call(
    f: &fixture::Fixture,
    r: &IntroRuntime,
    op: &str,
    raw: proto::ProductRequest,
) -> anyhow::Result<(u32, Value)> {
    let (s, p, raw) = acquire(f, op, raw).await?;
    let owned = s.clone();
    let r = r.clone();
    let v = s
        .run_owned(async move { p.scope(r.dispatch_for_scope(owned, raw)).await })
        .await?;
    let body = serde_json::from_slice(&v.body)?;
    // Headless Root/Gateway terminal callback, not a real HTTP delivery claim.
    // Follow DeliveryService's exact ordering: authenticate the bound capability,
    // settle actual owners/children, then persist completion. ROOT_FINISHED alone
    // keeps a delivered scope charged against the durable 16-scope capacity.
    s.mark_serialized().await?;
    let producer = s.producer_scope();
    let key = ScopeKey {
        instance: f.instance,
        scope: Uuid::parse_str(&producer.scope_id)?,
        binding_digest: producer.binding_digest.as_slice().try_into()?,
    };
    f.ledger.verify_delivery(&key, &[90; 32]).await?;
    s.finish_response().await?;
    f.ledger.record_delivery_completed(&key).await?;
    anyhow::ensure!(f.ledger.delivery_completed(&key).await?);
    f.assert_clean().await?;
    Ok((v.status, body))
}
fn draft(rev: i64, text: &str, restricted: bool) -> Value {
    json!({"expected_revision":rev,"text":text,"age_bounds":{"minimum_age":if restricted{Some(20)}else{None},"maximum_age_exclusive":if restricted{Some(40)}else{None}},"expires_at":null})
}
async fn publish(f: &fixture::Fixture, r: &IntroRuntime, restricted: bool) -> anyhow::Result<Uuid> {
    let (status, v) = call(
        f,
        r,
        "setConcertIntro",
        raw(
            f,
            "PUT",
            "me/intro",
            draft(0, "Meet fellow fans", restricted),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    assert_eq!(status, 200, "{v}");
    let id = Uuid::parse_str(v["intro_id"].as_str().unwrap())?;
    let (status, v) = call(
        f,
        r,
        "publishConcertIntro",
        raw(
            f,
            "POST",
            "me/intro/publish",
            json!({"expected_revision":1}),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    assert_eq!(status, 200, "{v}");
    Ok(id)
}

use concert_service::{
    account_private_data::{PrivateDataCategory, PrivateDataCategoryOwner},
    intro_live::account_deletion::{IntroAccountDeletion, PgIntroConsentDeletionEvidence},
    world::account_deletion::{
        DeletionAuthority, DeletionFailure, VerifiedDeletionSource, WorldDeletionCommand,
        WorldDeletionTarget,
    },
};
use tokio::time::Instant;
struct DeletionTestSource(u8);
#[async_trait::async_trait]
impl DeletionAuthority for DeletionTestSource {
    async fn verify(
        &self,
        c: &WorldDeletionCommand,
        d: Instant,
    ) -> Result<VerifiedDeletionSource, DeletionFailure> {
        VerifiedDeletionSource::from_verified_auth_source(c, d, [self.0; 32])
    }
}
struct ChangingSource(std::sync::atomic::AtomicUsize);
#[async_trait::async_trait]
impl DeletionAuthority for ChangingSource {
    async fn verify(
        &self,
        c: &WorldDeletionCommand,
        d: Instant,
    ) -> Result<VerifiedDeletionSource, DeletionFailure> {
        let n = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        VerifiedDeletionSource::from_verified_auth_source(c, d, [if n == 0 { 74 } else { 75 }; 32])
    }
}
fn deletion(user: Uuid) -> WorldDeletionCommand {
    WorldDeletionCommand {
        saga_id: Uuid::new_v4(),
        user_id: user,
        actor_id: user,
        target: WorldDeletionTarget::PrivateData,
        retention_until: "2070-01-01T00:00:00.123456789Z".parse().unwrap(),
    }
}
fn owner(pool: sqlx::PgPool, source: u8) -> IntroAccountDeletion {
    IntroAccountDeletion::new(
        pool,
        Arc::new(DeletionTestSource(source)),
        Arc::new(PgIntroConsentDeletionEvidence),
    )
}
fn deletion_error(e: DeletionFailure) -> anyhow::Error {
    anyhow::anyhow!("Intro deletion fixture: {e:?}")
}
async fn apply(
    pool: &sqlx::PgPool,
    c: &WorldDeletionCommand,
    byte: u8,
) -> anyhow::Result<concert_service::account_private_data::PrivateDataCategoryReceipt> {
    let d = Instant::now() + std::time::Duration::from_secs(4);
    let s = DeletionTestSource(byte)
        .verify(c, d)
        .await
        .map_err(deletion_error)?;
    owner(pool.clone(), byte)
        .apply_category(c, &s, d)
        .await
        .map_err(deletion_error)
}
// Explicit Social-owner storage fixture, never a production source verifier.
// Normal 0054 insert/update/terminal guards execute; no trigger is disabled.
async fn fixture_sharing(
    pool: &sqlx::PgPool,
    c: &WorldDeletionCommand,
    completed: bool,
) -> anyhow::Result<()> {
    let mut sharing = c.clone();
    sharing.target = WorldDeletionTarget::Sharing;
    let mut tx = pool.begin().await?;
    sqlx::query("insert into concert_social_account_deletions(saga_id,user_id,actor_id,retention_seconds,retention_nanos,identity_binding,command_binding,source_binding,fence_revision) values($1,$2,$3,$4,$5,$6,$7,$8,1) on conflict do nothing")
        .bind(c.saga_id).bind(c.user_id).bind(c.actor_id).bind(c.retention_until.timestamp()).bind(c.retention_until.timestamp_subsec_nanos() as i32).bind(sharing.identity_binding().map_err(deletion_error)?.as_slice()).bind(sharing.binding().map_err(deletion_error)?.as_slice()).bind([83u8;32].as_slice()).execute(&mut *tx).await?;
    if completed {
        sqlx::query("select set_config('mcr.social_deletion_saga',$1,true),set_config('mcr.social_deletion_source',$2,true),set_config('mcr.social_deletion_until',(clock_timestamp()+interval '4 seconds')::text,true)")
            .bind(c.saga_id.to_string()).bind("53".repeat(32)).execute(&mut *tx).await?;
        sqlx::query("update concert_social_account_deletions set completed_at=clock_timestamp(),progress_revision=progress_revision+1 where saga_id=$1 and completed_at is null").bind(c.saga_id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}
async fn native(state: &str) -> anyhow::Result<()> {
    let mut f = fixture::Fixture::new().await?;
    let r = runtime(&f, false);
    let (status, value) = call(
        &f,
        &r,
        "setConcertIntro",
        raw(
            &f,
            "PUT",
            "me/intro",
            draft(0, "Private Intro erasure canary", false),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    assert_eq!(status, 200, "{value}");
    if state != "DRAFT" {
        assert_eq!(
            call(
                &f,
                &r,
                "publishConcertIntro",
                raw(
                    &f,
                    "POST",
                    "me/intro/publish",
                    json!({"expected_revision":1}),
                    Some(Uuid::new_v4())
                )
            )
            .await?
            .0,
            200
        );
    }
    if state == "CLOSED" {
        assert_eq!(
            call(
                &f,
                &r,
                "closeConcertIntro",
                raw(
                    &f,
                    "POST",
                    "me/intro/close",
                    json!({"expected_revision":2}),
                    Some(Uuid::new_v4())
                )
            )
            .await?
            .0,
            200
        );
    }
    let base_event = f.event;
    let expected_rows = if state == "DRAFT" { 129 } else { 1 };
    if state == "DRAFT" {
        // Actual canonical Intro commands across 129 populated owning events:
        // first page must stop at128 and survive a subsequent replay boundary.
        for n in 0..128u128 {
            f.event = Uuid::from_u128(9_000_000 + n);
            sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Intro deletion fixture','OPEN')").bind(f.event).bind(format!("intro-deletion-{}",f.event.simple())).execute(&f.concert).await?;
            sqlx::query("insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')").bind(f.event).bind(f.actor).execute(&f.concert).await?;
            sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'PARADE',false,false,false)").bind(f.event).bind(f.actor).execute(&f.concert).await?;
            sqlx::query("insert into concert_meeting_settings(event_id,event_cutoff) values($1,clock_timestamp()+interval '1 day')").bind(f.event).execute(&f.concert).await?;
            let (status, body) = call(
                &f,
                &r,
                "setConcertIntro",
                raw(
                    &f,
                    "PUT",
                    "me/intro",
                    draft(0, "Private multi-event fixture", false),
                    Some(Uuid::new_v4()),
                ),
            )
            .await?;
            assert_eq!(status, 200, "{body}");
        }
        f.event = base_event;
    }
    // A second canonical principal's actual Intro must remain untouched.
    let original_actor = f.actor;
    let original_family = f.family;
    f.actor = f.subject;
    f.family = Uuid::new_v4();
    sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values($1,$2,$3,$4,clock_timestamp()+interval '1 hour')").bind(Uuid::new_v4().to_string()).bind(Uuid::new_v4().to_string()).bind(f.actor).bind(f.family).execute(&f.auth).await?;
    let (status, body) = call(
        &f,
        &r,
        "setConcertIntro",
        raw(
            &f,
            "PUT",
            "me/intro",
            draft(0, "Other owner must survive", false),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    assert_eq!(status, 200, "{body}");
    f.actor = original_actor;
    f.family = original_family;
    // Uncovered DELETE is explicitly denied even before a deletion fence exists.
    assert!(sqlx::query("delete from concert_intros where owner_id=$1")
        .bind(f.actor)
        .execute(&f.concert)
        .await
        .is_err());
    let c = deletion(f.actor);
    // A blocked owner lock consumes the original deadline and leaves no effect.
    let mut held = f.concert.acquire().await?;
    sqlx::query("select pg_advisory_lock(hashtextextended('mcr-world-account-deletion:'||$1::uuid::text,0))").bind(f.actor).execute(&mut *held).await?;
    let short = Instant::now() + std::time::Duration::from_millis(150);
    let proof = DeletionTestSource(74)
        .verify(&c, short)
        .await
        .map_err(deletion_error)?;
    assert!(owner(f.concert.clone(), 74)
        .apply_category(&c, &proof, short)
        .await
        .is_err());
    sqlx::query("select pg_advisory_unlock(hashtextextended('mcr-world-account-deletion:'||$1::uuid::text,0))").bind(f.actor).execute(&mut *held).await?;
    drop(held);
    let pending = apply(&f.concert, &c, 74).await?;
    assert!(!pending.complete);
    assert_eq!(pending.progress_revision, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from concert_intros where owner_id=$1")
            .bind(f.actor)
            .fetch_one(&f.concert)
            .await?,
        expected_rows
    );
    fixture_sharing(&f.concert, &c, false).await?;
    assert!(!apply(&f.concert, &c, 74).await?.complete);
    fixture_sharing(&f.concert, &c, true).await?;
    // Withdrawal at the final source check rolls back actual DELETE and progress.
    let deadline = Instant::now() + std::time::Duration::from_secs(4);
    let proof = DeletionTestSource(74)
        .verify(&c, deadline)
        .await
        .map_err(deletion_error)?;
    let changing = IntroAccountDeletion::new(
        f.concert.clone(),
        Arc::new(ChangingSource(std::sync::atomic::AtomicUsize::new(0))),
        Arc::new(PgIntroConsentDeletionEvidence),
    );
    assert!(changing.apply_category(&c, &proof, deadline).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from concert_intros where owner_id=$1")
            .bind(f.actor)
            .fetch_one(&f.concert)
            .await?,
        expected_rows
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select progress_revision from concert_intro_account_deletions where user_id=$1"
        )
        .bind(f.actor)
        .fetch_one(&f.concert)
        .await?,
        0
    );
    let mut done = apply(&f.concert, &c, 74).await?;
    if expected_rows > 128 {
        assert!(!done.complete);
        assert_eq!(done.progress_revision, 0);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("select count(*) from concert_intros where owner_id=$1")
                .bind(f.actor)
                .fetch_one(&f.concert)
                .await?,
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "select deleted_intros from concert_intro_account_deletions where user_id=$1"
            )
            .bind(f.actor)
            .fetch_one(&f.concert)
            .await?,
            128
        );
        done = apply(&f.concert, &c, 74).await?;
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from concert_intros where owner_id=$1")
            .bind(f.subject)
            .fetch_one(&f.concert)
            .await?,
        1
    );
    assert!(done.complete && done.progress_revision > 0);
    assert_eq!(done.category, PrivateDataCategory::IntroText);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from concert_intros where owner_id=$1")
            .bind(f.actor)
            .fetch_one(&f.concert)
            .await?,
        0
    );
    let replay = apply(&f.concert, &c, 74).await?;
    assert!(done == replay);
    assert!(apply(&f.concert, &c, 75).await.is_err());
    let mut changed = c.clone();
    changed.saga_id = Uuid::new_v4();
    assert!(apply(&f.concert, &changed, 74).await.is_err());
    changed = c.clone();
    changed.retention_until += Duration::nanoseconds(1);
    assert!(apply(&f.concert, &changed, 74).await.is_err());
    changed = c.clone();
    changed.actor_id = f.subject;
    assert!(apply(&f.concert, &changed, 74).await.is_err());
    changed = c.clone();
    changed.target = WorldDeletionTarget::Sharing;
    assert!(apply(&f.concert, &changed, 74).await.is_err());
    assert!(sqlx::query("insert into concert_intros(event_id,intro_id,owner_id,revision,state,text,created_at,updated_at) values($1,$2,$3,1,'DRAFT','recreation',clock_timestamp(),clock_timestamp())").bind(f.event).bind(Uuid::new_v4()).bind(f.actor).execute(&f.concert).await.is_err());
    let mut tx = f.concert.begin().await?;
    assert!(
        concert_service::adapters::out::repository::intro::lock_intro_actor_for_deletion_fence(
            &mut tx, f.actor
        )
        .await
        .is_err()
    );
    tx.rollback().await?;
    assert!(
        sqlx::query("delete from concert_intro_account_deletions where user_id=$1")
            .bind(f.actor)
            .execute(&f.concert)
            .await
            .is_err()
    );
    f.assert_clean().await?;
    Ok(())
}
#[tokio::test]
#[ignore = "requires newly marker-owned PG16 and canonical mTLS fixture"]
async fn intro_deletion_draft() -> anyhow::Result<()> {
    native("DRAFT").await
}
#[tokio::test]
#[ignore = "requires newly marker-owned PG16 and canonical mTLS fixture"]
async fn intro_deletion_published() -> anyhow::Result<()> {
    native("PUBLISHED").await
}
#[tokio::test]
#[ignore = "requires newly marker-owned PG16 and canonical mTLS fixture"]
async fn intro_deletion_closed() -> anyhow::Result<()> {
    native("CLOSED").await
}
#[tokio::test]
#[ignore = "replay on the exact same owned store after actual PostgreSQL restart"]
async fn intro_deletion_restart_replay() -> anyhow::Result<()> {
    let pool = environment::pool("concert").await?;
    let row=sqlx::query("select user_id,saga_id,actor_id,retention_seconds,retention_nanos,progress_revision from concert_intro_account_deletions where completed_at is not null").fetch_one(&pool).await?;
    let c = WorldDeletionCommand {
        user_id: row.get("user_id"),
        saga_id: row.get("saga_id"),
        actor_id: row.get("actor_id"),
        target: WorldDeletionTarget::PrivateData,
        retention_until: DateTime::from_timestamp(
            row.get("retention_seconds"),
            row.get::<i32, _>("retention_nanos") as u32,
        )
        .unwrap(),
    };
    let done = apply(&pool, &c, 74).await?;
    assert!(done.complete);
    assert_eq!(
        done.progress_revision,
        row.get::<i64, _>("progress_revision") as u64
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from concert_intros where owner_id=$1")
            .bind(c.user_id)
            .fetch_one(&pool)
            .await?,
        0
    );
    assert!(apply(&pool, &c, 75).await.is_err());
    Ok(())
}
#[tokio::test]
async fn intro_deletion_wrong_target_and_rebound_source_fail_before_database() -> anyhow::Result<()>
{
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgresql://unused:unused@127.0.0.1:1/unused")?;
    let c = deletion(Uuid::new_v4());
    let d = Instant::now() + std::time::Duration::from_secs(4);
    let source = DeletionTestSource(74)
        .verify(&c, d)
        .await
        .map_err(deletion_error)?;
    let mut wrong = c.clone();
    wrong.target = WorldDeletionTarget::Sharing;
    assert!(owner(pool.clone(), 74)
        .apply_category(&wrong, &source, d)
        .await
        .is_err());
    wrong = c.clone();
    wrong.user_id = Uuid::new_v4();
    assert!(owner(pool.clone(), 74)
        .apply_category(&wrong, &source, d)
        .await
        .is_err());
    assert!(owner(pool, 74)
        .apply_category(&c, &source, d + std::time::Duration::from_millis(1))
        .await
        .is_err());
    Ok(())
}
