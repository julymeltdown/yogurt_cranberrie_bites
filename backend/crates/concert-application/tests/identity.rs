use async_trait::async_trait;
use chrono::{DateTime, Duration, TimeZone, Utc};
use concert_application::{identity::*, *};
use concert_domain::*;
use std::sync::{
    atomic::{AtomicI64, AtomicUsize, Ordering},
    Arc, Mutex,
};
use uuid::Uuid;

fn now() -> DateTime<Utc> {
    Utc.timestamp_opt(1_900_000_000, 0).unwrap()
}
fn key() -> ParticipantKey {
    ParticipantKey {
        user_id: AuthUserId::from_uuid(Uuid::from_u128(1)),
        event_id: EventId::from_uuid(Uuid::from_u128(2)),
    }
}
fn context() -> IdentityContext {
    IdentityContext {
        key: key(),
        operation: Operation::ReadOwnParticipant,
    }
}
fn credential() -> AccessCredential {
    AccessCredential::new("private-bearer".into()).unwrap()
}
fn claim() -> ClaimReference {
    ClaimReference::new("opaque-private-claim-reference".into()).unwrap()
}
#[derive(Clone)]
struct TestClock(Arc<AtomicI64>);
impl TestClock {
    fn new() -> Self {
        Self(Arc::new(AtomicI64::new(now().timestamp())))
    }
}
impl Clock for TestClock {
    fn now(&self) -> DateTime<Utc> {
        Utc.timestamp_opt(self.0.load(Ordering::SeqCst), 0).unwrap()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Access,
    Session,
    Eligibility,
    Profile,
    Provider,
}
#[derive(Clone)]
struct Fake {
    access: VerifiedSession,
    session: SessionAuthority,
    eligibility: EligibilityAuthority,
    profile: ProfileAuthority,
    provider: ProviderAuthority,
    failure: Option<(Stage, IdentityError)>,
    calls: Arc<Mutex<Vec<Stage>>>,
    advance: Option<(Stage, TestClock)>,
}
impl Fake {
    fn new() -> Self {
        let binding = AuthorityBinding {
            context: context(),
            session_id: SessionId::new(Uuid::from_u128(3).to_string()).unwrap(),
        };
        let window = ProofWindow {
            checked_at: now(),
            expires_at: now() + Duration::seconds(20),
        };
        Self {
            access: VerifiedSession {
                binding: binding.clone(),
                window,
            },
            session: SessionAuthority {
                binding: binding.clone(),
                window,
                state: SessionState::Active,
                roles: vec!["ROLE_USER".into()],
            },
            eligibility: EligibilityAuthority {
                binding: binding.clone(),
                window,
                state: EligibilityState::Eligible,
            },
            profile: ProfileAuthority {
                binding: binding.clone(),
                window,
                state: ProfileState::Active,
                profile: BasicProfile::new(key().user_id, "Email participant".into(), None)
                    .unwrap(),
            },
            provider: ProviderAuthority {
                binding,
                window,
                provider: Provider::Email,
                claim: claim(),
                attached: true,
            },
            failure: None,
            calls: Arc::new(Mutex::new(vec![])),
            advance: None,
        }
    }
    fn check(&self, stage: Stage) -> Result<(), IdentityError> {
        self.calls.lock().unwrap().push(stage);
        if let Some((on, clock)) = &self.advance {
            if *on == stage {
                clock.0.store(now().timestamp() + 21, Ordering::SeqCst);
            }
        }
        if let Some((on, error)) = self.failure {
            if on == stage {
                return Err(error);
            }
        }
        Ok(())
    }
    fn binding(&mut self, stage: Stage) -> &mut AuthorityBinding {
        match stage {
            Stage::Access => &mut self.access.binding,
            Stage::Session => &mut self.session.binding,
            Stage::Eligibility => &mut self.eligibility.binding,
            Stage::Profile => &mut self.profile.binding,
            Stage::Provider => &mut self.provider.binding,
        }
    }
    fn window(&mut self, stage: Stage) -> &mut ProofWindow {
        match stage {
            Stage::Access => &mut self.access.window,
            Stage::Session => &mut self.session.window,
            Stage::Eligibility => &mut self.eligibility.window,
            Stage::Profile => &mut self.profile.window,
            Stage::Provider => &mut self.provider.window,
        }
    }
}
#[async_trait]
impl IdentityAuthority for Fake {
    async fn authenticate(
        &self,
        _: &AccessCredential,
        _: IdentityContext,
        _: DateTime<Utc>,
    ) -> Result<VerifiedSession, IdentityError> {
        self.check(Stage::Access)?;
        Ok(self.access.clone())
    }
    async fn session(
        &self,
        _: &AuthorityBinding,
        _: DateTime<Utc>,
    ) -> Result<SessionAuthority, IdentityError> {
        self.check(Stage::Session)?;
        Ok(self.session.clone())
    }
    async fn eligibility(
        &self,
        _: &AuthorityBinding,
        _: DateTime<Utc>,
    ) -> Result<EligibilityAuthority, IdentityError> {
        self.check(Stage::Eligibility)?;
        Ok(self.eligibility.clone())
    }
    async fn profile(
        &self,
        _: &AuthorityBinding,
        _: DateTime<Utc>,
    ) -> Result<ProfileAuthority, IdentityError> {
        self.check(Stage::Profile)?;
        Ok(self.profile.clone())
    }
    async fn provider(
        &self,
        _: &AuthorityBinding,
        _: Provider,
        _: &ClaimReference,
        _: DateTime<Utc>,
    ) -> Result<ProviderAuthority, IdentityError> {
        self.check(Stage::Provider)?;
        Ok(self.provider.clone())
    }
}
async fn run(fake: Fake, stage: Stage) -> Result<(), IdentityError> {
    let resolver = IdentityResolver::new(fake, TestClock::new());
    match stage {
        Stage::Profile => resolver
            .basic_profile(&credential(), context())
            .await
            .map(|_| ()),
        Stage::Provider => resolver
            .authorize_claim(&credential(), context(), Provider::Email, &claim())
            .await
            .map(|_| ()),
        _ => resolver
            .authorize(&credential(), context(), now())
            .await
            .map(|_| ()),
    }
}

#[tokio::test]
async fn email_only_user_needs_no_x_or_profile_for_participation() {
    let fake = Fake::new();
    let calls = fake.calls.clone();
    let grant = IdentityResolver::new(fake.clone(), TestClock::new())
        .authorize(&credential(), context(), now())
        .await
        .unwrap();
    assert_eq!(
        grant,
        PolicyGrant {
            key: key(),
            operation: Operation::ReadOwnParticipant
        }
    );
    assert_eq!(
        *calls.lock().unwrap(),
        vec![Stage::Access, Stage::Session, Stage::Eligibility]
    );
    assert!(run(fake, Stage::Provider).await.is_ok());
}
#[tokio::test]
async fn wrong_actor_event_operation_or_session_never_authorizes() {
    for stage in [
        Stage::Access,
        Stage::Session,
        Stage::Eligibility,
        Stage::Profile,
        Stage::Provider,
    ] {
        for field in 0..4 {
            // Authentication creates the verified session ID; subsequent independent session proof
            // detects a foreign one. All other proofs must match that authenticated session.
            let mut fake = Fake::new();
            let binding = fake.binding(stage);
            match field {
                0 => binding.context.key.user_id = AuthUserId::from_uuid(Uuid::from_u128(90)),
                1 => binding.context.key.event_id = EventId::from_uuid(Uuid::from_u128(91)),
                2 => binding.context.operation = Operation::WriteOwnPreferences,
                _ => binding.session_id = SessionId::new(Uuid::from_u128(92).to_string()).unwrap(),
            }
            assert_eq!(
                run(fake, stage).await,
                Err(IdentityError::Denied),
                "{stage:?} field {field}"
            );
        }
    }
}
#[tokio::test]
async fn stale_future_expired_and_boundary_proofs_are_rejected() {
    for stage in [
        Stage::Access,
        Stage::Session,
        Stage::Eligibility,
        Stage::Profile,
        Stage::Provider,
    ] {
        for field in 0..3 {
            let mut fake = Fake::new();
            let window = fake.window(stage);
            match field {
                0 => window.checked_at = now() - Duration::seconds(1),
                1 => window.checked_at = now() + Duration::seconds(1),
                _ => window.expires_at = now(),
            }
            assert_eq!(
                run(fake, stage).await,
                Err(IdentityError::Denied),
                "{stage:?} field {field}"
            );
        }
    }
}
#[tokio::test]
async fn ended_unknown_and_empty_or_unrecognized_roles_fail_closed() {
    for state in [SessionState::Ended, SessionState::Unknown] {
        let mut fake = Fake::new();
        fake.session.state = state;
        assert_eq!(run(fake, Stage::Session).await, Err(IdentityError::Denied));
    }
    for roles in [
        vec![],
        vec!["ROLE_ADMIN".into()],
        vec!["role_user".into()],
        vec!["X".into()],
    ] {
        let mut fake = Fake::new();
        fake.session.roles = roles;
        assert_eq!(run(fake, Stage::Session).await, Err(IdentityError::Denied));
    }
    for state in [
        EligibilityState::Suspended,
        EligibilityState::Ineligible,
        EligibilityState::Unknown,
    ] {
        let mut fake = Fake::new();
        fake.eligibility.state = state;
        assert_eq!(
            run(fake, Stage::Eligibility).await,
            Err(if state == EligibilityState::Unknown {
                IdentityError::Unavailable
            } else {
                IdentityError::Denied
            })
        );
    }
}
#[tokio::test]
async fn provider_claim_must_match_owner_provider_reference_and_binding() {
    for field in 0..3 {
        let mut fake = Fake::new();
        match field {
            0 => fake.provider.provider = Provider::X,
            1 => fake.provider.claim = ClaimReference::new("foreign-claim".into()).unwrap(),
            _ => fake.provider.attached = false,
        };
        assert_eq!(run(fake, Stage::Provider).await, Err(IdentityError::Denied));
    }
}
#[tokio::test]
async fn profile_cannot_supply_auth_or_foreign_content_and_unknown_lifecycle_is_unavailable() {
    let mut fake = Fake::new();
    fake.profile.profile = BasicProfile::new(
        AuthUserId::from_uuid(Uuid::from_u128(90)),
        "Foreign".into(),
        None,
    )
    .unwrap();
    assert_eq!(run(fake, Stage::Profile).await, Err(IdentityError::Denied));
    for state in [ProfileState::Deleted, ProfileState::Unknown] {
        let mut fake = Fake::new();
        fake.profile.state = state;
        assert_eq!(
            run(fake, Stage::Profile).await,
            Err(if state == ProfileState::Unknown {
                IdentityError::Unavailable
            } else {
                IdentityError::Denied
            })
        );
    }
    let mut fake = Fake::new();
    fake.session.state = SessionState::Ended;
    let calls = fake.calls.clone();
    assert_eq!(run(fake, Stage::Profile).await, Err(IdentityError::Denied));
    assert!(!calls.lock().unwrap().contains(&Stage::Profile));
}
#[tokio::test]
async fn dependency_errors_remain_distinct_and_stop_further_dispatch() {
    for stage in [
        Stage::Access,
        Stage::Session,
        Stage::Eligibility,
        Stage::Profile,
        Stage::Provider,
    ] {
        for error in [
            IdentityError::Unauthenticated,
            IdentityError::Denied,
            IdentityError::Unavailable,
        ] {
            let mut fake = Fake::new();
            fake.failure = Some((stage, error));
            let calls = fake.calls.clone();
            assert_eq!(run(fake, stage).await, Err(error));
            assert_eq!(calls.lock().unwrap().last(), Some(&stage));
        }
    }
}
#[tokio::test]
async fn expiration_during_dependency_call_invalidates_earlier_proofs() {
    for stage in [Stage::Eligibility, Stage::Profile, Stage::Provider] {
        let clock = TestClock::new();
        let mut fake = Fake::new();
        // Keep the last reply valid, while earlier session/access evidence expires during await.
        fake.window(stage).expires_at = now() + Duration::seconds(60);
        fake.advance = Some((stage, clock.clone()));
        let resolver = IdentityResolver::new(fake, clock);
        let result = match stage {
            Stage::Profile => resolver
                .basic_profile(&credential(), context())
                .await
                .map(|_| ()),
            Stage::Provider => resolver
                .authorize_claim(&credential(), context(), Provider::Email, &claim())
                .await
                .map(|_| ()),
            _ => resolver
                .authorize(&credential(), context(), now())
                .await
                .map(|_| ()),
        };
        assert_eq!(result, Err(IdentityError::Denied));
    }
}
#[test]
fn profile_is_allowlisted_bounded_and_diagnostics_hide_content() {
    for name in ["".into(), " ".into(), "a".repeat(41), "bad\nname".into()] {
        assert!(BasicProfile::new(key().user_id, name, None).is_err());
    }
    assert!(BasicProfile::new(key().user_id, "Name".into(), Some("a".repeat(161))).is_err());
    let profile = BasicProfile::new(
        key().user_id,
        "private-name".into(),
        Some("private-bio".into()),
    )
    .unwrap();
    let rendered = format!(
        "{profile:?} {:?} {:?} {:?}",
        credential(),
        claim(),
        Fake::new().access.binding.session_id
    );
    for private in [
        "private-name",
        "private-bio",
        "private-bearer",
        "opaque-private-claim-reference",
        &Uuid::from_u128(3).to_string(),
    ] {
        assert!(!rendered.contains(private));
    }
    let mut authority = Fake::new().session;
    authority
        .roles
        .push("private-role-or-email@example.test".into());
    assert!(!format!("{authority:?}").contains("private-role-or-email"));
    assert_eq!(profile.display_name(), "private-name");
    assert_eq!(profile.bio(), Some("private-bio"));
    assert!(SessionId::new("not-a-uuid".into()).is_err());
    assert!(SessionId::new(Uuid::from_u128(0xabcdef).to_string().to_uppercase()).is_err());
    assert!(AccessCredential::new("has whitespace".into()).is_err());
}

#[derive(Clone)]
struct Repository(Arc<AtomicUsize>);
#[async_trait]
impl CoreRepository for Repository {
    async fn event(&self, _: EventId) -> Result<Option<Event>, StoreError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
    async fn participant(&self, _: ParticipantKey) -> Result<Option<Participant>, StoreError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
    async fn avatar(&self, _: ParticipantKey) -> Result<Option<AvatarSettings>, StoreError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
    async fn update_preferences(
        &self,
        _: ParticipantKey,
        _: Revision,
        _: Preferences,
    ) -> Result<PreferenceUpdate, StoreError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(PreferenceUpdate::Missing)
    }
}
#[tokio::test]
async fn core_never_dispatches_persistence_when_identity_is_denied_or_unavailable() {
    for operation in [
        Operation::ReadOwnParticipant,
        Operation::ReadOwnAvatar,
        Operation::WriteOwnPreferences,
    ] {
        for error in [IdentityError::Denied, IdentityError::Unavailable] {
            let mut fake = Fake::new();
            let context = IdentityContext {
                key: key(),
                operation,
            };
            fake.access.binding.context = context;
            fake.session.binding.context = context;
            fake.eligibility.binding.context = context;
            fake.failure = Some((Stage::Eligibility, error));
            let count = Arc::new(AtomicUsize::new(0));
            let policy = IdentityPolicy::new(fake, TestClock::new(), credential(), context);
            let core = ConcertCore::new(Repository(count.clone()), TestClock::new(), policy);
            let result = match operation {
                Operation::ReadOwnParticipant => core.own_participant(key()).await.map(|_| ()),
                Operation::ReadOwnAvatar => core.own_avatar(key()).await.map(|_| ()),
                Operation::WriteOwnPreferences => core
                    .change_preferences(
                        key(),
                        Revision::initial(),
                        Preferences {
                            theme: Theme::Revenge,
                            low_power: false,
                            reduced_motion: false,
                            sound_enabled: true,
                        },
                    )
                    .await
                    .map(|_| ()),
            };
            assert_eq!(
                result,
                Err(if error == IdentityError::Denied {
                    CoreError::PermissionDenied
                } else {
                    CoreError::DependencyUnavailable
                })
            );
            assert_eq!(count.load(Ordering::SeqCst), 0);
        }
    }
    let fake = Fake::new();
    let calls = fake.calls.clone();
    let policy = IdentityPolicy::new(fake, TestClock::new(), credential(), context());
    assert_eq!(
        policy
            .authorize(key(), Operation::WriteOwnPreferences, now())
            .await,
        Err(PolicyError::Denied)
    );
    assert!(calls.lock().unwrap().is_empty());
}
#[tokio::test]
async fn email_only_policy_dispatches_repository_after_all_authorities_approve() {
    let fake = Fake::new();
    let calls = fake.calls.clone();
    let count = Arc::new(AtomicUsize::new(0));
    let core = ConcertCore::new(
        Repository(count.clone()),
        TestClock::new(),
        IdentityPolicy::new(fake, TestClock::new(), credential(), context()),
    );
    assert_eq!(
        core.own_participant(key()).await,
        Err(CoreError::ResourceUnavailable)
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(
        *calls.lock().unwrap(),
        vec![Stage::Access, Stage::Session, Stage::Eligibility]
    );
}
