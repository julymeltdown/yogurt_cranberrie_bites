use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::*;
use concert_domain::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use uuid::Uuid;

fn key() -> ParticipantKey {
    ParticipantKey {
        event_id: EventId::from_uuid(Uuid::new_v4()),
        user_id: AuthUserId::from_uuid(Uuid::new_v4()),
    }
}
fn preferences() -> Preferences {
    Preferences {
        theme: Theme::Revenge,
        low_power: false,
        reduced_motion: true,
        sound_enabled: false,
    }
}
fn participant(key: ParticipantKey) -> Participant {
    Participant {
        key,
        revision: Revision::initial(),
        preferences: preferences(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}
#[derive(Clone)]
struct FixedClock(DateTime<Utc>);
impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}
struct Policy {
    answer: Result<PolicyGrant, PolicyError>,
    at: DateTime<Utc>,
}
#[async_trait]
impl ParticipationPolicy for Policy {
    async fn authorize(
        &self,
        _: ParticipantKey,
        _: Operation,
        at: DateTime<Utc>,
    ) -> Result<PolicyGrant, PolicyError> {
        assert_eq!(at, self.at);
        self.answer
    }
}
struct Repository {
    calls: Arc<AtomicUsize>,
    value: Result<Option<Participant>, StoreError>,
    update: PreferenceUpdate,
}
#[async_trait]
impl CoreRepository for Repository {
    async fn event(&self, _: EventId) -> Result<Option<Event>, StoreError> {
        panic!("no event discovery in private participant use case")
    }
    async fn participant(&self, _: ParticipantKey) -> Result<Option<Participant>, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.value.clone()
    }
    async fn avatar(&self, _: ParticipantKey) -> Result<Option<AvatarSettings>, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
    async fn update_preferences(
        &self,
        _: ParticipantKey,
        _: Revision,
        _: Preferences,
    ) -> Result<PreferenceUpdate, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.update.clone())
    }
}
fn core(
    answer: Result<PolicyGrant, PolicyError>,
    value: Result<Option<Participant>, StoreError>,
    update: PreferenceUpdate,
) -> (
    ConcertCore<Repository, FixedClock, Policy>,
    Arc<AtomicUsize>,
) {
    let calls = Arc::new(AtomicUsize::new(0));
    let at = Utc::now();
    (
        ConcertCore::new(
            Repository {
                calls: calls.clone(),
                value,
                update,
            },
            FixedClock(at),
            Policy { answer, at },
        ),
        calls,
    )
}
#[tokio::test]
async fn denied_unavailable_foreign_subject_or_wrong_operation_policy_never_reaches_store() {
    let actor = key();
    for (answer, expected) in [
        (Err(PolicyError::Denied), CoreError::PermissionDenied),
        (
            Err(PolicyError::Unavailable),
            CoreError::DependencyUnavailable,
        ),
        (
            Ok(PolicyGrant {
                key: key(),
                operation: Operation::ReadOwnParticipant,
            }),
            CoreError::PermissionDenied,
        ),
        (
            Ok(PolicyGrant {
                key: actor,
                operation: Operation::WriteOwnPreferences,
            }),
            CoreError::PermissionDenied,
        ),
    ] {
        let (core, calls) = core(
            answer,
            Ok(Some(participant(actor))),
            PreferenceUpdate::Missing,
        );
        assert_eq!(core.own_participant(actor).await.unwrap_err(), expected);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}
#[tokio::test]
async fn denied_writes_and_avatar_reads_have_zero_store_effect() {
    let actor = key();
    let (core, calls) = core(
        Err(PolicyError::Unavailable),
        Ok(None),
        PreferenceUpdate::Missing,
    );
    assert_eq!(
        core.own_avatar(actor).await.unwrap_err(),
        CoreError::DependencyUnavailable
    );
    assert_eq!(
        core.change_preferences(actor, Revision::initial(), preferences())
            .await
            .unwrap_err(),
        CoreError::DependencyUnavailable
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn scoped_reads_reject_wrong_identity_and_classify_absence_and_failure() {
    let actor = key();
    let grant = Ok(PolicyGrant {
        key: actor,
        operation: Operation::ReadOwnParticipant,
    });
    for (value, expected) in [
        (Ok(None), CoreError::ResourceUnavailable),
        (
            Err(StoreError::Unavailable),
            CoreError::DependencyUnavailable,
        ),
        (
            Err(StoreError::InvalidData),
            CoreError::DependencyUnavailable,
        ),
        (
            Ok(Some(participant(key()))),
            CoreError::DependencyUnavailable,
        ),
    ] {
        let (core, _) = core(grant, value, PreferenceUpdate::Missing);
        assert_eq!(core.own_participant(actor).await.unwrap_err(), expected);
    }
    let (core, calls) = core(
        grant,
        Ok(Some(participant(actor))),
        PreferenceUpdate::Missing,
    );
    assert_eq!(core.own_participant(actor).await.unwrap().key, actor);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn mutation_outcomes_and_returned_revision_are_typed() {
    let actor = key();
    let grant = Ok(PolicyGrant {
        key: actor,
        operation: Operation::WriteOwnPreferences,
    });
    for (update, expected) in [
        (PreferenceUpdate::Missing, CoreError::ResourceUnavailable),
        (PreferenceUpdate::Conflict, CoreError::VersionConflict),
        (PreferenceUpdate::EventReadOnly, CoreError::EventReadOnly),
        (
            PreferenceUpdate::Updated(participant(actor)),
            CoreError::DependencyUnavailable,
        ),
        (
            PreferenceUpdate::Updated(participant(key())),
            CoreError::DependencyUnavailable,
        ),
    ] {
        let (core, _) = core(grant, Ok(None), update);
        assert_eq!(
            core.change_preferences(actor, Revision::initial(), preferences())
                .await
                .unwrap_err(),
            expected
        );
    }
    let mut updated = participant(actor);
    updated.revision = Revision::new(2).unwrap();
    let (core, _) = core(grant, Ok(None), PreferenceUpdate::Updated(updated));
    assert_eq!(
        core.change_preferences(actor, Revision::initial(), preferences())
            .await
            .unwrap()
            .revision
            .get(),
        2
    );
}
