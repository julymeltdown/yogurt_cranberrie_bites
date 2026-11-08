//! WP-05 T-401, T-402, T-403, T-404, T-407: the pure intro-post rules.
//!
//! These are unit tests over values: no database, no clock, no I/O. The SQL
//! layer has its own tests against a real cluster; these prove the rules
//! themselves, so a failure here names the rule rather than a query.
use concert_application::intro_posts::{
    attachments::{self, AttachmentCandidate, AttachmentError, MediaPurpose, MAX_ATTACHMENTS},
    audience::{Audience, AudienceError, GenderCondition, InclusiveAgeRange},
    comments::{self, CommentError, ParentComment, MAX_COMMENT_LENGTH},
    content::{ContentError, IntroContent, MAX_BODY, MAX_SINGLE_LINE},
    snapshot::{Snapshot, SnapshotError, SnapshotItem},
};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// T-407: the inclusive -> half-open conversion
// ---------------------------------------------------------------------------

/// The acceptance case: selecting 18–29 sends `minimum_age: 18,
/// maximum_age_exclusive: 30`.
#[test]
fn an_inclusive_range_converts_to_the_half_open_wire_form() {
    let audience = Audience::from_inclusive(
        GenderCondition::Any,
        Some(InclusiveAgeRange {
            minimum: 18,
            maximum_inclusive: 29,
        }),
    )
    .expect("valid");
    assert_eq!(audience.minimum_age, Some(18));
    assert_eq!(audience.maximum_age_exclusive, Some(30));
    let wire = audience.to_wire();
    assert_eq!(wire["minimum_age"], 18);
    assert_eq!(wire["maximum_age_exclusive"], 30);
    // And back again, so the composer can show what was saved.
    assert_eq!(
        audience.inclusive_range(),
        Some(InclusiveAgeRange {
            minimum: 18,
            maximum_inclusive: 29
        })
    );
}

#[test]
fn the_boundary_ages_convert_correctly() {
    // The narrowest legal window: 14–14 inclusive is 14..15 half-open.
    let audience = Audience::from_inclusive(
        GenderCondition::Any,
        Some(InclusiveAgeRange {
            minimum: 14,
            maximum_inclusive: 14,
        }),
    )
    .expect("valid");
    assert_eq!(audience.minimum_age, Some(14));
    assert_eq!(audience.maximum_age_exclusive, Some(15));
    // The widest: 100–100 inclusive is 100..101 half-open.
    let audience = Audience::from_inclusive(
        GenderCondition::Any,
        Some(InclusiveAgeRange {
            minimum: 100,
            maximum_inclusive: 100,
        }),
    )
    .expect("valid");
    assert_eq!(audience.minimum_age, Some(100));
    assert_eq!(audience.maximum_age_exclusive, Some(101));
}

#[test]
fn an_inverted_range_is_refused() {
    // 30–18 inclusive: minimum 30, maximum_exclusive 19, which is empty.
    assert_eq!(
        Audience::from_inclusive(
            GenderCondition::Any,
            Some(InclusiveAgeRange {
                minimum: 30,
                maximum_inclusive: 18
            })
        ),
        Err(AudienceError::EmptyRange)
    );
    // A single-year inversion is the same rule.
    assert_eq!(
        Audience::new(GenderCondition::Any, Some(20), Some(20)),
        Err(AudienceError::EmptyRange)
    );
}

#[test]
fn the_age_window_is_bounded() {
    for (min, max) in [(13, 30), (18, 102), (0, 10), (101, 102)] {
        assert_eq!(
            Audience::new(GenderCondition::Any, Some(min), Some(max)),
            Err(AudienceError::OutOfRange),
            "{min}..{max}"
        );
    }
}

#[test]
fn the_bounds_are_paired() {
    assert_eq!(
        Audience::new(GenderCondition::Any, Some(18), None),
        Err(AudienceError::UnpairedBounds)
    );
    assert_eq!(
        Audience::new(GenderCondition::Any, None, Some(30)),
        Err(AudienceError::UnpairedBounds)
    );
}

// ---------------------------------------------------------------------------
// T-407: gender and the summary
// ---------------------------------------------------------------------------

#[test]
fn the_gender_condition_is_closed() {
    for (value, expected) in [
        ("any", GenderCondition::Any),
        ("female", GenderCondition::Female),
        ("male", GenderCondition::Male),
        ("unknown", GenderCondition::Unknown),
    ] {
        assert_eq!(GenderCondition::parse(value), Some(expected));
    }
    for value in ["", "Any", "FEMALE", "women", "verified", "none"] {
        assert_eq!(GenderCondition::parse(value), None, "{value}");
    }
    // `unknown` is not a wildcard: it restricts, exactly like female and male.
    assert!(GenderCondition::Unknown.restricted());
    assert!(!GenderCondition::Any.restricted());
}

/// The summary is server-derived and is the only place it is produced.
#[test]
fn the_summary_is_derived_from_the_value() {
    let cases = [
        (Audience::ANYONE, "Anyone"),
        (
            Audience::new(GenderCondition::Female, Some(18), Some(30)).unwrap(),
            "Women \u{00b7} Ages 18\u{2013}29",
        ),
        (
            Audience::new(GenderCondition::Male, None, None).unwrap(),
            "Men",
        ),
        (
            Audience::new(GenderCondition::Unknown, Some(18), Some(101)).unwrap(),
            "Gender not specified \u{00b7} Ages 18+",
        ),
        (
            Audience::new(GenderCondition::Any, Some(25), Some(35)).unwrap(),
            "Ages 25\u{2013}34",
        ),
    ];
    for (audience, expected) in cases {
        assert_eq!(audience.summary(), expected);
    }
}

#[test]
fn the_wire_form_round_trips() {
    let audience = Audience::new(GenderCondition::Female, Some(18), Some(30)).expect("valid");
    let wire = audience.to_wire();
    assert_eq!(Audience::from_wire(&wire), Ok(audience));
    // A closed object: an unknown field is refused, never ignored.
    let mut extra = wire.clone();
    extra["extra"] = serde_json::json!(1);
    assert_eq!(Audience::from_wire(&extra), Err(AudienceError::InvalidGender));
    // A non-numeric bound is refused.
    let mut bad = wire.clone();
    bad["minimum_age"] = serde_json::json!("18");
    assert_eq!(Audience::from_wire(&bad), Err(AudienceError::OutOfRange));
}

#[test]
fn an_unrestricted_audience_requires_no_claim() {
    assert!(!Audience::ANYONE.is_restricted());
    assert!(
        Audience::new(GenderCondition::Any, Some(18), Some(30))
            .unwrap()
            .is_restricted()
    );
    assert!(
        Audience::new(GenderCondition::Unknown, None, None)
            .unwrap()
            .is_restricted()
    );
}

// ---------------------------------------------------------------------------
// T-401: content rules
// ---------------------------------------------------------------------------

#[test]
fn a_draft_may_have_an_empty_body_but_publishing_may_not() {
    let empty = IntroContent::default();
    assert_eq!(empty.validate_draft(), Ok(()));
    assert_eq!(empty.validate_publish(), Err(ContentError::EmptyBody));
    let whitespace = IntroContent {
        body: "   \n\t ".to_owned(),
        ..Default::default()
    };
    assert_eq!(whitespace.validate_draft(), Ok(()));
    assert_eq!(whitespace.validate_publish(), Err(ContentError::EmptyBody));
}

#[test]
fn the_body_is_bounded() {
    let at_limit = IntroContent {
        body: "a".repeat(MAX_BODY),
        ..Default::default()
    };
    assert_eq!(at_limit.validate_publish(), Ok(()));
    let over = IntroContent {
        body: "a".repeat(MAX_BODY + 1),
        ..Default::default()
    };
    assert_eq!(over.validate_draft(), Err(ContentError::BodyTooLong));
    // The bound is in Unicode code points, not bytes.
    let emoji = IntroContent {
        body: "\u{1F600}".repeat(MAX_BODY),
        ..Default::default()
    };
    assert_eq!(emoji.validate_publish(), Ok(()));
}

#[test]
fn a_body_with_a_nul_byte_is_refused() {
    let content = IntroContent {
        body: "hello\0world".to_owned(),
        ..Default::default()
    };
    assert_eq!(content.validate_draft(), Err(ContentError::BodyContainsNul));
}

/// A published body that tries to smuggle contact details past the audience
/// restriction fails to publish.
#[test]
fn a_contact_bypass_body_fails_to_publish() {
    for body in [
        "Call me on 010-1234-5678",
        "my kakao is mcrfan",
        "email me at someone@example.com",
        "https://open.kakao.com/o/abcdef",
    ] {
        let content = IntroContent {
            body: body.to_owned(),
            ..Default::default()
        };
        assert_eq!(
            content.validate_publish(),
            Err(ContentError::ContactBypass),
            "{body}"
        );
        // A draft still saves, so the author does not lose their work.
        assert_eq!(content.validate_draft(), Ok(()), "{body}");
    }
    // Ordinary prose still publishes.
    let fine = IntroContent {
        body: "I like sad songs and long walks by the river.".to_owned(),
        ..Default::default()
    };
    assert_eq!(fine.validate_publish(), Ok(()));
}

#[test]
fn the_single_line_fields_reject_newlines_and_overlong_values() {
    let at_limit = IntroContent {
        body: "body".to_owned(),
        currently_seeking: Some("a".repeat(MAX_SINGLE_LINE)),
        interaction_style: Some(String::new()),
    };
    assert_eq!(at_limit.validate_draft(), Ok(()));
    let too_long = IntroContent {
        body: "body".to_owned(),
        currently_seeking: Some("a".repeat(MAX_SINGLE_LINE + 1)),
        interaction_style: None,
    };
    assert_eq!(
        too_long.validate_draft(),
        Err(ContentError::SingleLineTooLong)
    );
    for value in ["two\nlines", "carriage\rreturn", "tab\there", "nul\0byte"] {
        let content = IntroContent {
            body: "body".to_owned(),
            currently_seeking: Some(value.to_owned()),
            interaction_style: None,
        };
        assert_eq!(
            content.validate_draft(),
            Err(ContentError::SingleLineNotSingleLine),
            "{value:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// T-403: attachments
// ---------------------------------------------------------------------------

fn candidate(owner: Uuid, purpose: MediaPurpose, ready: bool) -> AttachmentCandidate {
    AttachmentCandidate {
        media_id: Uuid::new_v4(),
        owner_profile_id: owner,
        purpose,
        ready,
    }
}

/// The acceptance case: a fifth attachment is rejected.
#[test]
fn a_fifth_attachment_is_rejected() {
    let author = Uuid::new_v4();
    let found: Vec<AttachmentCandidate> = (0..5)
        .map(|_| candidate(author, MediaPurpose::IntroPost, true))
        .collect();
    let requested: Vec<Uuid> = found.iter().map(|c| c.media_id).collect();
    assert_eq!(
        attachments::plan(&requested, author, &found),
        Err(AttachmentError::TooMany)
    );
    // Four is fine.
    assert!(attachments::plan(&requested[..MAX_ATTACHMENTS], author, &found).is_ok());
}

/// The acceptance case: a foreign media id is rejected without revealing
/// whether it exists.
#[test]
fn a_foreign_media_id_is_indistinguishable_from_a_missing_one() {
    let author = Uuid::new_v4();
    let stranger = Uuid::new_v4();
    let foreign = candidate(stranger, MediaPurpose::IntroPost, true);
    let absent = Uuid::new_v4();
    let found = [foreign];
    // A real asset owned by somebody else.
    assert_eq!(
        attachments::plan(&[foreign.media_id], author, &found),
        Err(AttachmentError::Unavailable)
    );
    // An id that does not exist at all.
    assert_eq!(
        attachments::plan(&[absent], author, &found),
        Err(AttachmentError::Unavailable)
    );
    // The two answers are the same value, so no caller can tell them apart.
    assert_eq!(
        attachments::plan(&[foreign.media_id], author, &found),
        attachments::plan(&[absent], author, &found)
    );
}

#[test]
fn a_wrong_purpose_or_unready_asset_is_refused() {
    let author = Uuid::new_v4();
    for (purpose, ready) in [
        (MediaPurpose::ProfilePost, true),
        (MediaPurpose::RoomPoster, true),
        (MediaPurpose::ProfileImage, true),
        (MediaPurpose::IntroPost, false),
    ] {
        let item = candidate(author, purpose, ready);
        assert_eq!(
            attachments::plan(&[item.media_id], author, &[item]),
            Err(AttachmentError::Unavailable),
            "{purpose:?} ready={ready}"
        );
    }
    let good = candidate(author, MediaPurpose::IntroPost, true);
    assert_eq!(
        attachments::plan(&[good.media_id], author, &[good]),
        Ok(attachments::AttachmentPlan {
            media_ids: vec![good.media_id]
        })
    );
}

#[test]
fn duplicates_and_nil_ids_are_refused() {
    let author = Uuid::new_v4();
    let item = candidate(author, MediaPurpose::IntroPost, true);
    assert_eq!(
        attachments::plan(&[item.media_id, item.media_id], author, &[item]),
        Err(AttachmentError::Duplicate)
    );
    assert_eq!(
        attachments::plan(&[Uuid::nil()], author, &[]),
        Err(AttachmentError::NilId)
    );
    // An empty set is valid: a post may have no photographs.
    assert_eq!(
        attachments::plan(&[], author, &[]),
        Ok(attachments::AttachmentPlan {
            media_ids: Vec::new()
        })
    );
}

// ---------------------------------------------------------------------------
// T-404: comments
// ---------------------------------------------------------------------------

#[test]
fn a_comment_body_is_bounded_and_non_empty() {
    let intro = Uuid::new_v4();
    assert_eq!(
        comments::validate(intro, "   ", None, None),
        Err(CommentError::Empty)
    );
    assert_eq!(
        comments::validate(intro, &"a".repeat(MAX_COMMENT_LENGTH + 1), None, None),
        Err(CommentError::TooLong)
    );
    assert_eq!(
        comments::validate(intro, &"a".repeat(MAX_COMMENT_LENGTH), None, None),
        Ok(comments::CommentInput {
            body: "a".repeat(MAX_COMMENT_LENGTH),
            parent_comment_id: None
        })
    );
    assert_eq!(
        comments::validate(intro, "nul\0", None, None),
        Err(CommentError::ContainsNul)
    );
}

/// The acceptance case: a cross-post parent is rejected.
#[test]
fn a_cross_post_parent_is_rejected() {
    let intro = Uuid::new_v4();
    let other = Uuid::new_v4();
    let parent = ParentComment {
        comment_id: Uuid::new_v4(),
        intro_id: other,
        parent_comment_id: None,
        deleted: false,
    };
    assert_eq!(
        comments::validate(intro, "reply", Some(parent.comment_id), Some(parent)),
        Err(CommentError::ParentUnavailable)
    );
    // And a parent that did not resolve at all gets the same answer.
    assert_eq!(
        comments::validate(intro, "reply", Some(parent.comment_id), None),
        Err(CommentError::ParentUnavailable)
    );
}

#[test]
fn a_reply_to_a_reply_is_refused() {
    let intro = Uuid::new_v4();
    let parent = ParentComment {
        comment_id: Uuid::new_v4(),
        intro_id: intro,
        parent_comment_id: Some(Uuid::new_v4()),
        deleted: false,
    };
    assert_eq!(
        comments::validate(intro, "reply", Some(parent.comment_id), Some(parent)),
        Err(CommentError::ReplyToReply)
    );
}

#[test]
fn a_deleted_parent_is_refused() {
    let intro = Uuid::new_v4();
    let parent = ParentComment {
        comment_id: Uuid::new_v4(),
        intro_id: intro,
        parent_comment_id: None,
        deleted: true,
    };
    assert_eq!(
        comments::validate(intro, "reply", Some(parent.comment_id), Some(parent)),
        Err(CommentError::ParentUnavailable)
    );
}

#[test]
fn a_valid_reply_is_accepted() {
    let intro = Uuid::new_v4();
    let parent = ParentComment {
        comment_id: Uuid::new_v4(),
        intro_id: intro,
        parent_comment_id: None,
        deleted: false,
    };
    assert_eq!(
        comments::validate(intro, "reply", Some(parent.comment_id), Some(parent)),
        Ok(comments::CommentInput {
            body: "reply".to_owned(),
            parent_comment_id: Some(parent.comment_id)
        })
    );
}

// ---------------------------------------------------------------------------
// T-402: the snapshot
// ---------------------------------------------------------------------------

fn snapshot_item(label: &str) -> SnapshotItem {
    SnapshotItem {
        catalog_item_id: Uuid::new_v4(),
        label: label.to_owned(),
        category: "MUSIC",
    }
}

#[test]
fn a_snapshot_is_bounded_and_unique() {
    let items: Vec<SnapshotItem> = (0..16).map(|index| snapshot_item(&format!("{index}"))).collect();
    assert_eq!(
        Snapshot::from_favorites(items),
        Err(SnapshotError::TooMany)
    );
    let item = snapshot_item("MCR");
    assert_eq!(
        Snapshot::from_favorites(vec![item.clone(), item]),
        Err(SnapshotError::Duplicate)
    );
    assert_eq!(
        Snapshot::from_favorites(vec![snapshot_item("")]),
        Err(SnapshotError::InvalidLabel)
    );
    // An empty snapshot is valid: a profile with no picks still publishes.
    assert!(Snapshot::empty().is_empty());
}

#[test]
fn a_snapshot_round_trips_through_storage() {
    let items = vec![snapshot_item("My Chemical Romance"), snapshot_item("NANA")];
    let snapshot = Snapshot::from_favorites(items.clone()).expect("valid");
    let stored = snapshot.to_stored();
    assert_eq!(Snapshot::from_stored(&stored), Ok(snapshot));
    // The stored form carries ids and labels only; no weight, no intensity.
    for entry in stored.as_array().expect("array") {
        let mut keys: Vec<&str> = entry
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort();
        assert_eq!(keys, ["catalog_item_id", "category", "external_url", "label"]);
    }
}

/// The acceptance case: changing favorites after publishing leaves the post
/// snapshot unchanged.
///
/// The snapshot is a value, so this test is about the *stored* form: a stored
/// snapshot read back is byte-identical regardless of what the profile holds
/// now. The command-level half of this case is in
/// `tests/intro_snapshot.rs`, where the profile really is edited.
#[test]
fn a_stored_snapshot_does_not_depend_on_the_profile() {
    let original = Snapshot::from_favorites(vec![snapshot_item("MCR")]).expect("valid");
    let stored = original.to_stored();
    // The author's profile now holds something completely different. Nothing
    // re-derives the snapshot, so reading the stored value is unchanged.
    let _profile_now = vec![snapshot_item("Paramore"), snapshot_item("NANA")];
    assert_eq!(Snapshot::from_stored(&stored), Ok(original.clone()));
    assert_eq!(Snapshot::from_stored(&stored).unwrap().to_stored(), stored);
    // The rule is stated where a reader will find it.
    assert!(Snapshot::unchanged_by_profile_edit().contains("never re-derived"));
}

#[test]
fn a_corrupted_stored_snapshot_is_refused() {
    for value in [
        serde_json::json!("not an array"),
        serde_json::json!([{"catalog_item_id": "not-a-uuid", "label": "x", "category": "MUSIC"}]),
        serde_json::json!([{"catalog_item_id": Uuid::new_v4(), "label": "x", "category": "MOVIE"}]),
        serde_json::json!([{"catalog_item_id": Uuid::new_v4(), "label": "x", "category": "MUSIC", "weight": 3}]),
    ] {
        assert!(
            Snapshot::from_stored(&value).is_err(),
            "{value} must be refused"
        );
    }
}
