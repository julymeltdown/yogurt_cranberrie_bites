use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use concert_application::{feed::*, Clock};
use concert_domain::feed::*;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}
fn now() -> DateTime<Utc> {
    Utc.timestamp_opt(1793710000, 0).unwrap()
}
#[derive(Clone, Copy)]
struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> DateTime<Utc> {
        now()
    }
}
fn ctx() -> Context {
    Context {
        actor: id(1),
        event: id(2),
        session_family: id(3),
        request: id(4),
        authority_digest: [9; 32],
        expires_at: now() + chrono::Duration::seconds(2),
    }
}
fn row(n: u128) -> Post {
    Post {
        id: id(n),
        event: id(2),
        author: id(10),
        content: Content {
            text: "Concert post".into(),
            media_ids: vec![],
            links: vec![],
        },
        state: ContentState::Published,
        public: true,
        revision: 1,
        created_at: now() - chrono::Duration::seconds(1),
        edited_at: None,
        comment_count: 0,
        reaction_count: 0,
        reacted_by_me: false,
    }
}
#[derive(Default)]
struct State {
    rows: Vec<Post>,
    deny: bool,
    stale: bool,
    links: bool,
    media: bool,
    hide_on_author: bool,
    wrong_binding: bool,
    comments: Vec<Comment>,
    scan_override: Option<Position>,
    more_override: Option<bool>,
}
#[derive(Clone)]
struct Ports(Arc<Mutex<State>>);
#[async_trait]
impl FeedPolicy for Ports {
    async fn observe(&self, c: Context, s: Uuid, a: Action) -> Result<Observation, FeedError> {
        let state = self.0.lock().unwrap();
        if state.deny {
            return Err(FeedError::PermissionDenied);
        }
        Ok(Observation {
            context: if state.wrong_binding {
                Context {
                    event: id(999),
                    ..c
                }
            } else {
                c
            },
            subject: s,
            action: a,
            event_revision: 1,
            relationship_revision: 1,
            auth_actor_revision: 1,
            auth_subject_revision: 1,
            friend_revision: 1,
            valid_until: c.expires_at,
        })
    }
    async fn fence(&self, _: Observation) -> Result<(), FeedError> {
        if self.0.lock().unwrap().stale {
            Err(FeedError::PermissionDenied)
        } else {
            Ok(())
        }
    }
}
#[async_trait]
impl PostReader for Ports {
    async fn page(
        &self,
        _: Context,
        _: Window,
        _: Filter,
        limit: u16,
    ) -> Result<ScannedPosts, FeedError> {
        let state = self.0.lock().unwrap();
        let items = state.rows.clone();
        let scanned_through = state
            .scan_override
            .or_else(|| items.last().map(Post::position));
        let has_more = state
            .more_override
            .unwrap_or(items.len() == usize::from(limit));
        Ok(ScannedPosts {
            items,
            scanned_through,
            has_more,
        })
    }
    async fn post(&self, _: Context, n: Uuid) -> Result<Option<Post>, FeedError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .rows
            .iter()
            .find(|p| p.id == n)
            .cloned())
    }
}
#[async_trait]
impl FeedResources for Ports {
    async fn author(&self, _: Context, n: Uuid) -> Result<PublicAuthor, FeedError> {
        let mut s = self.0.lock().unwrap();
        if s.hide_on_author {
            s.rows
                .iter_mut()
                .for_each(|r| r.state = ContentState::Hidden)
        }
        Ok(PublicAuthor {
            user_id: n,
            json: b"{}".to_vec(),
        })
    }
    async fn public_link(&self, _: Context, _: ResourceLink) -> Result<bool, FeedError> {
        Ok(self.0.lock().unwrap().links)
    }
    async fn public_media(&self, _: Context, _: Uuid, _: Uuid) -> Result<bool, FeedError> {
        Ok(self.0.lock().unwrap().media)
    }
}
fn setup(rows: Vec<Post>) -> (Feed<Ports, Ports, Ports, Fixed>, Ports) {
    let p = Ports(Arc::new(Mutex::new(State {
        rows,
        ..Default::default()
    })));
    (
        Feed {
            reader: p.clone(),
            policy: p.clone(),
            resources: p.clone(),
            clock: Fixed,
        },
        p,
    )
}
fn window() -> Window {
    Window {
        upper: now(),
        after: None,
    }
}
#[tokio::test]
async fn stable_tie_order_and_strict_cursor_tuple() {
    let (f, _) = setup(vec![row(12), row(11)]);
    let page = f.page(ctx(), window(), Filter::All, 2).await.unwrap();
    assert_eq!(
        page.items.iter().map(|p| p.post.id).collect::<Vec<_>>(),
        vec![id(12), id(11)]
    );
    assert_eq!(page.next, Some(row(11).position()));
    let (f, _) = setup(vec![row(10)]);
    let page = f
        .page(
            ctx(),
            Window {
                after: Some(row(11).position()),
                ..window()
            },
            Filter::All,
            2,
        )
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(page.next.is_none());
}
#[tokio::test]
async fn cross_event_or_invalid_order_is_dependency_failure() {
    for rows in [
        vec![Post {
            event: id(999),
            ..row(12)
        }],
        vec![row(11), row(12)],
        vec![row(12), row(12)],
    ] {
        let (f, _) = setup(rows);
        assert!(matches!(
            f.page(ctx(), window(), Filter::All, 2).await,
            Err(FeedError::DependencyUnavailable)
        ));
    }
}
#[tokio::test]
async fn private_hidden_deleted_rows_and_revoked_links_do_not_escape() {
    for mut p in [
        Post {
            public: false,
            ..row(12)
        },
        Post {
            state: ContentState::Hidden,
            ..row(12)
        },
        Post {
            state: ContentState::Deleted,
            ..row(12)
        },
    ] {
        p.content.links.push(ResourceLink {
            target_type: TargetType::Party,
            target_id: id(20),
        });
        let (f, _) = setup(vec![p]);
        let page = f.page(ctx(), window(), Filter::All, 1).await.unwrap();
        assert!(page.items.is_empty());
        assert!(page.next.is_some());
    }
    let mut p = row(12);
    p.content.links.push(ResourceLink {
        target_type: TargetType::Party,
        target_id: id(20),
    });
    p.content.media_ids.push(id(30));
    let (f, _) = setup(vec![p]);
    let page = f.page(ctx(), window(), Filter::All, 1).await.unwrap();
    assert!(page.items[0].post.content.links.is_empty());
    assert!(page.items[0].post.content.media_ids.is_empty());
}
#[tokio::test]
async fn filter_cannot_reveal_revoked_private_link() {
    let mut p = row(12);
    p.content.links.push(ResourceLink {
        target_type: TargetType::Party,
        target_id: id(20),
    });
    let (f, _) = setup(vec![p]);
    let page = f.page(ctx(), window(), Filter::PartyLink, 1).await.unwrap();
    assert!(page.items.is_empty());
    assert!(page.next.is_some());
}
#[tokio::test]
async fn block_stale_grant_wrong_binding_and_expiry_fail_closed() {
    for flag in 0..3 {
        let (f, p) = setup(vec![row(12)]);
        {
            let mut s = p.0.lock().unwrap();
            match flag {
                0 => s.deny = true,
                1 => s.stale = true,
                _ => s.wrong_binding = true,
            }
        }
        assert!(matches!(
            f.page(ctx(), window(), Filter::All, 1).await,
            Err(FeedError::PermissionDenied)
        ));
    }
    let (f, _) = setup(vec![row(12)]);
    assert!(f
        .post(
            Context {
                authority_digest: [9; 32],
                expires_at: now(),
                ..ctx()
            },
            id(12)
        )
        .await
        .is_err());
}
#[tokio::test]
async fn hide_during_profile_fetch_fails_closed() {
    let (f, p) = setup(vec![row(12)]);
    p.0.lock().unwrap().hide_on_author = true;
    assert!(matches!(
        f.post(ctx(), id(12)).await,
        Err(FeedError::ResourceUnavailable)
    ));
}
#[tokio::test]
async fn parent_event_is_checked_on_direct_read() {
    let (f, _) = setup(vec![Post {
        event: id(999),
        ..row(12)
    }]);
    assert!(matches!(
        f.post(ctx(), id(12)).await,
        Err(FeedError::ResourceUnavailable)
    ));
}
#[tokio::test]
async fn unavailable_defaults_are_not_fixture_posts() {
    let f = Feed {
        reader: UnavailableFeed,
        policy: UnavailableFeed,
        resources: UnavailableFeed,
        clock: Fixed,
    };
    assert!(matches!(
        f.page(ctx(), window(), Filter::All, 20).await,
        Err(FeedError::DependencyUnavailable)
    ));
}

#[async_trait]
impl CommentReader for Ports {
    async fn comment(&self, _: Context, n: Uuid) -> Result<Option<Comment>, FeedError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .comments
            .iter()
            .find(|c| c.id == n)
            .cloned())
    }
    async fn comments(
        &self,
        _: Context,
        _: Uuid,
        _: Window,
        limit: u16,
    ) -> Result<ScannedComments, FeedError> {
        let items = self.0.lock().unwrap().comments.clone();
        let scanned_through = items.last().map(Comment::position);
        let has_more = items.len() == usize::from(limit);
        Ok(ScannedComments {
            items,
            scanned_through,
            has_more,
        })
    }
}
fn comment(n: u128) -> Comment {
    Comment {
        id: id(n),
        post: id(12),
        event: id(2),
        author: id(1),
        text: "Reply".into(),
        state: ContentState::Published,
        revision: 1,
        created_at: now() - chrono::Duration::seconds(1),
    }
}
#[tokio::test]
async fn every_comment_route_rechecks_hidden_deleted_parent() {
    for state in [ContentState::Hidden, ContentState::Deleted] {
        let (f, p) = setup(vec![Post { state, ..row(12) }]);
        p.0.lock().unwrap().comments.push(comment(20));
        assert!(matches!(
            f.comments(ctx(), id(12), window(), 20).await,
            Err(FeedError::ResourceUnavailable)
        ));
        for cmd in [
            FeedCommand::EditComment {
                comment: id(20),
                expected_revision: 1,
                text: "Edit".into(),
            },
            FeedCommand::DeleteComment {
                comment: id(20),
                expected_revision: 1,
            },
            FeedCommand::CreateComment {
                post: id(12),
                post_revision: 1,
                text: "New".into(),
            },
            FeedCommand::SetReaction {
                post: id(12),
                present: true,
            },
        ] {
            assert!(matches!(
                f.prepare(ctx(), id(50), cmd).await,
                Err(FeedError::ResourceUnavailable)
            ));
        }
    }
}
#[tokio::test]
async fn foreign_post_and_comment_mutations_are_denied() {
    let (f, p) = setup(vec![row(12)]);
    p.0.lock().unwrap().comments.push(Comment {
        author: id(10),
        ..comment(20)
    });
    for cmd in [
        FeedCommand::EditPost {
            post: id(12),
            expected_revision: 1,
            content: row(12).content,
        },
        FeedCommand::DeletePost {
            post: id(12),
            expected_revision: 1,
        },
        FeedCommand::EditComment {
            comment: id(20),
            expected_revision: 1,
            text: "Edit".into(),
        },
        FeedCommand::DeleteComment {
            comment: id(20),
            expected_revision: 1,
        },
    ] {
        assert!(matches!(
            f.prepare(ctx(), id(50), cmd).await,
            Err(FeedError::PermissionDenied)
        ));
    }
}
#[tokio::test]
async fn comment_parent_and_revision_guard_and_order_are_enforced() {
    let (f, p) = setup(vec![row(12)]);
    p.0.lock().unwrap().comments.push(comment(20));
    let page = f.comments(ctx(), id(12), window(), 20).await.unwrap();
    assert_eq!(page.items.len(), 1);
    // Preparation preserves original expectation for receipt lookup; first execution
    // must enforce this CAS under the qualified owner/transaction fence.
    let prepared = f
        .prepare(
            ctx(),
            id(50),
            FeedCommand::CreateComment {
                post: id(12),
                post_revision: 2,
                text: "New".into(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        prepared.command(),
        FeedCommand::CreateComment {
            post_revision: 2,
            ..
        }
    ));
    p.0.lock().unwrap().comments[0].event = id(999);
    assert!(matches!(
        f.comments(ctx(), id(12), window(), 20).await,
        Err(FeedError::DependencyUnavailable)
    ));
}
#[tokio::test]
async fn new_reaction_uses_current_parent_and_cannot_choose_another_actor() {
    let (f, p) = setup(vec![row(12)]);
    let prepared = f
        .prepare(
            ctx(),
            id(50),
            FeedCommand::SetReaction {
                post: id(12),
                present: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(prepared.scope().unwrap().actor, ctx().actor);
    assert_eq!(prepared.parent(), Some(id(12)));
    assert_eq!(prepared.parent_revision(), Some(1));
    p.0.lock().unwrap().stale = true;
    assert!(f
        .prepare(
            ctx(),
            id(50),
            FeedCommand::SetReaction {
                post: id(12),
                present: false
            }
        )
        .await
        .is_err());
}
#[tokio::test]
async fn create_targets_and_canonical_fingerprints_are_stable_and_input_bound() {
    use concert_application::commands::*;
    let (f, _) = setup(vec![]);
    let c = FeedCommand::CreatePost {
        content: row(12).content,
    };
    let a = f.prepare(ctx(), id(50), c.clone()).await.unwrap();
    let b = f.prepare(ctx(), id(50), c).await.unwrap();
    assert_eq!(a.target(), b.target());
    let k = FingerprintKeys::new(SigningKey::new("test", &[17; 32]).unwrap(), vec![]).unwrap();
    let hash = k.sign(&a.canonical().unwrap()).unwrap();
    assert!(k.verify(&b.canonical().unwrap(), &hash).unwrap());
    let changed = f
        .prepare(
            ctx(),
            id(50),
            FeedCommand::CreatePost {
                content: Content {
                    text: "Different".into(),
                    ..row(12).content
                },
            },
        )
        .await
        .unwrap();
    assert_eq!(a.target(), changed.target());
    assert!(!k.verify(&changed.canonical().unwrap(), &hash).unwrap());
}
#[tokio::test]
async fn all_commands_generate_the_registered_exact_canonical_path() {
    let (f, p) = setup(vec![Post {
        author: ctx().actor,
        ..row(12)
    }]);
    p.0.lock().unwrap().comments.push(comment(20));
    for (n, cmd) in [
        FeedCommand::CreatePost {
            content: row(12).content,
        },
        FeedCommand::EditPost {
            post: id(12),
            expected_revision: 1,
            content: row(12).content,
        },
        FeedCommand::DeletePost {
            post: id(12),
            expected_revision: 1,
        },
        FeedCommand::CreateComment {
            post: id(12),
            post_revision: 1,
            text: "New".into(),
        },
        FeedCommand::EditComment {
            comment: id(20),
            expected_revision: 1,
            text: "Edit".into(),
        },
        FeedCommand::DeleteComment {
            comment: id(20),
            expected_revision: 1,
        },
        FeedCommand::SetReaction {
            post: id(12),
            present: true,
        },
        FeedCommand::SetReaction {
            post: id(12),
            present: false,
        },
    ]
    .into_iter()
    .enumerate()
    {
        let prepared = f.prepare(ctx(), id(50 + n as u128), cmd).await.unwrap();
        assert_eq!(
            prepared.canonical().unwrap().scope(),
            prepared.scope().unwrap()
        );
    }
}
#[tokio::test]
async fn create_rejects_unready_media_or_nonpublic_link() {
    for content in [
        Content {
            media_ids: vec![id(80)],
            ..row(12).content
        },
        Content {
            links: vec![ResourceLink {
                target_type: TargetType::Party,
                target_id: id(90),
            }],
            ..row(12).content
        },
    ] {
        let (f, _) = setup(vec![]);
        assert!(f
            .prepare(ctx(), id(50), FeedCommand::CreatePost { content })
            .await
            .is_err());
    }
}

#[tokio::test]
async fn owner_scan_continuation_survives_an_empty_authorized_page() {
    let (f, p) = setup(vec![]);
    let boundary = row(12).position();
    {
        let mut state = p.0.lock().unwrap();
        state.scan_override = Some(boundary);
        state.more_override = Some(true);
    }
    let page = f.page(ctx(), window(), Filter::All, 20).await.unwrap();
    assert!(page.items.is_empty());
    assert_eq!(page.next, Some(boundary));
}
#[tokio::test]
async fn owner_scan_cannot_jump_backwards_or_skip_a_returned_row() {
    for boundary in [
        Position {
            created_at: now() + chrono::Duration::seconds(1),
            id: id(20),
        },
        row(13).position(),
    ] {
        let (f, p) = setup(vec![row(12)]);
        let mut state = p.0.lock().unwrap();
        state.scan_override = Some(boundary);
        state.more_override = Some(true);
        drop(state);
        assert!(matches!(
            f.page(ctx(), window(), Filter::All, 20).await,
            Err(FeedError::DependencyUnavailable)
        ));
    }
}

#[tokio::test]
async fn identical_edit_intent_survives_revision_advance_for_receipt_replay() {
    let (f, p) = setup(vec![Post {
        author: ctx().actor,
        ..row(12)
    }]);
    p.0.lock().unwrap().comments.push(comment(20));
    for command in [
        FeedCommand::EditPost {
            post: id(12),
            expected_revision: 1,
            content: row(12).content,
        },
        FeedCommand::EditComment {
            comment: id(20),
            expected_revision: 1,
            text: "Original edit".into(),
        },
    ] {
        let before = f.prepare(ctx(), id(60), command.clone()).await.unwrap();
        {
            let mut state = p.0.lock().unwrap();
            state.rows[0].revision = 2;
            state.comments[0].revision = 2;
        }
        let replay = f.prepare(ctx(), id(60), command).await.unwrap();
        assert_eq!(before.scope().unwrap(), replay.scope().unwrap());
        assert_eq!(before.target(), replay.target());
    }
}
