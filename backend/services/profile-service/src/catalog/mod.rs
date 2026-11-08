//! WP-03 T-801: favorites catalog search.
//!
//! Search runs over exactly three categories — music, anime, games — and is
//! served from the `catalog_items` table created by migration `0016`. There is
//! no provider integration here on purpose: the catalog is a table plus a seed
//! path, and this module is the read side of it.
//!
//! Three rules shape this module:
//!
//! * **K-pop is absent by catalog policy, not by string filtering.** A K-pop
//!   row cannot be stored in the music category at all (migration constraint
//!   `catalog_items_music_excludes_kpop`), so this query has no reason to test
//!   a label and deliberately does not. If a future writer found a way to store
//!   one, the search would return it — which is exactly why the exclusion lives
//!   in the schema and has its own negative test.
//! * **Bounded result size.** `limit` is clamped to the frozen contract's
//!   1..=100 with a default of 20, and the page never exceeds it.
//! * **Stable ordering.** Rows are ordered by the normalized label and then by
//!   the item id, so the same query returns the same order twice even when two
//!   labels differ only by case.
//!
//! The cursor is an opaque keyset token carrying the last `(label, id)` pair.
//! It is base64url without padding, which satisfies the contract's
//! `^[A-Za-z0-9_-]+$` pattern.
use base64ct::{Base64UrlUnpadded, Encoding};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

/// The frozen contract's category enum, in the order the product presents it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Category {
    Music,
    Anime,
    Game,
}

/// Presentation order: bands and artists, then anime, then games.
pub const CATEGORY_ORDER: [Category; 3] = [Category::Music, Category::Anime, Category::Game];

impl Category {
    /// The path segment the operation uses: `/api/catalog/{category}/search`.
    pub fn path_segment(self) -> &'static str {
        match self {
            Self::Music => "music",
            Self::Anime => "anime",
            Self::Game => "game",
        }
    }

    /// The stored column value, matching `profile_favorites.category`.
    pub fn column_value(self) -> &'static str {
        match self {
            Self::Music => "music",
            Self::Anime => "anime",
            Self::Game => "game",
        }
    }

    /// Closed parsing. Anything outside the three categories is a client error,
    /// never a default.
    pub fn parse(segment: &str) -> Option<Self> {
        match segment {
            "music" => Some(Self::Music),
            "anime" => Some(Self::Anime),
            "game" => Some(Self::Game),
            _ => None,
        }
    }

    /// The contract's wire spelling, used by `FavoriteItem.category`.
    pub fn wire(self) -> &'static str {
        match self {
            Self::Music => "MUSIC",
            Self::Anime => "ANIME",
            Self::Game => "GAME",
        }
    }
}

/// Default page size, from the frozen `CatalogQuery`.
pub const DEFAULT_LIMIT: i64 = 20;
/// Maximum page size, from the frozen `CatalogQuery`.
pub const MAX_LIMIT: i64 = 100;
/// The contract's `q` bound.
pub const MAX_QUERY: usize = 80;
/// One catalog row as the contract's `CatalogItem` describes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CatalogItem {
    pub catalog_item_id: Uuid,
    pub label: String,
    pub category: &'static str,
    pub external_url: Option<String>,
}

/// The contract's `CatalogPage`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CatalogPage {
    pub category: &'static str,
    pub items: Vec<CatalogItem>,
    pub next_cursor: Option<String>,
}

/// A closed query. Every field of the frozen `CatalogQuery` is present and
/// nothing else is accepted; unknown fields are rejected by the route parser
/// before this type is built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogQuery {
    pub q: Option<String>,
    pub cursor: Option<String>,
    pub limit: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CatalogError {
    /// A query field violated its frozen bound.
    InvalidQuery(&'static str),
    /// The cursor is not a token this service issued.
    InvalidCursor,
    /// The store is unavailable. Never leaks the database error.
    Unavailable,
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "catalog error: {}", self.reason())
    }
}

impl std::error::Error for CatalogError {}

impl CatalogError {
    pub fn reason(&self) -> &'static str {
        match self {
            Self::InvalidQuery(field) => field,
            Self::InvalidCursor => "cursor",
            Self::Unavailable => "store",
        }
    }
}

/// The keyset position of the last row of a page.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Cursor {
    label: String,
    id: Uuid,
}

const CURSOR_PREFIX: &str = "c1:";

fn encode_cursor(cursor: &Cursor) -> String {
    let raw = format!("{CURSOR_PREFIX}{}\n{}", cursor.label, cursor.id);
    Base64UrlUnpadded::encode_string(raw.as_bytes())
}

fn decode_cursor(token: &str) -> Result<Cursor, CatalogError> {
    // The contract bounds the token; refuse anything longer before decoding.
    if token.is_empty() || token.len() > 1024 {
        return Err(CatalogError::InvalidCursor);
    }
    let raw = Base64UrlUnpadded::decode_vec(token).map_err(|_| CatalogError::InvalidCursor)?;
    let raw = String::from_utf8(raw).map_err(|_| CatalogError::InvalidCursor)?;
    let raw = raw
        .strip_prefix(CURSOR_PREFIX)
        .ok_or(CatalogError::InvalidCursor)?;
    let (label, id) = raw.split_once('\n').ok_or(CatalogError::InvalidCursor)?;
    // Canonical lowercase, non-nil, exactly as the contract spells a UUID.
    let id = Uuid::parse_str(id)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == id.hyphenated().to_string())
        .ok_or(CatalogError::InvalidCursor)?;
    Ok(Cursor {
        label: label.to_owned(),
        id,
    })
}

/// Validate a query against the frozen `CatalogQuery` bounds.
///
/// The route layer already rejects unknown fields; this is the value-level
/// check, and it is the only place that decides what a valid search is.
pub fn validate(query: CatalogQuery) -> Result<CatalogQuery, CatalogError> {
    if !(1..=MAX_LIMIT).contains(&query.limit) {
        return Err(CatalogError::InvalidQuery("limit"));
    }
    let q = match query.q.as_deref() {
        // An empty or whitespace-only `q` is not a search term. The contract
        // requires at least one character, so an empty value is invalid rather
        // than silently meaning "everything".
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(CatalogError::InvalidQuery("q"));
            }
            if value.chars().count() > MAX_QUERY {
                return Err(CatalogError::InvalidQuery("q"));
            }
            if value.chars().any(|c| c.is_control()) {
                return Err(CatalogError::InvalidQuery("q"));
            }
            Some(trimmed.to_owned())
        }
        None => None,
    };
    if let Some(cursor) = query.cursor.as_deref() {
        decode_cursor(cursor)?;
    }
    Ok(CatalogQuery {
        q,
        cursor: query.cursor,
        limit: query.limit,
    })
}

/// Search one category.
///
/// The result is bounded by `limit` and ordered by `(lower(label),
/// catalog_item_id)`. The K-pop exclusion is a property of the stored rows, so
/// this query performs no label test for it.
pub async fn search(
    pool: &PgPool,
    category: Category,
    query: &CatalogQuery,
) -> Result<CatalogPage, CatalogError> {
    let cursor = match query.cursor.as_deref() {
        Some(token) => Some(decode_cursor(token)?),
        None => None,
    };
    // A literal `%` or `_` in the search term is an ordinary character, not a
    // wildcard, so the pattern is escaped before it reaches `ILIKE`.
    let pattern = query.q.as_deref().map(|q| {
        let escaped = q
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        format!("%{escaped}%")
    });
    // One row more than the page proves whether a next page exists without a
    // second count query.
    let rows: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
        "select catalog_item_id, label, external_url from catalog_items \
         where category = $1 and status = 'ACTIVE' \
           and ($2::text is null or label ilike $2) \
           and ($3::text is null \
                or (lower(label), catalog_item_id) > ($3::text, $4::uuid)) \
         order by lower(label), catalog_item_id \
         limit $5",
    )
    .bind(category.column_value())
    .bind(pattern.as_deref())
    .bind(cursor.as_ref().map(|cursor| cursor.label.as_str()))
    .bind(cursor.as_ref().map(|cursor| cursor.id))
    .bind(query.limit + 1)
    .fetch_all(pool)
    .await
    .map_err(|_| CatalogError::Unavailable)?;
    let mut items: Vec<CatalogItem> = rows
        .iter()
        .map(|(id, label, url)| CatalogItem {
            catalog_item_id: *id,
            label: label.clone(),
            category: category.wire(),
            external_url: url.clone(),
        })
        .collect();
    let next_cursor = if items.len() as i64 > query.limit {
        items.truncate(query.limit as usize);
        let last = items.last().expect("a full page has a last row");
        Some(encode_cursor(&Cursor {
            label: last.label.to_lowercase(),
            id: last.catalog_item_id,
        }))
    } else {
        None
    };
    Ok(CatalogPage {
        category: category.wire(),
        items,
        next_cursor,
    })
}
