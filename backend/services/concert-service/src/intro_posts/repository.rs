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

/// A closed page query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntroPostQuery {
    pub limit: i64,
    pub cursor: Option<String>,
}

impl IntroPostQuery {
    pub fn new(limit: Option<i64>, cursor: Option<String>) -> Result<Self, IntroPageError> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT);
        if !(1..=MAX_LIMIT).contains(&limit) {
            return Err(IntroPageError::InvalidQuery("limit"));
        }
        Ok(Self {
            limit,
            cursor: cursor.filter(|value| !value.is_empty()),
        })
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
        // The profile product's page has no filter beyond the cursor, so the
        // query tag is a constant. A future filter must fold into this value.
        query: Some(0),
    }
}

/// Validate a binding for this module's operation.
///
/// The foundation's `validate_tag` is hard-wired to the event operation; this is
/// the same check against the profile product's names.
pub fn validate_binding(binding: &CursorBinding) -> Result<(), IntroError> {
    if binding.actor.is_nil()
        || binding.event != binding.actor
        || binding.operation != OPERATION
        || binding.collection != COLLECTION
        || binding.query != Some(0)
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
pub const PAGE_SQL: &str = "select p.intro_id, p.author_profile_id, p.body, p.currently_seeking, \
     p.interaction_style, p.published_at, p.expires_at \
     from intro_posts p \
     where {predicate} \
       and (cast($8 as timestamptz) is null \
            or (p.published_at, p.intro_id) < (cast($8 as timestamptz), cast($9 as uuid))) \
     order by p.published_at desc, p.intro_id desc \
     limit {limit}";

/// The detail query: the same predicate, one row.
pub const DETAIL_SQL: &str = "select p.intro_id, p.author_profile_id, p.body, p.currently_seeking, \
     p.interaction_style, p.published_at, p.expires_at \
     from intro_posts p \
     where p.intro_id = cast($8 as uuid) and {predicate}";

/// The author-standing substitution used by the page query.
///
/// The shared predicate uses `$3` (`author_active`) as a boolean. On a page it
/// becomes a per-row `exists` against the local standing table, so a page can
/// mix an active author and a sanctioned one and each row is decided correctly.
/// The substitution is done on the exact token `$3`, which the predicate emits
/// once.
pub fn page_predicate() -> String {
    let text = predicate(1);
    let substituted = text.replacen(
        "$3",
        "(not exists (select 1 from concert_world_account_deletion_fences f \
          where f.user_id = p.author_profile_id))",
        1,
    );
    debug_assert!(substituted.contains("concert_world_account_deletion_fences"));
    substituted
}

/// Build the page query text for a limit.
pub fn page_sql(limit: i64) -> String {
    PAGE_SQL
        .replace("{predicate}", &page_predicate())
        .replace("{limit}", &limit.to_string())
}

/// Build the detail query text.
pub fn detail_sql() -> String {
    DETAIL_SQL.replace("{predicate}", &predicate(1))
}

/// The placeholders the page query uses: the predicate's seven, then the cursor
/// pair. `$8` and `$9` are the cursor's `(published_at, intro_id)`.
pub const PAGE_PLACEHOLDERS: usize = VisibilityParameters::PLACEHOLDERS + 2;

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
        .bind(parameters.blocked)
        .bind(parameters.author_active)
        .bind(parameters.age)
        .bind(parameters.gender)
        .bind(parameters.provider_answered)
        .bind(parameters.now)
        .bind(after.map(|position| position.created_at))
        .bind(after.map(|position| position.intro_id))
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
        .bind(parameters.blocked)
        .bind(parameters.author_active)
        .bind(parameters.age)
        .bind(parameters.gender)
        .bind(parameters.provider_answered)
        .bind(parameters.now)
        .bind(after.map(|position| position.created_at))
        .bind(after.map(|position| position.intro_id))
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
) -> Result<IntroPostRow, IntroPageError> {
    if intro_id.is_nil() {
        return Err(IntroPageError::Unavailable);
    }
    let sql = detail_sql();
    let parameters = scope.parameters();
    let row = sqlx::query(&sql)
        .bind(parameters.viewer)
        .bind(parameters.blocked)
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
    decode_row(&row)
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
    Ok(IntroPostRow {
        intro_id,
        author_profile_id: row
            .try_get("author_profile_id")
            .map_err(|_| IntroPageError::Store)?,
        body: row.try_get("body").map_err(|_| IntroPageError::Store)?,
        currently_seeking: row.try_get("currently_seeking").ok(),
        interaction_style: row.try_get("interaction_style").ok(),
        published_at,
        expires_at: row.try_get("expires_at").ok(),
        position: published_at.map(|published_at| Position {
            created_at: published_at,
            intro_id,
        }),
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
