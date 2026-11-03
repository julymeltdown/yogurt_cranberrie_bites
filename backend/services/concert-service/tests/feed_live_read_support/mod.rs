//! Deterministic adapter fixtures only. Never installed in production startup.
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::{feed::*, Clock};
use concert_domain::feed::*;
use concert_service::feed::{cursor::CursorCodec, live_read::ReadFeed};
use serde_json::Value;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::Notify;
use uuid::Uuid;

pub fn context() -> Context {
    Context {
        actor: Uuid::from_u128(1),
        event: Uuid::from_u128(2),
        session_family: Uuid::from_u128(3),
        request: Uuid::from_u128(4),
        authority_digest: [5; 32],
        expires_at: Utc::now() + Duration::seconds(3),
    }
}
pub struct State {
    pub calls: AtomicUsize,
    pub hang: AtomicBool,
    pub started: Notify,
    pub dropped: AtomicBool,
    pub more: AtomicBool,
    pub with_row: AtomicBool,
    pub delete_parent: AtomicBool,
    pub invalid_author: AtomicBool,
    pub missing_author: AtomicBool,
    pub deny_author: AtomicBool,
    pub last_window: std::sync::Mutex<Option<Window>>,
    pub post: Post,
    pub comment: Comment,
}
impl State {
    pub fn new(context: Context) -> Arc<Self> {
        let author = Uuid::from_u128(10);
        let post = Post {
            id: Uuid::from_u128(9),
            event: context.event,
            author,
            content: Content {
                text: "Current owner text".into(),
                media_ids: vec![],
                links: vec![],
            },
            state: ContentState::Published,
            public: true,
            revision: 3,
            created_at: Utc::now() - Duration::seconds(10),
            edited_at: None,
            comment_count: 1,
            reaction_count: 0,
            reacted_by_me: false,
        };
        let comment = Comment {
            id: Uuid::from_u128(11),
            post: post.id,
            event: context.event,
            author,
            text: "Current comment".into(),
            state: ContentState::Published,
            revision: 2,
            created_at: post.created_at + Duration::seconds(1),
        };
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            hang: AtomicBool::new(false),
            started: Notify::new(),
            dropped: AtomicBool::new(false),
            more: AtomicBool::new(false),
            with_row: AtomicBool::new(false),
            delete_parent: AtomicBool::new(false),
            invalid_author: AtomicBool::new(false),
            missing_author: AtomicBool::new(false),
            deny_author: AtomicBool::new(false),
            last_window: std::sync::Mutex::new(None),
            post,
            comment,
        })
    }
}
pub struct Reader(pub Arc<State>);
struct DropProbe(Arc<State>);
impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.dropped.store(true, Ordering::SeqCst);
    }
}
#[async_trait]
impl PostReader for Reader {
    async fn page(
        &self,
        _: Context,
        window: Window,
        _: Filter,
        _: u16,
    ) -> Result<ScannedPosts, FeedError> {
        self.0.calls.fetch_add(1, Ordering::SeqCst);
        *self.0.last_window.lock().unwrap() = Some(window);
        if self.0.hang.load(Ordering::SeqCst) {
            let _probe = DropProbe(self.0.clone());
            self.0.started.notify_one();
            std::future::pending::<()>().await;
        }
        Ok(ScannedPosts {
            items: if self.0.with_row.load(Ordering::SeqCst) {
                vec![self.0.post.clone()]
            } else {
                vec![]
            },
            scanned_through: window
                .contains(self.0.post.position())
                .then(|| self.0.post.position()),
            has_more: self.0.more.load(Ordering::SeqCst),
        })
    }
    async fn post(&self, _: Context, id: Uuid) -> Result<Option<Post>, FeedError> {
        if self.0.delete_parent.load(Ordering::SeqCst) {
            return Ok(None);
        }
        Ok((id == self.0.post.id).then(|| self.0.post.clone()))
    }
}
#[async_trait]
impl CommentReader for Reader {
    async fn comment(&self, _: Context, id: Uuid) -> Result<Option<Comment>, FeedError> {
        Ok((id == self.0.comment.id).then(|| self.0.comment.clone()))
    }
    async fn comments(
        &self,
        _: Context,
        _: Uuid,
        _: Window,
        _: u16,
    ) -> Result<ScannedComments, FeedError> {
        Ok(ScannedComments {
            items: vec![self.0.comment.clone()],
            scanned_through: Some(self.0.comment.position()),
            has_more: self.0.more.load(Ordering::SeqCst),
        })
    }
}
pub struct Policy(pub Arc<State>);
#[async_trait]
impl FeedPolicy for Policy {
    async fn observe(
        &self,
        context: Context,
        subject: Uuid,
        action: Action,
    ) -> Result<Observation, FeedError> {
        if subject != context.actor && self.0.deny_author.load(Ordering::SeqCst) {
            return Err(FeedError::PermissionDenied);
        }
        Ok(Observation {
            context,
            subject,
            action,
            event_revision: 1,
            relationship_revision: 2,
            auth_actor_revision: 3,
            auth_subject_revision: 4,
            friend_revision: 5,
            valid_until: context.expires_at,
        })
    }
    async fn fence(&self, observation: Observation) -> Result<(), FeedError> {
        if observation.subject != observation.context.actor
            && self.0.deny_author.load(Ordering::SeqCst)
        {
            return Err(FeedError::PermissionDenied);
        }
        Ok(())
    }
}
pub struct Resources(pub Arc<State>);
#[async_trait]
impl FeedResources for Resources {
    async fn author(&self, _: Context, user_id: Uuid) -> Result<PublicAuthor, FeedError> {
        if self.0.missing_author.load(Ordering::SeqCst) {
            return Err(FeedError::DependencyUnavailable);
        }
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../../contracts/fixtures/domain/public-post.json"
        ))
        .unwrap();
        let mut author = fixture["author"].clone();
        author["user_id"] = serde_json::json!(user_id);
        if self.0.invalid_author.load(Ordering::SeqCst) {
            author["private_email"] = serde_json::json!("private@example.invalid");
        }
        Ok(PublicAuthor {
            user_id,
            json: serde_json::to_vec(&author).unwrap(),
        })
    }
    async fn public_link(&self, _: Context, _: ResourceLink) -> Result<bool, FeedError> {
        Err(FeedError::DependencyUnavailable)
    }
    async fn public_media(&self, _: Context, _: Uuid, _: Uuid) -> Result<bool, FeedError> {
        Err(FeedError::DependencyUnavailable)
    }
}
pub struct WallClock;
impl Clock for WallClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
pub type Harness = ReadFeed<Reader, Policy, Resources, WallClock>;
pub fn facade(state: Arc<State>) -> Harness {
    ReadFeed::new(
        Feed {
            reader: Reader(state.clone()),
            policy: Policy(state.clone()),
            resources: Resources(state),
            clock: WallClock,
        },
        CursorCodec::new(&[42; 32]).unwrap(),
    )
}
