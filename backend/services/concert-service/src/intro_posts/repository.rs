//! T-306: the Introduction page and detail queries.
//!
//! Both queries put the visibility predicate **inside** the SQL statement. The
//! page query fetches `limit + 1` rows that the predicate already admitted, so a
//! page is never shortened by post-filtering and the cursor is always derived
//! from the filtered set. This is the defect the product plan calls out
//! ("do not paginate on the server and then filter the returned page on the
//! client") and V-03-i is the case that proves it is absent.
//!
//! ## Cursor discipline
//!
//! The cursor is the foundation's authenticated cursor
//! ([`concert_application::intro::cursor`]): an HMAC over the complete request
//! binding and the row position, with a 15-minute lifetime. The binding includes
//! the viewer, so a cursor issued to viewer A is rejected for viewer B. The
//! binding also includes a query tag that folds in the audience-relevant parts of
//! the request, so a cursor cannot be replayed against a different filter set.
//!
//! The foundation's `validate_tag` pins the binding to the event-scoped
//! `listConcertIntros` operation. The profile product has no event and a
//! different operation name, so this module supplies its own tag validation and
//! reuses the encoding, the MAC and the window rules unchanged. The event field
//! of `CursorBinding` is filled with the profile product's fixed collection
//! epoch (all zeros is rejected by the encoder, so the page passes the viewer's
//! own profile id there and the tag validation checks it equals the viewer).
//!
//! ## What the page returns
//!
//! A row carries only what the viewer-facing projection needs. The audience is
//! **not** selected: `social.IntroPostDetail` deliberately has no audience field,
//! and a query that fetched one would be one serialization mistake away from
//! leaking it.
use chrono::{DateTime, Utc};
use concert_application::intro::cursor::{CursorBinding, CursorCodec, CursorWindow};
use concert_application::intro::{IntroError, Position};
use concert_application::intro_visibility::{PostState, Row};
use concert_domain::intro_audience::IntroAudience;
use sqlx::{PgPool, Row as _};
use uuid::Uuid;

use super::visibility_sql::{predicate, VisibilityParameters, VisibilityScope};

/// The default page size, from the frozen `social.IntroPostQuery`.
pub const DEFAULT_LIMIT: i64 = 20;
/// The maximum page size, from the frozen `social.IntroPostQuery`.
pub const MAX_LIMIT: i64 = 100;
/// The cursor lifetime, matching the foundation's 15-minute window.
pub const CURSOR_LIFETIME_MINUTES: i64 = 15;

/// The operation and collection this module's cursors are bound to.
pub const OPERATION: &str = "listIntroPosts";
pub const COLLECTION: &str = "intro-posts";

/// The longest search term the frozen `social.IntroPostQuery.q` admits.
pub const MAX_QUERY_CHARS: usize = 100;

/// A closed page query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntroPostQuery {
    pub limit: i64,
    pub cursor: Option<String>,
    /// The free-text filter. `None` means "no filter", which is different from
    /// `Some("")` — an empty term is refused rather than silently matching
    /// everything.
    pub q: Option<String>,
}

impl IntroPostQuery {
    pub fn new(limit: Option<i64>, cursor: Option<String>) -> Result<Self, IntroPageError> {
        Self::with_query(limit, cursor, None)
    }

    /// Build a query including the `q` term.
    ///
    /// `q` is trimmed and bounded to [`MAX_QUERY_CHARS`] Unicode code points, the
    /// contract's `maxLength: 100`. A term that is empty after trimming is
    /// `InvalidQuery("q")` rather than a filter that matches every row: a caller
    /// that sent `q=` asked for a search and would otherwise get an unfiltered
    /// page that looks like a successful search.
    pub fn with_query(
        limit: Option<i64>,
        cursor: Option<String>,
        q: Option<String>,
    ) -> Result<Self, IntroPageError> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT);
        if !(1..=MAX_LIMIT).contains(&limit) {
            return Err(IntroPageError::InvalidQuery("limit"));
        }
        let q = match q {
            None => None,
            Some(value) => {
                let trimmed = value.trim();
                if trimmed.is_empty() || trimmed.chars().count() > MAX_QUERY_CHARS {
                    return Err(IntroPageError::InvalidQuery("q"));
                }
                Some(trimmed.to_owned())
            }
        };
        Ok(Self {
            limit,
            cursor: cursor.filter(|value| !value.is_empty()),
            q,
        })
    }

    /// The query tag folded into the cursor binding.
    ///
    /// A cursor must not be replayable against a different filter set, so the
    /// presence and the content of `q` are part of the binding. The value is a
    /// stable 64-bit digest rather than the term itself, so the tag does not
    /// carry the viewer's search text in the token.
    pub fn tag(&self) -> u64 {
        match self.q.as_deref() {
            None => 0,
            Some(term) => {
                // FNV-1a, 64-bit. Stable across processes and platforms, which
                // is what a cursor tag needs; it is not a security primitive.
                let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
                for byte in term.as_bytes() {
                    hash ^= u64::from(*byte);
                    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
                }
                // `0` is reserved for "no filter", so a term that hashes to it is
                // mapped away rather than colliding with the unfiltered tag.
                if hash == 0 { 1 } else { hash }
            }
        }
    }
}

/// Why a page or detail read failed.
///
/// `Unavailable` is the only answer for a post the viewer may not see: it is
/// indistinguishable from a post that does not exist, which is what the HTTP
/// layer turns into `404 RESOURCE_UNAVAILABLE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntroPageError {
    /// The query itself is malformed (limit, cursor).
    InvalidQuery(&'static str),
    /// The cursor was issued for a different viewer, operation or window.
    InvalidCursor,
    /// The post does not exist, or the viewer may not see it.
    Unavailable,
    /// The store failed.
    Store,
}

impl std::fmt::Display for IntroPageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidQuery(field) => write!(formatter, "invalid query: {field}"),
            Self::InvalidCursor => formatter.write_str("invalid cursor"),
            Self::Unavailable => formatter.write_str("resource unavailable"),
            Self::Store => formatter.write_str("store unavailable"),
        }
    }
}

impl std::error::Error for IntroPageError {}

/// One row of the viewer-facing page.
///
/// Deliberately absent: the audience, the age bounds, the gender condition, the
/// author's account id, and anything about the viewer's own claims.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntroPostRow {
    pub intro_id: Uuid,
    pub author_profile_id: Uuid,
    pub body: String,
    pub currently_seeking: Option<String>,
    pub interaction_style: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    /// The position the cursor is derived from: `(published_at, intro_id)`.
    /// `None` for a row that was never published, which only the author's own
    /// detail read can return.
    pub position: Option<Position>,
    /// The author projection from `discovery_profiles`.
    ///
    /// `None` when the projection has no row for this author yet. The page keeps
    /// such a post and reports the author as unavailable; the detail query
    /// refuses it, because a detail without an author is not renderable.
    pub author: Option<IntroPostAuthor>,
    /// The stored favorites snapshot, as the author froze it.
    pub favorites_snapshot: serde_json::Value,
    /// How many likes the post has.
    pub like_count: i64,
    /// Whether the *viewer* of this read has liked it.
    pub liked_by_viewer: bool,
}

/// The author projection a viewer-facing row carries.
///
/// Deliberately absent: the author's account id, their claims, and anything the
/// profile service owns but the concert projection does not mirror.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntroPostAuthor {
    pub profile_id: Uuid,
    pub nickname: String,
    pub bio: Option<String>,
    pub profile_image_media_id: Option<Uuid>,
}

/// One viewer-facing detail.
///
/// The same row as the page, plus the two things only a detail shows: the
/// attachments in position order and the comment count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntroPostDetail {
    pub post: IntroPostRow,
    /// Attachment media ids in `position` order. At most four, per the schema.
    pub attachment_media_ids: Vec<Uuid>,
    pub comment_count: i64,
}

/// One page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntroPostPage {
    pub items: Vec<IntroPostRow>,
    pub next_cursor: Option<String>,
}

/// The cursor codec the page uses.
///
/// The foundation's [`concert_application::intro::cursor`] encodes a token as
/// `H(c)|T20|I36|B` and authenticates `T|I|B` with an HMAC, where `H` is a keyed
/// digest of the complete request binding and row position, `T` is the read
/// fence, `I` is the row position and `B` is the token lifetime. Its
/// `encode_authenticated`/`decode_authenticated` helpers pin the binding to the
/// event-scoped `listConcertIntros` operation, which the profile product is not.
///
/// This codec reuses the foundation's *format*, MAC construction, domain
/// separation, lifetime rule and constant-time comparison, and supplies the
/// profile product's binding: viewer-scoped, operation `listIntroPosts`,
/// collection `intro-posts`, and a query tag that folds in the page filter. The
/// token is therefore not portable between the two products, which is the point:
/// a cursor from one surface must never open on another.
pub struct IntroCursorCodec {
    key: [u8; 32],
}

impl IntroCursorCodec {
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }
}

impl CursorCodec for IntroCursorCodec {
    fn seal(
        &self,
        binding: &CursorBinding,
        window: CursorWindow,
        now: DateTime<Utc>,
    ) -> Result<String, IntroError> {
        validate_binding(binding)?;
        seal_cursor(&self.key, binding, window, now)
    }

    fn open(
        &self,
        binding: &CursorBinding,
        token: &str,
        now: DateTime<Utc>,
    ) -> Result<CursorWindow, IntroError> {
        validate_binding(binding)?;
        open_cursor(&self.key, binding, token, now)
    }
}

/// The token lifetime, matching the foundation's fifteen-minute window.
fn cursor_expiry(now: DateTime<Utc>) -> Result<DateTime<Utc>, IntroError> {
    now.checked_add_signed(chrono::Duration::minutes(CURSOR_LIFETIME_MINUTES))
        .ok_or(IntroError::Invalid)
}

/// The same wire form as the foundation's authenticated cursor.
fn seal_cursor(
    key: &[u8; 32],
    binding: &CursorBinding,
    window: CursorWindow,
    now: DateTime<Utc>,
) -> Result<String, IntroError> {
    // A window must satisfy the same invariants `open` enforces, or a token
    // could be sealed that no reader accepts. `upper` is the read fence and the
    // position it points past must be strictly older than it.
    if window.upper > now
        || window.after.is_some_and(|position| {
            position.intro_id.is_nil() || position.created_at > window.upper
        })
    {
        return Err(IntroError::Invalid);
    }
    let deadline = cursor_expiry(now)?;
    let body = format!(
        "{:020}|{}|{}|{:020}",
        window.upper.timestamp_micros(),
        encode_position(window.after),
        encode_binding_tag(binding),
        deadline.timestamp_micros()
    );
    let mac = cursor_mac(key, &body);
    Ok(format!("{mac}|{body}"))
}

fn open_cursor(
    key: &[u8; 32],
    binding: &CursorBinding,
    token: &str,
    now: DateTime<Utc>,
) -> Result<CursorWindow, IntroError> {
    if token.is_empty() || token.len() > concert_application::intro::INTRO_CURSOR_MAX {
        return Err(IntroError::Invalid);
    }
    let fields = token.split('|').collect::<Vec<_>>();
    // `H|T|I|A|E|O|C|Q|B`: the MAC, the read fence, the row position, the five
    // binding components and the lifetime.
    if fields.len() != 9 {
        return Err(IntroError::Invalid);
    }
    let mac = fields[0];
    let upper = fields[1];
    let position = fields[2];
    let query_tag = fields[3..8].join("|");
    let deadline = parse_micros(fields[8])?;
    if now > deadline || query_tag != encode_binding_tag(binding) {
        return Err(IntroError::Invalid);
    }
    // The tag is authenticated as one field; only split its components after the
    // MAC verifies, so differing UUID syntax cannot alter framing first.
    let expected = cursor_mac(
        key,
        &format!("{upper}|{position}|{query_tag}|{}", fields[8]),
    );
    if !constant_time_equal(mac.as_bytes(), expected.as_bytes()) {
        return Err(IntroError::Invalid);
    }
    let window = CursorWindow {
        upper: parse_micros(upper)?,
        after: decode_position(position)?,
    };
    if window.upper > now
        || window
            .after
            .is_some_and(|position| position.intro_id.is_nil() || position.created_at > window.upper)
    {
        return Err(IntroError::Invalid);
    }
    Ok(window)
}

fn encode_binding_tag(binding: &CursorBinding) -> String {
    let query = binding
        .query
        .map(|value| format!("{value:016x}"))
        .unwrap_or_else(|| "-".into());
    format!(
        "{}|{}|{}|{}|{}",
        binding.actor, binding.event, binding.operation, binding.collection, query
    )
}

fn encode_position(value: Option<Position>) -> String {
    value.map_or_else(
        || "-".to_owned(),
        |position| {
            format!(
                "{:020}:{}",
                position.created_at.timestamp_micros(),
                position.intro_id
            )
        },
    )
}

fn decode_position(value: &str) -> Result<Option<Position>, IntroError> {
    if value == "-" {
        return Ok(None);
    }
    let (time, id) = value.split_once(':').ok_or(IntroError::Invalid)?;
    Ok(Some(Position {
        created_at: parse_micros(time)?,
        intro_id: Uuid::parse_str(id).map_err(|_| IntroError::Invalid)?,
    }))
}

fn parse_micros(value: &str) -> Result<DateTime<Utc>, IntroError> {
    if value.len() != 20 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(IntroError::Invalid);
    }
    DateTime::from_timestamp_micros(value.parse().map_err(|_| IntroError::Invalid)?)
        .ok_or(IntroError::Invalid)
}

/// The foundation's MAC construction: HMAC-SHA256 over the domain separator and
/// the body, hex encoded.
///
/// `ring` is already a dependency of this service, so the MAC is computed with it
/// rather than pulling in a second HMAC implementation. The construction is the
/// foundation's: HMAC-SHA256 keyed with the cursor key, over
/// `INTRO_CURSOR_DOMAIN || body`.
fn cursor_mac(key: &[u8; 32], body: &str) -> String {
    let signing = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, key);
    let mut context = ring::hmac::Context::with_key(&signing);
    context.update(concert_application::intro::INTRO_CURSOR_DOMAIN);
    context.update(body.as_bytes());
    let bytes = context.sign().as_ref().to_vec();
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(64);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 15) as usize] as char);
    }
    encoded
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .fold(0_u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

/// The request binding for one (viewer, query) pair.
///
/// The foundation's `CursorBinding` carries an `event` field. The profile
/// product has no event, so the field carries the viewer's profile id and the
/// tag validation asserts the two are equal. That keeps the binding
/// viewer-scoped, which is the property the cursor must have, without inventing
/// a fake event.
pub fn binding(viewer_profile_id: Uuid, query: &IntroPostQuery) -> CursorBinding {
    CursorBinding {
        actor: viewer_profile_id,
        event: viewer_profile_id,
        operation: OPERATION,
        collection: COLLECTION,
        // The query tag folds in the filter set, so a cursor issued for one
        // search cannot be replayed against another. `0` means "no filter".
        query: Some(query.tag()),
    }
}

/// Validate a binding for this module's operation.
///
/// The foundation's `validate_tag` is hard-wired to the event operation; this is
/// the same check against the profile product's names.
pub fn validate_binding(binding: &CursorBinding) -> Result<(), IntroError> {
    // The query tag is checked for *presence* only. Its value is the filter set,
    // which `binding()` derives from the request; pinning it to a constant here
    // would refuse every filtered page. The tag is authenticated as part of the
    // MAC, so a caller cannot substitute a different one.
    if binding.actor.is_nil()
        || binding.event != binding.actor
        || binding.operation != OPERATION
        || binding.collection != COLLECTION
        || binding.query.is_none()
    {
        return Err(IntroError::Invalid);
    }
    Ok(())
}

/// The visibility scope for a request.
///
/// The caller builds this once: it resolves the viewer's blocks, the author
/// standing is a per-request fact only for the author's own management path (a
/// page mixes authors, so the page query checks standing per row through the
/// author's own local row), and the attributes come from the claim adapter.
///
/// Because a page mixes authors, the page query cannot take one author standing
/// as a parameter. It takes the viewer's own standing as "the author is active"
/// only for the author's own rows, and applies the author's standing through the
/// the deletion fence (see [`PAGE_SQL`]). The scope's `author_standing` is
/// therefore only used by the detail query, which has one author.
pub struct PageScope {
    pub viewer_profile_id: Uuid,
    pub blocks: concert_application::intro_visibility::BlockState,
    pub attributes: super::visibility_sql::ViewerAttributes,
    pub now: DateTime<Utc>,
}

impl PageScope {
    fn parameters(&self) -> VisibilityParameters {
        VisibilityScope::new(
            self.viewer_profile_id,
            self.blocks,
            // The page query replaces this with a per-row expression; see
            // `PAGE_SQL`. The value here is what the detail query uses.
            concert_application::intro_visibility::AuthorStanding::Active,
            self.attributes,
            self.now,
        )
        .parameters()
    }
}

/// The page query.
///
/// The predicate is `where`-clause text, so the `limit` is applied to the rows
/// the predicate admitted. `limit + 1` is fetched to prove a next page exists
/// without a second round trip.
///
/// The author-standing parameter of the shared predicate is `$3`. On a page the
/// author differs per row, so the query substitutes a per-row expression for it:
/// `not exists (select 1 from concert_world_account_deletion_fences f where
/// f.user_id = p.author_profile_id)`. The substitution is textual and
/// deliberate: it keeps one predicate definition (rows 1-12) while letting the
/// page evaluate row 4 from the table the concert service actually owns.
///
/// Every placeholder in these queries carries an explicit cast. SQLx sends
/// parameters as `unknown`-typed text, and PostgreSQL cannot infer a type from
/// `$1 is null` alone; the cast makes the statement preparable without relying on
/// a context PostgreSQL does not have.
///
/// The concert service has no local store of an author's account status or
/// sanctions; those live in the auth and moderation services. What it does own is
/// the permanent account-deletion fence, and a fenced author is exactly the
/// "author account is not active" case. A sanction is a per-request fact the
/// caller resolves and supplies through the scope, so the page query cannot
/// express it per row; the detail query can, and does. See
/// `docs/mcr/backend/wp04-claims.md` for the recorded limitation.
/// The columns every read selects.
///
/// The author projection comes from `discovery_profiles` (the concert-owned
/// projection, migration 0086), so no cross-database join is needed: the profile
/// service's own tables are never referenced. The like state and the like count
/// come from `intro_likes` in this database.
///
/// `coalesce` on the author columns: a published post whose author has no
/// projection row yet would otherwise drop out of the page through an inner
/// join. The projection is written by a worker, so a post can briefly precede its
/// author row; the page keeps the post and reports the author as unavailable
/// rather than hiding the post. The detail query refuses such a row instead,
/// because a detail without an author is not renderable.
const SELECT_COLUMNS: &str = "p.intro_id, p.author_profile_id, p.body, p.currently_seeking, \
     p.interaction_style, p.published_at, p.expires_at, \
     p.favorites_snapshot, \
     d.nickname as author_nickname, d.bio as author_bio, \
     d.profile_image_media_id as author_profile_image_media_id, \
     (select count(*) from intro_likes l where l.intro_id = p.intro_id) as like_count, \
     exists (select 1 from intro_likes lv \
             where lv.intro_id = p.intro_id \
               and lv.actor_profile_id = cast($1 as uuid)) as liked_by_viewer";

/// The page query.
///
/// The predicate is `where`-clause text, so the `limit` is applied to the rows
/// the predicate admitted. `limit + 1` is fetched to prove a next page exists
/// without a second round trip.
///
/// The `q` filter is applied in the same `where` clause as the predicate, so the
/// scan is filtered before `limit`. Filtering after the limit would return short
/// pages that still report a `next_cursor`, which is the V-03-i defect.
///
/// Placeholders are derived from [`VisibilityParameters::PLACEHOLDERS`] rather
/// than written out, so changing the predicate's arity cannot leave a stale
/// number behind: the predicate occupies `$1` through `$N`, the cursor position
/// takes `$N+1`/`$N+2`, and the search term takes `$N+3`.
pub const PAGE_SQL: &str = "select {columns} \
     from intro_posts p \
     left join discovery_profiles d on d.profile_id = p.author_profile_id \
     where {predicate} \
       and (cast({search} as text) is null \
            or p.body ilike ('%' || cast({search} as text) || '%') \
            or coalesce(p.currently_seeking, '') ilike ('%' || cast({search} as text) || '%') \
            or coalesce(p.interaction_style, '') ilike ('%' || cast({search} as text) || '%')) \
       and (cast({cursor_at} as timestamptz) is null \
            or (p.published_at, p.intro_id) < (cast({cursor_at} as timestamptz), cast({cursor_id} as uuid))) \
     order by p.published_at desc, p.intro_id desc \
     limit {limit}";

/// The detail query: the same predicate, one row.
///
/// The same projection and the same like state as the page. The author join is a
/// **left** join for the same reason as the page: a post can precede its author's
/// projection row, and the visibility decision must not depend on whether the
/// projection worker has caught up. A caller that needs a renderable author
/// checks `row.author` itself; the transport does, and refuses a detail without
/// one.
pub const DETAIL_SQL: &str = "select {columns}, \
     (select count(*) from intro_comments c \
      where c.intro_id = p.intro_id and c.state = 'PUBLISHED') as comment_count, \
     (select coalesce(json_agg(a.media_id order by a.position), '[]'::json) \
      from intro_attachments a where a.intro_id = p.intro_id) as attachment_media_ids \
     from intro_posts p \
     left join discovery_profiles d on d.profile_id = p.author_profile_id \
     where p.intro_id = cast({intro_id} as uuid) and {predicate}";

/// The author-standing substitution used by the page query.
///
/// The shared predicate uses `$3` (`author_active`) as a boolean. On a page it
/// becomes a per-row `exists` against the local standing table, so a page can
/// mix an active author and a sanctioned one and each row is decided correctly.
/// The substitution is done on the exact token `$3`, which the predicate emits
/// once.
pub fn page_predicate() -> String {
    let text = predicate(1);
    // The author-standing placeholder is the second one the fragment uses, after
    // the viewer. Deriving the number keeps this substitution correct when the
    // predicate's arity changes; hardcoding `$3` silently substituted a different
    // parameter the moment the block term stopped consuming a slot.
    let author_active = format!("${}", 1 + 1);
    let substituted = text.replacen(
        &author_active,
        "(not exists (select 1 from concert_world_account_deletion_fences f \
          where f.user_id = p.author_profile_id))",
        1,
    );
    debug_assert!(substituted.contains("concert_world_account_deletion_fences"));
    substituted
}

/// Build the page query text for a limit.
pub fn page_sql(limit: i64) -> String {
    let cursor_at = VisibilityParameters::PLACEHOLDERS + 1;
    let cursor_id = VisibilityParameters::PLACEHOLDERS + 2;
    let search = VisibilityParameters::PLACEHOLDERS + 3;
    PAGE_SQL
        .replace("{columns}", SELECT_COLUMNS)
        .replace("{predicate}", &page_predicate())
        .replace("{cursor_at}", &format!("${cursor_at}"))
        .replace("{cursor_id}", &format!("${cursor_id}"))
        .replace("{search}", &format!("${search}"))
        .replace("{limit}", &limit.to_string())
}

/// Build the detail query text.
pub fn detail_sql() -> String {
    let intro_id = VisibilityParameters::PLACEHOLDERS + 1;
    DETAIL_SQL
        .replace("{columns}", SELECT_COLUMNS)
        .replace("{predicate}", &predicate(1))
        .replace("{intro_id}", &format!("${intro_id}"))
}

/// The number of placeholders the detail query uses: the predicate's seven, then
/// the intro id. The like state reuses `$1` (the viewer), so it adds none.
pub const DETAIL_PLACEHOLDERS: usize = VisibilityParameters::PLACEHOLDERS + 1;

/// The placeholders the page query uses: the predicate's seven, then the cursor
/// pair. `$8` and `$9` are the cursor's `(published_at, intro_id)`.
pub const PAGE_PLACEHOLDERS: usize = VisibilityParameters::PLACEHOLDERS + 3;

/// Read one page, returning the driver error for diagnosis.
///
/// The production entry point maps every store failure to `Store`; this variant
/// exists so a test can assert on the actual PostgreSQL error rather than on the
/// opaque mapping.
pub async fn list_intro_posts_diagnostic(
    pool: &PgPool,
    codec: &dyn CursorCodec,
    scope: &PageScope,
    query: &IntroPostQuery,
) -> Result<IntroPostPage, String> {
    let binding = binding(scope.viewer_profile_id, query);
    validate_binding(&binding).map_err(|_| "invalid binding".to_owned())?;
    let window = match query.cursor.as_deref() {
        Some(token) => Some(
            codec
                .open(&binding, token, scope.now)
                .map_err(|error| format!("invalid cursor: {error:?}"))?,
        ),
        None => None,
    };
    let after = window.and_then(|window| window.after);
    let sql = page_sql(query.limit + 1);
    let parameters = scope.parameters();
    sqlx::query(&sql)
        .bind(parameters.viewer)
        .bind(parameters.author_active)
        .bind(parameters.age)
        .bind(parameters.gender)
        .bind(parameters.provider_answered)
        .bind(parameters.now)
        .bind(after.map(|position| position.created_at))
        .bind(after.map(|position| position.intro_id))
        .bind(query.q.as_deref())
        .fetch_all(pool)
        .await
        .map(|_| IntroPostPage {
            items: Vec::new(),
            next_cursor: None,
        })
        .map_err(|error| format!("{error} | sql: {sql}"))
}

/// Read one page.
///
/// The cursor is decoded against the viewer's binding before the query runs, so
/// a cursor issued to another viewer is refused without touching the store.
pub async fn list_intro_posts(
    pool: &PgPool,
    codec: &dyn CursorCodec,
    scope: &PageScope,
    query: &IntroPostQuery,
) -> Result<IntroPostPage, IntroPageError> {
    let binding = binding(scope.viewer_profile_id, query);
    validate_binding(&binding).map_err(|_| IntroPageError::InvalidCursor)?;
    let window = match query.cursor.as_deref() {
        Some(token) => Some(
            codec
                .open(&binding, token, scope.now)
                .map_err(|_| IntroPageError::InvalidCursor)?,
        ),
        None => None,
    };
    let after = window.and_then(|window| window.after);
    let sql = page_sql(query.limit + 1);
    let parameters = scope.parameters();
    let rows = sqlx::query(&sql)
        .bind(parameters.viewer)
        .bind(parameters.author_active)
        .bind(parameters.age)
        .bind(parameters.gender)
        .bind(parameters.provider_answered)
        .bind(parameters.now)
        .bind(after.map(|position| position.created_at))
        .bind(after.map(|position| position.intro_id))
        .bind(query.q.as_deref())
        .fetch_all(pool)
        .await
        .map_err(|_| IntroPageError::Store)?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(decode_row(&row)?);
    }
    // The predicate already filtered, so `limit + 1` rows means there is a next
    // page. The cursor is derived from the last row of *this* page, which is the
    // last row of the filtered set.
    let next_cursor = if items.len() as i64 > query.limit {
        items.truncate(query.limit as usize);
        let last = items.last().expect("a truncated page has a last row");
        // A page only contains published rows (row 2), so every row on it has a
        // position.
        let last_position = last
            .position
            .expect("a page row is published and therefore has a position");
        // The window's `upper` is the read fence: the newest instant the page
        // observed. It must not exceed `now`, and the position it points past
        // must not be later than it. A page is ordered by `(published_at,
        // intro_id)` descending and a row is published at or before `now`, so
        // `now` is both the newest possible publication instant and the read
        // instant. Using the row's own publication instant would fail the
        // foundation's `after.created_at > upper` check whenever the row is the
        // newest one on the page.
        //
        // The fence is the *request instant*, not the row's instant. The row
        // position is the cursor's `after`, and it must be strictly older than
        // the fence for the next page's `(published_at, intro_id) < ($8, $9)`
        // comparison to be exclusive of it.
        let upper = scope.now;
        Some(
            codec
                .seal(
                    &binding,
                    CursorWindow {
                        upper,
                        after: Some(Position {
                            created_at: last_position
                                .created_at
                                .min(scope.now - chrono::Duration::microseconds(1)),
                            intro_id: last_position.intro_id,
                        }),
                    },
                    scope.now,
                )
                .map_err(|_| IntroPageError::Store)?,
        )
    } else {
        None
    };
    Ok(IntroPostPage { items, next_cursor })
}

/// Read one post.
///
/// The same predicate is evaluated on the single row. A row that does not exist
/// and a row the viewer may not see produce the same `Unavailable`, so the caller
/// cannot distinguish them.
pub async fn get_intro_post(
    pool: &PgPool,
    scope: &VisibilityScope,
    intro_id: Uuid,
) -> Result<IntroPostDetail, IntroPageError> {
    if intro_id.is_nil() {
        return Err(IntroPageError::Unavailable);
    }
    let sql = detail_sql();
    let parameters = scope.parameters();
    let row = sqlx::query(&sql)
        .bind(parameters.viewer)
        .bind(parameters.author_active)
        .bind(parameters.age)
        .bind(parameters.gender)
        .bind(parameters.provider_answered)
        .bind(parameters.now)
        .bind(intro_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| IntroPageError::Store)?
        .ok_or(IntroPageError::Unavailable)?;
    let post = decode_row(&row)?;
    let attachment_media_ids: Vec<Uuid> = row
        .try_get::<serde_json::Value, _>("attachment_media_ids")
        .ok()
        .and_then(|value| value.as_array().cloned())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().and_then(|text| Uuid::parse_str(text).ok()))
                .collect()
        })
        .unwrap_or_default();
    let comment_count: i64 = row.try_get("comment_count").unwrap_or(0);
    Ok(IntroPostDetail {
        post,
        attachment_media_ids,
        comment_count,
    })
}

/// Decode one selected row.
fn decode_row(row: &sqlx::postgres::PgRow) -> Result<IntroPostRow, IntroPageError> {
    let intro_id: Uuid = row.try_get("intro_id").map_err(|_| IntroPageError::Store)?;
    let published_at: Option<DateTime<Utc>> = row
        .try_get("published_at")
        .map_err(|_| IntroPageError::Store)?;
    // A draft has no publication instant. The author's management path reads it,
    // so the row must decode; `published_at` is `None` for a draft and the
    // cursor position is then meaningless, which is why the page query never
    // returns a draft (row 2) and only the detail query can.
    let author_profile_id: Uuid = row
        .try_get("author_profile_id")
        .map_err(|_| IntroPageError::Store)?;
    // The author projection is optional: a post can precede its author's
    // projection row. `nickname` is the discriminator, because the join is a
    // left join and every other author column is nullable.
    let author = row
        .try_get::<Option<String>, _>("author_nickname")
        .ok()
        .flatten()
        .map(|nickname| IntroPostAuthor {
            profile_id: author_profile_id,
            nickname,
            bio: row.try_get("author_bio").ok().flatten(),
            profile_image_media_id: row
                .try_get("author_profile_image_media_id")
                .ok()
                .flatten(),
        });
    Ok(IntroPostRow {
        intro_id,
        author_profile_id,
        body: row.try_get("body").map_err(|_| IntroPageError::Store)?,
        currently_seeking: row.try_get("currently_seeking").ok(),
        interaction_style: row.try_get("interaction_style").ok(),
        published_at,
        expires_at: row.try_get("expires_at").ok(),
        position: published_at.map(|published_at| Position {
            created_at: published_at,
            intro_id,
        }),
        author,
        favorites_snapshot: row
            .try_get("favorites_snapshot")
            .unwrap_or(serde_json::Value::Array(Vec::new())),
        like_count: row.try_get("like_count").unwrap_or(0),
        liked_by_viewer: row.try_get("liked_by_viewer").unwrap_or(false),
    })
}

/// The row the decision table would report for a post, for the evidence record.
///
/// The SQL predicate answers allow/deny; this reports which row of the table the
/// pure function reached, so a test can name the reason a row was filtered out.
pub fn reported_row(
    scope: &VisibilityScope,
    intro_id: Uuid,
    author_profile_id: Uuid,
    state: PostState,
    audience: IntroAudience,
    expires_at: Option<DateTime<Utc>>,
) -> (concert_application::intro_visibility::Decision, Row) {
    super::visibility_sql::expected_row(
        scope,
        intro_id,
        author_profile_id,
        state,
        audience,
        expires_at,
    )
}
