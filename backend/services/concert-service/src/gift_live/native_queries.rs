//! Local SQL/query tests only. Synthetic policy is NOT actual canonical authority.
//! No positive OwnerGuardScope/GuardedSubject fixture is constructed here.
#[path = "native_completion.rs"]
mod completion_tests;
use super::*;
use crate::community::{giveaway::GiftSettings, CommunityAuthority};
use async_trait::async_trait;
use chrono::Duration;
use concert_application::{
    commands::{FingerprintKeys, SigningKey},
    community::giveaway::{GiftCommand, PickupKey, PickupKeys},
};
use concert_domain::community::giveaway::{GiftAction, GiftFields, GiftState, PickupMeeting};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use std::{collections::BTreeMap, sync::Arc};
#[path = "../../tests/gift_live_support/owned.rs"]
mod owned;

#[derive(Clone)]
struct SyntheticPolicy {
    event: Uuid,
    members: Arc<HashSet<Uuid>>,
}
#[async_trait]
impl CommunityAuthority for SyntheticPolicy {
    async fn actor_in(
        &self,
        c: &mut PgConnection,
        actor: Uuid,
        event: Uuid,
        _: CommunityAccess,
    ) -> Result<(), CommandError> {
        if event != self.event || !self.members.contains(&actor) {
            return Err(CommandError::Denied);
        }
        sqlx::query("select set_config('mcr.community_authority_until',(clock_timestamp()+interval '5 seconds')::text,true)").execute(c).await.map_err(|_|CommandError::Unavailable)?;
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
        self.actor_in(c, actor, event, access).await?;
        if self.members.contains(&subject) {
            Ok(())
        } else {
            Err(CommandError::Denied)
        }
    }
    async fn place_in(
        &self,
        _: &mut PgConnection,
        event: Uuid,
        place: Uuid,
    ) -> Result<(), CommandError> {
        if event == self.event && !place.is_nil() {
            Ok(())
        } else {
            Err(CommandError::Denied)
        }
    }
}
#[async_trait]
impl GiftAuthority for SyntheticPolicy {
    async fn settings_in(
        &self,
        _: &mut PgConnection,
        _: Uuid,
    ) -> Result<GiftSettings, CommandError> {
        Ok(GiftSettings {
            pickup_grace_seconds: 30,
            event_cutoff: "2099-01-01T00:00:00Z".parse().unwrap(),
        })
    }
    async fn media_in(
        &self,
        _: &mut PgConnection,
        _: Uuid,
        _: Uuid,
        _: Uuid,
        media: Option<Uuid>,
    ) -> Result<(), CommandError> {
        if media.is_none() {
            Ok(())
        } else {
            Err(CommandError::Denied)
        }
    }
    async fn meeting_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        m: &PickupMeeting,
    ) -> Result<(), CommandError> {
        self.place_in(c, event, m.place_id).await
    }
}
pub(crate) struct Fixture {
    pub(crate) pool: PgPool,
    authority: SyntheticPolicy,
    gifts: PgGifts<SyntheticPolicy>,
    pub(crate) owner: Uuid,
    pub(crate) recipients: Vec<Uuid>,
    pub(crate) event: Uuid,
    keys: FingerprintKeys,
    codes: PickupKeys,
}
impl Fixture {
    pub(crate) async fn new() -> anyhow::Result<Self> {
        let pool = owned::owned_pool().await?;
        let event = Uuid::new_v4();
        let owner = Uuid::new_v4();
        let recipients = vec![Uuid::new_v4(), Uuid::new_v4()];
        sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Gift query fixture','OPEN')").bind(event).bind(format!("giftq-{event}")).execute(&pool).await?;
        let mut members = HashSet::from([owner]);
        members.extend(recipients.iter().copied());
        let authority = SyntheticPolicy {
            event,
            members: Arc::new(members),
        };
        Ok(Self {
            gifts: PgGifts::new(pool.clone(), authority.clone()),
            pool,
            authority,
            owner,
            recipients,
            event,
            keys: FingerprintKeys::new(SigningKey::new("gift-query", &[63; 32])?, vec![])?,
            codes: PickupKeys::new(PickupKey::new("gift-query", &[64; 32])?, vec![])?,
        })
    }
    fn command(&self, actor: Uuid, giveaway: Uuid, action: GiftAction) -> GiftCommand {
        GiftCommand {
            actor,
            event: self.event,
            key: Uuid::new_v4(),
            giveaway,
            action,
            code: None,
        }
    }
    async fn execute(&self, c: &GiftCommand) -> anyhow::Result<()> {
        self.gifts
            .execute(c, &self.keys, &self.codes, None)
            .await
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        Ok(())
    }
    pub(crate) async fn gift(&self, starts: DateTime<Utc>) -> anyhow::Result<Uuid> {
        let fields = GiftFields {
            title: "Synthetic query gift".into(),
            description: String::new(),
            image_media_id: None,
            quantity_total: 10,
            pickup_starts_at: starts,
            pickup_ends_at: Utc::now() + Duration::hours(1),
            public_place_id: Uuid::new_v4(),
        };
        let c = self.command(self.owner, Uuid::nil(), GiftAction::Create(fields.clone()));
        let id = c.giveaway_id();
        self.execute(&c).await?;
        self.execute(&self.command(
            self.owner,
            id,
            GiftAction::SetMeeting {
                expected: 1,
                physical_acknowledged: true,
                meeting: PickupMeeting {
                    place_id: fields.public_place_id,
                    map_revision: 1,
                    place_revision: 1,
                    instructions: "Synthetic query fixture".into(),
                    identification: "Test only".into(),
                    expires_at: fields.pickup_ends_at + Duration::seconds(30),
                },
            },
        ))
        .await?;
        self.execute(&self.command(
            self.owner,
            id,
            GiftAction::Update {
                expected: 2,
                fields,
                state: GiftState::Open,
            },
        ))
        .await?;
        Ok(id)
    }
    pub(crate) async fn reserve(&self, id: Uuid, recipient: Uuid) -> anyhow::Result<Uuid> {
        let g = self
            .gifts
            .detail(self.owner, self.event, id)
            .await
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        let c = self.command(
            recipient,
            id,
            GiftAction::Reserve {
                giveaway_revision: g.revision,
            },
        );
        let r = c.target();
        self.execute(&c).await?;
        Ok(r)
    }
    pub(crate) fn identity(&self, actor: Uuid) -> GiftIdentity {
        GiftIdentity {
            actor,
            event: self.event,
            session_family: Uuid::new_v4(),
            request_id: Uuid::new_v4(),
            deadline: AuthorityRpcDeadline::new(
                Utc::now().timestamp_micros() + 5_000_000,
                std::time::Duration::from_secs(5),
            )
            .unwrap(),
        }
    }
    fn reads<'a>(&'a self, actor: Uuid, cursor: &'a CursorCodec) -> GiftReads<'a, SyntheticPolicy> {
        GiftReads {
            pool: &self.pool,
            authority: &self.authority,
            gifts: &self.gifts,
            identity: self.identity(actor),
            cursor,
        }
    }
    fn binding(
        &self,
        actor: Uuid,
        collection: Collection,
        giveaway: Option<Uuid>,
    ) -> CursorBinding {
        CursorBinding {
            actor,
            event: self.event,
            collection,
            giveaway,
            operation: match collection {
                Collection::Public => "listConcertGiveaways",
                Collection::Management => "getConcertGiveawayManagement",
                _ => "getConcertGiftActivity",
            }
            .into(),
            filters: BTreeMap::new(),
            limit: 1,
        }
    }
}
fn filter() -> GiftFilter {
    GiftFilter {
        state: None,
        in_stock: None,
        pickup_after: None,
        pickup_before: None,
        public_place_id: None,
        after: None,
    }
}
fn err(e: GiftServiceError) -> anyhow::Error {
    anyhow::anyhow!("{e:?}")
}

#[tokio::test]
#[ignore = "private Gift PG fixture; local SQL proof only"]
async fn public_descending_snapshot_excludes_later_insert_and_rebind() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let starts = Utc::now() - Duration::minutes(1);
    let first = f.gift(starts).await?;
    let second = f.gift(starts).await?;
    let third = f.gift(starts).await?;
    let codec = CursorCodec::new(&[47; 32]).unwrap();
    let reads = f.reads(f.owner, &codec);
    let b = f.binding(f.owner, Collection::Public, None);
    let p = reads.list(&b, None, filter()).await.map_err(err)?;
    assert_eq!(p.items[0].id, third);
    let window = p.next.unwrap();
    let token = codec.seal(&b, window, Utc::now()).unwrap();
    let later = f.gift(starts).await?;
    assert_ne!(later, third);
    let p = reads.list(&b, Some(&token), filter()).await.map_err(err)?;
    assert_eq!(p.items[0].id, second);
    assert_eq!(p.next.unwrap().upper, window.upper);
    let token = codec.seal(&b, p.next.unwrap(), Utc::now()).unwrap();
    let p = reads.list(&b, Some(&token), filter()).await.map_err(err)?;
    assert_eq!(p.items[0].id, first);
    assert!(p.next.is_none());
    let mut changed = b.clone();
    changed.filters.insert("state".into(), "OPEN".into());
    assert!(matches!(
        reads.list(&changed, Some(&token), filter()).await,
        Err(GiftServiceError::Command(CommandError::InvalidInput))
    ));
    f.pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "private Gift PG fixture; local SQL proof only"]
async fn receiving_uses_reservation_creation_order_and_actor_scoped_lookup() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let actor = f.recipients[0];
    let starts = Utc::now() - Duration::minutes(1);
    let a = f.gift(starts).await?;
    let b = f.gift(starts).await?;
    let first = f.reserve(b, actor).await?;
    let second = f.reserve(a, actor).await?;
    let codec = CursorCodec::new(&[48; 32]).unwrap();
    let reads = f.reads(actor, &codec);
    let binding = f.binding(actor, Collection::Receiving, None);
    let page = reads.receiving(&binding, None).await.map_err(err)?;
    assert_eq!(page.items[0].value.id, second);
    let token = codec
        .seal(&binding, page.next.unwrap(), Utc::now())
        .unwrap();
    let page = reads.receiving(&binding, Some(&token)).await.map_err(err)?;
    assert_eq!(page.items[0].value.id, first);
    assert!(page.next.is_none());
    let other = f.reads(f.recipients[1], &codec);
    assert!(matches!(
        other.root_for_reservation(first, false, false, false).await,
        Err(GiftServiceError::Command(CommandError::Denied))
    ));
    assert_eq!(
        reads
            .root_for_reservation(first, false, false, true)
            .await
            .map_err(err)?,
        b
    );
    assert!(matches!(
        reads.root_for_reservation(first, false, true, false).await,
        Err(GiftServiceError::Command(CommandError::Denied))
    ));
    f.pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "private Gift PG fixture; local SQL proof only"]
async fn management_is_owner_only_descending_and_durably_advances_readiness() -> anyhow::Result<()>
{
    let f = Fixture::new().await?;
    let starts: DateTime<Utc> =
        sqlx::query_scalar("select clock_timestamp()+interval '500 milliseconds'")
            .fetch_one(&f.pool)
            .await?;
    let gift = f.gift(starts).await?;
    let first = f.reserve(gift, f.recipients[0]).await?;
    let second = f.reserve(gift, f.recipients[1]).await?;
    sqlx::query("select pg_sleep(0.6)").execute(&f.pool).await?;
    let codec = CursorCodec::new(&[49; 32]).unwrap();
    let reads = f.reads(f.owner, &codec);
    let mut b = f.binding(f.owner, Collection::Management, Some(gift));
    b.filters.insert("state".into(), "READY".into());
    let page = reads
        .management(gift, &b, None, Some(ReservationState::Ready))
        .await
        .map_err(err)?;
    assert_eq!(page.reservations[0].id, second);
    assert_eq!(page.reservations[0].state, ReservationState::Ready);
    assert_eq!(page.reserved, 2);
    let token = codec.seal(&b, page.next.unwrap(), Utc::now()).unwrap();
    let page = reads
        .management(gift, &b, Some(&token), Some(ReservationState::Ready))
        .await
        .map_err(err)?;
    assert_eq!(page.reservations[0].id, first);
    assert!(page.next.is_none());
    let states: Vec<String> = sqlx::query_scalar(
        "select state from concert_community_reservations where event_id=$1 and giveaway_id=$2",
    )
    .bind(f.event)
    .bind(gift)
    .fetch_all(&f.pool)
    .await?;
    assert_eq!(states, vec!["READY", "READY"]);
    let other = f.reads(f.recipients[0], &codec);
    assert!(matches!(
        other.management(gift, &b, None, None).await,
        Err(GiftServiceError::Command(CommandError::Denied))
    ));
    f.pool.close().await;
    Ok(())
}
