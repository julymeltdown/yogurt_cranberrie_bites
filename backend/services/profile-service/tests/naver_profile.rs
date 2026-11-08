//! T-201: provider profile decoding.
//!
//! These are the seven mapping cases the task document names, plus the size
//! bound. The decoder is pure, so this binary needs no database and no provider:
//! it feeds literal `/v1/nid/me` bodies to the production `decode_profile`.
//!
//! The mapping is the donor's, and the point of the test is the *negative* half
//! of it: a missing or unrecognized gender maps to `None`, never to `Unknown`.
//! That distinction is what keeps a user without a provider gender from
//! satisfying an `unknown` audience.
use profile_service::identity::naver_profile::{
    decode_profile, NaverGender, NaverProfileDecodeError, MAX_PROFILE_BODY_BYTES,
};

fn body(gender: Option<&str>) -> String {
    let gender = match gender {
        Some(value) => format!("\"{value}\""),
        None => "null".to_owned(),
    };
    format!(
        r#"{{"resultcode":"00","message":"success","response":{{"id":"subject-1","email":null,"age":null,"gender":{gender},"birthyear":"2000","birthday":"07-12"}}}}"#
    )
}

/// The donor's table, case by case.
#[test]
fn the_donor_gender_mapping_is_preserved_exactly() {
    // V-12-c: F -> female.
    assert_eq!(
        decode_profile(body(Some("F")).as_bytes())
            .expect("F decodes")
            .normalized_gender(),
        Some(NaverGender::Female)
    );
    // V-12-d: M -> male.
    assert_eq!(
        decode_profile(body(Some("M")).as_bytes())
            .expect("M decodes")
            .normalized_gender(),
        Some(NaverGender::Male)
    );
    // V-12-e: U -> unknown. This is the *only* producer of the unknown bucket.
    assert_eq!(
        decode_profile(body(Some("U")).as_bytes())
            .expect("U decodes")
            .normalized_gender(),
        Some(NaverGender::Unknown)
    );
    // The donor also accepted the spelled-out forms.
    assert_eq!(
        decode_profile(body(Some("female")).as_bytes())
            .expect("female decodes")
            .normalized_gender(),
        Some(NaverGender::Female)
    );
    assert_eq!(
        decode_profile(body(Some("male")).as_bytes())
            .expect("male decodes")
            .normalized_gender(),
        Some(NaverGender::Male)
    );
    assert_eq!(
        decode_profile(body(Some("unknown")).as_bytes())
            .expect("unknown decodes")
            .normalized_gender(),
        Some(NaverGender::Unknown)
    );
}

/// V-12-g: an unrecognized value yields `None`, which fails the verification.
#[test]
fn an_unrecognized_gender_yields_none() {
    for value in ["X", "", "f", "m", "u", "FEMALE", "Female", "0", " ", "nonbinary"] {
        assert_eq!(
            decode_profile(body(Some(value)).as_bytes())
                .unwrap_or_else(|_| panic!("{value:?} is a decodable body"))
                .normalized_gender(),
            None,
            "{value:?} must not map to a bucket"
        );
    }
}

/// V-12-f: an absent gender yields `None`, which fails the verification.
#[test]
fn an_absent_gender_yields_none() {
    let absent = r#"{"resultcode":"00","response":{"id":"subject-1","gender":null,"birthyear":"2000","birthday":"07-12"}}"#;
    assert_eq!(
        decode_profile(absent.as_bytes()).expect("decodes").normalized_gender(),
        None
    );
    let missing = r#"{"resultcode":"00","response":{"id":"subject-1","birthyear":"2000","birthday":"07-12"}}"#;
    assert_eq!(
        decode_profile(missing.as_bytes()).expect("decodes").normalized_gender(),
        None
    );
}

/// The bucket strings are the schema's own CHECK values.
#[test]
fn the_gender_bucket_strings_are_the_schema_values() {
    assert_eq!(NaverGender::Female.gender_bucket(), "female");
    assert_eq!(NaverGender::Male.gender_bucket(), "male");
    assert_eq!(NaverGender::Unknown.gender_bucket(), "unknown");
}

/// The subject is trimmed, and an all-whitespace subject is absent.
#[test]
fn the_subject_is_trimmed_and_blank_is_absent() {
    let padded = r#"{"response":{"id":"  subject-1  ","gender":"F","birthyear":"2000","birthday":"07-12"}}"#;
    assert_eq!(
        decode_profile(padded.as_bytes()).expect("decodes").subject(),
        Some("subject-1")
    );
    let blank = r#"{"response":{"id":"   ","gender":"F","birthyear":"2000","birthday":"07-12"}}"#;
    assert_eq!(decode_profile(blank.as_bytes()).expect("decodes").subject(), None);
    let absent = r#"{"response":{"gender":"F","birthyear":"2000","birthday":"07-12"}}"#;
    assert_eq!(decode_profile(absent.as_bytes()).expect("decodes").subject(), None);
}

/// A body above the bound is refused **before** deserialization.
///
/// The proof that it is refused before parsing is that the body is not even
/// valid JSON: if the bound were checked after parsing, this would report
/// `Malformed` instead of `TooLarge`.
#[test]
fn a_body_above_the_bound_is_refused_before_deserialization() {
    let oversized = vec![b'{'; MAX_PROFILE_BODY_BYTES + 1];
    assert_eq!(
        decode_profile(&oversized).err(),
        Some(NaverProfileDecodeError::TooLarge)
    );
    // Exactly at the bound is still parsed, and a non-JSON body of that size is
    // `Malformed`, not `TooLarge`. That is the boundary.
    let at_bound = vec![b'{'; MAX_PROFILE_BODY_BYTES];
    assert_eq!(
        decode_profile(&at_bound).err(),
        Some(NaverProfileDecodeError::Malformed)
    );
}

/// A body that is not the documented envelope is `Malformed`.
#[test]
fn a_body_without_the_response_envelope_is_malformed() {
    for value in [
        "",
        "not json",
        "{}",
        r#"{"response":null}"#,
        r#"{"response":{"gender":5}}"#,
        r#"{"response":{"id":true}}"#,
    ] {
        assert_eq!(
            decode_profile(value.as_bytes()).err(),
            Some(NaverProfileDecodeError::Malformed),
            "{value:?}"
        );
    }
}

/// The decoded profile renders no value in `Debug`.
///
/// V-12-o: no provider subject, birth date or gender value may appear in a log.
/// A derived `Debug` would render all three, so this asserts the hand-written
/// one redacts them.
#[test]
fn the_decoded_profile_redacts_every_value_in_debug() {
    let profile = decode_profile(body(Some("F")).as_bytes()).expect("decodes");
    let rendered = format!("{profile:?}");
    for secret in ["subject-1", "2000", "07-12"] {
        assert!(
            !rendered.contains(secret),
            "{secret:?} must not appear in Debug output: {rendered}"
        );
    }
    // The gender value is not rendered either, only its presence.
    assert!(!rendered.contains("\"F\"") && !rendered.contains("Female"));
}

/// The bound is the donor's medium response bucket.
#[test]
fn the_size_bound_is_the_donor_medium_bucket() {
    assert_eq!(MAX_PROFILE_BODY_BYTES, 16 * 1024);
}
