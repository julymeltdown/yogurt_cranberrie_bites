//! Exact C03 participant PATCH, including field presence. Parsing supplies no
//! authority and does not commit either canonical Profile or event preferences.
use concert_application::avatar::AvatarError;
use concert_domain::{avatar::MAX_REQUEST_BYTES, Preferences, Revision};
use serde::{Deserialize, Deserializer};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Field<T> {
    Unchanged,
    Set(T),
}
impl<T> Default for Field<T> {
    fn default() -> Self {
        Self::Unchanged
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Field<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        T::deserialize(deserializer).map(Self::Set)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParticipantUpdate {
    pub expected_revision: Revision,
    pub display_name: Field<String>,
    pub bio: Field<Option<String>>,
    pub preferences: Field<Preferences>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePreferences {
    theme: String,
    low_power: bool,
    reduced_motion: bool,
    sound_enabled: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireUpdate {
    expected_revision: i64,
    #[serde(default)]
    display_name: Field<String>,
    #[serde(default)]
    bio: Field<Option<String>>,
    #[serde(default)]
    preferences: Field<WirePreferences>,
}
impl ParticipantUpdate {
    pub fn parse(bytes: &[u8]) -> Result<Self, AvatarError> {
        if bytes.is_empty() || bytes.len() > MAX_REQUEST_BYTES {
            return Err(AvatarError::InvalidInput);
        }
        let wire: WireUpdate =
            serde_json::from_slice(bytes).map_err(|_| AvatarError::InvalidInput)?;
        let expected_revision =
            Revision::new(wire.expected_revision).map_err(|_| AvatarError::InvalidInput)?;
        if let Field::Set(name) = &wire.display_name {
            if !(1..=40).contains(&name.chars().count()) {
                return Err(AvatarError::InvalidInput);
            }
        }
        if let Field::Set(Some(bio)) = &wire.bio {
            if bio.chars().count() > 160 {
                return Err(AvatarError::InvalidInput);
            }
        }
        let preferences = match wire.preferences {
            Field::Unchanged => Field::Unchanged,
            Field::Set(p) => Field::Set(Preferences {
                theme: p.theme.parse().map_err(|_| AvatarError::InvalidInput)?,
                low_power: p.low_power,
                reduced_motion: p.reduced_motion,
                sound_enabled: p.sound_enabled,
            }),
        };
        let value = Self {
            expected_revision,
            display_name: wire.display_name,
            bio: wire.bio,
            preferences,
        };
        if !value.changes_profile() && matches!(value.preferences, Field::Unchanged) {
            return Err(AvatarError::InvalidInput);
        }
        Ok(value)
    }
    pub fn changes_profile(&self) -> bool {
        !matches!(self.display_name, Field::Unchanged) || !matches!(self.bio, Field::Unchanged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn full_patch_matches_frozen_c03_shape_and_preserves_null_vs_absent() {
        let mut contract: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../docs/api/mcr-core.openapi.json"
        ))
        .unwrap();
        contract["$ref"] = "#/components/schemas/UpdateParticipantRequest".into();
        let schema = jsonschema::validator_for(&contract).unwrap();
        let preferences =
            json!({"theme":"DANGER","low_power":false,"reduced_motion":true,"sound_enabled":false});
        for document in [
            json!({"expected_revision":1,"bio":null}),
            json!({"expected_revision":1,"bio":""}),
            json!({"expected_revision":2,"display_name":"Current name"}),
            json!({"expected_revision":3,"preferences":preferences}),
            json!({"expected_revision":4,"display_name":"New name","bio":null,"preferences":preferences}),
            json!({"expected_revision":1}),
            json!({"expected_revision":0,"bio":null}),
            json!({"expected_revision":1,"display_name":null}),
            json!({"expected_revision":1,"display_name":""}),
            json!({"expected_revision":1,"display_name":"x".repeat(41)}),
            json!({"expected_revision":1,"bio":"x".repeat(161)}),
            json!({"expected_revision":1,"preferences":null}),
            json!({"expected_revision":1,"preferences":{"theme":"DANGER"}}),
            json!({"expected_revision":1,"bio":null,"user_id":"foreign-field"}),
        ] {
            assert_eq!(
                ParticipantUpdate::parse(&serde_json::to_vec(&document).unwrap()).is_ok(),
                schema.is_valid(&document),
                "{document}"
            );
        }
        let clear = ParticipantUpdate::parse(br#"{"expected_revision":1,"bio":null}"#).unwrap();
        assert_eq!(clear.bio, Field::Set(None));
        assert_eq!(clear.display_name, Field::Unchanged);
        assert_eq!(clear.preferences, Field::Unchanged);
        let keep =
            ParticipantUpdate::parse(br#"{"expected_revision":1,"display_name":"New"}"#).unwrap();
        assert_eq!(keep.bio, Field::Unchanged);
    }
    #[test]
    fn duplicate_patch_fields_and_transport_overflow_fail_closed() {
        for bytes in [
            br#"{"expected_revision":1,"expected_revision":2,"bio":null}"#.as_slice(),
            br#"{"expected_revision":1,"bio":"keep","bio":null}"#.as_slice(),
            br#"{"expected_revision":1,"display_name":"one","display_name":"two"}"#.as_slice(),
            br#"{"expected_revision":1,"preferences":{"theme":"DANGER","theme":"PARADE","low_power":false,"reduced_motion":false,"sound_enabled":true}}"#.as_slice(),
        ] { assert_eq!(ParticipantUpdate::parse(bytes), Err(AvatarError::InvalidInput)); }
        let mut bytes = br#"{"expected_revision":1,"bio":null}"#.to_vec();
        bytes.resize(MAX_REQUEST_BYTES + 1, b' ');
        assert_eq!(
            ParticipantUpdate::parse(&bytes),
            Err(AvatarError::InvalidInput)
        );
    }
}
