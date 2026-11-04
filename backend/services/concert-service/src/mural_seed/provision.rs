use super::{SeedError, ValidatedPack};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub enum InitialState {
    Editable,
    Paused,
    Frozen,
}
impl InitialState {
    fn sql(self) -> &'static str {
        match self {
            Self::Editable => "EDITABLE",
            Self::Paused => "PAUSED",
            Self::Frozen => "FROZEN",
        }
    }
}
/// Supplied by the local owning-operator workflow after selecting a qualified
/// source revision. `operator_binding` identifies that real operator/run; it is
/// not an Auth role, browser assertion or a capability minted by this module.
/// The database credential/call site must remain operator-only. No HTTP route.
pub struct OperatorSelection {
    pub event_id: Uuid,
    pub source_revision: String,
    pub operator_binding: String,
    pub cooldown_ms: u32,
    pub initial_state: InitialState,
}
#[derive(Debug, serde::Serialize)]
pub struct ProvisionedSeed {
    pub event_id: Uuid,
    pub snapshot_id: Uuid,
    pub revision: i64,
    pub manifest_sha256: [u8; 32],
    pub input_digest: [u8; 32],
    pub provisioned_at: DateTime<Utc>,
    pub exact_pristine_retry: bool,
}
fn input_digest(pack: &ValidatedPack, selected: &OperatorSelection) -> Result<[u8; 32], SeedError> {
    if selected.event_id.is_nil()
        || selected.cooldown_ms > 3_600_000
        || selected.source_revision.len() != 40
        || !selected
            .source_revision
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || selected.operator_binding.trim().is_empty()
        || selected.operator_binding.len() > 512
        || selected.operator_binding.chars().any(char::is_control)
    {
        return Err(SeedError::InvalidSelection);
    }
    let bytes=serde_json::to_vec(&serde_json::json!({
        "format":"mcr-mural-operator-seed/1", "event_id":selected.event_id,
        "source_revision":selected.source_revision,"operator_binding":selected.operator_binding,
        "manifest_sha256":pack.manifest_sha256,"qualification_sha256":pack.qualification_sha256,
        "background_index":0,"cooldown_ms":selected.cooldown_ms,"initial_state":selected.initial_state.sql()
    })).map_err(|_|SeedError::InvalidSelection)?;
    Ok(Sha256::digest(bytes).into())
}
const HISTORY: &str =
    "select exists(select 1 from public.concert_mural_operations where event_id=$1)
 or exists(select 1 from public.concert_mural_snapshots where event_id=$1)
 or exists(select 1 from public.concert_mural_limits where event_id=$1)
 or exists(select 1 from public.concert_mural_locks where event_id=$1)
 or exists(select 1 from public.concert_mural_tool_actions where event_id=$1)
 or exists(select 1 from public.concert_mural_tool_correlations where event_id=$1)
 or exists(select 1 from public.concert_mural_tool_charges where event_id=$1)";

/// Inserts state, explicit background role and full private provenance in one
/// transaction. Locks canonical event then mural root. Never updates a mural,
/// clears history, makes a checkpoint, fabricates admission or creates an event.
/// An existing mural can only return its exact untouched original seed, selected
/// by the identical operator/source/input digest and complete row comparison.
pub async fn provision(
    pool: &PgPool,
    pack: &ValidatedPack,
    selected: &OperatorSelection,
) -> Result<ProvisionedSeed, SeedError> {
    let digest = input_digest(pack, selected)?;
    let mut tx = pool.begin().await?;
    sqlx::query("set local lock_timeout='3s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("set local statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    let event: Option<Uuid> = sqlx::query_scalar(
        "select event_id from public.concert_events where event_id=$1 for update",
    )
    .bind(selected.event_id)
    .fetch_optional(&mut *tx)
    .await?;
    if event.is_none() {
        return Err(SeedError::EventUnavailable);
    }
    let row=sqlx::query("select revision,snapshot_id,state,pixels,palette,cooldown_ms,retained_from_revision,reset_revision,created_at,snapshot_created_at from public.concert_mural_state where event_id=$1 for update")
        .bind(selected.event_id).fetch_optional(&mut *tx).await?;
    let history: bool = sqlx::query_scalar(HISTORY)
        .bind(selected.event_id)
        .fetch_one(&mut *tx)
        .await?;
    if history {
        return Err(SeedError::NotPristine);
    }
    if let Some(row) = row {
        let provenance=sqlx::query("select initial_snapshot_id,manifest_sha256,input_digest,source_revision,operator_binding,initial_created_at,initial_snapshot_created_at,provisioned_at from public.concert_mural_seed_provenance where event_id=$1 for share")
            .bind(selected.event_id).fetch_optional(&mut *tx).await?.ok_or(SeedError::NotPristine)?;
        let background:Option<i16>=sqlx::query_scalar("select background_index from public.concert_mural_tool_config where event_id=$1 for share")
            .bind(selected.event_id).fetch_optional(&mut *tx).await?;
        if row.get::<i64, _>("revision") != 1
            || row.get::<i64, _>("retained_from_revision") != 1
            || row.get::<i64, _>("reset_revision") != 1
            || background != Some(0)
            || row.get::<Uuid, _>("snapshot_id") != provenance.get::<Uuid, _>("initial_snapshot_id")
            || row.get::<String, _>("state") != selected.initial_state.sql()
            || row.get::<Vec<u8>, _>("pixels") != pack.pixels
            || row.get::<Vec<String>, _>("palette") != pack.palette
            || row.get::<i64, _>("cooldown_ms") != i64::from(selected.cooldown_ms)
            || row.get::<DateTime<Utc>, _>("created_at")
                != provenance.get::<DateTime<Utc>, _>("initial_created_at")
            || row.get::<DateTime<Utc>, _>("snapshot_created_at")
                != provenance.get::<DateTime<Utc>, _>("initial_snapshot_created_at")
            || provenance.get::<Vec<u8>, _>("manifest_sha256") != pack.manifest_sha256
            || provenance.get::<Vec<u8>, _>("input_digest") != digest
            || provenance.get::<String, _>("source_revision") != selected.source_revision
            || provenance.get::<String, _>("operator_binding") != selected.operator_binding
        {
            return Err(SeedError::NotPristine);
        }
        let result = ProvisionedSeed {
            event_id: selected.event_id,
            snapshot_id: row.get("snapshot_id"),
            revision: 1,
            manifest_sha256: pack.manifest_sha256,
            input_digest: digest,
            provisioned_at: provenance.get("provisioned_at"),
            exact_pristine_retry: true,
        };
        tx.commit().await?;
        return Ok(result);
    }
    // History FKs and event serialization close insertion races. Any competing
    // external producer's unique conflict rolls back this entire transaction.
    let snapshot = Uuid::new_v4();
    let inserted=sqlx::query("insert into public.concert_mural_state(event_id,revision,snapshot_id,state,pixels,palette,cooldown_ms,retained_from_revision,reset_revision) values($1,1,$2,$3,$4,$5,$6,1,1) returning created_at,snapshot_created_at")
        .bind(selected.event_id).bind(snapshot).bind(selected.initial_state.sql()).bind(&pack.pixels).bind(&pack.palette)
        .bind(i64::from(selected.cooldown_ms)).fetch_one(&mut *tx).await?;
    sqlx::query(
        "insert into public.concert_mural_tool_config(event_id,background_index) values($1,0)",
    )
    .bind(selected.event_id)
    .execute(&mut *tx)
    .await?;
    let provisioned_at:DateTime<Utc>=sqlx::query_scalar("insert into public.concert_mural_seed_provenance(event_id,initial_snapshot_id,manifest_sha256,input_digest,source_revision,operator_binding,initial_created_at,initial_snapshot_created_at) values($1,$2,$3,$4,$5,$6,$7,$8) returning provisioned_at")
        .bind(selected.event_id).bind(snapshot).bind(pack.manifest_sha256.as_slice()).bind(digest.as_slice())
        .bind(&selected.source_revision).bind(&selected.operator_binding)
        .bind(inserted.get::<DateTime<Utc>,_>("created_at")).bind(inserted.get::<DateTime<Utc>,_>("snapshot_created_at"))
        .fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(ProvisionedSeed {
        event_id: selected.event_id,
        snapshot_id: snapshot,
        revision: 1,
        manifest_sha256: pack.manifest_sha256,
        input_digest: digest,
        provisioned_at,
        exact_pristine_retry: false,
    })
}
