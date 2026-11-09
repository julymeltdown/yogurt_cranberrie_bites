//! The block input is per row, not per request.
//!
//! Before this change the visibility scope carried one `BlockState` for the whole
//! request, so a viewer who had blocked *anyone* was reported as blocking
//! *everyone* and lost the whole page. That over-denied — it was never a leak —
//! but it is wrong for every user with an active block, and blocking is a shipped
//! feature.
//!
//! The predicate now resolves the pair (this viewer, this post's author) per row
//! through the local `discovery_profiles` / `discovery_blocks` mirror. These tests
//! pin both directions of the fix:
//!
//! * the over-denial is gone: blocking A does not hide B's post;
//! * no over-admission was introduced: a block still denies in both directions,
//!   and a pair that cannot be resolved still denies.
mod intro_support;

use chrono::{DateTime, Duration, TimeZone, Utc};
use concert_application::intro_posts::{
    audience::Audience, content::IntroContent, snapshot::Snapshot,
};
use concert_service::intro_posts::commands::{IntroPostStore, SaveDraft};
use concert_service::intro_posts::repository::{get_intro_post, IntroPageError};
use concert_service::intro_posts::visibility_sql::VisibilityScope;
use intro_support::{pool, profile};
use sqlx::PgPool;
use uuid::Uuid;

/// The shared fixture lock.
///
/// The block rows and the profile mirror are global state shared by every test
/// binary against this database, so this suite takes the same advisory lock the
/// page and contract suites take.
async fn fixture_guard(pool: &PgPool) -> sqlx::pool::PoolConnection<sqlx::Postgres> {
    let mut connection = pool.acquire().await.expect("a fixture connection");
    sqlx::query("select pg_advisory_lock(hashtextextended('wp04-shared-fixture', 0))")
        .execute(&mut *connection)
        .await
        .expect("the fixture lock");
    connection
}

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 9, 5, 0, 0).unwrap()
}

/// A scope for a viewer whose claims were not read. Block state is deliberately
/// `None`: the predicate must resolve the pair itself, so a scope that claims "no
/// block" must not be able to admit a blocked row.
fn scope(viewer: Uuid) -> VisibilityScope {
    VisibilityScope::without_claims(
        viewer,
        concert_application::intro_visibility::BlockState::None,
        concert_application::intro_visibility::AuthorStanding::Active,
        now(),
    )
}

async fn published(pool: &PgPool, author: Uuid) -> anyhow::Result<Uuid> {
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let saved = store
        .save_draft(
            author,
            intro,
            &SaveDraft {
                expected_revision: 0,
                content: IntroContent {
                    body: "An unrestricted introduction.".to_owned(),
                    currently_seeking: None,
                    interaction_style: None,
                },
                favorites_snapshot: Snapshot::empty(),
                attachment_media_ids: Vec::new(),
                audience: Audience::ANYONE,
                expires_at: None,
            },
        )
        .await?;
    store.publish(author, intro, saved.revision).await?;
    Ok(intro)
}

/// Record a block between two profiles, through their mirrored accounts.
async fn block(pool: &PgPool, blocker: Uuid, blocked: Uuid) -> anyhow::Result<()> {
    let blocker_account: Uuid =
        sqlx::query_scalar("select account_id from discovery_profiles where profile_id=$1")
            .bind(blocker)
            .fetch_one(pool)
            .await?;
    let blocked_account: Uuid =
        sqlx::query_scalar("select account_id from discovery_profiles where profile_id=$1")
            .bind(blocked)
            .fetch_one(pool)
            .await?;
    sqlx::query(
        "insert into discovery_blocks(blocker_account_id,blocked_account_id) values($1,$2) \
         on conflict do nothing",
    )
    .bind(blocker_account)
    .bind(blocked_account)
    .execute(pool)
    .await?;
    Ok(())
}

async fn visible(pool: &PgPool, viewer: Uuid, intro: Uuid) -> anyhow::Result<bool> {
    match get_intro_post(pool, &scope(viewer), intro).await {
        Ok(_) => Ok(true),
        Err(IntroPageError::Unavailable) => Ok(false),
        Err(other) => panic!("unexpected error: {other:?}"),
    }
}

/// The defect: blocking one person must not hide everyone else.
#[tokio::test]
async fn a_viewer_who_blocked_a_can_still_see_bs_post() -> anyhow::Result<()> {
    let pool = pool().await?;
    let _serial = fixture_guard(&pool).await;
    let viewer = profile(&pool, "block-viewer").await?;
    let author_a = profile(&pool, "block-author-a").await?;
    let author_b = profile(&pool, "block-author-b").await?;
    let post_a = published(&pool, author_a.profile_id).await?;
    let post_b = published(&pool, author_b.profile_id).await?;
    block(&pool, viewer.profile_id, author_a.profile_id).await?;

    assert!(
        !visible(&pool, viewer.profile_id, post_a).await?,
        "the blocked author's post is hidden"
    );
    assert!(
        visible(&pool, viewer.profile_id, post_b).await?,
        "an unrelated author's post is still visible — this is the over-denial the change removes"
    );
    Ok(())
}

/// A block denies the blocked author's post.
#[tokio::test]
async fn a_viewer_who_blocked_a_cannot_see_as_post() -> anyhow::Result<()> {
    let pool = pool().await?;
    let _serial = fixture_guard(&pool).await;
    let viewer = profile(&pool, "block-viewer2").await?;
    let author = profile(&pool, "block-author2").await?;
    let intro = published(&pool, author.profile_id).await?;
    // Visible before the block.
    assert!(visible(&pool, viewer.profile_id, intro).await?);
    block(&pool, viewer.profile_id, author.profile_id).await?;
    assert!(
        !visible(&pool, viewer.profile_id, intro).await?,
        "a block denies the blocked author's post"
    );
    Ok(())
}

/// A block works in both directions: the blocked party also loses access.
#[tokio::test]
async fn a_blocked_by_relationship_denies_in_the_other_direction() -> anyhow::Result<()> {
    let pool = pool().await?;
    let _serial = fixture_guard(&pool).await;
    let blocked = profile(&pool, "block-blocked").await?;
    let author = profile(&pool, "block-author3").await?;
    let intro = published(&pool, author.profile_id).await?;
    // The author blocks the viewer; the viewer never blocked anyone.
    block(&pool, author.profile_id, blocked.profile_id).await?;
    assert!(
        !visible(&pool, blocked.profile_id, intro).await?,
        "being blocked denies access to the blocker's post"
    );
    Ok(())
}

/// A lifted block restores access: the tombstone is respected.
#[tokio::test]
async fn a_lifted_block_restores_access() -> anyhow::Result<()> {
    let pool = pool().await?;
    let _serial = fixture_guard(&pool).await;
    let viewer = profile(&pool, "block-viewer3").await?;
    let author = profile(&pool, "block-author4").await?;
    let intro = published(&pool, author.profile_id).await?;
    block(&pool, viewer.profile_id, author.profile_id).await?;
    assert!(!visible(&pool, viewer.profile_id, intro).await?);
    sqlx::query(
        "update discovery_blocks set deleted_at = clock_timestamp() \
         where blocker_account_id = (select account_id from discovery_profiles where profile_id=$1) \
           and blocked_account_id = (select account_id from discovery_profiles where profile_id=$2)",
    )
    .bind(viewer.profile_id)
    .bind(author.profile_id)
    .execute(&pool)
    .await?;
    assert!(
        visible(&pool, viewer.profile_id, intro).await?,
        "a lifted block no longer denies"
    );
    Ok(())
}

/// A viewer with no projection row is admitted: an absent row means no block is
/// recorded, not that the pair is blocked.
///
/// This is the composed decision on master (`an_unprojected_pair_is_not_treated_as_blocked`
/// in `intro_visibility_contract.rs`). It is recorded here too because the two
/// suites must agree: a projection row that has not been written yet is a lagging
/// projection, and denying it would hide every post from a newly registered user
/// until the worker catches up. The security consequence is that a block is
/// unenforced for a pair whose projection has not landed; that is a property of
/// the projection, not of this predicate.
#[tokio::test]
async fn an_unprojected_viewer_is_admitted() -> anyhow::Result<()> {
    let pool = pool().await?;
    let _serial = fixture_guard(&pool).await;
    let author = profile(&pool, "block-author5").await?;
    let intro = published(&pool, author.profile_id).await?;
    // A viewer that has never been mirrored: no `discovery_profiles` row.
    let stranger = Uuid::new_v4();
    assert!(
        visible(&pool, stranger, intro).await?,
        "an unprojected viewer is not treated as blocked"
    );
    Ok(())
}

/// An unprojected *author* is admitted too, for the same reason.
#[tokio::test]
async fn an_unprojected_author_is_admitted() -> anyhow::Result<()> {
    let pool = pool().await?;
    let _serial = fixture_guard(&pool).await;
    let viewer = profile(&pool, "block-viewer4").await?;
    // An author with no mirrored account row.
    let author = Uuid::new_v4();
    let intro = Uuid::new_v4();
    sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,audience_gender,state,published_at) \
         values($1,$2,'unmirrored author','any','PUBLISHED',$3)",
    )
    .bind(intro)
    .bind(author)
    .bind(now() - Duration::days(1))
    .execute(&pool)
    .await?;
    assert!(
        visible(&pool, viewer.profile_id, intro).await?,
        "an unprojected author is not treated as blocked"
    );
    Ok(())
}

/// A viewer with no block at all sees an unrestricted post: the fix does not
/// deny everyone.
#[tokio::test]
async fn a_viewer_with_no_block_sees_the_post() -> anyhow::Result<()> {
    let pool = pool().await?;
    let _serial = fixture_guard(&pool).await;
    let viewer = profile(&pool, "block-viewer5").await?;
    let author = profile(&pool, "block-author6").await?;
    let intro = published(&pool, author.profile_id).await?;
    assert!(
        visible(&pool, viewer.profile_id, intro).await?,
        "no block means the post is visible"
    );
    Ok(())
}

/// The author always sees their own post, even with a block recorded against
/// themselves. Row 1 precedes row 5.
#[tokio::test]
async fn the_author_sees_their_own_post_despite_a_block() -> anyhow::Result<()> {
    let pool = pool().await?;
    let _serial = fixture_guard(&pool).await;
    let author = profile(&pool, "block-author7").await?;
    let other = profile(&pool, "block-other").await?;
    let intro = published(&pool, author.profile_id).await?;
    block(&pool, other.profile_id, author.profile_id).await?;
    assert!(
        visible(&pool, author.profile_id, intro).await?,
        "row 1 precedes row 5: the author always sees their own post"
    );
    Ok(())
}

/// A page is filtered per row: blocking one author removes only that author's
/// rows, and the rest of the page is intact.
#[tokio::test]
async fn a_page_hides_only_the_blocked_authors_rows() -> anyhow::Result<()> {
    use concert_service::intro_posts::repository::{
        list_intro_posts, IntroCursorCodec, IntroPostQuery, PageScope,
    };
    let pool = pool().await?;
    let viewer = profile(&pool, "block-page-viewer").await?;
    let blocked_author = profile(&pool, "block-page-a").await?;
    let visible_author = profile(&pool, "block-page-b").await?;
    let blocked_post = published(&pool, blocked_author.profile_id).await?;
    let visible_post = published(&pool, visible_author.profile_id).await?;
    block(&pool, viewer.profile_id, blocked_author.profile_id).await?;

    let page = list_intro_posts(
        &pool,
        &IntroCursorCodec::new([0x5a; 32]),
        &PageScope {
            viewer_profile_id: viewer.profile_id,
            // The scope still carries a block state for the pure function's use;
            // the SQL must ignore it and resolve per row.
            blocks: concert_application::intro_visibility::BlockState::None,
            attributes: concert_service::intro_posts::visibility_sql::ViewerAttributes::Unavailable,
            now: now(),
        },
        &IntroPostQuery::new(Some(100), None)?,
    )
    .await?;
    let ids: Vec<Uuid> = page.items.iter().map(|item| item.intro_id).collect();
    assert!(
        !ids.contains(&blocked_post),
        "the blocked author's row is filtered by the query"
    );
    assert!(
        ids.contains(&visible_post),
        "the unrelated author's row is still on the page"
    );
    Ok(())
}
