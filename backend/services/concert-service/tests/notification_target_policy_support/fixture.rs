//! Owned PG validation adapted read-only from gift_live_support/owned.rs at cde817b2.
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::{path::PathBuf, str::FromStr};
pub async fn owned_pool() -> anyhow::Result<PgPool> {
    let root = PathBuf::from(
        std::env::var("NB03_LOCAL_TEST_OWNER_ROOT")
            .map_err(|_| anyhow::anyhow!("owned fixture root required"))?,
    );
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        info.is_dir() && !info.file_type().is_symlink() && info.permissions().mode() & 0o077 == 0,
        "private owned root required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino())
            == std::env::var("NB03_LOCAL_TEST_OWNER_IDENTITY")?,
        "fixture inode changed"
    );
    let marker_path = root.join("mcr-owner.json");
    let marker_info = std::fs::symlink_metadata(&marker_path)?;
    anyhow::ensure!(
        marker_info.is_file()
            && !marker_info.file_type().is_symlink()
            && marker_info.permissions().mode() & 0o077 == 0,
        "private marker required"
    );
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(marker_path)?)?;
    anyhow::ensure!(
        marker["token"].as_str() == Some(std::env::var("NB03_LOCAL_TEST_OWNER_TOKEN")?.as_str())
            && marker["uid"].as_u64() == Some(info.uid() as u64),
        "fixture owner mismatch"
    );
    let options = PgConnectOptions::from_str(&std::env::var("NB03_LOCAL_TEST_DATABASE_URL")?)
        .map_err(|_| anyhow::anyhow!("invalid owned fixture options"))?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_notification_target_policy")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await
        .map_err(|_| anyhow::anyhow!("owned fixture connection failed"))?;
    let actual: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        actual == root.join("data").to_string_lossy(),
        "foreign data directory refused"
    );
    Ok(pool)
}

use chrono::{DateTime, Duration, Utc};
use concert_application::notification::{Candidate, Context};
use concert_domain::{
    community::{
        giveaway::{GiftFields, GiftState, Giveaway, Reservation, ReservationState},
        party::{Party, PartyAction, PartyFields},
    },
    notification::{Target, TargetType},
};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

pub struct Fixture {
    pub pool: PgPool,
    pub actor: Uuid,
    pub owner: Uuid,
    pub outsider: Uuid,
    pub event: Uuid,
    pub party: Uuid,
    pub application: Uuid,
    pub giveaway: Uuid,
    pub reservation: Uuid,
    pub social: Uuid,
    pub notice: Uuid,
}
impl Fixture {
    pub async fn new() -> anyhow::Result<Self> {
        let pool = owned_pool().await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        let f = Self {
            pool,
            actor: Uuid::new_v4(),
            owner: Uuid::new_v4(),
            outsider: Uuid::new_v4(),
            event: Uuid::new_v4(),
            party: Uuid::new_v4(),
            application: Uuid::new_v4(),
            giveaway: Uuid::new_v4(),
            reservation: Uuid::new_v4(),
            social: Uuid::new_v4(),
            notice: Uuid::new_v4(),
        };
        let mut tx = f.pool.begin().await?;
        let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *tx)
            .await?;
        sqlx::query("insert into public.concert_events(event_id,slug,title,phase) values($1,$2,'Local policy fixture','OPEN')").bind(f.event).bind(format!("nb03-{}",f.event.simple())).execute(&mut *tx).await?;
        for user in [f.actor, f.owner, f.outsider] {
            sqlx::query("insert into public.concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')").bind(f.event).bind(user).execute(&mut *tx).await?;
            sqlx::query("insert into public.concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(f.event).bind(user).execute(&mut *tx).await?;
        }
        let mut party = Party::create(
            f.party,
            f.event,
            f.owner,
            PartyFields {
                title: "Fixture party".into(),
                description: String::new(),
                ticket_section: None,
                capacity: 3,
                meeting_starts_at: now + Duration::hours(1),
                meeting_ends_at: now + Duration::hours(2),
                requirements: String::new(),
                public_place_id: Uuid::new_v4(),
            },
            now,
        )?;
        party.change(
            f.actor,
            &PartyAction::Apply {
                party_revision: 1,
                note: String::new(),
            },
            f.application,
            now,
        )?;
        sqlx::query("insert into public.concert_community_parties(event_id,party_id,owner_id,revision,state,member_count,capacity,meeting_revision,audience_revision,aggregate) values($1,$2,$3,$4,'OPEN',$5,3,$6,$7,$8)")
            .bind(f.event).bind(f.party).bind(f.owner).bind(party.revision).bind(party.member_count()).bind(party.meeting_revision).bind(party.audience_revision).bind(serde_json::to_value(&party)?).execute(&mut *tx).await?;
        sqlx::query("insert into public.concert_community_applications(event_id,party_id,application_id,user_id,state,revision) values($1,$2,$3,$4,'PENDING',1)").bind(f.event).bind(f.party).bind(f.application).bind(f.actor).execute(&mut *tx).await?;
        let mut gift = Giveaway::create(
            f.giveaway,
            f.event,
            f.owner,
            GiftFields {
                title: "Fixture gift".into(),
                description: String::new(),
                image_media_id: None,
                quantity_total: 3,
                pickup_starts_at: now + Duration::hours(1),
                pickup_ends_at: now + Duration::hours(2),
                public_place_id: Uuid::new_v4(),
            },
            0,
            now,
        )?;
        // Private metadata, not a pickup command or authority. No meeting/code.
        gift.state = GiftState::Open;
        gift.reserved_unclaimed = 1;
        gift.reservations.push(Reservation {
            id: f.reservation,
            recipient: f.actor,
            state: ReservationState::Held,
            revision: 1,
            expires_at: gift.expires_at(),
            created_at: now,
            claim_receipt_id: None,
            claimed_at: None,
            code_expires_at: None,
            code_generation: 0,
        });
        gift.validate()?;
        sqlx::query("insert into public.concert_community_giveaways(event_id,giveaway_id,owner_id,revision,state,quantity_total,reserved_unclaimed,quantity_claimed,aggregate) values($1,$2,$3,1,'OPEN',3,1,0,$4)").bind(f.event).bind(f.giveaway).bind(f.owner).bind(serde_json::to_value(&gift)?).execute(&mut *tx).await?;
        sqlx::query("insert into public.concert_community_reservations(event_id,giveaway_id,reservation_id,recipient_id,state,revision,expires_at) values($1,$2,$3,$4,'HELD',1,$5)").bind(f.event).bind(f.giveaway).bind(f.reservation).bind(f.actor).bind(gift.expires_at()).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(f)
    }
    /// Protected fixture rows remain UNCOMMITTED inside the caller transaction.
    /// No authority GUC, disabled trigger, positive provider or peer-store write.
    /// Roll back after observation: this proves SQL policy, never owner effects.
    pub async fn prepare(&self, c: &mut PgConnection, expired_social: bool) -> anyhow::Result<()> {
        sqlx::query("insert into public.concert_event_settings(event_id,parties,giveaways,intros,social,mural,beacon_ttl_seconds,pickup_grace_seconds,social_grant_ttl_seconds,mural_cooldown_ms) values($1,true,true,true,true,true,60,0,60,1000)").bind(self.event).execute(&mut *c).await?;
        let mut users = vec![self.actor, self.owner, self.outsider];
        users.sort();
        for (n, low) in users.iter().enumerate() {
            for high in users.iter().skip(n + 1) {
                sqlx::query("insert into public.concert_relationship_pairs(event_id,low_user,high_user) values($1,$2,$3)").bind(self.event).bind(low).bind(high).execute(&mut *c).await?;
            }
        }
        sqlx::query("insert into public.concert_operation_notices(notice_id,event_id,title,text,source_uri,confirmed_at,state) values($1,$2,'Fixture notice','Fixture text','https://example.invalid/fixture',statement_timestamp(),'PUBLISHED')").bind(self.notice).bind(self.event).execute(&mut *c).await?;
        let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *c)
            .await?;
        sqlx::query("insert into public.concert_social_requests(event_id,request_id,requester_id,recipient_id,intro_id,intro_revision,authority_revision,relationship_revision,state,created_at,updated_at,expires_at) values($1,$2,$3,$4,$5,1,1,1,'PENDING',$6,$7,$8)")
            .bind(self.event).bind(self.social).bind(self.actor).bind(self.owner).bind(Uuid::new_v4()).bind(now-Duration::hours(2)).bind(now).bind(if expired_social{now-Duration::hours(1)}else{now+Duration::hours(1)}).execute(&mut *c).await?;
        sqlx::query("insert into public.concert_social_bindings(event_id,request_id,platform,requester_version,recipient_version,requester_consents) values($1,$2,'INSTAGRAM',1,1,true)").bind(self.event).bind(self.social).execute(c).await?;
        Ok(())
    }
    pub fn context(&self) -> Context {
        let now = Utc::now();
        Context {
            actor_id: self.actor,
            event_id: self.event,
            session_family_id: Uuid::new_v4(),
            request_id: Uuid::new_v4(),
            authority_digest: [71; 32],
            observed_at: now,
            expires_at: now + Duration::seconds(5),
        }
    }
    pub fn candidate(&self, kind: TargetType) -> Candidate {
        let id = match kind {
            TargetType::Party => self.party,
            TargetType::Application => self.application,
            TargetType::Giveaway => self.giveaway,
            TargetType::Reservation => self.reservation,
            TargetType::SocialRequest => self.social,
            TargetType::Notice => self.notice,
            TargetType::Post | TargetType::Report => Uuid::new_v4(),
        };
        Candidate {
            notification_id: Uuid::new_v4(),
            source_actor_id: Some(self.owner),
            target: Some(Target {
                target_type: kind,
                target_id: id,
            }),
        }
    }
}
