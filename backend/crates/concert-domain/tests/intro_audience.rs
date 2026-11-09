//! T-303: the audience value and its validation.
//!
//! Every case here is a rule the schema CHECK in `0082_intro_posts.sql` also
//! enforces. The value type must refuse exactly what the database refuses, so a
//! body that reaches the database cannot be rejected there for a reason the
//! domain accepted.
use concert_domain::intro_audience::{
    AudienceError, ExactAge, GenderCondition, IntroAudience, MaximumAgeExclusive,
    MAX_AUDIENCE_MAXIMUM_EXCLUSIVE, MAX_AUDIENCE_MINIMUM_AGE, MIN_AUDIENCE_AGE,
    MIN_AUDIENCE_MAXIMUM_EXCLUSIVE,
};

fn ages(minimum: i16, maximum: i16) -> Result<IntroAudience, AudienceError> {
    IntroAudience::from_parts("any", Some(minimum), Some(maximum))
}

#[test]
fn unpaired_bounds_are_rejected() {
    assert_eq!(
        IntroAudience::from_parts("any", Some(18), None),
        Err(AudienceError::UnpairedBounds)
    );
    assert_eq!(
        IntroAudience::from_parts("any", None, Some(30)),
        Err(AudienceError::UnpairedBounds)
    );
}

#[test]
fn minimum_at_or_above_maximum_is_rejected() {
    assert_eq!(ages(30, 30), Err(AudienceError::InvalidBounds));
    assert_eq!(ages(31, 30), Err(AudienceError::InvalidBounds));
    // One apart is the smallest legal window: exactly one age.
    assert!(ages(29, 30).is_ok());
}

#[test]
fn bounds_outside_the_configured_window_are_rejected() {
    assert_eq!(ages(MIN_AUDIENCE_AGE - 1, 30), Err(AudienceError::InvalidBounds));
    assert_eq!(
        ages(MAX_AUDIENCE_MINIMUM_AGE + 1, MAX_AUDIENCE_MAXIMUM_EXCLUSIVE),
        Err(AudienceError::InvalidBounds)
    );
    assert_eq!(
        ages(18, MIN_AUDIENCE_MAXIMUM_EXCLUSIVE - 1),
        Err(AudienceError::InvalidBounds)
    );
    assert_eq!(
        ages(MAX_AUDIENCE_MINIMUM_AGE, MAX_AUDIENCE_MAXIMUM_EXCLUSIVE + 1),
        Err(AudienceError::InvalidBounds)
    );
    assert!(ages(MIN_AUDIENCE_AGE, MIN_AUDIENCE_MAXIMUM_EXCLUSIVE).is_ok());
    assert!(ages(MAX_AUDIENCE_MINIMUM_AGE, MAX_AUDIENCE_MAXIMUM_EXCLUSIVE).is_ok());
}

#[test]
fn is_restricted_is_false_only_for_any_with_no_bounds() {
    assert!(!IntroAudience::ANYONE.is_restricted());
    assert!(!IntroAudience::default().is_restricted());
    // Each restriction on its own is enough.
    for gender in [
        GenderCondition::Female,
        GenderCondition::Male,
        GenderCondition::Unknown,
    ] {
        let audience = IntroAudience::new(gender, None, None).expect("gender-only audience");
        assert!(audience.is_restricted(), "{gender:?} must restrict");
        assert!(!audience.has_age_bound(), "{gender:?} carries no age bound");
    }
    let age_only = ages(18, 30).expect("age-only audience");
    assert!(age_only.is_restricted());
    assert!(age_only.has_age_bound());
}

#[test]
fn unknown_is_a_condition_and_never_a_wildcard() {
    assert!(GenderCondition::Unknown.restricts());
    assert!(!GenderCondition::Any.restricts());
    // The wire names are the donor's lowercase values.
    for (text, expected) in [
        ("any", GenderCondition::Any),
        ("female", GenderCondition::Female),
        ("male", GenderCondition::Male),
        ("unknown", GenderCondition::Unknown),
    ] {
        assert_eq!(GenderCondition::parse(text), Ok(expected));
        assert_eq!(expected.as_str(), text);
    }
    assert_eq!(
        GenderCondition::parse("ANY"),
        Err(AudienceError::InvalidGender)
    );
    assert_eq!(
        GenderCondition::parse("nonbinary"),
        Err(AudienceError::InvalidGender)
    );
}

#[test]
fn the_half_open_window_admits_exactly_the_shown_range() {
    // "Ages 18-29" is 18 inclusive to 30 exclusive.
    let audience = ages(18, 30).expect("audience");
    assert_eq!(audience.minimum_age().map(ExactAge::get), Some(18));
    assert_eq!(
        audience.maximum_age_exclusive().map(MaximumAgeExclusive::get),
        Some(30)
    );
    assert_eq!(
        audience
            .maximum_age_exclusive()
            .map(MaximumAgeExclusive::inclusive_upper),
        Some(29)
    );
    assert!(!audience.admits_age(17));
    assert!(audience.admits_age(18));
    assert!(audience.admits_age(29));
    assert!(!audience.admits_age(30));
    // An unbounded audience admits every age; the decision never calls this for
    // an unknown age.
    assert!(IntroAudience::ANYONE.admits_age(14));
    assert!(IntroAudience::ANYONE.admits_age(100));
}

#[test]
fn summaries_are_derived_from_the_value() {
    assert_eq!(IntroAudience::ANYONE.summary(), "Anyone");
    assert_eq!(
        IntroAudience::from_parts("female", Some(18), Some(30))
            .expect("audience")
            .summary(),
        "Women · Ages 18–29"
    );
    assert_eq!(
        IntroAudience::from_parts("male", None, None)
            .expect("audience")
            .summary(),
        "Men"
    );
    assert_eq!(
        IntroAudience::from_parts("unknown", Some(18), Some(101))
            .expect("audience")
            .summary(),
        "Gender not specified · Ages 18–100"
    );
    assert_eq!(
        IntroAudience::from_parts("any", Some(18), Some(101))
            .expect("audience")
            .summary(),
        "Ages 18–100"
    );
}

#[test]
fn strict_deserialization_refuses_unknown_fields_and_bad_bounds() {
    let ok: IntroAudience =
        serde_json::from_str(r#"{"allowed_gender":"female","minimum_age":18,"maximum_age_exclusive":30}"#)
            .expect("a complete audience deserializes");
    assert_eq!(ok.gender(), GenderCondition::Female);

    // An unknown field is refused rather than ignored.
    assert!(serde_json::from_str::<IntroAudience>(
        r#"{"allowed_gender":"any","minimum_age":null,"maximum_age_exclusive":null,"verified_only":true}"#
    )
    .is_err());

    // Every field is required: an omitted bound is not "no bound".
    assert!(serde_json::from_str::<IntroAudience>(
        r#"{"allowed_gender":"any","maximum_age_exclusive":null}"#
    )
    .is_err());

    // Unpaired, unordered and out-of-range bounds are all refused.
    for body in [
        r#"{"allowed_gender":"any","minimum_age":18,"maximum_age_exclusive":null}"#,
        r#"{"allowed_gender":"any","minimum_age":null,"maximum_age_exclusive":30}"#,
        r#"{"allowed_gender":"any","minimum_age":30,"maximum_age_exclusive":30}"#,
        r#"{"allowed_gender":"any","minimum_age":13,"maximum_age_exclusive":30}"#,
        r#"{"allowed_gender":"any","minimum_age":18,"maximum_age_exclusive":102}"#,
        r#"{"allowed_gender":"nonbinary","minimum_age":null,"maximum_age_exclusive":null}"#,
    ] {
        assert!(
            serde_json::from_str::<IntroAudience>(body).is_err(),
            "must refuse {body}"
        );
    }
}

#[test]
fn serialization_keeps_the_half_open_upper_bound() {
    let audience = IntroAudience::from_parts("female", Some(18), Some(30)).expect("audience");
    let value = serde_json::to_value(audience).expect("serializes");
    assert_eq!(value["allowed_gender"], serde_json::json!("female"));
    assert_eq!(value["minimum_age"], serde_json::json!(18));
    assert_eq!(value["maximum_age_exclusive"], serde_json::json!(30));
    // Round trip.
    let back: IntroAudience = serde_json::from_value(value).expect("round trips");
    assert_eq!(back, audience);
}

#[test]
fn the_two_bound_types_are_not_interchangeable() {
    // The point of separate newtypes: an inclusive age cannot be passed where an
    // exclusive bound is expected. This is a compile-time property, so the test
    // asserts the runtime ranges instead.
    assert_eq!(ExactAge::new(MIN_AUDIENCE_AGE).map(ExactAge::get), Ok(14));
    assert_eq!(ExactAge::new(101), Err(AudienceError::InvalidBounds));
    assert_eq!(
        MaximumAgeExclusive::new(15).map(MaximumAgeExclusive::get),
        Ok(15)
    );
    assert_eq!(
        MaximumAgeExclusive::new(14),
        Err(AudienceError::InvalidBounds)
    );
}
