//! Closed C04 serialization boundary. New feed types never expose inherited DTOs.
use concert_application::feed::{FeedPage, ProjectedPost};
use concert_domain::feed::{Content, FeedError};
use serde_json::{json, Value};
use std::sync::OnceLock;

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
    let wrapper = json!({"$schema":"https://json-schema.org/draft/2020-12/schema","$ref":format!("#/components/schemas/{name}"),"components":schema()?["components"]});
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .build(&wrapper)
        .map_err(|_| FeedError::DependencyUnavailable)?;
    if validator.is_valid(value) {
        Ok(())
    } else {
        Err(FeedError::InvalidInput)
    }
}
pub fn post(value: ProjectedPost) -> Result<Value, FeedError> {
    let p = value.post;
    let author: Value =
        serde_json::from_slice(&value.author.json).map_err(|_| FeedError::DependencyUnavailable)?;
    if value.author.user_id != p.author
        || author["user_id"] != p.author.to_string()
        || !p.visible_in(p.event)
    {
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
