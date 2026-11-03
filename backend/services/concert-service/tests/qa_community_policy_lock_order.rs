//! Independent native lock-order/rollback qualification. Remote Auth/Friend proof is synthetic.
//! No production policy, public-route, live-multiuser, or IN02 acceptance is established.
#[path = "support/meeting/mod.rs"]
mod support;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::{
    beacon::*, commands::*, community::PartyCommand, place::SelectionOutcome,
};
use concert_domain::{
    beacon::*, community::party::*, place::PlaceId, relationship::*, AuthUserId, EventId,
    ParticipantKey, Revision,
};
use concert_service::{
    adapters::out::repository::place::PgPlaceRepository,
    community::{CommunityAccess, CommunityAuthority, CommunityError, PgCommunity},
    meeting::{
        authority::PgMeetingAuthority,
        beacon::{read_in, PgMeetingRepository},
        commands::BeaconCommands,
        MeetingAuthority,
    },
    relationship::{FencedRelationshipAuthority, PgRelationshipPolicy},
};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    Acquire, PgConnection, PgPool, Row,
};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration as StdDuration,
};
use tokio::sync::{mpsc, Semaphore};
use uuid::Uuid;
const WAIT: StdDuration = StdDuration::from_secs(3);

/// Owned PostgreSQL cluster pool verification ensuring marker, inode, socket, and data directory isolation.
pub async fn guarded_owned_pool() -> anyhow::Result<PgPool> {
    let root = PathBuf::from(
        std::env::var("CONCERT_TEST_OWNER_ROOT")
            .or_else(|_| std::env::var("MEETING_TEST_OWNER_ROOT"))
            .map_err(|_| anyhow::anyhow!("guarded owned fixture root required"))?,
    );
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        info.is_dir() && !info.file_type().is_symlink() && info.permissions().mode() & 0o077 == 0,
        "private owned root required"
    );
    let identity_var = std::env::var("CONCERT_TEST_OWNER_IDENTITY")
        .or_else(|_| std::env::var("MEETING_TEST_OWNER_IDENTITY"))?;
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == identity_var,
        "fixture inode changed"
    );
    let marker_path = root.join("mcr-owner.json");
    let marker_info = std::fs::symlink_metadata(&marker_path)?;
    anyhow::ensure!(
        marker_info.is_file()
            && !marker_info.file_type().is_symlink()
            && marker_info.uid() == info.uid()
            && marker_info.permissions().mode() & 0o077 == 0,
        "private marker required"
    );
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(marker_path)?)?;
    let token_var = std::env::var("CONCERT_TEST_OWNER_TOKEN")
        .or_else(|_| std::env::var("MEETING_TEST_OWNER_TOKEN"))?;
    anyhow::ensure!(
        marker["token"].as_str() == Some(token_var.as_str())
            && marker["uid"].as_u64() == Some(u64::from(info.uid())),
        "fixture owner mismatch"
    );
    for child in ["data", "socket"] {
        let child_info = std::fs::symlink_metadata(root.join(child))?;
        anyhow::ensure!(
            child_info.is_dir()
                && !child_info.file_type().is_symlink()
                && child_info.uid() == info.uid()
                && child_info.permissions().mode() & 0o077 == 0,
            "private fixture child required"
        );
    }
    let url_var = std::env::var("CONCERT_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("MEETING_TEST_DATABASE_URL"))?;
    let options = PgConnectOptions::from_str(&url_var)
        .map_err(|_| anyhow::anyhow!("invalid guarded fixture options"))?;
    anyhow::ensure!(
        (options.get_database() == Some("mcr_community")
            || options.get_database() == Some("mcr_meeting"))
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(StdDuration::from_secs(3))
        .after_connect(|c, _| {
            Box::pin(async move {
                // Prepared single statements keep the callback's borrowed
                // connection lifetime explicit; raw_sql's stream is not Send
                // for every callback lifetime on the pinned SQLx toolchain.
                for statement in [
                    "set statement_timeout='5s'",
                    "set lock_timeout='4s'",
                    "set idle_in_transaction_session_timeout='10s'",
                ] {
                    sqlx::query(statement).execute(&mut *c).await?;
                }
                Ok(())
            })
        })
        .connect_with(options)
        .await
        .map_err(|_| anyhow::anyhow!("guarded connection failed"))?;
    let actual: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        actual == root.join("data").to_string_lossy(),
        "foreign data directory refused"
    );
    let listen: String = sqlx::query_scalar("show listen_addresses")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(listen.is_empty(), "TCP-enabled fixture refused");
    Ok(pool)
}

// Fixture rows model only remote owner restrictions, never community membership or pair policy.
#[derive(Clone, Copy)]
struct SyntheticOwnerFence;
#[async_trait]
impl FencedRelationshipAuthority for SyntheticOwnerFence {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        q: PolicyQuery,
        now: DateTime<Utc>,
    ) -> Result<FreshAuthority, RelationshipError> {
        let row = sqlx::query("select * from meeting_fixture_authority where event=$1 for share")
            .bind(q.event.uuid())
            .fetch_one(c)
            .await
            .map_err(|_| RelationshipError::Unavailable)?;
        if row.get::<bool, _>("unavailable") {
            return Err(RelationshipError::Unavailable);
        }
        let members: Vec<Uuid> = row.get("members");
        let blocked: Vec<Uuid> = row.get("blocked");
        Ok(FreshAuthority {
            query: q,
            checked_at: now,
            valid_until: now + Duration::seconds(30),
            revision: Revision::initial(),
            actor_allowed: members.contains(&q.actor.uuid()),
            subject_allowed: members.contains(&q.subject.uuid()),
            inherited_block: blocked.contains(&q.actor.uuid())
                || blocked.contains(&q.subject.uuid()),
        })
    }
}
// A scheduling gate around the real policy call; it never grants access itself.
struct WriterGate {
    first: AtomicBool,
    reached: mpsc::Sender<i32>,
    proceed: Semaphore,
    subjects: Mutex<Vec<Uuid>>,
}
struct ActualCommunityPolicy {
    policy: PgRelationshipPolicy<SyntheticOwnerFence>,
    gate: Option<Arc<WriterGate>>,
}
fn command_policy(error: RelationshipError) -> CommandError {
    match error {
        RelationshipError::Denied | RelationshipError::Stale => CommandError::Denied,
        RelationshipError::Unavailable => CommandError::Unavailable,
    }
}
#[async_trait]
impl CommunityAuthority for ActualCommunityPolicy {
    async fn actor_in(
        &self,
        c: &mut PgConnection,
        actor: Uuid,
        event: Uuid,
        _: CommunityAccess,
    ) -> Result<(), CommandError> {
        self.policy
            .authorize_actor_in(
                c,
                ParticipantKey {
                    event_id: EventId::from_uuid(event),
                    user_id: AuthUserId::from_uuid(actor),
                },
            )
            .await
            .map_err(command_policy)?;
        Ok(())
    }
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        actor: Uuid,
        event: Uuid,
        subject: Uuid,
        access: CommunityAccess,
    ) -> Result<(), CommandError> {
        if let Some(gate) = &self.gate {
            if !gate.first.swap(true, Ordering::SeqCst) {
                let pid = sqlx::query_scalar("select pg_backend_pid()")
                    .fetch_one(&mut *c)
                    .await
                    .map_err(|_| CommandError::Unavailable)?;
                gate.reached
                    .try_send(pid)
                    .map_err(|_| CommandError::Unavailable)?;
                tokio::time::timeout(WAIT, gate.proceed.acquire())
                    .await
                    .map_err(|_| CommandError::Unavailable)?
                    .map_err(|_| CommandError::Unavailable)?
                    .forget();
            }
        }
        let field = match access {
            CommunityAccess::ReadParties | CommunityAccess::ReadGifts => Field::PublicProfile,
            CommunityAccess::WriteParties | CommunityAccess::WriteGifts => Field::Interaction,
            _ if actor == subject => Field::SafetySelf,
            _ => Field::SafetyInteraction,
        };
        self.policy
            .authorize_in(
                c,
                PolicyQuery {
                    event: EventId::from_uuid(event),
                    actor: AuthUserId::from_uuid(actor),
                    subject: AuthUserId::from_uuid(subject),
                    field,
                },
                None,
            )
            .await
            .map_err(command_policy)?;
        if let Some(gate) = &self.gate {
            gate.subjects.lock().unwrap().push(subject);
        }
        Ok(())
    }
    async fn place_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        place: Uuid,
    ) -> Result<(), CommandError> {
        let mut tx = c.begin().await.map_err(|_| CommandError::Unavailable)?;
        let row = sqlx::query("select catalog_revision,map_revision from concert_events_map_releases where event_id=$1 for share")
            .bind(event).fetch_one(&mut *tx).await.map_err(|_| CommandError::Unavailable)?;
        let outcome = PgPlaceRepository::select_physical_in_transaction(
            &mut tx,
            EventId::from_uuid(event),
            PlaceId::from_uuid(place).unwrap(),
            Revision::new(row.get("catalog_revision")).unwrap(),
            row.get("map_revision"),
        )
        .await
        .map_err(|_| CommandError::Unavailable)?;
        if !matches!(outcome, SelectionOutcome::Eligible(_)) {
            return Err(CommandError::Denied);
        }
        tx.commit().await.map_err(|_| CommandError::Unavailable)
    }
}
fn keys() -> FingerprintKeys {
    FingerprintKeys::new(
        SigningKey::new("independent-lock-qa", &[77; 32]).unwrap(),
        vec![],
    )
    .unwrap()
}
fn command(f: &support::Fixture, actor: Uuid, party: Uuid, action: PartyAction) -> PartyCommand {
    PartyCommand {
        actor,
        event: f.event,
        key: Uuid::new_v4(),
        party,
        action,
    }
}
fn bridge(pool: &PgPool, actor: MeetingActor) -> PgMeetingAuthority<SyntheticOwnerFence> {
    PgMeetingAuthority::new(pool.clone(), actor, SyntheticOwnerFence)
}
fn community(pool: &PgPool, gate: Option<Arc<WriterGate>>) -> PgCommunity<ActualCommunityPolicy> {
    PgCommunity::new(
        pool.clone(),
        ActualCommunityPolicy {
            policy: PgRelationshipPolicy::new(pool.clone(), SyntheticOwnerFence),
            gate,
        },
    )
}
async fn execute(
    service: &PgCommunity<ActualCommunityPolicy>,
    c: &PartyCommand,
) -> anyhow::Result<SafeReceipt> {
    let result = tokio::time::timeout(WAIT, service.execute_party(c, &keys()))
        .await?
        .map_err(|e| anyhow::anyhow!("actual community command: {e:?}"))?;
    anyhow::ensure!(
        result.state == ReceiptState::Succeeded,
        "fixture command must succeed"
    );
    Ok(result)
}
async fn blocked(pool: &PgPool, waiting: i32, holder: i32) -> anyhow::Result<()> {
    tokio::time::timeout(WAIT, async {
        loop {
            let pids: Vec<i32> = sqlx::query_scalar("select pg_blocking_pids($1)")
                .bind(waiting)
                .fetch_one(pool)
                .await?;
            if pids.contains(&holder) {
                return Ok::<(), sqlx::Error>(());
            }
            tokio::time::sleep(StdDuration::from_millis(5)).await;
        }
    })
    .await??;
    Ok(())
}
// Observe a real command connection without replacing actor/root/pair checks.
struct ObservedMeeting {
    inner: PgMeetingAuthority<SyntheticOwnerFence>,
    reached: mpsc::Sender<i32>,
}
#[async_trait]
impl MeetingAuthority for ObservedMeeting {
    async fn actor_in(
        &self,
        c: &mut PgConnection,
        actor: MeetingActor,
    ) -> Result<(), MeetingError> {
        self.inner.actor_in(c, actor).await?;
        let pid = sqlx::query_scalar("select pg_backend_pid()")
            .fetch_one(c)
            .await
            .map_err(|_| MeetingError::StorageUnavailable)?;
        let _ = self.reached.try_send(pid);
        Ok(())
    }
    async fn lock_audiences_in(
        &self,
        c: &mut PgConnection,
        actor: MeetingActor,
        audiences: &[Audience],
    ) -> Result<(), MeetingError> {
        self.actor_in(c, actor).await?;
        self.inner.lock_audiences_in(c, actor, audiences).await
    }
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        q: AudienceQuery,
    ) -> Result<AudienceGrant, MeetingError> {
        self.inner.authorize_in(c, q).await
    }
}

#[tokio::test]
async fn actual_commands_contend_stage_all_effects_then_rollback_and_reauthorize(
) -> anyhow::Result<()> {
    let pool = guarded_owned_pool().await?;
    // Existing place fixture is test-only provenance, not reviewed real venue guidance.
    let f = support::setup(&pool).await?;
    for user in [f.owner, f.peer] {
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)")
            .bind(f.event).bind(user).execute(&pool).await?;
    }
    let service = community(&pool, None);
    let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&pool)
        .await?;
    let create = command(
        &f,
        f.owner,
        Uuid::nil(),
        PartyAction::Create(PartyFields {
            title: "Independent lock QA".into(),
            description: "Native fixture only".into(),
            ticket_section: None,
            capacity: 2,
            meeting_starts_at: now,
            meeting_ends_at: now + Duration::hours(1),
            requirements: String::new(),
            public_place_id: f.meeting.place_id,
        }),
    );
    let party = create.party_id();
    execute(&service, &create).await?;
    let apply = command(
        &f,
        f.peer,
        party,
        PartyAction::Apply {
            party_revision: 1,
            note: String::new(),
        },
    );
    execute(&service, &apply).await?;
    execute(
        &service,
        &command(
            &f,
            f.owner,
            party,
            PartyAction::Decide {
                application: apply.target(),
                expected: 1,
                party_revision: 1,
                approve: true,
            },
        ),
    )
    .await?;
    let mut meeting = f.meeting.clone();
    meeting.audience = Audience::PartyMembers { party_id: party };
    let beacon = Uuid::new_v4();
    let actor = f.actor(f.owner);
    let beacon_receipt = tokio::time::timeout(
        WAIT,
        BeaconCommands::new(pool.clone(), bridge(&pool, actor)).execute(
            actor,
            beacon,
            Uuid::new_v4(),
            BeaconMutation::Create(CreateBeacon {
                meeting: meeting.clone(),
                physical_presence_acknowledged: true,
            }),
            &keys(),
        ),
    )
    .await??;
    assert_eq!(beacon_receipt.state, ReceiptState::Succeeded);
    let before: serde_json::Value = sqlx::query_scalar(
        "select aggregate from concert_community_parties where event_id=$1 and party_id=$2",
    )
    .bind(f.event)
    .bind(party)
    .fetch_one(&pool)
    .await?;
    let before_budget: i64 = sqlx::query_scalar("select receipts_used from concert_command_budget")
        .fetch_one(&pool)
        .await?;
    let removal = command(
        &f,
        f.owner,
        party,
        PartyAction::Remove {
            user: f.peer,
            expected: 1,
            party_revision: 2,
        },
    );
    let removal_key = removal.key;
    // The AFTER UPDATE hook runs after the ACTUAL BE15 producer inserts outbox and
    // terminal receipt. It reaches the witness lock only if all staged effects exist.
    // An exception after that lock is released forces real repository rollback.
    let witness_key: i64 = 7_371_009;
    sqlx::raw_sql(&format!(r#"
        create function qa_abort_staged_command() returns trigger language plpgsql as $$
        begin
          if (select count(*) from concert_command_receipts where receipt_id=NEW.receipt_id and state='SUCCEEDED'
              and idempotency_key='{removal_key}' and target_id='{peer}') <> 1
            or (select count(*) from concert_outbox where receipt_id=NEW.receipt_id and event_id=NEW.event_id
              and resource_id=NEW.target_id and resource_revision=NEW.resource_revision and state='READY') <> 1
            or not exists(select 1 from concert_community_members where event_id=NEW.event_id
              and party_id='{party}' and user_id='{peer}' and state='REMOVED' and revision=2)
            or not exists(select 1 from concert_community_parties where event_id=NEW.event_id
              and party_id='{party}' and revision=3 and member_count=1
              and exists(select 1 from jsonb_array_elements(aggregate->'members') m
                where m->>'user'='{peer}' and m->>'state'='REMOVED'))
            then raise exception 'QA staged-effect precondition failed';
          end if;
          perform pg_advisory_xact_lock({witness_key});
          raise exception 'QA injected failure after verified domain receipt outbox staging';
        end $$;
        create trigger qa_abort_staged_command after update on concert_command_receipts
          for each row when (NEW.idempotency_key='{removal_key}' and NEW.state='SUCCEEDED')
          execute function qa_abort_staged_command();
    "#, peer=f.peer)).execute(&pool).await?;
    let mut witness = pool.begin().await?;
    let witness_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *witness)
        .await?;
    sqlx::query("select pg_advisory_xact_lock($1)")
        .bind(witness_key)
        .execute(&mut *witness)
        .await?;
    let (reached, mut writer_rx) = mpsc::channel(1);
    let gate = Arc::new(WriterGate {
        first: AtomicBool::new(false),
        reached,
        proceed: Semaphore::new(0),
        subjects: Mutex::new(vec![]),
    });
    let writer_service = community(&pool, Some(gate.clone()));
    // Timeout lives INSIDE each spawned task so an assertion failure cannot leave
    // an unbounded writer/reader running; server statement/lock timeouts also apply.
    let writer = tokio::spawn(async move {
        tokio::time::timeout(
            StdDuration::from_secs(8),
            writer_service.execute_party(&removal, &keys()),
        )
        .await
    });
    let writer_pid = tokio::time::timeout(WAIT, writer_rx.recv())
        .await?
        .ok_or_else(|| anyhow::anyhow!("writer did not reach root"))?;
    let mut reader = pool.begin().await?;
    let reader_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *reader)
        .await?;
    let authority = bridge(&pool, actor);
    tokio::time::timeout(WAIT, authority.actor_in(&mut reader, actor)).await??;
    let pending_reader = tokio::spawn(async move {
        tokio::time::timeout(StdDuration::from_secs(8), async move {
            let value = read_in(&mut reader, &authority, actor, beacon).await?;
            reader
                .commit()
                .await
                .map_err(|_| MeetingError::StorageUnavailable)?;
            Ok::<_, MeetingError>(value)
        })
        .await
    });
    blocked(&pool, reader_pid, writer_pid).await?;
    // A second REAL receipt-backed meeting command must block before BE15 budget.
    let (reached, mut beacon_rx) = mpsc::channel(8);
    let command_pool = pool.clone();
    let pending_beacon = Uuid::new_v4();
    let beacon_writer = tokio::spawn(async move {
        let authority = ObservedMeeting {
            inner: bridge(&command_pool, actor),
            reached,
        };
        tokio::time::timeout(
            StdDuration::from_secs(8),
            BeaconCommands::new(command_pool, authority).execute(
                actor,
                pending_beacon,
                Uuid::new_v4(),
                BeaconMutation::Create(CreateBeacon {
                    meeting,
                    physical_presence_acknowledged: true,
                }),
                &keys(),
            ),
        )
        .await
    });
    let beacon_pid = tokio::time::timeout(WAIT, beacon_rx.recv())
        .await?
        .ok_or_else(|| anyhow::anyhow!("meeting command did not reach actor guard"))?;
    blocked(&pool, beacon_pid, writer_pid).await?;
    gate.proceed.add_permits(1);
    // Bounded writer progress while BOTH readers still wait on its actual root:
    // reaching the witness proves pair policy, BE15 budget, domain+receipt+outbox SQL completed.
    blocked(&pool, writer_pid, witness_pid).await?;
    blocked(&pool, reader_pid, writer_pid).await?;
    blocked(&pool, beacon_pid, writer_pid).await?;
    let subjects = gate.subjects.lock().unwrap().clone();
    let mut expected = vec![f.owner, f.peer];
    expected.sort();
    assert!(subjects.len() >= 2);
    assert_eq!(
        &subjects[..2],
        expected.as_slice(),
        "actual producer takes sorted peer pairs after root"
    );
    let visible: i64 = sqlx::query_scalar(
        "select count(*) from concert_command_receipts where event_id=$1 and idempotency_key=$2",
    )
    .bind(f.event)
    .bind(removal_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(visible, 0, "staged receipt is isolated before abort");
    witness.rollback().await?;
    let failed = tokio::time::timeout(WAIT, writer).await???;
    assert_eq!(
        failed,
        Err(CommunityError::Command(CommandError::Unavailable))
    );
    let got = tokio::time::timeout(WAIT, pending_reader).await????;
    assert_eq!(got.beacon_id, beacon);
    assert_eq!(got.revision, 1);
    let completed = tokio::time::timeout(WAIT, beacon_writer).await????;
    assert_eq!(completed.state, ReceiptState::Succeeded);
    let after: serde_json::Value = sqlx::query_scalar(
        "select aggregate from concert_community_parties where event_id=$1 and party_id=$2",
    )
    .bind(f.event)
    .bind(party)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        after, before,
        "rollback restores the entire actual aggregate"
    );
    let member: (String, i64) = sqlx::query_as("select state,revision from concert_community_members where event_id=$1 and party_id=$2 and user_id=$3")
        .bind(f.event).bind(party).bind(f.peer).fetch_one(&pool).await?;
    assert_eq!(member, ("ACTIVE".into(), 1));
    let receipts: i64 = sqlx::query_scalar(
        "select count(*) from concert_command_receipts where event_id=$1 and idempotency_key=$2",
    )
    .bind(f.event)
    .bind(removal_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(receipts, 0, "actual removal receipt rolled back");
    let outbox: i64 = sqlx::query_scalar(
        "select count(*) from concert_outbox where event_id=$1 and resource_id=$2",
    )
    .bind(f.event)
    .bind(f.peer)
    .fetch_one(&pool)
    .await?;
    assert_eq!(outbox, 0, "previously witnessed removal outbox rolled back");
    let budget: i64 = sqlx::query_scalar("select receipts_used from concert_command_budget")
        .fetch_one(&pool)
        .await?;
    assert_eq!(
        budget,
        before_budget + 1,
        "only competing committed beacon consumed budget"
    );
    // Positive control: same key remains executable, then actual committed removal
    // denies stale meeting reads without waiting for outbox delivery.
    sqlx::raw_sql("drop trigger qa_abort_staged_command on concert_command_receipts; drop function qa_abort_staged_command()")
        .execute(&pool).await?;
    let mut retry = command(
        &f,
        f.owner,
        party,
        PartyAction::Remove {
            user: f.peer,
            expected: 1,
            party_revision: 2,
        },
    );
    retry.key = removal_key;
    let receipt = execute(&service, &retry).await?;
    assert_eq!(execute(&service, &retry).await?.id, receipt.id);
    let outbox: i64 = sqlx::query_scalar(
        "select count(*) from concert_outbox where receipt_id=$1 and resource_id=$2",
    )
    .bind(receipt.id)
    .bind(f.peer)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        outbox, 1,
        "positive control confirms nonvacuous exact outbox correlation"
    );
    assert_eq!(
        PgMeetingRepository::new(pool.clone(), bridge(&pool, f.actor(f.peer)))
            .read(f.actor(f.peer), beacon)
            .await,
        Err(MeetingError::ResourceUnavailable)
    );
    pool.close().await;
    Ok(())
}
