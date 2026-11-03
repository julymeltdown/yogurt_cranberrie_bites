use concert_domain::feed::*;
use uuid::Uuid;
#[test]
fn content_bounds_preserve_user_text_and_reject_invalid_media() {
    let mut c = Content {
        text: " \nA real post\n ".into(),
        media_ids: vec![],
        links: vec![],
    };
    assert!(c.validate().is_ok());
    c.text = " ".into();
    assert_eq!(c.validate(), Err(FeedError::InvalidInput));
    c.text = "a".repeat(4001);
    assert!(c.validate().is_err());
    c.text = "🎸".repeat(4000);
    assert!(c.validate().is_ok());
    c.media_ids = vec![Uuid::from_u128(1); 2];
    assert!(c.validate().is_err());
}
#[test]
fn revision_is_safe_and_positive() {
    for n in [0, -1, 9_007_199_254_740_992] {
        assert!(revision(n).is_err())
    }
    assert_eq!(
        revision(9_007_199_254_740_991).unwrap(),
        9_007_199_254_740_991
    );
}
