//! Synthetic native fixtures only. These are not venue or production policy evidence.
use async_trait::async_trait;
use concert_application::{beacon::*, place::PlaceRepository};
use concert_domain::{
    beacon::*,
    place::{canonical_json_bytes, parse_stored_release, CatalogRelease},
    EventId,
};
use concert_service::{
    adapters::out::repository::place::PgPlaceRepository, meeting::MeetingAuthority,
};
use sha2::{Digest, Sha256};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgConnection, PgPool, Row,
};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    str::FromStr,
    time::Duration,
};
use uuid::Uuid;

pub async fn owned_pool() -> anyhow::Result<PgPool> {
    let root = PathBuf::from(std::env::var("MEETING_TEST_OWNER_ROOT")?);
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        info.is_dir() && !info.file_type().is_symlink() && info.permissions().mode() & 0o077 == 0,
        "private fixture root"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == std::env::var("MEETING_TEST_OWNER_IDENTITY")?,
        "fixture identity"
    );
    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("mcr-owner.json"))?)?;
    anyhow::ensure!(
        marker["token"].as_str() == Some(std::env::var("MEETING_TEST_OWNER_TOKEN")?.as_str()),
        "fixture marker"
    );
    let options = PgConnectOptions::from_str(&std::env::var("MEETING_TEST_DATABASE_URL")?)?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_meeting")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "foreign database"
    );
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .acquire_timeout(Duration::from_secs(3))
        .connect_with(options)
        .await?;
    let actual: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        actual == root.join("data").to_string_lossy(),
        "foreign data directory"
    );
    Ok(pool)
}
#[derive(Clone, Copy)]
pub struct SyntheticAuthority;
#[async_trait]
impl MeetingAuthority for SyntheticAuthority {
    async fn actor_in(&self, c: &mut PgConnection, a: MeetingActor) -> Result<(), MeetingError> {
        let row = sqlx::query("select * from meeting_fixture_authority where event=$1 for share")
            .bind(a.event)
            .fetch_optional(c)
            .await
            .map_err(|_| MeetingError::PolicyUnavailable)?
            .ok_or(MeetingError::ResourceUnavailable)?;
        if row.get::<bool, _>("unavailable") {
            return Err(MeetingError::PolicyUnavailable);
        }
        let members: Vec<Uuid> = row.get("members");
        if !members.contains(&a.actor) {
            return Err(MeetingError::ResourceUnavailable);
        }
        Ok(())
    }
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        q: AudienceQuery,
    ) -> Result<AudienceGrant, MeetingError> {
        self.actor_in(c, q.actor).await?;
        let row = sqlx::query("select * from meeting_fixture_authority where event=$1 for share")
            .bind(q.actor.event)
            .fetch_one(c)
            .await
            .map_err(|_| MeetingError::PolicyUnavailable)?;
        let members: Vec<Uuid> = row.get("members");
        if !members.contains(&q.owner) {
            return Err(MeetingError::ResourceUnavailable);
        }
        if q.operation == MeetingOperation::Revoke {
            return Ok(AudienceGrant {
                query: q,
                party: None,
            });
        }
        let blocked: Vec<Uuid> = row.get("blocked");
        if blocked.contains(&q.actor.actor) || blocked.contains(&q.owner) {
            return Err(MeetingError::ResourceUnavailable);
        }
        match &q.audience {
            Audience::PartyMembers { party_id } if *party_id == row.get::<Uuid, _>("party") => {}
            Audience::ExplicitUsers { user_ids } => {
                if matches!(
                    q.operation,
                    MeetingOperation::Create | MeetingOperation::Update
                ) {
                    if user_ids
                        .iter()
                        .any(|id| !members.contains(id) || blocked.contains(id))
                    {
                        return Err(MeetingError::ResourceUnavailable);
                    }
                } else if q.actor.actor != q.owner && !user_ids.contains(&q.actor.actor) {
                    return Err(MeetingError::ResourceUnavailable);
                }
            }
            _ => return Err(MeetingError::ResourceUnavailable),
        }
        let party = match &q.audience {
            Audience::PartyMembers { party_id } => Some(PartyFence {
                party: *party_id,
                meeting_revision: row.get("meeting_revision"),
                audience_revision: row.get("audience_revision"),
            }),
            _ => None,
        };
        Ok(AudienceGrant { query: q, party })
    }
}
pub struct Fixture {
    pub event: Uuid,
    pub owner: Uuid,
    pub peer: Uuid,
    pub party: Uuid,
    pub meeting: MeetingDescription,
}
impl Fixture {
    pub fn actor(&self, id: Uuid) -> MeetingActor {
        MeetingActor {
            event: self.event,
            actor: id,
        }
    }
}
pub async fn setup(pool: &PgPool) -> anyhow::Result<Fixture> {
    let event = Uuid::new_v4();
    let owner = Uuid::new_v4();
    let peer = Uuid::new_v4();
    let party = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Synthetic meeting fixture','OPEN')").bind(event).bind(format!("meeting-{}",event.simple())).execute(pool).await?;
    sqlx::query("insert into concert_meeting_settings(event_id,event_cutoff) values($1,clock_timestamp()+interval '1 day')").bind(event).execute(pool).await?;
    sqlx::query("create table if not exists meeting_fixture_authority(event uuid primary key,party uuid not null,members uuid[] not null,blocked uuid[] not null default '{}',unavailable boolean not null default false,owner uuid not null,public_place uuid,meeting_revision bigint not null default 1,audience_revision bigint not null default 1,member_generation bigint not null default 1,ends_at timestamptz not null default clock_timestamp()+interval '1 hour')").execute(pool).await?;
    sqlx::query(
        "insert into meeting_fixture_authority(event,party,members,owner) values($1,$2,$3,$4)",
    )
    .bind(event)
    .bind(party)
    .bind(vec![owner, peer])
    .bind(owner)
    .execute(pool)
    .await?;
    let mut value: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../../../contracts/fixtures/assets/map-manifest.json"
    ))?;
    value["map_id"] = format!("meeting_{}", event.simple()).into();
    let place = Uuid::new_v4();
    value["places"][0]["place_id"] = place.to_string().into();
    value["geometry"]["interactions"][0]["place_id"] = place.to_string().into();
    value["places"][0]["place_kind"] = "PHYSICAL".into();
    value["places"][0]["verification_state"] = "VERIFIED".into();
    value["places"][0]["provenance"] = serde_json::json!({"source_id":"synthetic_meeting","source_kind":"PLACE_REFERENCE","source_revision":"native-test-only-v1","source_uri":"https://meeting-fixture.invalid/synthetic","author":"MCR synthetic test","license":"Test only","review_state":"REVIEWED","reviewed_by":"synthetic-fixture-review","reviewed_at":"2026-11-04T00:00:00Z","photo_permission":"NOT_APPLICABLE"});
    let geometry = canonical_json_bytes(&serde_json::to_vec(&value["geometry"])?)?;
    value["geometry_hash"] = format!("sha256:{:x}", Sha256::digest(&geometry)).into();
    value.as_object_mut().unwrap().remove("content_hash");
    let content = canonical_json_bytes(&serde_json::to_vec(&value)?)?;
    value["content_hash"] = format!("sha256:{:x}", Sha256::digest(&content)).into();
    let bytes = canonical_json_bytes(&serde_json::to_vec(&value)?)?;
    let release = CatalogRelease::new(parse_stored_release(&bytes)?, bytes)?;
    PgPlaceRepository::new(pool.clone())
        .publish_release(EventId::from_uuid(event), release)
        .await?;
    sqlx::query("update meeting_fixture_authority set public_place=$2 where event=$1")
        .bind(event)
        .bind(place)
        .execute(pool)
        .await?;
    let expires = sqlx::query_scalar("select clock_timestamp()+interval '120 seconds'")
        .fetch_one(pool)
        .await?;
    Ok(Fixture {
        event,
        owner,
        peer,
        party,
        meeting: MeetingDescription {
            place_id: place,
            map_revision: 1,
            place_revision: 1,
            instructions: "Synthetic guidance only".into(),
            identification: "Synthetic test actor".into(),
            audience: Audience::ExplicitUsers {
                user_ids: vec![peer],
            },
            expires_at: expires,
        },
    })
}

#[async_trait]
impl concert_service::meeting::rally::RallyAuthority for SyntheticAuthority {
    async fn party_in(
        &self,
        c: &mut PgConnection,
        q: concert_application::rally::RallyQuery,
    ) -> Result<concert_application::rally::PartyMeeting, MeetingError> {
        use concert_application::rally::*;
        self.actor_in(c, q.actor).await?;
        let row = sqlx::query("select * from meeting_fixture_authority where event=$1 for share")
            .bind(q.actor.event)
            .fetch_one(c)
            .await
            .map_err(|_| MeetingError::PolicyUnavailable)?;
        if row.get::<Uuid, _>("party") != q.party {
            return Err(MeetingError::ResourceUnavailable);
        }
        Ok(PartyMeeting {
            query: q,
            owner: row.get("owner"),
            meeting_revision: row.get("meeting_revision"),
            audience_revision: 1,
            public_place_id: row.get("public_place"),
            ends_at: row.get("ends_at"),
            members: row
                .get::<Vec<Uuid>, _>("members")
                .into_iter()
                .map(|user| RallyMember {
                    user,
                    membership_revision: row.get("member_generation"),
                })
                .collect(),
        })
    }
    async fn peer_in(
        &self,
        c: &mut PgConnection,
        q: concert_application::rally::RallyQuery,
        peer: Uuid,
    ) -> Result<(), MeetingError> {
        self.actor_in(c, q.actor).await?;
        let row = sqlx::query("select * from meeting_fixture_authority where event=$1 for share")
            .bind(q.actor.event)
            .fetch_one(c)
            .await
            .map_err(|_| MeetingError::PolicyUnavailable)?;
        if row.get::<Uuid, _>("party") != q.party
            || !row.get::<Vec<Uuid>, _>("members").contains(&peer)
            || row.get::<Vec<Uuid>, _>("blocked").contains(&peer)
            || row.get::<Vec<Uuid>, _>("blocked").contains(&q.actor.actor)
        {
            return Err(MeetingError::ResourceUnavailable);
        }
        Ok(())
    }
}
