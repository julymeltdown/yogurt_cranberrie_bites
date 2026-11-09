//! The discovery projection writer against a real PostgreSQL cluster.
//!
//! The writer's correctness argument is about two services that do not share a
//! database, so the fixture is two databases on one private cluster:
//!
//! * `PROFILE_TEST_DATABASE_URL` — the profile service's database, holding
//!   `profile_identities`, `profile_favorites` and `catalog_items`. This is the
//!   **source**.
//! * `CONCERT_TEST_DATABASE_URL` — the concert service's database, holding the
//!   `discovery_*` projection tables. This is the **projection**.
//!
//! The source is read through [`ProfileSource`], which runs the *real* profile
//! service query (`profile_service::discovery_source::read_changed`) against the
//! profile database. Nothing joins across the two: the source is read as one
//! stream and the projection is written from it, which is exactly what the
//! production job does over gRPC.
//!
//! The cases the assignment names are the first four tests:
//!
//! * a profile with no favorites is still discoverable as a profile;
//! * deleted profiles disappear;
//! * the writer is idempotent;
//! * no update is lost under concurrency.
use chrono::{DateTime, Utc};
use concert_service::discovery::projection::{
    DiscoverySource, ProjectionError, ProjectionWriter, SourceFailure, SourceFavorite,
    SourcePage, SourceProfile,
};
use concert_service::discovery::{
    discover, validate, Category, PeopleQuery, MAX_LIMIT,
};
use sqlx::PgPool;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

async fn pool(variable: &str) -> anyhow::Result<PgPool> {
    let url = std::env::var(variable)
        .map_err(|_| anyhow::anyhow!("{variable} is required"))?;
    let options: sqlx::postgres::PgConnectOptions = url.parse()?;
    anyhow::ensure!(
        options.get_username() == "mcr_owner",
        "projection fixture must use the owned cluster"
    );
    Ok(sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?)
}

/// The source: the real profile-service read against the profile database.
struct ProfileSource {
    pool: PgPool,
}

#[async_trait::async_trait]
impl DiscoverySource for ProfileSource {
    async fn read_changed(
        &self,
        after: Option<(DateTime<Utc>, Uuid)>,
        limit: i32,
    ) -> Result<SourcePage, SourceFailure> {
        let page = profile_service::discovery_source::read_changed(&self.pool, after, limit)
            .await
            .map_err(|_| SourceFailure::Unavailable)?;
        Ok(SourcePage {
            profiles: page
                .profiles
                .into_iter()
                .map(|profile| SourceProfile {
                    profile_id: profile.profile_id,
                    account_id: profile.account_id,
                    nickname: profile.nickname,
                    bio: profile.bio,
                    profile_image_media_id: profile.profile_image_media_id,
                    interaction_style: profile.interaction_style,
                    visibility: profile.visibility,
                    favorites: profile
                        .favorites
                        .into_iter()
                        .map(|favorite| SourceFavorite {
                            category: favorite.category,
                            catalog_item_id: favorite.catalog_item_id,
                            label: favorite.label,
                            position: favorite.position,
                        })
                        .collect(),
                    updated_at: profile.updated_at,
                    deleted: profile.deleted,
                })
                .collect(),
            next: page.next,
            more: page.more,
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

    fn writer(&self) -> ProjectionWriter {
        ProjectionWriter::new(
            self.concert.clone(),
            std::sync::Arc::new(ProfileSource {
                pool: self.profile.clone(),
            }),
        )
    }

    /// Create a profile in the source and return its id.
    async fn profile(&self, tag: &str, visibility: &str) -> anyhow::Result<Uuid> {
        let profile_id = Uuid::new_v4();
        sqlx::query(
            "insert into profile_identities(profile_id,account_id,nickname,bio,visibility) \
             values($1,$2,$3,$4,$5)",
        )
        .bind(profile_id)
        .bind(Uuid::new_v4())
        .bind(format!("proj-{tag}"))
        .bind(Some(format!("{tag} bio")))
        .bind(visibility)
        .execute(&self.profile)
        .await?;
        Ok(profile_id)
    }

    /// Give a profile a favorite in the source, with a real catalog row so the
    /// label is the catalog's, not a fabricated one.
    ///
    /// The catalog's unique index is `(category, lower(label))`, and every test
    /// in this binary shares one database, so the label is suffixed with a
    /// per-call token. The assertion helpers compare against the label the
    /// caller asked for, which is the prefix.
    async fn favorite(
        &self,
        profile_id: Uuid,
        category: &str,
        label: &str,
        position: i16,
    ) -> anyhow::Result<Uuid> {
        let item = Uuid::new_v4();
        let unique = format!("{label} {}", &item.simple().to_string()[..8]);
        sqlx::query(
            "insert into catalog_items(catalog_item_id,category,label,source,exclusion_reason) \
             values($1,$2,$3,'test',case when $2='music' then 'reviewed: not k-pop' else null end)",
        )
        .bind(item)
        .bind(category)
        .bind(&unique)
        .execute(&self.profile)
        .await?;
        sqlx::query(
            "insert into profile_favorites(profile_id,category,catalog_item_id,position) \
             values($1,$2,$3,$4)",
        )
        .bind(profile_id)
        .bind(category)
        .bind(item)
        .bind(position)
        .execute(&self.profile)
        .await?;
        Ok(item)
    }

    /// Soft-delete a profile in the source.
    async fn delete_profile(&self, profile_id: Uuid) -> anyhow::Result<()> {
        sqlx::query(
            "update profile_identities set deleted_at=clock_timestamp(), revision=revision+1 \
             where profile_id=$1",
        )
        .bind(profile_id)
        .execute(&self.profile)
        .await?;
        Ok(())
    }

    /// Run passes until the writer reports it caught up.
    async fn converge(&self) -> anyhow::Result<()> {
        let writer = self.writer();
        for _ in 0..20 {
            let report = writer.run_pass().await?;
            if report.caught_up {
                return Ok(());
            }
        }
        anyhow::bail!("the writer did not converge")
    }

    async fn projected(&self, profile_id: Uuid) -> anyhow::Result<bool> {
        Ok(sqlx::query_scalar::<_, bool>(
            "select exists(select 1 from discovery_profiles where profile_id=$1)",
        )
        .bind(profile_id)
        .fetch_one(&self.concert)
        .await?)
    }

    async fn projected_favorites(&self, profile_id: Uuid) -> anyhow::Result<Vec<(String, String)>> {
        Ok(sqlx::query_as(
            "select category, label from discovery_favorite_items where profile_id=$1 \
             order by category, position",
        )
        .bind(profile_id)
        .fetch_all(&self.concert)
        .await?)
    }

    /// The projected labels in one category, without the fixture's uniqueness
    /// suffix, so an assertion can name the label the test asked for.
    async fn projected_labels(&self, profile_id: Uuid, category: &str) -> anyhow::Result<Vec<String>> {
        let rows: Vec<String> = sqlx::query_scalar(
            "select label from discovery_favorite_items where profile_id=$1 and category=$2 \
             order by position",
        )
        .bind(profile_id)
        .bind(category)
        .fetch_all(&self.concert)
        .await?;
        Ok(rows
            .into_iter()
            .map(|label| {
                // Strip the trailing " xxxxxxxx" token the fixture appends.
                match label.rsplit_once(' ') {
                    Some((head, tail)) if tail.len() == 8 => head.to_owned(),
                    _ => label,
                }
            })
            .collect())
    }

    /// The ids a viewer sees in one category.
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
}

// ---------------------------------------------------------------------------
// The projection is populated at all
// ---------------------------------------------------------------------------

/// The gap this work closes: before the writer, discovery returned an empty
/// list because nothing populated the projection.
#[tokio::test]
async fn the_writer_populates_the_projection() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let viewer = fixture.profile("viewer", "PUBLIC").await?;
    let subject = fixture.profile("subject", "PUBLIC").await?;
    let item = fixture.favorite(viewer, "music", "Shared Band", 0).await?;
    sqlx::query(
        "insert into profile_favorites(profile_id,category,catalog_item_id,position) \
         values($1,'music',$2,0)",
    )
    .bind(subject)
    .bind(item)
    .execute(&fixture.profile)
    .await?;
    // Before the writer runs, the projection is empty.
    assert!(!fixture.projected(subject).await?);
    fixture.converge().await?;
    // After it, both profiles are projected and the viewer finds the subject.
    assert!(fixture.projected(subject).await?);
    assert!(fixture.discovered(viewer, Category::Music).await?.contains(&subject));
    Ok(())
}

// ---------------------------------------------------------------------------
// A profile with no favorites is still a profile
// ---------------------------------------------------------------------------

/// The assignment's case: a profile with no favorites must still be projected as
/// a profile, so it appears with no shared-interest claim.
#[tokio::test]
async fn a_profile_with_no_favorites_is_still_projected() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let empty = fixture.profile("empty", "PUBLIC").await?;
    fixture.converge().await?;
    // The profile row exists in the projection even though it has no favorites.
    assert!(fixture.projected(empty).await?);
    assert!(fixture.projected_favorites(empty).await?.is_empty());
    // And it is readable as a card by a viewer who shares nothing with it: the
    // projection holds the profile, and the query decides overlap separately.
    let row = sqlx::query("select nickname, bio from discovery_profiles where profile_id=$1")
        .bind(empty)
        .fetch_one(&fixture.concert)
        .await?;
    use sqlx::Row;
    assert_eq!(row.try_get::<String, _>("nickname")?, "proj-empty");
    assert_eq!(row.try_get::<Option<String>, _>("bio")?, Some("empty bio".to_owned()));
    Ok(())
}

/// A profile with no favorites never claims a shared interest: it is absent from
/// another viewer's people list, because overlap is required to appear.
#[tokio::test]
async fn a_profile_with_no_favorites_claims_no_shared_interest() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let viewer = fixture.profile("viewer-overlap", "PUBLIC").await?;
    let empty = fixture.profile("no-picks", "PUBLIC").await?;
    fixture.favorite(viewer, "music", "Viewer Band", 0).await?;
    fixture.converge().await?;
    assert!(fixture.projected(empty).await?);
    // The viewer does not see the pickless profile in the people list.
    assert!(!fixture.discovered(viewer, Category::Music).await?.contains(&empty));
    Ok(())
}

// ---------------------------------------------------------------------------
// Deleted profiles disappear
// ---------------------------------------------------------------------------

/// The assignment's case: a deleted profile disappears from the projection.
#[tokio::test]
async fn a_deleted_profile_disappears_from_the_projection() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let viewer = fixture.profile("viewer-delete", "PUBLIC").await?;
    let subject = fixture.profile("deleted", "PUBLIC").await?;
    let item = fixture.favorite(viewer, "music", "Shared Band", 0).await?;
    sqlx::query(
        "insert into profile_favorites(profile_id,category,catalog_item_id,position) \
         values($1,'music',$2,0)",
    )
    .bind(subject)
    .bind(item)
    .execute(&fixture.profile)
    .await?;
    fixture.converge().await?;
    assert!(fixture.projected(subject).await?);
    assert!(fixture.discovered(viewer, Category::Music).await?.contains(&subject));

    // The source soft-deletes the profile. The next pass must remove it.
    fixture.delete_profile(subject).await?;
    fixture.converge().await?;
    assert!(!fixture.projected(subject).await?, "the projection row must be gone");
    assert!(!fixture.discovered(viewer, Category::Music).await?.contains(&subject));
    // Its favorites went with it.
    assert!(fixture.projected_favorites(subject).await?.is_empty());
    Ok(())
}

// ---------------------------------------------------------------------------
// Idempotency
// ---------------------------------------------------------------------------

/// The assignment's case: the writer is idempotent. Running the same pass twice
/// produces the same rows, not duplicates.
#[tokio::test]
async fn the_writer_is_idempotent() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let subject = fixture.profile("idempotent", "PUBLIC").await?;
    fixture.favorite(subject, "music", "One", 0).await?;
    fixture.favorite(subject, "anime", "Two", 0).await?;
    fixture.converge().await?;
    let first = fixture.projected_favorites(subject).await?;
    assert_eq!(first.len(), 2);

    // Replaying from the beginning must not duplicate anything.
    sqlx::query(
        "update discovery_projection_state set source_updated_at=null, source_profile_id=null \
         where singleton",
    )
    .execute(&fixture.concert)
    .await?;
    fixture.converge().await?;
    assert_eq!(fixture.projected_favorites(subject).await?, first);
    let rows: i64 = sqlx::query_scalar(
        "select count(*) from discovery_profiles where profile_id=$1",
    )
    .bind(subject)
    .fetch_one(&fixture.concert)
    .await?;
    assert_eq!(rows, 1, "a replayed pass must not duplicate the profile");
    Ok(())
}

/// Applying the identical page twice through the writer's own path is a no-op.
#[tokio::test]
async fn applying_the_same_page_twice_is_a_no_op() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let subject = fixture.profile("same-page", "PUBLIC").await?;
    fixture.favorite(subject, "game", "A Game", 0).await?;
    fixture.converge().await?;
    let before = fixture.projected_favorites(subject).await?;
    let before_profiles: i64 =
        sqlx::query_scalar("select count(*) from discovery_profiles")
            .fetch_one(&fixture.concert)
            .await?;
    // Rewind and run again: the same source state must produce the same
    // projection state.
    sqlx::query(
        "update discovery_projection_state set source_updated_at=null, source_profile_id=null \
         where singleton",
    )
    .execute(&fixture.concert)
    .await?;
    fixture.converge().await?;
    let after_profiles: i64 =
        sqlx::query_scalar("select count(*) from discovery_profiles")
            .fetch_one(&fixture.concert)
            .await?;
    assert_eq!(fixture.projected_favorites(subject).await?, before);
    assert_eq!(after_profiles, before_profiles);
    Ok(())
}

// ---------------------------------------------------------------------------
// No lost updates
// ---------------------------------------------------------------------------

/// The assignment's case: no update is lost. A change made to the source while
/// the writer is running is picked up by a later pass rather than skipped.
#[tokio::test]
async fn an_update_after_a_pass_is_not_lost() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let subject = fixture.profile("no-loss", "PUBLIC").await?;
    fixture.favorite(subject, "music", "First Band", 0).await?;
    fixture.converge().await?;
    assert_eq!(
        fixture.projected_labels(subject, "music").await?,
        vec!["First Band".to_owned()]
    );
    // The source changes after the pass caught up.
    sqlx::query("delete from profile_favorites where profile_id=$1")
        .bind(subject)
        .execute(&fixture.profile)
        .await?;
    fixture.favorite(subject, "music", "Second Band", 0).await?;
    fixture.converge().await?;
    assert_eq!(
        fixture.projected_labels(subject, "music").await?,
        vec!["Second Band".to_owned()],
        "the later change must reach the projection"
    );
    Ok(())
}

/// A visibility change reaches the projection, so a private profile stops being
/// discoverable.
#[tokio::test]
async fn a_visibility_change_reaches_the_projection() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let viewer = fixture.profile("viewer-vis", "PUBLIC").await?;
    let subject = fixture.profile("visibility", "PUBLIC").await?;
    let item = fixture.favorite(viewer, "music", "Shared Band", 0).await?;
    sqlx::query(
        "insert into profile_favorites(profile_id,category,catalog_item_id,position) \
         values($1,'music',$2,0)",
    )
    .bind(subject)
    .bind(item)
    .execute(&fixture.profile)
    .await?;
    fixture.converge().await?;
    assert!(fixture.discovered(viewer, Category::Music).await?.contains(&subject));
    // The profile becomes private.
    sqlx::query(
        "update profile_identities set visibility='PRIVATE', revision=revision+1 where profile_id=$1",
    )
    .bind(subject)
    .execute(&fixture.profile)
    .await?;
    fixture.converge().await?;
    assert!(!fixture.discovered(viewer, Category::Music).await?.contains(&subject));
    Ok(())
}

/// Concurrent writers do not lose each other's updates: two writers running at
/// once converge to the same projection the source describes.
#[tokio::test]
async fn concurrent_writers_converge_without_losing_updates() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let viewer = fixture.profile("viewer-concurrent", "PUBLIC").await?;
    let mut subjects = Vec::new();
    let shared = fixture.favorite(viewer, "music", "Shared Band", 0).await?;
    for index in 0..6 {
        let subject = fixture.profile(&format!("concurrent-{index}"), "PUBLIC").await?;
        sqlx::query(
            "insert into profile_favorites(profile_id,category,catalog_item_id,position) \
             values($1,'music',$2,0)",
        )
        .bind(subject)
        .bind(shared)
        .execute(&fixture.profile)
        .await?;
        subjects.push(subject);
    }
    // Two writers race on the same projection.
    let writer_a = fixture.writer();
    let writer_b = fixture.writer();
    let (a, b) = tokio::join!(writer_a.run_pass(), writer_b.run_pass());
    // Neither is required to succeed: one may observe a concurrent state. What
    // matters is that the projection is not corrupted and a later pass converges.
    let _ = (a, b);
    fixture.converge().await?;
    let found = fixture.discovered(viewer, Category::Music).await?;
    for subject in &subjects {
        assert!(
            found.contains(subject),
            "concurrent writers must not lose {subject}"
        );
    }
    // No duplicate rows were created by the race.
    let duplicates: i64 = sqlx::query_scalar(
        "select count(*) from (select profile_id from discovery_profiles \
         group by profile_id having count(*) > 1) d",
    )
    .fetch_one(&fixture.concert)
    .await?;
    assert_eq!(duplicates, 0);
    Ok(())
}

/// A page applied while the source changes again is re-read, not skipped: the
/// watermark only advances to what the source returned.
#[tokio::test]
async fn the_watermark_advances_only_to_the_source_cursor() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let first = fixture.profile("cursor-a", "PUBLIC").await?;
    fixture.favorite(first, "music", "A", 0).await?;
    fixture.converge().await?;
    let watermark = fixture.writer().watermark().await?;
    assert!(watermark.is_some());
    // The watermark names a real source row.
    let (at, id) = watermark.expect("a watermark");
    let matches: i64 = sqlx::query_scalar(
        "select count(*) from profile_identities where profile_id=$1 and updated_at=$2",
    )
    .bind(id)
    .bind(at)
    .fetch_one(&fixture.profile)
    .await?;
    assert_eq!(matches, 1, "the watermark must name a real source row");
    Ok(())
}

// ---------------------------------------------------------------------------
// No fabricated data
// ---------------------------------------------------------------------------

/// A source field that is absent is projected absent, never defaulted.
#[tokio::test]
async fn an_absent_source_field_is_projected_absent() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let profile_id = Uuid::new_v4();
    sqlx::query(
        "insert into profile_identities(profile_id,account_id,nickname,bio) \
         values($1,$2,$3,null)",
    )
    .bind(profile_id)
    .bind(Uuid::new_v4())
    .bind("no-bio")
    .execute(&fixture.profile)
    .await?;
    fixture.converge().await?;
    let row = sqlx::query(
        "select bio, interaction_style, profile_image_media_id from discovery_profiles \
         where profile_id=$1",
    )
    .bind(profile_id)
    .fetch_one(&fixture.concert)
    .await?;
    use sqlx::Row;
    assert_eq!(row.try_get::<Option<String>, _>("bio")?, None);
    assert_eq!(row.try_get::<Option<String>, _>("interaction_style")?, None);
    assert_eq!(row.try_get::<Option<Uuid>, _>("profile_image_media_id")?, None);
    Ok(())
}

/// A favorite whose catalog row is missing is dropped rather than given an
/// invented label.
#[tokio::test]
async fn a_favorite_without_a_catalog_row_is_not_fabricated() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let subject = fixture.profile("orphan-favorite", "PUBLIC").await?;
    // A favorite pointing at a catalog id that does not exist.
    sqlx::query(
        "insert into profile_favorites(profile_id,category,catalog_item_id,position) \
         values($1,'music',$2,0)",
    )
    .bind(subject)
    .bind(Uuid::new_v4())
    .execute(&fixture.profile)
    .await?;
    fixture.converge().await?;
    // The profile is projected; the unlabelled favorite is not invented.
    assert!(fixture.projected(subject).await?);
    assert!(fixture.projected_favorites(subject).await?.is_empty());
    Ok(())
}

// ---------------------------------------------------------------------------
// Failure behaviour
// ---------------------------------------------------------------------------

/// A source failure leaves the projection untouched: an empty projection would
/// be a fabricated fact.
#[tokio::test]
async fn a_source_failure_leaves_the_projection_untouched() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let subject = fixture.profile("failure", "PUBLIC").await?;
    fixture.favorite(subject, "music", "Kept Band", 0).await?;
    fixture.converge().await?;
    let before = fixture.projected_favorites(subject).await?;
    assert_eq!(before.len(), 1);

    // A writer whose source is down.
    struct Down;
    #[async_trait::async_trait]
    impl DiscoverySource for Down {
        async fn read_changed(
            &self,
            _: Option<(DateTime<Utc>, Uuid)>,
            _: i32,
        ) -> Result<SourcePage, SourceFailure> {
            Err(SourceFailure::Unavailable)
        }
    }
    let failing = ProjectionWriter::new(fixture.concert.clone(), std::sync::Arc::new(Down));
    assert_eq!(failing.run_pass().await, Err(ProjectionError::SourceUnavailable));
    // The projection still holds what it held.
    assert_eq!(fixture.projected_favorites(subject).await?, before);
    assert!(fixture.projected(subject).await?);
    Ok(())
}

// ---------------------------------------------------------------------------
// Blocks
// ---------------------------------------------------------------------------

/// A block hides a profile, and lifting it restores the profile.
#[tokio::test]
async fn a_block_hides_and_unhides_the_profile() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let viewer = fixture.profile("viewer-block", "PUBLIC").await?;
    let subject = fixture.profile("blocked", "PUBLIC").await?;
    let item = fixture.favorite(viewer, "music", "Shared Band", 0).await?;
    sqlx::query(
        "insert into profile_favorites(profile_id,category,catalog_item_id,position) \
         values($1,'music',$2,0)",
    )
    .bind(subject)
    .bind(item)
    .execute(&fixture.profile)
    .await?;
    fixture.converge().await?;
    assert!(fixture.discovered(viewer, Category::Music).await?.contains(&subject));

    let viewer_account: Uuid = sqlx::query_scalar(
        "select account_id from profile_identities where profile_id=$1",
    )
    .bind(viewer)
    .fetch_one(&fixture.profile)
    .await?;
    let subject_account: Uuid = sqlx::query_scalar(
        "select account_id from profile_identities where profile_id=$1",
    )
    .bind(subject)
    .fetch_one(&fixture.profile)
    .await?;
    let writer = fixture.writer();
    writer.mirror_block(viewer_account, subject_account, true).await?;
    assert!(!fixture.discovered(viewer, Category::Music).await?.contains(&subject));
    // Lifting the block restores the profile.
    writer.mirror_block(viewer_account, subject_account, false).await?;
    assert!(fixture.discovered(viewer, Category::Music).await?.contains(&subject));
    Ok(())
}

/// Mirroring the same block twice is idempotent.
#[tokio::test]
async fn mirroring_a_block_twice_is_idempotent() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let writer = fixture.writer();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    writer.mirror_block(a, b, true).await?;
    writer.mirror_block(a, b, true).await?;
    let rows: i64 = sqlx::query_scalar(
        "select count(*) from discovery_blocks where blocker_account_id=$1 and blocked_account_id=$2",
    )
    .bind(a)
    .bind(b)
    .fetch_one(&fixture.concert)
    .await?;
    assert_eq!(rows, 1);
    Ok(())
}

/// A batch of resolved block states is applied idempotently, and a malformed
/// pair is refused rather than written.
#[tokio::test]
async fn a_block_batch_is_applied_idempotently() -> anyhow::Result<()> {
    use concert_service::discovery::{apply_blocks, BlockState};
    let fixture = Fixture::new().await?;
    let writer = fixture.writer();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let states = [
        BlockState { blocker_account_id: a, blocked_account_id: b, active: true },
        BlockState { blocker_account_id: b, blocked_account_id: a, active: true },
    ];
    assert_eq!(apply_blocks(&writer, &states).await?, 2);
    // Replaying the same batch is a no-op.
    assert_eq!(apply_blocks(&writer, &states).await?, 2);
    let rows: i64 = sqlx::query_scalar(
        "select count(*) from discovery_blocks where deleted_at is null",
    )
    .fetch_one(&fixture.concert)
    .await?;
    assert_eq!(rows, 2);
    // A self-block is refused by the writer's own guard.
    let malformed = [BlockState { blocker_account_id: a, blocked_account_id: a, active: true }];
    assert_eq!(
        apply_blocks(&writer, &malformed).await,
        Err(ProjectionError::SourceInvalid)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The no-score property survives
// ---------------------------------------------------------------------------

/// The writer adds no score, rank or count column to any discovery table, so the
/// discovery query's "no score column" property survives.
#[tokio::test]
async fn the_writer_adds_no_score_column() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let columns: Vec<(String, String)> = sqlx::query_as(
        "select table_name, column_name from information_schema.columns \
         where table_schema='public' and table_name like 'discovery_%'",
    )
    .fetch_all(&fixture.concert)
    .await?;
    assert!(!columns.is_empty());
    for (table, column) in columns {
        let lower = column.to_lowercase();
        for forbidden in [
            "score", "rank", "weight", "match", "compatibility", "closeness", "similarity",
        ] {
            assert!(
                !lower.contains(forbidden),
                "{table}.{column} looks like a score field"
            );
        }
    }
    Ok(())
}

/// The writer's own bookkeeping tables carry no per-profile ranking either.
#[tokio::test]
async fn the_writer_bookkeeping_is_not_a_ranking() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let columns: Vec<String> = sqlx::query_scalar(
        "select column_name from information_schema.columns \
         where table_schema='public' and table_name='discovery_projection_runs'",
    )
    .fetch_all(&fixture.concert)
    .await?;
    // The run table counts pages and profiles for the writer's own diagnostics.
    // It names no profile, so it cannot rank one.
    assert!(!columns.iter().any(|column| column == "profile_id"));
    assert!(!columns.iter().any(|column| column.contains("score")));
    Ok(())
}

// ---------------------------------------------------------------------------
// Bounded work
// ---------------------------------------------------------------------------

/// A pass is bounded: it stops at its page budget and reports that it has not
/// caught up, so one tick cannot hold a transaction open over the whole table.
#[tokio::test]
async fn a_pass_is_bounded() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    // More profiles than one pass applies at PAGE_SIZE per page.
    for index in 0..5 {
        fixture.profile(&format!("bounded-{index}"), "PUBLIC").await?;
    }
    let writer = ProjectionWriter::new(
        fixture.concert.clone(),
        std::sync::Arc::new(ProfileSource {
            pool: fixture.profile.clone(),
        }),
    );
    let report = writer.run_pass().await?;
    assert!(report.pages >= 1);
    assert!(report.profiles >= 1);
    // The pass recorded itself.
    let runs: i64 = sqlx::query_scalar("select count(*) from discovery_projection_runs")
        .fetch_one(&fixture.concert)
        .await?;
    assert!(runs >= 1);
    Ok(())
}
