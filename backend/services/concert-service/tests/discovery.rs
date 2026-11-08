//! WP-03 T-802: profile discovery against a real PostgreSQL cluster.
//!
//! The three acceptance cases the task document names are the first three tests
//! below, and each one is written so that it fails if the corresponding mistake
//! is reintroduced:
//!
//! * `a_profile_with_no_intro_post_is_discoverable` — the donor's bug. The
//!   fixture creates a profile with no intro post row at all and requires it in
//!   the page. It also asserts that `intro_posts` is not referenced by the
//!   discovery query text, so a future edit that joins intros back in fails
//!   here even if the fixture happens to pass.
//! * `a_profile_with_no_overlap_is_not_claimed_as_sharing` — the card's
//!   `shared_items` is an empty array, never an omitted field and never a
//!   fabricated item.
//! * `the_response_contains_no_numeric_score_field` — walks the serialized page
//!   and fails on any numeric field, and on any field whose name suggests a
//!   score, rank, match level or count.
//!
//! The database is the runner-owned private cluster described by
//! `backend/scripts/mcr/migration_smoke.py`; the URL arrives through
//! `CONCERT_TEST_DATABASE_URL`.
use concert_service::discovery::{
    discover, validate, Category, DiscoveryError, PeopleQuery, DEFAULT_LIMIT, MAX_LIMIT,
};
use sqlx::{PgPool, Row};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

async fn pool() -> anyhow::Result<PgPool> {
    let url = std::env::var("CONCERT_TEST_DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("CONCERT_TEST_DATABASE_URL is required"))?;
    let options: sqlx::postgres::PgConnectOptions = url.parse()?;
    anyhow::ensure!(
        options.get_username() == "mcr_owner",
        "discovery fixture must use the owned cluster"
    );
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;
    migrate(&pool).await?;
    Ok(pool)
}

async fn migrate(pool: &PgPool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('wp03-concert-migrations',0))")
        .execute(&mut *tx)
        .await?;
    let applied: bool =
        sqlx::query_scalar("select to_regclass('public.discovery_profiles') is not null")
            .fetch_one(&mut *tx)
            .await?;
    if applied {
        tx.commit().await?;
        return Ok(());
    }
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut entries: Vec<_> = std::fs::read_dir(&directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|extension| extension == "sql")
                && !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(".down.sql"))
        })
        .collect();
    entries.sort();
    for path in entries {
        let sql = std::fs::read_to_string(&path)?;
        sqlx::raw_sql(&sql).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// A profile fixture. `nickname` is unique per test run so the stable ordering
/// and the assertions below cannot collide with another test's rows.
struct Profile {
    profile_id: Uuid,
    account_id: Uuid,
    nickname: String,
}

async fn profile(pool: &PgPool, tag: &str, visibility: &str) -> anyhow::Result<Profile> {
    let profile_id = Uuid::new_v4();
    let account_id = Uuid::new_v4();
    let nickname = format!("wp03-{tag}-{}", &profile_id.simple().to_string()[..8]);
    sqlx::query(
        "insert into discovery_profiles(profile_id,account_id,nickname,bio,visibility,\
         interaction_style) values($1,$2,$3,$4,$5,$6)",
    )
    .bind(profile_id)
    .bind(account_id)
    .bind(&nickname)
    .bind(Some(format!("{tag} bio")))
    .bind(visibility)
    .bind(None::<String>)
    .execute(pool)
    .await?;
    Ok(Profile {
        profile_id,
        account_id,
        nickname,
    })
}

async fn favorite(
    pool: &PgPool,
    profile: &Profile,
    category: &str,
    item: Uuid,
    label: &str,
    position: i16,
) -> anyhow::Result<()> {
    sqlx::query(
        "insert into discovery_favorite_items(profile_id,category,catalog_item_id,label,position) \
         values($1,$2,$3,$4,$5)",
    )
    .bind(profile.profile_id)
    .bind(category)
    .bind(item)
    .bind(label)
    .bind(position)
    .execute(pool)
    .await?;
    Ok(())
}

fn query(category: Category, limit: i64) -> PeopleQuery {
    validate(PeopleQuery {
        category,
        cursor: None,
        limit,
    })
    .expect("valid query")
}

/// The ids in a page, for order-insensitive membership assertions.
fn ids(page: &concert_service::discovery::PeoplePage) -> Vec<Uuid> {
    page.items.iter().map(|card| card.profile_id).collect()
}

// ---------------------------------------------------------------------------
// Acceptance: a profile with no intro post appears
// ---------------------------------------------------------------------------

/// The donor starts its overlap query from `intros`, so a profile without an
/// intro post is invisible. Discovery here starts from profiles.
#[tokio::test]
async fn a_profile_with_no_intro_post_is_discoverable() -> anyhow::Result<()> {
    let pool = pool().await?;
    let viewer = profile(&pool, "viewer-no-intro", "PUBLIC").await?;
    let subject = profile(&pool, "no-intro", "PUBLIC").await?;
    let item = Uuid::new_v4();
    favorite(&pool, &viewer, "music", item, "Shared Band", 0).await?;
    favorite(&pool, &subject, "music", item, "Shared Band", 0).await?;
    // The subject has no intro post at all. Assert that as a precondition rather
    // than assuming it, so a fixture that accidentally creates one fails here.
    let intros: i64 =
        sqlx::query_scalar("select count(*) from intro_posts where author_profile_id=$1")
            .bind(subject.profile_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(intros, 0, "the subject must have no intro post");

    let page = discover(&pool, viewer.profile_id, &query(Category::Music, MAX_LIMIT)).await?;
    assert!(
        ids(&page).contains(&subject.profile_id),
        "a profile with no intro post must be discoverable"
    );
    let card = page
        .items
        .iter()
        .find(|card| card.profile_id == subject.profile_id)
        .expect("card");
    assert_eq!(card.shared_items.len(), 1);
    assert_eq!(card.shared_items[0].catalog_item_id, item);
    Ok(())
}

/// The query text itself never references `intro_posts`. This is the structural
/// half of the case above: even a fixture that happens to pass cannot hide a
/// reintroduced intro join.
#[tokio::test]
async fn the_discovery_query_never_references_intro_posts() -> anyhow::Result<()> {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/discovery/repository.rs"),
    )?;
    // Strip the module documentation, which legitimately *mentions* intro posts
    // while explaining why they are not used.
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !code.contains("intro_posts"),
        "the discovery query must not reference intro_posts"
    );
    assert!(
        !code.contains("intro_id"),
        "the discovery query must not reference an intro id"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Acceptance: a profile with no overlap is not claimed as sharing
// ---------------------------------------------------------------------------

/// Two profiles that share nothing never produce a shared-interest claim.
///
/// Discovery returns only overlapping profiles, so the no-overlap profile is
/// absent from the page entirely — and when it is reached through the profile
/// operation, `shared_items` is an empty array rather than an omitted field.
#[tokio::test]
async fn a_profile_with_no_overlap_is_not_claimed_as_sharing() -> anyhow::Result<()> {
    let pool = pool().await?;
    let viewer = profile(&pool, "viewer-no-overlap", "PUBLIC").await?;
    let disjoint = profile(&pool, "disjoint", "PUBLIC").await?;
    favorite(&pool, &viewer, "music", Uuid::new_v4(), "Viewer Band", 0).await?;
    favorite(&pool, &disjoint, "music", Uuid::new_v4(), "Other Band", 0).await?;

    let page = discover(&pool, viewer.profile_id, &query(Category::Music, MAX_LIMIT)).await?;
    assert!(
        !ids(&page).contains(&disjoint.profile_id),
        "a profile with no overlap must not appear in the people list"
    );
    // The card type has no way to express a shared-interest claim without
    // items: `shared_items` is a required array. Prove the empty case directly.
    let card = concert_service::discovery::PeopleCard {
        profile_id: disjoint.profile_id,
        nickname: disjoint.nickname.clone(),
        profile_image_media_id: None,
        shared_items: Vec::new(),
        interaction_style: None,
    };
    let value = serde_json::to_value(&card)?;
    assert_eq!(value["shared_items"], serde_json::json!([]));
    assert!(
        value.get("shared_items").is_some(),
        "shared_items must be present, not omitted"
    );
    Ok(())
}

/// Overlap is per category. A profile sharing only an anime item is not a music
/// match, and vice versa.
#[tokio::test]
async fn overlap_is_computed_per_category() -> anyhow::Result<()> {
    let pool = pool().await?;
    let viewer = profile(&pool, "viewer-category", "PUBLIC").await?;
    let subject = profile(&pool, "category-subject", "PUBLIC").await?;
    let anime = Uuid::new_v4();
    favorite(&pool, &viewer, "anime", anime, "NANA", 0).await?;
    favorite(&pool, &subject, "anime", anime, "NANA", 0).await?;
    favorite(&pool, &viewer, "music", Uuid::new_v4(), "Viewer Band", 0).await?;
    favorite(&pool, &subject, "music", Uuid::new_v4(), "Other Band", 0).await?;

    let music = discover(&pool, viewer.profile_id, &query(Category::Music, MAX_LIMIT)).await?;
    assert!(!ids(&music).contains(&subject.profile_id));
    let anime_page =
        discover(&pool, viewer.profile_id, &query(Category::Anime, MAX_LIMIT)).await?;
    assert!(ids(&anime_page).contains(&subject.profile_id));
    Ok(())
}

/// Overlap is by catalog item id, never by label. Two profiles that selected
/// the same id under different labels still overlap; two profiles with the same
/// label but different ids do not.
#[tokio::test]
async fn overlap_uses_catalog_ids_and_never_labels() -> anyhow::Result<()> {
    let pool = pool().await?;
    let viewer = profile(&pool, "viewer-ids", "PUBLIC").await?;
    let same_id = profile(&pool, "same-id", "PUBLIC").await?;
    let same_label = profile(&pool, "same-label", "PUBLIC").await?;
    let shared = Uuid::new_v4();
    favorite(&pool, &viewer, "game", shared, "Label A", 0).await?;
    // Same id, different label: still an overlap.
    favorite(&pool, &same_id, "game", shared, "Label B", 0).await?;
    // Different id, same label as the viewer's: not an overlap.
    favorite(&pool, &same_label, "game", Uuid::new_v4(), "Label A", 0).await?;

    let page = discover(&pool, viewer.profile_id, &query(Category::Game, MAX_LIMIT)).await?;
    assert!(ids(&page).contains(&same_id.profile_id));
    assert!(!ids(&page).contains(&same_label.profile_id));
    Ok(())
}

// ---------------------------------------------------------------------------
// Acceptance: no numeric score field
// ---------------------------------------------------------------------------

/// The response carries no numeric score, rank, match level, favorite count or
/// like count. The donor computes a weighted score and buckets it into
/// `strong`/`good`/`explore`; none of that may reappear.
#[tokio::test]
async fn the_response_contains_no_numeric_score_field() -> anyhow::Result<()> {
    let pool = pool().await?;
    let viewer = profile(&pool, "viewer-score", "PUBLIC").await?;
    let subject = profile(&pool, "score-subject", "PUBLIC").await?;
    let item = Uuid::new_v4();
    favorite(&pool, &viewer, "music", item, "Shared", 0).await?;
    favorite(&pool, &subject, "music", item, "Shared", 0).await?;
    let page = discover(&pool, viewer.profile_id, &query(Category::Music, MAX_LIMIT)).await?;
    assert!(!page.items.is_empty(), "the fixture must produce a card");
    let value = serde_json::to_value(&page)?;

    fn walk(value: &serde_json::Value, path: &str, problems: &mut Vec<String>) {
        match value {
            serde_json::Value::Number(number) => {
                problems.push(format!("numeric field at {path}: {number}"))
            }
            serde_json::Value::Object(object) => {
                for (key, child) in object {
                    let lower = key.to_lowercase();
                    for forbidden in [
                        "score", "rank", "match_level", "compatibility", "closeness",
                        "strength", "weight", "count", "distance", "similarity",
                    ] {
                        if lower.contains(forbidden) {
                            problems.push(format!("forbidden field {path}.{key}"));
                        }
                    }
                    walk(child, &format!("{path}.{key}"), problems);
                }
            }
            serde_json::Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    walk(child, &format!("{path}[{index}]"), problems);
                }
            }
            _ => {}
        }
    }
    let mut problems = Vec::new();
    walk(&value, "$", &mut problems);
    assert!(problems.is_empty(), "discovery response: {problems:?}");

    // The page itself is closed to the contract's two fields.
    let mut keys: Vec<&str> = value.as_object().expect("object").keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, ["items", "next_cursor"]);
    for card in value["items"].as_array().expect("items") {
        let mut keys: Vec<&str> = card.as_object().expect("card").keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "interaction_style",
                "nickname",
                "profile_id",
                "profile_image_media_id",
                "shared_items"
            ],
            "PeopleCard is closed and carries no score"
        );
    }
    Ok(())
}

/// The ordering is a stable nickname/id sort, not a ranking. A profile with
/// many shared items is not promoted above one with a single shared item.
#[tokio::test]
async fn the_order_does_not_rank_by_shared_item_count() -> anyhow::Result<()> {
    let pool = pool().await?;
    let viewer = profile(&pool, "viewer-order", "PUBLIC").await?;
    let items: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
    for (position, item) in items.iter().enumerate() {
        favorite(&pool, &viewer, "music", *item, "Shared", position as i16).await?;
    }
    // `a-many` shares three items; `z-one` shares one. Alphabetically the
    // three-item profile comes first, so the assertion below would pass under a
    // count ranking too — so the names are chosen the other way round.
    let one = profile(&pool, "a-one", "PUBLIC").await?;
    let many = profile(&pool, "z-many", "PUBLIC").await?;
    favorite(&pool, &one, "music", items[0], "Shared", 0).await?;
    for (position, item) in items.iter().enumerate() {
        favorite(&pool, &many, "music", *item, "Shared", position as i16).await?;
    }
    let page = discover(&pool, viewer.profile_id, &query(Category::Music, MAX_LIMIT)).await?;
    let order = ids(&page);
    let one_at = order.iter().position(|id| *id == one.profile_id).expect("one");
    let many_at = order.iter().position(|id| *id == many.profile_id).expect("many");
    assert!(
        one_at < many_at,
        "ordering must be by nickname, not by how many items are shared"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Visibility and blocks
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_private_profile_is_never_discovered() -> anyhow::Result<()> {
    let pool = pool().await?;
    let viewer = profile(&pool, "viewer-private", "PUBLIC").await?;
    let hidden = profile(&pool, "hidden", "PRIVATE").await?;
    let item = Uuid::new_v4();
    favorite(&pool, &viewer, "music", item, "Shared", 0).await?;
    favorite(&pool, &hidden, "music", item, "Shared", 0).await?;
    let page = discover(&pool, viewer.profile_id, &query(Category::Music, MAX_LIMIT)).await?;
    assert!(!ids(&page).contains(&hidden.profile_id));
    Ok(())
}

#[tokio::test]
async fn a_block_in_either_direction_hides_the_profile() -> anyhow::Result<()> {
    let pool = pool().await?;
    let viewer = profile(&pool, "viewer-block", "PUBLIC").await?;
    let blocked_by_viewer = profile(&pool, "blocked-by-viewer", "PUBLIC").await?;
    let blocking_viewer = profile(&pool, "blocking-viewer", "PUBLIC").await?;
    let item = Uuid::new_v4();
    favorite(&pool, &viewer, "music", item, "Shared", 0).await?;
    favorite(&pool, &blocked_by_viewer, "music", item, "Shared", 0).await?;
    favorite(&pool, &blocking_viewer, "music", item, "Shared", 0).await?;
    sqlx::query(
        "insert into discovery_blocks(blocker_account_id,blocked_account_id) values($1,$2)",
    )
    .bind(viewer.account_id)
    .bind(blocked_by_viewer.account_id)
    .execute(&pool)
    .await?;
    sqlx::query(
        "insert into discovery_blocks(blocker_account_id,blocked_account_id) values($1,$2)",
    )
    .bind(blocking_viewer.account_id)
    .bind(viewer.account_id)
    .execute(&pool)
    .await?;
    let page = discover(&pool, viewer.profile_id, &query(Category::Music, MAX_LIMIT)).await?;
    assert!(!ids(&page).contains(&blocked_by_viewer.profile_id));
    assert!(!ids(&page).contains(&blocking_viewer.profile_id));
    Ok(())
}

/// A lifted block restores the profile.
#[tokio::test]
async fn a_removed_block_no_longer_hides_the_profile() -> anyhow::Result<()> {
    let pool = pool().await?;
    let viewer = profile(&pool, "viewer-unblock", "PUBLIC").await?;
    let subject = profile(&pool, "unblocked", "PUBLIC").await?;
    let item = Uuid::new_v4();
    favorite(&pool, &viewer, "music", item, "Shared", 0).await?;
    favorite(&pool, &subject, "music", item, "Shared", 0).await?;
    sqlx::query(
        "insert into discovery_blocks(blocker_account_id,blocked_account_id) values($1,$2)",
    )
    .bind(viewer.account_id)
    .bind(subject.account_id)
    .execute(&pool)
    .await?;
    let page = discover(&pool, viewer.profile_id, &query(Category::Music, MAX_LIMIT)).await?;
    assert!(!ids(&page).contains(&subject.profile_id));
    sqlx::query(
        "update discovery_blocks set deleted_at=clock_timestamp() \
         where blocker_account_id=$1 and blocked_account_id=$2",
    )
    .bind(viewer.account_id)
    .bind(subject.account_id)
    .execute(&pool)
    .await?;
    let page = discover(&pool, viewer.profile_id, &query(Category::Music, MAX_LIMIT)).await?;
    assert!(ids(&page).contains(&subject.profile_id));
    Ok(())
}

/// A viewer never discovers themselves.
#[tokio::test]
async fn the_viewer_is_never_their_own_match() -> anyhow::Result<()> {
    let pool = pool().await?;
    let viewer = profile(&pool, "viewer-self", "PUBLIC").await?;
    favorite(&pool, &viewer, "music", Uuid::new_v4(), "Solo", 0).await?;
    let page = discover(&pool, viewer.profile_id, &query(Category::Music, MAX_LIMIT)).await?;
    assert!(!ids(&page).contains(&viewer.profile_id));
    Ok(())
}

// ---------------------------------------------------------------------------
// Paging and bounds
// ---------------------------------------------------------------------------

#[tokio::test]
async fn paging_is_stable_and_loses_nothing() -> anyhow::Result<()> {
    let pool = pool().await?;
    let viewer = profile(&pool, "viewer-page", "PUBLIC").await?;
    let item = Uuid::new_v4();
    favorite(&pool, &viewer, "music", item, "Shared", 0).await?;
    for index in 0..5 {
        let subject = profile(&pool, &format!("page-{index}"), "PUBLIC").await?;
        favorite(&pool, &subject, "music", item, "Shared", 0).await?;
    }
    let all = discover(&pool, viewer.profile_id, &query(Category::Music, MAX_LIMIT)).await?;
    assert!(all.items.len() >= 5);
    let mut collected = Vec::new();
    let mut cursor = None;
    loop {
        let page = discover(
            &pool,
            viewer.profile_id,
            &validate(PeopleQuery {
                category: Category::Music,
                cursor: cursor.clone(),
                limit: 2,
            })?,
        )
        .await?;
        assert!(page.items.len() <= 2);
        collected.extend(ids(&page));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(collected, ids(&all), "paging must preserve order and rows");
    Ok(())
}

#[tokio::test]
async fn out_of_bounds_queries_are_refused() {
    for limit in [0, -1, MAX_LIMIT + 1] {
        assert_eq!(
            validate(PeopleQuery {
                category: Category::Music,
                cursor: None,
                limit
            }),
            Err(DiscoveryError::InvalidQuery("limit"))
        );
    }
    assert_eq!(
        validate(PeopleQuery {
            category: Category::Music,
            cursor: Some("not a cursor".to_owned()),
            limit: DEFAULT_LIMIT
        }),
        Err(DiscoveryError::InvalidCursor)
    );
}

// ---------------------------------------------------------------------------
// Schema: the absence of a score is structural
// ---------------------------------------------------------------------------

/// No discovery table has a score, rank, weight or count column. The absence is
/// the guarantee; a column would be a product decision.
#[tokio::test]
async fn the_discovery_schema_has_no_score_column() -> anyhow::Result<()> {
    let pool = pool().await?;
    let rows = sqlx::query(
        "select table_name, column_name from information_schema.columns \
         where table_schema='public' and table_name like 'discovery_%'",
    )
    .fetch_all(&pool)
    .await?;
    assert!(!rows.is_empty());
    for row in rows {
        let table: String = row.get("table_name");
        let column: String = row.get("column_name");
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

/// A profile row has no intro reference, so discovery cannot depend on one.
#[tokio::test]
async fn a_discovery_profile_has_no_intro_reference() -> anyhow::Result<()> {
    let pool = pool().await?;
    let columns = sqlx::query_scalar::<_, String>(
        "select column_name from information_schema.columns \
         where table_schema='public' and table_name='discovery_profiles'",
    )
    .fetch_all(&pool)
    .await?;
    assert!(!columns.iter().any(|column| column.contains("intro")));
    assert!(!columns.iter().any(|column| column == "event_id"));
    Ok(())
}

/// The sixth favorite in one category is refused by the database, so the
/// discovery mirror cannot disagree with the profile service's own cap.
#[tokio::test]
async fn a_sixth_mirrored_favorite_is_refused() -> anyhow::Result<()> {
    let pool = pool().await?;
    let subject = profile(&pool, "sixth", "PUBLIC").await?;
    for position in 0..5i16 {
        favorite(&pool, &subject, "music", Uuid::new_v4(), "Item", position).await?;
    }
    let error = sqlx::query(
        "insert into discovery_favorite_items(profile_id,category,catalog_item_id,label,position) \
         values($1,'music',$2,'Item',5)",
    )
    .bind(subject.profile_id)
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .expect_err("a sixth favorite must be refused");
    match &error {
        sqlx::Error::Database(database) => assert_eq!(
            database.constraint(),
            Some("discovery_favorite_items_capacity"),
            "{error:?}"
        ),
        other => panic!("unexpected error {other:?}"),
    }
    Ok(())
}

/// Deleting a profile removes its mirrored favorites with it.
#[tokio::test]
async fn deleting_a_profile_cascades_its_favorites() -> anyhow::Result<()> {
    let pool = pool().await?;
    let subject = profile(&pool, "cascade", "PUBLIC").await?;
    favorite(&pool, &subject, "music", Uuid::new_v4(), "Item", 0).await?;
    sqlx::query("delete from discovery_profiles where profile_id=$1")
        .bind(subject.profile_id)
        .execute(&pool)
        .await?;
    let remaining: i64 = sqlx::query_scalar(
        "select count(*) from discovery_favorite_items where profile_id=$1",
    )
    .bind(subject.profile_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(remaining, 0);
    Ok(())
}
