//! Real PgGifts races; seed/code/expiry patterns adapted from
//! community_giveaway.rs at 93ef0d8 (Gift owner confirms byte-identical5329d3f4).
use super::support::*;
use chrono::{Duration, Utc};
use concert_application::{commands::*, community::giveaway::*};
use concert_domain::community::giveaway::*;
use concert_service::community::giveaway::*;
use sqlx::{PgPool, Row};
use uuid::Uuid;
fn codes() -> PickupKeys {
    PickupKeys::new(PickupKey::new("qa03-pickup", &[73; 32]).unwrap(), vec![]).unwrap()
}
fn command(actor: Uuid, event: Uuid, giveaway: Uuid, action: GiftAction) -> GiftCommand {
    GiftCommand {
        actor,
        event,
        giveaway,
        action,
        key: Uuid::new_v4(),
        code: None,
    }
}
async fn execute(pool: &PgPool, c: &GiftCommand) -> Result<GiftResult, GiftServiceError> {
    PgGifts::new(pool.clone(), IsolatedAuthority)
        .execute(
            c,
            &keys(),
            &codes(),
            Some(&TrustedPickupContext::from_trusted_keyed_ip_bucket(
                [19; 32],
            )),
        )
        .await
}
async fn fixture(pool: &PgPool, total: i32) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
    let (event, owner, place) = event(pool).await?;
    let fields = GiftFields {
        title: "QA03 stock".into(),
        description: String::new(),
        image_media_id: None,
        quantity_total: total,
        pickup_starts_at: Utc::now() - Duration::minutes(1),
        pickup_ends_at: future(),
        public_place_id: place,
    };
    let create = command(
        owner,
        event,
        Uuid::nil(),
        GiftAction::Create(fields.clone()),
    );
    let id = create.giveaway_id();
    execute(pool, &create)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    execute(
        pool,
        &command(
            owner,
            event,
            id,
            GiftAction::SetMeeting {
                expected: 1,
                meeting: PickupMeeting {
                    place_id: place,
                    map_revision: 1,
                    place_revision: 1,
                    instructions: "Synthetic QA03 fixture".into(),
                    identification: "Not venue guidance".into(),
                    expires_at: fields.pickup_ends_at + Duration::seconds(60),
                },
                physical_acknowledged: true,
            },
        ),
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    execute(
        pool,
        &command(
            owner,
            event,
            id,
            GiftAction::Update {
                expected: 2,
                fields,
                state: GiftState::Open,
            },
        ),
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    Ok((event, owner, id))
}
async fn root(pool: &PgPool, event: Uuid, gift: Uuid) -> anyhow::Result<Giveaway> {
    let row=sqlx::query("select aggregate,reserved_unclaimed,quantity_claimed,quantity_total from concert_community_giveaways where event_id=$1 and giveaway_id=$2").bind(event).bind(gift).fetch_one(pool).await?;
    let g: Giveaway = serde_json::from_value(row.get("aggregate"))?;
    assert_eq!(
        g.reserved_unclaimed,
        row.get::<i32, _>("reserved_unclaimed")
    );
    assert_eq!(g.quantity_claimed, row.get::<i32, _>("quantity_claimed"));
    assert_eq!(g.fields.quantity_total, row.get::<i32, _>("quantity_total"));
    assert!(g.available() >= 0);
    if g.reserved_unclaimed == 0 {
        let due:Option<chrono::DateTime<Utc>>=sqlx::query_scalar("select next_action_at from concert_community_giveaways where event_id=$1 and giveaway_id=$2").bind(event).bind(gift).fetch_one(pool).await?;
        assert_eq!(due, None, "terminal root must leave no pending expiry scan");
    }
    assert_eq!(
        g.reservations.iter().filter(|r| r.state.active()).count() as i32,
        g.reserved_unclaimed
    );
    assert_eq!(
        g.reservations
            .iter()
            .filter(|r| r.state == ReservationState::Claimed)
            .count() as i32,
        g.quantity_claimed
    );
    let indexed: i64 = sqlx::query_scalar(
        "select count(*) from concert_community_reservations where event_id=$1 and giveaway_id=$2",
    )
    .bind(event)
    .bind(gift)
    .fetch_one(pool)
    .await?;
    assert_eq!(indexed, g.reservations.len() as i64);
    for reservation in &g.reservations {
        let row=sqlx::query("select recipient_id,state,revision,expires_at from concert_community_reservations where event_id=$1 and reservation_id=$2").bind(event).bind(reservation.id).fetch_one(pool).await?;
        assert_eq!(row.get::<Uuid, _>("recipient_id"), reservation.recipient);
        assert_eq!(
            row.get::<String, _>("state"),
            serde_json::to_value(reservation.state)?.as_str().unwrap()
        );
        assert_eq!(row.get::<i64, _>("revision"), reservation.revision);
        assert_eq!(
            row.get::<chrono::DateTime<Utc>, _>("expires_at"),
            reservation.expires_at
        );
        if reservation.state == ReservationState::Claimed {
            assert!(reservation.claim_receipt_id.is_some() && reservation.claimed_at.is_some());
        }
    }
    terminal(pool).await?;
    Ok(g)
}
async fn rows(pool: &PgPool, event: Uuid, gift: Uuid) -> anyhow::Result<serde_json::Value> {
    let row=sqlx::query("select revision,state,quantity_total,reserved_unclaimed,quantity_claimed,next_action_at from concert_community_giveaways where event_id=$1 and giveaway_id=$2").bind(event).bind(gift).fetch_one(pool).await?;
    let active:i64=sqlx::query_scalar("select count(*) from concert_community_reservations where event_id=$1 and giveaway_id=$2 and state in ('HELD','READY')").bind(event).bind(gift).fetch_one(pool).await?;
    let receipts: i64 = sqlx::query_scalar(
        "select count(*) from concert_command_receipts where event_id=$1 and state='SUCCEEDED'",
    )
    .bind(event)
    .fetch_one(pool)
    .await?;
    let outbox: i64 = sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
        .bind(event)
        .fetch_one(pool)
        .await?;
    Ok(serde_json::json!({
        "revision": row.get::<i64, _>("revision"),
        "state": row.get::<String, _>("state"),
        "quantity_total": row.get::<i32, _>("quantity_total"),
        "reserved_unclaimed": row.get::<i32, _>("reserved_unclaimed"),
        "quantity_claimed": row.get::<i32, _>("quantity_claimed"),
        "next_action_at": row.get::<Option<chrono::DateTime<Utc>>, _>("next_action_at"),
        "active_reservations": active,
        "succeeded_receipts": receipts,
        "outbox_rows": outbox,
    }))
}
async fn lock(
    pool: &PgPool,
    gift: Uuid,
) -> anyhow::Result<(sqlx::Transaction<'_, sqlx::Postgres>, i32)> {
    let mut held = pool.begin().await?;
    let pid = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *held)
        .await?;
    sqlx::query(
        "select giveaway_id from concert_community_giveaways where giveaway_id=$1 for update",
    )
    .bind(gift)
    .fetch_one(&mut *held)
    .await?;
    Ok((held, pid))
}
#[tokio::test]
#[ignore = "requires owned QA03 PostgreSQL controller"]
async fn five_buyers_forced_one_unit_and_duplicate_replay() -> anyhow::Result<()> {
    let pool = pool().await?;
    let (event, _, gift) = fixture(&pool, 1).await?;
    let mut cmds = Vec::new();
    for _ in 0..5 {
        let user = Uuid::new_v4();
        allow(&pool, event, user).await?;
        cmds.push(command(
            user,
            event,
            gift,
            GiftAction::Reserve {
                giveaway_revision: 3,
            },
        ));
    }
    let (held, pid) = lock(&pool, gift).await?;
    state("gift_stock_before", rows(&pool, event, gift).await?);
    let mut jobs = Vec::new();
    for c in cmds {
        let p = pool.clone();
        jobs.push(tokio::spawn(async move {
            let r = execute(&p, &c).await;
            (c, r)
        }));
    }
    waiting(&pool, pid, 5).await?;
    held.commit().await?;
    let mut results = Vec::new();
    for job in jobs {
        results.push(job.await?)
    }
    assert_eq!(results.iter().filter(|(_, r)| r.is_ok()).count(), 1);
    assert!(results
        .iter()
        .filter(|(_, r)| r.is_err())
        .all(|(_, r)| matches!(
            r,
            Err(GiftServiceError::Gift(
                GiftError::VersionConflict | GiftError::SoldOut
            ))
        )));
    let (winner, result) = results.iter().find(|(_, r)| r.is_ok()).unwrap();
    let before: i64 = sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
        .bind(event)
        .fetch_one(&pool)
        .await?;
    assert_eq!(
        execute(&pool, winner).await.unwrap().receipt,
        result.as_ref().unwrap().receipt
    );
    let after: i64 = sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
        .bind(event)
        .fetch_one(&pool)
        .await?;
    assert_eq!(before, after);
    let mut changed = command(
        winner.actor,
        event,
        gift,
        GiftAction::Reserve {
            giveaway_revision: 4,
        },
    );
    changed.key = winner.key;
    assert!(matches!(
        execute(&pool, &changed).await,
        Err(GiftServiceError::Command(CommandError::Conflict))
    ));
    let g = root(&pool, event, gift).await?;
    assert_eq!((g.reserved_unclaimed, g.quantity_claimed), (1, 0));
    state("gift_stock_after", rows(&pool, event, gift).await?);
    let retry = command(
        results.iter().find(|(_, r)| r.is_err()).unwrap().0.actor,
        event,
        gift,
        GiftAction::Reserve {
            giveaway_revision: 4,
        },
    );
    assert!(matches!(
        execute(&pool, &retry).await,
        Err(GiftServiceError::Gift(GiftError::SoldOut))
    ));
    Ok(())
}
async fn reserved(pool: &PgPool) -> anyhow::Result<(Uuid, Uuid, Uuid, Uuid, Uuid, PickupCode)> {
    let (event, owner, gift) = fixture(pool, 1).await?;
    let user = Uuid::new_v4();
    allow(pool, event, user).await?;
    let reserve = command(
        user,
        event,
        gift,
        GiftAction::Reserve {
            giveaway_revision: 3,
        },
    );
    let reservation = reserve.target();
    execute(pool, &reserve)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let issue = command(
        user,
        event,
        gift,
        GiftAction::IssueCode {
            reservation,
            expected: 1,
        },
    );
    let code = execute(pool, &issue)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .issued_code
        .unwrap()
        .code;
    assert!(
        execute(pool, &issue).await.unwrap().issued_code.is_none(),
        "code replay cannot mint plaintext"
    );
    Ok((event, owner, gift, user, reservation, code))
}
#[tokio::test]
#[ignore = "requires owned QA03 PostgreSQL controller"]
async fn claim_cancel_forced_terminal_transition_once() -> anyhow::Result<()> {
    let pool = pool().await?;
    let (event, owner, gift, user, reservation, code) = reserved(&pool).await?;
    let mut claim = command(
        owner,
        event,
        gift,
        GiftAction::Claim {
            reservation,
            expected: 2,
        },
    );
    claim.code = Some(code);
    let cancel = command(
        user,
        event,
        gift,
        GiftAction::Cancel {
            reservation,
            expected: 2,
        },
    );
    let (held, pid) = lock(&pool, gift).await?;
    state("gift_terminal_before", rows(&pool, event, gift).await?);
    let p = pool.clone();
    let a = tokio::spawn(async move {
        let r = execute(&p, &claim).await;
        (claim, r)
    });
    let p = pool.clone();
    let b = tokio::spawn(async move {
        let r = execute(&p, &cancel).await;
        (cancel, r)
    });
    waiting(&pool, pid, 2).await?;
    held.commit().await?;
    let (ca, ra) = a.await?;
    let (cb, rb) = b.await?;
    assert_eq!(usize::from(ra.is_ok()) + usize::from(rb.is_ok()), 1);
    let g = root(&pool, event, gift).await?;
    assert_eq!(g.reserved_unclaimed, 0);
    let terminal_state = g
        .reservations
        .iter()
        .find(|r| r.id == reservation)
        .unwrap()
        .state;
    assert!(matches!(
        terminal_state,
        ReservationState::Claimed | ReservationState::Cancelled
    ));
    assert_eq!(
        g.quantity_claimed,
        i32::from(terminal_state == ReservationState::Claimed)
    );
    let secrets: i64 = sqlx::query_scalar(
        "select count(*) from concert_community_pickup_secrets where event_id=$1",
    )
    .bind(event)
    .fetch_one(&pool)
    .await?;
    assert_eq!(secrets, 0);
    let (c, r) = if ra.is_ok() {
        (&ca, ra.unwrap())
    } else {
        (&cb, rb.unwrap())
    };
    assert_eq!(execute(&pool, c).await.unwrap().receipt, r.receipt);
    state("gift_terminal_after", rows(&pool, event, gift).await?);
    Ok(())
}
#[tokio::test]
#[ignore = "requires owned QA03 PostgreSQL controller"]
async fn duplicate_claim_and_negative_actors_codes() -> anyhow::Result<()> {
    let pool = pool().await?;
    let (event, owner, gift, user, reservation, code) = reserved(&pool).await?;
    let mut wrong = command(
        owner,
        event,
        gift,
        GiftAction::Claim {
            reservation,
            expected: 2,
        },
    );
    wrong.code = Some(
        PickupCode::parse(
            if code.expose() == "000000" {
                "111111"
            } else {
                "000000"
            }
            .into(),
        )
        .unwrap(),
    );
    assert!(matches!(
        execute(&pool, &wrong).await,
        Err(GiftServiceError::Gift(GiftError::CodeInvalid))
    ));
    let mut foreign = command(
        user,
        event,
        gift,
        GiftAction::Claim {
            reservation,
            expected: 2,
        },
    );
    foreign.code = Some(PickupCode::parse(code.expose().into()).unwrap());
    assert!(matches!(
        execute(&pool, &foreign).await,
        Err(GiftServiceError::Command(CommandError::Denied))
    ));
    let key = Uuid::new_v4();
    let raw = code.expose().to_owned();
    let (held, pid) = lock(&pool, gift).await?;
    let mut jobs = Vec::new();
    for _ in 0..2 {
        let p = pool.clone();
        let raw = raw.clone();
        jobs.push(tokio::spawn(async move {
            let mut c = command(
                owner,
                event,
                gift,
                GiftAction::Claim {
                    reservation,
                    expected: 2,
                },
            );
            c.key = key;
            c.code = Some(PickupCode::parse(raw).unwrap());
            execute(&p, &c).await
        }));
    }
    waiting(&pool, pid, 2).await?;
    held.commit().await?;
    let a = jobs.remove(0).await?.unwrap();
    let b = jobs.remove(0).await?.unwrap();
    assert_eq!(a.receipt, b.receipt);
    let attempts_before:i64=sqlx::query_scalar("select coalesce(sum(attempts),0)::bigint from concert_community_pickup_attempts where event_id=$1").bind(event).fetch_one(&pool).await?;
    let admissions_before: i64 = sqlx::query_scalar(
        "select count(*) from concert_community_pickup_admissions where event_id=$1",
    )
    .bind(event)
    .fetch_one(&pool)
    .await?;
    let mut replay = command(
        owner,
        event,
        gift,
        GiftAction::Claim {
            reservation,
            expected: 2,
        },
    );
    replay.key = key;
    replay.code = Some(PickupCode::parse(raw).unwrap());
    assert_eq!(execute(&pool, &replay).await.unwrap().receipt, a.receipt);
    let attempts_after:i64=sqlx::query_scalar("select coalesce(sum(attempts),0)::bigint from concert_community_pickup_attempts where event_id=$1").bind(event).fetch_one(&pool).await?;
    let admissions_after: i64 = sqlx::query_scalar(
        "select count(*) from concert_community_pickup_admissions where event_id=$1",
    )
    .bind(event)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        (attempts_before, admissions_before),
        (attempts_after, admissions_after)
    );
    let g = root(&pool, event, gift).await?;
    assert_eq!((g.reserved_unclaimed, g.quantity_claimed), (0, 1));
    let count:i64=sqlx::query_scalar("select count(*) from concert_command_receipts where event_id=$1 and operation_id='claimConcertReservation' and state='SUCCEEDED'").bind(event).fetch_one(&pool).await?;
    assert_eq!(count, 1);
    let mut second = command(
        owner,
        event,
        gift,
        GiftAction::Claim {
            reservation,
            expected: 2,
        },
    );
    second.key = Uuid::new_v4();
    second.code = Some(PickupCode::parse(code.expose().into()).unwrap());
    assert!(matches!(
        execute(&pool, &second).await,
        Err(GiftServiceError::Gift(GiftError::CodeInvalid))
    ));
    let count:i64=sqlx::query_scalar("select count(*) from concert_command_receipts where event_id=$1 and operation_id='claimConcertReservation' and state='SUCCEEDED'").bind(event).fetch_one(&pool).await?;
    assert_eq!(
        count, 1,
        "a distinct claim key cannot mint a second receipt"
    );
    state("gift_claim_replay_after", rows(&pool, event, gift).await?);
    Ok(())
}
#[tokio::test]
#[ignore = "requires owned QA03 PostgreSQL controller"]
async fn reservation_release_and_re_reservation_forced() -> anyhow::Result<()> {
    let pool = pool().await?;
    let (event, _, gift) = fixture(&pool, 1).await?;
    let first = Uuid::new_v4();
    allow(&pool, event, first).await?;
    let reserve = command(
        first,
        event,
        gift,
        GiftAction::Reserve {
            giveaway_revision: 3,
        },
    );
    let reservation = reserve.target();
    execute(&pool, &reserve)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    state("gift_release_before", rows(&pool, event, gift).await?);
    let before = root(&pool, event, gift).await?;
    assert_eq!((before.reserved_unclaimed, before.quantity_claimed), (1, 0));

    let second = Uuid::new_v4();
    allow(&pool, event, second).await?;
    let cancel = command(
        first,
        event,
        gift,
        GiftAction::Cancel {
            reservation,
            expected: 1,
        },
    );
    // Both contenders hold the same pre-release revision. The reservation may
    // take the released unit only through a fresh revision after the release
    // commits; neither the loss of the unit nor a stale write may pass silently.
    let contender = command(
        second,
        event,
        gift,
        GiftAction::Reserve {
            giveaway_revision: 4,
        },
    );
    let (held, pid) = lock(&pool, gift).await?;
    let p = pool.clone();
    let release = tokio::spawn(async move { execute(&p, &cancel).await });
    let p = pool.clone();
    let stale = tokio::spawn(async move { execute(&p, &contender).await });
    waiting(&pool, pid, 2).await?;
    held.commit().await?;
    let released = release.await?;
    let stale = stale.await?;
    assert!(released.is_ok(), "the valid release must commit");
    assert!(
        matches!(
            stale,
            Err(GiftServiceError::Gift(
                GiftError::VersionConflict | GiftError::SoldOut
            ))
        ),
        "stale reservation must not take the unit"
    );
    let released_root = root(&pool, event, gift).await?;
    assert_eq!(
        (
            released_root.reserved_unclaimed,
            released_root.quantity_claimed
        ),
        (0, 0)
    );
    assert_eq!(
        released_root
            .reservations
            .iter()
            .find(|r| r.id == reservation)
            .unwrap()
            .state,
        ReservationState::Cancelled
    );

    let retry = command(
        second,
        event,
        gift,
        GiftAction::Reserve {
            giveaway_revision: released_root.revision,
        },
    );
    execute(&pool, &retry)
        .await
        .map_err(|e| anyhow::anyhow!("re-reservation failed: {e:?}"))?;
    let after = root(&pool, event, gift).await?;
    assert_eq!((after.reserved_unclaimed, after.quantity_claimed), (1, 0));
    assert_eq!(after.fields.quantity_total, 1);
    assert_eq!(
        after
            .reservations
            .iter()
            .filter(|r| r.state.active())
            .count(),
        1
    );
    assert_eq!(
        after
            .reservations
            .iter()
            .filter(|r| r.recipient == second)
            .count(),
        1
    );
    state("gift_release_after", rows(&pool, event, gift).await?);
    Ok(())
}

#[tokio::test]
#[ignore = "requires owned QA03 PostgreSQL controller before restart"]
async fn interrupted_claim_rolls_back_receipt_survives_restart() -> anyhow::Result<()> {
    let pool = pool().await?;
    let (event, owner, gift, _, reservation, code) = reserved(&pool).await?;
    let key = Uuid::new_v4();
    let raw = code.expose().to_owned();
    state(
        "gift_claim_interrupted_before",
        rows(&pool, event, gift).await?,
    );

    // Hold the pickup secret, not the giveaway root, so admission and the
    // PENDING receipt insert have already started when the claim blocks in the
    // production secret lock. Terminating that backend is a mid-flight crash.
    let mut held = pool.begin().await?;
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *held)
        .await?;
    sqlx::query("select reservation_id from concert_community_pickup_secrets where event_id=$1 and reservation_id=$2 for update")
        .bind(event)
        .bind(reservation)
        .fetch_one(&mut *held)
        .await?;
    let p = pool.clone();
    let job = tokio::spawn(async move {
        let mut c = command(
            owner,
            event,
            gift,
            GiftAction::Claim {
                reservation,
                expected: 2,
            },
        );
        c.key = key;
        c.code = Some(PickupCode::parse(raw).unwrap());
        execute(&p, &c).await
    });
    waiting(&pool, pid, 1).await?;
    let blocked = blocked_backend(&pool, pid).await?;
    let write_locks: (bool, bool) = sqlx::query_as(
        "select exists(select 1 from pg_locks where pid=$1 and locktype='transactionid' and granted), exists(select 1 from pg_locks where pid=$1 and relation='concert_command_receipts'::regclass and mode='RowExclusiveLock')",
    )
    .bind(blocked)
    .fetch_one(&pool)
    .await?;
    assert!(
        write_locks.0 && write_locks.1,
        "claim must already hold its write transaction and receipt relation lock"
    );
    state(
        "gift_claim_blocked_locks",
        serde_json::json!({"backend_pid": blocked, "write_transaction": write_locks.0, "receipt_relation_row_exclusive": write_locks.1}),
    );
    let terminated: bool = sqlx::query_scalar("select pg_terminate_backend($1)")
        .bind(blocked)
        .fetch_one(&pool)
        .await?;
    assert!(terminated, "blocked claim backend must terminate");
    assert!(
        job.await?.is_err(),
        "interrupted claim cannot report success"
    );
    held.rollback().await?;

    terminal(&pool).await?;
    let receipt: i64 = sqlx::query_scalar(
        "select count(*) from concert_command_receipts where actor_id=$1 and event_id=$2 and idempotency_key=$3",
    )
    .bind(owner)
    .bind(event)
    .bind(key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(receipt, 0, "uncommitted receipt must roll back");
    let admission: i64 = sqlx::query_scalar(
        "select count(*) from concert_community_pickup_admissions where actor_id=$1 and event_id=$2 and idempotency_key=$3",
    )
    .bind(owner)
    .bind(event)
    .bind(key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        admission, 1,
        "admission is durable before the interrupted receipt"
    );
    let root = root(&pool, event, gift).await?;
    assert_eq!((root.reserved_unclaimed, root.quantity_claimed), (1, 0));
    state(
        "gift_claim_interrupted_after",
        rows(&pool, event, gift).await?,
    );
    sqlx::query("insert into qa03_restart values('gift_claim',$1) on conflict(kind) do update set payload=excluded.payload")
        .bind(serde_json::json!({"actor":owner,"event":event,"gift":gift,"reservation":reservation,"key":key,"code":code.expose()}))
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires actual server restart phase of owned QA03 controller"]
async fn restarted_claim_completes_once() -> anyhow::Result<()> {
    let pool = pool().await?;
    let v: serde_json::Value =
        sqlx::query_scalar("select payload from qa03_restart where kind='gift_claim'")
            .fetch_one(&pool)
            .await?;
    let id = |k: &str| -> Uuid { serde_json::from_value(v[k].clone()).unwrap() };
    let (event, owner, gift, reservation) =
        (id("event"), id("actor"), id("gift"), id("reservation"));
    let code = PickupCode::parse(v["code"].as_str().unwrap().into()).unwrap();
    let mut claim = command(
        owner,
        event,
        gift,
        GiftAction::Claim {
            reservation,
            expected: 2,
        },
    );
    claim.key = id("key");
    claim.code = Some(code);
    let first = execute(&pool, &claim)
        .await
        .map_err(|e| anyhow::anyhow!("post-restart claim failed: {e:?}"))?;
    assert_eq!(execute(&pool, &claim).await.unwrap().receipt, first.receipt);
    let receipts: i64 = sqlx::query_scalar(
        "select count(*) from concert_command_receipts where actor_id=$1 and event_id=$2 and operation_id='claimConcertReservation' and idempotency_key=$3 and state='SUCCEEDED'",
    )
    .bind(owner)
    .bind(event)
    .bind(claim.key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(receipts, 1);
    let root = root(&pool, event, gift).await?;
    assert_eq!((root.reserved_unclaimed, root.quantity_claimed), (0, 1));
    state(
        "gift_claim_restarted_after",
        rows(&pool, event, gift).await?,
    );
    Ok(())
}

async fn expire(pool: &PgPool, event: Uuid, gift: Uuid) -> anyhow::Result<()> {
    // Fixture-only clock injection updates the aggregate and indexed deadline
    // together; it never changes application code or attaches an ambient store.
    let mut tx = pool.begin().await?;
    let expired: chrono::DateTime<Utc> =
        sqlx::query_scalar("select clock_timestamp()-interval '1 second'")
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query("update concert_community_giveaways set aggregate=jsonb_set(aggregate,'{reservations,0,expires_at}',$3),next_action_at=$4 where event_id=$1 and giveaway_id=$2").bind(event).bind(gift).bind(serde_json::to_value(expired)?).bind(expired).execute(&mut *tx).await?;
    sqlx::query("update concert_community_reservations set expires_at=$3 where event_id=$1 and giveaway_id=$2").bind(event).bind(gift).bind(expired).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
#[tokio::test]
#[ignore = "requires owned QA03 PostgreSQL controller"]
async fn expiry_cancel_claim_release_once() -> anyhow::Result<()> {
    let pool = pool().await?;
    let (event, owner, gift, user, reservation, code) = reserved(&pool).await?;
    expire(&pool, event, gift).await?;
    let (held, pid) = lock(&pool, gift).await?;
    assert_eq!(
        PgGifts::new(pool.clone(), IsolatedAuthority)
            .sweep(100)
            .await
            .unwrap(),
        0
    );
    let mut claim = command(
        owner,
        event,
        gift,
        GiftAction::Claim {
            reservation,
            expected: 2,
        },
    );
    claim.code = Some(code);
    let cancel = command(
        user,
        event,
        gift,
        GiftAction::Cancel {
            reservation,
            expected: 2,
        },
    );
    let p = pool.clone();
    let a = tokio::spawn(async move { execute(&p, &claim).await });
    let p = pool.clone();
    let b = tokio::spawn(async move { execute(&p, &cancel).await });
    waiting(&pool, pid, 2).await?;
    held.commit().await?;
    let claim = a.await?;
    let cancel = b.await?;
    assert!(claim.is_err(), "expired reservation cannot be claimed");
    assert!(
        cancel.is_ok()
            || matches!(
                cancel,
                Err(GiftServiceError::Gift(
                    GiftError::VersionConflict | GiftError::InvalidTransition
                ))
            )
    );
    let service = PgGifts::new(pool.clone(), IsolatedAuthority);
    service.sweep(100).await.unwrap();
    let g = root(&pool, event, gift).await?;
    assert_eq!((g.reserved_unclaimed, g.quantity_claimed), (0, 0));
    assert!(matches!(
        g.reservations[0].state,
        ReservationState::Expired | ReservationState::Cancelled
    ));
    let before: i64 = sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
        .bind(event)
        .fetch_one(&pool)
        .await?;
    assert_eq!(service.sweep(100).await.unwrap(), 0);
    let after: i64 = sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
        .bind(event)
        .fetch_one(&pool)
        .await?;
    assert_eq!(before, after);
    let secrets: i64 = sqlx::query_scalar(
        "select count(*) from concert_community_pickup_secrets where event_id=$1",
    )
    .bind(event)
    .fetch_one(&pool)
    .await?;
    assert_eq!(secrets, 0);
    terminal(&pool).await
}
#[tokio::test]
#[ignore = "requires owned QA03 PostgreSQL controller"]
async fn expiry_skip_locked_pending_worker_restart_seed() -> anyhow::Result<()> {
    let pool = pool().await?;
    let (event, owner, gift, user, reservation, code) = reserved(&pool).await?;
    expire(&pool, event, gift).await?;
    let (held, pid) = lock(&pool, gift).await?;
    assert_eq!(
        PgGifts::new(pool.clone(), IsolatedAuthority)
            .sweep(100)
            .await
            .unwrap(),
        0,
        "worker must skip held aggregate"
    );
    let mut claim = command(
        owner,
        event,
        gift,
        GiftAction::Claim {
            reservation,
            expected: 2,
        },
    );
    claim.code = Some(code);
    let cancel = command(
        user,
        event,
        gift,
        GiftAction::Cancel {
            reservation,
            expected: 2,
        },
    );
    let p = pool.clone();
    let a = tokio::spawn(async move { execute(&p, &claim).await });
    let p = pool.clone();
    let b = tokio::spawn(async move { execute(&p, &cancel).await });
    waiting(&pool, pid, 2).await?;
    // Aborting both blocked worker tasks models interruption before admission.
    a.abort();
    b.abort();
    let _ = a.await;
    let _ = b.await;
    held.rollback().await?;
    sqlx::query("insert into qa03_restart values('gift',$1)")
        .bind(serde_json::json!({"event":event,"gift":gift,"reservation":reservation}))
        .execute(&pool)
        .await?;
    let g = root(&pool, event, gift).await?;
    assert_eq!(g.reserved_unclaimed, 1);
    Ok(())
}
#[tokio::test]
#[ignore = "requires actual server restart phase of owned QA03 controller"]
async fn restarted_worker_drains_expiry_exactly_once() -> anyhow::Result<()> {
    let pool = pool().await?;
    let v: serde_json::Value =
        sqlx::query_scalar("select payload from qa03_restart where kind='gift'")
            .fetch_one(&pool)
            .await?;
    let id = |k: &str| -> Uuid { serde_json::from_value(v[k].clone()).unwrap() };
    let service = PgGifts::new(pool.clone(), IsolatedAuthority);
    assert_eq!(service.sweep(1).await.unwrap(), 1);
    let g = root(&pool, id("event"), id("gift")).await?;
    assert_eq!((g.reserved_unclaimed, g.quantity_claimed), (0, 0));
    assert_eq!(g.reservations[0].state, ReservationState::Expired);
    let before: i64 = sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
        .bind(id("event"))
        .fetch_one(&pool)
        .await?;
    assert_eq!(service.sweep(100).await.unwrap(), 0);
    let after: i64 = sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
        .bind(id("event"))
        .fetch_one(&pool)
        .await?;
    assert_eq!(before, after);
    let secrets: i64 = sqlx::query_scalar(
        "select count(*) from concert_community_pickup_secrets where event_id=$1",
    )
    .bind(id("event"))
    .fetch_one(&pool)
    .await?;
    assert_eq!(secrets, 0);
    terminal(&pool).await
}
