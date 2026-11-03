//! BE07 native persistence. Production HTTP/RPC composition is deliberately
//! delegated; no read-only Bootstrap proof is promoted to mutation authority.
use async_trait::async_trait;
use concert_application::{avatar::*, bootstrap::ValidatedProductRelease};
use concert_domain::{
    avatar::{config_value, AvatarCatalog, SetAvatar, MAX_REQUEST_BYTES},
    *,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgRow, PgPool, Row};
use std::sync::Arc;

pub struct AvatarPersistence {
    repository: PgAvatarRepository,
    release: Arc<ValidatedProductRelease>,
    catalog: AvatarCatalog,
}
impl AvatarPersistence {
    pub fn from_product(
        pool: PgPool,
        release: Arc<ValidatedProductRelease>,
    ) -> Result<Self, AvatarError> {
        Self::build(pool, release, false)
    }
    /// Explicit metadata-test seam. Never used by product construction.
    pub fn from_synthetic_test(
        pool: PgPool,
        release: Arc<ValidatedProductRelease>,
        allow_synthetic: bool,
    ) -> Result<Self, AvatarError> {
        if !allow_synthetic {
            return Err(AvatarError::Unavailable);
        }
        Self::build(pool, release, true)
    }
    fn build(
        pool: PgPool,
        release: Arc<ValidatedProductRelease>,
        synthetic: bool,
    ) -> Result<Self, AvatarError> {
        let doc: Value =
            serde_json::from_slice(release.release().bootstrap_json()).map_err(invalid)?;
        let avatar = &doc["avatar_manifest"];
        // This is an already validated immutable C03/C06 release. Rebind the
        // exact avatar content hash before projecting its inventory.
        let mut unhashed = avatar.clone();
        unhashed
            .as_object_mut()
            .ok_or(AvatarError::Unavailable)?
            .remove("content_hash");
        let bytes = concert_domain::place::canonical_json_bytes(
            &serde_json::to_vec(&unhashed).map_err(invalid)?,
        )
        .map_err(invalid)?;
        let hash = format!("sha256:{:x}", Sha256::digest(bytes));
        if avatar["content_hash"].as_str() != Some(hash.as_str()) {
            return Err(AvatarError::Unavailable);
        }
        if !synthetic {
            let mut resources = vec![avatar, &doc["map_manifest"]];
            resources.extend(
                doc["theme_manifests"]
                    .as_array()
                    .ok_or(AvatarError::Unavailable)?,
            );
            if resources.len() != 5
                || resources.iter().any(|v| {
                    v["provenance"]["review_state"] != "REVIEWED"
                        || v["provenance"]["source_kind"] != "ORIGINAL_ART"
                })
                || avatar["atlas"]["texture"]["image_state"] != "PROVIDED"
                || doc["map_manifest"]["tilemap"]["resource_state"] != "PROVIDED"
                || doc["theme_manifests"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v["texture"]["image_state"] != "PROVIDED")
            {
                return Err(AvatarError::Unavailable);
            }
        }
        let catalog = AvatarCatalog::from_validated_manifest(avatar).map_err(invalid)?;
        catalog
            .validate(&release.release().defaults().avatar)
            .map_err(invalid)?;
        Ok(Self {
            repository: PgAvatarRepository::new(pool),
            release,
            catalog,
        })
    }
    pub fn release_digest(&self) -> &str {
        self.release.source_digest()
    }
    pub async fn read(
        &self,
        key: ParticipantKey,
        authority: &dyn AvatarAuthority,
    ) -> Result<AvatarSnapshot, AvatarError> {
        self.execute(key, AvatarMutation::Read, authority).await
    }
    pub async fn set_avatar(
        &self,
        key: ParticipantKey,
        bytes: &[u8],
        authority: &dyn AvatarAuthority,
    ) -> Result<Value, AvatarError> {
        authorize(
            authority,
            key,
            AvatarOperation::SetAvatar,
            self.release_digest(),
        )?;
        let command = SetAvatar::parse(bytes).map_err(|_| AvatarError::InvalidInput)?;
        let state = self
            .execute(key, AvatarMutation::Avatar(command), authority)
            .await?;
        Ok(avatar_state(&state.avatar))
    }
    /// The preference-only subset of UpdateParticipantRequest. Profile writes
    /// belong to canonical Profile ownership and must be separately composed.
    pub async fn set_preferences(
        &self,
        key: ParticipantKey,
        bytes: &[u8],
        authority: &dyn AvatarAuthority,
    ) -> Result<AvatarSnapshot, AvatarError> {
        authorize(
            authority,
            key,
            AvatarOperation::SetPreferences,
            self.release_digest(),
        )?;
        let (expected, value) = parse_preferences(bytes)?;
        self.execute(
            key,
            AvatarMutation::Preferences { expected, value },
            authority,
        )
        .await
    }
    async fn execute(
        &self,
        key: ParticipantKey,
        mutation: AvatarMutation,
        authority: &dyn AvatarAuthority,
    ) -> Result<AvatarSnapshot, AvatarError> {
        self.repository
            .execute(
                key,
                mutation,
                &self.catalog,
                self.release_digest(),
                authority,
            )
            .await
    }
}
pub fn avatar_state(value: &AvatarSettings) -> Value {
    json!({"revision":value.revision.get(),"config":config_value(&value.selection)})
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
struct PreferenceRequest {
    expected_revision: i64,
    preferences: WirePreferences,
}
pub fn parse_preferences(bytes: &[u8]) -> Result<(Revision, Preferences), AvatarError> {
    if bytes.is_empty() || bytes.len() > MAX_REQUEST_BYTES {
        return Err(AvatarError::InvalidInput);
    }
    let raw: PreferenceRequest =
        serde_json::from_slice(bytes).map_err(|_| AvatarError::InvalidInput)?;
    Ok((
        Revision::new(raw.expected_revision).map_err(|_| AvatarError::InvalidInput)?,
        Preferences {
            theme: raw
                .preferences
                .theme
                .parse()
                .map_err(|_| AvatarError::InvalidInput)?,
            low_power: raw.preferences.low_power,
            reduced_motion: raw.preferences.reduced_motion,
            sound_enabled: raw.preferences.sound_enabled,
        },
    ))
}
fn unavailable(_: sqlx::Error) -> AvatarError {
    AvatarError::Unavailable
}
fn invalid(_: impl std::fmt::Debug) -> AvatarError {
    AvatarError::InvalidState
}
#[derive(Clone)]
pub struct PgAvatarRepository {
    pool: PgPool,
}
impl PgAvatarRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
#[async_trait]
impl AvatarRepository for PgAvatarRepository {
    async fn execute(
        &self,
        key: ParticipantKey,
        mutation: AvatarMutation,
        catalog: &AvatarCatalog,
        digest: &str,
        authority: &dyn AvatarAuthority,
    ) -> Result<AvatarSnapshot, AvatarError> {
        let op = mutation.operation();
        let check = || authorize(authority, key, op, digest);
        check()?;
        if let AvatarMutation::Avatar(command) = &mutation {
            catalog
                .validate(&command.selection)
                .map_err(|_| AvatarError::InvalidInput)?;
        }
        let event = key.event_id.uuid();
        let user = key.user_id.uuid();
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        check()?;
        let phase: Option<String> = sqlx::query_scalar(
            "select phase from public.concert_events where event_id=$1 for update",
        )
        .bind(event)
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?;
        check()?;
        let phase: EventPhase = phase
            .ok_or(AvatarError::Missing)?
            .parse()
            .map_err(invalid)?;
        if phase == EventPhase::Draft {
            return Err(AvatarError::Missing);
        }
        if op != AvatarOperation::Read && !phase.permits_participant_writes() {
            return Err(AvatarError::ReadOnly);
        }
        let state: Option<String> = sqlx::query_scalar("select state from public.concert_event_admissions where event_id=$1 and user_id=$2 for update")
            .bind(event).bind(user).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        check()?;
        if state.as_deref() != Some("ACTIVE") {
            return Err(AvatarError::Denied);
        }
        let row = sqlx::query("select revision,theme,low_power,reduced_motion,sound_enabled,created_at,updated_at from public.concert_participants where event_id=$1 and user_id=$2 for update")
            .bind(event).bind(user).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        check()?;
        let mut participant = participant(row.ok_or(AvatarError::Missing)?, key)?;
        let row = sqlx::query("select revision,contract_version,manifest_id,manifest_version,body_id,eyes_id,hair_id,outfit_id,accessory_id,skin_tone_id,created_at,updated_at from public.concert_avatar_settings where event_id=$1 and user_id=$2 for update")
            .bind(event).bind(user).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        check()?;
        let mut avatar = avatar(row.ok_or(AvatarError::Missing)?, key)?;
        catalog.validate(&avatar.selection).map_err(invalid)?;
        match mutation {
            AvatarMutation::Read => {}
            AvatarMutation::Avatar(command) => {
                if avatar.revision != command.expected {
                    return Err(AvatarError::Conflict);
                }
                let next = command.expected.next().map_err(|_| AvatarError::Conflict)?;
                let s = command.selection;
                let updated: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("update public.concert_avatar_settings set revision=$3,manifest_id=$4,manifest_version=$5,body_id=$6,eyes_id=$7,hair_id=$8,outfit_id=$9,accessory_id=$10,skin_tone_id=$11 where event_id=$1 and user_id=$2 returning updated_at")
                    .bind(event).bind(user).bind(next.get()).bind(s.manifest_id.as_str()).bind(s.manifest_version.get())
                    .bind(s.body.as_str()).bind(s.eyes.as_str()).bind(s.hair.as_str()).bind(s.outfit.as_str())
                    .bind(s.accessory.as_ref().map(AssetId::as_str)).bind(s.skin_tone.as_str())
                    .fetch_one(&mut *tx).await.map_err(unavailable)?;
                check()?;
                avatar.selection = s;
                avatar.revision = next;
                avatar.updated_at = updated;
            }
            AvatarMutation::Preferences { expected, value } => {
                if participant.revision != expected {
                    return Err(AvatarError::Conflict);
                }
                let next = expected.next().map_err(|_| AvatarError::Conflict)?;
                let updated: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("update public.concert_participants set revision=$3,theme=$4,low_power=$5,reduced_motion=$6,sound_enabled=$7 where event_id=$1 and user_id=$2 returning updated_at")
                    .bind(event).bind(user).bind(next.get()).bind(value.theme.as_str()).bind(value.low_power).bind(value.reduced_motion).bind(value.sound_enabled)
                    .fetch_one(&mut *tx).await.map_err(unavailable)?;
                check()?;
                participant.revision = next;
                participant.preferences = value;
                participant.updated_at = updated;
            }
        }
        catalog.validate(&avatar.selection).map_err(invalid)?;
        validate_times(avatar.created_at, avatar.updated_at).map_err(invalid)?;
        validate_times(participant.created_at, participant.updated_at).map_err(invalid)?;
        check()?;
        tx.commit().await.map_err(unavailable)?;
        check()?;
        Ok(AvatarSnapshot {
            avatar,
            participant,
        })
    }
}
fn participant(row: PgRow, key: ParticipantKey) -> Result<Participant, AvatarError> {
    Ok(Participant {
        key,
        revision: Revision::new(row.try_get("revision").map_err(invalid)?).map_err(invalid)?,
        preferences: Preferences {
            theme: row
                .try_get::<String, _>("theme")
                .map_err(invalid)?
                .parse()
                .map_err(invalid)?,
            low_power: row.try_get("low_power").map_err(invalid)?,
            reduced_motion: row.try_get("reduced_motion").map_err(invalid)?,
            sound_enabled: row.try_get("sound_enabled").map_err(invalid)?,
        },
        created_at: row.try_get("created_at").map_err(invalid)?,
        updated_at: row.try_get("updated_at").map_err(invalid)?,
    })
}
fn avatar(row: PgRow, key: ParticipantKey) -> Result<AvatarSettings, AvatarError> {
    let asset = |column| AssetId::new(row.try_get(column).map_err(invalid)?).map_err(invalid);
    if row
        .try_get::<String, _>("contract_version")
        .map_err(invalid)?
        != "1.0.0"
    {
        return Err(AvatarError::InvalidState);
    }
    Ok(AvatarSettings {
        key,
        revision: Revision::new(row.try_get("revision").map_err(invalid)?).map_err(invalid)?,
        selection: AvatarSelection {
            manifest_id: asset("manifest_id")?,
            manifest_version: Revision::new(row.try_get("manifest_version").map_err(invalid)?)
                .map_err(invalid)?,
            body: asset("body_id")?,
            eyes: asset("eyes_id")?,
            hair: asset("hair_id")?,
            outfit: asset("outfit_id")?,
            accessory: row
                .try_get::<Option<String>, _>("accessory_id")
                .map_err(invalid)?
                .map(AssetId::new)
                .transpose()
                .map_err(invalid)?,
            skin_tone: asset("skin_tone_id")?,
        },
        created_at: row.try_get("created_at").map_err(invalid)?,
        updated_at: row.try_get("updated_at").map_err(invalid)?,
    })
}
