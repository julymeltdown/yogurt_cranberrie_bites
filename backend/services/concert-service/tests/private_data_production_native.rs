//! BE42 production PrivateData composition against one owned PostgreSQL.
//!
//! Every category owner is built by `PrivateDataRuntimeOwners::production`, so
//! this test exercises the actual factory the serving runtime composes. The
//! test-only authority below authenticates exact command/source bindings; it is
//! not a production Auth transport. A whole PrivateData acknowledgement exists
//! only when all seven category receipts are durable complete.
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_domain::community::party::{Party, PartyFields};
use concert_service::{
    account_private_data::PrivateDataCategory,
    config::private_data_runtime::{production_coordinator, PrivateDataRuntimeOwners},
    world::account_deletion::{
        DeletionAuthority, DeletionFailure, VerifiedDeletionSource, WorldDeletionCommand,
        WorldDeletionTarget,
    },
};
use sha2::{Digest, Sha256};
use sqlx::{
    migrate::Migrator,
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::{path::Path, str::FromStr, sync::Arc, time::Duration};
use tokio::time::Instant;
use uuid::Uuid;

const DATABASE: &str = "mcr_private_data_production";
const USER: Uuid = Uuid::from_u128(0xbe42_1001);
const OTHER: Uuid = Uuid::from_u128(0xbe42_1002);
const EVENT: Uuid = Uuid::from_u128(0xbe42_1101);

struct TestAuthority {
    salt: u8,
}
#[async_trait]
impl DeletionAuthority for TestAuthority {
    async fn verify(
        &self,
        c: &WorldDeletionCommand,
        d: Instant,
    ) -> Result<VerifiedDeletionSource, DeletionFailure> {
        let mut h = Sha256::new();
        h.update(b"BE42-PRODUCTION-COMPOSITION-NATIVE-v1\0");
        h.update([self.salt]);
        h.update(c.binding()?);
        VerifiedDeletionSource::from_verified_auth_source(c, d, h.finalize().into())
    }
}

fn command() -> WorldDeletionCommand {
    WorldDeletionCommand {
        saga_id: Uuid::from_u128(0xbe42_1201),
        user_id: USER,
        actor_id: USER,
        target: WorldDeletionTarget::PrivateData,
        retention_until: "2070-01-01T00:00:00.123456000Z".parse().unwrap(),
    }
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(4)
}

async fn owned_pool() -> Result<PgPool> {
    let options = PgConnectOptions::from_str(&std::env::var("CONCERT_TEST_DATABASE_URL")?)?;
    anyhow::ensure!(
        options.get_database() == Some(DATABASE),
        "foreign database refused"
    );
    let p = PgPoolOptions::new()
        .max_connections(10)
        .connect_with(options)
        .await?;
    let actual: String = sqlx::query_scalar("select current_database()")
        .fetch_one(&p)
        .await?;
    anyhow::ensure!(actual == DATABASE, "foreign database refused");
    Migrator::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations"))
        .await?
        .run(&p)
        .await?;
    Ok(p)
}

async fn seed(p: &PgPool) -> Result<()> {
    sqlx::query(
        "insert into concert_events(event_id,slug,title,phase) values($1,'be42-production-composition','Private data production','OPEN')",
    )
    .bind(EVENT)
    .execute(p)
    .await?;
    for user in [USER, OTHER] {
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'DANGER',true,true,true)")
            .bind(EVENT)
            .bind(user)
            .execute(p)
            .await?;
    }
    sqlx::query("insert into concert_avatar_settings(event_id,user_id,contract_version,representation,preset_id,manifest_id,manifest_version) values($1,$2,'1.1.0','whole_character','photo01_front_01','mcr_selected_static_20261103',1)")
        .bind(EVENT)
        .bind(USER)
        .execute(p)
        .await?;
    sqlx::query("insert into concert_age_owners(user_id) values($1)")
        .bind(USER)
        .execute(p)
        .await?;
    sqlx::query("insert into concert_age_verifications(verification_id,user_id,event_id,session_id,verifier_id,source_revision,state_digest,return_target,created_at,expires_at,state) values($1,$2,$3,$4,$5,$6,decode(repeat('ab',32),'hex'),'AGE_PRIVACY',clock_timestamp(),clock_timestamp()+interval '5 minutes','PENDING')")
        .bind(Uuid::from_u128(0xbe42_1401))
        .bind(USER)
        .bind(EVENT)
        .bind(Uuid::from_u128(0xbe42_1402))
        .bind("be42-native-v1")
        .bind("be42-native-v1")
        .execute(p)
        .await?;
    sqlx::query("insert into concert_age_claims(claim_id,user_id,event_id,verifier_id,source_revision,status,verified_at,expires_at,age_at_verification,next_birthday_at) values($1,$2,$3,'be42-native-v1','be42-native-v1','VERIFIED',clock_timestamp()-interval '1 hour',clock_timestamp()+interval '1 day',25,(clock_timestamp() at time zone 'Asia/Seoul')::date+1)")
        .bind(Uuid::from_u128(0xbe42_1401))
        .bind(USER)
        .bind(EVENT)
        .execute(p)
        .await?;
    sqlx::query("insert into concert_media(media_id,event_id,owner_id,purpose,content_type,declared_bytes,input_sha256,policy_event_revision,policy_relationship_revision,policy_authority_revision,state,staged) values($1,$2,$3,'POST','image/png',3,decode(repeat('a1',32),'hex'),1,1,1,'UPLOADED',decode('010203','hex'))")
        .bind(Uuid::from_u128(0xbe42_1501))
        .bind(EVENT)
        .bind(USER)
        .execute(p)
        .await?;
    sqlx::query("insert into concert_meeting_settings(event_id,event_cutoff) values($1,clock_timestamp()+interval '1 day')")
        .bind(EVENT)
        .execute(p)
        .await?;
    let now = Utc::now();
    let fields = PartyFields {
        title: "Private production Party".into(),
        description: "Private fixture description".into(),
        ticket_section: None,
        capacity: 2,
        meeting_starts_at: now + chrono::Duration::hours(1),
        meeting_ends_at: now + chrono::Duration::hours(2),
        requirements: "fixture".into(),
        public_place_id: Uuid::from_u128(0xbe42_1602),
    };
    let party = Party::create(Uuid::from_u128(0xbe42_1601), EVENT, USER, fields, now)?;
    sqlx::query("insert into concert_community_parties(event_id,party_id,owner_id,revision,state,member_count,capacity,meeting_revision,audience_revision,aggregate) values($1,$2,$3,$4,'OPEN',$5,$6,$7,$8,$9)")
        .bind(EVENT)
        .bind(party.id)
        .bind(USER)
        .bind(party.revision)
        .bind(party.member_count())
        .bind(party.fields.capacity)
        .bind(party.meeting_revision)
        .bind(party.audience_revision)
        .bind(serde_json::to_value(&party)?)
        .execute(p)
        .await?;
    sqlx::query("insert into concert_meeting_beacons(beacon_id,event_id,owner_id,place_id,map_revision,place_revision,selected_catalog_revision,instructions,identification,audience,expires_at) values($1,$2,$3,$4,1,1,1,'Private meeting guidance','Private appearance',jsonb_build_object('kind','EXPLICIT_USERS','user_ids',jsonb_build_array($5::text)),clock_timestamp()+interval '10 minutes')")
        .bind(Uuid::from_u128(0xbe42_1701))
        .bind(EVENT)
        .bind(USER)
        .bind(Uuid::from_u128(0xbe42_1702))
        .bind(OTHER.to_string())
        .execute(p)
        .await?;
    sqlx::query("insert into concert_mural_state(event_id,snapshot_id,state,pixels,palette,cooldown_ms) values($1,$2,'EDITABLE',$3,$4,1000)")
        .bind(EVENT)
        .bind(Uuid::from_u128(0xbe42_1801))
        .bind(vec![0u8; 16384])
        .bind(vec!["#123ABC"; 16])
        .execute(p)
        .await?;
    sqlx::query("insert into concert_mural_limits(event_id,actor_id,next_allowed_at) values($1,$2,clock_timestamp()+interval '1 hour')")
        .bind(EVENT)
        .bind(USER)
        .execute(p)
        .await?;
    Ok(())
}

/// Intro erasure reads the actual Sharing owner's terminal evidence on the
/// common fence. The Sharing target runs before PrivateData in the canonical
/// saga, so this fixture records that terminal row only after the aggregate has
/// established the fence in its first (Core) round.
async fn sharing_terminal(p: &PgPool, c: &WorldDeletionCommand) -> Result<()> {
    let mut sharing = c.clone();
    sharing.target = WorldDeletionTarget::Sharing;
    let source = [0x42u8; 32];
    sqlx::query("insert into concert_social_account_deletions(saga_id,user_id,actor_id,retention_seconds,retention_nanos,identity_binding,command_binding,source_binding,fence_revision) values($1,$2,$3,$4,$5,$6,$7,$8,1)")
        .bind(c.saga_id)
        .bind(c.user_id)
        .bind(c.actor_id)
        .bind(c.retention_until.timestamp())
        .bind(c.retention_until.timestamp_subsec_nanos() as i32)
        .bind(sharing.identity_binding().map_err(|e| anyhow::anyhow!("{e:?}"))?.as_slice())
        .bind(sharing.binding().map_err(|e| anyhow::anyhow!("{e:?}"))?.as_slice())
        .bind(source.as_slice())
        .execute(p)
        .await?;
    let hex: String = source.iter().map(|x| format!("{x:02x}")).collect();
    let mut tx = p.begin().await?;
    sqlx::query("select set_config('mcr.social_deletion_saga',$1,true),set_config('mcr.social_deletion_source',$2,true),set_config('mcr.social_deletion_until',(clock_timestamp()+interval '4 seconds')::text,true)")
        .bind(c.saga_id.to_string())
        .bind(hex)
        .execute(&mut *tx)
        .await?;
    sqlx::query("update concert_social_account_deletions set completed_at=clock_timestamp(),progress_revision=progress_revision+1 where saga_id=$1 and completed_at is null")
        .bind(c.saga_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// No affected table may retain a row belonging to the deleted identity.
async fn no_orphans(p: &PgPool) -> Result<()> {
    let checks = [
        (
            "core participant",
            "select count(*) from concert_participants where user_id=$1 and (theme<>'PARADE' or low_power or reduced_motion or sound_enabled)",
        ),
        (
            "core avatar",
            "select count(*) from concert_avatar_settings where user_id=$1",
        ),
        (
            "intro",
            "select count(*) from concert_intros where owner_id=$1",
        ),
        (
            "age",
            "select (select count(*) from concert_age_verifications where user_id=$1)+(select count(*) from concert_age_claims where user_id=$1)+(select count(*) from concert_age_owners where user_id=$1)",
        ),
        (
            "media",
            "select count(*) from concert_media where owner_id=$1 and (state<>'DELETED' or staged is not null or sanitized is not null or width is not null or height is not null or target_id is not null)",
        ),
        (
            "party",
            "select case when concert_party_has_private_input($1) then 1::bigint else 0::bigint end",
        ),
        (
            "meeting",
            "select case when concert_meeting_has_private_input($1) then 1::bigint else 0::bigint end",
        ),
        (
            "mural limits",
            "select count(*) from concert_mural_limits where actor_id=$1",
        ),
        (
            "mural operations",
            "select count(*) from concert_mural_operations where actor_id=$1",
        ),
    ];
    for (name, sql) in checks {
        let remaining: i64 = sqlx::query_scalar(sql).bind(USER).fetch_one(p).await?;
        anyhow::ensure!(remaining == 0, "{name} still has {remaining} private rows");
    }
    let other: String = sqlx::query_scalar(
        "select theme from concert_participants where event_id=$1 and user_id=$2",
    )
    .bind(EVENT)
    .bind(OTHER)
    .fetch_one(p)
    .await?;
    anyhow::ensure!(other == "DANGER", "another user's row was altered");
    Ok(())
}

#[tokio::test]
#[ignore = "requires marker-owned PostgreSQL fixture"]
async fn production_private_data_converges_and_is_idempotent() -> Result<()> {
    let p = owned_pool().await?;
    anyhow::ensure!(
        !sqlx::query_scalar::<_, bool>(
            "select exists(select 1 from concert_private_data_owner_progress where user_id=$1)"
        )
        .bind(USER)
        .fetch_one(&p)
        .await?,
        "fixture database is not fresh"
    );
    seed(&p).await?;
    let c = command();
    let authority: Arc<dyn DeletionAuthority> = Arc::new(TestAuthority { salt: 7 });
    let owners = PrivateDataRuntimeOwners::production(p.clone(), authority.clone());
    anyhow::ensure!(
        owners.is_complete(),
        "production factory did not register all seven categories"
    );
    anyhow::ensure!(
        owners.registered_categories() == PrivateDataCategory::ALL.to_vec(),
        "production categories are incomplete or out of order"
    );
    let coordinator = production_coordinator(p.clone(), authority);

    let first = coordinator.apply(c.clone(), deadline()).await;
    anyhow::ensure!(
        matches!(first, Err(DeletionFailure::Incomplete)),
        "first round must stop incomplete: {first:?}"
    );
    let fence_count: i64 = sqlx::query_scalar(
        "select count(*) from concert_world_account_deletion_fences where user_id=$1 and saga_id=$2",
    )
    .bind(USER)
    .bind(c.saga_id)
    .fetch_one(&p)
    .await?;
    anyhow::ensure!(fence_count == 1, "common fence was not established");

    sharing_terminal(&p, &c).await?;

    let mut ack = None;
    for round in 1..PrivateDataCategory::ALL.len() {
        let result = coordinator.apply(c.clone(), deadline()).await;
        let complete: i64 = sqlx::query_scalar(
            "select count(*) from concert_private_data_category_progress where user_id=$1 and saga_id=$2 and state='COMPLETE'",
        )
        .bind(USER)
        .bind(c.saga_id)
        .fetch_one(&p)
        .await?;
        anyhow::ensure!(
            complete == round as i64 + 1,
            "round {round} recorded {complete} terminal categories"
        );
        if round + 1 == PrivateDataCategory::ALL.len() {
            ack = Some(
                result.map_err(|e| anyhow::anyhow!("production terminal round: {e:?}"))?,
            );
        } else {
            anyhow::ensure!(
                matches!(result, Err(DeletionFailure::Incomplete)),
                "bounded round {round} did not stop incomplete: {result:?}"
            );
        }
    }
    let ack = ack.expect("whole acknowledgement");
    anyhow::ensure!(ack.verifies(&c), "whole acknowledgement did not verify");
    anyhow::ensure!(
        ack.completed_categories == PrivateDataCategory::ALL.len(),
        "whole acknowledgement covered {} categories",
        ack.completed_categories
    );
    no_orphans(&p).await?;
    let whole: Option<DateTime<Utc>> = sqlx::query_scalar(
        "select completed_at from concert_private_data_owner_progress where user_id=$1",
    )
    .bind(USER)
    .fetch_one(&p)
    .await?;
    let whole = whole.expect("durable whole receipt");
    let before: i64 = sqlx::query_scalar(
        "select count(*) from concert_private_data_category_progress where user_id=$1",
    )
    .bind(USER)
    .fetch_one(&p)
    .await?;

    let second = coordinator
        .apply(c.clone(), deadline())
        .await
        .map_err(|e| anyhow::anyhow!("second deletion request: {e:?}"))?;
    anyhow::ensure!(second.verifies(&c));
    anyhow::ensure!(second.fence_revision == ack.fence_revision);
    let whole_after: DateTime<Utc> = sqlx::query_scalar(
        "select completed_at from concert_private_data_owner_progress where user_id=$1",
    )
    .bind(USER)
    .fetch_one(&p)
    .await?;
    anyhow::ensure!(whole_after == whole, "terminal receipt was rewritten");
    let after: i64 = sqlx::query_scalar(
        "select count(*) from concert_private_data_category_progress where user_id=$1",
    )
    .bind(USER)
    .fetch_one(&p)
    .await?;
    anyhow::ensure!(after == before, "replay created category receipts");

    anyhow::ensure!(
        sqlx::query("insert into concert_mural_limits(event_id,actor_id,next_allowed_at) values($2,$1,clock_timestamp()+interval '1 hour')")
            .bind(USER)
            .bind(EVENT)
            .execute(&p)
            .await
            .is_err(),
        "Wall recreation bypassed the sealed fence"
    );
    p.close().await;
    Ok(())
}
