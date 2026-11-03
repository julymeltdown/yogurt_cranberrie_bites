use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use concert_application::bootstrap::*;
use concert_domain::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use uuid::Uuid;

fn event() -> EventId {
    EventId::from_uuid(Uuid::from_u128(2))
}
fn user() -> AuthUserId {
    AuthUserId::from_uuid(Uuid::from_u128(1))
}
fn session() -> Uuid {
    Uuid::from_u128(3)
}
fn request() -> Uuid {
    Uuid::from_u128(4)
}
fn admission() -> BootstrapAdmission {
    BootstrapAdmission::from_verified(
        user(),
        event(),
        session(),
        request(),
        "Current public name".into(),
        Some("A public bio".into()),
        Utc.timestamp_opt(1_900_000_000, 0).unwrap(),
    )
    .unwrap()
}
fn avatar() -> AvatarSelection {
    AvatarSelection {
        manifest_id: AssetId::new("avatar_product".into()).unwrap(),
        manifest_version: Revision::initial(),
        body: AssetId::new("body_product".into()).unwrap(),
        eyes: AssetId::new("eyes_product".into()).unwrap(),
        hair: AssetId::new("hair_product".into()).unwrap(),
        outfit: AssetId::new("outfit_product".into()).unwrap(),
        accessory: None,
        skin_tone: AssetId::new("tone_product".into()).unwrap(),
    }
}
fn defaults() -> BootstrapDefaults {
    BootstrapDefaults {
        theme: Theme::Parade,
        avatar: avatar(),
    }
}
fn release() -> BootstrapRelease {
    BootstrapRelease::new(defaults(), b"{}".to_vec()).unwrap()
}
fn record() -> BootstrapRecord {
    BootstrapRecord {
        event_id: event(),
        event_revision: 7,
        event_slug: "test-event".into(),
        event_title: "Current event".into(),
        event_timezone: "Asia/Seoul".into(),
        phase: EventPhase::Open,
        restriction_revision: 2,
        active: true,
        participant_revision: 3,
        preferences: Preferences {
            theme: Theme::Parade,
            low_power: true,
            reduced_motion: false,
            sound_enabled: false,
        },
        avatar_revision: 4,
        avatar: avatar(),
        completed_at: Utc.timestamp_opt(1_900_000_001, 0).unwrap(),
    }
}
#[derive(Clone)]
struct Fake {
    response: Result<BootstrapRecord, BootstrapError>,
    calls: Arc<AtomicUsize>,
    seen: Arc<Mutex<Vec<BootstrapDefaults>>>,
}
impl Fake {
    fn new(response: Result<BootstrapRecord, BootstrapError>) -> Self {
        Self {
            response,
            calls: Arc::new(AtomicUsize::new(0)),
            seen: Arc::new(Mutex::new(vec![])),
        }
    }
}
#[async_trait]
impl BootstrapRepository for Fake {
    async fn bootstrap(
        &self,
        _: &BootstrapAdmission,
        defaults: &BootstrapDefaults,
    ) -> Result<BootstrapRecord, BootstrapError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.seen.lock().unwrap().push(defaults.clone());
        self.response.clone()
    }
}

#[tokio::test]
async fn valid_admission_dispatches_once_with_exact_defaults_and_result() {
    let fake = Fake::new(Ok(record()));
    let calls = fake.calls.clone();
    let seen = fake.seen.clone();
    let result = BootstrapUseCase::new(fake, release())
        .execute(&admission())
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(*seen.lock().unwrap(), vec![defaults()]);
    assert_eq!(result.record, record());
    assert_eq!(result.release.defaults(), &defaults());
    assert_eq!(result.release.bootstrap_json(), b"{}");
}

#[tokio::test]
async fn identity_and_profile_validation_fail_before_repository_dispatch() {
    let long_name = "x".repeat(41);
    let long_bio = "y".repeat(161);
    let cases = [
        ("", None),
        ("   ", None),
        ("\nprivate", None),
        (long_name.as_str(), None),
        ("name", Some(long_bio.as_str())),
        ("name", Some("\u{0}private")),
    ];
    for (name, bio) in cases {
        assert!(BootstrapAdmission::from_verified(
            user(),
            event(),
            session(),
            request(),
            name.into(),
            bio.map(str::to_owned),
            Utc.timestamp_opt(1_900_000_000, 0).unwrap(),
        )
        .is_err());
    }
    for family in [Uuid::nil(), request()] {
        assert!(BootstrapAdmission::from_verified(
            user(),
            event(),
            family,
            if family == request() {
                Uuid::nil()
            } else {
                request()
            },
            "name".into(),
            None,
            Utc::now(),
        )
        .is_err());
    }
}

#[tokio::test]
async fn repository_failures_propagate_without_fabricated_success() {
    for error in [
        BootstrapError::Unauthenticated,
        BootstrapError::Denied,
        BootstrapError::EventNotFound,
        BootstrapError::EventNotOpen,
        BootstrapError::RestrictionUnavailable,
        BootstrapError::DependencyUnavailable,
        BootstrapError::InvalidResponse,
    ] {
        let fake = Fake::new(Err(error));
        let calls = fake.calls.clone();
        assert_eq!(
            BootstrapUseCase::new(fake, release())
                .execute(&admission())
                .await
                .unwrap_err(),
            error
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn malformed_or_cross_event_repository_results_are_rejected() {
    let cases = [
        BootstrapRecord {
            event_id: EventId::from_uuid(Uuid::from_u128(99)),
            ..record()
        },
        BootstrapRecord {
            event_revision: 0,
            ..record()
        },
        BootstrapRecord {
            restriction_revision: 0,
            ..record()
        },
        BootstrapRecord {
            phase: EventPhase::Draft,
            ..record()
        },
        BootstrapRecord {
            active: false,
            ..record()
        },
        BootstrapRecord {
            participant_revision: 0,
            ..record()
        },
        BootstrapRecord {
            avatar_revision: 0,
            ..record()
        },
    ];
    for response in cases {
        assert_eq!(
            BootstrapUseCase::new(Fake::new(Ok(response)), release())
                .execute(&admission())
                .await
                .unwrap_err(),
            BootstrapError::InvalidResponse
        );
    }
}

#[test]
fn release_and_diagnostics_are_bounded_and_redacted() {
    assert!(BootstrapRelease::new(defaults(), vec![]).is_err());
    assert!(BootstrapRelease::new(defaults(), vec![0; 2 * 1024 * 1024 + 1]).is_err());
    assert!(ValidatedProductRelease::from_validated(release(), "ABC".into()).is_err());
    assert!(ValidatedProductRelease::from_validated(release(), "z".repeat(64)).is_err());
    let validated = ValidatedProductRelease::from_validated(release(), "a".repeat(64)).unwrap();
    let diagnostics = format!("{:?} {:?} {:?}", admission(), release(), validated);
    for secret in [
        "Current public name",
        "A public bio",
        session().to_string().as_str(),
        "{}",
    ] {
        assert!(!diagnostics.contains(secret));
    }
}

#[test]
fn exact_display_and_bio_limits_are_inclusive_and_preserved() {
    let name = "\u{1f3b8}".repeat(40);
    let bio = "\u{1f30c}".repeat(160);
    let item = BootstrapAdmission::from_verified(
        user(),
        event(),
        session(),
        request(),
        name.clone(),
        Some(bio.clone()),
        Utc::now(),
    )
    .unwrap();
    assert_eq!(item.display_name(), name);
    assert_eq!(item.bio(), Some(bio.as_str()));
}
