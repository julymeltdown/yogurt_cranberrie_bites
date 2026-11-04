//! Real PostgreSQL transaction/codec tests. Remote owner proof is synthetic only.
mod meeting_photo_support;
use concert_application::{commands::*, media::Purpose};
use concert_service::meeting_photo::PhotoMutation;
use meeting_photo_support::*;
use sqlx::Row;
use std::time::Duration;
use uuid::Uuid;
#[tokio::test]
async fn attach_private_reads_exact_replay_and_current_policy() -> anyhow::Result<()> {
    let (p, f, s) = fixture().await?;
    let id = upload(&p, &f, f.owner, Purpose::Beacon).await?;
    let b = Uuid::new_v4();
    let key = Uuid::new_v4();
    let m = create(&f, Some(id));
    let receipt = s.execute(frame(&f, b, key, &m), m.clone()).await?;
    assert_eq!(receipt.state, ReceiptState::Succeeded);
    assert_eq!(s.execute(frame(&f, b, key, &m), m).await?, receipt);
    let got = s.read(read(&f, b, f.peer)).await?;
    assert_eq!(got.meeting.media_id, Some(id));
    let image = s.content(content(&f, b, id, f.peer), id).await?;
    assert!(!image.bytes.is_empty());
    assert_eq!(image.metadata.purpose, Purpose::Beacon);
    assert!(image
        .headers()
        .contains(&("cache-control", "private, no-store")));
    assert!(s
        .content(content(&f, b, id, Uuid::new_v4()), id)
        .await
        .is_err());
    sqlx::query("update meeting_fixture_authority set blocked=$2 where event=$1")
        .bind(f.event)
        .bind(vec![f.peer])
        .execute(&p)
        .await?;
    assert!(s.read(read(&f, b, f.peer)).await.is_err());
    assert!(s.content(content(&f, b, id, f.peer), id).await.is_err());
    sqlx::query(
        "update meeting_fixture_authority set blocked='{}',unavailable=true where event=$1",
    )
    .bind(f.event)
    .execute(&p)
    .await?;
    assert!(s.content(content(&f, b, id, f.owner), id).await.is_err());
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn replacement_removal_and_revoke_erase_bytes_atomically() -> anyhow::Result<()> {
    let (p, f, s) = fixture().await?;
    let old = upload(&p, &f, f.owner, Purpose::Beacon).await?;
    let new = upload(&p, &f, f.owner, Purpose::Beacon).await?;
    let b = Uuid::new_v4();
    let m = create(&f, Some(old));
    s.execute(frame(&f, b, Uuid::new_v4(), &m), m).await?;
    let m = update(&f, Some(new), 1);
    s.execute(frame(&f, b, Uuid::new_v4(), &m), m).await?;
    assert_eq!(state(&p, old).await?, ("DELETED".into(), None, true));
    assert!(s.content(content(&f, b, old, f.peer), old).await.is_err());
    // An old authorized HTTP target cannot be reused with a new current ID.
    assert!(s.content(content(&f, b, old, f.peer), new).await.is_err());
    assert_eq!(
        s.read(read(&f, b, f.peer)).await?.meeting.media_id,
        Some(new)
    );
    let m = update(&f, None, 2);
    s.execute(frame(&f, b, Uuid::new_v4(), &m), m).await?;
    assert_eq!(state(&p, new).await?, ("DELETED".into(), None, true));
    assert!(s
        .read(read(&f, b, f.peer))
        .await?
        .meeting
        .media_id
        .is_none());
    let last = upload(&p, &f, f.owner, Purpose::Beacon).await?;
    let m = update(&f, Some(last), 3);
    s.execute(frame(&f, b, Uuid::new_v4(), &m), m).await?;
    let m = PhotoMutation::Revoke {
        expected_revision: 4,
    };
    let key = Uuid::new_v4();
    let r = s.execute(frame(&f, b, key, &m), m.clone()).await?;
    assert_eq!(r, s.execute(frame(&f, b, key, &m), m).await?);
    assert_eq!(state(&p, last).await?, ("DELETED".into(), None, true));
    assert!(s.read(read(&f, b, f.owner)).await.is_err());
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn foreign_wrong_purpose_pending_and_stale_revision_cannot_attach() -> anyhow::Result<()> {
    let (p, f, s) = fixture().await?;
    let foreign = upload(&p, &f, f.peer, Purpose::Beacon).await?;
    let wrong = upload(&p, &f, f.owner, Purpose::Giveaway).await?;
    let pending = pending(&p, &f).await?;
    for id in [foreign, wrong, pending, Uuid::new_v4()] {
        let b = Uuid::new_v4();
        let m = create(&f, Some(id));
        assert!(s
            .execute(frame(&f, b, Uuid::new_v4(), &m), m)
            .await
            .is_err());
        let n: i64 =
            sqlx::query_scalar("select count(*) from concert_meeting_beacons where beacon_id=$1")
                .bind(b)
                .fetch_one(&p)
                .await?;
        assert_eq!(n, 0);
    }
    let id = upload(&p, &f, f.owner, Purpose::Beacon).await?;
    let b = Uuid::new_v4();
    let m = create(&f, Some(id));
    s.execute(frame(&f, b, Uuid::new_v4(), &m), m).await?;
    let one = update(&f, Some(id), 1);
    let two = update(&f, Some(id), 1);
    let (a, both) = tokio::join!(
        s.execute(frame(&f, b, Uuid::new_v4(), &one), one),
        s.execute(frame(&f, b, Uuid::new_v4(), &two), two)
    );
    let states = [a?, both?];
    assert_eq!(
        states
            .iter()
            .filter(|r| r.state == ReceiptState::Succeeded)
            .count(),
        1
    );
    assert_eq!(s.read(read(&f, b, f.peer)).await?.revision, 2);
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn failed_late_erasure_rolls_back_new_link_beacon_receipt_and_binding() -> anyhow::Result<()>
{
    let (p, f, s) = fixture().await?;
    let old = upload(&p, &f, f.owner, Purpose::Beacon).await?;
    let new = upload(&p, &f, f.owner, Purpose::Beacon).await?;
    let b = Uuid::new_v4();
    let m = create(&f, Some(old));
    s.execute(frame(&f, b, Uuid::new_v4(), &m), m).await?;
    sqlx::query("create or replace function meeting_photo_fixture_refuse_delete() returns trigger language plpgsql as $$ begin if NEW.state='DELETED' and OLD.media_id=current_setting('mcr.photo_fixture_refuse',true)::uuid then raise exception 'synthetic refusal'; end if; return NEW; end; $$").execute(&p).await?;
    // All service pool connections inherit this exact fixture setting through the DB role.
    sqlx::query(&format!(
        "alter role mcr_owner in database mcr_meeting set mcr.photo_fixture_refuse='{old}'"
    ))
    .execute(&p)
    .await?;
    sqlx::query("create trigger meeting_photo_fixture_delete before update on concert_media for each row execute function meeting_photo_fixture_refuse_delete()").execute(&p).await?;
    // Existing pool connections predate role defaults; a fresh owned pool captures them.
    let fresh = owned_pool().await?;
    let srv = service(&fresh);
    let key = Uuid::new_v4();
    let m = update(&f, Some(new), 1);
    assert!(srv.execute(frame(&f, b, key, &m), m).await.is_err());
    sqlx::query("drop trigger meeting_photo_fixture_delete on concert_media")
        .execute(&p)
        .await?;
    sqlx::query("alter role mcr_owner in database mcr_meeting reset mcr.photo_fixture_refuse")
        .execute(&p)
        .await?;
    assert_eq!(s.read(read(&f, b, f.peer)).await?.revision, 1);
    assert_eq!(state(&p, old).await?, ("READY".into(), Some(b), false));
    assert_eq!(state(&p, new).await?, ("READY".into(), None, false));
    let count: i64 = sqlx::query_scalar(
        "select count(*) from concert_command_receipts where idempotency_key=$1",
    )
    .bind(key)
    .fetch_one(&p)
    .await?;
    assert_eq!(count, 0);
    fresh.close().await;
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn actual_beacon_reader_waits_before_media_and_budget_locks() -> anyhow::Result<()> {
    let (p, f, s) = fixture().await?;
    let id = upload(&p, &f, f.owner, Purpose::Beacon).await?;
    let b = Uuid::new_v4();
    let m = create(&f, Some(id));
    s.execute(frame(&f, b, Uuid::new_v4(), &m), m).await?;
    let mut writer = p.begin().await?;
    sqlx::query("select beacon_id from concert_meeting_beacons where beacon_id=$1 for update")
        .bind(b)
        .execute(&mut *writer)
        .await?;
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *writer)
        .await?;
    let server = s.clone();
    let input = content(&f, b, id, f.peer);
    let pending = tokio::spawn(async move { server.content(input, id).await });
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "select exists(select 1 from pg_stat_activity where $1=any(pg_blocking_pids(pid)))",
            )
            .bind(pid)
            .fetch_one(&p)
            .await?;
            if waiting {
                break;
            }
            tokio::task::yield_now().await;
        }
        Ok::<(), sqlx::Error>(())
    })
    .await??;
    tokio::time::timeout(
        Duration::from_millis(500),
        sqlx::query("select media_id from concert_media where media_id=$1 for update")
            .bind(id)
            .execute(&mut *writer),
    )
    .await??;
    tokio::time::timeout(
        Duration::from_millis(500),
        sqlx::query("select singleton from concert_command_budget where singleton for update")
            .execute(&mut *writer),
    )
    .await??;
    writer.rollback().await?;
    assert!(!tokio::time::timeout(Duration::from_secs(2), pending)
        .await???
        .bytes
        .is_empty());
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn persisted_photo_and_receipt_survive_actual_postgres_restart() -> anyhow::Result<()> {
    let p = owned_pool().await?;
    sqlx::query("create table if not exists meeting_photo_restart(event uuid,owner uuid,peer uuid,beacon uuid,media uuid,key uuid,body jsonb,receipt uuid)").execute(&p).await?;
    if std::env::var("MEETING_RESTART_VERIFY").as_deref() == Ok("1") {
        let rows = sqlx::query("select * from meeting_photo_restart")
            .fetch_all(&p)
            .await?;
        assert!(!rows.is_empty());
        for r in rows {
            let mut f = setup(&p).await?;
            f.event = r.get("event");
            f.owner = r.get("owner");
            f.peer = r.get("peer");
            let b = r.get("beacon");
            let id = r.get("media");
            let create = serde_json::from_value(r.get("body"))?;
            let m = PhotoMutation::Create(create);
            let s = service(&p);
            let receipt = s.execute(frame(&f, b, r.get("key"), &m), m).await?;
            assert_eq!(receipt.id, r.get::<Uuid, _>("receipt"));
            assert_eq!(
                s.read(read(&f, b, f.peer)).await?.meeting.media_id,
                Some(id)
            );
            assert!(!s
                .content(content(&f, b, id, f.peer), id)
                .await?
                .bytes
                .is_empty());
        }
    } else {
        let f = setup(&p).await?;
        for u in [f.owner, f.peer] {
            sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(f.event).bind(u).execute(&p).await?;
        }
        let id = upload(&p, &f, f.owner, Purpose::Beacon).await?;
        let b = Uuid::new_v4();
        let key = Uuid::new_v4();
        let m = create(&f, Some(id));
        let PhotoMutation::Create(ref input) = m else {
            unreachable!()
        };
        let body = serde_json::to_value(input)?;
        let s = service(&p);
        let receipt = s.execute(frame(&f, b, key, &m), m).await?;
        sqlx::query("insert into meeting_photo_restart values($1,$2,$3,$4,$5,$6,$7,$8)")
            .bind(f.event)
            .bind(f.owner)
            .bind(f.peer)
            .bind(b)
            .bind(id)
            .bind(key)
            .bind(body)
            .bind(receipt.id)
            .execute(&p)
            .await?;
    }
    p.close().await;
    Ok(())
}

#[tokio::test]
async fn expiry_denies_content_then_bounded_cleanup_erases_and_is_repeatable() -> anyhow::Result<()>
{
    let (p, mut f, s) = fixture().await?;
    let id = upload(&p, &f, f.owner, Purpose::Beacon).await?;
    f.meeting.expires_at =
        sqlx::query_scalar("select clock_timestamp()+interval '300 milliseconds'")
            .fetch_one(&p)
            .await?;
    let b = Uuid::new_v4();
    let m = create(&f, Some(id));
    s.execute(frame(&f, b, Uuid::new_v4(), &m), m).await?;
    sqlx::query("select pg_sleep(0.35)").execute(&p).await?;
    assert!(s.content(content(&f, b, id, f.peer), id).await.is_err());
    assert!(s.read(read(&f, b, f.owner)).await.is_err());
    let m = PhotoMutation::Revoke {
        expected_revision: 1,
    };
    let input = frame(&f, b, Uuid::new_v4(), &m);
    let mut tx = p.begin().await?;
    assert_eq!(s.cleanup_expired_in(&mut tx, input.clone()).await?, 1);
    tx.commit().await?;
    assert_eq!(state(&p, id).await?, ("DELETED".into(), None, true));
    let mut tx = p.begin().await?;
    assert_eq!(s.cleanup_expired_in(&mut tx, input).await?, 0);
    tx.commit().await?;
    p.close().await;
    Ok(())
}

#[tokio::test]
async fn audience_generation_place_closure_and_removal_invalidate_photo_reads() -> anyhow::Result<()>
{
    let (p, mut f, s) = fixture().await?;
    f.meeting.audience = concert_domain::beacon::Audience::PartyMembers { party_id: f.party };
    let id = upload(&p, &f, f.owner, Purpose::Beacon).await?;
    let b = Uuid::new_v4();
    let m = create(&f, Some(id));
    s.execute(frame(&f, b, Uuid::new_v4(), &m), m).await?;
    assert!(!s
        .content(content(&f, b, id, f.peer), id)
        .await?
        .bytes
        .is_empty());
    sqlx::query("update meeting_fixture_authority set audience_revision=2 where event=$1")
        .bind(f.event)
        .execute(&p)
        .await?;
    assert!(s.content(content(&f, b, id, f.peer), id).await.is_err());
    // Explicit owner reconfirmation uses the new current party fence; old
    // ordinary reads above stayed denied until this acknowledged replacement.
    let m = update(&f, Some(id), 1);
    s.execute(frame(&f, b, Uuid::new_v4(), &m), m).await?;
    assert_eq!(s.read(read(&f, b, f.peer)).await?.revision, 2);
    sqlx::query("update concert_place_current_state set closure_state='CLOSED',revision=revision+1 where place_id=$1").bind(f.meeting.place_id).execute(&p).await?;
    assert!(s.content(content(&f, b, id, f.peer), id).await.is_err());
    sqlx::query("update meeting_fixture_authority set members=$2 where event=$1")
        .bind(f.event)
        .bind(vec![f.owner])
        .execute(&p)
        .await?;
    assert!(s.content(content(&f, b, id, f.peer), id).await.is_err());
    // Revoke still erases bytes after old place/audience Read becomes unavailable.
    let m = PhotoMutation::Revoke {
        expected_revision: 2,
    };
    s.execute(frame(&f, b, Uuid::new_v4(), &m), m).await?;
    assert_eq!(state(&p, id).await?, ("DELETED".into(), None, true));
    p.close().await;
    Ok(())
}
