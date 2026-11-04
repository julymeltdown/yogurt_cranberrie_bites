//! Actual local SQL exclusions/recovery with synthetic Root metadata. These
//! checks cannot certify durable Root intent, HTTP EOF or canonical owner scope.
use super::*;
#[path = "../social_remaining_support/owned.rs"]
mod owned;

async fn recovery_in(c: &mut PgConnection, query: ChildQuery) -> SocialReadRecovery {
    let row = sqlx::query("select (pg_control_system()).system_identifier::text system_identifier,(select oid::bigint from pg_database where datname=current_database()) database_oid,pg_current_xact_id()::text full_xid,pg_backend_pid() backend_pid,(select (extract(epoch from backend_start)*1000000)::bigint from pg_stat_activity where pid=pg_backend_pid()) backend_started")
        .fetch_one(c).await.unwrap();
    let until = query.expires_unix_micros;
    SocialReadRecovery::SocialRead {
        query,
        child_nonce: Uuid::new_v4(),
        native: ChildNativeIdentity {
            system_identifier: row.get("system_identifier"),
            database_oid: u32::try_from(row.get::<i64, _>("database_oid")).unwrap(),
            full_xid: row.get("full_xid"),
            backend_pid: row.get("backend_pid"),
            backend_started_unix_micros: row.get("backend_started"),
            producer_instance_id: Uuid::new_v4(),
        },
        valid_until_unix_micros: until,
    }
}
fn query(actor: Uuid, event: Uuid) -> ChildQuery {
    ChildQuery {
        actor,
        event,
        subject: actor,
        family: Uuid::new_v4(),
        request: Uuid::new_v4(),
        expires_unix_micros: (Utc::now() + chrono::Duration::minutes(1)).timestamp_micros(),
        binding_digest: [41; 32],
    }
}
async fn denied_lock(pool: &PgPool, sql: &str, event: Uuid, subject: Uuid) {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("set local lock_timeout='100ms'")
        .execute(&mut *tx)
        .await
        .unwrap();
    let error = sqlx::query(sql)
        .bind(event)
        .bind(subject)
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("55P03")
    );
    tx.rollback().await.unwrap();
}
#[tokio::test]
#[ignore = "owned social_live native runner required"]
async fn prepared_read_sources_survive_projector_commit_and_exclude_real_local_writes() {
    let pool = owned::pool().await;
    let event = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let peer = Uuid::new_v4();
    let intro = Uuid::new_v4();
    let request = Uuid::new_v4();
    sqlx::query(
        "insert into concert_events(event_id,slug,title,phase) values($1,$2,'Read fence','LIVE')",
    )
    .bind(event)
    .bind(event.to_string())
    .execute(&pool)
    .await
    .unwrap();
    let mut seed = pool.begin().await.unwrap();
    sqlx::query("select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '1 minute')::text,true)").execute(&mut *seed).await.unwrap();
    for user in [actor, peer] {
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,true)").bind(event).bind(user).execute(&mut *seed).await.unwrap();
        sqlx::query("insert into concert_age_owners(user_id) values($1)")
            .bind(user)
            .execute(&mut *seed)
            .await
            .unwrap();
        sqlx::query("insert into concert_social_handles(event_id,user_id,platform,key_id,nonce,ciphertext) values($1,$2,'INSTAGRAM','fixture',decode(repeat('00',12),'hex'),decode(repeat('00',17),'hex'))").bind(event).bind(user).execute(&mut *seed).await.unwrap();
    }
    sqlx::query("insert into concert_intros(event_id,intro_id,owner_id,revision,state,text,created_at,updated_at) values($1,$2,$3,1,'PUBLISHED','Read fence',clock_timestamp(),clock_timestamp())").bind(event).bind(intro).bind(peer).execute(&mut *seed).await.unwrap();
    sqlx::query("insert into concert_social_requests(event_id,request_id,requester_id,recipient_id,intro_id,intro_revision,authority_revision,relationship_revision,state,expires_at) values($1,$2,$3,$4,$5,1,1,1,'PENDING',clock_timestamp()+interval '1 hour')").bind(event).bind(request).bind(actor).bind(peer).bind(intro).execute(&mut *seed).await.unwrap();
    sqlx::query("update concert_social_requests set state='APPROVED',revision=revision+1 where event_id=$1 and request_id=$2").bind(event).bind(request).execute(&mut *seed).await.unwrap();
    sqlx::query("insert into concert_social_bindings(event_id,request_id,platform,requester_version,recipient_version,requester_consents,approved) values($1,$2,'INSTAGRAM',1,1,true,true)").bind(event).bind(request).execute(&mut *seed).await.unwrap();
    for (owner, viewer) in [(actor, peer), (peer, actor)] {
        sqlx::query("insert into concert_social_grants(event_id,request_id,owner_id,viewer_id,platform,handle_version,peer_handle_version,state,expires_at) select $1,$2,$3,$4,'INSTAGRAM',1,1,'ACTIVE',expires_at from concert_social_requests where event_id=$1 and request_id=$2").bind(event).bind(request).bind(owner).bind(viewer).execute(&mut *seed).await.unwrap();
    }
    seed.commit().await.unwrap();
    let mut projecting = pool.begin().await.unwrap();
    sqlx::query("select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '1 minute')::text,true)").execute(&mut *projecting).await.unwrap();
    sqlx::query("select event_id from concert_events where event_id=$1 for share")
        .bind(event)
        .execute(&mut *projecting)
        .await
        .unwrap();
    sqlx::query("select request_id from concert_social_requests where event_id=$1 and request_id=$2 for update").bind(event).bind(request).execute(&mut *projecting).await.unwrap();
    sqlx::query("insert into concert_relationship_pairs(event_id,low_user,high_user) values($1,least($2,$3),greatest($2,$3))").bind(event).bind(actor).bind(peer).execute(&mut *projecting).await.unwrap();
    sqlx::query("select revision from concert_relationship_pairs where event_id=$1 for update")
        .bind(event)
        .execute(&mut *projecting)
        .await
        .unwrap();
    let q = query(actor, event);
    let mut child = pool.begin().await.unwrap();
    sqlx::query("set local statement_timeout='2s'")
        .execute(&mut *child)
        .await
        .unwrap();
    lock_sources(
        &mut child,
        &q,
        SocialReadSources::Reveal {
            request,
            platform: Platform::Instagram,
        },
    )
    .await
    .unwrap();
    capacity(&mut child).await.unwrap();
    let recovery = recovery_in(&mut child, q).await;
    let gid = recovery.gid().unwrap();
    // Explicit synthetic intent: exercise SQL lifetime only, never pretend this
    // direct test PREPARE was admitted into Root's durable recovery ledger.
    sqlx::query(&format!("prepare transaction '{gid}'"))
        .execute(&mut *child)
        .await
        .unwrap();
    child.commit().await.unwrap();
    projecting.commit().await.unwrap();
    denied_lock(
        &pool,
        "update concert_events set revision=revision+1 where event_id=$1 and $2::uuid is not null",
        event,
        actor,
    )
    .await;
    denied_lock(
        &pool,
        "update concert_age_owners set user_id=user_id where user_id=$2 and $1::uuid is not null",
        event,
        actor,
    )
    .await;
    denied_lock(
        &pool,
        "delete from concert_participants where event_id=$1 and user_id=$2",
        event,
        peer,
    )
    .await;
    denied_lock(
        &pool,
        "update concert_social_handles set revision=revision+1 where event_id=$1 and user_id=$2",
        event,
        actor,
    )
    .await;
    denied_lock(
        &pool,
        "update concert_intros set revision=revision+1 where event_id=$1 and owner_id=$2",
        event,
        peer,
    )
    .await;
    denied_lock(
        &pool,
        "update concert_social_grants set revision=revision+1 where event_id=$1 and owner_id=$2",
        event,
        actor,
    )
    .await;
    let mut wrong = recovery.clone();
    let SocialReadRecovery::SocialRead { native, .. } = &mut wrong;
    native.database_oid += 1;
    assert!(recover_closed(&pool, &wrong).await.is_err());
    let count: i64 = sqlx::query_scalar("select count(*) from pg_prepared_xacts where gid=$1")
        .bind(&gid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    recover_closed(&pool, &recovery).await.unwrap();
    recover_closed(&pool, &recovery).await.unwrap();
    sqlx::query("update concert_events set revision=revision+1 where event_id=$1")
        .bind(event)
        .execute(&pool)
        .await
        .unwrap();
    let state: String = sqlx::query_scalar("select pg_xact_status($1::text::xid8)")
        .bind(&recovery.parts().2.full_xid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "aborted");
    pool.close().await;
}
#[tokio::test]
#[ignore = "owned social_live native runner required"]
async fn missing_prepared_gid_does_not_prove_source_exclusion_aborted() {
    let pool = owned::pool().await;
    let mut tx = pool.begin().await.unwrap();
    let recovery = recovery_in(&mut tx, query(Uuid::new_v4(), Uuid::new_v4())).await;
    assert!(recover_closed(&pool, &recovery).await.is_err());
    tx.commit().await.unwrap();
    assert!(recover_closed(&pool, &recovery).await.is_err());
    let mut tx = pool.begin().await.unwrap();
    let recovery = recovery_in(&mut tx, query(Uuid::new_v4(), Uuid::new_v4())).await;
    tx.rollback().await.unwrap();
    recover_closed(&pool, &recovery).await.unwrap();
    pool.close().await;
}
