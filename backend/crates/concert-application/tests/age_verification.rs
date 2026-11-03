pub use concert_application::{age_policy, identity};
#[path = "../src/age_verification/mod.rs"]
mod age_verification;
use age_verification::*;
use chrono::{DateTime, Duration, Utc};
use concert_domain::{
    age_policy::{AgeStatus, StoredStatus},
    AuthUserId, EventId,
};
use uuid::Uuid;
fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-11-03T14:59:00Z")
        .unwrap()
        .with_timezone(&Utc)
}
fn config() -> ProviderConfig {
    ProviderConfig::new(
        "fixture",
        "v1",
        "https://verify.example/authorize",
        &["https://verify.example/authorize"],
    )
    .unwrap()
}
fn context() -> age_policy::Context {
    age_policy::Context {
        actor: AuthUserId::from_uuid(Uuid::new_v4()),
        event: EventId::from_uuid(Uuid::new_v4()),
        session: Uuid::new_v4(),
        request: Uuid::new_v4(),
        expires_at: now() + Duration::days(120),
    }
}
fn challenge(c: age_policy::Context) -> Challenge {
    Challenge {
        binding: Binding {
            verification: Uuid::new_v4(),
            actor: c.actor.uuid(),
            event: c.event.uuid(),
            session: c.session,
            verifier: "fixture".into(),
            state_digest: [42; 32],
        },
        created_at: now() - Duration::minutes(1),
        expires_at: now() + Duration::minutes(9),
        source_revision: "v1".into(),
    }
}
fn proof(ch: &Challenge) -> VerifiedProof {
    VerifiedProof {
        binding: ch.binding.clone(),
        verified_at: now(),
        expires_at: now() + Duration::days(90),
        source_revision: "v1".into(),
        birthyear: "2000".into(),
        birthday: "11-04".into(),
    }
}
#[test]
fn authorization_is_fixed_exact_allowlisted_https() {
    for endpoint in [
        "http://verify.example/authorize",
        "https://user@verify.example/authorize",
        "https://verify.example:443/authorize",
        "https://verify.example/a?redirect=evil",
        "https://verify.example/a#x",
        "https://127.0.0.1/a",
        "https://verify.example/%61",
        "https://verify.example/../a",
    ] {
        assert!(ProviderConfig::new("fixture", "v1", endpoint, &[endpoint]).is_err());
    }
    let state = StateToken::from_random_bytes([9; 32]);
    let url = config().authorization_url(Uuid::new_v4(), &state);
    assert!(url.starts_with("https://verify.example/authorize?"));
    assert!(url.ends_with("return_target=AGE_PRIVACY"));
    assert!(ProviderConfig::new(
        "fixture",
        "v1",
        "https://evil.example/authorize",
        &["https://verify.example/authorize"]
    )
    .is_err());
}
#[test]
fn state_and_response_are_closed_secret_values() {
    let a = StateToken::from_random_bytes([1; 32]);
    let b = StateToken::from_random_bytes([2; 32]);
    assert_eq!(a.expose().len(), 64);
    assert_ne!(a.digest(), b.digest());
    assert!(StateToken::parse(a.expose().into()).is_ok());
    for value in ["x".repeat(64), "A".repeat(64), "a".repeat(65)] {
        assert!(StateToken::parse(value).is_err());
    }
    assert!(ProviderResponse::new("\nsecret".into()).is_err());
    assert!(ProviderResponse::new("a".repeat(4097)).is_err());
}
#[test]
fn provider_proof_must_match_every_binding() {
    let c = context();
    let ch = challenge(c);
    for field in 0..7 {
        let mut p = proof(&ch);
        match field {
            0 => p.binding.actor = Uuid::new_v4(),
            1 => p.binding.event = Uuid::new_v4(),
            2 => p.binding.session = Uuid::new_v4(),
            3 => p.binding.verification = Uuid::new_v4(),
            4 => p.binding.state_digest = [3; 32],
            5 => p.binding.verifier = "other".into(),
            _ => p.source_revision = "other".into(),
        };
        assert!(claim(p, &ch, &config(), now()).is_err());
    }
}
#[test]
fn proof_lifetime_and_state_deadline_are_strict() {
    let ch = challenge(context());
    for case in 0..5 {
        let mut p = proof(&ch);
        match case {
            0 => p.verified_at = now() + Duration::seconds(1),
            1 => p.verified_at = ch.created_at - Duration::seconds(1),
            2 => p.expires_at = now(),
            3 => p.expires_at = now() + Duration::days(91),
            _ => {}
        }
        assert!(claim(
            p,
            &ch,
            &config(),
            if case == 4 { ch.expires_at } else { now() }
        )
        .is_err());
    }
}
#[test]
fn seoul_birthday_expiry_and_missing_provider_remain_separate() {
    let c = context();
    let ch = challenge(c);
    let value = claim(proof(&ch), &ch, &config(), now()).unwrap();
    assert_eq!(value.age_at_verification, Some(25));
    assert_eq!(
        own_status(c, &[value.clone()], false, now())
            .unwrap()
            .status,
        AgeStatus::Verified
    );
    let birthday = DateTime::parse_from_rfc3339("2026-11-03T15:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(
        own_status(c, &[value.clone()], true, birthday)
            .unwrap()
            .status,
        AgeStatus::Verified
    );
    assert_eq!(
        own_status(c, &[value.clone()], true, value.expires_at)
            .unwrap()
            .status,
        AgeStatus::Expired
    );
    assert_eq!(
        own_status(c, &[], false, now()).unwrap().status,
        AgeStatus::ProviderUnavailable
    );
}
#[test]
fn unsupported_newest_and_revoked_claim_never_fall_back() {
    let c = context();
    let ch = challenge(c);
    let old = claim(proof(&ch), &ch, &config(), now()).unwrap();
    let mut p = proof(&ch);
    p.birthyear = "1900".into();
    let mut newest = claim(p, &ch, &config(), now()).unwrap();
    newest.claim_id = Uuid::new_v4();
    newest.verified_at = now() + Duration::seconds(1);
    assert_eq!(
        own_status(
            c,
            &[old.clone(), newest.clone()],
            true,
            now() + Duration::seconds(2)
        )
        .unwrap()
        .status,
        AgeStatus::Unsupported
    );
    newest.status = StoredStatus::Revoked;
    assert_eq!(
        own_status(c, &[old, newest], true, now() + Duration::seconds(2))
            .unwrap()
            .status,
        AgeStatus::Revoked
    );
}
#[test]
fn malformed_birth_proof_is_never_a_verified_claim() {
    let ch = challenge(context());
    for birth in ["02-30", "2-03", "00-00"] {
        let mut p = proof(&ch);
        p.birthday = birth.into();
        assert!(claim(p, &ch, &config(), now()).is_err());
    }
}
#[test]
fn stored_config_revision_is_part_of_binding() {
    let mut ch = challenge(context());
    ch.source_revision = "v0".into();
    assert!(claim(proof(&ch), &ch, &config(), now()).is_err());
}
#[test]
fn command_target_stable_but_session_is_not_authority() {
    let c = context();
    let key = Uuid::new_v4();
    assert_eq!(verification_id(c, key), verification_id(c, key));
    let mut other = c;
    other.actor = AuthUserId::from_uuid(Uuid::new_v4());
    assert_ne!(verification_id(c, key), verification_id(other, key));
    other = c;
    other.session = Uuid::nil();
    assert!(check(other, now()).is_err());
}
#[tokio::test]
async fn unavailable_provider_cannot_create_a_proof() {
    let ch = challenge(context());
    assert!(!UnavailableVerifier.available());
    assert!(matches!(
        UnavailableVerifier
            .verify(&ch, &ProviderResponse::new("fixture".into()).unwrap())
            .await,
        Err(Error::ProviderUnavailable)
    ));
}
