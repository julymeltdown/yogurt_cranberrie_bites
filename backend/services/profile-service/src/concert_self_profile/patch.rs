//! Parse exact C03 PATCH bytes once, including duplicate and unknown-field refusal.
use super::model::{BioPatch, Error, Patch, MAX_REVISION};
use serde::{Deserialize, Deserializer};

const MAX_BYTES: usize = 65_536;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    expected_revision: i64,
    #[serde(default, deserialize_with = "non_null_option")]
    display_name: Option<String>,
    #[serde(default)]
    bio: BioInput,
    #[serde(default, deserialize_with = "non_null_option")]
    preferences: Option<Preferences>,
}
fn non_null_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Default)]
enum BioInput {
    #[default]
    Keep,
    Clear,
    Set(String),
}
impl<'de> Deserialize<'de> for BioInput {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<String>::deserialize(deserializer).map(|value| match value {
            None => Self::Clear,
            Some(value) => Self::Set(value),
        })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Preferences {
    theme: Theme,
    low_power: bool,
    reduced_motion: bool,
    sound_enabled: bool,
}
#[derive(Deserialize)]
enum Theme {
    REVENGE,
    PARADE,
    DANGER,
}

pub struct ParticipantPatch {
    expected_revision: i64,
    profile: Option<Patch>,
}
impl ParticipantPatch {
    pub fn parse(raw: &[u8]) -> Result<Self, Error> {
        if raw.is_empty() || raw.len() > MAX_BYTES {
            return Err(Error::Invalid);
        }
        let input: Input = serde_json::from_slice(raw).map_err(|_| Error::Invalid)?;
        if !(1..=MAX_REVISION).contains(&input.expected_revision) {
            return Err(Error::Invalid);
        }
        let bio = match input.bio {
            BioInput::Keep => BioPatch::Keep,
            BioInput::Clear => BioPatch::Clear,
            BioInput::Set(value) => BioPatch::Set(value),
        };
        let profile = if input.display_name.is_none() && matches!(&bio, BioPatch::Keep) {
            None
        } else {
            Some(Patch::new(input.display_name, bio)?)
        };
        // Preferences are validated for exact field/type/enum shape, but remain
        // exclusively Concert-owned. This producer never persists them.
        if let Some(prefs) = &input.preferences {
            let _ = (
                &prefs.theme,
                prefs.low_power,
                prefs.reduced_motion,
                prefs.sound_enabled,
            );
        }
        if profile.is_none() && input.preferences.is_none() {
            return Err(Error::Invalid);
        }
        Ok(Self {
            expected_revision: input.expected_revision,
            profile,
        })
    }
    pub fn expected_participant_revision(&self) -> i64 {
        self.expected_revision
    }
    pub fn profile(&self) -> Option<&Patch> {
        self.profile.as_ref()
    }
}
impl std::fmt::Debug for ParticipantPatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ParticipantPatch([REDACTED])")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_unknown_invalid_or_no_fields_are_refused() {
        for raw in [
            r#"{"expected_revision":1,"display_name":"A","display_name":"B"}"#,
            r#"{"expected_revision":1,"expected_revision":2,"bio":null}"#,
            r#"{"expected_revision":1,"bio":null,"avatar":{}}"#,
            r#"{"expected_revision":0,"bio":null}"#,
            r#"{"expected_revision":1}"#,
            r#"{"expected_revision":1,"display_name":null}"#,
            r#"{"expected_revision":1,"display_name":null,"bio":null}"#,
            r#"{"expected_revision":1,"preferences":null,"bio":null}"#,
            r#"{"expected_revision":1,"preferences":null}"#,
            r#"{"expected_revision":1,"preferences":{"theme":"REVENGE","low_power":false}}"#,
        ] {
            assert!(ParticipantPatch::parse(raw.as_bytes()).is_err());
        }
    }
    #[test]
    fn nullable_bio_and_preferences_only_keep_their_own_meanings() {
        let clear = ParticipantPatch::parse(br#"{"expected_revision":2,"bio":null}"#).unwrap();
        assert_eq!(clear.expected_participant_revision(), 2);
        assert!(matches!(clear.profile().unwrap().bio(), BioPatch::Clear));
        let name = ParticipantPatch::parse(br#"{"expected_revision":2,"display_name":"Current"}"#)
            .unwrap();
        assert!(matches!(name.profile().unwrap().bio(), BioPatch::Keep));
        let prefs = ParticipantPatch::parse(br#"{"expected_revision":2,"preferences":{"theme":"DANGER","low_power":false,"reduced_motion":true,"sound_enabled":false}}"#).unwrap();
        assert!(prefs.profile().is_none());
    }
}
