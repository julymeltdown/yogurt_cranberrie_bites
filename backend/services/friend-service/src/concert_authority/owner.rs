use super::proto;
use sqlx::{PgConnection, Row};
use tonic::Status;
use uuid::Uuid;

pub(super) const WORKLOAD: platform::grpc_auth::Workload = platform::grpc_auth::Workload::Friend;
pub(super) const GID_PREFIX: &str = "mcr_oa_f_";
pub(super) const REVISION_LOCK: &str = "select owner_epoch,revision from public.friend_concert_relationship_revision where singleton for share";
pub(super) async fn facts(
    connection: &mut PgConnection,
    binding: &proto::GuardBinding,
    subjects: &[Uuid],
) -> Result<proto::CheckResponse, Status> {
    let actor = super::model::uuid(&binding.actor_id)?;
    let row=sqlx::query("select owner_epoch,revision,not exists(select 1 from public.friend_user_deletion_tombstones where user_id=$1) active from public.friend_concert_relationship_revision where singleton")
        .bind(actor).fetch_one(&mut *connection).await.map_err(super::unavailable)?;
    if !row
        .try_get::<bool, _>("active")
        .map_err(super::unavailable)?
    {
        return Err(Status::permission_denied("Current owner authority denied"));
    }
    let rows=sqlx::query("select s.user_id,not exists(select 1 from public.friend_user_deletion_tombstones where user_id=s.user_id) active,exists(select 1 from public.user_blocks where blocker_id=$1 and blocked_id=s.user_id and deleted_at is null) actor_blocks_subject,exists(select 1 from public.user_blocks where blocker_id=s.user_id and blocked_id=$1 and deleted_at is null) subject_blocks_actor from unnest($2::uuid[]) s(user_id) order by s.user_id")
        .bind(actor).bind(subjects).fetch_all(&mut *connection).await.map_err(super::unavailable)?;
    let states = rows
        .into_iter()
        .map(|r| {
            Ok(proto::SubjectState {
                user_id: r
                    .try_get::<Uuid, _>("user_id")
                    .map_err(super::unavailable)?
                    .to_string(),
                active: r.try_get("active").map_err(super::unavailable)?,
                actor_blocks_subject: r
                    .try_get("actor_blocks_subject")
                    .map_err(super::unavailable)?,
                subject_blocks_actor: r
                    .try_get("subject_blocks_actor")
                    .map_err(super::unavailable)?,
                security_revision: 0,
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
