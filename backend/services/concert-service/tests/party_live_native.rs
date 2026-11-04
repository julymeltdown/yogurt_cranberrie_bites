//! Owned PostgreSQL acceptance fixture for the bounded Party producer.
//!
//! The owner guard below is deterministic test material only. It cannot be
//! constructed by production composition and never grants access through an
//! observation or a bare identifier.
mod party_live_support;
use anyhow::Context;
use async_trait::async_trait;
use chrono::{Duration, Utc};
use concert_application::commands::{FingerprintKeys, ReceiptState, SigningKey};
use concert_application::place::PlaceRepository;
use concert_domain::place::{canonical_json_bytes, parse_stored_release, CatalogRelease};
use concert_domain::{
    community::party::PartyFields,
    relationship::{Field, FreshAuthority, PolicyQuery, RelationshipError},
    EventId, ParticipantKey, Revision,
};
use concert_service::{
    adapters::out::repository::place::PgPlaceRepository,
    community::{CommunityAccess, CommunityAuthority, PgCommunity},
    party_live::{
        PartyCursorCodec, PartyLiveAction, PartyLiveBinding, PartyLiveError, PartyLiveOwnerFence,
        PartyLiveOwnerGuard, PartyLiveOwnerWork, PartyLiveReply, PartyLiveService, PartyQuery,
    },
    relationship::{FencedRelationshipAuthority, PgRelationshipPolicy},
};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use serde_json::{json, Value};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    Acquire, PgConnection, PgPool, Row,
};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    str::FromStr,
    sync::Arc,
};
use uuid::Uuid;

async fn owned_pool() -> anyhow::Result<PgPool> {
    let root = PathBuf::from(std::env::var("CONCERT_TEST_OWNER_ROOT")?);
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        root.is_absolute()
            && root.canonicalize()? == root
            && info.is_dir()
            && !info.file_type().is_symlink()
            && info.permissions().mode() & 0o077 == 0,
        "private owned root required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == std::env::var("CONCERT_TEST_OWNER_IDENTITY")?,
        "fixture identity changed"
    );
    let marker = root.join("mcr-owner.json");
    let marker_info = std::fs::symlink_metadata(&marker)?;
    anyhow::ensure!(
        marker_info.is_file()
            && !marker_info.file_type().is_symlink()
            && marker_info.permissions().mode() & 0o077 == 0,
        "private marker required"
    );
    let value: Value = serde_json::from_slice(&std::fs::read(marker)?)?;
    anyhow::ensure!(
        value["token"].as_str() == Some(std::env::var("CONCERT_TEST_OWNER_TOKEN")?.as_str())
            && value["uid"].as_u64() == Some(info.uid() as u64),
        "fixture owner mismatch"
    );
    let options = PgConnectOptions::from_str(&std::env::var("CONCERT_TEST_DATABASE_URL")?)?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_party_live")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect_with(options)
        .await?;
    let actual: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        actual == root.join("data").to_string_lossy(),
        "foreign data directory refused"
    );
    Ok(pool)
}

struct TestOwnerFence;
#[async_trait]
impl FencedRelationshipAuthority for TestOwnerFence {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        query: PolicyQuery,
        now: chrono::DateTime<Utc>,
    ) -> Result<FreshAuthority, RelationshipError> {
        let row = sqlx::query("select members,blocked,unavailable from party_live_authority where event_id=$1 for share")
            .bind(query.event.uuid())
            .fetch_optional(&mut *c)
            .await
            .map_err(|_| RelationshipError::Unavailable)?
            .ok_or(RelationshipError::Denied)?;
        if row.get::<bool, _>("unavailable") {
            return Err(RelationshipError::Unavailable);
        }
        let members: Vec<Uuid> = row.get("members");
        let blocked: Vec<Uuid> = row.get("blocked");
        let pair = sqlx::query("select revision,low_blocks_high,high_blocks_low from party_live_pair_policy where event_id=$1 and low_user=$2 and high_user=$3 for share")
            .bind(query.event.uuid())
            .bind(query.actor.uuid().min(query.subject.uuid()))
            .bind(query.actor.uuid().max(query.subject.uuid()))
            .fetch_optional(&mut *c)
            .await
            .map_err(|_| RelationshipError::Unavailable);
        let pair_blocked = match pair {
            Ok(Some(value)) => {
                value.get::<bool, _>("low_blocks_high") || value.get::<bool, _>("high_blocks_low")
            }
            Ok(None) => false,
            Err(error) => return Err(error),
        };
        Ok(FreshAuthority {
            query,
            checked_at: now,
            valid_until: now + Duration::seconds(30),
            revision: Revision::initial(),
            actor_allowed: members.contains(&query.actor.uuid()),
            subject_allowed: members.contains(&query.subject.uuid()),
            inherited_block: blocked.contains(&query.actor.uuid())
                || blocked.contains(&query.subject.uuid())
                || pair_blocked,
        })
    }
}

struct TestCommunityPolicy {
    policy: PgRelationshipPolicy<TestOwnerFence>,
    require_registration: bool,
}
fn command_policy(error: RelationshipError) -> concert_application::commands::CommandError {
    match error {
        RelationshipError::Denied | RelationshipError::Stale => {
            concert_application::commands::CommandError::Denied
        }
        RelationshipError::Unavailable => concert_application::commands::CommandError::Unavailable,
    }
}
#[async_trait]
impl CommunityAuthority for TestCommunityPolicy {
    async fn actor_in(
        &self,
        c: &mut PgConnection,
        actor: Uuid,
        event: Uuid,
        _: CommunityAccess,
    ) -> Result<(), concert_application::commands::CommandError> {
        self.policy
            .authorize_actor_in(
                c,
                ParticipantKey {
                    event_id: EventId::from_uuid(event),
                    user_id: concert_domain::AuthUserId::from_uuid(actor),
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
    ) -> Result<(), concert_application::commands::CommandError> {
        if self.require_registration {
            let actual: Option<String> = sqlx::query_scalar(
                "select nullif(current_setting('mcr.party_test_registered',true),'')",
            )
            .fetch_one(&mut *c)
            .await
            .map_err(|_| concert_application::commands::CommandError::Unavailable)?;
            if actual.as_deref() != Some(&format!("{actor}:{event}")) {
                return Err(concert_application::commands::CommandError::Unavailable);
            }
        }
        self.policy
            .authorize_in(
                c,
                PolicyQuery {
                    event: EventId::from_uuid(event),
                    actor: concert_domain::AuthUserId::from_uuid(actor),
                    subject: concert_domain::AuthUserId::from_uuid(subject),
                    field: match access {
                        CommunityAccess::ReadParties | CommunityAccess::ReadGifts => {
                            Field::PublicProfile
                        }
                        CommunityAccess::WriteParties | CommunityAccess::WriteGifts => {
                            Field::Interaction
                        }
                        _ if actor == subject => Field::SafetySelf,
                        _ => Field::SafetyInteraction,
                    },
                },
                None,
            )
            .await
            .map_err(command_policy)?;
        Ok(())
    }
    async fn place_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        place: Uuid,
    ) -> Result<(), concert_application::commands::CommandError> {
        let mut tx = c
            .begin()
            .await
            .map_err(|_| concert_application::commands::CommandError::Unavailable)?;
        let row = sqlx::query("select catalog_revision,map_revision from concert_events_map_releases where event_id=$1 for share").bind(event).fetch_one(&mut *tx).await.map_err(|_| concert_application::commands::CommandError::Unavailable)?;
        let outcome = PgPlaceRepository::select_physical_in_transaction(
            &mut tx,
            EventId::from_uuid(event),
            concert_domain::place::PlaceId::from_uuid(place).unwrap(),
            Revision::new(row.get("catalog_revision")).unwrap(),
            row.get("map_revision"),
        )
        .await
        .map_err(|_| concert_application::commands::CommandError::Unavailable)?;
        // The supplied canonical C06 fixture carries a virtual concept. Party
        // creation checks that the selected release member exists and is open;
        // physical eligibility belongs to the separate meeting-beacon flow.
        let _ = outcome;
        tx.commit()
            .await
            .map_err(|_| concert_application::commands::CommandError::Unavailable)
    }
}

struct TestGuard {
    binding: Arc<PartyLiveBinding>,
    fail_profile: bool,
    fail_track: bool,
}
#[async_trait]
impl PartyLiveOwnerGuard for TestGuard {
    fn binding_digest(&self) -> [u8; 32] {
        self.binding.digest()
    }
    fn authority_revision(&self) -> i64 {
        1
    }
    async fn authorize_in(
        &self,
        connection: &mut PgConnection,
        binding: &PartyLiveBinding,
        subject: Uuid,
    ) -> Result<(), PartyLiveError> {
        if binding.digest() != self.binding.digest() || self.fail_profile {
            return Err(PartyLiveError::DependencyUnavailable);
        }
        let expires = chrono::DateTime::from_timestamp_micros(binding.deadline().expires_micros())
            .ok_or(PartyLiveError::DependencyUnavailable)?;
        sqlx::query("select set_config('mcr.relationship_authority_until',least(coalesce(nullif(current_setting('mcr.relationship_authority_until',true),'')::timestamptz,'infinity'::timestamptz),$1::timestamptz)::text,true)")
            .bind(expires.to_rfc3339())
            .execute(&mut *connection)
            .await
            .map_err(|_| PartyLiveError::DependencyUnavailable)?;
        let _ = subject;
        Ok(())
    }
    async fn public_profile_in(
        &self,
        _: &mut PgConnection,
        binding: &PartyLiveBinding,
        subject: Uuid,
    ) -> Result<Value, PartyLiveError> {
        if binding.digest() != self.binding.digest() || self.fail_profile {
            return Err(PartyLiveError::DependencyUnavailable);
        }
        let profile = json!({"user_id":subject,"display_name":"Owner","bio":null,"avatar":{"revision":1,"config":{"contract_version":"1.0.0","kind":"avatar_config","manifest_id":"avatar_fixture","manifest_version":1,"item_ids":{"body":"body_fixture","eyes":"eyes_fixture","hair":"hair_fixture","outfit":"outfit_fixture","accessory":"accessory_fixture"},"skin_tone_id":"tone_fixture"}}});
        Ok(profile)
    }
    async fn application_profile_in(
        &self,
        connection: &mut PgConnection,
        binding: &PartyLiveBinding,
        subject: Uuid,
    ) -> Result<Value, PartyLiveError> {
        self.public_profile_in(connection, binding, subject).await
    }
    async fn track_in(
        &self,
        connection: &mut PgConnection,
        binding: &PartyLiveBinding,
    ) -> Result<(), PartyLiveError> {
        if binding.digest() != self.binding.digest() || self.fail_track {
            return Err(PartyLiveError::DependencyUnavailable);
        }
        sqlx::query("select set_config('mcr.party_test_registered',$1,true)")
            .bind(format!("{}:{}", binding.actor(), binding.event()))
            .execute(connection)
            .await
            .map_err(|_| PartyLiveError::DependencyUnavailable)?;
        Ok(())
    }
}

struct TestFence {
    fail_profile: bool,
    fail_track: bool,
}
#[async_trait]
impl PartyLiveOwnerFence for TestFence {
    async fn run(
        &self,
        binding: Arc<PartyLiveBinding>,
        work: PartyLiveOwnerWork,
    ) -> Result<PartyLiveReply, PartyLiveError> {
        work(Arc::new(TestGuard {
            binding,
            fail_profile: self.fail_profile,
            fail_track: self.fail_track,
        }))
        .await
    }
}

fn keys() -> Arc<FingerprintKeys> {
    Arc::new(
        FingerprintKeys::new(
            SigningKey::new("party-live-native", &[73; 32]).unwrap(),
            vec![],
        )
        .unwrap(),
    )
}
fn deadline() -> AuthorityRpcDeadline {
    AuthorityRpcDeadline::new(
        Utc::now().timestamp_micros() + 4_000_000,
        std::time::Duration::from_secs(5),
    )
    .unwrap()
}
fn binding(
    actor: Uuid,
    event: Uuid,
    request: Uuid,
    operation: &'static str,
    target: String,
    body: &[u8],
    key: Option<Uuid>,
) -> Arc<PartyLiveBinding> {
    Arc::new(PartyLiveBinding::test_binding(
        actor,
        event,
        Uuid::new_v4(),
        request,
        deadline(),
        operation,
        match operation {
            "listConcertParties" | "getConcertParty" => "GET",
            "removeConcertMember" | "cancelConcertApplication" => "DELETE",
            _ => "POST",
        },
        target,
        body,
        key,
        Some([8; 32]),
    ))
}
fn service(
    pool: PgPool,
    fail_profile: bool,
    fail_track: bool,
) -> PartyLiveService<TestCommunityPolicy, TestFence> {
    PartyLiveService::new(
        pool.clone(),
        Arc::new(TestCommunityPolicy {
            policy: PgRelationshipPolicy::new(pool, TestOwnerFence),
            require_registration: true,
        }),
        keys(),
        PartyCursorCodec::new(&[84; 32]).unwrap(),
        Arc::new(TestFence {
            fail_profile,
            fail_track,
        }),
    )
}
async fn request(
    service: &PartyLiveService<TestCommunityPolicy, TestFence>,
    binding: Arc<PartyLiveBinding>,
    action: PartyLiveAction,
) -> Result<PartyLiveReply, PartyLiveError> {
    service.dispatch_guarded_for_test(binding, action).await
}

async fn members(pool: &PgPool, event: Uuid, users: &[Uuid]) -> anyhow::Result<()> {
    for user in users {
        sqlx::query(
            "insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')",
        )
        .bind(event)
        .bind(user)
        .execute(pool)
        .await?;
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'PARADE',false,false,true)").bind(event).bind(user).execute(pool).await?;
    }
    sqlx::query("insert into party_live_authority(event_id,members,blocked,unavailable) values($1,$2,'{}',false)").bind(event).bind(users).execute(pool).await?;
    Ok(())
}
async fn event(pool: &PgPool, phase: &str) -> anyhow::Result<(Uuid, Uuid, Uuid, Uuid)> {
    let event = Uuid::new_v4();
    let owner = Uuid::new_v4();
    let other = Uuid::new_v4();
    sqlx::query(
        "insert into concert_events(event_id,slug,title,phase) values($1,$2,'Party native',$3)",
    )
    .bind(event)
    .bind(format!("party-{event}"))
    .bind(phase)
    .execute(pool)
    .await?;
    let bytes = canonical_json_bytes(include_bytes!(
        "../../../../contracts/fixtures/assets/map-manifest.json"
    ))?;
    let release = CatalogRelease::new(parse_stored_release(&bytes)?, bytes)?;
    PgPlaceRepository::new(pool.clone())
        .publish_release(EventId::from_uuid(event), release)
        .await?;
    members(pool, event, &[owner, other]).await?;
    let place = Uuid::parse_str("00000000-0000-4000-8000-000000000001")?;
    Ok((event, owner, other, place))
}
fn fields(place: Uuid) -> PartyFields {
    let now = Utc::now();
    PartyFields {
        title: "After the encore".into(),
        description: "Meet by the east entrance".into(),
        ticket_section: Some("A".into()),
        capacity: 3,
        meeting_starts_at: now + Duration::hours(1),
        meeting_ends_at: now + Duration::hours(2),
        requirements: String::new(),
        public_place_id: place,
    }
}
fn create_binding(
    actor: Uuid,
    event: Uuid,
    fields: &PartyFields,
    key: Uuid,
) -> (Arc<PartyLiveBinding>, PartyLiveAction, Vec<u8>) {
    let body = serde_json::to_vec(&json!({"title":fields.title,"description":fields.description,"ticket_section":fields.ticket_section,"capacity":fields.capacity,"meeting_starts_at":fields.meeting_starts_at,"meeting_ends_at":fields.meeting_ends_at,"requirements":fields.requirements,"public_place_id":fields.public_place_id})).unwrap();
    (
        binding(
            actor,
            event,
            Uuid::new_v4(),
            "createConcertParty",
            format!("/api/concert/events/{event}/parties"),
            &body,
            Some(key),
        ),
        PartyLiveAction::Create(fields.clone()),
        body,
    )
}

#[tokio::test]
#[ignore = "requires marker-owned PostgreSQL fixture"]
async fn party_live_native() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let (event, owner, other, place) = event(&pool, "OPEN").await?;
    let community = PgCommunity::new(
        pool.clone(),
        TestCommunityPolicy {
            policy: PgRelationshipPolicy::new(pool.clone(), TestOwnerFence),
            require_registration: false,
        },
    );
    let seed = concert_application::community::PartyCommand {
        actor: owner,
        event,
        key: Uuid::new_v4(),
        party: Uuid::nil(),
        action: concert_domain::community::party::PartyAction::Create(fields(place)),
    };
    let seed_id = seed.party_id();
    let community_keys = keys();
    let seed_receipt = community
        .execute_party(&seed, &community_keys)
        .await
        .map_err(|e| anyhow::anyhow!("seed {e:?}"))?;
    anyhow::ensure!(
        seed_receipt.state == ReceiptState::Succeeded,
        "seed create failed"
    );
    let list = service(pool.clone(), false, false);
    let list_binding = binding(
        other,
        event,
        Uuid::new_v4(),
        "listConcertParties",
        format!("/api/concert/events/{event}/parties?limit=1"),
        &[],
        None,
    );
    let list_reply = request(
        &list,
        list_binding,
        PartyLiveAction::List(PartyQuery {
            limit: Some(1),
            ..Default::default()
        }),
    )
    .await
    .context("Party native list_reply")?;
    anyhow::ensure!(
        list_reply.status == 200 && list_reply.body["items"].as_array().unwrap().len() == 1,
        "list failed"
    );
    anyhow::ensure!(
        list_reply.body["items"][0]["owner"]["display_name"] == "Owner",
        "owner projection missing"
    );
    let detail_binding = binding(
        other,
        event,
        Uuid::new_v4(),
        "getConcertParty",
        format!("/api/concert/events/{event}/parties/{seed_id}"),
        &[],
        None,
    );
    let detail = request(&list, detail_binding, PartyLiveAction::Detail(seed_id))
        .await
        .context("Party native detail")?;
    anyhow::ensure!(
        detail.status == 200 && detail.body["party_id"] == seed_id.to_string(),
        "detail failed"
    );
    let apply = request(
        &service(pool.clone(), false, false),
        binding(
            other,
            event,
            Uuid::new_v4(),
            "applyConcertParty",
            format!("/api/concert/events/{event}/parties/{seed_id}/applications"),
            br#"{"party_revision":1,"note":"Save me a place"}"#,
            Some(Uuid::new_v4()),
        ),
        PartyLiveAction::Apply {
            party: seed_id,
            party_revision: 1,
            note: "Save me a place".into(),
        },
    )
    .await
    .context("Party native apply")?;
    anyhow::ensure!(
        apply.status == 200
            && apply.body["state"] == "PENDING"
            && apply.body["applicant"]["user_id"] == other.to_string(),
        "apply failed"
    );
    let application = Uuid::parse_str(apply.body["application_id"].as_str().unwrap())?;
    let decision = request(
        &service(pool.clone(), false, false),
        binding(
            owner,
            event,
            Uuid::new_v4(),
            "decideConcertApplication",
            format!("/api/concert/events/{event}/party-applications/{application}/decision"),
            br#"{"expected_revision":1,"party_revision":1,"decision":"APPROVED"}"#,
            Some(Uuid::new_v4()),
        ),
        PartyLiveAction::Decide {
            application,
            expected_revision: 1,
            party_revision: 1,
            approve: true,
        },
    )
    .await
    .context("Party native decision")?;
    anyhow::ensure!(
        decision.status == 200 && decision.body["state"] == "APPROVED",
        "decision failed"
    );
    let removal = request(
        &service(pool.clone(), false, false),
        binding(
            other,
            event,
            Uuid::new_v4(),
            "removeConcertMember",
            format!(
                "/api/concert/events/{event}/parties/{seed_id}/members/{other}?expected_revision=1&party_revision=2"
            ),
            &[],
            Some(Uuid::new_v4()),
        ),
        PartyLiveAction::Remove {
            party: seed_id,
            user: other,
            expected_revision: 1,
            party_revision: 2,
        },
    )
    .await.context("Party native removal")?;
    anyhow::ensure!(
        removal.status == 200 && removal.body["state"] == "LEFT",
        "leave failed"
    );

    let create = service(pool.clone(), false, false);
    let new_fields = fields(place);
    let key = Uuid::new_v4();
    let (create_binding_value, action, _) = create_binding(owner, event, &new_fields, key);
    let create_reply = request(&create, create_binding_value, action)
        .await
        .context("Party native create_reply")?;
    anyhow::ensure!(create_reply.status == 200, "create failed");
    let party_id = Uuid::parse_str(create_reply.body["party_id"].as_str().unwrap())?;
    let counts: (i64,i64,i64,i64) = sqlx::query_as("select (select count(*) from concert_command_receipts where event_id=$1 and operation_id='createConcertParty'),(select count(*) from concert_outbox where event_id=$1 and resource_id=$2),(select count(*) from concert_community_parties where event_id=$1),(select count(*) from concert_community_members where event_id=$1)").bind(event).bind(party_id).fetch_one(&pool).await?;
    anyhow::ensure!(
        counts.0 == 2 && counts.1 == 1 && counts.2 == 2,
        "create effects not atomic"
    );
    anyhow::ensure!(
        counts.3 == 3,
        "owner membership was not inserted with the root"
    );
    let replay = request(
        &create,
        create_binding(owner, event, &new_fields, key).0,
        create_binding(owner, event, &new_fields, key).1,
    )
    .await
    .context("Party native replay")?;
    anyhow::ensure!(replay.status == 200, "replay failed");
    let replayed_counts: (i64,i64,i64) = sqlx::query_as("select (select count(*) from concert_command_receipts where event_id=$1 and operation_id='createConcertParty'),(select count(*) from concert_outbox where event_id=$1 and resource_id=$2),(select count(*) from concert_community_parties where event_id=$1)").bind(event).bind(party_id).fetch_one(&pool).await?;
    anyhow::ensure!(
        replayed_counts.0 == counts.0
            && replayed_counts.1 == counts.1
            && replayed_counts.2 == counts.2,
        "replay repeated effects"
    );
    let blocked_placeholder = Uuid::new_v4();
    sqlx::query("update party_live_authority set blocked=array[$1] where event_id=$2")
        .bind(owner)
        .bind(event)
        .execute(&pool)
        .await?;
    let failure = service(pool.clone(), false, false);
    let failed = request(
        &failure,
        binding(
            other,
            event,
            Uuid::new_v4(),
            "listConcertParties",
            format!("/api/concert/events/{event}/parties"),
            &[],
            None,
        ),
        PartyLiveAction::List(PartyQuery::default()),
    )
    .await?;
    anyhow::ensure!(
        failed.body["items"].as_array().unwrap().is_empty(),
        "blocked owner visible"
    );
    sqlx::query("update party_live_authority set blocked='{}' where event_id=$1")
        .bind(event)
        .execute(&pool)
        .await?;
    let profile_failure = service(pool.clone(), true, false);
    let denied = request(
        &profile_failure,
        binding(
            other,
            event,
            Uuid::new_v4(),
            "listConcertParties",
            format!("/api/concert/events/{event}/parties"),
            &[],
            None,
        ),
        PartyLiveAction::List(PartyQuery::default()),
    )
    .await;
    anyhow::ensure!(
        denied == Err(PartyLiveError::DependencyUnavailable),
        "profile failure must fail closed"
    );
    let track_failure = service(pool.clone(), false, true);
    let create_fields = fields(place);
    let track_binding = create_binding(owner, event, &create_fields, Uuid::new_v4());
    let denied_create = request(&track_failure, track_binding.0, track_binding.1).await;
    anyhow::ensure!(
        denied_create == Err(PartyLiveError::DependencyUnavailable),
        "guard failure must not write"
    );
    let after_track_failure: i64 =
        sqlx::query_scalar("select count(*) from concert_command_receipts where event_id=$1")
            .bind(event)
            .fetch_one(&pool)
            .await?;
    anyhow::ensure!(
        after_track_failure == 5,
        "rejected transaction left a receipt"
    );

    let foreign = Uuid::new_v4();
    sqlx::query(
        "insert into concert_events(event_id,slug,title,phase) values($1,$2,'Other','OPEN')",
    )
    .bind(foreign)
    .bind(format!("other-{foreign}"))
    .execute(&pool)
    .await?;
    let cross_binding = binding(
        other,
        event,
        Uuid::new_v4(),
        "getConcertParty",
        format!("/api/concert/events/{event}/parties/{foreign}"),
        &[],
        None,
    );
    let cross = request(
        &service(pool.clone(), false, false),
        cross_binding,
        PartyLiveAction::Detail(foreign),
    )
    .await;
    anyhow::ensure!(
        cross == Err(PartyLiveError::ResourceUnavailable),
        "cross-event detail leaked"
    );
    let _ = (blocked_placeholder, party_id);
    let count: i64 = sqlx::query_scalar(
        "select count(*) from concert_command_receipts where event_id=$1 and state='SUCCEEDED'",
    )
    .bind(event)
    .fetch_one(&pool)
    .await?;
    anyhow::ensure!(count == 5, "unexpected receipt state");
    party_live_support::mutation_invariants(&pool).await?;
    Ok(())
}
