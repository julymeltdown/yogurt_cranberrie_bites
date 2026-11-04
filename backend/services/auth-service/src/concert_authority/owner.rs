use super::proto;
use sqlx::{PgConnection, Row};
use tonic::Status;
use uuid::Uuid;

pub(super) const WORKLOAD: platform::grpc_auth::Workload = platform::grpc_auth::Workload::Auth;
pub(super) const GID_PREFIX: &str = "mcr_oa_a_";
pub(super) const REVISION_LOCK: &str = "select owner_epoch,revision from public.auth_concert_relationship_revision where singleton for share";

/// The already-prepared source exclusion is held; never take a second source
/// SHARE lock behind a waiting mutation during Check/Enroll.
pub(super) async fn facts(
    connection: &mut PgConnection,
    binding: &proto::GuardBinding,
    subjects: &[Uuid],
) -> Result<proto::CheckResponse, Status> {
    let actor = super::model::uuid(&binding.actor_id)?;
    let family = super::model::uuid(&binding.session_family_id)?;
    let row = sqlx::query("select r.owner_epoch,r.revision,exists(select 1 from public.users where id=$1 and status='ACTIVE' and deleted_at is null) active,(select max(expires_at) from public.refresh_tokens where user_id=$1 and family_id=$2 and revoked_at is null and expires_at>clock_timestamp()) session_expires from public.auth_concert_relationship_revision r where singleton")
        .bind(actor).bind(family).fetch_one(&mut *connection).await.map_err(super::unavailable)?;
    let expiry: Option<chrono::DateTime<chrono::Utc>> =
        row.try_get("session_expires").map_err(super::unavailable)?;
    if !row
        .try_get::<bool, _>("active")
        .map_err(super::unavailable)?
        || expiry.map_or(true, |v| v.timestamp_micros() < binding.expires_unix_micros)
    {
        return Err(Status::permission_denied("Current owner authority denied"));
    }
    let rows=sqlx::query("select ids.user_id,s.generation,exists(select 1 from public.users u where u.id=ids.user_id and u.status='ACTIVE' and u.deleted_at is null) active from unnest($1::uuid[]) ids(user_id) left join public.auth_concert_user_security_generation s using(user_id) order by ids.user_id")
        .bind(subjects).fetch_all(&mut *connection).await.map_err(super::unavailable)?;
    if rows.len() != subjects.len() {
        return Err(Status::permission_denied(
            "Subject owner authority unavailable",
        ));
    }
    let states = rows
        .into_iter()
        .map(|r| {
            let active: bool = r.try_get("active").map_err(super::unavailable)?;
            let generation: Option<i64> = r.try_get("generation").map_err(super::unavailable)?;
            if active && generation.is_none() {
                return Err(super::model::unavailable());
            }
            Ok(proto::SubjectState {
                user_id: r
                    .try_get::<Uuid, _>("user_id")
                    .map_err(super::unavailable)?
                    .to_string(),
                active: r.try_get("active").map_err(super::unavailable)?,
                actor_blocks_subject: false,
                subject_blocks_actor: false,
                security_revision: generation.unwrap_or(0),
            })
        })
        .collect::<Result<Vec<_>, Status>>()?;
    Ok(proto::CheckResponse {
        revision: Some(proto::OwnerRevision {
            epoch: row
                .try_get::<Uuid, _>("owner_epoch")
                .map_err(super::unavailable)?
                .to_string(),
            revision: row.try_get("revision").map_err(super::unavailable)?,
        }),
        subjects: states,
        session_live: true,
    })
}
