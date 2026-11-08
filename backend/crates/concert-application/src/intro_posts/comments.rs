//! T-404: comment rules.
//!
//! Comments are one level deep. A reply names its parent, the parent must belong
//! to the same intro post, and a reply to a reply is refused so the thread
//! cannot deepen without bound. The database enforces the same three rules
//! (`intro_comments_parent`, `intro_comments_parent_intro`,
//! `intro_comments_one_level`); this module is where a caller learns *which* rule
//! it broke, before a transaction opens.
//!
//! **Deletion rule (documented and tested).** Deleting a comment is a soft
//! delete: the row keeps its place and its replies keep their parent. The
//! comment's body is no longer returned, and the comment is rendered as
//! "deleted" with its replies still visible. Replies are **not** deleted with
//! their parent, because a reply is a separate author's contribution and
//! removing one person's comment must not silently erase somebody else's.
//! Deleting the *post* removes everything, which is the separate cascade in
//! T-401.
use uuid::Uuid;

/// The frozen `body` bound for a comment.
pub const MAX_COMMENT_LENGTH: usize = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommentError {
    /// The body was empty or whitespace-only.
    Empty,
    /// The body exceeded 500 characters.
    TooLong,
    /// The body contained a NUL byte.
    ContainsNul,
    /// The named parent does not exist, is deleted, or belongs to another post.
    ///
    /// Undifferentiated on purpose: the caller must not be able to tell a
    /// cross-post parent from an absent one.
    ParentUnavailable,
    /// The named parent is itself a reply.
    ReplyToReply,
}

impl std::fmt::Display for CommentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid intro input: {self:?}")
    }
}

impl std::error::Error for CommentError {}

/// A validated comment input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommentInput {
    pub body: String,
    pub parent_comment_id: Option<Uuid>,
}

/// What the store knows about the named parent, if there is one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParentComment {
    pub comment_id: Uuid,
    pub intro_id: Uuid,
    pub parent_comment_id: Option<Uuid>,
    pub deleted: bool,
}

/// Validate a comment before it is written.
///
/// `intro_id` is the post the comment is addressed to; `parent` is what the
/// store found for `parent_comment_id`, or `None` when the id did not resolve.
pub fn validate(
    intro_id: Uuid,
    body: &str,
    parent_comment_id: Option<Uuid>,
    parent: Option<ParentComment>,
) -> Result<CommentInput, CommentError> {
    if body.trim().is_empty() {
        return Err(CommentError::Empty);
    }
    if body.chars().count() > MAX_COMMENT_LENGTH {
        return Err(CommentError::TooLong);
    }
    if body.contains('\0') {
        return Err(CommentError::ContainsNul);
    }
    match parent_comment_id {
        None => Ok(CommentInput {
            body: body.to_owned(),
            parent_comment_id: None,
        }),
        Some(id) => {
            if id.is_nil() {
                return Err(CommentError::ParentUnavailable);
            }
            let parent = parent.ok_or(CommentError::ParentUnavailable)?;
            if parent.deleted {
                return Err(CommentError::ParentUnavailable);
            }
            if parent.intro_id != intro_id {
                return Err(CommentError::ParentUnavailable);
            }
            if parent.parent_comment_id.is_some() {
                return Err(CommentError::ReplyToReply);
            }
            Ok(CommentInput {
                body: body.to_owned(),
                parent_comment_id: Some(id),
            })
        }
    }
}
