use async_trait::async_trait;
use chrono::{DateTime, Utc};
use profile_application::concert_identity::*;
use std::{
    collections::VecDeque,
    future::Future,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};
use uuid::Uuid;

fn user() -> Uuid {
    Uuid::from_u128(0xabcdef)
}
fn observation() -> DateTime<Utc> {
    "2030-01-01T00:00:00Z".parse().unwrap()
}
fn row() -> ConcertProfileRecord {
    ConcertProfileRecord {
        user_id: user(),
        profile_id: Some(Uuid::from_u128(0x123456)),
        nickname: "  Mira  ".into(),
        bio: "Private self bio".into(),
        observed_at: observation(),
    }
}
fn ready<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("repository double must resolve synchronously"),
    }
}
type Reply = Result<Option<ConcertProfileRecord>, ConcertProfileError>;
#[derive(Default)]
struct Double {
    replies: Mutex<VecDeque<Reply>>,
    calls: Mutex<Vec<Uuid>>,
}
impl Double {
    fn with(replies: Vec<Reply>) -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(replies.into()),
            calls: Mutex::new(vec![]),
        })
    }
}
#[async_trait]
impl ConcertProfileRepository for Double {
    async fn find_self_content(&self, query: ConcertProfileQuery) -> Reply {
        self.calls.lock().unwrap().push(query.user_id());
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .expect("one expected repository dispatch per call")
    }
}
fn read(value: ConcertProfileRecord) -> Result<ConcertSelfProfile, ConcertProfileError> {
    ready(
        ReadConcertSelfProfile::new(Double::with(vec![Ok(Some(value))]))
            .execute(ConcertProfileQuery::new(user()).unwrap()),
    )
}
#[test]
fn selectors_reject_nil_and_noncanonical_subjects_before_repository_dispatch() {
    let double = Double::default();
    assert_eq!(
        ConcertProfileQuery::new(Uuid::nil()),
        Err(ConcertProfileError::InvalidSubject)
    );
    for input in [
        "not-a-uuid".to_owned(),
        Uuid::nil().to_string(),
        user().to_string().to_uppercase(),
        user().simple().to_string(),
        format!(" {}", user()),
    ] {
        assert_eq!(
            ConcertProfileQuery::from_canonical(&input),
            Err(ConcertProfileError::InvalidSubject)
        );
    }
    assert_eq!(
        ConcertProfileQuery::from_canonical(&user().to_string())
            .unwrap()
            .user_id(),
        user()
    );
    assert!(double.calls.lock().unwrap().is_empty());
}
#[test]
fn exact_content_and_observation_are_preserved_without_inventing_identity_authority() {
    let profile = read(row()).unwrap();
    assert_eq!(profile.user_id(), user());
    assert_eq!(profile.display_name(), "  Mira  ");
    assert_eq!(profile.bio(), Some("Private self bio"));
    assert_eq!(profile.observed_at(), observation());
}
#[test]
fn only_empty_bio_is_normalized_not_spaces_or_stored_content() {
    let mut record = row();
    record.bio = String::new();
    assert_eq!(read(record).unwrap().bio(), None);
    let mut record = row();
    record.bio = "  ".into();
    assert_eq!(read(record).unwrap().bio(), Some("  "));
}
#[test]
fn unicode_character_limits_match_consumer_not_utf8_bytes_or_graphemes() {
    let mut record = row();
    record.nickname = "🎸".repeat(40);
    record.bio = "가".repeat(160);
    let profile = read(record).unwrap();
    assert_eq!(profile.display_name().chars().count(), 40);
    assert_eq!(profile.bio().unwrap().chars().count(), 160);
    for (name, bio) in [
        ("🎸".repeat(41), "valid".into()),
        ("Valid".into(), "가".repeat(161)),
        ("e\u{301}".repeat(21), String::new()),
    ] {
        let mut record = row();
        record.nickname = name;
        record.bio = bio;
        assert_eq!(read(record), Err(ConcertProfileError::ContentUnavailable));
    }
}
#[test]
fn blank_control_and_overlong_stored_values_are_refused_without_sanitizing() {
    for nickname in [
        "".into(),
        " \t ".into(),
        "bad\nname".into(),
        "bad\u{7f}name".into(),
        "bad\u{85}name".into(),
        "x".repeat(41),
    ] {
        let mut record = row();
        record.nickname = nickname;
        assert_eq!(read(record), Err(ConcertProfileError::ContentUnavailable));
    }
    for bio in [
        "raw\nproof".into(),
        "private\0token".into(),
        "x".repeat(161),
    ] {
        let mut record = row();
        record.bio = bio;
        assert_eq!(read(record), Err(ConcertProfileError::ContentUnavailable));
    }
}
#[test]
fn foreign_nil_subject_and_unrepresentable_observation_fail_closed() {
    for subject in [Uuid::nil(), Uuid::from_u128(2)] {
        let mut record = row();
        record.user_id = subject;
        assert_eq!(read(record), Err(ConcertProfileError::ContentUnavailable));
    }
    for observed in [DateTime::<Utc>::MIN_UTC, DateTime::<Utc>::MAX_UTC] {
        let mut record = row();
        record.observed_at = observed;
        assert_eq!(read(record), Err(ConcertProfileError::ContentUnavailable));
    }
}
#[test]
fn absent_and_deleted_content_share_one_safe_unavailable_result() {
    // The owner port deliberately has no deleted/hidden/missing reason or content variant.
    let double = Double::with(vec![Ok(None), Ok(None)]);
    let service = ReadConcertSelfProfile::new(double.clone());
    for _ in 0..2 {
        assert_eq!(
            ready(service.execute(ConcertProfileQuery::new(user()).unwrap())),
            Err(ConcertProfileError::ContentUnavailable)
        );
    }
    assert_eq!(double.calls.lock().unwrap().as_slice(), &[user(), user()]);
}
#[test]
fn persistence_refusal_never_returns_partial_or_cached_content() {
    let double = Double::with(vec![
        Ok(Some(row())),
        Err(ConcertProfileError::DependencyUnavailable),
        Ok(None),
    ]);
    let service = ReadConcertSelfProfile::new(double.clone());
    let query = ConcertProfileQuery::new(user()).unwrap();
    assert!(ready(service.execute(query)).is_ok());
    assert_eq!(
        ready(service.execute(query)),
        Err(ConcertProfileError::DependencyUnavailable)
    );
    assert_eq!(
        ready(service.execute(query)),
        Err(ConcertProfileError::ContentUnavailable)
    );
    assert_eq!(double.calls.lock().unwrap().len(), 3);
}
#[test]
fn repeated_reads_dispatch_again_and_observe_updated_content() {
    let first = row();
    let mut second = row();
    second.nickname = "Changed".into();
    second.bio = "new".into();
    let double = Double::with(vec![Ok(Some(first)), Ok(Some(second))]);
    let service = ReadConcertSelfProfile::new(double.clone());
    let query = ConcertProfileQuery::new(user()).unwrap();
    assert_eq!(
        ready(service.execute(query)).unwrap().display_name(),
        "  Mira  "
    );
    assert_eq!(
        ready(service.execute(query)).unwrap().display_name(),
        "Changed"
    );
    assert_eq!(double.calls.lock().unwrap().len(), 2);
}
#[test]
fn debug_and_errors_contain_no_subject_content_or_private_canaries() {
    let mut record = row();
    record.nickname = "private-name".into();
    record.bio = "raw-email@example.test".into();
    assert_eq!(format!("{record:?}"), "ConcertProfileRecord([REDACTED])");
    let profile = read(record).unwrap();
    assert_eq!(format!("{profile:?}"), "ConcertSelfProfile([REDACTED])");
    assert_eq!(
        format!("{:?}", ConcertProfileQuery::new(user()).unwrap()),
        "ConcertProfileQuery([REDACTED])"
    );
    assert_eq!(
        ConcertProfileError::ContentUnavailable.to_string(),
        "Profile content is unavailable"
    );
    assert_eq!(
        ConcertProfileError::DependencyUnavailable.to_string(),
        "Profile owner is unavailable"
    );
    for error in [
        ConcertProfileError::InvalidSubject,
        ConcertProfileError::ContentUnavailable,
        ConcertProfileError::DependencyUnavailable,
    ] {
        for secret in [
            "private-name",
            "raw-email@example.test",
            &user().to_string(),
        ] {
            assert!(!format!("{error:?} {error}").contains(secret));
        }
    }
}
