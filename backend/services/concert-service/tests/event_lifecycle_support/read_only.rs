//! Owned SQL fixtures. Auth/role observations are synthetic; World SQL and community
//! aggregate effects use their real adapters. No qualified production send proof here.
use async_trait::async_trait;
use chrono::{Duration, Utc};
use concert_application::{
    commands::CommandError, event_lifecycle::Actor, identity::SessionId, world_ticket::*,
};
use concert_domain::{
    community::{giveaway::*, party::*},
    event_lifecycle::{Retention, Step},
    AssetId, AuthUserId, EventId, ParticipantKey, Revision,
};
use concert_service::{
    event_lifecycle::{read_only::apply_read_only_in, Applied, EffectKey, LifecycleOwners},
    world::PgWorldRepository,
};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub struct ActualReadOnly {
    pub fail_after: bool,
    pub expire_after: bool,
}
#[async_trait]
impl LifecycleOwners for ActualReadOnly {
    async fn apply_in(
        &self,
        c: &mut PgConnection,
        key: EffectKey,
        _: Retention,
    ) -> Result<Applied, CommandError> {
        if key.step != Step::ReadOnly {
            return Err(CommandError::Unavailable);
        }
        let value = apply_read_only_in(c, key).await?;
        if self.fail_after {
            return Err(CommandError::Unavailable);
        }
        if self.expire_after {
            sqlx::query(
                "select set_config('mcr.lifecycle_authority_until','2000-01-01T00:00:00Z',true)",
            )
            .execute(c)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        }
        Ok(value)
    }
}
#[derive(Clone)]
pub struct Seed {
    pub party: Uuid,
    pub gift: Uuid,
    pub held: Uuid,
    pub claimed: Uuid,
    pub room: Uuid,
    pub record: TicketRecord,
    pub owner: OwnerIdentity,
}
pub async fn seed(pool: &PgPool, actor: Actor) -> anyhow::Result<Seed> {
    let at = Utc::now();
    let peer = Uuid::new_v4();
    let mut party = Party::create(
        Uuid::new_v4(),
        actor.event,
        actor.actor,
        PartyFields {
            title: "Owned closure fixture".into(),
            description: "".into(),
            ticket_section: None,
            capacity: 4,
            meeting_starts_at: at + Duration::minutes(5),
            meeting_ends_at: at + Duration::hours(1),
            requirements: "".into(),
            public_place_id: Uuid::new_v4(),
        },
        at,
    )?;
    party.change(
        peer,
        &PartyAction::Apply {
            party_revision: 1,
            note: "".into(),
        },
        Uuid::new_v4(),
        at,
    )?;
    let held = Uuid::new_v4();
    let claimed = Uuid::new_v4();
    let mut gift = Giveaway::create(
        Uuid::new_v4(),
        actor.event,
        actor.actor,
        GiftFields {
            title: "Owned gift".into(),
            description: "".into(),
            image_media_id: None,
            quantity_total: 3,
            pickup_starts_at: at - Duration::minutes(1),
            pickup_ends_at: at + Duration::hours(1),
            public_place_id: Uuid::new_v4(),
        },
        0,
        at,
    )?;
    gift.state = GiftState::Open;
    gift.meeting = Some(PickupMeeting {
        place_id: gift.fields.public_place_id,
        map_revision: 1,
        place_revision: 1,
        instructions: "Owned fixture".into(),
        identification: "Owned fixture".into(),
        expires_at: gift.expires_at(),
    });
    gift.reservations = vec![
        Reservation {
            id: held,
            recipient: peer,
            state: ReservationState::Ready,
            revision: 1,
            expires_at: gift.expires_at(),
            created_at: at,
            claim_receipt_id: None,
            claimed_at: None,
            code_expires_at: Some(gift.expires_at()),
            code_generation: 1,
        },
        Reservation {
            id: claimed,
            recipient: Uuid::new_v4(),
            state: ReservationState::Claimed,
            revision: 2,
            expires_at: gift.expires_at(),
            created_at: at,
            claim_receipt_id: Some(Uuid::new_v4()),
            claimed_at: Some(at),
            code_expires_at: None,
            code_generation: 1,
        },
    ];
    gift.reserved_unclaimed = 1;
    gift.quantity_claimed = 1;
    gift.validate()?;
    let room = Uuid::new_v4();
    let owner = OwnerIdentity::new(Uuid::new_v4().to_string(), Uuid::new_v4())?;
    let record = TicketRecord {
        digest: TicketDigest::from_bytes(Sha256::digest(Uuid::new_v4().as_bytes()).into())?,
        subject: WorldSubject {
            key: ParticipantKey {
                event_id: EventId::from_uuid(actor.event),
                user_id: AuthUserId::from_uuid(actor.actor),
            },
            event_id: EventId::from_uuid(actor.event),
            session_id: SessionId::new(Uuid::new_v4().to_string())?,
        },
        pin: ResourcePin {
            event_id: EventId::from_uuid(actor.event),
            room_id: RoomId::from_uuid(room),
            map: MapPin::new(AssetId::new("map_fixture".into())?, Revision::initial()),
            avatar: AvatarPin::new(AssetId::new("avatar_fixture".into())?, Revision::initial()),
        },
        issued_at: at,
        expires_at: at + Duration::seconds(25),
        access_expires_at: at + Duration::minutes(2),
    };
    let mut tx = pool.begin().await?;
    sqlx::query("insert into concert_community_parties(event_id,party_id,owner_id,revision,state,member_count,capacity,meeting_revision,audience_revision,aggregate) values($1,$2,$3,$4,'OPEN',1,4,$5,$6,$7)")
        .bind(actor.event).bind(party.id).bind(actor.actor).bind(party.revision).bind(party.meeting_revision).bind(party.audience_revision).bind(serde_json::to_value(&party)?).execute(&mut *tx).await?;
    sqlx::query("insert into concert_community_members(event_id,party_id,user_id,is_owner,state,revision) values($1,$2,$3,true,'ACTIVE',1)")
        .bind(actor.event).bind(party.id).bind(actor.actor).execute(&mut *tx).await?;
    for a in &party.applications {
        sqlx::query("insert into concert_community_applications(event_id,party_id,application_id,user_id,state,revision) values($1,$2,$3,$4,'PENDING',$5)")
            .bind(actor.event).bind(party.id).bind(a.id).bind(a.user).bind(a.revision).execute(&mut *tx).await?;
    }
    sqlx::query("insert into concert_community_giveaways(event_id,giveaway_id,owner_id,revision,state,quantity_total,reserved_unclaimed,quantity_claimed,aggregate) values($1,$2,$3,1,'OPEN',3,1,1,$4)")
        .bind(actor.event).bind(gift.id).bind(actor.actor).bind(serde_json::to_value(&gift)?).execute(&mut *tx).await?;
    for r in &gift.reservations {
        sqlx::query("insert into concert_community_reservations(event_id,giveaway_id,reservation_id,recipient_id,state,revision,expires_at) values($1,$2,$3,$4,$5,$6,$7)")
            .bind(actor.event).bind(gift.id).bind(r.id).bind(r.recipient).bind(if r.state==ReservationState::Ready {"READY"}else{"CLAIMED"}).bind(r.revision).bind(r.expires_at).execute(&mut *tx).await?;
    }
    sqlx::query("insert into concert_community_pickup_secrets(event_id,reservation_id,generation,key_id,keyed_hash,expires_at) values($1,$2,1,'fixture',$3,$4)")
        .bind(actor.event).bind(held).bind(vec![0x24_u8;32]).bind(gift.expires_at()).execute(&mut *tx).await?;
    sqlx::query("insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,is_default) values($1,$2,'Owned room',4,'map_fixture',1,'avatar_fixture',1,true)")
        .bind(actor.event).bind(room).execute(&mut *tx).await?;
    sqlx::query("insert into concert_world_owner_leases values($1,$2,$3,$4,$5,$6)")
        .bind(actor.event)
        .bind(room)
        .bind(&owner.process_id)
        .bind(owner.epoch)
        .bind(at - Duration::seconds(1))
        .bind(at + Duration::minutes(1))
        .execute(&mut *tx)
        .await?;
    sqlx::query("insert into concert_world_memberships(event_id,user_id,room_id,session_id,connection_id,owner_process_id,owner_epoch,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,admitted_at,lease_until,access_expires_at) values($1,$2,$3,$4,$5,$6,$7,'map_fixture',1,'avatar_fixture',1,$8,$9,$10)")
        .bind(actor.event).bind(actor.actor).bind(room).bind(Uuid::parse_str(record.subject.session_id.as_str())?).bind(Uuid::new_v4()).bind(&owner.process_id).bind(owner.epoch).bind(at-Duration::seconds(1)).bind(at+Duration::minutes(1)).bind(record.access_expires_at).execute(&mut *tx).await?;
    tx.commit().await?;
    assert_eq!(
        PgWorldRepository::new(pool.clone())
            .issue(record.clone())
            .await?,
        IssueOutcome::Issued
    );
    Ok(Seed {
        party: party.id,
        gift: gift.id,
        held,
        claimed,
        room,
        record,
        owner,
    })
}
pub async fn snapshot(pool: &PgPool, event: Uuid) -> anyhow::Result<serde_json::Value> {
    Ok(sqlx::query_scalar("select jsonb_build_object('event',(select to_jsonb(e) from concert_events e where event_id=$1),'parties',(select jsonb_agg(to_jsonb(p) order by party_id) from concert_community_parties p where event_id=$1),'gifts',(select jsonb_agg(to_jsonb(g) order by giveaway_id) from concert_community_giveaways g where event_id=$1),'applications',(select jsonb_agg(to_jsonb(a) order by application_id) from concert_community_applications a where event_id=$1),'reservations',(select jsonb_agg(to_jsonb(r) order by reservation_id) from concert_community_reservations r where event_id=$1),'secrets',(select jsonb_agg(to_jsonb(s) order by reservation_id) from concert_community_pickup_secrets s where event_id=$1),'rooms',(select jsonb_agg(to_jsonb(r) order by room_id) from concert_world_rooms r where event_id=$1),'owners',(select jsonb_agg(to_jsonb(o) order by room_id) from concert_world_owner_leases o where event_id=$1),'members',(select jsonb_agg(to_jsonb(m) order by user_id) from concert_world_memberships m where event_id=$1),'tickets',(select jsonb_agg(to_jsonb(t) order by encode(ticket_digest,'hex')) from concert_world_tickets t where event_id=$1))")
        .bind(event).fetch_one(pool).await?)
}
