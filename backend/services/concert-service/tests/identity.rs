use async_trait::async_trait;
use chrono::{DateTime, Duration, TimeZone, Utc};
use concert_application::{identity::*, Clock, Operation};
use concert_application::{ParticipationPolicy, PolicyError};
use concert_domain::{AuthUserId, EventId, ParticipantKey};
use concert_service::adapters::out::{identity::*, UnavailablePolicy};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
fn now() -> DateTime<Utc> {
    Utc.timestamp_opt(1_900_000_000, 0).unwrap()
}
struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        now()
    }
}
fn context() -> IdentityContext {
    IdentityContext {
        key: ParticipantKey {
            user_id: AuthUserId::from_uuid(Uuid::from_u128(1)),
            event_id: EventId::from_uuid(Uuid::from_u128(2)),
        },
        operation: Operation::ReadOwnParticipant,
    }
}
fn credential() -> AccessCredential {
    AccessCredential::new("secret-access".into()).unwrap()
}
fn claim() -> ClaimReference {
    ClaimReference::new("private-owner-reference".into()).unwrap()
}
fn proof() -> OwnerProof {
    OwnerProof {
        user_id: context().key.user_id.uuid().to_string(),
        event_id: context().key.event_id.uuid().to_string(),
        session_id: Uuid::from_u128(3).to_string(),
        operation: context().operation,
        checked_at: now(),
        expires_at: now() + Duration::seconds(20),
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
struct Owner {
    access: OwnerProof,
    session: OwnerSession,
    eligibility: OwnerEligibility,
    profile: OwnerProfile,
    provider: OwnerProvider,
    failure: Option<(Stage, OwnerFailure)>,
    calls: Arc<Mutex<Vec<Stage>>>,
}
impl Owner {
    fn new() -> Self {
        Self {
            access: proof(),
            session: OwnerSession {
                proof: proof(),
                active: true,
                roles: vec!["ROLE_USER".into()],
            },
            eligibility: OwnerEligibility {
                proof: proof(),
                state: EligibilityState::Eligible,
            },
            profile: OwnerProfile {
                proof: proof(),
                user_id: proof().user_id,
                nickname: "Email participant".into(),
                bio: String::new(),
                lifecycle_status: "ACTIVE".into(),
            },
            provider: OwnerProvider {
                proof: proof(),
                provider: Provider::Email,
                claim: claim(),
                attached: true,
            },
            failure: None,
            calls: Arc::new(Mutex::new(vec![])),
        }
    }
    fn check(&self, stage: Stage) -> Result<(), OwnerFailure> {
        self.calls.lock().unwrap().push(stage);
        match self.failure {
            Some((on, error)) if on == stage => Err(error),
            _ => Ok(()),
        }
    }
    fn proof(&mut self, stage: Stage) -> &mut OwnerProof {
        match stage {
            Stage::Access => &mut self.access,
            Stage::Session => &mut self.session.proof,
            Stage::Eligibility => &mut self.eligibility.proof,
            Stage::Profile => &mut self.profile.proof,
            Stage::Provider => &mut self.provider.proof,
        }
    }
}
#[async_trait]
impl OwnerIdentityClient for Owner {
    async fn authenticate(
        &self,
        _: &AccessCredential,
        _: IdentityContext,
        _: DateTime<Utc>,
    ) -> Result<OwnerProof, OwnerFailure> {
        self.check(Stage::Access)?;
        Ok(self.access.clone())
    }
    async fn check_session(
        &self,
        _: &AuthorityBinding,
        _: DateTime<Utc>,
    ) -> Result<OwnerSession, OwnerFailure> {
        self.check(Stage::Session)?;
        Ok(self.session.clone())
    }
    async fn check_eligibility(
        &self,
        _: &AuthorityBinding,
        _: DateTime<Utc>,
    ) -> Result<OwnerEligibility, OwnerFailure> {
        self.check(Stage::Eligibility)?;
        Ok(self.eligibility.clone())
    }
    async fn get_profile(
        &self,
        _: &AuthorityBinding,
        _: DateTime<Utc>,
    ) -> Result<OwnerProfile, OwnerFailure> {
        self.check(Stage::Profile)?;
        Ok(self.profile.clone())
    }
    async fn check_provider(
        &self,
        _: &AuthorityBinding,
        _: Provider,
        _: &ClaimReference,
        _: DateTime<Utc>,
    ) -> Result<OwnerProvider, OwnerFailure> {
        self.check(Stage::Provider)?;
        Ok(self.provider.clone())
    }
}
async fn run(owner: Owner, stage: Stage) -> Result<(), IdentityError> {
    let resolver = IdentityResolver::new(OwnerIdentityAdapter::new(owner), FixedClock);
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
async fn email_only_adapter_maps_minimal_profile_without_legacy_avatar_or_x_lookup() {
    let owner = Owner::new();
    let calls = owner.calls.clone();
    let resolver = IdentityResolver::new(OwnerIdentityAdapter::new(owner), FixedClock);
    let profile = resolver
        .basic_profile(&credential(), context())
        .await
        .unwrap();
    assert_eq!(profile.user_id(), context().key.user_id);
    assert_eq!(profile.display_name(), "Email participant");
    assert_eq!(profile.bio(), None);
    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            Stage::Access,
            Stage::Session,
            Stage::Eligibility,
            Stage::Profile
        ]
    );
    assert!(!format!("{profile:?}").contains("Email participant"));
}
#[tokio::test]
async fn malformed_owner_identifiers_and_windows_are_dependency_failures() {
    for stage in [
        Stage::Access,
        Stage::Session,
        Stage::Eligibility,
        Stage::Profile,
        Stage::Provider,
    ] {
        for field in 0..5 {
            let mut owner = Owner::new();
            let proof = owner.proof(stage);
            match field {
                0 => proof.user_id = "not-a-user".into(),
                1 => proof.event_id = "not-an-event".into(),
                2 => proof.session_id = "not-a-session".into(),
                3 => proof.user_id = Uuid::from_u128(0xabcdef).to_string().to_uppercase(),
                _ => proof.expires_at = proof.checked_at,
            };
            assert_eq!(
                run(owner, stage).await,
                Err(IdentityError::Unavailable),
                "{stage:?} {field}"
            );
        }
    }
}
#[tokio::test]
async fn owner_proofs_cannot_substitute_other_users_events_operations_sessions_or_times() {
    for stage in [
        Stage::Access,
        Stage::Session,
        Stage::Eligibility,
        Stage::Profile,
        Stage::Provider,
    ] {
        for field in 0..6 {
            let mut owner = Owner::new();
            let proof = owner.proof(stage);
            match field {
                0 => proof.user_id = Uuid::from_u128(90).to_string(),
                1 => proof.event_id = Uuid::from_u128(91).to_string(),
                2 => proof.operation = Operation::ReadOwnAvatar,
                3 => proof.session_id = Uuid::from_u128(92).to_string(),
                4 => proof.checked_at = now() - Duration::seconds(1),
                _ => {
                    proof.checked_at = now() - Duration::seconds(1);
                    proof.expires_at = now();
                }
            };
            assert_eq!(
                run(owner, stage).await,
                Err(IdentityError::Denied),
                "{stage:?} {field}"
            );
        }
    }
}
#[tokio::test]
async fn current_session_denial_is_not_authenticated_permission_or_transport_outage() {
    for (error, expected) in [
        (
            OwnerFailure::CredentialRejected,
            IdentityError::Unauthenticated,
        ),
        (OwnerFailure::PermissionDenied, IdentityError::Denied),
        (OwnerFailure::Timeout, IdentityError::Unavailable),
        (OwnerFailure::Unavailable, IdentityError::Unavailable),
        (OwnerFailure::Malformed, IdentityError::Unavailable),
        (OwnerFailure::CallerRejected, IdentityError::Unavailable),
    ] {
        for stage in [
            Stage::Access,
            Stage::Session,
            Stage::Eligibility,
            Stage::Profile,
            Stage::Provider,
        ] {
            let mut owner = Owner::new();
            owner.failure = Some((stage, error));
            assert_eq!(
                run(owner, stage).await,
                Err(
                    if stage != Stage::Access && error == OwnerFailure::CredentialRejected {
                        IdentityError::Unavailable
                    } else {
                        expected
                    }
                ),
                "{stage:?} {error:?}"
            );
        }
    }
    let mut owner = Owner::new();
    owner.session.active = false;
    assert_eq!(run(owner, Stage::Session).await, Err(IdentityError::Denied));
    for roles in [vec![], vec!["ROLE_ADMIN".into()]] {
        let mut owner = Owner::new();
        owner.session.roles = roles;
        assert_eq!(run(owner, Stage::Session).await, Err(IdentityError::Denied));
    }
}
#[tokio::test]
async fn suspension_unknown_authority_and_provider_mismatch_never_attach_claims() {
    for state in [
        EligibilityState::Suspended,
        EligibilityState::Ineligible,
        EligibilityState::Unknown,
    ] {
        let mut owner = Owner::new();
        owner.eligibility.state = state;
        assert_eq!(
            run(owner, Stage::Provider).await,
            Err(if state == EligibilityState::Unknown {
                IdentityError::Unavailable
            } else {
                IdentityError::Denied
            })
        );
    }
    for field in 0..3 {
        let mut owner = Owner::new();
        match field {
            0 => owner.provider.provider = Provider::X,
            1 => owner.provider.claim = ClaimReference::new("foreign-ref".into()).unwrap(),
            _ => owner.provider.attached = false,
        };
        assert_eq!(
            run(owner, Stage::Provider).await,
            Err(IdentityError::Denied)
        );
    }
    assert_eq!(run(Owner::new(), Stage::Provider).await, Ok(()));
}
#[tokio::test]
async fn malformed_cross_actor_deleted_unknown_or_oversized_profile_cannot_project() {
    for field in 0..7 {
        let mut owner = Owner::new();
        let expected = match field {
            0 => {
                owner.profile.user_id = "malformed".into();
                IdentityError::Unavailable
            }
            1 => {
                owner.profile.user_id = Uuid::from_u128(90).to_string();
                IdentityError::Denied
            }
            2 => {
                owner.profile.lifecycle_status = "DELETED".into();
                IdentityError::Denied
            }
            3 => {
                owner.profile.lifecycle_status = "UNKNOWN".into();
                IdentityError::Unavailable
            }
            4 => {
                owner.profile.nickname = String::new();
                IdentityError::Unavailable
            }
            5 => {
                owner.profile.nickname = "a".repeat(41);
                IdentityError::Unavailable
            }
            _ => {
                owner.profile.bio = "a".repeat(161);
                IdentityError::Unavailable
            }
        };
        assert_eq!(run(owner, Stage::Profile).await, Err(expected));
    }
}
#[tokio::test]
async fn unsupported_production_authority_remains_explicitly_unavailable() {
    let resolver = IdentityResolver::new(OwnerIdentityAdapter::new(UnavailableOwner), FixedClock);
    assert_eq!(
        resolver.authorize(&credential(), context(), now()).await,
        Err(IdentityError::Unavailable)
    );
    assert_eq!(
        UnavailablePolicy
            .authorize(context().key, context().operation, now())
            .await,
        Err(PolicyError::Unavailable)
    );
}
