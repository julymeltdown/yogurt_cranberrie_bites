//! Real owned PostgreSQL. The Friend/Auth owner is synthetic; VENUE_FRIEND_V1 itself
//! is not claimed here, and the production port is proven to fail closed.
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::commands::RegisteredOperation;
use concert_application::venue_location::{VenueLocation, VenueLocationPolicyProvider};
use concert_domain::venue_location::*;
use concert_domain::{AuthUserId, EventId, ParticipantKey};
use concert_service::venue_location::{
    routes::VenueLocationOperation, PgVenueLocation, PgVenueLocationAuthority,
    VenueLocationAuthority,
};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgConnection, PgPool,
};
use std::{path::PathBuf, str::FromStr};
use uuid::Uuid;

async fn owned_pool() -> anyhow::Result<PgPool> {
    let options = PgConnectOptions::from_str(&std::env::var("CONCERT_TEST_DATABASE_URL")?)
        .map_err(|_| anyhow::anyhow!("invalid owned fixture options"))?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_concert_venue_location")
            && options.get_username() == "mcr_owner",
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;
    if let Ok(root) = std::env::var("CONCERT_TEST_OWNER_ROOT") {
        let actual: String = sqlx::query_scalar("show data_directory")
            .fetch_one(&pool)
            .await?;
        anyhow::ensure!(
            actual == PathBuf::from(root).join("data").to_string_lossy(),
            "foreign data directory refused"
        );
    }
    Ok(pool)
}

/// Test-only owner tables standing in for the synthetic Friend/Auth fixture.
/// They are created here, never in a service migration, and are serialized so
/// parallel test binaries cannot race the catalog.
const FIXTURE_DDL: &str = "
create table if not exists public.venue_test_follows(
    event_id uuid not null,
    a uuid not null,
    b uuid not null,
    primary key(event_id,a,b));
create table if not exists public.venue_test_restrictions(
    event_id uuid not null,
    user_id uuid not null,
    blocked boolean not null default false,
    suspended boolean not null default false,
    primary key(event_id,user_id));";

async fn provision(pool: &PgPool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(7613023461094200116)")
        .execute(&mut *tx)
        .await?;
    sqlx::raw_sql(FIXTURE_DDL).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

struct FixtureAuthority;
#[async_trait]
impl VenueLocationAuthority for FixtureAuthority {
    async fn actor_in(
        &self,
        c: &mut PgConnection,
        actor: ParticipantKey,
        _: DateTime<Utc>,
    ) -> Result<(), VenueLocationError> {
        let phase: Option<String> =
            sqlx::query_scalar("select phase from concert_events where event_id=$1 for share")
                .bind(actor.event_id.uuid())
                .fetch_optional(&mut *c)
                .await
                .map_err(|_| VenueLocationError::DependencyUnavailable)?;
        let member: Option<Uuid> = sqlx::query_scalar(
            "select user_id from concert_participants where event_id=$1 and user_id=$2 for share",
        )
        .bind(actor.event_id.uuid())
        .bind(actor.user_id.uuid())
        .fetch_optional(&mut *c)
        .await
        .map_err(|_| VenueLocationError::DependencyUnavailable)?;
        let suspended: Option<bool> = sqlx::query_scalar(
            "select suspended from venue_test_restrictions where event_id=$1 and user_id=$2",
        )
        .bind(actor.event_id.uuid())
        .bind(actor.user_id.uuid())
        .fetch_optional(&mut *c)
        .await
        .map_err(|_| VenueLocationError::DependencyUnavailable)?;
        if matches!(phase.as_deref(), Some("OPEN") | Some("LIVE"))
            && member == Some(actor.user_id.uuid())
            && suspended != Some(true)
        {
            Ok(())
        } else {
            Err(VenueLocationError::Denied)
        }
    }

    async fn friend_in(
        &self,
        c: &mut PgConnection,
        event: EventId,
        viewer: AuthUserId,
        owner: AuthUserId,
        _: DateTime<Utc>,
    ) -> Result<bool, VenueLocationError> {
        if viewer == owner {
            return Ok(false);
        }
        let edge: i64 = sqlx::query_scalar(
            "select count(*) from venue_test_follows where event_id=$1
               and ((a=$2 and b=$3) or (a=$3 and b=$2))",
        )
        .bind(event.uuid())
        .bind(viewer.uuid())
        .bind(owner.uuid())
        .fetch_one(&mut *c)
        .await
        .map_err(|_| VenueLocationError::DependencyUnavailable)?;
        let restricted: i64 = sqlx::query_scalar(
            "select count(*) from venue_test_restrictions
              where event_id=$1 and user_id in ($2,$3) and (blocked or suspended)",
        )
        .bind(event.uuid())
        .bind(viewer.uuid())
        .bind(owner.uuid())
        .fetch_one(&mut *c)
        .await
        .map_err(|_| VenueLocationError::DependencyUnavailable)?;
        Ok(edge == 2 && restricted == 0)
    }
}

struct FixturePolicy {
    event: EventId,
}
#[async_trait]
impl VenueLocationPolicyProvider for FixturePolicy {
    async fn policy(&self, event: EventId) -> Result<VenueLocationPolicy, VenueLocationError> {
        if event != self.event {
            return Err(VenueLocationError::PolicyUnavailable);
        }
        Ok(VenueLocationPolicy {
            event_id: event,
            venue_key: "venue-test".into(),
            revision: 1,
            timezone: "Asia/Seoul".into(),
            valid_until: Utc::now() + Duration::hours(1),
            fresh_ms: 30_000,
            last_known_ms: 120_000,
            fix_max_age_ms: 120_000,
            future_skew_ms: 5_000,
            sharing_lease_ms: 60_000,
            request_ttl_ms: 60_000,
            check_in_ttl_ms: 60_000,
            coordinate_retention_ms: 300_000,
            command_retention_ms: 60_000,
            max_accuracy_m: 50.0,
            max_viewers: 10,
        })
    }
}

struct Fixture {
    pool: PgPool,
    event: EventId,
    a: AuthUserId,
    b: AuthUserId,
    c: AuthUserId,
}
impl Fixture {
    fn key(&self, user: AuthUserId) -> ParticipantKey {
        ParticipantKey {
            event_id: self.event,
            user_id: user,
        }
    }
    async fn location(&self) -> PgVenueLocation<FixturePolicy, FixtureAuthority> {
        PgVenueLocation::new(
            self.pool.clone(),
            FixturePolicy { event: self.event },
            FixtureAuthority,
        )
    }
    /// Removes this test's rows in dependency order. The empty shared fixture
    /// tables stay: they are idempotent test scaffolding, not product data.
    async fn cleanup(&self) -> anyhow::Result<()> {
        let event = self.event.uuid();
        for statement in [
            "delete from public.venue_test_follows where event_id=$1",
            "delete from public.venue_test_restrictions where event_id=$1",
            "delete from public.concert_venue_location_viewers where event_id=$1",
            "delete from public.concert_venue_location_fixes where event_id=$1",
            "delete from public.concert_venue_location_sharing where event_id=$1",
            "delete from public.concert_participants where event_id=$1",
            "delete from public.concert_events where event_id=$1",
        ] {
            sqlx::query(statement)
                .bind(event)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }
}

async fn fixture(pool: PgPool) -> anyhow::Result<Fixture> {
    provision(&pool).await?;
    let event = Uuid::new_v4();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    sqlx::query(
        "insert into concert_events(event_id,slug,title,phase) values($1,$2,'Venue test','OPEN')",
    )
    .bind(event)
    .bind(format!("venue-{}", event.simple()))
    .execute(&pool)
    .await?;
    for user in [a, b, c] {
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)")
            .bind(event).bind(user).execute(&pool).await?;
    }
    for (x, y) in [(a, b), (b, a), (a, c), (c, a)] {
        sqlx::query("insert into venue_test_follows(event_id,a,b) values($1,$2,$3)")
            .bind(event)
            .bind(x)
            .bind(y)
            .execute(&pool)
            .await?;
    }
    for user in [a, b, c] {
        sqlx::query("insert into venue_test_restrictions(event_id,user_id) values($1,$2)")
            .bind(event)
            .bind(user)
            .execute(&pool)
            .await?;
    }
    Ok(Fixture {
        pool,
        event: EventId::from_uuid(event),
        a: AuthUserId::from_uuid(a),
        b: AuthUserId::from_uuid(b),
        c: AuthUserId::from_uuid(c),
    })
}

async fn restriction(f: &Fixture, user: AuthUserId, blocked: bool, suspended: bool) {
    sqlx::query("update venue_test_restrictions set blocked=$3,suspended=$4 where event_id=$1 and user_id=$2")
        .bind(f.event.uuid()).bind(user.uuid()).bind(blocked).bind(suspended)
        .execute(&f.pool).await.unwrap();
}

async fn fix(
    f: &Fixture,
    owner: AuthUserId,
    revision: i64,
    captured: DateTime<Utc>,
) -> Result<CommandReceipt, VenueLocationError> {
    f.location()
        .await
        .publish_fix(
            f.key(owner),
            Uuid::new_v4(),
            PhoneFixInput {
                expected_sharing_revision: revision,
                latitude: 37.5665,
                longitude: 126.9780,
                accuracy_m: 8.0,
                captured_at: captured,
            },
        )
        .await
}

#[tokio::test]
async fn consent_viewer_scope_freshness_and_revocation() {
    let pool = owned_pool().await.unwrap();
    let f = fixture(pool).await.unwrap();
    let location = f.location().await;
    let key = Uuid::new_v4();

    let before = location.snapshot(f.key(f.b)).await.unwrap();
    assert_eq!(before.sharing.state, SharingState::Off);
    assert!(before.friends.is_empty());
    assert_eq!(
        location.visible_for(f.key(f.b), f.a).await,
        Err(VenueLocationError::NotFound)
    );

    let receipt = location
        .set_sharing(
            f.key(f.a),
            key,
            SharingCommand::Active {
                expected_revision: 0,
                viewer_ids: vec![f.b.uuid()],
            },
        )
        .await
        .unwrap();
    assert_eq!(receipt.revision, 1);
    assert_eq!(receipt.operation, "setVenueLocationSharing");
    assert_eq!(receipt.resource_id, f.a.uuid());

    let captured = Utc::now() - Duration::seconds(1);
    let published = fix(&f, f.a, 1, captured).await.unwrap();
    assert_eq!(published.revision, 1);
    assert_eq!(published.resource_kind, "PHONE_FIX");

    let seen = location.snapshot(f.key(f.b)).await.unwrap();
    assert_eq!(seen.sharing.state, SharingState::Off);
    assert_eq!(seen.friends.len(), 1);
    let visible = &seen.friends[0];
    assert_eq!(visible.user_id, f.a.uuid());
    assert_eq!(visible.source, VisibleSource::Phone);
    assert_eq!(visible.position.latitude, 37.5665);
    assert_eq!(
        visible.fresh_until,
        Some(captured + Duration::milliseconds(30_000))
    );
    assert!(visible.access_until <= seen.policy.valid_until);
    assert!(!visible.physical_arrival_verified);
    assert_eq!(seen.clock.timezone, "Asia/Seoul");
    assert!(seen.clock.end_of_day > seen.server_time);
    assert_eq!(seen.clock.end_of_day, seoul_end_of_day(seen.server_time));

    assert!(location.visible_for(f.key(f.b), f.a).await.is_ok());
    assert_eq!(
        location.visible_for(f.key(f.c), f.a).await,
        Err(VenueLocationError::Denied)
    );

    let paused = location
        .set_sharing(
            f.key(f.a),
            Uuid::new_v4(),
            SharingCommand::Paused {
                expected_revision: 1,
                viewer_ids: Vec::new(),
            },
        )
        .await
        .unwrap();
    assert_eq!(paused.revision, 2);
    let fixes: i64 = sqlx::query_scalar(
        "select count(*) from concert_venue_location_fixes where event_id=$1 and owner_id=$2",
    )
    .bind(f.event.uuid())
    .bind(f.a.uuid())
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(fixes, 0, "pause must delete the precise fix");
    let paused_view = location.snapshot(f.key(f.a)).await.unwrap();
    assert_eq!(paused_view.sharing.state, SharingState::Paused);
    assert!(paused_view.self_location.is_none());
    assert!(location
        .snapshot(f.key(f.b))
        .await
        .unwrap()
        .friends
        .is_empty());
    assert_eq!(
        location.visible_for(f.key(f.b), f.a).await,
        Err(VenueLocationError::NotFound)
    );

    location
        .set_sharing(
            f.key(f.a),
            Uuid::new_v4(),
            SharingCommand::Active {
                expected_revision: 2,
                viewer_ids: vec![f.b.uuid()],
            },
        )
        .await
        .unwrap();
    fix(&f, f.a, 3, Utc::now()).await.unwrap();
    assert_eq!(
        location.snapshot(f.key(f.b)).await.unwrap().friends.len(),
        1
    );

    sqlx::query("update concert_venue_location_sharing set expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and owner_id=$2")
        .bind(f.event.uuid()).bind(f.a.uuid()).execute(&f.pool).await.unwrap();
    let expired = location.snapshot(f.key(f.a)).await.unwrap();
    assert_eq!(expired.sharing.state, SharingState::Expired);
    assert!(expired.self_location.is_none());
    assert!(location
        .snapshot(f.key(f.b))
        .await
        .unwrap()
        .friends
        .is_empty());
    assert_eq!(
        location.visible_for(f.key(f.b), f.a).await,
        Err(VenueLocationError::NotFound)
    );

    location
        .set_sharing(
            f.key(f.a),
            Uuid::new_v4(),
            SharingCommand::Active {
                expected_revision: 3,
                viewer_ids: vec![f.b.uuid()],
            },
        )
        .await
        .unwrap();
    fix(&f, f.a, 4, Utc::now()).await.unwrap();
    restriction(&f, f.b, true, false).await;
    assert!(location
        .snapshot(f.key(f.b))
        .await
        .unwrap()
        .friends
        .is_empty());
    assert_eq!(
        location.visible_for(f.key(f.b), f.a).await,
        Err(VenueLocationError::Denied)
    );
    restriction(&f, f.b, false, true).await;
    assert_eq!(
        location.snapshot(f.key(f.b)).await,
        Err(VenueLocationError::Denied)
    );
    assert_eq!(
        location.visible_for(f.key(f.b), f.a).await,
        Err(VenueLocationError::Denied)
    );
    restriction(&f, f.b, false, false).await;

    let revoked = location
        .set_sharing(
            f.key(f.a),
            Uuid::new_v4(),
            SharingCommand::Revoked {
                expected_revision: 4,
                viewer_ids: Vec::new(),
            },
        )
        .await
        .unwrap();
    assert_eq!(revoked.revision, 5);
    assert_eq!(
        location.visible_for(f.key(f.b), f.a).await,
        Err(VenueLocationError::NotFound)
    );
    f.cleanup().await.unwrap();
}

#[tokio::test]
async fn stale_fix_is_last_known_and_invalid_input_fails() {
    let pool = owned_pool().await.unwrap();
    let f = fixture(pool).await.unwrap();
    let location = f.location().await;
    location
        .set_sharing(
            f.key(f.a),
            Uuid::new_v4(),
            SharingCommand::Active {
                expected_revision: 0,
                viewer_ids: vec![f.b.uuid()],
            },
        )
        .await
        .unwrap();

    fix(&f, f.a, 1, Utc::now() - Duration::seconds(60))
        .await
        .unwrap();
    let stale = location.snapshot(f.key(f.b)).await.unwrap();
    assert_eq!(stale.friends.len(), 1);
    assert_eq!(stale.friends[0].source, VisibleSource::LastKnown);
    assert_eq!(stale.friends[0].fresh_until, None);

    assert_eq!(
        fix(&f, f.a, 1, Utc::now() - Duration::seconds(200)).await,
        Err(VenueLocationError::InvalidInput)
    );
    assert_eq!(
        fix(&f, f.a, 1, Utc::now() + Duration::seconds(10)).await,
        Err(VenueLocationError::InvalidInput)
    );
    let mut excessive = PhoneFixInput {
        expected_sharing_revision: 1,
        latitude: 37.0,
        longitude: 127.0,
        accuracy_m: 51.0,
        captured_at: Utc::now(),
    };
    assert_eq!(
        location
            .publish_fix(f.key(f.a), Uuid::new_v4(), excessive.clone())
            .await,
        Err(VenueLocationError::InvalidInput)
    );
    excessive.accuracy_m = 5.0;
    excessive.latitude = f64::NAN;
    assert_eq!(
        location
            .publish_fix(f.key(f.a), Uuid::new_v4(), excessive)
            .await,
        Err(VenueLocationError::InvalidInput)
    );

    assert_eq!(
        location
            .set_sharing(
                f.key(f.a),
                Uuid::new_v4(),
                SharingCommand::Paused {
                    expected_revision: 1,
                    viewer_ids: vec![f.b.uuid()],
                },
            )
            .await,
        Err(VenueLocationError::InvalidInput)
    );
    assert_eq!(
        location
            .set_sharing(
                f.key(f.a),
                Uuid::new_v4(),
                SharingCommand::Active {
                    expected_revision: 1,
                    viewer_ids: vec![f.c.uuid(), f.b.uuid()],
                },
            )
            .await
            .is_ok(),
        true
    );
    f.cleanup().await.unwrap();
}

#[tokio::test]
async fn missing_bilateral_friend_read_fails_closed() {
    let pool = owned_pool().await.unwrap();
    let f = fixture(pool.clone()).await.unwrap();
    let fixture_location = f.location().await;
    fixture_location
        .set_sharing(
            f.key(f.a),
            Uuid::new_v4(),
            SharingCommand::Active {
                expected_revision: 0,
                viewer_ids: vec![f.b.uuid()],
            },
        )
        .await
        .unwrap();
    fix(&f, f.a, 1, Utc::now()).await.unwrap();

    let unavailable = PgVenueLocation::new(
        pool,
        FixturePolicy { event: f.event },
        PgVenueLocationAuthority,
    );
    assert_eq!(
        unavailable.snapshot(f.key(f.b)).await,
        Err(VenueLocationError::PolicyUnavailable)
    );
    f.cleanup().await.unwrap();
}

#[test]
fn public_operation_registration_is_exact() {
    for (id, method, path) in concert_service::venue_location::routes::ROUTES {
        let registered = RegisteredOperation::find(id).unwrap();
        assert_eq!(registered.method(), *method);
        assert_eq!(registered.template(), *path);
    }
    let event = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
    let base = format!("/api/concert/events/{event}/venue/location");
    assert_eq!(
        VenueLocationOperation::route(event, "GET", &base).unwrap(),
        VenueLocationOperation::Snapshot
    );
    assert_eq!(
        VenueLocationOperation::route(event, "PUT", &format!("{base}/sharing")).unwrap(),
        VenueLocationOperation::SetSharing
    );
    assert_eq!(
        VenueLocationOperation::route(event, "PUT", &format!("{base}/fix")).unwrap(),
        VenueLocationOperation::PublishFix
    );
    assert_eq!(
        VenueLocationOperation::route(event, "POST", &format!("{base}/sharing")),
        Err(VenueLocationError::Denied)
    );
    assert_eq!(
        VenueLocationOperation::route(event, "GET", &format!("{base}/fix")),
        Err(VenueLocationError::Denied)
    );
}
