//! T-405: the closed route parser for the 13 frozen intro operations.
//!
//! The parser is *closed* in three directions, and each is a test:
//!
//! * an unknown path is refused with `permission_denied`, never forwarded;
//! * an unknown query field is refused with `invalid_argument`, never ignored —
//!   an ignored field is a lie, because a client that sends `sort=likes` and
//!   gets an unsorted page believes it was sorted;
//! * a method mismatch is refused, never allowed to fall through to another
//!   handler.
//!
//! The operations are the `social.*` rows of `contracts/domain/v1/operations.json`
//! §2.2. The parser produces a [`IntroOperation`], which the producer matches on;
//! there is no generic dispatch.
use std::collections::BTreeMap;
use tonic::Status;
use uuid::Uuid;

/// The 13 frozen intro operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntroOperation {
    /// `listIntroPosts` — `GET /api/discover/intro-posts`
    ListIntroPosts,
    /// `getIntroPost` — `GET /api/intro-posts/{introId}`
    GetIntroPost(Uuid),
    /// `getOwnIntroPost` — `GET /api/me/intro-posts/{introId}`
    GetOwnIntroPost(Uuid),
    /// `listOwnIntroPosts` — `GET /api/me/intro-posts`
    ListOwnIntroPosts,
    /// `saveIntroPostDraft` — `PUT /api/me/intro-posts/{introId}`
    SaveIntroPostDraft(Uuid),
    /// `publishIntroPost` — `POST /api/me/intro-posts/{introId}/publish`
    PublishIntroPost(Uuid),
    /// `deleteIntroPost` — `DELETE /api/me/intro-posts/{introId}`
    DeleteIntroPost(Uuid),
    /// `setIntroPostLike` — `PUT /api/intro-posts/{introId}/like`
    SetIntroPostLike(Uuid),
    /// `clearIntroPostLike` — `DELETE /api/intro-posts/{introId}/like`
    ClearIntroPostLike(Uuid),
    /// `listIntroPostLikers` — `GET /api/intro-posts/{introId}/likes`
    ListIntroPostLikers(Uuid),
    /// `listIntroPostComments` — `GET /api/intro-posts/{introId}/comments`
    ListIntroPostComments(Uuid),
    /// `createIntroPostComment` — `POST /api/intro-posts/{introId}/comments`
    CreateIntroPostComment(Uuid),
    /// `deleteIntroPostComment` — `DELETE /api/intro-posts/{introId}/comments/{commentId}`
    DeleteIntroPostComment(Uuid, Uuid),
}

impl IntroOperation {
    /// The frozen operation id.
    pub fn contract_id(self) -> &'static str {
        match self {
            Self::ListIntroPosts => "listIntroPosts",
            Self::GetIntroPost(_) => "getIntroPost",
            Self::GetOwnIntroPost(_) => "getOwnIntroPost",
            Self::ListOwnIntroPosts => "listOwnIntroPosts",
            Self::SaveIntroPostDraft(_) => "saveIntroPostDraft",
            Self::PublishIntroPost(_) => "publishIntroPost",
            Self::DeleteIntroPost(_) => "deleteIntroPost",
            Self::SetIntroPostLike(_) => "setIntroPostLike",
            Self::ClearIntroPostLike(_) => "clearIntroPostLike",
            Self::ListIntroPostLikers(_) => "listIntroPostLikers",
            Self::ListIntroPostComments(_) => "listIntroPostComments",
            Self::CreateIntroPostComment(_) => "createIntroPostComment",
            Self::DeleteIntroPostComment(_, _) => "deleteIntroPostComment",
        }
    }

    /// Whether the operation mutates state. A mutation carries an idempotency
    /// key and an `X-MCR-Intent`; a read carries neither.
    pub fn mutation(self) -> bool {
        matches!(
            self,
            Self::SaveIntroPostDraft(_)
                | Self::PublishIntroPost(_)
                | Self::DeleteIntroPost(_)
                | Self::SetIntroPostLike(_)
                | Self::ClearIntroPostLike(_)
                | Self::CreateIntroPostComment(_)
                | Self::DeleteIntroPostComment(_, _)
        )
    }

    /// The query fields this operation accepts, and nothing else.
    fn fields(self) -> &'static [&'static str] {
        match self {
            Self::ListIntroPosts => &["limit", "cursor", "q"],
            Self::ListOwnIntroPosts => &["limit", "cursor", "state"],
            Self::ListIntroPostLikers(_) => &["limit", "cursor"],
            Self::ListIntroPostComments(_) => &["limit", "cursor"],
            Self::DeleteIntroPost(_) => &["expected_revision"],
            _ => &[],
        }
    }

}

/// The query values the parser extracted. Only the fields the operation
/// declares are ever populated, so a producer cannot read a field its operation
/// does not accept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    pub limit: Option<i64>,
    pub cursor: Option<String>,
    pub q: Option<String>,
    pub state: Option<String>,
    pub expected_revision: Option<i64>,
}

/// A parsed request: the operation plus its query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parsed {
    pub operation: IntroOperation,
    pub query: Query,
}

impl Parsed {
    pub fn contract_id(&self) -> &'static str {
        self.operation.contract_id()
    }
    pub fn mutation(&self) -> bool {
        self.operation.mutation()
    }
}

/// A canonical lowercase non-nil UUID, exactly as the contract spells it.
fn uuid(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or_else(|| Status::invalid_argument("Invalid intro identifier"))
}

/// Percent-decode one query component. A malformed escape is a client error.
fn decode(value: &str) -> Result<String, Status> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = bytes
                .get(index + 1..index + 3)
                .ok_or_else(|| Status::invalid_argument("Invalid intro query"))?;
            let text = std::str::from_utf8(hex)
                .map_err(|_| Status::invalid_argument("Invalid intro query"))?;
            let byte = u8::from_str_radix(text, 16)
                .map_err(|_| Status::invalid_argument("Invalid intro query"))?;
            out.push(byte);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).map_err(|_| Status::invalid_argument("Invalid intro query"))
}

/// Parse a request target into an operation and its query.
pub fn parse(method: &str, target: &str) -> Result<Parsed, Status> {
    if target.len() > 4096 || target.chars().any(char::is_control) {
        return Err(Status::invalid_argument("Invalid intro target"));
    }
    // Intro operations are profile-owned: they never carry an event path.
    if target.contains("/events/") {
        return Err(Status::invalid_argument("Intro operations never carry an event path"));
    }
    let (path, raw_query) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| (path, Some(query)));
    if !path.starts_with("/api/") {
        return Err(Status::permission_denied("Unsupported intro operation"));
    }
    let operation = match (method, path) {
        ("GET", "/api/discover/intro-posts") => IntroOperation::ListIntroPosts,
        ("GET", "/api/me/intro-posts") => IntroOperation::ListOwnIntroPosts,
        ("GET", rest) => {
            if let Some(id) = rest.strip_prefix("/api/me/intro-posts/") {
                if id.contains('/') {
                    return Err(Status::permission_denied("Unsupported intro operation"));
                }
                IntroOperation::GetOwnIntroPost(uuid(id)?)
            } else if let Some(rest) = rest.strip_prefix("/api/intro-posts/") {
                match rest.split_once('/') {
                    None => IntroOperation::GetIntroPost(uuid(rest)?),
                    // `GET .../like` is not an operation: a like is set with PUT
                    // and cleared with DELETE. Matching it here would let a read
                    // reach a mutation handler.
                    Some((id, "likes")) => IntroOperation::ListIntroPostLikers(uuid(id)?),
                    Some((id, "comments")) => IntroOperation::ListIntroPostComments(uuid(id)?),
                    Some(_) => return Err(Status::permission_denied("Unsupported intro operation")),
                }
            } else {
                return Err(Status::permission_denied("Unsupported intro operation"));
            }
        }
        ("PUT", rest) => {
            if let Some(id) = rest.strip_prefix("/api/me/intro-posts/") {
                if id.contains('/') {
                    return Err(Status::permission_denied("Unsupported intro operation"));
                }
                IntroOperation::SaveIntroPostDraft(uuid(id)?)
            } else if let Some(rest) = rest.strip_prefix("/api/intro-posts/") {
                match rest.strip_suffix("/like") {
                    Some(id) if !id.contains('/') => IntroOperation::SetIntroPostLike(uuid(id)?),
                    _ => return Err(Status::permission_denied("Unsupported intro operation")),
                }
            } else {
                return Err(Status::permission_denied("Unsupported intro operation"));
            }
        }
        ("POST", rest) => {
            if let Some(rest) = rest.strip_prefix("/api/me/intro-posts/") {
                match rest.strip_suffix("/publish") {
                    Some(id) if !id.contains('/') => IntroOperation::PublishIntroPost(uuid(id)?),
                    _ => return Err(Status::permission_denied("Unsupported intro operation")),
                }
            } else if let Some(rest) = rest.strip_prefix("/api/intro-posts/") {
                match rest.strip_suffix("/comments") {
                    Some(id) if !id.contains('/') => {
                        IntroOperation::CreateIntroPostComment(uuid(id)?)
                    }
                    _ => return Err(Status::permission_denied("Unsupported intro operation")),
                }
            } else {
                return Err(Status::permission_denied("Unsupported intro operation"));
            }
        }
        ("DELETE", rest) => {
            if let Some(id) = rest.strip_prefix("/api/me/intro-posts/") {
                if id.contains('/') {
                    return Err(Status::permission_denied("Unsupported intro operation"));
                }
                IntroOperation::DeleteIntroPost(uuid(id)?)
            } else if let Some(rest) = rest.strip_prefix("/api/intro-posts/") {
                match rest.split_once('/') {
                    Some((id, "like")) => IntroOperation::ClearIntroPostLike(uuid(id)?),
                    Some((id, comments)) => match comments.strip_prefix("comments/") {
                        Some(comment) if !comment.contains('/') => {
                            IntroOperation::DeleteIntroPostComment(uuid(id)?, uuid(comment)?)
                        }
                        _ => return Err(Status::permission_denied("Unsupported intro operation")),
                    },
                    None => return Err(Status::permission_denied("Unsupported intro operation")),
                }
            } else {
                return Err(Status::permission_denied("Unsupported intro operation"));
            }
        }
        _ => return Err(Status::permission_denied("Unsupported intro operation")),
    };
    let query = parse_query(operation, raw_query)?;
    Ok(Parsed { operation, query })
}

fn parse_query(operation: IntroOperation, raw: Option<&str>) -> Result<Query, Status> {
    let allowed = operation.fields();
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    if let Some(raw) = raw {
        for pair in raw.split('&') {
            if pair.is_empty() {
                continue;
            }
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            let name = decode(name)?;
            // Closed: an unknown field is refused, never ignored.
            if !allowed.contains(&name.as_str()) {
                return Err(Status::invalid_argument("Unexpected intro query field"));
            }
            if fields.insert(name.clone(), decode(value)?).is_some() {
                return Err(Status::invalid_argument("Repeated intro query field"));
            }
        }
    }
    let limit = match fields.get("limit") {
        Some(value) => {
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(Status::invalid_argument("Invalid intro limit"));
            }
            let parsed: i64 = value
                .parse()
                .map_err(|_| Status::invalid_argument("Invalid intro limit"))?;
            if !(1..=100).contains(&parsed) {
                return Err(Status::invalid_argument("Invalid intro limit"));
            }
            Some(parsed)
        }
        None => None,
    };
    let cursor = match fields.get("cursor") {
        Some(value) => {
            if value.is_empty()
                || value.len() > 1024
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
            {
                return Err(Status::invalid_argument("Invalid intro cursor"));
            }
            Some(value.clone())
        }
        None => None,
    };
    let q = match fields.get("q") {
        Some(value) => {
            let length = value.chars().count();
            if length == 0 || length > 100 || value.chars().any(char::is_control) {
                return Err(Status::invalid_argument("Invalid intro search term"));
            }
            Some(value.clone())
        }
        None => None,
    };
    let state = match fields.get("state") {
        Some(value) => {
            if !matches!(value.as_str(), "DRAFT" | "PUBLISHED" | "DELETED") {
                return Err(Status::invalid_argument("Invalid intro state"));
            }
            Some(value.clone())
        }
        None => None,
    };
    let expected_revision = match fields.get("expected_revision") {
        Some(value) => {
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(Status::invalid_argument("Invalid intro revision"));
            }
            Some(
                value
                    .parse()
                    .map_err(|_| Status::invalid_argument("Invalid intro revision"))?,
            )
        }
        None => None,
    };
    Ok(Query {
        limit,
        cursor,
        q,
        state,
        expected_revision,
    })
}
