//! Database invariants for the actual Party/BE15 mutation producer.
//! These injected-owner tests do not certify canonical RPC or browser delivery.
use super::*;
use concert_domain::community::party::Party;

async fn mutate(
    pool: &PgPool,
    actor: Uuid,
    event: Uuid,
    key: Uuid,
    action: PartyLiveAction,
) -> Result<PartyLiveReply, PartyLiveError> {
    let base = format!("/api/concert/events/{event}");
    let (op, target, body) = match &action {
        PartyLiveAction::Apply { party, party_revision, note } => (
            "applyConcertParty", format!("{base}/parties/{party}/applications"),
            json!({"party_revision":party_revision,"note":note}),
        ),
        PartyLiveAction::Decide { application, expected_revision, party_revision, approve } => (
            "decideConcertApplication", format!("{base}/party-applications/{application}/decision"),
            json!({"expected_revision":expected_revision,"party_revision":party_revision,
                "decision":if *approve {"APPROVED"} else {"REJECTED"}}),
        ),
        PartyLiveAction::CancelApplication { application, expected_revision } => (
            "cancelConcertApplication", format!("{base}/party-applications/{application}?expected_revision={expected_revision}"), Value::Null,
        ),
        PartyLiveAction::Remove { party, user, expected_revision, party_revision } => (
            "removeConcertMember", format!("{base}/parties/{party}/members/{user}?expected_revision={expected_revision}&party_revision={party_revision}"), Value::Null,
        ),
        PartyLiveAction::CancelParty { party, expected_revision } => (
            "cancelConcertParty", format!("{base}/parties/{party}/cancel"), json!({"expected_revision":expected_revision}),
        ),
        _ => panic!("mutation fixture scope"),
    };
    let bytes = if body.is_null() {
        vec![]
    } else {
        serde_json::to_vec(&body).unwrap()
    };
    request(
        &service(pool.clone(), false, false),
        binding(actor, event, Uuid::new_v4(), op, target, &bytes, Some(key)),
        action,
    )
    .await
}

async fn aggregate(pool: &PgPool, event: Uuid, party: Uuid) -> anyhow::Result<Party> {
    let value: Value = sqlx::query_scalar(
        "select aggregate from concert_community_parties where event_id=$1 and party_id=$2",
    )
    .bind(event)
    .bind(party)
    .fetch_one(pool)
    .await?;
    Ok(serde_json::from_value(value)?)
}

async fn apply(
    pool: &PgPool,
    actor: Uuid,
    event: Uuid,
    party: Uuid,
    revision: i64,
) -> anyhow::Result<Uuid> {
    let reply = mutate(
        pool,
        actor,
        event,
        Uuid::new_v4(),
        PartyLiveAction::Apply {
            party,
            party_revision: revision,
            note: "Private applicant note".into(),
        },
    )
    .await?;
    anyhow::ensure!(reply.body["state"] == "PENDING");
    Ok(Uuid::parse_str(
        reply.body["application_id"].as_str().unwrap(),
    )?)
}

fn decide(application: Uuid, party_revision: i64) -> PartyLiveAction {
    PartyLiveAction::Decide {
        application,
        expected_revision: 1,
        party_revision,
        approve: true,
    }
}

async fn read_root_before_pair(
    pool: &PgPool,
    event: Uuid,
    owner: Uuid,
    party: Uuid,
) -> anyhow::Result<()> {
    for list in [false, true] {
        let mut pair_holder = pool.begin().await?;
        let holder_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
            .fetch_one(&mut *pair_holder)
            .await?;
        sqlx::query("select revision from concert_relationship_pairs where event_id=$1 and low_user=$2 and high_user=$2 for update")
            .bind(event).bind(owner).fetch_one(&mut *pair_holder).await?;
        let reader_pool = pool.clone();
        let reader = tokio::spawn(async move {
            let (operation, target, action) = if list {
                (
                    "listConcertParties",
                    format!("/api/concert/events/{event}/parties"),
                    PartyLiveAction::List(PartyQuery::default()),
                )
            } else {
                (
                    "getConcertParty",
                    format!("/api/concert/events/{event}/parties/{party}"),
                    PartyLiveAction::Detail(party),
                )
            };
            request(
                &service(reader_pool, false, false),
                binding(owner, event, Uuid::new_v4(), operation, target, &[], None),
                action,
            )
            .await
        });
        let mut waiting = false;
        for _ in 0..100 {
            waiting = sqlx::query_scalar("select exists(select 1 from pg_stat_activity where $1 = any(pg_blocking_pids(pid)))")
                .bind(holder_pid).fetch_one(pool).await?;
            if waiting {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        anyhow::ensure!(
            waiting && !reader.is_finished(),
            "read did not reach held pair"
        );
        // A separate real transaction must find the root already SHARE-locked
        // while the public read is blocked at policy. No scheduling sleep proves it.
        let mut probe = pool.begin().await?;
        let locked = sqlx::query("select party_id from concert_community_parties where event_id=$1 and party_id=$2 for update nowait")
            .bind(event).bind(party).fetch_one(&mut *probe).await;
        let has_root = locked
            .as_ref()
            .err()
            .and_then(sqlx::Error::as_database_error)
            .and_then(|error| error.code())
            .as_deref()
            == Some("55P03");
        probe.rollback().await?;
        pair_holder.rollback().await?;
        reader.await??;
        anyhow::ensure!(
            has_root,
            "public read acquired policy pair before Party root"
        );
    }
    Ok(())
}

async fn fixture_block(
    pool: &PgPool,
    event: Uuid,
    actor: Uuid,
    subject: Uuid,
    blocked: bool,
) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    // Use the explicitly injected fixture authority, retaining the real BE37
    // deadline and revision triggers rather than disabling either guard.
    PgRelationshipPolicy::new(pool.clone(), TestOwnerFence)
        .authorize_in(
            &mut tx,
            PolicyQuery {
                event: EventId::from_uuid(event),
                actor: concert_domain::AuthUserId::from_uuid(actor),
                subject: concert_domain::AuthUserId::from_uuid(actor),
                field: Field::SafetySelf,
            },
            None,
        )
        .await
        .map_err(|error| anyhow::anyhow!("block fixture authority: {error:?}"))?;
    sqlx::query("insert into concert_relationship_pairs(event_id,low_user,high_user,low_blocks_high,high_blocks_low) values($1,$2,$3,$4,false) on conflict(event_id,low_user,high_user) do update set low_blocks_high=excluded.low_blocks_high,revision=concert_relationship_pairs.revision+1")
        .bind(event).bind(actor.min(subject)).bind(actor.max(subject)).bind(blocked)
        .execute(&mut *tx).await.context("block fixture guarded update")?;
    tx.commit().await.context("block fixture guarded commit")?;
    Ok(())
}

pub(super) async fn mutation_invariants(pool: &PgPool) -> anyhow::Result<()> {
    let (event, owner, first, place) = event(pool, "OPEN").await?;
    let second = Uuid::new_v4();
    let stranger = Uuid::new_v4();
    for user in [second, stranger] {
        sqlx::query(
            "insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')",
        )
        .bind(event)
        .bind(user)
        .execute(pool)
        .await?;
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'PARADE',false,false,true)")
            .bind(event).bind(user).execute(pool).await?;
        sqlx::query(
            "update party_live_authority set members=array_append(members,$1) where event_id=$2",
        )
        .bind(user)
        .bind(event)
        .execute(pool)
        .await?;
    }
    let mut f = fields(place);
    f.capacity = 2;
    let c = create_binding(owner, event, &f, Uuid::new_v4());
    let created = request(&service(pool.clone(), false, false), c.0, c.1).await?;
    let party = Uuid::parse_str(created.body["party_id"].as_str().unwrap())?;
    read_root_before_pair(pool, event, owner, party).await?;
    let one = apply(pool, first, event, party, 1).await?;
    let two = apply(pool, second, event, party, 1).await?;
    let duplicate_key = Uuid::new_v4();
    for _ in 0..2 {
        anyhow::ensure!(
            mutate(
                pool,
                first,
                event,
                duplicate_key,
                PartyLiveAction::Apply {
                    party,
                    party_revision: 1,
                    note: "duplicate".into(),
                }
            )
            .await
                == Err(PartyLiveError::DuplicateApplication),
            "duplicate must be durable"
        );
    }
    let before: i64 =
        sqlx::query_scalar("select count(*) from concert_command_receipts where event_id=$1")
            .bind(event)
            .fetch_one(pool)
            .await?;
    for action in [
        decide(one, 1),
        PartyLiveAction::CancelApplication {
            application: one,
            expected_revision: 1,
        },
        PartyLiveAction::Remove {
            party,
            user: owner,
            expected_revision: 1,
            party_revision: 1,
        },
        PartyLiveAction::CancelParty {
            party,
            expected_revision: 1,
        },
    ] {
        anyhow::ensure!(
            mutate(pool, stranger, event, Uuid::new_v4(), action).await
                == Err(PartyLiveError::Denied),
            "foreign actor mutation permitted"
        );
    }
    let after: i64 =
        sqlx::query_scalar("select count(*) from concert_command_receipts where event_id=$1")
            .bind(event)
            .fetch_one(pool)
            .await?;
    anyhow::ensure!(before == after, "authorization failures wrote receipts");

    fixture_block(pool, event, owner, first, true).await?;
    anyhow::ensure!(
        mutate(pool, owner, event, Uuid::new_v4(), decide(one, 1)).await
            == Err(PartyLiveError::Denied),
        "blocked applicant approved"
    );
    fixture_block(pool, event, owner, first, false).await?;

    // Competing approvals serialize on the root; only one can use the last slot.
    let key_one = Uuid::new_v4();
    let key_two = Uuid::new_v4();
    let (a, b) = tokio::join!(
        mutate(pool, owner, event, key_one, decide(one, 1)),
        mutate(pool, owner, event, key_two, decide(two, 1)),
    );
    anyhow::ensure!(
        a.is_ok() != b.is_ok(),
        "last slot had zero or two winners: {a:?} {b:?}"
    );
    let (winner, winner_app, winner_key, loser_app, loser) = if a.is_ok() {
        anyhow::ensure!(b == Err(PartyLiveError::VersionConflict));
        (first, one, key_one, two, second)
    } else {
        anyhow::ensure!(a == Err(PartyLiveError::VersionConflict));
        (second, two, key_two, one, first)
    };
    let root = aggregate(pool, event, party).await?;
    anyhow::ensure!(root.member_count() == 2 && root.revision == 2);
    let full_key = Uuid::new_v4();
    for _ in 0..2 {
        anyhow::ensure!(
            mutate(pool, owner, event, full_key, decide(loser_app, 2)).await
                == Err(PartyLiveError::PartyFull)
        );
    }
    anyhow::ensure!(
        mutate(pool, owner, event, winner_key, decide(winner_app, 1))
            .await?
            .body["state"]
            == "APPROVED",
        "successful decision replay failed"
    );
    let effects_before: i64 =
        sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
            .bind(event)
            .fetch_one(pool)
            .await?;
    let (replay_a, replay_b) = tokio::join!(
        mutate(pool, owner, event, winner_key, decide(winner_app, 1)),
        mutate(pool, owner, event, winner_key, decide(winner_app, 1)),
    );
    anyhow::ensure!(replay_a?.body["state"] == "APPROVED" && replay_b?.body["state"] == "APPROVED");
    let effects_after: i64 =
        sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
            .bind(event)
            .fetch_one(pool)
            .await?;
    anyhow::ensure!(
        effects_before == effects_after,
        "concurrent replay repeated outbox effects"
    );
    anyhow::ensure!(
        mutate(pool, owner, event, winner_key, decide(winner_app, 2)).await
            == Err(PartyLiveError::VersionConflict),
        "changed command reused key"
    );
    anyhow::ensure!(
        mutate(
            pool,
            owner,
            event,
            Uuid::new_v4(),
            PartyLiveAction::Remove {
                party,
                user: owner,
                expected_revision: 1,
                party_revision: 2,
            }
        )
        .await
            == Err(PartyLiveError::InvalidTransition),
        "owner departure orphaned active party"
    );

    // A committed audience reader prevents removal until its own transaction ends.
    let mut reader = pool.begin().await?;
    let reader_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *reader)
        .await?;
    let audience = concert_service::community::audience::party_audience_in(
        &mut reader,
        event,
        party,
        winner,
        None,
    )
    .await?;
    let task_pool = pool.clone();
    let leave_key = Uuid::new_v4();
    let leave = tokio::spawn(async move {
        mutate(
            &task_pool,
            winner,
            event,
            leave_key,
            PartyLiveAction::Remove {
                party,
                user: winner,
                expected_revision: 1,
                party_revision: 2,
            },
        )
        .await
    });
    let mut blocked = false;
    for _ in 0..100 {
        blocked = sqlx::query_scalar(
            "select exists(select 1 from pg_stat_activity where $1 = any(pg_blocking_pids(pid)))",
        )
        .bind(reader_pid)
        .fetch_one(pool)
        .await?;
        if blocked {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    anyhow::ensure!(
        blocked && !leave.is_finished(),
        "mutation did not wait for held audience SHARE"
    );
    reader.commit().await?;
    anyhow::ensure!(leave.await??.body["state"] == "LEFT");
    let mut tx = pool.begin().await?;
    anyhow::ensure!(
        concert_service::community::audience::party_audience_in(
            &mut tx, event, party, winner, None
        )
        .await
        .is_err(),
        "departed member retained audience"
    );
    anyhow::ensure!(
        concert_service::community::audience::party_audience_in(
            &mut tx,
            event,
            party,
            owner,
            Some(audience.audience_revision)
        )
        .await
        .is_err(),
        "old audience generation survived leave"
    );
    tx.rollback().await?;
    anyhow::ensure!(
        aggregate(pool, event, party).await?.member_count() == 1,
        "leave did not release capacity"
    );
    anyhow::ensure!(
        mutate(
            pool,
            winner,
            event,
            leave_key,
            PartyLiveAction::Remove {
                party,
                user: winner,
                expected_revision: 1,
                party_revision: 2,
            }
        )
        .await?
        .body["state"]
            == "LEFT",
        "leave replay repeated effects"
    );
    mutate(pool, owner, event, Uuid::new_v4(), decide(loser_app, 3)).await?;
    let audience_before_remove = aggregate(pool, event, party).await?.audience_revision;
    let remove_key = Uuid::new_v4();
    for _ in 0..2 {
        let removed = mutate(
            pool,
            owner,
            event,
            remove_key,
            PartyLiveAction::Remove {
                party,
                user: loser,
                expected_revision: 1,
                party_revision: 4,
            },
        )
        .await?;
        anyhow::ensure!(removed.body["state"] == "REMOVED");
    }
    let mut tx = pool.begin().await?;
    anyhow::ensure!(
        concert_service::community::audience::party_audience_in(&mut tx, event, party, loser, None)
            .await
            .is_err(),
        "removed member retained private audience"
    );
    anyhow::ensure!(
        concert_service::community::audience::party_audience_in(
            &mut tx,
            event,
            party,
            owner,
            Some(audience_before_remove)
        )
        .await
        .is_err(),
        "old audience generation survived owner removal"
    );
    tx.rollback().await?;
    let reapply = apply(pool, winner, event, party, 5).await?;
    mutate(pool, owner, event, Uuid::new_v4(), decide(reapply, 5)).await?;
    let root = aggregate(pool, event, party).await?;
    anyhow::ensure!(
        root.members
            .iter()
            .find(|m| m.user == winner)
            .unwrap()
            .revision
            == 3,
        "reapproval reused old member generation"
    );

    let pending = apply(pool, loser, event, party, 6).await?;
    let cancel_key = Uuid::new_v4();
    for _ in 0..2 {
        anyhow::ensure!(
            mutate(
                pool,
                loser,
                event,
                cancel_key,
                PartyLiveAction::CancelApplication {
                    application: pending,
                    expected_revision: 1,
                }
            )
            .await?
            .body["state"]
                == "CANCELLED"
        );
    }
    let pending = apply(pool, loser, event, party, 6).await?;
    let cancel_key = Uuid::new_v4();
    for _ in 0..2 {
        anyhow::ensure!(
            mutate(
                pool,
                owner,
                event,
                cancel_key,
                PartyLiveAction::CancelParty {
                    party,
                    expected_revision: 6,
                }
            )
            .await?
            .body["state"]
                == "CANCELLED"
        );
    }
    let root = aggregate(pool, event, party).await?;
    anyhow::ensure!(root.revision == 7 && root.owner == owner);
    anyhow::ensure!(
        root.applications
            .iter()
            .find(|a| a.id == pending)
            .unwrap()
            .state
            == concert_domain::community::party::ApplicationState::Cancelled
    );
    let mut tx = pool.begin().await?;
    anyhow::ensure!(
        concert_service::community::audience::party_audience_in(
            &mut tx, event, party, winner, None
        )
        .await
        .is_err(),
        "cancelled party retained audience"
    );
    tx.rollback().await?;
    // Current authority also gates replay, even for previously succeeded receipts.
    sqlx::query("update party_live_authority set unavailable=true where event_id=$1")
        .bind(event)
        .execute(pool)
        .await?;
    anyhow::ensure!(
        mutate(
            pool,
            owner,
            event,
            cancel_key,
            PartyLiveAction::CancelParty {
                party,
                expected_revision: 6,
            }
        )
        .await
            == Err(PartyLiveError::DependencyUnavailable)
    );
    Ok(())
}
