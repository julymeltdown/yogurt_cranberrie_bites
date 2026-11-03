use auth_application::concert_identity::*;
use chrono::Utc;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[test]
fn canonical_ids_must_be_present() {
    let id = Uuid::new_v4();
    assert_eq!(
        AdmissionRead::new(Uuid::nil(), id, None).unwrap_err(),
        ReadError::InvalidInput
    );
    assert_eq!(
        AdmissionRead::new(id, Uuid::nil(), None).unwrap_err(),
        ReadError::InvalidInput
    );
    assert!(AdmissionRead::new(id, id, None).is_ok());
}
#[test]
fn exact_binding_rejects_malformed_foreign_shapes_without_echo() {
    for id in [
        "private-subject",
        "",
        "00000000-0000-0000-0000-000000000000",
        "123456781234123412341234567890ab",
    ] {
        assert_eq!(
            ProviderBinding::parse(id, "X", 1).unwrap_err(),
            ReadError::InvalidInput
        );
    }
    for provider in ["", "INSTAGRAM", "x", "X ", "private-provider"] {
        assert_eq!(
            ProviderBinding::parse(&Uuid::new_v4().to_string(), provider, 1).unwrap_err(),
            ReadError::InvalidInput
        );
    }
    for epoch in [i64::MIN, -1, 0, ProviderBinding::MAX_EPOCH + 1, i64::MAX] {
        assert_eq!(
            ProviderBinding::new(Uuid::new_v4(), BindingProvider::X, epoch).unwrap_err(),
            ReadError::InvalidInput
        );
    }
}
#[test]
fn exact_binding_has_only_owner_reference_provider_and_positive_epoch() {
    let id = Uuid::new_v4();
    for name in ["EMAIL", "GOOGLE", "APPLE", "X"] {
        let binding =
            ProviderBinding::parse(&id.to_string(), name, ProviderBinding::MAX_EPOCH).unwrap();
        assert_eq!(binding.identity_id(), id);
        assert_eq!(binding.epoch(), ProviderBinding::MAX_EPOCH);
        assert_eq!(binding.provider().owner_name(), name);
    }
}
#[test]
fn eligibility_fails_closed_for_every_account_session_binding_combination() {
    for account in [AccountEligibility::Active, AccountEligibility::Ineligible] {
        for session in [SessionEligibility::Live, SessionEligibility::Ended] {
            for binding in [
                BindingEligibility::Bound,
                BindingEligibility::NotBound,
                BindingEligibility::NotRequested,
            ] {
                let value = CurrentAdmission {
                    observed_at: Utc::now(),
                    account,
                    session,
                    binding,
                };
                assert_eq!(
                    value.eligible(),
                    account == AccountEligibility::Active
                        && session == SessionEligibility::Live
                        && binding != BindingEligibility::NotBound
                );
            }
        }
    }
}
#[test]
fn cancellation_is_shared_and_never_reset() {
    let cancel = ReadCancellation::default();
    let budget = ReadBudget::new(Instant::now() + Duration::from_secs(1), cancel.clone()).unwrap();
    assert!(budget.check().is_ok());
    cancel.cancel();
    assert_eq!(budget.check(), Err(ReadError::Cancelled));
    assert_eq!(budget.clone().remaining(), Err(ReadError::Cancelled));
}
#[test]
fn expired_budget_and_unbounded_budget_are_rejected() {
    let budget = ReadBudget::new(
        Instant::now() - Duration::from_secs(1),
        ReadCancellation::default(),
    )
    .unwrap();
    assert_eq!(budget.check(), Err(ReadError::DeadlineExceeded));
    assert_eq!(
        ReadBudget::new(
            Instant::now() + Duration::from_secs(3),
            ReadCancellation::default()
        )
        .unwrap_err(),
        ReadError::InvalidInput
    );
}
#[test]
fn cloning_budget_preserves_absolute_deadline() {
    let deadline = Instant::now() + Duration::from_secs(1);
    let budget = ReadBudget::new(deadline, ReadCancellation::default()).unwrap();
    assert_eq!(budget.clone().deadline(), deadline);
}
#[test]
fn diagnostics_do_not_include_ids_reference_epoch_or_owner_facts() {
    let id = Uuid::new_v4();
    let binding = ProviderBinding::new(id, BindingProvider::X, 987654321).unwrap();
    let request = AdmissionRead::new(id, id, Some(binding)).unwrap();
    let output = CurrentAdmission {
        observed_at: Utc::now(),
        account: AccountEligibility::Active,
        session: SessionEligibility::Live,
        binding: BindingEligibility::Bound,
    };
    let diagnostic = format!("{binding:?} {request:?} {output:?}");
    assert!(!diagnostic.contains(&id.to_string()));
    assert!(!diagnostic.contains("987654321"));
    assert!(!diagnostic.contains("Active"));
    for error in [
        ReadError::InvalidInput,
        ReadError::Cancelled,
        ReadError::DeadlineExceeded,
        ReadError::Unavailable,
    ] {
        assert!(format!("{error:?} {error}").len() < 100);
    }
}
