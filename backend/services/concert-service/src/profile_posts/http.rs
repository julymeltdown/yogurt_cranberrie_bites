//! WP-06 T-504: the closed route parser for profile posts and profile media.
//!
//! Eight frozen operations arrive here (http-contract.md §2.3):
//!
//! | # | operationId | Method | Path |
//! | --- | --- | --- | --- |
//! | 2.3.1 | `listProfilePosts` | GET | `/api/profiles/{profileId}/posts` |
//! | 2.3.2 | `createProfilePost` | POST | `/api/me/posts` |
//! | 2.3.3 | `deleteProfilePost` | DELETE | `/api/me/posts/{postId}` |
//! | 2.3.4 | `initProfileMediaUpload` | POST | `/api/me/media/uploads` |
//! | 2.3.5 | `putProfileMediaBytes` | PUT | `/api/me/media/{mediaId}/bytes` |
//! | 2.3.6 | `finalizeProfileMedia` | POST | `/api/me/media/{mediaId}/finalize` |
//! | 2.3.7 | `getProfileMedia` | GET | `/api/media/{mediaId}` |
//! | 2.3.8 | `readProfileMedia` | GET | `/api/media/{mediaId}/content` |
//!
//! The table is **closed**: a `(method, path)` outside it is not a profile post
//! or media operation at all, so it is refused before any producer work. A path
//! naming an event is refused before the table is consulted, so a concert-shaped
//! request can never reach the profile producer even if a later edit added a
//! permissive entry.
//!
//! Every body limit is declared here rather than at the call site, so the
//! gateway and the producer read the same number.
use super::commands::{ProfilePostError, MAX_ATTACHMENTS, MAX_DESCRIPTION};
use super::read::{ProfilePostQuery, DEFAULT_LIMIT, MAX_LIMIT};
use concert_application::media::{Mime, Purpose, MAX_BYTES};
use uuid::Uuid;

/// The largest JSON body any profile post or media operation accepts.
///
/// `CreateProfilePost` is bounded by four UUIDs and a 500-code-point
/// description, so 64 KiB is generous; the limit exists so an oversized body is
/// refused by the transport before it is parsed.
pub const MAX_POST_BODY: usize = 65536;

/// The body limit of one route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyLimit {
    /// A read: no body at all.
    None,
    /// A JSON body, bounded.
    Json(usize),
    /// Raw image bytes, bounded by the foundation's media limit.
    Bytes(usize),
}

impl BodyLimit {
    /// The numeric limit, for a transport that only needs a number.
    pub fn bytes(self) -> usize {
        match self {
            Self::None => 0,
            Self::Json(limit) | Self::Bytes(limit) => limit,
        }
    }
    /// Whether this route carries raw image bytes rather than JSON.
    pub fn is_raw(self) -> bool {
        matches!(self, Self::Bytes(_))
    }
}

/// One frozen profile post or media operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfilePostRoute {
    /// `listProfilePosts`
    ListPosts { profile_id: Uuid },
    /// `createProfilePost`
    CreatePost,
    /// `deleteProfilePost`
    DeletePost { post_id: Uuid },
    /// `initProfileMediaUpload`
    InitMediaUpload,
    /// `putProfileMediaBytes`
    PutMediaBytes { media_id: Uuid },
    /// `finalizeProfileMedia`
    FinalizeMedia { media_id: Uuid },
    /// `getProfileMedia`
    GetMedia { media_id: Uuid },
    /// `readProfileMedia`
    ReadMedia { media_id: Uuid },
}

impl ProfilePostRoute {
    /// The frozen operation id.
    pub fn operation(self) -> &'static str {
        match self {
            Self::ListPosts { .. } => "listProfilePosts",
            Self::CreatePost => "createProfilePost",
            Self::DeletePost { .. } => "deleteProfilePost",
            Self::InitMediaUpload => "initProfileMediaUpload",
            Self::PutMediaBytes { .. } => "putProfileMediaBytes",
            Self::FinalizeMedia { .. } => "finalizeProfileMedia",
            Self::GetMedia { .. } => "getProfileMedia",
            Self::ReadMedia { .. } => "readProfileMedia",
        }
    }
    /// Whether this operation changes state.
    pub fn mutation(self) -> bool {
        matches!(
            self,
            Self::CreatePost
                | Self::DeletePost { .. }
                | Self::InitMediaUpload
                | Self::PutMediaBytes { .. }
                | Self::FinalizeMedia { .. }
        )
    }
    /// The body limit of this operation.
    pub fn body_limit(self) -> BodyLimit {
        match self {
            Self::ListPosts { .. }
            | Self::GetMedia { .. }
            | Self::ReadMedia { .. } => BodyLimit::None,
            Self::CreatePost | Self::InitMediaUpload | Self::FinalizeMedia { .. } => {
                BodyLimit::Json(MAX_POST_BODY)
            }
            Self::PutMediaBytes { .. } => BodyLimit::Bytes(MAX_BYTES),
            // A delete carries no body. Its revision arrives as a query.
            Self::DeletePost { .. } => BodyLimit::None,
        }
    }
    /// The upload context this operation belongs to, if it is an upload step.
    ///
    /// The profile-post path admits only `PROFILE_POST`; that is the
    /// purpose/context rule T-501 requires, expressed where the route is parsed.
    pub fn upload_context(self) -> Option<super::super::profile_media::UploadPurpose> {
        match self {
            Self::InitMediaUpload | Self::PutMediaBytes { .. } | Self::FinalizeMedia { .. } => {
                Some(super::super::profile_media::UploadPurpose::ProfilePost)
            }
            _ => None,
        }
    }
}

/// A refusal that must become a closed error envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteRefusal {
    pub status: u16,
    pub reason: &'static str,
}

impl RouteRefusal {
    fn invalid() -> Self {
        Self {
            status: 400,
            reason: "INVALID_INPUT",
        }
    }
    fn not_found() -> Self {
        Self {
            status: 404,
            reason: "RESOURCE_UNAVAILABLE",
        }
    }
    fn too_large() -> Self {
        Self {
            status: 413,
            reason: "PAYLOAD_TOO_LARGE",
        }
    }
    fn unsupported() -> Self {
        Self {
            status: 415,
            reason: "INVALID_MEDIA_TYPE",
        }
    }
}

impl From<ProfilePostError> for RouteRefusal {
    fn from(error: ProfilePostError) -> Self {
        Self {
            status: error.status(),
            reason: error.reason(),
        }
    }
}

/// Whether a raw target names a concert event path.
///
/// Checked on the raw target, before parsing, so a percent-encoded or nested
/// segment cannot slip past a later, narrower parser.
pub fn names_event_path(target: &str) -> bool {
    let lowered = target.to_ascii_lowercase();
    lowered.contains("/events/")
        || lowered.contains("%2fevents%2f")
        || lowered.contains("%2fevents/")
        || lowered.contains("/events%2f")
}

/// Percent-decode one path segment. A malformed escape is a client error.
fn decode_segment(value: &str) -> Result<String, RouteRefusal> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                let hex = bytes.get(index + 1..index + 3).ok_or_else(RouteRefusal::invalid)?;
                let text = std::str::from_utf8(hex).map_err(|_| RouteRefusal::invalid())?;
                let byte = u8::from_str_radix(text, 16).map_err(|_| RouteRefusal::invalid())?;
                out.push(byte);
                index += 3;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| RouteRefusal::invalid())
}

/// A canonical non-nil UUID. Upper case, braces and a urn prefix are refused, so
/// two spellings of one id cannot both be served.
fn canonical_uuid(value: &str) -> Result<Uuid, RouteRefusal> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or_else(RouteRefusal::invalid)
}

/// Whether the path is covered by the profile post/media surface at all.
///
/// Used by the transport to decide that a suffix no route matches must still
/// receive a closed profile error rather than falling through to another
/// handler's envelope.
pub fn protects(path: &str) -> bool {
    path.starts_with("/api/me/posts")
        || path.starts_with("/api/me/media/")
        || path.starts_with("/api/media/")
        || (path.starts_with("/api/profiles/") && path.ends_with("/posts"))
}

/// The closed route table, as `(method, segments)` with `:name` parameters.
///
/// Kept as data so a test can compare it with the frozen operation register.
pub const ROUTES: &[(&str, &str, &str)] = &[
    ("GET", "/api/profiles/:profile_id/posts", "listProfilePosts"),
    ("POST", "/api/me/posts", "createProfilePost"),
    ("DELETE", "/api/me/posts/:post_id", "deleteProfilePost"),
    ("POST", "/api/me/media/uploads", "initProfileMediaUpload"),
    ("PUT", "/api/me/media/:media_id/bytes", "putProfileMediaBytes"),
    (
        "POST",
        "/api/me/media/:media_id/finalize",
        "finalizeProfileMedia",
    ),
    ("GET", "/api/media/:media_id", "getProfileMedia"),
    ("GET", "/api/media/:media_id/content", "readProfileMedia"),
];

/// Parse a profile post or media target into its operation.
///
/// `target` is the request path plus query exactly as received. The query is
/// parsed per operation: `listProfilePosts` accepts `common.PageQuery`
/// (`limit`, `cursor`), `deleteProfilePost` accepts `common.RevisionRequest`
/// (`expected_revision`), and every other operation accepts no query at all — a
/// query on those is refused rather than ignored.
pub fn route(
    method: &str,
    target: &str,
) -> Result<(ProfilePostRoute, ProfilePostQuery), RouteRefusal> {
    let operation = parse_operation(method, target)?;
    let query = target
        .split_once('?')
        .map_or(None, |(_, query)| Some(query));
    let query = parse_query(operation, query)?;
    Ok((operation, query))
}

/// Parse only the operation of a target, without its query.
///
/// A delete carries its revision in the query, so its route must be resolvable
/// before the revision is read. Every other operation's query is validated by
/// [`route`].
pub fn parse_operation(method: &str, target: &str) -> Result<ProfilePostRoute, RouteRefusal> {
    if names_event_path(target) {
        return Err(RouteRefusal::invalid());
    }
    if target.len() > 4096 || target.chars().any(char::is_control) {
        return Err(RouteRefusal::invalid());
    }
    let (path, _query) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| (path, Some(query)));
    let segments: Vec<&str> = path.split('/').collect();
    let operation = match (method, segments.as_slice()) {
        ("GET", ["", "api", "profiles", profile_id, "posts"]) => ProfilePostRoute::ListPosts {
            profile_id: canonical_uuid(&decode_segment(profile_id)?)?,
        },
        ("POST", ["", "api", "me", "posts"]) => ProfilePostRoute::CreatePost,
        ("DELETE", ["", "api", "me", "posts", post_id]) => ProfilePostRoute::DeletePost {
            post_id: canonical_uuid(&decode_segment(post_id)?)?,
        },
        ("POST", ["", "api", "me", "media", "uploads"]) => ProfilePostRoute::InitMediaUpload,
        ("PUT", ["", "api", "me", "media", media_id, "bytes"]) => {
            ProfilePostRoute::PutMediaBytes {
                media_id: canonical_uuid(&decode_segment(media_id)?)?,
            }
        }
        ("POST", ["", "api", "me", "media", media_id, "finalize"]) => {
            ProfilePostRoute::FinalizeMedia {
                media_id: canonical_uuid(&decode_segment(media_id)?)?,
            }
        }
        ("GET", ["", "api", "media", media_id]) => ProfilePostRoute::GetMedia {
            media_id: canonical_uuid(&decode_segment(media_id)?)?,
        },
        ("GET", ["", "api", "media", media_id, "content"]) => ProfilePostRoute::ReadMedia {
            media_id: canonical_uuid(&decode_segment(media_id)?)?,
        },
        _ => return Err(RouteRefusal::not_found()),
    };
    Ok(operation)
}

/// Parse the query of one operation.
///
/// Only `listProfilePosts` carries a query. The `expected_revision` of a delete
/// is read by [`delete_revision`] instead, so a caller cannot confuse a page size
/// with a revision.
fn parse_query(
    operation: ProfilePostRoute,
    query: Option<&str>,
) -> Result<ProfilePostQuery, RouteRefusal> {
    let mut limit = DEFAULT_LIMIT;
    let mut limit_seen = false;
    let mut cursor = None;
    if let Some(query) = query {
        if query.is_empty() {
            return Ok(ProfilePostQuery { limit, cursor });
        }
        if matches!(operation, ProfilePostRoute::DeletePost { .. }) {
            // A delete's only query field is `expected_revision`, read by
            // `delete_revision`. The page query is not part of this operation.
            return Ok(ProfilePostQuery { limit, cursor });
        }
        // A delete carries its revision in the query; every other non-list
        // operation carries none.
        if !matches!(
            operation,
            ProfilePostRoute::ListPosts { .. } | ProfilePostRoute::DeletePost { .. }
        ) {
            return Err(RouteRefusal::invalid());
        }
        for pair in query.split('&') {
            if pair.is_empty() {
                continue;
            }
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            let name = decode_segment(name)?;
            let value = decode_segment(value)?;
            match name.as_str() {
                "limit" => {
                    if limit_seen {
                        // A repeated field is ambiguous, never "last one wins".
                        return Err(RouteRefusal::invalid());
                    }
                    limit_seen = true;
                    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                        return Err(RouteRefusal::invalid());
                    }
                    let parsed: i64 = value.parse().map_err(|_| RouteRefusal::invalid())?;
                    if !(1..=MAX_LIMIT).contains(&parsed) {
                        return Err(RouteRefusal::invalid());
                    }
                    limit = parsed;
                }
                "cursor" => {
                    if cursor.is_some() {
                        return Err(RouteRefusal::invalid());
                    }
                    cursor = Some(value);
                }
                _ => return Err(RouteRefusal::invalid()),
            }
        }
    }
    Ok(ProfilePostQuery { limit, cursor })
}

/// The `expected_revision` of a `deleteProfilePost` request.
///
/// `common.RevisionRequest` is a body schema, but the frozen operation is a
/// bodyless `DELETE`, so the revision arrives as the single query field. A
/// missing, repeated or out-of-range field is a `400`.
pub fn delete_revision(target: &str) -> Result<i64, RouteRefusal> {
    let (_, query) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| (path, Some(query)));
    let Some(query) = query else {
        return Err(RouteRefusal::invalid());
    };
    let mut revision = None;
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        if decode_segment(name)? != "expected_revision" || revision.is_some() {
            return Err(RouteRefusal::invalid());
        }
        let value = decode_segment(value)?;
        if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
            return Err(RouteRefusal::invalid());
        }
        let parsed: i64 = value.parse().map_err(|_| RouteRefusal::invalid())?;
        if !(1..=9007199254740991).contains(&parsed) {
            return Err(RouteRefusal::invalid());
        }
        revision = Some(parsed);
    }
    revision.ok_or_else(RouteRefusal::invalid)
}

/// Parse a `feed.InitUpload` body for the profile upload path.
///
/// The purpose must be admitted by the upload context. A `ROOM_POSTER` or an
/// `INTRO_POST` purpose sent to the profile-post upload path is
/// `400 INVALID_PURPOSE` — the purpose/context mismatch T-501 requires — and
/// nothing is written.
pub fn parse_init_upload(
    bytes: &[u8],
    context: super::super::profile_media::UploadPurpose,
) -> Result<concert_application::media::InitUpload, RouteRefusal> {
    use concert_application::media::InitUpload;
    reject_duplicate_keys(bytes)?;
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| RouteRefusal::invalid())?;
    let object = value.as_object().ok_or_else(RouteRefusal::invalid)?;
    const FIELDS: [&str; 4] = ["purpose", "content_type", "byte_length", "sha256"];
    if object.len() != FIELDS.len() || FIELDS.iter().any(|field| !object.contains_key(*field)) {
        return Err(RouteRefusal::invalid());
    }
    let purpose = Purpose::parse(object["purpose"].as_str().ok_or_else(RouteRefusal::invalid)?)
        .map_err(|_| RouteRefusal::invalid())?;
    if !context.admits(purpose) {
        return Err(RouteRefusal {
            status: 400,
            reason: "INVALID_PURPOSE",
        });
    }
    let mime = Mime::parse(
        object["content_type"]
            .as_str()
            .ok_or_else(RouteRefusal::invalid)?,
    )
    .map_err(|_| RouteRefusal::unsupported())?;
    let byte_length = object["byte_length"]
        .as_u64()
        .ok_or_else(RouteRefusal::invalid)?;
    if byte_length == 0 {
        return Err(RouteRefusal::invalid());
    }
    if byte_length > MAX_BYTES as u64 {
        return Err(RouteRefusal::too_large());
    }
    let sha256 = object["sha256"].as_str().ok_or_else(RouteRefusal::invalid)?;
    if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(RouteRefusal::invalid());
    }
    let mut digest = [0u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&sha256[index * 2..index * 2 + 2], 16)
            .map_err(|_| RouteRefusal::invalid())?;
    }
    Ok(InitUpload {
        purpose,
        mime,
        byte_length: byte_length as usize,
        sha256: digest,
    })
}

/// Parse a `feed.FinalizeUpload` body: exactly `{ "expected_revision": n }`.
pub fn parse_finalize_upload(bytes: &[u8]) -> Result<i64, RouteRefusal> {
    reject_duplicate_keys(bytes)?;
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| RouteRefusal::invalid())?;
    let object = value.as_object().ok_or_else(RouteRefusal::invalid)?;
    if object.len() != 1 || !object.contains_key("expected_revision") {
        return Err(RouteRefusal::invalid());
    }
    let revision = object["expected_revision"]
        .as_i64()
        .ok_or_else(RouteRefusal::invalid)?;
    if !(1..=9007199254740991).contains(&revision) {
        return Err(RouteRefusal::invalid());
    }
    Ok(revision)
}

/// Refuse a JSON object that repeats a key.
///
/// `serde_json` keeps the last occurrence silently, which would let a caller
/// send `{"expected_revision":1,"expected_revision":2}` and have one value
/// ignored. A closed schema means exactly one of each field, so a repeat is a
/// client error. Nested objects are checked too.
fn reject_duplicate_keys(bytes: &[u8]) -> Result<(), RouteRefusal> {
    #[derive(Clone, Copy)]
    enum Frame {
        Object,
        Array,
    }
    let mut stack: Vec<(Frame, std::collections::BTreeSet<String>)> = Vec::new();
    let mut current = String::new();
    let mut pending_key: Option<String> = None;
    let mut expecting_key = false;
    let mut parser = serde_json::Deserializer::from_slice(bytes).into_iter::<serde_json::Value>();
    // A raw scanner is simpler than a full JSON parser: walk the bytes, track
    // object depth and string boundaries, and record each top-level key.
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                let start = index + 1;
                let mut end = start;
                while end < bytes.len() {
                    match bytes[end] {
                        b'\\' => end += 2,
                        b'"' => break,
                        _ => end += 1,
                    }
                }
                if end >= bytes.len() {
                    return Err(RouteRefusal::invalid());
                }
                let text = std::str::from_utf8(&bytes[start..end])
                    .map_err(|_| RouteRefusal::invalid())?;
                if expecting_key {
                    current = text.to_owned();
                    pending_key = Some(current.clone());
                }
                index = end + 1;
                continue;
            }
            b'{' => {
                stack.push((Frame::Object, std::collections::BTreeSet::new()));
                expecting_key = true;
                index += 1;
                continue;
            }
            b'[' => {
                stack.push((Frame::Array, std::collections::BTreeSet::new()));
                expecting_key = false;
                index += 1;
                continue;
            }
            b'}' | b']' => {
                stack.pop();
                expecting_key = false;
                index += 1;
                continue;
            }
            b':' => {
                if let (Some((Frame::Object, keys)), Some(key)) = (stack.last_mut(), pending_key.take()) {
                    if !keys.insert(key) {
                        return Err(RouteRefusal::invalid());
                    }
                }
                expecting_key = false;
                index += 1;
                continue;
            }
            b',' => {
                expecting_key = matches!(stack.last(), Some((Frame::Object, _)));
                index += 1;
                continue;
            }
            _ => {
                index += 1;
            }
        }
    }
    // The scan is only a duplicate check; `serde_json` still parses the value.
    let _ = &mut parser;
    Ok(())
}

/// The content type of a `putProfileMediaBytes` request.
///
/// Only the three image types the foundation supports are accepted. GIF and
/// video are refused with `415`, before any byte is stored.
pub fn parse_bytes_content_type(value: &str) -> Result<Mime, RouteRefusal> {
    Mime::parse(value).map_err(|_| RouteRefusal::unsupported())
}

/// The maximum number of attachments a create body may name.
pub const ATTACHMENT_LIMIT: usize = MAX_ATTACHMENTS;
/// The maximum description length a create body may carry.
pub const DESCRIPTION_LIMIT: usize = MAX_DESCRIPTION;

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &str = "0f0e0d0c-0b0a-4908-8706-050403020100";
    const MEDIA: &str = "1a1b1c1d-1e1f-4a2b-8c3d-4e5f60718293";
    const POST: &str = "2a2b2c2d-2e2f-4a3b-8c4d-5e6f708192a3";

    #[test]
    fn every_frozen_operation_is_reachable_and_nothing_else_is() {
        for (method, template, operation) in ROUTES {
            let concrete = template
                .replace(":profile_id", PROFILE)
                .replace(":media_id", MEDIA)
                .replace(":post_id", POST);
            let (parsed, _) = route(method, &concrete).expect(template);
            assert_eq!(parsed.operation(), *operation, "{concrete}");
            // A suffix is never matched.
            assert_eq!(
                route(method, &format!("{concrete}/extra")),
                Err(RouteRefusal::not_found()),
                "{concrete}/extra"
            );
        }
        assert_eq!(ROUTES.len(), 8);
    }

    #[test]
    fn an_event_path_is_refused_before_the_table() {
        let event = "3a3b3c3d-3e3f-4a4b-8c5d-6e7f8091a2b3";
        for target in [
            format!("/api/profiles/events/{event}/posts"),
            format!("/api/me/posts/events/{event}"),
            format!("/api/media/events%2F{event}/content"),
            format!("/api/media/%2Fevents%2F{event}/content"),
        ] {
            assert_eq!(
                route("GET", &target),
                Err(RouteRefusal::invalid()),
                "{target}"
            );
        }
    }

    #[test]
    fn a_non_canonical_uuid_is_refused() {
        let upper = MEDIA.to_ascii_uppercase();
        assert_eq!(
            route("GET", &format!("/api/media/{upper}")),
            Err(RouteRefusal::invalid())
        );
        assert_eq!(
            route("GET", &format!("/api/media/{{{MEDIA}}}")),
            Err(RouteRefusal::invalid())
        );
        assert_eq!(
            route("GET", "/api/media/00000000-0000-0000-0000-000000000000"),
            Err(RouteRefusal::invalid())
        );
    }

    #[test]
    fn body_limits_are_declared_per_operation() {
        assert_eq!(
            ProfilePostRoute::PutMediaBytes {
                media_id: Uuid::nil()
            }
            .body_limit(),
            BodyLimit::Bytes(MAX_BYTES)
        );
        assert_eq!(ProfilePostRoute::CreatePost.body_limit(), BodyLimit::Json(MAX_POST_BODY));
        assert_eq!(
            ProfilePostRoute::ListPosts {
                profile_id: Uuid::nil()
            }
            .body_limit(),
            BodyLimit::None
        );
        assert_eq!(
            ProfilePostRoute::DeletePost {
                post_id: Uuid::nil()
            }
            .body_limit(),
            BodyLimit::None
        );
    }

    #[test]
    fn the_page_query_is_closed_and_bounded() {
        let (_, query) =
            route("GET", &format!("/api/profiles/{PROFILE}/posts?limit=5")).unwrap();
        assert_eq!(query.limit, 5);
        for bad in ["limit=0", "limit=101", "limit=+5", "sort=score", "limit=5&limit=6"] {
            assert_eq!(
                route("GET", &format!("/api/profiles/{PROFILE}/posts?{bad}")),
                Err(RouteRefusal::invalid()),
                "{bad}"
            );
        }
    }

    #[test]
    fn only_the_list_operation_accepts_a_query() {
        assert_eq!(
            route("GET", &format!("/api/media/{MEDIA}?limit=5")),
            Err(RouteRefusal::invalid())
        );
        assert_eq!(
            route("GET", &format!("/api/media/{MEDIA}/content?x=1")),
            Err(RouteRefusal::invalid())
        );
        assert_eq!(
            route("POST", "/api/me/media/uploads?x=1"),
            Err(RouteRefusal::invalid())
        );
    }

    #[test]
    fn a_delete_revision_is_required_and_bounded() {
        let target = format!("/api/me/posts/{POST}");
        assert!(delete_revision(&target).is_err());
        assert_eq!(
            delete_revision(&format!("{target}?expected_revision=3")),
            Ok(3)
        );
        for bad in ["expected_revision=0", "expected_revision=x", "expected_revision=1&expected_revision=2", "other=1"] {
            assert!(delete_revision(&format!("{target}?{bad}")).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_profile_upload_path_admits_only_profile_post() {
        use super::super::super::profile_media::UploadPurpose;
        let body = |purpose: &str| {
            serde_json::to_vec(&serde_json::json!({
                "purpose": purpose,
                "content_type": "image/png",
                "byte_length": 12,
                "sha256": "aa".repeat(32),
            }))
            .unwrap()
        };
        assert!(parse_init_upload(&body("PROFILE_POST"), UploadPurpose::ProfilePost).is_ok());
        for purpose in ["ROOM_POSTER", "INTRO_POST", "PROFILE_IMAGE", "POST", "GIVEAWAY"] {
            assert_eq!(
                parse_init_upload(&body(purpose), UploadPurpose::ProfilePost),
                Err(RouteRefusal {
                    status: 400,
                    reason: "INVALID_PURPOSE"
                }),
                "{purpose}"
            );
        }
    }

    #[test]
    fn only_the_three_image_types_are_accepted_for_bytes() {
        for accepted in ["image/jpeg", "image/png", "image/webp"] {
            assert!(parse_bytes_content_type(accepted).is_ok(), "{accepted}");
        }
        for refused in ["image/gif", "video/mp4", "image/avif", "text/plain"] {
            assert_eq!(
                parse_bytes_content_type(refused),
                Err(RouteRefusal {
                    status: 415,
                    reason: "INVALID_MEDIA_TYPE"
                }),
                "{refused}"
            );
        }
    }
}
