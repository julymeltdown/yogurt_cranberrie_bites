//! The only authority implementation capable of owner SQL writes in this slice.
//! Compiled exclusively under cfg(test); never accepted on the RPC surface.
use super::*;
use async_trait::async_trait;
use chrono::Duration;
use post_application::concert_feed::{Filter, Window};
use post_domain::concert_feed::{Command, Content, Link, LinkKind, Mutation};
use sqlx::postgres::PgConnectOptions;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tokio::sync::{OwnedRwLockReadGuard, RwLock};
use uuid::Uuid;

#[derive(Default)]
struct Policy {
    blocked: BTreeSet<(Uuid, Uuid)>,
    media: BTreeMap<Uuid, (Uuid, Uuid, bool)>, // event,uploader,qualified READY POST
    links: BTreeMap<(String, Uuid), (Uuid, bool)>, // event,current public eligibility
}
#[derive(Default)]
struct TestAuthority {
    policy: Arc<RwLock<Policy>>,
    reject_commit: Arc<AtomicBool>,
}
/// Storage pagination fixture only; this module is entirely cfg(test). This
/// constructor is never available to production RPC/root configuration.
pub(super) fn paging_owner(pool: PgPool, blocked: Vec<(Uuid, Uuid)>) -> Owner {
    Owner {
        pool,
        authority: Arc::new(TestAuthority {
            policy: Arc::new(RwLock::new(Policy {
                blocked: blocked.into_iter().collect(),
                ..Default::default()
            })),
            reject_commit: Arc::default(),
        }),
    }
}
struct TestFence {
    context: Context,
    binding: [u8; 32],
    _lock: OwnedRwLockReadGuard<Policy>,
    checks: AtomicUsize,
    reject: Arc<AtomicBool>,
}
impl Fence for TestFence {
    fn check(&self, c: &Context, b: &[u8; 32], now: DateTime<Utc>) -> Result<(), Error> {
        c.validate(now)?;
        if self.context != *c
            || self.binding != *b
            || (self.checks.fetch_add(1, Ordering::SeqCst) > 0
                && self.reject.load(Ordering::SeqCst))
        {
            return Err(Error::Authority);
        }
        Ok(())
    }
}
impl Policy {
    fn subject(&self, viewer: Uuid, author: Uuid) -> Result<(), Error> {
        if self.blocked.contains(&(viewer, author)) || self.blocked.contains(&(author, viewer)) {
            Err(Error::Forbidden)
        } else {
            Ok(())
        }
    }
    fn content(&self, c: &Content, event: Uuid, uploader: Uuid) -> Result<(), Error> {
        for id in &c.media_ids {
            if self.media.get(id) != Some(&(event, uploader, true)) {
                return Err(Error::Forbidden);
            }
        }
        for link in &c.links {
            if self
                .links
                .get(&(format!("{:?}", link.target_type), link.target_id))
                != Some(&(event, true))
            {
                return Err(Error::Forbidden);
            }
        }
        Ok(())
    }
}
#[async_trait]
impl Authority for TestAuthority {
    async fn acquire(&self, c: &Context, a: &Access, b: [u8; 32]) -> Result<Box<dyn Fence>, Error> {
        let policy = self.policy.clone().read_owned().await;
        match a {
            Access::ListPosts { .. } => {}
            Access::Write {
                command,
                parent,
                comment,
            } => {
                if let Some(p) = parent {
                    policy.subject(c.actor_id, p.author_id)?;
                    if command.mutation.content().is_none() {
                        policy.content(&p.content, c.event_id, p.author_id)?;
                    }
                }
                if let Some(comment) = comment {
                    policy.subject(c.actor_id, comment.author_id)?;
                }
                if let Some(content) = command.mutation.content() {
                    policy.content(content, c.event_id, c.actor_id)?;
                }
            }
            Access::ReadPost { post } => {
                policy.subject(c.actor_id, post.author_id)?;
                policy.content(&post.content, c.event_id, post.author_id)?;
            }
            Access::ReadComment { parent, comment } => {
                policy.subject(c.actor_id, parent.author_id)?;
                policy.subject(c.actor_id, comment.author_id)?;
            }
            Access::Receipt {
                parent, comment, ..
            } => {
                if let Some(p) = parent {
                    policy.subject(c.actor_id, p.author_id)?;
                }
                if let Some(comment) = comment {
                    policy.subject(c.actor_id, comment.author_id)?;
                }
            }
        }
        Ok(Box::new(TestFence {
            context: c.clone(),
            binding: b,
            _lock: policy,
            checks: AtomicUsize::new(0),
            reject: self.reject_commit.clone(),
        }))
    }
}
fn context(actor: Uuid, event: Uuid) -> Context {
    Context {
        actor_id: actor,
        event_id: event,
        session_family_id: Uuid::new_v4(),
        request_id: Uuid::new_v4(),
        original_authority: [7; 32],
        expires_at: Utc::now() + Duration::seconds(60),
    }
}
fn text(s: &str) -> Content {
    Content {
        text: s.into(),
        media_ids: vec![],
        links: vec![],
    }
}
fn command(c: &Context, post: Uuid, mutation: Mutation) -> Command {
    Command {
        operation_id: Uuid::new_v4(),
        actor_id: c.actor_id,
        event_id: c.event_id,
        post_id: post,
        idempotency_key: Uuid::new_v4().to_string(),
        mutation,
    }
}
async fn count(pool: &PgPool, table: &str) -> i64 {
    assert!([
        "post_concert_posts",
        "post_concert_receipts",
        "post_concert_outbox"
    ]
    .contains(&table));
    sqlx::query_scalar(&format!("select count(*) from {table}"))
        .fetch_one(pool)
        .await
        .unwrap()
}
async fn fixture() -> PgPool {
    let url = std::env::var("POST_CONCERT_TEST_DATABASE_URL").expect("use owned fixture runner");
    let options: PgConnectOptions = url.parse().unwrap();
    assert_eq!(options.get_host(), "127.0.0.1");
    assert!(options
        .get_database()
        .unwrap()
        .starts_with("mcr_post_concert_fixture_"));
    assert!(
        options.get_socket().is_none()
            && options.get_options().is_none()
            && !url.contains('?')
            && !url.contains('#')
    );
    let pool = PgPool::connect(&url).await.unwrap();
    let lineage: String =
        sqlx::query_scalar("select system_identifier::text from pg_control_system()")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        Some(lineage),
        std::env::var("POST_CONCERT_TEST_LINEAGE").ok()
    );
    let empty: bool =
        sqlx::query_scalar("select not exists(select 1 from pg_tables where schemaname='public')")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(empty);
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    sqlx::migrate::Migrator::new(directory)
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
    pool
}

#[tokio::test]
#[ignore = "owned ephemeral PostgreSQL fixture only; use post_concert_feed_postgres.py"]
async fn postgres_owner_authorization_persistence_and_races() {
    let pool = fixture().await;
    let policy = Arc::new(TestAuthority::default());
    let owner = Owner {
        pool: pool.clone(),
        authority: policy.clone(),
    };
    let a = context(Uuid::new_v4(), Uuid::new_v4());
    let b = context(Uuid::new_v4(), a.event_id);
    let p = Uuid::new_v4();
    let create = command(
        &a,
        p,
        Mutation::CreatePost {
            content: text("PRIVATE_TEXT_CANARY"),
        },
    );
    assert_eq!(
        Owner::new(pool.clone()).execute(&a, &create).await,
        Err(Error::Authority)
    );
    assert_eq!(count(&pool, "post_concert_posts").await, 0);
    let first = owner.execute(&a, &create).await.unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(owner.execute(&a, &create).await.unwrap(), first);
    assert_eq!(
        owner.receipt(&a, &create).await.unwrap(),
        Some(first.clone())
    );
    assert_eq!(count(&pool, "post_concert_receipts").await, 1);
    assert_eq!(count(&pool, "post_concert_outbox").await, 1);
    // Exact command identity, actor and event are immutable on retry.
    let mut changed = create.clone();
    changed.mutation = Mutation::CreatePost {
        content: text("changed"),
    };
    assert_eq!(owner.execute(&a, &changed).await, Err(Error::Idempotency));
    let mut changed = create.clone();
    changed.operation_id = Uuid::new_v4();
    assert_eq!(owner.execute(&a, &changed).await, Err(Error::Idempotency));
    let other_event = context(a.actor_id, Uuid::new_v4());
    assert!(owner.post(&other_event, p).await.unwrap().is_none());
    assert_eq!(
        owner
            .execute(
                &b,
                &command(
                    &b,
                    p,
                    Mutation::EditPost {
                        expected_revision: 1,
                        content: text("foreign")
                    }
                )
            )
            .await,
        Err(Error::Forbidden)
    );
    // Concurrent owner edits: exactly one current revision wins.
    let e1 = command(
        &a,
        p,
        Mutation::EditPost {
            expected_revision: 1,
            content: text("one"),
        },
    );
    let e2 = command(
        &a,
        p,
        Mutation::EditPost {
            expected_revision: 1,
            content: text("two"),
        },
    );
    let (r1, r2) = tokio::join!(owner.execute(&a, &e1), owner.execute(&a, &e2));
    assert_eq!(
        [r1.is_ok(), r2.is_ok()].into_iter().filter(|v| *v).count(),
        1
    );
    assert!(r1 == Err(Error::Conflict) || r2 == Err(Error::Conflict));
    let view = owner.post(&a, p).await.unwrap().unwrap();
    assert_eq!(view.post.revision, 2);
    assert_eq!(view.post.created_at, first.committed_at);
    assert!(view.post.edited_at.is_some());
    // Duplicate create retry race has one durable result and one outbox.
    let p2 = Uuid::new_v4();
    let c2 = command(
        &a,
        p2,
        Mutation::CreatePost {
            content: text("second"),
        },
    );
    let (r1, r2) = tokio::join!(owner.execute(&a, &c2), owner.execute(&a, &c2));
    assert_eq!(r1.unwrap(), r2.unwrap());
    // Comment owner, parent CAS, and revision semantics.
    let comment_id = Uuid::new_v4();
    let cc = command(
        &b,
        p,
        Mutation::CreateComment {
            comment_id,
            post_revision: 2,
            text: "reply".into(),
        },
    );
    let cr = owner.execute(&b, &cc).await.unwrap();
    assert_eq!((cr.revision, cr.post_revision), (1, 3));
    assert_eq!(
        owner
            .execute(
                &a,
                &command(
                    &a,
                    p,
                    Mutation::DeleteComment {
                        comment_id,
                        expected_revision: 1
                    }
                )
            )
            .await,
        Err(Error::Forbidden)
    );
    let ec = command(
        &b,
        p,
        Mutation::EditComment {
            comment_id,
            expected_revision: 1,
            text: "edited reply".into(),
        },
    );
    let er = owner.execute(&b, &ec).await.unwrap();
    assert_eq!((er.revision, er.post_revision), (2, 4));
    assert_eq!(
        owner.comment(&b, comment_id).await.unwrap().unwrap().text,
        "edited reply"
    );
    assert!(owner
        .comment(&other_event, comment_id)
        .await
        .unwrap()
        .is_none());
    // Block in either direction denies reply, reaction, read and receipt replay.
    policy
        .policy
        .write()
        .await
        .blocked
        .insert((a.actor_id, b.actor_id));
    assert!(matches!(
        owner.comments(&b, p, Utc::now(), None, 20).await,
        Err(Error::Forbidden)
    ));
    assert_eq!(
        owner
            .execute(
                &b,
                &command(
                    &b,
                    p,
                    Mutation::SetReaction {
                        expected_revision: 4,
                        active: true
                    }
                )
            )
            .await,
        Err(Error::Forbidden)
    );
    assert_eq!(owner.execute(&b, &cc).await, Err(Error::Forbidden));
    policy.policy.write().await.blocked.clear();
    // Reactions are set semantics; duplicate retries do not count twice.
    let like = command(
        &b,
        p,
        Mutation::SetReaction {
            expected_revision: 4,
            active: true,
        },
    );
    let lr = owner.execute(&b, &like).await.unwrap();
    assert_eq!((lr.reaction_count, lr.post_revision), (1, 5));
    assert_eq!(owner.execute(&b, &like).await.unwrap(), lr);
    let no_op = owner
        .execute(
            &b,
            &command(
                &b,
                p,
                Mutation::SetReaction {
                    expected_revision: 5,
                    active: true,
                },
            ),
        )
        .await
        .unwrap();
    assert_eq!((no_op.reaction_count, no_op.post_revision), (1, 6));
    let unlike = owner
        .execute(
            &b,
            &command(
                &b,
                p,
                Mutation::SetReaction {
                    expected_revision: 6,
                    active: false,
                },
            ),
        )
        .await
        .unwrap();
    assert_eq!((unlike.reaction_count, unlike.post_revision), (0, 7));
    // Fence loss rolls back content, receipt AND outbox, including reaction rows.
    let before = (
        count(&pool, "post_concert_receipts").await,
        count(&pool, "post_concert_outbox").await,
    );
    policy.reject_commit.store(true, Ordering::SeqCst);
    let bad = command(
        &b,
        p,
        Mutation::SetReaction {
            expected_revision: 7,
            active: true,
        },
    );
    assert_eq!(owner.execute(&b, &bad).await, Err(Error::Authority));
    policy.reject_commit.store(false, Ordering::SeqCst);
    assert_eq!(
        (
            count(&pool, "post_concert_receipts").await,
            count(&pool, "post_concert_outbox").await
        ),
        before
    );
    let view = owner.post(&b, p).await.unwrap().unwrap();
    assert_eq!(view.post.revision, 7);
    assert!(!view.reacted_by_me);
    // Expired original deadline and foreign actor binding fail before effects.
    let mut expired = a.clone();
    expired.expires_at = Utc::now() - Duration::seconds(1);
    assert_eq!(
        owner.execute(&expired, &create).await,
        Err(Error::Authority)
    );
    assert_eq!(owner.execute(&b, &create).await, Err(Error::Authority));
    // Comment paging is DESC with immutable upper bound; deletion updates the
    // parent counter and cannot expose the removed row through its ID.
    let ceiling = Utc::now();
    let second_comment = Uuid::new_v4();
    owner
        .execute(
            &b,
            &command(
                &b,
                p,
                Mutation::CreateComment {
                    comment_id: second_comment,
                    post_revision: 7,
                    text: "newer reply".into(),
                },
            ),
        )
        .await
        .unwrap();
    let comments = owner.comments(&b, p, Utc::now(), None, 1).await.unwrap();
    assert_eq!(comments.items[0].comment_id, second_comment);
    assert!(comments.has_more);
    let rest = owner
        .comments(&b, p, Utc::now(), comments.scanned_through, 100)
        .await
        .unwrap();
    assert_eq!(rest.items[0].comment_id, comment_id);
    let bounded = owner.comments(&b, p, ceiling, None, 100).await.unwrap();
    assert_eq!(bounded.items.len(), 1);
    assert_eq!(bounded.items[0].comment_id, comment_id);
    let dc = owner
        .execute(
            &b,
            &command(
                &b,
                p,
                Mutation::DeleteComment {
                    comment_id: second_comment,
                    expected_revision: 1,
                },
            ),
        )
        .await
        .unwrap();
    assert_eq!(dc.post_revision, 9);
    assert!(matches!(
        owner.comment(&b, second_comment).await,
        Err(Error::NotFound)
    ));
    let reopened = Owner {
        pool: pool.clone(),
        authority: policy.clone(),
    };
    let persisted = reopened.post(&b, p).await.unwrap().unwrap();
    assert_eq!(
        (persisted.post.revision, persisted.post.comment_count),
        (9, 1)
    );
    // Exact media and linked-object qualifications; assertions are insufficient.
    let media = Uuid::new_v4();
    let link = Link {
        target_type: LinkKind::Party,
        target_id: Uuid::new_v4(),
    };
    let content = Content {
        text: "qualified".into(),
        media_ids: vec![media],
        links: vec![link.clone()],
    };
    let qualified = command(&a, Uuid::new_v4(), Mutation::CreatePost { content });
    assert_eq!(owner.execute(&a, &qualified).await, Err(Error::Forbidden));
    {
        let mut state = policy.policy.write().await;
        state.media.insert(media, (a.event_id, a.actor_id, true));
        state
            .links
            .insert(("Party".into(), link.target_id), (Uuid::new_v4(), true));
    }
    assert_eq!(owner.execute(&a, &qualified).await, Err(Error::Forbidden));
    policy
        .policy
        .write()
        .await
        .links
        .insert(("Party".into(), link.target_id), (a.event_id, true));
    policy
        .policy
        .write()
        .await
        .media
        .insert(media, (a.event_id, b.actor_id, true));
    assert_eq!(owner.execute(&a, &qualified).await, Err(Error::Forbidden));
    policy
        .policy
        .write()
        .await
        .media
        .insert(media, (Uuid::new_v4(), a.actor_id, true));
    assert_eq!(owner.execute(&a, &qualified).await, Err(Error::Forbidden));
    policy
        .policy
        .write()
        .await
        .media
        .insert(media, (a.event_id, a.actor_id, true));
    owner.execute(&a, &qualified).await.unwrap();
    policy
        .policy
        .write()
        .await
        .media
        .insert(media, (a.event_id, a.actor_id, false));
    assert!(matches!(
        owner.post(&a, qualified.post_id).await,
        Err(Error::Forbidden)
    ));
    policy
        .policy
        .write()
        .await
        .media
        .insert(media, (a.event_id, a.actor_id, true));
    // Hidden/deleted parents cannot leak via comment ID/list/edit/replay body.
    sqlx::query("update post_concert_posts set state='HIDDEN' where post_id=$1")
        .bind(p)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        owner.comment(&b, comment_id).await,
        Err(Error::NotFound)
    ));
    assert!(matches!(
        owner.comments(&b, p, Utc::now(), None, 20).await,
        Err(Error::NotFound)
    ));
    assert_eq!(
        owner
            .execute(
                &b,
                &command(
                    &b,
                    p,
                    Mutation::EditComment {
                        comment_id,
                        expected_revision: 2,
                        text: "leak".into()
                    }
                )
            )
            .await,
        Err(Error::NotFound)
    );
    assert_eq!(owner.execute(&b, &cc).await, Err(Error::NotFound));
    assert_eq!(owner.receipt(&b, &cc).await, Err(Error::NotFound));
    // Metadata-only lookup never contains old content even for owner delete replay.
    let delete_command = command(
        &a,
        p2,
        Mutation::DeletePost {
            expected_revision: 1,
        },
    );
    let deleted = owner.execute(&a, &delete_command).await.unwrap();
    assert_eq!(owner.execute(&a, &delete_command).await.unwrap(), deleted);
    assert_eq!(
        owner.receipt(&a, &delete_command).await.unwrap(),
        Some(deleted.clone())
    );
    assert!(deleted.deleted);
    assert!(matches!(owner.post(&a, p2).await, Err(Error::NotFound)));
    let json: String =
        sqlx::query_scalar("select string_agg(outcome::text,'') from post_concert_receipts")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!json.contains("PRIVATE_TEXT_CANARY"));
    assert!(!json.contains(&create.idempotency_key));
    // Relational immutability is enforced independently of application checks.
    assert!(
        sqlx::query("update post_concert_posts set event_id=$2 where post_id=$1")
            .bind(p)
            .bind(Uuid::new_v4())
            .execute(&pool)
            .await
            .is_err()
    );
    // Stable chronological order and strict boundary, even after edits/reactions.
    let upper = Utc::now();
    let page = owner
        .page(
            &a,
            &Window {
                upper,
                after: None,
                filter: Filter::All,
                limit: 1,
            },
        )
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    let next = owner
        .page(
            &a,
            &Window {
                upper,
                after: page.scanned_through,
                filter: Filter::All,
                limit: 100,
            },
        )
        .await
        .unwrap();
    assert!(next
        .items
        .iter()
        .all(|i| i.post.post_id != page.items[0].post.post_id));
    let filtered = owner
        .page(
            &a,
            &Window {
                upper,
                after: None,
                filter: Filter::PartyLink,
                limit: 100,
            },
        )
        .await
        .unwrap();
    assert_eq!(filtered.items.len(), 1);
    assert_eq!(filtered.items[0].post.post_id, qualified.post_id);
    // Current revocation in related owner hides the whole post conservatively.
    policy
        .policy
        .write()
        .await
        .links
        .insert(("Party".into(), link.target_id), (a.event_id, false));
    assert!(owner
        .page(
            &a,
            &Window {
                upper,
                after: None,
                filter: Filter::All,
                limit: 100
            }
        )
        .await
        .unwrap()
        .items
        .is_empty());
    // Fence holds policy revision stable until transaction completion.
    let mut tx = pool.begin().await.unwrap();
    let access = Access::ListPosts {
        window: Window {
            upper,
            after: None,
            filter: Filter::All,
            limit: 1,
        },
    };
    let guard = owner.admit(&a, &access, &mut tx).await.unwrap();
    assert!(policy.policy.try_write().is_err());
    let mut wrong = a.clone();
    wrong.request_id = Uuid::new_v4();
    assert_eq!(
        guard.0.check(&wrong, &guard.1, Utc::now()),
        Err(Error::Authority)
    );
    let mut wrong = a.clone();
    wrong.original_authority = [8; 32];
    assert_eq!(
        guard.0.check(&wrong, &guard.1, Utc::now()),
        Err(Error::Authority)
    );
    assert_eq!(
        guard.0.check(&a, &[0; 32], Utc::now()),
        Err(Error::Authority)
    );
    checked_commit(tx, &a, &[guard]).await.unwrap();
    assert!(policy.policy.try_write().is_ok());
    // Equal timestamp fixtures prove the UUID tie-break in actual owner SQL.
    let tied = context(a.actor_id, Uuid::new_v4());
    let timestamp = Utc::now() - Duration::seconds(1);
    for id in [Uuid::from_u128(1), Uuid::from_u128(2)] {
        sqlx::query("insert into post_concert_posts(post_id,event_id,author_id,content,revision,state,created_at) values($1,$2,$3,$4,1,'PUBLISHED',$5)")
            .bind(id).bind(tied.event_id).bind(tied.actor_id).bind(serde_json::to_value(text("tie fixture")).unwrap()).bind(timestamp).execute(&pool).await.unwrap();
    }
    let one = owner
        .page(
            &tied,
            &Window {
                upper: Utc::now(),
                after: None,
                filter: Filter::All,
                limit: 1,
            },
        )
        .await
        .unwrap();
    assert_eq!(one.items[0].post.post_id, Uuid::from_u128(2));
    let two = owner
        .page(
            &tied,
            &Window {
                upper: Utc::now(),
                after: one.scanned_through,
                filter: Filter::All,
                limit: 1,
            },
        )
        .await
        .unwrap();
    assert_eq!(two.items[0].post.post_id, Uuid::from_u128(1));
    assert!(!two.has_more);
    // A receipt lookup never executes a missing write.
    let missing = command(
        &a,
        Uuid::new_v4(),
        Mutation::CreatePost {
            content: text("missing"),
        },
    );
    let n = count(&pool, "post_concert_posts").await;
    assert!(owner.receipt(&a, &missing).await.unwrap().is_none());
    assert_eq!(count(&pool, "post_concert_posts").await, n);
    let mismatch:i64=sqlx::query_scalar("select count(*) from post_concert_receipts r full join post_concert_outbox o using(operation_id) where r.operation_id is null or o.operation_id is null").fetch_one(&pool).await.unwrap();
    assert_eq!(mismatch, 0);
    pool.close().await;
}
