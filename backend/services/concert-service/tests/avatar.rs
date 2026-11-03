#[path = "support/avatar/mod.rs"]
mod support;
use concert_application::{avatar::*, bootstrap::BootstrapRepository};
use concert_domain::{avatar::config_value, *};
use concert_service::{
    adapters::out::repository::avatar::*, bootstrap::BootstrapPostgresRepository,
};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;
const EVENT: &str = "00000000-0000-4000-8000-000000000701";
const USER: &str = "00000000-0000-4000-8000-000000000702";
fn key(event: Uuid, user: Uuid) -> ParticipantKey {
    ParticipantKey {
        event_id: EventId::from_uuid(event),
        user_id: AuthUserId::from_uuid(user),
    }
}
fn command(expected: i64, config: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"expected_revision":expected,"config":config})).unwrap()
}
fn prefs(expected: i64) -> Vec<u8> {
    serde_json::to_vec(&json!({"expected_revision":expected,"preferences":{"theme":"DANGER","low_power":true,"reduced_motion":true,"sound_enabled":false}})).unwrap()
}
#[test]
fn preference_payload_is_exact_bounded_and_no_character_fields() {
    assert!(parse_preferences(&prefs(1)).is_ok());
    for raw in [
        b"{}".as_slice(),
        br#"{"expected_revision":1,"preferences":{"theme":"DANGER"}}"#,
        br#"{"expected_revision":1,"expected_revision":2,"preferences":{}}"#,
    ] {
        assert!(parse_preferences(raw).is_err());
    }
    let mut wire: Value = serde_json::from_slice(&prefs(1)).unwrap();
    wire["preferences"]["avatar"] = json!({});
    assert!(parse_preferences(&serde_json::to_vec(&wire).unwrap()).is_err());
    wire["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("avatar");
    wire["preferences"]["theme"] = json!("OTHER");
    assert!(parse_preferences(&serde_json::to_vec(&wire).unwrap()).is_err());
}
#[tokio::test]
async fn release_projection_refuses_corruption_and_product_fixture_inventory() {
    use concert_application::bootstrap::{
        BootstrapDefaults, BootstrapRelease, ValidatedProductRelease,
    };
    let mut document: Value = serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/core/bootstrap-open.json"
    ))
    .unwrap();
    let command =
        concert_domain::avatar::SetAvatar::parse(&command(1, document["avatar"]["config"].clone()))
            .unwrap();
    let defaults = BootstrapDefaults {
        theme: Theme::Revenge,
        avatar: command.selection,
    };
    let make_release = |doc: &Value| {
        Arc::new(
            ValidatedProductRelease::from_validated(
                BootstrapRelease::new(defaults.clone(), serde_json::to_vec(doc).unwrap()).unwrap(),
                "a".repeat(64),
            )
            .unwrap(),
        )
    };
    // No connection is attempted by validation. This test address is not a
    // database fixture, and cannot authorize a live write.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused@127.0.0.1:1/unused")
        .unwrap();
    assert!(AvatarPersistence::from_product(pool.clone(), make_release(&document)).is_err());
    assert!(
        AvatarPersistence::from_synthetic_test(pool.clone(), make_release(&document), false)
            .is_err()
    );
    assert!(
        AvatarPersistence::from_synthetic_test(pool.clone(), make_release(&document), true).is_ok()
    );
    document["avatar_manifest"]["atlas"]["items"][0]["item_id"] = json!("tampered");
    assert!(AvatarPersistence::from_synthetic_test(pool, make_release(&document), true).is_err());
}
#[tokio::test]
#[ignore = "requires avatar_smoke.py owned PostgreSQL cluster"]
async fn native_avatar_transactions() {
    let pool = support::pool().await;
    let release = support::release();
    let event = EVENT.parse::<Uuid>().unwrap();
    let user = USER.parse::<Uuid>().unwrap();
    let own = key(event, user);
    support::seed(&pool, event).await;
    let bootstrap = BootstrapPostgresRepository::new(pool.clone());
    for u in [user, Uuid::new_v4()] {
        bootstrap
            .bootstrap(&support::admission(event, u), release.release().defaults())
            .await
            .unwrap();
    }
    assert!(AvatarPersistence::from_product(pool.clone(), release.clone()).is_err());
    assert!(AvatarPersistence::from_synthetic_test(pool.clone(), release.clone(), false).is_err());
    let service = Arc::new(
        AvatarPersistence::from_synthetic_test(pool.clone(), release.clone(), true).unwrap(),
    );
    let mut desired = config_value(&release.release().defaults().avatar);
    desired["item_ids"]["accessory"] = Value::Null;
    let mut tasks = vec![];
    for _ in 0..16 {
        let service = service.clone();
        let bytes = command(1, desired.clone());
        tasks.push(tokio::spawn(async move {
            let a =
                support::Authority::new(own, AvatarOperation::SetAvatar, service.release_digest());
            service.set_avatar(own, &bytes, &a).await
        }));
    }
    let mut winners = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(value) => {
                assert_eq!(value, json!({"revision":2,"config":desired}));
                winners += 1;
            }
            Err(e) => assert_eq!(e, AvatarError::Conflict),
        }
    }
    assert_eq!(winners, 1);
    // A fresh authority failure AFTER SQL update must roll the mutation back.
    let mut fail =
        support::Authority::new(own, AvatarOperation::SetAvatar, service.release_digest());
    fail.fail_at = 8;
    assert_eq!(
        service
            .set_avatar(
                own,
                &command(2, config_value(&release.release().defaults().avatar)),
                &fail
            )
            .await
            .unwrap_err(),
        AvatarError::Unauthenticated
    );
    let read = support::Authority::new(own, AvatarOperation::Read, service.release_digest());
    let before = service.read(own, &read).await.unwrap();
    assert_eq!(before.avatar.revision.get(), 2);
    assert_eq!(config_value(&before.avatar.selection), desired);
    // Preference races use participant revision and retain the complete avatar.
    let mut tasks = vec![];
    for _ in 0..12 {
        let service = service.clone();
        tasks.push(tokio::spawn(async move {
            let a = support::Authority::new(
                own,
                AvatarOperation::SetPreferences,
                service.release_digest(),
            );
            service.set_preferences(own, &prefs(1), &a).await
        }));
    }
    let mut winners = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(v) => {
                assert_eq!(v.participant.revision.get(), 2);
                assert_eq!(v.avatar, before.avatar);
                winners += 1;
            }
            Err(e) => assert_eq!(e, AvatarError::Conflict),
        }
    }
    assert_eq!(winners, 1);
    let after = service.read(own, &read).await.unwrap();
    assert_eq!(after.avatar, before.avatar);
    assert_eq!(after.participant.preferences.theme, Theme::Danger);
    let mut fail = support::Authority::new(
        own,
        AvatarOperation::SetPreferences,
        service.release_digest(),
    );
    fail.fail_at = 8;
    assert_eq!(
        service
            .set_preferences(own, &prefs(2), &fail)
            .await
            .unwrap_err(),
        AvatarError::Unauthenticated
    );
    assert_eq!(service.read(own, &read).await.unwrap(), after);
    let write = support::Authority::new(own, AvatarOperation::SetAvatar, service.release_digest());
    let mut bad = desired.clone();
    bad["item_ids"]["body"] = "eyes_fixture".into();
    assert_eq!(
        service
            .set_avatar(own, &command(2, bad), &write)
            .await
            .unwrap_err(),
        AvatarError::InvalidInput
    );
    // Actor/event/operation/release are checked independently of request payload.
    for foreign in [key(event, Uuid::new_v4()), key(Uuid::new_v4(), user)] {
        assert_eq!(
            service
                .set_avatar(foreign, &command(2, desired.clone()), &write)
                .await
                .unwrap_err(),
            AvatarError::Denied
        );
    }
    let stale = support::Authority::new(own, AvatarOperation::SetAvatar, &"0".repeat(64));
    assert_eq!(
        service
            .set_avatar(own, &command(2, desired.clone()), &stale)
            .await
            .unwrap_err(),
        AvatarError::Unavailable
    );
    assert_eq!(
        service
            .set_avatar(own, &command(2, desired.clone()), &read)
            .await
            .unwrap_err(),
        AvatarError::Denied
    );
    // Missing admissions never create or revive participant defaults.
    let unknown = key(event, Uuid::new_v4());
    let a = support::Authority::new(unknown, AvatarOperation::Read, service.release_digest());
    assert_eq!(
        service.read(unknown, &a).await.unwrap_err(),
        AvatarError::Denied
    );
    // A failure observed only after commit is uncertain, not proof of rollback.
    let uncertain_event = Uuid::new_v4();
    support::seed(&pool, uncertain_event).await;
    bootstrap
        .bootstrap(
            &support::admission(uncertain_event, user),
            release.release().defaults(),
        )
        .await
        .unwrap();
    let uncertain_key = key(uncertain_event, user);
    let mut uncertain = support::Authority::new(
        uncertain_key,
        AvatarOperation::SetAvatar,
        service.release_digest(),
    );
    uncertain.fail_at = 10;
    assert_eq!(
        service
            .set_avatar(uncertain_key, &command(1, desired.clone()), &uncertain)
            .await
            .unwrap_err(),
        AvatarError::Unauthenticated
    );
    let read_uncertain = support::Authority::new(
        uncertain_key,
        AvatarOperation::Read,
        service.release_digest(),
    );
    let committed = service.read(uncertain_key, &read_uncertain).await.unwrap();
    assert_eq!(committed.avatar.revision.get(), 2);
    assert_eq!(config_value(&committed.avatar.selection), desired);
    // A write waiting behind event closure rechecks the new phase under lock.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("select event_id from concert_events where event_id=$1 for update")
        .bind(event)
        .execute(&mut *tx)
        .await
        .unwrap();
    let delayed = service.clone();
    let config = desired.clone();
    let probe = Arc::new(support::Authority::new(
        own,
        AvatarOperation::SetAvatar,
        service.release_digest(),
    ));
    let signal = probe.clone();
    let waiting =
        tokio::spawn(async move { delayed.set_avatar(own, &command(2, config), &*signal).await });
    while probe.calls.load(std::sync::atomic::Ordering::SeqCst) < 3 {
        tokio::task::yield_now().await;
    }
    sqlx::query(
        "update concert_events set phase='READ_ONLY',revision=revision+1 where event_id=$1",
    )
    .bind(event)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(waiting.await.unwrap().unwrap_err(), AvatarError::ReadOnly);
    assert_eq!(service.read(own, &read).await.unwrap(), after);
    for phase in ["ARCHIVED", "DRAFT", "OPEN"] {
        sqlx::query("update concert_events set phase=$2,revision=revision+1 where event_id=$1")
            .bind(event)
            .bind(phase)
            .execute(&pool)
            .await
            .unwrap();
        if phase != "OPEN" {
            let error = service
                .set_avatar(own, &command(2, desired.clone()), &write)
                .await
                .unwrap_err();
            assert_eq!(
                error,
                if phase == "DRAFT" {
                    AvatarError::Missing
                } else {
                    AvatarError::ReadOnly
                }
            );
        }
    }
    // Suspension wins both reads and writes; no state is leaked or overwritten.
    sqlx::query("update concert_event_admissions set state='SUSPENDED',restriction_revision=restriction_revision+1 where event_id=$1 and user_id=$2").bind(event).bind(user).execute(&pool).await.unwrap();
    assert_eq!(
        service.read(own, &read).await.unwrap_err(),
        AvatarError::Denied
    );
    assert_eq!(
        service
            .set_avatar(own, &command(2, desired.clone()), &write)
            .await
            .unwrap_err(),
        AvatarError::Denied
    );
    // Re-enable fixture for postmaster restart verification. This is test setup.
    sqlx::query("update concert_event_admissions set state='ACTIVE',restriction_revision=restriction_revision+1 where event_id=$1 and user_id=$2").bind(event).bind(user).execute(&pool).await.unwrap();
    assert_eq!(service.read(own, &read).await.unwrap(), after);
    let untouched:i64=sqlx::query_scalar("select count(*) from concert_avatar_settings where event_id=$1 and user_id<>$2 and revision=1 and accessory_id is not null").bind(event).bind(user).fetch_one(&pool).await.unwrap();
    assert_eq!(untouched, 1);
    std::fs::write(
        std::path::PathBuf::from(std::env::var("MCR_BOOTSTRAP_ROOT").unwrap())
            .join("avatar-state.json"),
        serde_json::to_vec(&avatar_state(&after.avatar)).unwrap(),
    )
    .unwrap();
    pool.close().await;
}
#[tokio::test]
#[ignore = "requires avatar_smoke.py restarted owned PostgreSQL cluster"]
async fn native_avatar_restart_persistence() {
    let pool = support::pool().await;
    let release = support::release();
    let own = key(EVENT.parse().unwrap(), USER.parse().unwrap());
    let service = AvatarPersistence::from_synthetic_test(pool.clone(), release, true).unwrap();
    let read = support::Authority::new(own, AvatarOperation::Read, service.release_digest());
    let snapshot = service.read(own, &read).await.unwrap();
    assert_eq!(snapshot.avatar.revision.get(), 2);
    assert!(snapshot.avatar.selection.accessory.is_none());
    assert_eq!(snapshot.participant.revision.get(), 2);
    assert_eq!(snapshot.participant.preferences.theme, Theme::Danger);
    let old: Value = serde_json::from_slice(
        &std::fs::read(
            std::path::PathBuf::from(std::env::var("MCR_BOOTSTRAP_ROOT").unwrap())
                .join("avatar-state.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(old, avatar_state(&snapshot.avatar));
    pool.close().await;
}
