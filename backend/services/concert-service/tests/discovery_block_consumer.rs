//! The block consumer: pair signals become projection rows.
//!
//! The consumer's job is narrower than it looks. A `FRIEND_RELATIONSHIP_CHANGED`
//! signal says *something about this pair changed* — it is emitted for a block,
//! an unblock, a follow and a mute alike. Treating the signal as "blocked" would
//! mirror every follow as a block, so the consumer resolves the pair through the
//! authority and writes only the resolved state.
//!
//! The cases here:
//!
//! * a pair signal resolves to a block and the profile disappears from discovery;
//! * a pair signal that resolves to "unblocked" restores the profile;
//! * a non-pair signal is skipped without touching the projection;
//! * an authority that cannot answer stops the cursor rather than restoring a
//!   blocked profile;
//! * the cursor advances only past applied signals, so a crash re-reads.
use concert_service::discovery::block_consumer::{
    seed_cursor, BlockConsumer, BlockJournal, JournalPage, PairAuthority, PairFailure, PairState,
};
use concert_service::discovery::projection::{
    DiscoverySource, ProjectionError, ProjectionWriter, SourceFailure, SourcePage,
};
use concert_service::discovery::{discover, validate, Category, PeopleQuery, MAX_LIMIT};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use uuid::Uuid;

use concert_service::adapters::out::relationship_authority::invalidation::{
    InvalidationOwner, OwnerCursor, SignalReason, SignalScope,
};

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

async fn pool(variable: &str) -> anyhow::Result<PgPool> {
    let url = std::env::var(variable).map_err(|_| anyhow::anyhow!("{variable} is required"))?;
    let options: sqlx::postgres::PgConnectOptions = url.parse()?;
    anyhow::ensure!(
        options.get_username() == "mcr_owner",
        "block fixture must use the owned cluster"
    );
    Ok(sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?)
}

/// A source that reports nothing changed: these tests drive the projection
/// directly so the block consumer is the only thing under test.
struct NoopSource;

#[async_trait::async_trait]
impl DiscoverySource for NoopSource {
    async fn read_changed(
        &self,
        _: Option<(DateTime<Utc>, Uuid)>,
        _: i32,
    ) -> Result<SourcePage, SourceFailure> {
        Ok(SourcePage {
            profiles: Vec::new(),
            next: None,
            more: false,
        })
    }
}

/// A journal that returns one prepared page, then reports caught-up.
struct ScriptedJournal {
    page: JournalPage,
    reads: AtomicUsize,
}

#[async_trait::async_trait]
impl BlockJournal for ScriptedJournal {
    async fn read_after(&self, after: OwnerCursor, _limit: u16) -> Result<JournalPage, PairFailure> {
        let index = self.reads.fetch_add(1, Ordering::SeqCst);
        if index == 0 {
            // The page's own `next` is the cursor the consumer should land on;
            // it is deliberately NOT overwritten with `after`, so the test can
            // observe that the cursor actually advanced.
            Ok(self.page.clone())
        } else {
            Ok(JournalPage {
                next: after,
                has_more: false,
                signals: Vec::new(),
            })
        }
    }
}

/// An authority that answers from a fixed map.
struct MapAuthority {
    blocked: std::collections::BTreeMap<(Uuid, Uuid), bool>,
    failing: bool,
}

#[async_trait::async_trait]
impl PairAuthority for MapAuthority {
    async fn block_state(&self, actor: Uuid, subject: Uuid) -> Result<PairState, PairFailure> {
        if self.failing {
            return Err(PairFailure::Unavailable);
        }
        Ok(match self.blocked.get(&(actor, subject)) {
            Some(true) => PairState::Blocked,
            Some(false) => PairState::Unblocked,
            None => PairState::Unknown,
        })
    }
}

struct Fixture {
    profile: PgPool,
    concert: PgPool,
}

impl Fixture {
    async fn new() -> anyhow::Result<Self> {
        Ok(Self {
            profile: pool("PROFILE_TEST_DATABASE_URL").await?,
            concert: pool("CONCERT_TEST_DATABASE_URL").await?,
        })
    }

    fn writer(&self) -> Arc<ProjectionWriter> {
        Arc::new(ProjectionWriter::new(
            self.concert.clone(),
            Arc::new(NoopSource),
        ))
    }

    /// Insert a projected profile directly, so discovery has something to
    /// filter. The profile stream is not under test here.
    async fn projected_profile(&self, tag: &str) -> anyhow::Result<(Uuid, Uuid)> {
        let profile_id = Uuid::new_v4();
        let account_id = Uuid::new_v4();
        sqlx::query(
            "insert into discovery_profiles(profile_id,account_id,nickname,visibility) \
             values($1,$2,$3,'PUBLIC')",
        )
        .bind(profile_id)
        .bind(account_id)
        .bind(format!("block-{tag}"))
        .execute(&self.concert)
        .await?;
        Ok((profile_id, account_id))
    }

    async fn add_favorite(&self, profile_id: Uuid, item: Uuid) -> anyhow::Result<()> {
        sqlx::query(
            "insert into discovery_favorite_items(profile_id,category,catalog_item_id,label,position) \
             values($1,'music',$2,'Shared Band',0)",
        )
        .bind(profile_id)
        .bind(item)
        .execute(&self.concert)
        .await?;
        Ok(())
    }

    async fn discovered(&self, viewer: Uuid, category: Category) -> anyhow::Result<Vec<Uuid>> {
        let page = discover(
            &self.concert,
            viewer,
            &validate(PeopleQuery {
                category,
                cursor: None,
                limit: MAX_LIMIT,
            })?,
        )
        .await?;
        Ok(page.items.iter().map(|card| card.profile_id).collect())
    }

    async fn cursor_row(&self, consumer: Uuid) -> anyhow::Result<Option<(Option<Uuid>, Option<i64>)>> {
        let row: Option<(Option<Uuid>, Option<i64>)> = sqlx::query_as(
            "select epoch, sequence from discovery_block_cursor where consumer_id=$1",
        )
        .bind(consumer)
        .fetch_optional(&self.concert)
        .await?;
        Ok(row)
    }
}

fn cursor(epoch: Uuid, sequence: i64) -> OwnerCursor {
    OwnerCursor::new(InvalidationOwner::Friend, epoch, sequence).expect("valid cursor")
}

/// A pair signal for a relationship change.
fn pair_signal(actor: Uuid, subject: Uuid) -> (SignalReason, SignalScope) {
    (
        SignalReason::FriendRelationshipChanged,
        SignalScope::Pair { actor, subject },
    )
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

/// A pair signal that resolves to a block hides the profile from discovery.
#[tokio::test]
async fn a_pair_signal_resolves_to_a_block() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let (viewer, viewer_account) = fixture.projected_profile("viewer").await?;
    let (subject, subject_account) = fixture.projected_profile("subject").await?;
    let item = Uuid::new_v4();
    fixture.add_favorite(viewer, item).await?;
    fixture.add_favorite(subject, item).await?;
    assert!(fixture.discovered(viewer, Category::Music).await?.contains(&subject));

    let epoch = Uuid::new_v4();
    let consumer_id = Uuid::new_v4();
    seed_cursor(&fixture.concert, consumer_id, cursor(epoch, 0)).await?;
    let mut blocked = std::collections::BTreeMap::new();
    blocked.insert((viewer_account, subject_account), true);
    let journal = Arc::new(ScriptedJournal {
        page: JournalPage {
            next: cursor(epoch, 1),
            has_more: false,
            signals: vec![pair_signal(viewer_account, subject_account)],
        },
        reads: AtomicUsize::new(0),
    });
    let consumer = BlockConsumer::new(
        fixture.concert.clone(),
        journal,
        Arc::new(MapAuthority {
            blocked,
            failing: false,
        }),
        fixture.writer(),
        consumer_id,
    )?;
    let report = consumer.poll().await?;
    assert_eq!(report.signals, 1);
    assert_eq!(report.applied, 1);
    assert_eq!(report.unresolved, 0);
    // The profile is gone from discovery.
    assert!(!fixture.discovered(viewer, Category::Music).await?.contains(&subject));
    // And the cursor advanced.
    assert_eq!(fixture.cursor_row(consumer_id).await?, Some((Some(epoch), Some(1))));
    Ok(())
}

/// A pair signal that resolves to "unblocked" restores the profile.
#[tokio::test]
async fn a_pair_signal_resolves_to_an_unblock() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let (viewer, viewer_account) = fixture.projected_profile("unblock-viewer").await?;
    let (subject, subject_account) = fixture.projected_profile("unblock-subject").await?;
    let item = Uuid::new_v4();
    fixture.add_favorite(viewer, item).await?;
    fixture.add_favorite(subject, item).await?;
    // A block already exists.
    sqlx::query(
        "insert into discovery_blocks(blocker_account_id,blocked_account_id) values($1,$2)",
    )
    .bind(viewer_account)
    .bind(subject_account)
    .execute(&fixture.concert)
    .await?;
    assert!(!fixture.discovered(viewer, Category::Music).await?.contains(&subject));

    let epoch = Uuid::new_v4();
    let consumer_id = Uuid::new_v4();
    seed_cursor(&fixture.concert, consumer_id, cursor(epoch, 0)).await?;
    let mut blocked = std::collections::BTreeMap::new();
    blocked.insert((viewer_account, subject_account), false);
    let journal = Arc::new(ScriptedJournal {
        page: JournalPage {
            next: cursor(epoch, 1),
            has_more: false,
            signals: vec![pair_signal(viewer_account, subject_account)],
        },
        reads: AtomicUsize::new(0),
    });
    let consumer = BlockConsumer::new(
        fixture.concert.clone(),
        journal,
        Arc::new(MapAuthority {
            blocked,
            failing: false,
        }),
        fixture.writer(),
        consumer_id,
    )?;
    let report = consumer.poll().await?;
    assert_eq!(report.applied, 1);
    // The profile is back.
    assert!(fixture.discovered(viewer, Category::Music).await?.contains(&subject));
    Ok(())
}

/// A signal that is not a pair-scoped relationship change is skipped, and the
/// cursor still advances past it so it is not re-read forever.
#[tokio::test]
async fn a_non_pair_signal_is_skipped() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let epoch = Uuid::new_v4();
    let consumer_id = Uuid::new_v4();
    seed_cursor(&fixture.concert, consumer_id, cursor(epoch, 0)).await?;
    let user = Uuid::new_v4();
    let journal = Arc::new(ScriptedJournal {
        page: JournalPage {
            next: cursor(epoch, 1),
            has_more: false,
            signals: vec![
                (
                    SignalReason::AuthSessionChanged,
                    SignalScope::User { user },
                ),
                (
                    SignalReason::FriendRelationshipChanged,
                    SignalScope::User { user },
                ),
            ],
        },
        reads: AtomicUsize::new(0),
    });
    let consumer = BlockConsumer::new(
        fixture.concert.clone(),
        journal,
        Arc::new(MapAuthority {
            blocked: Default::default(),
            failing: false,
        }),
        fixture.writer(),
        consumer_id,
    )?;
    let report = consumer.poll().await?;
    assert_eq!(report.signals, 2);
    assert_eq!(report.skipped, 2);
    assert_eq!(report.applied, 0);
    // The cursor advanced past them.
    assert_eq!(fixture.cursor_row(consumer_id).await?, Some((Some(epoch), Some(1))));
    Ok(())
}

// ---------------------------------------------------------------------------
// Failure behaviour
// ---------------------------------------------------------------------------

/// An authority that cannot answer must not be read as "unblocked": the cursor
/// stops before that signal and the failure is reported.
#[tokio::test]
async fn an_unresolvable_pair_stops_the_cursor() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let epoch = Uuid::new_v4();
    let consumer_id = Uuid::new_v4();
    seed_cursor(&fixture.concert, consumer_id, cursor(epoch, 5)).await?;
    let journal = Arc::new(ScriptedJournal {
        page: JournalPage {
            next: cursor(epoch, 6),
            has_more: false,
            signals: vec![pair_signal(Uuid::new_v4(), Uuid::new_v4())],
        },
        reads: AtomicUsize::new(0),
    });
    let consumer = BlockConsumer::new(
        fixture.concert.clone(),
        journal,
        Arc::new(MapAuthority {
            blocked: Default::default(),
            failing: true,
        }),
        fixture.writer(),
        consumer_id,
    )?;
    let report = consumer.poll().await?;
    assert_eq!(report.unresolved, 1);
    assert_eq!(report.applied, 0);
    assert!(!report.caught_up);
    // The cursor did NOT advance: the signal will be retried.
    assert_eq!(fixture.cursor_row(consumer_id).await?, Some((Some(epoch), Some(5))));
    Ok(())
}

/// A consumer that has never been seeded refuses to poll rather than replaying
/// a retained journal from its beginning.
#[tokio::test]
async fn an_unseeded_consumer_refuses_to_poll() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let consumer_id = Uuid::new_v4();
    let journal = Arc::new(ScriptedJournal {
        page: JournalPage {
            next: cursor(Uuid::new_v4(), 0),
            has_more: false,
            signals: Vec::new(),
        },
        reads: AtomicUsize::new(0),
    });
    let consumer = BlockConsumer::new(
        fixture.concert.clone(),
        journal,
        Arc::new(MapAuthority {
            blocked: Default::default(),
            failing: false,
        }),
        fixture.writer(),
        consumer_id,
    )?;
    assert_eq!(consumer.poll().await, Err(ProjectionError::SourceInvalid));
    Ok(())
}

/// A failure is recorded so it is visible, and the cursor is unchanged.
#[tokio::test]
async fn a_failure_is_recorded_and_the_cursor_is_unchanged() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let epoch = Uuid::new_v4();
    let consumer_id = Uuid::new_v4();
    seed_cursor(&fixture.concert, consumer_id, cursor(epoch, 3)).await?;
    let consumer = BlockConsumer::new(
        fixture.concert.clone(),
        Arc::new(ScriptedJournal {
            page: JournalPage {
                next: cursor(epoch, 4),
                has_more: false,
                signals: Vec::new(),
            },
            reads: AtomicUsize::new(0),
        }),
        Arc::new(MapAuthority {
            blocked: Default::default(),
            failing: false,
        }),
        fixture.writer(),
        consumer_id,
    )?;
    consumer.record_failure("block_journal").await?;
    let failure: Option<String> = sqlx::query_scalar(
        "select last_failure from discovery_block_cursor where consumer_id=$1",
    )
    .bind(consumer_id)
    .fetch_one(&fixture.concert)
    .await?;
    assert_eq!(failure, Some("block_journal".to_owned()));
    // The cursor did not move.
    assert_eq!(fixture.cursor_row(consumer_id).await?, Some((Some(epoch), Some(3))));
    Ok(())
}

// ---------------------------------------------------------------------------
// Cursor discipline
// ---------------------------------------------------------------------------

/// Re-applying the same page is idempotent: the projection ends at the same
/// state and no duplicate block rows appear.
#[tokio::test]
async fn replaying_a_page_is_idempotent() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let epoch = Uuid::new_v4();
    let consumer_id = Uuid::new_v4();
    seed_cursor(&fixture.concert, consumer_id, cursor(epoch, 0)).await?;
    let actor = Uuid::new_v4();
    let subject = Uuid::new_v4();
    let mut blocked = std::collections::BTreeMap::new();
    blocked.insert((actor, subject), true);
    for round in 0..2 {
        // Replay the same page twice. The second replay uses a NEW epoch, which
        // is the only way a cursor may legitimately restart (the journal was
        // recreated); rewinding within an epoch is refused by the schema, as
        // `the_cursor_cannot_regress` asserts.
        if round == 1 {
            sqlx::query(
                "update discovery_block_cursor set epoch=$1, sequence=0 where consumer_id=$2",
            )
            .bind(Uuid::new_v4())
            .bind(consumer_id)
            .execute(&fixture.concert)
            .await?;
        }
        let consumer = BlockConsumer::new(
            fixture.concert.clone(),
            Arc::new(ScriptedJournal {
                page: JournalPage {
                    next: cursor(epoch, 1),
                    has_more: false,
                    signals: vec![pair_signal(actor, subject)],
                },
                reads: AtomicUsize::new(0),
            }),
            Arc::new(MapAuthority {
                blocked: blocked.clone(),
                failing: false,
            }),
            fixture.writer(),
            consumer_id,
        )?;
        consumer.poll().await?;
    }
    let rows: i64 = sqlx::query_scalar(
        "select count(*) from discovery_blocks where blocker_account_id=$1 and blocked_account_id=$2",
    )
    .bind(actor)
    .bind(subject)
    .fetch_one(&fixture.concert)
    .await?;
    assert_eq!(rows, 1, "replaying must not duplicate the block row");
    Ok(())
}

/// The cursor never regresses within an epoch: the database refuses it.
#[tokio::test]
async fn the_cursor_cannot_regress() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let epoch = Uuid::new_v4();
    let consumer_id = Uuid::new_v4();
    seed_cursor(&fixture.concert, consumer_id, cursor(epoch, 5)).await?;
    let error = sqlx::query("update discovery_block_cursor set sequence=3 where consumer_id=$1")
        .bind(consumer_id)
        .execute(&fixture.concert)
        .await
        .expect_err("a regressing cursor must be refused");
    match &error {
        sqlx::Error::Database(database) => assert_eq!(
            database.constraint(),
            Some("discovery_block_cursor_regress"),
            "{error:?}"
        ),
        other => panic!("unexpected error {other:?}"),
    }
    Ok(())
}

/// A new epoch may reset the sequence: the journal was recreated, so the
/// position is a different stream.
#[tokio::test]
async fn a_new_epoch_may_reset_the_sequence() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let consumer_id = Uuid::new_v4();
    seed_cursor(&fixture.concert, consumer_id, cursor(Uuid::new_v4(), 9)).await?;
    let new_epoch = Uuid::new_v4();
    sqlx::query("update discovery_block_cursor set epoch=$1, sequence=0 where consumer_id=$2")
        .bind(new_epoch)
        .bind(consumer_id)
        .execute(&fixture.concert)
        .await?;
    assert_eq!(
        fixture.cursor_row(consumer_id).await?,
        Some((Some(new_epoch), Some(0)))
    );
    Ok(())
}

/// Seeding is idempotent: a second seed does not move an existing cursor.
#[tokio::test]
async fn seeding_is_idempotent() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let consumer_id = Uuid::new_v4();
    let epoch = Uuid::new_v4();
    seed_cursor(&fixture.concert, consumer_id, cursor(epoch, 4)).await?;
    seed_cursor(&fixture.concert, consumer_id, cursor(Uuid::new_v4(), 0)).await?;
    assert_eq!(fixture.cursor_row(consumer_id).await?, Some((Some(epoch), Some(4))));
    Ok(())
}

/// A nil consumer id is refused at construction.
#[tokio::test]
async fn a_nil_consumer_is_refused() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    assert_eq!(
        BlockConsumer::new(
            fixture.concert.clone(),
            Arc::new(ScriptedJournal {
                page: JournalPage {
                    next: cursor(Uuid::new_v4(), 0),
                    has_more: false,
                    signals: Vec::new(),
                },
                reads: AtomicUsize::new(0),
            }),
            Arc::new(MapAuthority {
                blocked: Default::default(),
                failing: false,
            }),
            fixture.writer(),
            Uuid::nil(),
        )
        .err(),
        Some(ProjectionError::SourceInvalid)
    );
    Ok(())
}
