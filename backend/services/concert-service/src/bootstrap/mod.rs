//! BE06 bootstrap producer. Product construction requires a validated immutable
//! release; the test constructor still requires an explicit test flag.
use crate::rpc::{self, proto, VerifiedBootstrap};
use async_trait::async_trait;
use chrono::Utc;
use concert_application::bootstrap::*;
use concert_domain::{
    AssetId, AuthUserId, AvatarSelection, EventId, EventPhase, Preferences, Revision,
};
use serde_json::Value;
use sqlx::{postgres::PgRow, PgPool, Row};
use std::{fmt, path::Path, sync::Arc};
use tonic::{Request, Response, Status};

mod json;
pub mod release;
mod schema;

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
}

fn unavailable(_: sqlx::Error) -> BootstrapError {
    BootstrapError::DependencyUnavailable
}
fn invalid(_: impl fmt::Debug) -> BootstrapError {
    BootstrapError::InvalidResponse
}
fn selection(row: &PgRow) -> Result<AvatarSelection, BootstrapError> {
    let asset = |column| AssetId::new(row.try_get(column).map_err(invalid)?).map_err(invalid);
    Ok(AvatarSelection {
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
    })
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
            sqlx::query(
            "insert into public.concert_avatar_settings(event_id,user_id,contract_version,manifest_id,manifest_version,body_id,eyes_id,hair_id,outfit_id,accessory_id,skin_tone_id) values($1,$2,'1.0.0',$3,$4,$5,$6,$7,$8,$9,$10) on conflict(event_id,user_id) do nothing",
        )
        .bind(event)
        .bind(user)
        .bind(selected.manifest_id.as_str())
        .bind(selected.manifest_version.get())
        .bind(selected.body.as_str())
        .bind(selected.eyes.as_str())
        .bind(selected.hair.as_str())
        .bind(selected.outfit.as_str())
        .bind(selected.accessory.as_ref().map(AssetId::as_str))
        .bind(selected.skin_tone.as_str())
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
            authority()?;
        }
        let avatar = sqlx::query(
            "select revision,manifest_id,manifest_version,body_id,eyes_id,hair_id,outfit_id,accessory_id,skin_tone_id from public.concert_avatar_settings where event_id=$1 and user_id=$2 for update",
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
}

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
