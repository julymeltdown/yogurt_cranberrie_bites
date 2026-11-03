//! Closed C04 serialization boundary. New feed types never expose inherited DTOs.
use concert_application::feed::{FeedPage, ProjectedPost};
use concert_domain::feed::{Content, FeedError};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

fn schema() -> Result<&'static Value, FeedError> {
    static SOURCE: OnceLock<Result<Value, FeedError>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../../../../../contracts/generated/mcr-domain.openapi.json"
            ))
            .map_err(|_| FeedError::DependencyUnavailable)
        })
        .as_ref()
        .map_err(|e| *e)
}
/// The frozen generated schema has local refs only; no network retriever is used.
pub fn validate(name: &str, value: &Value) -> Result<(), FeedError> {
    if !matches!(
        name,
        "Domain_feed_PostPublic"
            | "PublicProfile"
            | "Domain_feed_FeedPage"
            | "Domain_feed_CreatePost"
            | "Domain_feed_EditPost"
            | "Domain_feed_CommentPublic"
            | "Domain_feed_CommentList"
            | "Domain_feed_CreateComment"
            | "Domain_feed_EditComment"
            | "Domain_feed_PostDetail"
            | "Domain_feed_ReactionState"
            | "Domain_common_Deleted"
            | "Domain_common_RevisionRequest"
            | "Domain_common_EmptyRequest"
    ) {
        return Err(FeedError::InvalidInput);
    }
    static VALIDATORS: OnceLock<Mutex<HashMap<String, Arc<jsonschema::Validator>>>> =
        OnceLock::new();
    let mut cache = VALIDATORS
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| FeedError::DependencyUnavailable)?;
    let validator = if let Some(validator) = cache.get(name) {
        validator.clone()
    } else {
        let wrapper = json!({"$schema":"https://json-schema.org/draft/2020-12/schema","$ref":format!("#/components/schemas/{name}"),"components":schema()?["components"]});
        let validator = Arc::new(
            jsonschema::options()
                .should_validate_formats(true)
                .build(&wrapper)
                .map_err(|_| FeedError::DependencyUnavailable)?,
        );
        cache.insert(name.into(), validator.clone());
        validator
    };
    drop(cache);
    if validator.is_valid(value) {
        Ok(())
    } else {
        Err(FeedError::InvalidInput)
    }
}
/// Validate the exact C03 allowlist and subject before any enclosing feed DTO.
/// A syntactically valid profile for another subject is not a safe author.
pub fn author(
    value: concert_application::feed::PublicAuthor,
    expected: uuid::Uuid,
) -> Result<Value, FeedError> {
    let author: Value =
        serde_json::from_slice(&value.json).map_err(|_| FeedError::DependencyUnavailable)?;
    if expected.is_nil() || value.user_id != expected || author["user_id"] != expected.to_string() {
        return Err(FeedError::DependencyUnavailable);
    }
    validate("PublicProfile", &author).map_err(|_| FeedError::DependencyUnavailable)?;
    Ok(author)
}
pub fn post(value: ProjectedPost) -> Result<Value, FeedError> {
    let p = value.post;
    let author = author(value.author, p.author)?;
    if !p.visible_in(p.event) {
        return Err(FeedError::DependencyUnavailable);
    }
    let dto = json!({"post_id":p.id,"event_id":p.event,"author":author,"text":p.content.text,
        "media_ids":p.content.media_ids,"links":p.content.links,"created_at":p.created_at,
        "edited_at":p.edited_at,"comment_count":p.comment_count,"reaction_count":p.reaction_count,
        "reacted_by_me":p.reacted_by_me,"state":"PUBLISHED","revision":p.revision});
    validate("Domain_feed_PostPublic", &dto).map_err(|_| FeedError::DependencyUnavailable)?;
    Ok(dto)
}
pub fn page(page: FeedPage, next_cursor: Option<String>) -> Result<Value, FeedError> {
    if page.next.is_some() != next_cursor.is_some() {
        return Err(FeedError::DependencyUnavailable);
    }
    let items = page
        .items
        .into_iter()
        .map(post)
        .collect::<Result<Vec<_>, _>>()?;
    let dto = json!({"items":items,"next_cursor":next_cursor});
    validate("Domain_feed_FeedPage", &dto).map_err(|_| FeedError::DependencyUnavailable)?;
    Ok(dto)
}
/// Direct struct deserialization rejects duplicate keys; schema validation rejects
/// noncanonical UUIDs, unknown fields and wrong types before constructing Content.
pub fn create_post(bytes: &[u8]) -> Result<Content, FeedError> {
    if bytes.len() > 65536 {
        return Err(FeedError::InvalidInput);
    }
    let content: Content = serde_json::from_slice(bytes).map_err(|_| FeedError::InvalidInput)?;
    let value: Value = serde_json::from_slice(bytes).map_err(|_| FeedError::InvalidInput)?;
    validate("Domain_feed_CreatePost", &value)?;
    content.validate()?;
    Ok(content)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EditPostInput {
    expected_revision: i64,
    text: String,
    media_ids: Vec<uuid::Uuid>,
    links: Vec<concert_domain::feed::ResourceLink>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateCommentInput {
    text: String,
    post_revision: i64,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EditCommentInput {
    text: String,
    expected_revision: i64,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RevisionRequest {
    expected_revision: i64,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
fn decode<T: serde::de::DeserializeOwned>(name: &str, bytes: &[u8]) -> Result<T, FeedError> {
    if bytes.len() > 65536 {
        return Err(FeedError::InvalidInput);
    }
    let typed: T = serde_json::from_slice(bytes).map_err(|_| FeedError::InvalidInput)?;
    let value: Value = serde_json::from_slice(bytes).map_err(|_| FeedError::InvalidInput)?;
    validate(name, &value)?;
    Ok(typed)
}
/// Path ID and operation come from the verified root route. For DELETE revision
/// requests, bytes are the closed validated query object, not a DELETE body.
pub fn command(
    operation: &str,
    target: Option<uuid::Uuid>,
    bytes: &[u8],
) -> Result<concert_domain::feed::FeedCommand, FeedError> {
    use concert_domain::feed::FeedCommand::*;
    let id = || {
        target
            .filter(|id| !id.is_nil())
            .ok_or(FeedError::InvalidInput)
    };
    let result = match operation {
        "createConcertPost" => {
            if target.is_some() {
                return Err(FeedError::InvalidInput);
            }
            CreatePost {
                content: create_post(bytes)?,
            }
        }
        "editConcertPost" => {
            let r: EditPostInput = decode("Domain_feed_EditPost", bytes)?;
            EditPost {
                post: id()?,
                expected_revision: r.expected_revision,
                content: Content {
                    text: r.text,
                    media_ids: r.media_ids,
                    links: r.links,
                },
            }
        }
        "deleteConcertPost" => {
            let r: RevisionRequest = decode("Domain_common_RevisionRequest", bytes)?;
            DeletePost {
                post: id()?,
                expected_revision: r.expected_revision,
            }
        }
        "createConcertComment" => {
            let r: CreateCommentInput = decode("Domain_feed_CreateComment", bytes)?;
            CreateComment {
                post: id()?,
                post_revision: r.post_revision,
                text: r.text,
            }
        }
        "editConcertComment" => {
            let r: EditCommentInput = decode("Domain_feed_EditComment", bytes)?;
            EditComment {
                comment: id()?,
                expected_revision: r.expected_revision,
                text: r.text,
            }
        }
        "deleteConcertComment" => {
            let r: RevisionRequest = decode("Domain_common_RevisionRequest", bytes)?;
            DeleteComment {
                comment: id()?,
                expected_revision: r.expected_revision,
            }
        }
        "likeConcertPost" | "unlikeConcertPost" => {
            let _: Empty = decode("Domain_common_EmptyRequest", bytes)?;
            SetReaction {
                post: id()?,
                present: operation == "likeConcertPost",
            }
        }
        _ => return Err(FeedError::InvalidInput),
    };
    result.validate()?;
    Ok(result)
}
pub fn comment(value: concert_application::feed::ProjectedComment) -> Result<Value, FeedError> {
    let c = value.comment;
    let author = author(value.author, c.author)?;
    if c.state != concert_domain::feed::ContentState::Published {
        return Err(FeedError::DependencyUnavailable);
    }
    let dto = json!({"comment_id":c.id,"post_id":c.post,"author":author,"text":c.text,"revision":c.revision,"created_at":c.created_at});
    validate("Domain_feed_CommentPublic", &dto).map_err(|_| FeedError::DependencyUnavailable)?;
    Ok(dto)
}
pub fn comments(
    page: concert_application::feed::CommentPage,
    next_cursor: Option<String>,
) -> Result<Value, FeedError> {
    if page.next.is_some() != next_cursor.is_some() {
        return Err(FeedError::DependencyUnavailable);
    }
    let items = page
        .items
        .into_iter()
        .map(comment)
        .collect::<Result<Vec<_>, _>>()?;
    let dto = json!({"items":items,"next_cursor":next_cursor});
    validate("Domain_feed_CommentList", &dto).map_err(|_| FeedError::DependencyUnavailable)?;
    Ok(dto)
}
pub fn thread(
    value: concert_application::feed::Thread,
    next_cursor: Option<String>,
) -> Result<Value, FeedError> {
    let comments = comments(value.comments, next_cursor)?;
    let dto = json!({"post":post(value.post)?,"comments":comments["items"],"next_cursor":comments["next_cursor"]});
    validate("Domain_feed_PostDetail", &dto).map_err(|_| FeedError::DependencyUnavailable)?;
    Ok(dto)
}
pub fn error(error: FeedError) -> (u16, &'static str) {
    match error {
        FeedError::InvalidInput => (400, "INVALID_INPUT"),
        FeedError::ResourceUnavailable => (404, "RESOURCE_UNAVAILABLE"),
        FeedError::PermissionDenied => (403, "PERMISSION_DENIED"),
        FeedError::DependencyUnavailable => (503, "DEPENDENCY_UNAVAILABLE"),
        FeedError::VersionConflict => (409, "VERSION_CONFLICT"),
        FeedError::EventReadOnly => (409, "EVENT_READ_ONLY"),
        FeedError::MediaNotReady => (409, "MEDIA_NOT_READY"),
    }
}
