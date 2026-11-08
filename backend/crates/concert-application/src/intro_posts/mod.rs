//! WP-05: the pure rules for mutuals-intro posts.
//!
//! Everything in this module is a decision over values. There is no I/O, no
//! clock read and no database handle, so each rule can be tested exhaustively
//! and the SQL layer cannot quietly disagree with it: the commands call these
//! functions and the database constraints are written to match.
//!
//! The four rule sets, one per task:
//!
//! * [`audience`] — the audience value, its bounds and its server-derived
//!   summary (T-401, T-407's wire form);
//! * [`content`] — body, single-line fields and the contact-bypass rule (T-401);
//! * [`attachments`] — at most four, unique, owned, READY, `INTRO_POST` (T-403);
//! * [`comments`] — body, parent and the deletion rule (T-404).
//!
//! [`snapshot`] holds the favorites-snapshot rule (T-402): the snapshot is a
//! display copy taken when the draft is created, and a later profile edit must
//! never rewrite it.
pub mod attachments;
pub mod audience;
pub mod comments;
pub mod content;
pub mod snapshot;

pub use attachments::{AttachmentCandidate, AttachmentError, AttachmentPlan, MAX_ATTACHMENTS};
pub use audience::{Audience, AudienceError, GenderCondition};
pub use comments::{CommentError, CommentInput, MAX_COMMENT_LENGTH};
pub use content::{ContentError, IntroContent, MAX_BODY, MAX_SINGLE_LINE};
pub use snapshot::{SnapshotError, MAX_SNAPSHOT};
