//! Durable Concert half of the canonical Profile/event participant protocol.
//! This module is not product composition. The actual owner-scope adapter must
//! enroll the same connection's transaction before protected effects.
use super::update::{Field, ParticipantUpdate};
use crate::adapters::out::relationship_authority::guarded::{
    proto as lease, wire_binding_digest, GuardBinding,
};
use crate::participant::context::VerifiedParticipantRequest;
use crate::participant::Operation;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::avatar::AvatarError;
use concert_domain::{EventPhase, Preferences, Revision};
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq)]
pub struct NativeTransaction {
    pub(crate) system_identifier: String,
    pub(crate) database_oid: u32,
    pub(crate) full_xid: String,
    pub(crate) backend_pid: i32,
    pub(crate) backend_started: DateTime<Utc>,
}
impl std::fmt::Debug for NativeTransaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeTransaction([REDACTED])")
    }
}
impl NativeTransaction {
    async fn capture(tx: &mut Transaction<'_, Postgres>) -> Result<Self, AvatarError> {
        let row = sqlx::query("select (select system_identifier::text from pg_control_system()) as system_identifier,(select oid::bigint from pg_database where datname=current_database()) as database_oid,pg_current_xact_id()::text as full_xid,pg_backend_pid() as backend_pid,backend_start from pg_stat_activity where pid=pg_backend_pid()")
            .fetch_one(&mut **tx).await.map_err(unavailable)?;
        let oid: i64 = row.try_get("database_oid").map_err(unavailable)?;
        Ok(Self {
            system_identifier: row.try_get("system_identifier").map_err(unavailable)?,
            database_oid: u32::try_from(oid)
                .ok()
                .filter(|v| *v > 0)
                .ok_or(AvatarError::Unavailable)?,
            full_xid: row.try_get("full_xid").map_err(unavailable)?,
            backend_pid: row.try_get("backend_pid").map_err(unavailable)?,
            backend_started: row.try_get("backend_start").map_err(unavailable)?,
        })
    }
    pub fn system_identifier(&self) -> &str {
        &self.system_identifier
    }
    pub fn database_oid(&self) -> u32 {
        self.database_oid
    }
    pub fn full_xid(&self) -> &str {
        &self.full_xid
    }
    pub fn backend_pid(&self) -> i32 {
        self.backend_pid
    }
    pub fn backend_started(&self) -> DateTime<Utc> {
        self.backend_started
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanonicalProfileVersion {
    pub epoch: Uuid,
    pub revision: Revision,
}
#[derive(Clone)]
pub struct ExpectedProfileEffect {
    pub(crate) version: CanonicalProfileVersion,
    pub(crate) gid: String,
    pub(crate) digest: [u8; 32],
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProfileTerminalOutcome {
    Committed,
    Aborted,
}
pub(crate) struct ProfileTerminalReceipt {
    operation: Uuid,
    gid: String,
    digest: [u8; 32],
    native: NativeTransaction,
    outcome: ProfileTerminalOutcome,
}
impl ProfileTerminalReceipt {
    pub(crate) fn outcome(&self) -> ProfileTerminalOutcome {
        self.outcome
    }
    /// The owning client calls this ONLY after authenticating and validating the
    /// bounded canonical Profile response. No HTTP/request field supplies it.
    pub(crate) fn from_owner_response(
        operation: Uuid,
        expected: &ExpectedProfileEffect,
        returned_gid: &str,
        returned_effect: [u8; 32],
        native: NativeTransaction,
        outcome: ProfileTerminalOutcome,
    ) -> Result<Self, AvatarError> {
        let decimal = |text: &str| text.parse::<u64>().ok().is_some_and(|value| value > 0);
        if operation.is_nil()
            || expected.gid != format!("mcr_sp_{}", operation.simple())
            || returned_gid != expected.gid
            || returned_effect != expected.digest
            || !decimal(&native.system_identifier)
            || !decimal(&native.full_xid)
            || native.database_oid == 0
            || native.backend_pid <= 0
        {
            return Err(AvatarError::Denied);
        }
        Ok(Self {
            operation,
            gid: expected.gid.clone(),
            digest: expected.digest,
            native,
            outcome,
        })
    }
}
impl ExpectedProfileEffect {
    pub fn for_request(
        request: &VerifiedParticipantRequest,
        binding_digest: [u8; 32],
        version: CanonicalProfileVersion,
    ) -> Result<Self, AvatarError> {
        request
            .parent_authority()
            .map_err(|_| AvatarError::Denied)?;
        if request.operation() != Operation::UpdatePreferences {
            return Err(AvatarError::Denied);
        }
        if version.epoch.is_nil() || binding_digest == [0; 32] {
            return Err(AvatarError::InvalidInput);
        }
        let update = ParticipantUpdate::parse(request.raw_body())?;
        if !update.changes_profile() {
            return Err(AvatarError::InvalidInput);
        }
        let mut digest = Sha256::new();
        digest.update(b"mcr profile self effect v1\0");
        digest.update(binding_digest);
        digest.update(request.request_id().as_bytes());
        digest.update(version.epoch.as_bytes());
        digest.update(version.revision.get().to_be_bytes());
        digest.update(request.body_sha256());
        Ok(Self {
            version,
            gid: format!("mcr_sp_{}", request.request_id().simple()),
            digest: digest.finalize().into(),
        })
    }
    pub fn gid(&self) -> &str {
        &self.gid
    }
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
    pub fn version(&self) -> CanonicalProfileVersion {
        self.version
    }
}

/// Implement only using the actual held canonical owner scope. No account read,
/// client boolean, profile observation, or task-local binding is a write grant.
#[async_trait]
pub trait CoordinatorAuthority: Send + Sync {
    async fn check(&self, request: &VerifiedParticipantRequest) -> Result<(), AvatarError>;
    async fn track_concert_transaction(
        &self,
        request: &VerifiedParticipantRequest,
        transaction: &NativeTransaction,
        effect_digest: [u8; 32],
    ) -> Result<(), AvatarError>;
}

#[derive(Clone)]
struct Binding {
    actor: Uuid,
    event: Uuid,
    family: Uuid,
    operation: Uuid,
    parent_digest: [u8; 32],
    body_digest: [u8; 32],
    guard_digest: [u8; 32],
    owner_binding_proto: Vec<u8>,
    release: String,
}
impl Binding {
    fn capture(
        request: &VerifiedParticipantRequest,
        guard_digest: [u8; 32],
    ) -> Result<Self, AvatarError> {
        let parent = request
            .parent_authority()
            .map_err(|_| AvatarError::Denied)?;
        let canonical = GuardBinding::from_parent(
            parent,
            request.key().user_id.uuid(),
            request.key().event_id.uuid(),
            request.session_family_id(),
            request.request_id(),
            super::guarded::operation_label(request.operation()),
            request.http_method(),
            request.http_target(),
            request.body_sha256(),
            request.deadline(),
        )
        .map_err(|_| AvatarError::Denied)?;
        if canonical.digest() != guard_digest {
            return Err(AvatarError::Denied);
        }
        Ok(Self {
            actor: request.key().user_id.uuid(),
            event: request.key().event_id.uuid(),
            family: request.session_family_id(),
            operation: request.request_id(),
            parent_digest: parent.parent_digest(),
            body_digest: request.body_sha256(),
            guard_digest,
            owner_binding_proto: canonical.wire([]).encode_to_vec(),
            release: request.release_digest().into(),
        })
    }
    fn check(&self, request: &VerifiedParticipantRequest) -> Result<(), AvatarError> {
        let candidate = Self::capture(request, self.guard_digest)?;
        if self.actor != candidate.actor
            || self.event != candidate.event
            || self.family != candidate.family
            || self.operation != candidate.operation
            || self.parent_digest != candidate.parent_digest
            || self.body_digest != candidate.body_digest
            || self.release != candidate.release
        {
            return Err(AvatarError::Denied);
        }
        Ok(())
    }
}

pub struct ParticipantProfileJournal {
    pool: PgPool,
    release_digest: String,
}
pub(crate) struct RecoveryOperation {
    pub operation: Uuid,
    pub actor: Uuid,
    pub binding: lease::GuardBinding,
    pub expected_profile: ExpectedProfileEffect,
}
pub(crate) type RecoveryCursor = (DateTime<Utc>, Uuid);
pub struct ParticipantProfileTransaction {
    tx: Transaction<'static, Postgres>,
    pool: PgPool,
    binding: Binding,
    native: NativeTransaction,
    update: ParticipantUpdate,
    expected_profile: ExpectedProfileEffect,
    preferences: Option<Preferences>,
}
impl ParticipantProfileJournal {
    /// Keyset pagination gives unresolved old operations bounded retries without
    /// starving newer ones. Missing/timeout outcomes never manufacture ABORT.
    pub(crate) async fn recovery_page(
        &self,
        after: Option<RecoveryCursor>,
    ) -> Result<Vec<RecoveryCursor>, AvatarError> {
        let (time, operation) = after.map_or((None, None), |(time, operation)| {
            (Some(time), Some(operation))
        });
        sqlx::query_as("select registered_at,operation_id from public.concert_participant_profile_operations op where (op.decision='COMMIT' or exists(select 1 from public.concert_participant_profile_prepare_attempts a where a.operation_id=op.operation_id)) and not exists(select 1 from public.concert_participant_profile_terminal_receipts r where r.operation_id=op.operation_id) and ($1::timestamptz is null or (registered_at,operation_id)>($1,$2::uuid)) order by registered_at,operation_id limit 16")
            .bind(time).bind(operation).fetch_all(&self.pool).await.map_err(unavailable)
    }
    /// Runtime must supply the actual validated deployed release and isolated
    /// Concert database with migration 0027. A signed client digest is not enough.
    pub fn from_product(
        pool: PgPool,
        release: &concert_application::bootstrap::ValidatedProductRelease,
    ) -> Result<Self, AvatarError> {
        let digest = release.source_digest();
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(AvatarError::Unavailable);
        }
        Ok(Self {
            pool,
            release_digest: digest.into(),
        })
    }
    #[cfg(test)]
    pub(super) fn for_fixture(pool: PgPool) -> Self {
        Self {
            pool,
            release_digest: "a".repeat(64),
        }
    }

    /// Restore only a registered immutable original operation. The old binding
    /// permits a bounded terminal query, never another read or mutation grant.
    pub(crate) async fn recovery_operation(
        &self,
        operation: Uuid,
    ) -> Result<RecoveryOperation, AvatarError> {
        let row=sqlx::query("select actor_id,event_id,session_family_id,parent_digest,body_digest,guard_binding_digest,owner_binding_proto,profile_epoch,profile_revision,profile_prepared_gid,profile_effect_digest from public.concert_participant_profile_operations where operation_id=$1")
            .bind(operation).fetch_optional(&self.pool).await.map_err(unavailable)?.ok_or(AvatarError::Missing)?;
        let bytes: Vec<u8> = row.try_get("owner_binding_proto").map_err(unavailable)?;
        let binding =
            lease::GuardBinding::decode(bytes.as_slice()).map_err(|_| AvatarError::InvalidState)?;
        let actual = wire_binding_digest(&binding).map_err(|_| AvatarError::InvalidState)?;
        let digest: Vec<u8> = row.try_get("guard_binding_digest").map_err(unavailable)?;
        let actor: Uuid = row.try_get("actor_id").map_err(unavailable)?;
        let event: Uuid = row.try_get("event_id").map_err(unavailable)?;
        let family: Uuid = row.try_get("session_family_id").map_err(unavailable)?;
        let parent: Vec<u8> = row.try_get("parent_digest").map_err(unavailable)?;
        let body: Vec<u8> = row.try_get("body_digest").map_err(unavailable)?;
        if digest.as_slice() != actual
            || binding.actor_id != actor.to_string()
            || binding.event_id != event.to_string()
            || binding.session_family_id != family.to_string()
            || binding.request_id != operation.to_string()
            || binding.parent_digest != parent
            || binding.body_digest != body
            || binding.operation != "updateConcertParticipant"
            || binding.http_method != "PATCH"
            || binding.http_target != format!("/api/concert/events/{event}/me")
        {
            return Err(AvatarError::InvalidState);
        }
        let version = CanonicalProfileVersion {
            epoch: row.try_get("profile_epoch").map_err(unavailable)?,
            revision: Revision::new(row.try_get("profile_revision").map_err(unavailable)?)
                .map_err(|_| AvatarError::InvalidState)?,
        };
        let effect: Vec<u8> = row.try_get("profile_effect_digest").map_err(unavailable)?;
        let gid: String = row.try_get("profile_prepared_gid").map_err(unavailable)?;
        if version.epoch.is_nil() || gid != format!("mcr_sp_{}", operation.simple()) {
            return Err(AvatarError::InvalidState);
        }
        Ok(RecoveryOperation {
            operation,
            actor,
            binding,
            expected_profile: ExpectedProfileEffect {
                version,
                gid,
                digest: effect.try_into().map_err(|_| AvatarError::InvalidState)?,
            },
        })
    }

    pub async fn begin(
        &self,
        request: &VerifiedParticipantRequest,
        guard_digest: [u8; 32],
        profile_version: CanonicalProfileVersion,
        authority: &dyn CoordinatorAuthority,
    ) -> Result<ParticipantProfileTransaction, AvatarError> {
        if request.release_digest() != self.release_digest {
            return Err(AvatarError::Unavailable);
        }
        authority.check(request).await?;
        let binding = Binding::capture(request, guard_digest)?;
        let update = ParticipantUpdate::parse(request.raw_body())?;
        update
            .expected_revision
            .next()
            .map_err(|_| AvatarError::Conflict)?;
        let expected_profile =
            ExpectedProfileEffect::for_request(request, guard_digest, profile_version)?;
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        authority.check(request).await?;
        let native = NativeTransaction::capture(&mut tx).await?;
        authority.check(request).await?;
        authority
            .track_concert_transaction(request, &native, expected_profile.digest)
            .await?;
        authority.check(request).await?;
        // This registry write commits independently BEFORE either protected
        // effect. The captured XID belongs to `tx`, never this pool connection.
        sqlx::query("insert into public.concert_participant_profile_operations(operation_id,actor_id,event_id,session_family_id,parent_digest,body_digest,guard_binding_digest,release_digest,expected_revision,concert_system_identifier,concert_database_oid,concert_full_xid,concert_backend_pid,concert_backend_started,profile_epoch,profile_revision,profile_prepared_gid,profile_effect_digest,owner_binding_proto) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19)")
            .bind(binding.operation).bind(binding.actor).bind(binding.event).bind(binding.family)
            .bind(binding.parent_digest.as_slice()).bind(binding.body_digest.as_slice()).bind(binding.guard_digest.as_slice()).bind(&binding.release)
            .bind(update.expected_revision.get()).bind(&native.system_identifier).bind(i64::from(native.database_oid)).bind(&native.full_xid)
            .bind(native.backend_pid).bind(native.backend_started).bind(profile_version.epoch).bind(profile_version.revision.get())
            .bind(&expected_profile.gid).bind(expected_profile.digest.as_slice()).bind(&binding.owner_binding_proto).execute(&self.pool).await.map_err(registry_error)?;
        authority.check(request).await?;
        Ok(ParticipantProfileTransaction {
            tx,
            pool: self.pool.clone(),
            binding,
            native,
            update,
            expected_profile,
            preferences: None,
        })
    }

    /// Call after a local snapshot and again after the fresh canonical Profile
    /// read. A later coordinated revision cannot be joined to an older snapshot.
    pub async fn ensure_visible(
        &self,
        request: &VerifiedParticipantRequest,
        snapshot_revision: Option<Revision>,
    ) -> Result<(), AvatarError> {
        request.check().map_err(|_| AvatarError::Unauthenticated)?;
        ensure_visible(&self.pool, request, snapshot_revision).await?;
        request.check().map_err(|_| AvatarError::Unauthenticated)
    }

    /// Recovery appends only an authenticated actual Profile terminal receipt,
    /// and only if Concert's independently resolved native decision agrees.
    pub(crate) async fn record_terminal(
        &self,
        receipt: &ProfileTerminalReceipt,
    ) -> Result<(), AvatarError> {
        let outcome = match receipt.outcome {
            ProfileTerminalOutcome::Committed => "COMMITTED",
            ProfileTerminalOutcome::Aborted => "ABORTED",
        };
        let native = &receipt.native;
        let changed=sqlx::query("insert into public.concert_participant_profile_terminal_receipts(operation_id,profile_prepared_gid,profile_effect_digest,outcome,profile_system_identifier,profile_database_oid,profile_full_xid,profile_backend_pid,profile_backend_started) select op.operation_id,op.profile_prepared_gid,op.profile_effect_digest,$4,$5,$6,$7,$8,$9 from public.concert_participant_profile_operations op where op.operation_id=$1 and op.profile_prepared_gid=$2 and op.profile_effect_digest=$3 and op.concert_system_identifier=(select system_identifier::text from pg_control_system()) and op.concert_database_oid=(select oid::bigint from pg_database where datname=current_database()) and (($4='COMMITTED' and op.decision='COMMIT' and pg_xact_status(op.concert_full_xid::xid8)='committed') or ($4='ABORTED' and op.decision='PENDING' and pg_xact_status(op.concert_full_xid::xid8)='aborted')) on conflict(operation_id) do nothing")
            .bind(receipt.operation).bind(&receipt.gid).bind(receipt.digest.as_slice()).bind(outcome).bind(&native.system_identifier).bind(i64::from(native.database_oid)).bind(&native.full_xid).bind(native.backend_pid).bind(native.backend_started)
            .execute(&self.pool).await.map_err(unavailable)?.rows_affected();
        if changed == 1 {
            return Ok(());
        }
        let matches:bool=sqlx::query_scalar("select exists(select 1 from public.concert_participant_profile_terminal_receipts where operation_id=$1 and profile_prepared_gid=$2 and profile_effect_digest=$3 and outcome=$4 and profile_system_identifier=$5 and profile_database_oid=$6 and profile_full_xid=$7 and profile_backend_pid=$8 and profile_backend_started=$9)")
            .bind(receipt.operation).bind(&receipt.gid).bind(receipt.digest.as_slice()).bind(outcome).bind(&native.system_identifier).bind(i64::from(native.database_oid)).bind(&native.full_xid).bind(native.backend_pid).bind(native.backend_started).fetch_one(&self.pool).await.map_err(unavailable)?;
        if matches {
            Ok(())
        } else {
            Err(AvatarError::Unavailable)
        }
    }
}
impl ParticipantProfileTransaction {
    pub fn expected_profile(&self) -> &ExpectedProfileEffect {
        &self.expected_profile
    }
    pub fn native_transaction(&self) -> &NativeTransaction {
        &self.native
    }
    async fn check(
        &self,
        request: &VerifiedParticipantRequest,
        authority: &dyn CoordinatorAuthority,
    ) -> Result<(), AvatarError> {
        self.binding.check(request)?;
        authority.check(request).await?;
        self.binding.check(request)
    }
    /// Resolve stale participant versions before asking Profile to prepare.
    /// Locks retain event -> admission -> participant order through the decision.
    pub async fn lock_participant(
        &mut self,
        request: &VerifiedParticipantRequest,
        authority: &dyn CoordinatorAuthority,
    ) -> Result<(), AvatarError> {
        self.check(request, authority).await?;
        let phase: Option<String> = sqlx::query_scalar(
            "select phase from public.concert_events where event_id=$1 for update",
        )
        .bind(self.binding.event)
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(unavailable)?;
        self.check(request, authority).await?;
        let phase: EventPhase = phase
            .ok_or(AvatarError::Missing)?
            .parse()
            .map_err(|_| AvatarError::InvalidState)?;
        if phase == EventPhase::Draft {
            return Err(AvatarError::Missing);
        }
        if !phase.permits_participant_writes() {
            return Err(AvatarError::ReadOnly);
        }
        let admission: Option<String> = sqlx::query_scalar("select state from public.concert_event_admissions where event_id=$1 and user_id=$2 for update")
            .bind(self.binding.event).bind(self.binding.actor).fetch_optional(&mut *self.tx).await.map_err(unavailable)?;
        self.check(request, authority).await?;
        if admission.as_deref() != Some("ACTIVE") {
            return Err(AvatarError::Denied);
        }
        let row = sqlx::query("select revision,theme,low_power,reduced_motion,sound_enabled from public.concert_participants where event_id=$1 and user_id=$2 for update")
            .bind(self.binding.event).bind(self.binding.actor).fetch_optional(&mut *self.tx).await.map_err(unavailable)?.ok_or(AvatarError::Missing)?;
        self.check(request, authority).await?;
        if row.try_get::<i64, _>("revision").map_err(unavailable)?
            != self.update.expected_revision.get()
        {
            return Err(AvatarError::Conflict);
        }
        self.preferences = Some(Preferences {
            theme: row
                .try_get::<String, _>("theme")
                .map_err(unavailable)?
                .parse()
                .map_err(|_| AvatarError::InvalidState)?,
            low_power: row.try_get("low_power").map_err(unavailable)?,
            reduced_motion: row.try_get("reduced_motion").map_err(unavailable)?,
            sound_enabled: row.try_get("sound_enabled").map_err(unavailable)?,
        });
        ensure_visible(&self.pool, request, None).await?;
        self.check(request, authority).await?;
        Ok(())
    }
    /// This independent durable append MUST precede dispatch of PrepareUpdate.
    /// A missing/unknown Profile reply keeps this operation gated for recovery.
    pub async fn mark_prepare_attempt(
        &self,
        request: &VerifiedParticipantRequest,
        authority: &dyn CoordinatorAuthority,
    ) -> Result<(), AvatarError> {
        self.check(request, authority).await?;
        if self.preferences.is_none() {
            return Err(AvatarError::Unavailable);
        }
        sqlx::query("insert into public.concert_participant_profile_prepare_attempts(operation_id) values($1) on conflict(operation_id) do nothing")
            .bind(self.binding.operation).execute(&self.pool).await.map_err(unavailable)?;
        self.check(request, authority).await
    }
    /// The live adapter must verify the actual prepared Profile receipt before
    /// invoking this method. This method itself never tells Profile to commit.
    pub(crate) async fn commit_after_prepared(
        mut self,
        request: &VerifiedParticipantRequest,
        authority: &dyn CoordinatorAuthority,
        prepared_gid: &str,
        prepared_effect: [u8; 32],
    ) -> Result<Revision, AvatarError> {
        self.check(request, authority).await?;
        if prepared_gid != self.expected_profile.gid
            || prepared_effect != self.expected_profile.digest
        {
            return Err(AvatarError::Denied);
        }
        let attempted: bool = sqlx::query_scalar("select exists(select 1 from public.concert_participant_profile_prepare_attempts where operation_id=$1)")
            .bind(self.binding.operation).fetch_one(&mut *self.tx).await.map_err(unavailable)?;
        self.check(request, authority).await?;
        if !attempted {
            return Err(AvatarError::Unavailable);
        }
        let current = self.preferences.ok_or(AvatarError::Unavailable)?;
        let preferences = match self.update.preferences {
            Field::Unchanged => current,
            Field::Set(value) => value,
        };
        let next = self
            .update
            .expected_revision
            .next()
            .map_err(|_| AvatarError::Conflict)?;
        let changed = sqlx::query("update public.concert_participants set revision=$3,theme=$4,low_power=$5,reduced_motion=$6,sound_enabled=$7 where event_id=$1 and user_id=$2 and revision=$8")
            .bind(self.binding.event).bind(self.binding.actor).bind(next.get()).bind(preferences.theme.as_str()).bind(preferences.low_power).bind(preferences.reduced_motion).bind(preferences.sound_enabled).bind(self.update.expected_revision.get())
            .execute(&mut *self.tx).await.map_err(unavailable)?.rows_affected();
        self.check(request, authority).await?;
        if changed != 1 {
            return Err(AvatarError::Conflict);
        }
        let changed = sqlx::query("update public.concert_participant_profile_operations set decision='COMMIT',committed_revision=$2,decided_at=clock_timestamp() where operation_id=$1 and decision='PENDING'")
            .bind(self.binding.operation).bind(next.get()).execute(&mut *self.tx).await.map_err(unavailable)?.rows_affected();
        self.check(request, authority).await?;
        if changed != 1 {
            return Err(AvatarError::Unavailable);
        }
        // Reconfirm the exact live connection, not a second pool connection.
        if NativeTransaction::capture(&mut self.tx).await? != self.native {
            return Err(AvatarError::Unavailable);
        }
        self.check(request, authority).await?;
        self.tx.commit().await.map_err(unavailable)?;
        // A failed return after commit must be reconciled via the durable journal,
        // never by issuing another mutation or inferring rollback from an error.
        authority.check(request).await?;
        Ok(next)
    }
}
async fn ensure_visible(
    pool: &PgPool,
    request: &VerifiedParticipantRequest,
    snapshot_revision: Option<Revision>,
) -> Result<(), AvatarError> {
    let blocked: bool = sqlx::query_scalar("select exists(select 1 from public.concert_participant_profile_operations op where op.event_id=$1 and op.actor_id=$2 and (((op.decision='COMMIT' or exists(select 1 from public.concert_participant_profile_prepare_attempts a where a.operation_id=op.operation_id)) and not exists(select 1 from public.concert_participant_profile_terminal_receipts r where r.operation_id=op.operation_id)) or ($3::bigint is not null and op.decision='COMMIT' and op.committed_revision>$3)))")
        .bind(request.key().event_id.uuid()).bind(request.key().user_id.uuid()).bind(snapshot_revision.map(Revision::get)).fetch_one(pool).await.map_err(unavailable)?;
    if blocked {
        Err(AvatarError::Unavailable)
    } else {
        Ok(())
    }
}
fn unavailable(_: sqlx::Error) -> AvatarError {
    AvatarError::Unavailable
}
fn registry_error(error: sqlx::Error) -> AvatarError {
    if error.as_database_error().and_then(|e| e.code()).as_deref() == Some("23505") {
        AvatarError::Conflict
    } else {
        AvatarError::Unavailable
    }
}
