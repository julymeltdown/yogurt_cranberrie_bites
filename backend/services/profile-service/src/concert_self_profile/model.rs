//! Canonical owner content and patch semantics. No authentication constructors.
use uuid::Uuid;

pub const MAX_REVISION: i64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Invalid,
    Missing,
    Conflict,
    Unavailable,
}

/// Internal concurrency token. It never becomes the event participant revision.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Version {
    epoch: Uuid,
    revision: i64,
}
impl Version {
    pub fn new(epoch: Uuid, revision: i64) -> Result<Self, Error> {
        if epoch.is_nil() || !(1..=MAX_REVISION).contains(&revision) {
            return Err(Error::Invalid);
        }
        Ok(Self { epoch, revision })
    }
    pub fn epoch(self) -> Uuid {
        self.epoch
    }
    pub fn revision(self) -> i64 {
        self.revision
    }
}
impl std::fmt::Debug for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Version([REDACTED])")
    }
}

/// Absence leaves the stored bio alone; Clear represents an explicit JSON null.
#[derive(Clone, PartialEq, Eq)]
pub enum BioPatch {
    Keep,
    Clear,
    Set(String),
}
#[derive(Clone, PartialEq, Eq)]
pub struct Patch {
    display_name: Option<String>,
    bio: BioPatch,
}
impl Patch {
    pub fn new(display_name: Option<String>, bio: BioPatch) -> Result<Self, Error> {
        if display_name.as_ref().is_some_and(|s| !valid_name(s))
            || matches!(&bio, BioPatch::Set(s) if !valid_bio(s))
            || (display_name.is_none() && matches!(&bio, BioPatch::Keep))
        {
            return Err(Error::Invalid);
        }
        Ok(Self { display_name, bio })
    }
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }
    pub fn bio(&self) -> &BioPatch {
        &self.bio
    }
    pub fn apply(&self, current: &Content) -> Result<Content, Error> {
        let name = self
            .display_name
            .clone()
            .unwrap_or_else(|| current.display_name.clone());
        let bio = match &self.bio {
            BioPatch::Keep => current.bio.clone(),
            BioPatch::Clear => None,
            BioPatch::Set(value) if value.is_empty() => None,
            BioPatch::Set(value) => Some(value.clone()),
        };
        Content::new(current.user_id, name, bio, current.version)
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct Content {
    user_id: Uuid,
    display_name: String,
    bio: Option<String>,
    version: Version,
}
impl Content {
    pub fn new(
        user_id: Uuid,
        display_name: String,
        bio: Option<String>,
        version: Version,
    ) -> Result<Self, Error> {
        if user_id.is_nil()
            || !valid_name(&display_name)
            || bio.as_ref().is_some_and(|s| !valid_bio(s))
        {
            return Err(Error::Invalid);
        }
        Ok(Self {
            user_id,
            display_name,
            bio: bio.filter(|s| !s.is_empty()),
            version,
        })
    }
    pub fn user_id(&self) -> Uuid {
        self.user_id
    }
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    pub fn bio(&self) -> Option<&str> {
        self.bio.as_deref()
    }
    pub fn version(&self) -> Version {
        self.version
    }
}
fn valid_name(s: &str) -> bool {
    !s.trim().is_empty() && s.chars().count() <= 40 && !s.chars().any(char::is_control)
}
fn valid_bio(s: &str) -> bool {
    s.chars().count() <= 160 && !s.chars().any(char::is_control)
}
macro_rules! redacted {
    ($($ty:ty),+) => { $(impl std::fmt::Debug for $ty {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(concat!(stringify!($ty), "([REDACTED])")) }
    })+ };
}
redacted!(BioPatch, Patch, Content);

#[cfg(test)]
mod tests {
    use super::*;
    fn content() -> Content {
        Content::new(
            Uuid::new_v4(),
            "Current".into(),
            Some("Existing bio".into()),
            Version::new(Uuid::new_v4(), 1).unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn omitted_null_empty_and_selected_fields_have_distinct_patch_semantics() {
        let current = content();
        let renamed = Patch::new(Some("Updated".into()), BioPatch::Keep)
            .unwrap()
            .apply(&current)
            .unwrap();
        assert_eq!(renamed.bio(), current.bio());
        assert_eq!(renamed.user_id(), current.user_id());
        assert_eq!(renamed.version(), current.version());
        for bio in [BioPatch::Clear, BioPatch::Set(String::new())] {
            let changed = Patch::new(None, bio).unwrap().apply(&current).unwrap();
            assert_eq!(changed.bio(), None);
            assert_eq!(changed.display_name(), current.display_name());
        }
        assert!(Patch::new(None, BioPatch::Keep).is_err());
    }
    #[test]
    fn unicode_limits_and_control_rejection_match_minimal_owner_content() {
        assert!(Patch::new(Some("界".repeat(40)), BioPatch::Set("界".repeat(160))).is_ok());
        for name in ["".into(), " \t".into(), "a\n".into(), "界".repeat(41)] {
            assert!(Patch::new(Some(name), BioPatch::Keep).is_err());
        }
        for bio in ["a\n".into(), "界".repeat(161)] {
            assert!(Patch::new(None, BioPatch::Set(bio)).is_err());
        }
        assert!(Version::new(Uuid::nil(), 1).is_err());
        assert!(Version::new(Uuid::new_v4(), 0).is_err());
        assert!(Version::new(Uuid::new_v4(), MAX_REVISION + 1).is_err());
    }
    #[test]
    fn debug_never_contains_profile_content_or_ids() {
        let current = content();
        let rendered = format!("{current:?}");
        assert!(!rendered.contains(current.display_name()));
        assert!(!rendered.contains(&current.user_id().to_string()));
        assert!(!rendered.contains(current.bio().unwrap()));
    }
}
