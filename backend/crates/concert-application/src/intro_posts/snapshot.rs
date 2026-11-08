//! T-402: the favorites snapshot.
//!
//! An intro post carries a `favorites_snapshot`: a display copy of the author's
//! picks taken when the draft is created. Three rules, and the second is the one
//! the task calls out:
//!
//! 1. the snapshot is taken once, at draft creation;
//! 2. **editing the profile afterwards never changes a published post's
//!    snapshot** — the snapshot is stored on the post, not read through the
//!    profile;
//! 3. discovery uses the profile favorites, never the snapshot, so a stale
//!    snapshot cannot make a post discoverable under picks the author no longer
//!    has.
//!
//! Rule 2 is a property of *where the value lives*, so this module's job is to
//! make the copy operation explicit and to give the command a single function to
//! call. [`Snapshot::from_favorites`] is the only constructor, and
//! [`Snapshot::unchanged_by_profile_edit`] documents the invariant the tests
//! assert.
use uuid::Uuid;

/// The frozen `favorites_snapshot` bound.
pub const MAX_SNAPSHOT: usize = 15;
/// The frozen `FavoriteItem.label` bound.
pub const MAX_LABEL: usize = 80;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotError {
    /// More than fifteen items were supplied.
    TooMany,
    /// A label was empty or longer than 80 characters.
    InvalidLabel,
    /// The same catalog item appeared twice.
    Duplicate,
    /// A catalog item id was nil.
    NilId,
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid intro favorites snapshot: {self:?}")
    }
}

impl std::error::Error for SnapshotError {}

/// One snapshot entry: a catalog item id and its label at the moment of copying.
///
/// Deliberately no weight, intensity or preference field. The destination has no
/// such concept, and the snapshot is a display copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotItem {
    pub catalog_item_id: Uuid,
    pub label: String,
    pub category: &'static str,
}

/// A validated, frozen snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    items: Vec<SnapshotItem>,
}

impl Snapshot {
    /// Copy the author's current favorites into a snapshot.
    ///
    /// The caller passes whatever the profile currently holds. This function
    /// copies it; it never reads the profile itself, which is what makes the
    /// snapshot independent of later edits.
    pub fn from_favorites(items: Vec<SnapshotItem>) -> Result<Self, SnapshotError> {
        if items.len() > MAX_SNAPSHOT {
            return Err(SnapshotError::TooMany);
        }
        let mut seen = std::collections::BTreeSet::new();
        for item in &items {
            if item.catalog_item_id.is_nil() {
                return Err(SnapshotError::NilId);
            }
            let length = item.label.chars().count();
            if length == 0 || length > MAX_LABEL {
                return Err(SnapshotError::InvalidLabel);
            }
            if !seen.insert(item.catalog_item_id) {
                return Err(SnapshotError::Duplicate);
            }
        }
        Ok(Self { items })
    }

    /// An empty snapshot. A profile with no picks still publishes fine.
    pub fn empty() -> Self {
        Self { items: Vec::new() }
    }

    /// Read a stored snapshot back. A stored value that violates a rule is
    /// refused rather than repaired, so a corrupted row cannot silently become
    /// a different post.
    pub fn from_stored(value: &serde_json::Value) -> Result<Self, SnapshotError> {
        let array = value.as_array().ok_or(SnapshotError::InvalidLabel)?;
        let mut items = Vec::with_capacity(array.len());
        for entry in array {
            let object = entry.as_object().ok_or(SnapshotError::InvalidLabel)?;
            // Closed: only the three contract fields, nothing else.
            if object
                .keys()
                .any(|key| !matches!(key.as_str(), "catalog_item_id" | "label" | "category" | "external_url"))
            {
                return Err(SnapshotError::InvalidLabel);
            }
            let id = object
                .get("catalog_item_id")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
                .ok_or(SnapshotError::NilId)?;
            let label = object
                .get("label")
                .and_then(serde_json::Value::as_str)
                .ok_or(SnapshotError::InvalidLabel)?;
            let category = match object.get("category").and_then(serde_json::Value::as_str) {
                Some("MUSIC") => "MUSIC",
                Some("ANIME") => "ANIME",
                Some("GAME") => "GAME",
                _ => return Err(SnapshotError::InvalidLabel),
            };
            items.push(SnapshotItem {
                catalog_item_id: id,
                label: label.to_owned(),
                category,
            });
        }
        Self::from_favorites(items)
    }

    /// The stored `favorites_snapshot` value.
    pub fn to_stored(&self) -> serde_json::Value {
        serde_json::Value::Array(
            self.items
                .iter()
                .map(|item| {
                    serde_json::json!({
                        "catalog_item_id": item.catalog_item_id,
                        "label": item.label,
                        "category": item.category,
                        "external_url": serde_json::Value::Null,
                    })
                })
                .collect(),
        )
    }

    pub fn items(&self) -> &[SnapshotItem] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The invariant T-402 asserts, stated where a reader will find it.
    ///
    /// A snapshot is a value stored on the post. Re-reading the profile and
    /// re-deriving the snapshot would violate this rule, so the command never
    /// does: it reads the stored column and writes back only what the author
    /// explicitly sent.
    pub const fn unchanged_by_profile_edit() -> &'static str {
        "a stored favorites_snapshot is never re-derived from the profile"
    }
}
