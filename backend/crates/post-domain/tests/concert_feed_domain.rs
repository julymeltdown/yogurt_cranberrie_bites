use chrono::Utc;
use post_domain::concert_feed::*;
use uuid::Uuid;
fn content() -> Content {
    Content {
        text: "hello".into(),
        media_ids: vec![],
        links: vec![],
    }
}
fn post() -> Post {
    Post {
        post_id: Uuid::new_v4(),
        event_id: Uuid::new_v4(),
        author_id: Uuid::new_v4(),
        content: content(),
        revision: 1,
        state: State::Published,
        created_at: Utc::now(),
        edited_at: None,
        comment_count: 0,
        reaction_count: 0,
    }
}
#[test]
fn owner_scope_revision_and_state_are_independent_guards() {
    let mut p = post();
    assert_eq!(
        p.owner(p.author_id, Uuid::new_v4(), 1),
        Err(Error::NotFound)
    );
    assert_eq!(
        p.owner(Uuid::new_v4(), p.event_id, 1),
        Err(Error::Forbidden)
    );
    assert_eq!(p.owner(p.author_id, p.event_id, 2), Err(Error::Conflict));
    assert!(p.owner(p.author_id, p.event_id, 1).is_ok());
    p.state = State::Hidden;
    assert_eq!(p.owner(p.author_id, p.event_id, 1), Err(Error::NotFound));
    p.state = State::Deleted;
    assert_eq!(p.visible_in(p.event_id), Err(Error::NotFound));
}
#[test]
fn c04_text_media_links_and_js_safe_revisions() {
    let mut c = content();
    c.text = "한".repeat(4000);
    assert!(c.validate().is_ok());
    c.text.push('a');
    assert_eq!(c.validate(), Err(Error::Invalid));
    c.text = "valid".into();
    let id = Uuid::new_v4();
    c.media_ids = vec![id, id];
    assert_eq!(c.validate(), Err(Error::Invalid));
    c.media_ids = vec![];
    c.links = vec![Link {
        target_type: LinkKind::Party,
        target_id: Uuid::nil(),
    }];
    assert_eq!(c.validate(), Err(Error::Invalid));
    assert_eq!(next_revision(MAX_REVISION), Err(Error::Conflict));
    assert_eq!(valid_revision(0), Err(Error::Invalid));
    assert_eq!(valid_text("  \n", 1000), Err(Error::Invalid));
}
#[test]
fn comment_owner_cannot_move_between_parents_or_events() {
    let p = post();
    let c = Comment {
        comment_id: Uuid::new_v4(),
        event_id: p.event_id,
        post_id: p.post_id,
        author_id: p.author_id,
        text: "reply".into(),
        revision: 1,
        state: State::Published,
        created_at: Utc::now(),
        edited_at: None,
    };
    assert_eq!(
        c.owner(c.author_id, c.event_id, Uuid::new_v4(), 1),
        Err(Error::NotFound)
    );
    assert_eq!(
        c.owner(c.author_id, Uuid::new_v4(), c.post_id, 1),
        Err(Error::NotFound)
    );
    assert_eq!(
        c.owner(Uuid::new_v4(), c.event_id, c.post_id, 1),
        Err(Error::Forbidden)
    );
}
