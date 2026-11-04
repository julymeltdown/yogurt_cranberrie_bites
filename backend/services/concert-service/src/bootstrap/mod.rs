//! BE06 bootstrap producer. Product construction requires a validated immutable
//! release; the test constructor still requires an explicit test flag.
use crate::rpc::{self, proto, VerifiedBootstrap};
use async_trait::async_trait;
use chrono::Utc;
use concert_application::bootstrap::*;
use concert_application::world_ticket::{AvatarPin, MapPin};
use concert_domain::{
    AssetId, AuthUserId, AvatarSelection, EventId, EventPhase, ParticipantKey, Preferences, Revision,
};
use serde_json::Value;
use sqlx::{postgres::PgRow, PgPool, Row};
use std::{fmt, path::Path, sync::Arc};
use tonic::{Request, Response, Status};
use uuid::Uuid;

mod json;
pub mod release;
mod schema;
pub mod spatial;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootstrapFeatures {
    pub world: bool,
    pub feed: bool,
    pub meet: bool,
    pub gifts: bool,
    pub wall: bool,
    pub social_sharing: bool,
}
impl Default for BootstrapFeatures {
    fn default() -> Self {
        Self {
            world: false,
            feed: false,
            meet: false,
            gifts: false,
            wall: false,
            social_sharing: false,
        }
    }
}

/// Explicit production configuration. The service validates the complete C03
/// document against the repository's frozen schema URLs and C06 bundle checks.
#[derive(Clone, Debug)]
pub struct ProductReleaseBundle {
    pub bootstrap_json: Vec<u8>,
    pub avatar_manifest: Value,
    pub map_manifest: Value,
    pub theme_manifests: Vec<Value>,
    pub features: BootstrapFeatures,
}
impl ProductReleaseBundle {
    pub fn from_product_files(
        bootstrap: &Path,
        avatar_manifest: &Path,
        map_manifest: &Path,
        themes: [&Path; 3],
        features: BootstrapFeatures,
    ) -> Result<Arc<ValidatedProductRelease>, BootstrapServiceError> {
        release::validate_product_files(
            bootstrap,
            avatar_manifest,
            map_manifest,
            themes,
            features,
            ReleasePolicy::Product,
        )
    }
    /// Keeps the existing product policy and additionally verifies the exact
    /// source geometry, actual resource bytes/dimensions and inert Tiled grids.
    pub fn from_source_bound_files(
        bootstrap: &Path,
        avatar_manifest: &Path,
        map_manifest: &Path,
        themes: [&Path; 3],
        features: BootstrapFeatures,
        resource_root: &Path,
        source_binding: &Path,
    ) -> Result<Arc<ValidatedProductRelease>, BootstrapServiceError> {
        let release = Self::from_product_files(bootstrap, avatar_manifest, map_manifest, themes, features)?;
        spatial::validate_product(&release, map_manifest, source_binding, resource_root)?;
        Ok(release)
    }
    /// Test-only entry. It cannot be selected by the product filesystem API and
    /// is intended for the isolated native harness.
    pub fn from_synthetic_files(
        bootstrap: &Path,
        avatar_manifest: &Path,
        map_manifest: &Path,
        themes: [&Path; 3],
        features: BootstrapFeatures,
        allow_synthetic: bool,
    ) -> Result<Arc<ValidatedProductRelease>, BootstrapServiceError> {
        if !allow_synthetic {
            return Err(BootstrapServiceError::ReleaseUnavailable);
        }
        release::validate_product_files(
            bootstrap,
            avatar_manifest,
            map_manifest,
            themes,
            features,
            ReleasePolicy::ExplicitSynthetic,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleasePolicy {
    Product,
    ExplicitSynthetic,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BootstrapServiceError {
    ReleaseUnavailable,
}

pub struct BootstrapPostgresRepository {
    pool: PgPool,
}
impl BootstrapPostgresRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Read the current event/participant/restriction state without creating or
    /// changing participation. It shares the same locked tables as Bootstrap,
    /// but has no default-write side effect.
    pub async fn world_admission(
        &self,
        event: EventId,
        user: AuthUserId,
    ) -> Result<(i64, i64, bool), BootstrapError> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        let phase: String = sqlx::query_scalar(
            "select phase from public.concert_events where event_id=$1 for share",
        )
        .bind(event.uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?
        .ok_or(BootstrapError::EventNotFound)?;
        if !matches!(phase.as_str(), "OPEN" | "LIVE") {
            return Err(BootstrapError::EventNotOpen);
        }
        let state: Option<(String, i64)> = sqlx::query_as(
            "select state,restriction_revision from public.concert_event_admissions where event_id=$1 and user_id=$2 for share",
        )
        .bind(event.uuid())
        .bind(user.uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?;
        let participant: Option<i64> = sqlx::query_scalar(
            "select revision from public.concert_participants where event_id=$1 and user_id=$2 for share",
        )
        .bind(event.uuid())
        .bind(user.uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        let (state, restriction_revision) = state.ok_or(BootstrapError::Denied)?;
        if state != "ACTIVE" {
            return Err(BootstrapError::Denied);
        }
        let participant_revision = participant.ok_or(BootstrapError::Denied)?;
        let permitted = matches!(restriction_revision, 0..=9_007_199_254_740_991)
            && matches!(participant_revision, 1..=9_007_199_254_740_991);
        Ok((restriction_revision, participant_revision, permitted))
    }
}

fn unavailable(_: sqlx::Error) -> BootstrapError {
    BootstrapError::DependencyUnavailable
}
fn invalid(_: impl fmt::Debug) -> BootstrapError {
    BootstrapError::InvalidResponse
}
fn selection(row: &PgRow) -> Result<AvatarSelection, BootstrapError> {
    crate::adapters::out::repository::avatar::selection_from_row(row).map_err(invalid)
}

#[async_trait]
impl BootstrapRepository for BootstrapPostgresRepository {
    async fn bootstrap(
        &self,
        admission: &BootstrapAdmission,
        defaults: &BootstrapDefaults,
    ) -> Result<BootstrapRecord, BootstrapError> {
        self.bootstrap_checked(admission, defaults, &|_| Ok(()))
            .await
    }
}
impl BootstrapPostgresRepository {
    /// Runs authority/response checks while all transaction locks are held.
    pub async fn bootstrap_checked(
        &self,
        admission: &BootstrapAdmission,
        defaults: &BootstrapDefaults,
        check: &(dyn Fn(&BootstrapRecord) -> Result<(), BootstrapError> + Send + Sync),
    ) -> Result<BootstrapRecord, BootstrapError> {
        self.bootstrap_with_checks(admission, defaults, check, &|| Ok(()))
            .await
    }
    async fn bootstrap_with_checks(
        &self,
        admission: &BootstrapAdmission,
        defaults: &BootstrapDefaults,
        check: &(dyn Fn(&BootstrapRecord) -> Result<(), BootstrapError> + Send + Sync),
        authority: &(dyn Fn() -> Result<(), BootstrapError> + Send + Sync),
    ) -> Result<BootstrapRecord, BootstrapError> {
        authority()?;
        let key = admission.key();
        let event = key.event_id.uuid();
        let user = key.user_id.uuid();
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        authority()?;
        let (phase, event_revision, event_slug, event_title, event_timezone): (String, i64, String, String, String) = sqlx::query_as(
            "select phase,revision,slug,title,timezone from public.concert_events where event_id=$1 for update",
        )
        .bind(event)
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?
        .ok_or(BootstrapError::EventNotFound)?;
        authority()?;
        let phase: EventPhase = phase.parse().map_err(invalid)?;
        if !matches!(
            phase,
            EventPhase::Open | EventPhase::Live | EventPhase::ReadOnly | EventPhase::Archived
        ) {
            return Err(BootstrapError::EventNotOpen);
        }
        let writable = matches!(phase, EventPhase::Open | EventPhase::Live);
        if writable {
            sqlx::query(
            "insert into public.concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE') on conflict(event_id,user_id) do nothing",
        )
        .bind(event)
        .bind(user)
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
            authority()?;
        }
        let (state, restriction_revision): (String, i64) = sqlx::query_as(
            "select state,restriction_revision from public.concert_event_admissions where event_id=$1 and user_id=$2 for update",
        )
        .bind(event)
        .bind(user)
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?
        .ok_or(BootstrapError::Denied)?;
        authority()?;
        if state != "ACTIVE" {
            return Err(BootstrapError::Denied);
        }
        if writable {
            sqlx::query(
            "insert into public.concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,$3,false,false,false) on conflict(event_id,user_id) do nothing",
        )
        .bind(event)
        .bind(user)
        .bind(defaults.theme.as_str())
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
            authority()?;
        }
        let participant = sqlx::query(
            "select revision,theme,low_power,reduced_motion,sound_enabled from public.concert_participants where event_id=$1 and user_id=$2 for update",
        )
        .bind(event)
        .bind(user)
        .fetch_one(&mut *tx)
        .await
        .map_err(unavailable)?;
        authority()?;
        if writable {
            let selected = defaults.avatar.clone();
            let columns = crate::adapters::out::repository::avatar::storage_columns(&selected);
            sqlx::query(
            "insert into public.concert_avatar_settings(event_id,user_id,contract_version,manifest_id,manifest_version,body_id,eyes_id,hair_id,outfit_id,accessory_id,skin_tone_id,representation,preset_id) values($1,$2,$11,$3,$4,$5,$6,$7,$8,$9,$10,$12,$13) on conflict(event_id,user_id) do nothing",
        )
        .bind(event)
        .bind(user)
        .bind(selected.manifest_id.as_str())
        .bind(selected.manifest_version.get())
        .bind(columns.body)
        .bind(columns.eyes)
        .bind(columns.hair)
        .bind(columns.outfit)
        .bind(columns.accessory)
        .bind(columns.skin_tone)
        .bind(columns.contract_version)
        .bind(columns.representation)
        .bind(columns.preset_id)
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
            authority()?;
        }
        let avatar = sqlx::query(
            "select revision,contract_version,representation,preset_id,manifest_id,manifest_version,body_id,eyes_id,hair_id,outfit_id,accessory_id,skin_tone_id from public.concert_avatar_settings where event_id=$1 and user_id=$2 for update",
        )
        .bind(event)
        .bind(user)
        .fetch_one(&mut *tx)
        .await
        .map_err(unavailable)?;
        authority()?;
        let completed_at = Utc::now();
        let record = BootstrapRecord {
            event_id: key.event_id,
            event_revision,
            event_slug,
            event_title,
            event_timezone,
            phase,
            restriction_revision,
            active: true,
            participant_revision: participant.try_get("revision").map_err(invalid)?,
            preferences: Preferences {
                theme: participant
                    .try_get::<String, _>("theme")
                    .map_err(invalid)?
                    .parse()
                    .map_err(invalid)?,
                low_power: participant.try_get("low_power").map_err(invalid)?,
                reduced_motion: participant.try_get("reduced_motion").map_err(invalid)?,
                sound_enabled: participant.try_get("sound_enabled").map_err(invalid)?,
            },
            avatar_revision: avatar.try_get("revision").map_err(invalid)?,
            avatar: selection(&avatar)?,
            completed_at,
        };
        check(&record)?;
        authority()?;
        tx.commit().await.map_err(unavailable)?;
        authority()?;
        Ok(record)
    }
}

pub struct BootstrapService {
    repository: Arc<BootstrapPostgresRepository>,
    release: Arc<ValidatedProductRelease>,
}
impl BootstrapService {
    pub fn new(pool: PgPool, release: Arc<ValidatedProductRelease>) -> Self {
        Self {
            repository: Arc::new(BootstrapPostgresRepository::new(pool)),
            release,
        }
    }
    pub fn release(&self) -> &Arc<ValidatedProductRelease> {
        &self.release
    }
    pub fn repository(&self) -> &Arc<BootstrapPostgresRepository> {
        &self.repository
    }

    pub fn release_scope(&self) -> Result<(EventId, i64), BootstrapServiceError> {
        let document: serde_json::Value =
            serde_json::from_slice(self.release.release().bootstrap_json())
                .map_err(|_| BootstrapServiceError::ReleaseUnavailable)?;
        let event = document["event"]["event_id"]
            .as_str()
            .and_then(|value| value.parse::<Uuid>().ok())
            .filter(|value| !value.is_nil())
            .ok_or(BootstrapServiceError::ReleaseUnavailable)?;
        let revision = document["event"]["revision"]
            .as_i64()
            .filter(|value| (1..=9_007_199_254_740_991).contains(value))
            .ok_or(BootstrapServiceError::ReleaseUnavailable)?;
        Ok((EventId::from_uuid(event), revision))
    }

    /// Configuration-only current release pins. They are not a claim that the
    /// account/event has been authorized; the world composition must combine
    /// these with fresh Auth and Concert ownership reads before ticket issuance.
    pub fn canonical_pins(
        &self,
        event: EventId,
    ) -> Result<WorldReleasePins, BootstrapServiceError> {
        let (release_event, _) = self.release_scope()?;
        if event != release_event {
            return Err(BootstrapServiceError::ReleaseUnavailable);
        }
        let document: serde_json::Value =
            serde_json::from_slice(self.release.release().bootstrap_json())
                .map_err(|_| BootstrapServiceError::ReleaseUnavailable)?;
        let map = document["map_manifest"]
            .as_object()
            .ok_or(BootstrapServiceError::ReleaseUnavailable)?;
        let avatar = document["avatar_manifest"]
            .as_object()
            .ok_or(BootstrapServiceError::ReleaseUnavailable)?;
        let asset = |value: Option<&serde_json::Value>| {
            AssetId::new(
                value
                    .and_then(serde_json::Value::as_str)
                    .ok_or(BootstrapServiceError::ReleaseUnavailable)?
                    .to_owned(),
            )
            .map_err(|_| BootstrapServiceError::ReleaseUnavailable)
        };
        let revision = |value: Option<&serde_json::Value>| {
            Revision::new(
                value
                    .and_then(serde_json::Value::as_i64)
                    .ok_or(BootstrapServiceError::ReleaseUnavailable)?,
            )
            .map_err(|_| BootstrapServiceError::ReleaseUnavailable)
        };
        Ok(WorldReleasePins {
            event_id: event,
            map: MapPin::new(
                asset(map.get("map_id"))?,
                revision(map.get("map_revision"))?,
            ),
            avatar: AvatarPin::new(
                asset(avatar.get("manifest_id"))?,
                revision(avatar.get("manifest_version"))?,
            ),
            captured_at: Utc::now(),
        })
    }

    /// Canonical event/participant/restriction read for World issuance. The
    /// underlying query shares Bootstrap's event/admission locks but never
    /// invokes Bootstrap's participant/default write path.
    pub async fn world_admission(
        &self,
        user: AuthUserId,
        event: EventId,
        session_family_id: uuid::Uuid,
        request_id: uuid::Uuid,
    ) -> Result<WorldAdmission, BootstrapError> {
        let admission = BootstrapAdmission::from_verified(
            user,
            event,
            session_family_id,
            request_id,
            "world admission".into(),
            None,
            Utc::now(),
        )?;
        let session = concert_application::identity::SessionId::new(session_family_id.to_string())
            .map_err(|_| BootstrapError::Unauthenticated)?;
        let (restriction_revision, _, permitted) = self
            .repository
            .world_admission(admission.key().event_id, admission.key().user_id)
            .await?;
        if !permitted {
            return Err(BootstrapError::Denied);
        }
        Ok(WorldAdmission {
            key: ParticipantKey {
                event_id: event,
                user_id: user,
            },
            session_id: session,
            event_id: event,
            restriction_revision,
        })
    }
}

pub struct WorldAdmission {
    pub key: ParticipantKey,
    pub session_id: concert_application::identity::SessionId,
    pub event_id: EventId,
    pub restriction_revision: i64,
}

pub type WorldReleasePins = concert_application::world_ticket::ReleasePins;

#[tonic::async_trait]
impl proto::concert_service_server::ConcertService for BootstrapService {
    async fn bootstrap(
        &self,
        request: Request<proto::BootstrapRequest>,
    ) -> Result<Response<proto::BootstrapResponse>, Status> {
        let verified: VerifiedBootstrap = rpc::authorize_bootstrap(&request)?;
        verified.check()?;
        let admission = BootstrapAdmission::from_verified(
            AuthUserId::from_uuid(verified.user_id()),
            EventId::from_uuid(verified.event_id()),
            verified.session_family_id(),
            verified.request_id(),
            verified.display_name().to_owned(),
            verified.bio().map(str::to_owned),
            Utc::now(),
        )
        .map_err(status_from_bootstrap)?;
        let release = self.release.release();
        let validate_record = |record: &BootstrapRecord| {
            verified
                .check()
                .map_err(|_| BootstrapError::Unauthenticated)?;
            let result = BootstrapResult {
                record: record.clone(),
                release: release.clone(),
            };
            release::bind_response(
                release.bootstrap_json(),
                &admission,
                &result,
                self.release.source_digest(),
            )
            .map(|_| ())
        };
        let authority = || {
            verified
                .check()
                .map_err(|_| BootstrapError::Unauthenticated)
        };
        let record = tokio::time::timeout_at(
            verified.deadline().stop(),
            self.repository.bootstrap_with_checks(
                &admission,
                release.defaults(),
                &validate_record,
                &authority,
            ),
        )
        .await
        .map_err(|_| Status::deadline_exceeded("Bootstrap deadline elapsed"))?
        .map_err(status_from_bootstrap)?;
        verified.check()?;
        let result = BootstrapResult {
            record,
            release: release.clone(),
        };
        let document = release::bind_response(
            release.bootstrap_json(),
            &admission,
            &result,
            self.release.source_digest(),
        )
        .map_err(status_from_bootstrap)?;
        verified.check()?;
        let mut response = Response::new(proto::BootstrapResponse {
            bootstrap_json: document,
        });
        response.metadata_mut().insert(
            "cache-control",
            "no-store"
                .parse()
                .map_err(|_| Status::internal("Response metadata unavailable"))?,
        );
        Ok(response)
    }
}

fn status_from_bootstrap(error: BootstrapError) -> Status {
    match error {
        BootstrapError::InvalidInput | BootstrapError::InvalidResponse => {
            Status::invalid_argument("Invalid bootstrap input")
        }
        BootstrapError::Unauthenticated => Status::unauthenticated("Authentication required"),
        BootstrapError::Denied | BootstrapError::EventNotOpen => {
            Status::permission_denied("Bootstrap is not permitted")
        }
        BootstrapError::EventNotFound => Status::not_found("Event is unavailable"),
        BootstrapError::RestrictionUnavailable
        | BootstrapError::ReleaseUnavailable
        | BootstrapError::DependencyUnavailable => Status::unavailable("Bootstrap unavailable"),
    }
}
