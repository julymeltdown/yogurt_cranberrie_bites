//! WP-03 T-802: the profile discovery query.
//!
//! Discovery answers one question: *which profiles selected the same catalog
//! items as me?* Everything about the shape of this module follows from taking
//! that question literally.
//!
//! **It starts from profiles, not from intro posts.** The donor
//! (`explicit_taste_matches.rs`) joins `intros` and hangs the overlap off them,
//! so a profile that never wrote an intro is invisible. Here the driving table
//! is `discovery_profiles`, and `intro_posts` is not referenced anywhere in this
//! module. A profile with no intro post is therefore discoverable by
//! construction, and `discovery.rs` proves it.
//!
//! **Overlap is computed from catalog item ids only.** The join is on
//! `catalog_item_id`; the label is carried along for display and never compared.
//! There is no image analysis, no text similarity and no tag matching.
//!
//! **There is no score.** The donor computes a weighted score, buckets it into
//! `strong`/`good`/`explore`, and orders by it. None of that exists here. The
//! result type has no numeric field at all, and the ordering is a stable
//! `(nickname, profile_id)` sort so pagination is deterministic without ranking
//! anybody. A test asserts the serialized page contains no numeric score field.
//!
//! **Visibility and blocks are applied in the query.** A private profile is
//! never returned, and a block in either direction removes the profile from the
//! viewer's results.
//!
//! The one place the product plan is deliberately broader than the query: a
//! profile with no overlap may still be browsed through `getProfile`. Discovery
//! returns only overlapping profiles, and the *card* never claims a shared
//! interest when `shared_items` is empty. `shared_items` is a required array, so
//! "no overlap" is an empty array, never an omitted field.
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

/// The frozen contract's category enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Music,
    Anime,
    Game,
}

impl Category {
    pub fn wire(self) -> &'static str {
        match self {
            Self::Music => "MUSIC",
            Self::Anime => "ANIME",
            Self::Game => "GAME",
        }
    }

    /// The stored column value, matching `discovery_favorite_items.category`.
    pub fn column_value(self) -> &'static str {
        match self {
            Self::Music => "music",
            Self::Anime => "anime",
            Self::Game => "game",
        }
    }

    /// Closed parsing of the query's `category` value.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "MUSIC" => Some(Self::Music),
            "ANIME" => Some(Self::Anime),
            "GAME" => Some(Self::Game),
            _ => None,
        }
    }
}

/// Default page size, from the frozen `PeopleQuery`.
pub const DEFAULT_LIMIT: i64 = 20;
/// Maximum page size, from the frozen `PeopleQuery`.
pub const MAX_LIMIT: i64 = 100;

/// A catalog item as a card shows it: the contract's `FavoriteItem`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FavoriteItem {
    pub catalog_item_id: Uuid,
    pub label: String,
    pub category: &'static str,
    pub external_url: Option<String>,
}

/// The contract's `PeopleCard`.
///
/// Note what is absent: no score, no rank, no match level, no favorite count and
/// no like count. `shared_items` is required and may be empty.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PeopleCard {
    pub profile_id: Uuid,
    pub nickname: String,
    pub profile_image_media_id: Option<Uuid>,
    pub shared_items: Vec<FavoriteItem>,
    pub interaction_style: Option<String>,
}

/// The contract's `PeoplePage`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PeoplePage {
    pub items: Vec<PeopleCard>,
    pub next_cursor: Option<String>,
}

/// A closed query. Every field of the frozen `PeopleQuery` is present and
/// nothing else is accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeopleQuery {
    pub category: Category,
    pub cursor: Option<String>,
    pub limit: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DiscoveryError {
    InvalidQuery(&'static str),
    InvalidCursor,
    Unavailable,
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "discovery error: {}", self.reason())
    }
}

impl std::error::Error for DiscoveryError {}

impl DiscoveryError {
    pub fn reason(&self) -> &'static str {
        match self {
            Self::InvalidQuery(field) => field,
            Self::InvalidCursor => "cursor",
            Self::Unavailable => "store",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Cursor {
    nickname: String,
    id: Uuid,
}

const CURSOR_PREFIX: &str = "p1:";

fn encode_cursor(cursor: &Cursor) -> String {
    use base64ct::{Base64UrlUnpadded, Encoding};
    Base64UrlUnpadded::encode_string(
        format!("{CURSOR_PREFIX}{}\n{}", cursor.nickname, cursor.id).as_bytes(),
    )
}

fn decode_cursor(token: &str) -> Result<Cursor, DiscoveryError> {
    use base64ct::{Base64UrlUnpadded, Encoding};
    if token.is_empty() || token.len() > 1024 {
        return Err(DiscoveryError::InvalidCursor);
    }
    let raw = Base64UrlUnpadded::decode_vec(token).map_err(|_| DiscoveryError::InvalidCursor)?;
    let raw = String::from_utf8(raw).map_err(|_| DiscoveryError::InvalidCursor)?;
    let raw = raw
        .strip_prefix(CURSOR_PREFIX)
        .ok_or(DiscoveryError::InvalidCursor)?;
    let (nickname, id) = raw.split_once('\n').ok_or(DiscoveryError::InvalidCursor)?;
    let id = Uuid::parse_str(id)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == id.hyphenated().to_string())
        .ok_or(DiscoveryError::InvalidCursor)?;
    Ok(Cursor {
        nickname: nickname.to_owned(),
        id,
    })
}

/// Validate a query against the frozen `PeopleQuery` bounds.
pub fn validate(query: PeopleQuery) -> Result<PeopleQuery, DiscoveryError> {
    if !(1..=MAX_LIMIT).contains(&query.limit) {
        return Err(DiscoveryError::InvalidQuery("limit"));
    }
    if let Some(cursor) = query.cursor.as_deref() {
        decode_cursor(cursor)?;
    }
    Ok(query)
}

/// One row of the overlap join, before it is folded into a card.
struct Row {
    profile_id: Uuid,
    nickname: String,
    profile_image_media_id: Option<Uuid>,
    interaction_style: Option<String>,
    catalog_item_id: Uuid,
    label: String,
    item_category: String,
}

fn item_category(value: &str) -> &'static str {
    match value {
        "music" => "MUSIC",
        "anime" => "ANIME",
        "game" => "GAME",
        // The column is constrained to these three values, so this arm is
        // unreachable for stored data. It is not a default: an unexpected value
        // is refused rather than mislabelled.
        _ => "",
    }
}

/// Discover profiles that share at least one catalog item with the viewer in
/// `query.category`.
///
/// The query starts from `discovery_profiles`. It never references
/// `intro_posts`, so an intro post is neither required nor consulted.
pub async fn discover(
    pool: &PgPool,
    viewer_profile_id: Uuid,
    query: &PeopleQuery,
) -> Result<PeoplePage, DiscoveryError> {
    let cursor = match query.cursor.as_deref() {
        Some(token) => Some(decode_cursor(token)?),
        None => None,
    };
    // One row per (profile, shared item). The page is cut by profile, so the
    // query fetches one profile more than the page to prove a next page exists.
    type DiscoveryRow = (Uuid, String, Option<Uuid>, Option<String>, Uuid, String, String);
    let rows: Vec<Row> = sqlx::query_as(
            // The driving table is the profile. There is no join to intro_posts
            // and there must never be one: an intro post is optional, and a
            // profile without one is still discoverable.
            "select p.profile_id, p.nickname, p.profile_image_media_id, p.interaction_style, \
                    f.catalog_item_id, f.label, f.category \
             from discovery_profiles p \
             join discovery_favorite_items f \
               on f.profile_id = p.profile_id and f.category = $2 \
             where p.visibility = 'PUBLIC' \
               and p.profile_id <> $1 \
               and not exists ( \
                   select 1 from discovery_blocks b \
                   join discovery_profiles me on me.profile_id = $1 \
                   where b.deleted_at is null \
                     and ((b.blocker_account_id = me.account_id \
                           and b.blocked_account_id = p.account_id) \
                       or (b.blocker_account_id = p.account_id \
                           and b.blocked_account_id = me.account_id))) \
               and exists ( \
                   select 1 from discovery_favorite_items mine \
                   where mine.profile_id = $1 \
                     and mine.category = $2 \
                     and mine.catalog_item_id = f.catalog_item_id) \
             order by p.nickname, p.profile_id, f.position",
        )
        .bind(viewer_profile_id)
        .bind(query.category.column_value())
        .fetch_all(pool)
        .await
        .map_err(|_| DiscoveryError::Unavailable)?
        .into_iter()
        .map(|row: DiscoveryRow| Row {
            profile_id: row.0,
            nickname: row.1,
            profile_image_media_id: row.2,
            interaction_style: row.3,
            catalog_item_id: row.4,
            label: row.5,
            item_category: row.6,
        })
        .collect();
    // Fold rows into cards, preserving the stable profile order.
    let mut cards: Vec<PeopleCard> = Vec::new();
    for row in rows {
        if item_category(&row.item_category).is_empty() {
            return Err(DiscoveryError::Unavailable);
        }
        match cards.last_mut() {
            Some(card) if card.profile_id == row.profile_id => card.shared_items.push(FavoriteItem {
                catalog_item_id: row.catalog_item_id,
                label: row.label,
                category: item_category(&row.item_category),
                external_url: None,
            }),
            _ => cards.push(PeopleCard {
                profile_id: row.profile_id,
                nickname: row.nickname,
                profile_image_media_id: row.profile_image_media_id,
                shared_items: vec![FavoriteItem {
                    catalog_item_id: row.catalog_item_id,
                    label: row.label,
                    category: item_category(&row.item_category),
                    external_url: None,
                }],
                interaction_style: row.interaction_style,
            }),
        }
    }
    // Apply the keyset cursor on the stable ordering.
    let start = cursor.as_ref().map_or(0, |cursor| {
        cards
            .iter()
            .position(|card| {
                (card.nickname.as_str(), card.profile_id)
                    > (cursor.nickname.as_str(), cursor.id)
            })
            .unwrap_or(cards.len())
    });
    let mut page: Vec<PeopleCard> = cards.into_iter().skip(start).collect();
    let next_cursor = if page.len() as i64 > query.limit {
        page.truncate(query.limit as usize);
        let last = page.last().expect("a full page has a last card");
        Some(encode_cursor(&Cursor {
            nickname: last.nickname.clone(),
            id: last.profile_id,
        }))
    } else {
        None
    };
    Ok(PeoplePage {
        items: page,
        next_cursor,
    })
}
