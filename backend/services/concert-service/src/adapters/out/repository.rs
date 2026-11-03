pub mod outbox;
pub mod outbox_maintenance;
pub mod place;
pub mod receipt;

use async_trait::async_trait;
use concert_application::{CoreRepository, PreferenceUpdate, StoreError};
use concert_domain::*;
use sqlx::{postgres::PgRow, PgPool, Row};

#[derive(Clone)]
pub struct PgConcertRepository {
    pool: PgPool,
}
impl PgConcertRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn invalid(_: impl std::fmt::Debug) -> StoreError {
    StoreError::InvalidData
}
fn unavailable(_: sqlx::Error) -> StoreError {
    StoreError::Unavailable
}
fn parse_key(row: &PgRow) -> Result<ParticipantKey, StoreError> {
    Ok(ParticipantKey {
        event_id: EventId::from_uuid(row.try_get("event_id").map_err(invalid)?),
        user_id: AuthUserId::from_uuid(row.try_get("user_id").map_err(invalid)?),
    })
}
fn participant(row: PgRow) -> Result<Participant, StoreError> {
    let value = Participant {
        key: parse_key(&row)?,
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
    };
    validate_times(value.created_at, value.updated_at).map_err(invalid)?;
    Ok(value)
}
const PARTICIPANT_COLUMNS: &str =
    "event_id,user_id,revision,theme,low_power,reduced_motion,sound_enabled,created_at,updated_at";

#[async_trait]
impl CoreRepository for PgConcertRepository {
    async fn event(&self, id: EventId) -> Result<Option<Event>, StoreError> {
        let row = sqlx::query("select event_id,slug,title,phase,revision,created_at,updated_at from public.concert_events where event_id=$1")
            .bind(id.uuid()).fetch_optional(&self.pool).await.map_err(unavailable)?;
        row.map(|row| {
            let value = Event {
                id: EventId::from_uuid(row.try_get("event_id").map_err(invalid)?),
                slug: EventSlug::new(row.try_get("slug").map_err(invalid)?).map_err(invalid)?,
                title: EventTitle::new(row.try_get("title").map_err(invalid)?).map_err(invalid)?,
                phase: row
                    .try_get::<String, _>("phase")
                    .map_err(invalid)?
                    .parse()
                    .map_err(invalid)?,
                revision: Revision::new(row.try_get("revision").map_err(invalid)?)
                    .map_err(invalid)?,
                created_at: row.try_get("created_at").map_err(invalid)?,
                updated_at: row.try_get("updated_at").map_err(invalid)?,
            };
            validate_times(value.created_at, value.updated_at).map_err(invalid)?;
            Ok(value)
        })
        .transpose()
    }
    async fn participant(&self, key: ParticipantKey) -> Result<Option<Participant>, StoreError> {
        sqlx::query(&format!("select {PARTICIPANT_COLUMNS} from public.concert_participants where event_id=$1 and user_id=$2"))
            .bind(key.event_id.uuid()).bind(key.user_id.uuid()).fetch_optional(&self.pool).await.map_err(unavailable)?.map(participant).transpose()
    }
    async fn avatar(&self, key: ParticipantKey) -> Result<Option<AvatarSettings>, StoreError> {
        let row = sqlx::query("select event_id,user_id,revision,manifest_id,manifest_version,body_id,eyes_id,hair_id,outfit_id,accessory_id,skin_tone_id,created_at,updated_at from public.concert_avatar_settings where event_id=$1 and user_id=$2")
            .bind(key.event_id.uuid()).bind(key.user_id.uuid()).fetch_optional(&self.pool).await.map_err(unavailable)?;
        row.map(|row| {
            let asset =
                |column| AssetId::new(row.try_get(column).map_err(invalid)?).map_err(invalid);
            let value = AvatarSettings {
                key: parse_key(&row)?,
                revision: Revision::new(row.try_get("revision").map_err(invalid)?)
                    .map_err(invalid)?,
                selection: AvatarSelection {
                    manifest_id: asset("manifest_id")?,
                    manifest_version: Revision::new(
                        row.try_get("manifest_version").map_err(invalid)?,
                    )
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
            };
            validate_times(value.created_at, value.updated_at).map_err(invalid)?;
            Ok(value)
        })
        .transpose()
    }
    async fn update_preferences(
        &self,
        key: ParticipantKey,
        expected: Revision,
        preferences: Preferences,
    ) -> Result<PreferenceUpdate, StoreError> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        // SHARE conflicts with event closure UPDATE, so phase cannot change between check and commit.
        let phase: Option<String> = sqlx::query_scalar(
            "select phase from public.concert_events where event_id=$1 for share",
        )
        .bind(key.event_id.uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?;
        let Some(phase) = phase else {
            return Ok(PreferenceUpdate::Missing);
        };
        let phase: EventPhase = phase.parse().map_err(invalid)?;
        if !phase.permits_participant_writes() {
            return Ok(PreferenceUpdate::EventReadOnly);
        }
        let revision: Option<i64> = sqlx::query_scalar("select revision from public.concert_participants where event_id=$1 and user_id=$2 for update")
            .bind(key.event_id.uuid()).bind(key.user_id.uuid()).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        let Some(revision) = revision else {
            return Ok(PreferenceUpdate::Missing);
        };
        if revision != expected.get() || expected.next().is_err() {
            return Ok(PreferenceUpdate::Conflict);
        }
        let row = sqlx::query(&format!("update public.concert_participants set theme=$3,low_power=$4,reduced_motion=$5,sound_enabled=$6,revision=revision+1 where event_id=$1 and user_id=$2 and revision=$7 returning {PARTICIPANT_COLUMNS}"))
            .bind(key.event_id.uuid()).bind(key.user_id.uuid()).bind(preferences.theme.as_str()).bind(preferences.low_power).bind(preferences.reduced_motion).bind(preferences.sound_enabled).bind(expected.get())
            .fetch_one(&mut *tx).await.map_err(unavailable)?;
        let value = participant(row)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(PreferenceUpdate::Updated(value))
    }
}

pub mod avatar;

pub mod mural;
