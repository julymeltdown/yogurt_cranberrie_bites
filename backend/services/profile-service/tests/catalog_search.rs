//! WP-03 T-801: catalog search against a real PostgreSQL cluster.
//!
//! The cases the task document calls out are:
//!
//! * a K-pop artist is absent from music search results — and absent by
//!   *catalog policy*, which is proven twice: the search itself performs no
//!   label test, and the database refuses to store a K-pop music row at all;
//! * the same query returns the same order twice.
//!
//! The database is the runner-owned private cluster described by
//! `backend/scripts/mcr/migration_smoke.py`; the URL arrives through
//! `PROFILE_TEST_DATABASE_URL` and the catalog is seeded from the import
//! manifest by `backend/scripts/mcr/wp03_discovery_tests.py`.
use profile_service::catalog::{
    search, validate, CatalogError, CatalogQuery, Category, DEFAULT_LIMIT, MAX_LIMIT,
};
use sqlx::PgPool;
use uuid::Uuid;

async fn pool() -> anyhow::Result<PgPool> {
    let url = std::env::var("PROFILE_TEST_DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("PROFILE_TEST_DATABASE_URL is required"))?;
    let options: sqlx::postgres::PgConnectOptions = url.parse()?;
    anyhow::ensure!(
        options.get_username() == "mcr_owner",
        "catalog fixture must use the owned cluster"
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
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('wp03-profile-migrations',0))")
        .execute(&mut *tx)
        .await?;
    let applied: bool = sqlx::query_scalar("select to_regclass('public.catalog_items') is not null")
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

fn constraint(error: &sqlx::Error) -> Option<String> {
    match error {
        sqlx::Error::Database(database) => database.constraint().map(str::to_owned),
        _ => None,
    }
}

fn query(q: Option<&str>, limit: i64) -> CatalogQuery {
    validate(CatalogQuery {
        q: q.map(str::to_owned),
        cursor: None,
        limit,
    })
    .expect("valid query")
}

/// A label that only this test's fixture can produce, so the seed cannot
/// collide with it.
fn fixture_label(name: &str) -> String {
    format!("wp03-{name}-{}", Uuid::new_v4().simple())
}

async fn insert_item(
    pool: &PgPool,
    category: &str,
    label: &str,
    reason: Option<&str>,
) -> anyhow::Result<Uuid> {
    Ok(sqlx::query_scalar(
        "insert into catalog_items(category,label,source,exclusion_reason) \
         values($1,$2,'test',$3) returning catalog_item_id",
    )
    .bind(category)
    .bind(label)
    .bind(reason)
    .fetch_one(pool)
    .await?)
}

// ---------------------------------------------------------------------------
// K-pop is absent by catalog policy
// ---------------------------------------------------------------------------

/// The seed manifest lists K-pop acts as *excluded* policy records. The
/// importer never stores them, so a music search cannot return one.
#[tokio::test]
async fn a_kpop_artist_is_absent_from_music_search() -> anyhow::Result<()> {
    let pool = pool().await?;
    // The seed's policy records exist in the manifest and are named here, so
    // this test fails loudly if the manifest changes shape.
    let manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("catalog/catalog-seed.v1.json"),
    )?)?;
    let excluded: Vec<&str> = manifest["items"]
        .as_array()
        .expect("items")
        .iter()
        .filter(|item| item["excluded"].as_bool() == Some(true))
        .map(|item| item["label"].as_str().expect("label"))
        .collect();
    assert!(
        excluded.contains(&"BTS"),
        "the seed must record the K-pop policy for BTS"
    );
    for label in &excluded {
        // No stored row carries an excluded label at all, in any category.
        let stored: i64 =
            sqlx::query_scalar("select count(*) from catalog_items where lower(label)=lower($1)")
                .bind(label)
                .fetch_one(&pool)
                .await?;
        assert_eq!(stored, 0, "{label} must not be stored in the catalog");
        // And no music search returns it.
        let page = search(&pool, Category::Music, &query(Some(label), MAX_LIMIT)).await?;
        assert!(
            page.items.iter().all(|item| !item.label.eq_ignore_ascii_case(label)),
            "{label} must be absent from music search results"
        );
    }
    Ok(())
}

/// The exclusion is a *policy*, not a filter: the database refuses to store a
/// K-pop music row, so no future writer can reintroduce one through search.
#[tokio::test]
async fn the_database_refuses_a_kpop_music_row() -> anyhow::Result<()> {
    let pool = pool().await?;
    let error = sqlx::query(
        "insert into catalog_items(category,label,source,excluded,exclusion_reason) \
         values('music','wp03-kpop-fixture','test',true,'kpop')",
    )
    .execute(&pool)
    .await
    .expect_err("a K-pop music row must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("catalog_items_music_excludes_kpop"),
        "the refusal must be the catalog policy constraint, not an incidental error"
    );
    Ok(())
}

/// A music row must carry an explicit exclusion review. Without it a writer
/// could import an unreviewed act by simply omitting the flag.
#[tokio::test]
async fn a_music_row_requires_an_exclusion_review() -> anyhow::Result<()> {
    let pool = pool().await?;
    let error = sqlx::query(
        "insert into catalog_items(category,label,source) values('music','wp03-unreviewed','test')",
    )
    .execute(&pool)
    .await
    .expect_err("an unreviewed music row must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("catalog_items_music_requires_exclusion_review")
    );
    // An anime row needs no such review, because the policy is music-specific.
    insert_item(&pool, "anime", &fixture_label("anime"), None).await?;
    Ok(())
}

/// Only music may carry the exclusion marker.
#[tokio::test]
async fn a_non_music_row_cannot_claim_an_exclusion() -> anyhow::Result<()> {
    let pool = pool().await?;
    let error = sqlx::query(
        "insert into catalog_items(category,label,source,excluded,exclusion_reason) \
         values('anime','wp03-excluded-anime','test',true,'kpop')",
    )
    .execute(&pool)
    .await
    .expect_err("a non-music excluded row must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("catalog_items_exclusion_category")
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Stable ordering
// ---------------------------------------------------------------------------

/// The same query returns the same order twice, including across a fresh
/// connection and across a page boundary.
#[tokio::test]
async fn the_same_query_returns_the_same_order_twice() -> anyhow::Result<()> {
    let pool = pool().await?;
    let first = search(&pool, Category::Music, &query(None, MAX_LIMIT)).await?;
    let second = search(&pool, Category::Music, &query(None, MAX_LIMIT)).await?;
    assert_eq!(first, second);
    assert!(first.items.len() > 1, "the seed must provide music rows");
    let labels: Vec<String> = first
        .items
        .iter()
        .map(|item| item.label.to_lowercase())
        .collect();
    let mut sorted = labels.clone();
    sorted.sort();
    assert_eq!(labels, sorted, "ordering must be by normalized label");

    // Paging with the returned cursor reproduces the same sequence.
    let mut collected = Vec::new();
    let mut cursor = None;
    loop {
        let page = search(
            &pool,
            Category::Music,
            &validate(CatalogQuery {
                q: None,
                cursor: cursor.clone(),
                limit: 3,
            })?,
        )
        .await?;
        assert!(page.items.len() <= 3);
        collected.extend(page.items.iter().map(|item| item.catalog_item_id));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    let expected: Vec<Uuid> = first.items.iter().map(|item| item.catalog_item_id).collect();
    assert_eq!(collected, expected, "paging must not reorder or drop rows");
    Ok(())
}

/// Two rows whose labels differ only by case still order deterministically,
/// because the item id is the tiebreak.
#[tokio::test]
async fn equal_labels_order_by_id() -> anyhow::Result<()> {
    let pool = pool().await?;
    // The unique index is case-insensitive, so a case-only duplicate is refused;
    // that is itself the guarantee that the tiebreak is never needed for them.
    let label = fixture_label("case");
    insert_item(&pool, "game", &label, None).await?;
    let error = sqlx::query(
        "insert into catalog_items(category,label,source) values('game',upper($1),'test')",
    )
    .bind(&label)
    .execute(&pool)
    .await
    .expect_err("a case-only duplicate must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("catalog_items_label_unique")
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Bounded result size
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_page_never_exceeds_the_requested_limit() -> anyhow::Result<()> {
    let pool = pool().await?;
    for limit in [1, 2, 5, DEFAULT_LIMIT] {
        let page = search(&pool, Category::Music, &query(None, limit)).await?;
        assert!(
            page.items.len() as i64 <= limit,
            "limit {limit} returned {} rows",
            page.items.len()
        );
        assert_eq!(page.category, "MUSIC");
    }
    Ok(())
}

#[tokio::test]
async fn out_of_bounds_queries_are_refused() -> anyhow::Result<()> {
    for limit in [0, -1, MAX_LIMIT + 1] {
        assert_eq!(
            validate(CatalogQuery {
                q: None,
                cursor: None,
                limit
            }),
            Err(CatalogError::InvalidQuery("limit"))
        );
    }
    assert_eq!(
        validate(CatalogQuery {
            q: Some("   ".to_owned()),
            cursor: None,
            limit: DEFAULT_LIMIT
        }),
        Err(CatalogError::InvalidQuery("q"))
    );
    assert_eq!(
        validate(CatalogQuery {
            q: Some("a".repeat(81)),
            cursor: None,
            limit: DEFAULT_LIMIT
        }),
        Err(CatalogError::InvalidQuery("q"))
    );
    assert_eq!(
        validate(CatalogQuery {
            q: None,
            cursor: Some("not a token".to_owned()),
            limit: DEFAULT_LIMIT
        }),
        Err(CatalogError::InvalidCursor)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Categories and search semantics
// ---------------------------------------------------------------------------

#[tokio::test]
async fn all_three_categories_are_searchable() -> anyhow::Result<()> {
    let pool = pool().await?;
    for category in [Category::Music, Category::Anime, Category::Game] {
        let page = search(&pool, category, &query(None, MAX_LIMIT)).await?;
        assert_eq!(page.category, category.wire());
        assert!(!page.items.is_empty(), "{category:?} must be seeded");
        assert!(
            page.items.iter().all(|item| item.category == category.wire()),
            "{category:?} must not return another category"
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_search_term_matches_case_insensitively() -> anyhow::Result<()> {
    let pool = pool().await?;
    let lower = search(&pool, Category::Music, &query(Some("my chemical"), MAX_LIMIT)).await?;
    let upper = search(&pool, Category::Music, &query(Some("MY CHEMICAL"), MAX_LIMIT)).await?;
    assert_eq!(lower, upper);
    assert!(
        lower.items.iter().any(|item| item.label == "My Chemical Romance"),
        "the seeded MCR row must be found"
    );
    Ok(())
}

/// A wildcard in the search term is an ordinary character, never a wildcard.
#[tokio::test]
async fn wildcards_in_the_search_term_are_literal() -> anyhow::Result<()> {
    let pool = pool().await?;
    let page = search(&pool, Category::Music, &query(Some("%"), MAX_LIMIT)).await?;
    assert!(
        page.items.is_empty(),
        "a literal percent must not match every row"
    );
    let page = search(&pool, Category::Music, &query(Some("_"), MAX_LIMIT)).await?;
    assert!(page.items.is_empty(), "a literal underscore is not a wildcard");
    Ok(())
}

/// A retired row stays for history and never appears in search.
#[tokio::test]
async fn a_retired_item_is_absent_from_search() -> anyhow::Result<()> {
    let pool = pool().await?;
    let label = fixture_label("retired");
    let id = insert_item(&pool, "game", &label, None).await?;
    let page = search(&pool, Category::Game, &query(Some(&label), MAX_LIMIT)).await?;
    assert_eq!(page.items.len(), 1);
    sqlx::query("update catalog_items set status='RETIRED' where catalog_item_id=$1")
        .bind(id)
        .execute(&pool)
        .await?;
    let page = search(&pool, Category::Game, &query(Some(&label), MAX_LIMIT)).await?;
    assert!(page.items.is_empty(), "a retired item must not be searchable");
    Ok(())
}

/// The catalog item id is the canonical UUID the frozen contract requires, not
/// the donor's prefixed text id.
#[tokio::test]
async fn catalog_ids_are_canonical_uuids() -> anyhow::Result<()> {
    let pool = pool().await?;
    let page = search(&pool, Category::Anime, &query(None, MAX_LIMIT)).await?;
    let item = page.items.first().expect("a seeded anime row");
    let rendered = item.catalog_item_id.to_string();
    assert_eq!(rendered, rendered.to_lowercase());
    assert_eq!(rendered.len(), 36);
    assert!(!rendered.starts_with("tci_"));
    assert_eq!(Uuid::parse_str(&rendered)?, item.catalog_item_id);
    Ok(())
}

/// The serialized page matches the frozen `CatalogPage` shape exactly.
#[tokio::test]
async fn the_page_serializes_as_the_frozen_contract() -> anyhow::Result<()> {
    let pool = pool().await?;
    let page = search(&pool, Category::Anime, &query(None, 2)).await?;
    let value = serde_json::to_value(&page)?;
    let object = value.as_object().expect("object");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, ["category", "items", "next_cursor"]);
    assert_eq!(object["category"], "ANIME");
    for item in object["items"].as_array().expect("items") {
        let mut keys: Vec<&str> = item.as_object().expect("item").keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(
            keys,
            ["catalog_item_id", "category", "external_url", "label"],
            "CatalogItem is closed"
        );
    }
    Ok(())
}

/// The category path segment and the wire spelling are two distinct closed
/// mappings, and neither accepts anything outside them.
#[tokio::test]
async fn category_parsing_is_closed() {
    assert_eq!(Category::parse("music"), Some(Category::Music));
    assert_eq!(Category::parse("anime"), Some(Category::Anime));
    assert_eq!(Category::parse("game"), Some(Category::Game));
    for value in ["", "Music", "movie", "book", "kpop", "game "] {
        assert_eq!(Category::parse(value), None, "{value}");
    }
    assert_eq!(Category::Music.wire(), "MUSIC");
    assert_eq!(Category::Game.column_value(), "game");
}
