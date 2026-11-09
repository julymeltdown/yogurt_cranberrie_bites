//! T-301: the identity claim port.
//!
//! The assertions here are about what the port *cannot* do: it cannot render an
//! age, it cannot turn a failure into a value, and it cannot make the provider's
//! explicit "not specified" answer look like a live one.
use async_trait::async_trait;
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use concert_application::identity_claims::{
    Attribute, ClaimSource, ClaimsError, Gender, IdentityClaims, IdentityClaimsPort, ProfileId,
    StaticClaims, UnavailableClaims,
};
use concert_domain::age_policy::arithmetic::{derive_exact_age_claim, SeoulDate};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use uuid::Uuid;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 9, 5, 0, 0).unwrap()
}

fn source() -> ClaimSource {
    ClaimSource {
        verifier: "naver".to_owned(),
        source_revision: "naver.nid.me.v1".to_owned(),
        verified_at: now() - Duration::days(1),
        expires_at: now() + Duration::days(89),
    }
}

fn age_claim() -> concert_domain::age_policy::arithmetic::ExactAgeClaim {
    derive_exact_age_claim(
        Some("2000"),
        Some("07-12"),
        SeoulDate::new(NaiveDate::from_ymd_opt(2026, 7, 12).unwrap()),
    )
    .expect("a supported birth date")
}

fn profile() -> ProfileId {
    ProfileId::from_uuid(Uuid::new_v4())
}

#[tokio::test]
async fn a_live_claim_carries_both_attributes_and_its_source() {
    let port = StaticClaims::live(
        Attribute::Known(age_claim()),
        Attribute::Known(Gender::Female),
        source(),
    );
    let subject = profile();
    let claims = port.read(subject, now()).await.expect("a live claim");
    assert_eq!(claims.profile, subject);
    assert_eq!(claims.gender, Attribute::Known(Gender::Female));
    assert!(claims.age.known().is_some());
    assert_eq!(claims.source.as_ref().map(|s| s.verifier.as_str()), Some("naver"));
    assert_eq!(claims.evaluated_at, now());
}

#[tokio::test]
async fn a_transport_failure_never_becomes_a_value() {
    let port = StaticClaims::failed();
    assert_eq!(port.read(profile(), now()).await.err(), Some(ClaimsError::Unavailable));
    // The unconfigured composition root behaves the same way.
    assert_eq!(
        UnavailableClaims.read(profile(), now()).await.err(),
        Some(ClaimsError::Unavailable)
    );
}

#[tokio::test]
async fn the_no_claim_snapshot_is_unavailable_not_unspecified() {
    let claims = IdentityClaims::unavailable(profile(), now());
    assert!(claims.age.is_unavailable());
    assert!(claims.gender.is_unavailable());
    // "We have nothing" is not "the provider said not specified".
    assert!(!claims.age.is_evaluable());
    assert!(!claims.gender.is_evaluable());
    assert!(claims.source.is_none());
}

#[test]
fn unspecified_is_evaluable_and_known_only_returns_a_live_value() {
    let unspecified: Attribute<Gender> = Attribute::Unspecified;
    assert!(unspecified.is_evaluable());
    assert_eq!(unspecified.known(), None);
    assert_eq!(Attribute::Known(Gender::Male).known(), Some(Gender::Male));
    assert_eq!(Attribute::<Gender>::Unavailable.known(), None);
    // `map` preserves the two non-live states.
    assert_eq!(unspecified.map(|_| Gender::Female), Attribute::Unspecified);
    assert_eq!(
        Attribute::<Gender>::Unavailable.map(|_| Gender::Female),
        Attribute::Unavailable
    );
}

#[test]
fn debug_never_renders_the_numeric_age() {
    let claims = IdentityClaims {
        profile: profile(),
        age: Attribute::Known(age_claim()),
        gender: Attribute::Known(Gender::Female),
        source: Some(source()),
        evaluated_at: now(),
    };
    let rendered = format!("{claims:?}");
    // The claim is a 26-year-old on the fixture date; neither the age at
    // verification nor the current age may appear.
    //
    // The scan covers the attribute fields only. A profile id is a random UUID,
    // so its hex can contain "26" or "27" by chance and scanning it would make
    // this test fail for reasons unrelated to redaction. The fields that could
    // carry an age are the two attributes, and the source carries no age at all.
    let attributes = rendered
        .split("source:")
        .next()
        .expect("the rendered claim has a source field");
    let attributes = attributes
        .split("profile:")
        .nth(1)
        .and_then(|rest| rest.split_once(',').map(|(_, tail)| tail))
        .expect("the rendered claim has a profile field followed by the attributes");
    assert!(!attributes.contains("26"), "{rendered}");
    assert!(!attributes.contains("27"), "{rendered}");
    assert!(attributes.contains("REDACTED"), "{rendered}");
    // The attribute renders the same way on its own.
    let attribute = format!("{:?}", claims.age);
    assert_eq!(attribute, "Known([REDACTED])");
    // The non-live states still render, so a log line is useful.
    assert_eq!(format!("{:?}", Attribute::<Gender>::Unspecified), "Unspecified");
    assert_eq!(format!("{:?}", Attribute::<Gender>::Unavailable), "Unavailable");
}

#[test]
fn the_port_is_object_safe() {
    // The composition root stores the adapter behind `Arc<dyn IdentityClaimsPort>`,
    // so the trait must be usable as a trait object. This is a compile-time
    // assertion with a runtime body.
    let port: Arc<dyn IdentityClaimsPort> = Arc::new(StaticClaims::failed());
    assert!(Arc::strong_count(&port) == 1);
}

/// A port that records how many times it was read, so a caller can prove it did
/// not read claims at all.
struct CountingClaims {
    reads: AtomicUsize,
}

#[async_trait]
impl IdentityClaimsPort for CountingClaims {
    async fn read(
        &self,
        profile: ProfileId,
        now: DateTime<Utc>,
    ) -> Result<IdentityClaims, ClaimsError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(IdentityClaims::unavailable(profile, now))
    }
}

#[tokio::test]
async fn a_reader_can_count_its_own_reads() {
    // This double is what the visibility tests use to assert the unrestricted
    // path performs no read. Proving the double counts is what makes that
    // assertion meaningful.
    let port = CountingClaims {
        reads: AtomicUsize::new(0),
    };
    assert_eq!(port.reads.load(Ordering::SeqCst), 0);
    port.read(profile(), now()).await.expect("reads");
    assert_eq!(port.reads.load(Ordering::SeqCst), 1);
}

#[test]
fn gender_buckets_round_trip_and_unknown_is_not_a_gender() {
    assert_eq!(Gender::Female.bucket(), "female");
    assert_eq!(Gender::Male.bucket(), "male");
    assert_eq!(Gender::parse("female"), Some(Gender::Female));
    assert_eq!(Gender::parse("male"), Some(Gender::Male));
    // The explicit "not specified" bucket is a different state, not a variant.
    assert_eq!(Gender::parse("unknown"), None);
    assert_eq!(Gender::parse("F"), None);
}

#[test]
fn profile_ids_are_canonical_or_nothing() {
    let id = Uuid::new_v4();
    assert_eq!(ProfileId::parse(&id.to_string()).map(ProfileId::uuid), Some(id));
    // Uppercase, braced and nil forms are not canonical identities.
    assert_eq!(ProfileId::parse(&id.to_string().to_uppercase()), None);
    assert_eq!(ProfileId::parse(&format!("{{{id}}}")), None);
    assert_eq!(ProfileId::parse(&Uuid::nil().to_string()), None);
    assert_eq!(ProfileId::parse("not-a-uuid"), None);
}
