//! Uses real PgCommunity/BE15. Scenario construction adapted from
//! community_party.rs at 93ef0d8; external lock graph assertions are new QA03 work.
use super::support::*;
use concert_application::{commands::CommandError, community::PartyCommand};
use concert_domain::community::party::*;
use concert_service::community::*;
use sqlx::{PgPool, Row};
use uuid::Uuid;

fn command(actor: Uuid, event: Uuid, party: Uuid, action: PartyAction) -> PartyCommand {
    PartyCommand {
        actor,
        event,
        party,
        action,
        key: Uuid::new_v4(),
    }
}
async fn create(pool: &PgPool) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
    let (event, owner, place) = event(pool).await?;
    let c = command(
        owner,
        event,
        Uuid::nil(),
        PartyAction::Create(PartyFields {
            title: "QA03 capacity".into(),
            description: String::new(),
            ticket_section: None,
            capacity: 2,
            meeting_starts_at: future(),
            meeting_ends_at: future() + chrono::Duration::hours(1),
            requirements: String::new(),
            public_place_id: place,
        }),
    );
    let id = c.party_id();
    PgCommunity::new(pool.clone(), IsolatedAuthority)
        .execute_party(&c, &keys())
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    Ok((event, owner, id))
}
async fn apply(pool: &PgPool, event: Uuid, party: Uuid) -> anyhow::Result<(Uuid, Uuid)> {
    let user = Uuid::new_v4();
    allow(pool, event, user).await?;
    let c = command(
        user,
        event,
        party,
        PartyAction::Apply {
            party_revision: 1,
            note: "QA03 private fixture".into(),
        },
    );
    let id = c.target();
    PgCommunity::new(pool.clone(), IsolatedAuthority)
        .execute_party(&c, &keys())
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    Ok((user, id))
}
async fn check(pool: &PgPool, party: Uuid) -> anyhow::Result<()> {
    let value: serde_json::Value =
        sqlx::query_scalar("select aggregate from concert_community_parties where party_id=$1")
            .bind(party)
            .fetch_one(pool)
            .await?;
    let p: Party = serde_json::from_value(value)?;
    let active = p
        .members
        .iter()
        .filter(|m| m.state == MemberState::Active)
        .count();
    assert!(active <= p.fields.capacity as usize);
    assert!(p
        .members
        .iter()
        .any(|m| m.user == p.owner && m.state == MemberState::Active));
    let indexed: (i32, i32) = sqlx::query_as(
        "select member_count,capacity from concert_community_parties where party_id=$1",
    )
    .bind(party)
    .fetch_one(pool)
    .await?;
    assert_eq!(indexed, (active as i32, p.fields.capacity));
    let members: i64 = sqlx::query_scalar(
        "select count(*) from concert_community_members where party_id=$1 and state='ACTIVE'",
    )
    .bind(party)
    .fetch_one(pool)
    .await?;
    assert_eq!(members, active as i64);
    let owners:i64=sqlx::query_scalar("select count(*) from concert_community_members where party_id=$1 and is_owner and state='ACTIVE'").bind(party).fetch_one(pool).await?;
    assert_eq!(owners, 1);
    terminal(pool).await
}
#[tokio::test]
#[ignore = "requires owned QA03 PostgreSQL controller"]
async fn last_slot_and_identical_replay_are_forced() -> anyhow::Result<()> {
    let pool = pool().await?;
    let (event, owner, party) = create(&pool).await?;
    let (_, a) = apply(&pool, event, party).await?;
    let (_, b) = apply(&pool, event, party).await?;
    let mut held = pool.begin().await?;
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *held)
        .await?;
    sqlx::query("select party_id from concert_community_parties where party_id=$1 for update")
        .bind(party)
        .fetch_one(&mut *held)
        .await?;
    let mut jobs = Vec::new();
    for application in [a, b] {
        let p = pool.clone();
        jobs.push(tokio::spawn(async move {
            let c = command(
                owner,
                event,
                party,
                PartyAction::Decide {
                    application,
                    expected: 1,
                    party_revision: 1,
                    approve: true,
                },
            );
            let result = PgCommunity::new(p, IsolatedAuthority)
                .execute_party(&c, &keys())
                .await;
            (c, result)
        }));
    }
    waiting(&pool, pid, 2).await?;
    held.commit().await?;
    let mut outcomes = Vec::new();
    for job in jobs {
        outcomes.push(job.await?)
    }
    assert_eq!(outcomes.iter().filter(|(_, r)| r.is_ok()).count(), 1);
    assert!(outcomes
        .iter()
        .any(|(_, r)| matches!(r, Err(CommunityError::Party(PartyError::VersionConflict)))));
    let (winner, result) = outcomes.iter().find(|(_, r)| r.is_ok()).unwrap();
    let loser = &outcomes.iter().find(|(_, r)| r.is_err()).unwrap().0;
    let application = match loser.action {
        PartyAction::Decide { application, .. } => application,
        _ => unreachable!(),
    };
    let full = command(
        owner,
        event,
        party,
        PartyAction::Decide {
            application,
            expected: 1,
            party_revision: 2,
            approve: true,
        },
    );
    let service = PgCommunity::new(pool.clone(), IsolatedAuthority);
    assert_eq!(
        service.execute_party(&full, &keys()).await,
        Err(CommunityError::Party(PartyError::Full))
    );
    assert_eq!(
        service.execute_party(&full, &keys()).await,
        Err(CommunityError::Party(PartyError::Full)),
        "failed receipt replay keeps capacity full"
    );
    let before: i64 = sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
        .bind(event)
        .fetch_one(&pool)
        .await?;
    let mut held = pool.begin().await?;
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *held)
        .await?;
    sqlx::query("select party_id from concert_community_parties where party_id=$1 for update")
        .bind(party)
        .fetch_one(&mut *held)
        .await?;
    let mut replays = Vec::new();
    for _ in 0..2 {
        let p = pool.clone();
        let c = PartyCommand {
            actor: winner.actor,
            event: winner.event,
            key: winner.key,
            party: winner.party,
            action: winner.action.clone(),
        };
        replays.push(tokio::spawn(async move {
            PgCommunity::new(p, IsolatedAuthority)
                .execute_party(&c, &keys())
                .await
        }));
    }
    waiting(&pool, pid, 2).await?;
    held.commit().await?;
    for replay in replays {
        assert_eq!(&replay.await?, result)
    }
    let after: i64 = sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
        .bind(event)
        .fetch_one(&pool)
        .await?;
    assert_eq!(before, after);
    let mut changed = PartyCommand {
        actor: winner.actor,
        event: winner.event,
        key: winner.key,
        party: winner.party,
        action: winner.action.clone(),
    };
    if let PartyAction::Decide {
        ref mut approve, ..
    } = changed.action
    {
        *approve = false;
    }
    assert_eq!(
        PgCommunity::new(pool.clone(), IsolatedAuthority)
            .execute_party(&changed, &keys())
            .await,
        Err(CommunityError::Command(CommandError::Conflict))
    );
    let orphan = command(
        owner,
        event,
        party,
        PartyAction::Remove {
            user: owner,
            expected: 1,
            party_revision: 2,
        },
    );
    assert!(PgCommunity::new(pool.clone(), IsolatedAuthority)
        .execute_party(&orphan, &keys())
        .await
        .is_err());
    check(&pool, party).await?;
    // Durable replay identity survives a fresh process and actual server restart.
    sqlx::query("insert into qa03_restart values('party',$1)").bind(serde_json::json!({"actor":winner.actor,"event":winner.event,"key":winner.key,"party":winner.party,"application":match winner.action{PartyAction::Decide{application,..}=>application,_=>unreachable!()},"receipt":result.as_ref().unwrap().id})).execute(&pool).await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires restart phase of owned QA03 controller"]
async fn restart_preserves_approval_receipt() -> anyhow::Result<()> {
    let pool = pool().await?;
    let row = sqlx::query("select payload from qa03_restart where kind='party'")
        .fetch_one(&pool)
        .await?;
    let v: serde_json::Value = row.get(0);
    let id = |k: &str| -> Uuid { serde_json::from_value(v[k].clone()).unwrap() };
    let c = PartyCommand {
        actor: id("actor"),
        event: id("event"),
        key: id("key"),
        party: id("party"),
        action: PartyAction::Decide {
            application: id("application"),
            expected: 1,
            party_revision: 1,
            approve: true,
        },
    };
    let before: i64 = sqlx::query_scalar("select count(*) from concert_outbox")
        .fetch_one(&pool)
        .await?;
    let result = PgCommunity::new(pool.clone(), IsolatedAuthority)
        .execute_party(&c, &keys())
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    assert_eq!(result.id, id("receipt"));
    let after: i64 = sqlx::query_scalar("select count(*) from concert_outbox")
        .fetch_one(&pool)
        .await?;
    assert_eq!(before, after);
    check(&pool, c.party).await
}
