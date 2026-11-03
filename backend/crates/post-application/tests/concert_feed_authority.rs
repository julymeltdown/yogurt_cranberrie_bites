use chrono::{Duration, Utc};
use post_application::concert_feed::*;
use post_domain::concert_feed::Error;
use uuid::Uuid;
#[test]
fn original_authorization_deadline_and_identity_are_required() {
    let now = Utc::now();
    let mut c = Context {
        actor_id: Uuid::new_v4(),
        event_id: Uuid::new_v4(),
        session_family_id: Uuid::new_v4(),
        request_id: Uuid::new_v4(),
        original_authority: [7; 32],
        expires_at: now + Duration::seconds(30),
    };
    assert!(c.validate(now).is_ok());
    c.expires_at = now;
    assert_eq!(c.validate(now), Err(Error::Authority));
    c.expires_at = now + Duration::seconds(66);
    assert_eq!(c.validate(now), Err(Error::Authority));
    c.expires_at = now + Duration::seconds(30);
    c.original_authority = [0; 32];
    assert_eq!(c.validate(now), Err(Error::Authority));
    c.original_authority = [1; 32];
    c.actor_id = Uuid::nil();
    assert_eq!(c.validate(now), Err(Error::Authority));
}
#[test]
fn page_boundaries_are_bounded() {
    let mut w = Window {
        upper: Utc::now(),
        after: None,
        filter: Filter::All,
        limit: 100,
    };
    assert!(w.validate().is_ok());
    w.limit = 101;
    assert_eq!(w.validate(), Err(Error::Invalid));
    w.limit = 1;
    w.after = Some(Position {
        created_at: w.upper + Duration::seconds(1),
        id: Uuid::new_v4(),
    });
    assert_eq!(w.validate(), Err(Error::Invalid));
}
