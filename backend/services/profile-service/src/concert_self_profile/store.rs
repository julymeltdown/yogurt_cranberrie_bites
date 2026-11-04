//! Owning SQL only. These private helpers grant no actor or mutation authority;
//! the transport must hold and register the concrete canonical owner scope first.
use super::{
    identity::EffectConnection,
    model::{BioPatch, Content, Error, Patch, Version},
};
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

pub(super) async fn read_in(
    effect: &mut EffectConnection,
    actor: Uuid,
    for_update: bool,
) -> Result<(Content, DateTime<Utc>), Error> {
    effect.check()?;
    if actor.is_nil() {
        return Err(Error::Invalid);
    }
    let suffix = if for_update {
        "for update of p"
    } else {
        "for share of p"
    };
    // Both query strings are source constants. No caller text becomes SQL.
    let query = format!("select p.user_id,p.nickname,p.bio,p.concert_self_epoch,p.concert_self_revision,clock_timestamp() as observed_at from profiles p where p.user_id=$1 and p.deleted_at is null and octet_length(p.nickname)<=160 and char_length(p.nickname)<=40 and octet_length(p.bio)<=640 and char_length(p.bio)<=160 and not exists(select 1 from profile_user_deletion_tombstones d where d.user_id=p.user_id) {suffix}");
    let row = sqlx::query(&query)
        .bind(actor)
        .fetch_optional(effect.connection())
        .await
        .map_err(unavailable)?
        .ok_or(Error::Missing)?;
    effect.check()?;
    let content = decode(&row)?;
    Ok((content, row.try_get("observed_at").map_err(unavailable)?))
}
fn decode(row: &sqlx::postgres::PgRow) -> Result<Content, Error> {
    let version = Version::new(
        row.try_get("concert_self_epoch").map_err(unavailable)?,
        row.try_get("concert_self_revision").map_err(unavailable)?,
    )?;
    let bio: String = row.try_get("bio").map_err(unavailable)?;
    Content::new(
        row.try_get("user_id").map_err(unavailable)?,
        row.try_get("nickname").map_err(unavailable)?,
        Some(bio),
        version,
    )
    .map_err(|_| Error::Missing)
}
pub(super) async fn update_in(
    effect: &mut EffectConnection,
    actor: Uuid,
    expected: Version,
    patch: &Patch,
) -> Result<Content, Error> {
    effect.check()?;
    // The same subject fence used by the retained deletion saga. No insert or
    // upsert is allowed; missing/deleted content is never fabricated.
    sqlx::query("select profile_assert_subjects_not_deleted(array[$1]::uuid[])")
        .bind(actor)
        .execute(effect.connection())
        .await
        .map_err(unavailable)?;
    let (current, _) = read_in(effect, actor, true).await?;
    if current.version() != expected {
        return Err(Error::Conflict);
    }
    let (keep_bio, bio) = match patch.bio() {
        BioPatch::Keep => (true, ""),
        BioPatch::Clear => (false, ""),
        BioPatch::Set(value) => (false, value.as_str()),
    };
    let row=sqlx::query("update profiles set nickname=coalesce($2::text,nickname),bio=case when $3 then bio else $4 end,updated_at=clock_timestamp() where user_id=$1 and deleted_at is null and concert_self_epoch=$5 and concert_self_revision=$6 returning user_id,nickname,bio,concert_self_epoch,concert_self_revision")
        .bind(actor).bind(patch.display_name()).bind(keep_bio).bind(bio).bind(expected.epoch()).bind(expected.revision())
        .fetch_optional(effect.connection()).await.map_err(unavailable)?.ok_or(Error::Conflict)?;
    effect.check()?;
    decode(&row)
}
pub(super) async fn receipt_in(
    effect: &mut EffectConnection,
    operation: Uuid,
    actor: Uuid,
    event: Uuid,
    command_digest: [u8; 32],
    content: &Content,
) -> Result<(), Error> {
    effect.check()?;
    let effect_digest = effect.identity().effect_digest;
    let receipt=sqlx::query("insert into concert_self_profile_receipts(operation_id,effect_binding_digest,command_digest,profile_epoch,profile_revision) select o.operation_id,$2,$3,$4,$5 from concert_self_profile_operations o join profiles p on p.user_id=o.actor_id where o.operation_id=$1 and o.actor_id=$6 and o.event_id=$7 and o.command_digest=$3 and o.effect_binding_digest=$2 and o.full_xid=pg_current_xact_id()::text and o.system_identifier=(select system_identifier::text from pg_control_system()) and o.database_oid=(select oid::bigint from pg_database where datname=current_database()) and o.backend_pid=pg_backend_pid() and o.backend_started_at=(select backend_start from pg_stat_activity where pid=pg_backend_pid()) and o.producer_instance_id=$8 and p.deleted_at is null and p.concert_self_epoch=$4 and p.concert_self_revision=$5")
        .bind(operation).bind(effect_digest.as_slice()).bind(command_digest.as_slice()).bind(content.version().epoch()).bind(content.version().revision())
        .bind(actor).bind(event).bind(effect.identity().producer_instance)
        .execute(effect.connection()).await.map_err(unavailable)?;
    if receipt.rows_affected() != 1 {
        return Err(Error::Conflict);
    }
    // The success audit and content/receipt are all in this exact prepared XID.
    // It contains identifiers and revisions, never Profile text or credentials.
    sqlx::query("insert into profile_audit_events(id,actor_id,profile_user_id,action,outcome,request_id,metadata_json) values($1,$2,$2,'CONCERT_SELF_PROFILE_UPDATED','SUCCESS',$3,$4)")
        .bind(Uuid::new_v4()).bind(actor).bind(operation.to_string())
        .bind(serde_json::json!({"operation_id":operation,"event_id":event,"profile_revision":content.version().revision()}).to_string())
        .execute(effect.connection()).await.map_err(unavailable)?;
    effect.check()
}
pub(super) async fn exact_receipt(
    connection: &mut PgConnection,
    operation: Uuid,
    effect_digest: &[u8],
    command_digest: &[u8],
) -> Result<Option<Version>, Error> {
    let row=sqlx::query("select profile_epoch,profile_revision,effect_binding_digest,command_digest from concert_self_profile_receipts where operation_id=$1").bind(operation).fetch_optional(connection).await.map_err(unavailable)?;
    row.map(|r| {
        let recorded_effect: Vec<u8> = r.try_get("effect_binding_digest").map_err(unavailable)?;
        let recorded_command: Vec<u8> = r.try_get("command_digest").map_err(unavailable)?;
        if recorded_effect != effect_digest || recorded_command != command_digest {
            return Err(Error::Conflict);
        }
        Version::new(
            r.try_get("profile_epoch").map_err(unavailable)?,
            r.try_get("profile_revision").map_err(unavailable)?,
        )
    })
    .transpose()
}
fn unavailable(_: sqlx::Error) -> Error {
    Error::Unavailable
}
