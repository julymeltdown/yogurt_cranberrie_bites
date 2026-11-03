//! Real BE37 SQL adapter with deliberately synthetic request-bound owner/resource ports.
#[path = "support/community/owned.rs"]
mod owned;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::{commands::*, community::PartyCommand};
use concert_domain::{
    community::{giveaway::PickupMeeting, party::*},
    relationship::*,
    *,
};
use concert_service::{
    community::{giveaway::GiftSettings, policy::*, *},
    relationship::{FencedRelationshipAuthority, PgRelationshipPolicy},
};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;
struct SyntheticOwner;
#[async_trait]
impl FencedRelationshipAuthority for SyntheticOwner {
    async fn check_in(
        &self,
        _: &mut PgConnection,
        query: PolicyQuery,
        now: DateTime<Utc>,
    ) -> Result<FreshAuthority, RelationshipError> {
        Ok(FreshAuthority {
            query,
            checked_at: now,
            valid_until: now + Duration::seconds(10),
            revision: Revision::initial(),
            actor_allowed: true,
            subject_allowed: true,
            inherited_block: false,
        })
    }
}
struct SyntheticResources;
#[async_trait]
impl CommunityResources for SyntheticResources {
    async fn feature_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        actor: Uuid,
        _: CommunityAccess,
    ) -> Result<(), CommandError> {
        let allowed: Option<bool> = sqlx::query_scalar(
            "select allowed from community_test_authority where event=$1 and actor=$2 for share",
        )
        .bind(event)
        .bind(actor)
        .fetch_optional(c)
        .await
        .map_err(|_| CommandError::Unavailable)?;
        if allowed == Some(true) {
            Ok(())
        } else {
            Err(CommandError::Denied)
        }
    }
    async fn place_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        place: Uuid,
    ) -> Result<(), CommandError> {
        let allowed: Option<bool> = sqlx::query_scalar(
            "select allowed from community_test_places where event=$1 and place=$2 for share",
        )
        .bind(event)
        .bind(place)
        .fetch_optional(c)
        .await
        .map_err(|_| CommandError::Unavailable)?;
        if allowed == Some(true) {
            Ok(())
        } else {
            Err(CommandError::Denied)
        }
    }
    async fn gift_settings_in(
        &self,
        _: &mut PgConnection,
        _: Uuid,
    ) -> Result<GiftSettings, CommandError> {
        Err(CommandError::Unavailable)
    }
    async fn gift_media_in(
        &self,
        _: &mut PgConnection,
        _: Uuid,
        _: Uuid,
        _: Uuid,
        _: Option<Uuid>,
    ) -> Result<(), CommandError> {
        Err(CommandError::Unavailable)
    }
    async fn pickup_meeting_in(
        &self,
        _: &mut PgConnection,
        _: Uuid,
        _: &PickupMeeting,
    ) -> Result<(), CommandError> {
        Err(CommandError::Unavailable)
    }
}
fn keys() -> FingerprintKeys {
    FingerprintKeys::new(SigningKey::new("policy-native", &[35; 32]).unwrap(), vec![]).unwrap()
}
fn fields(place: Uuid) -> PartyFields {
    PartyFields {
        title: "Policy fixture".into(),
        description: String::new(),
        ticket_section: None,
        capacity: 2,
        meeting_starts_at: "2090-01-01T00:00:00Z".parse().unwrap(),
        meeting_ends_at: "2090-01-02T00:00:00Z".parse().unwrap(),
        requirements: String::new(),
        public_place_id: place,
    }
}
async fn member(pool: &PgPool, event: Uuid, user: Uuid) -> anyhow::Result<()> {
    sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false) on conflict do nothing").bind(event).bind(user).execute(pool).await?;
    sqlx::query("insert into community_test_authority(event,actor,allowed) values($1,$2,true) on conflict do nothing").bind(event).bind(user).execute(pool).await?;
    Ok(())
}
async fn policy_restart_fixture() -> anyhow::Result<()> {
    let pool = owned::owned_pool().await?;
    let event = Uuid::from_u128(39001);
    let owner = Uuid::from_u128(39002);
    let place = Uuid::from_u128(39003);
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,'shared-community-policy','Policy fixture','OPEN') on conflict do nothing").bind(event).execute(&pool).await?;
    member(&pool, event, owner).await?;
    sqlx::query("insert into community_test_places(event,place,allowed) values($1,$2,true) on conflict do nothing").bind(event).bind(place).execute(&pool).await?;
    let service = PgCommunity::new(
        pool.clone(),
        SharedCommunityAuthority::new(
            PgRelationshipPolicy::new(pool.clone(), SyntheticOwner),
            SyntheticResources,
        ),
    );
    let cmd = PartyCommand {
        actor: owner,
        event,
        key: Uuid::from_u128(39004),
        party: Uuid::nil(),
        action: PartyAction::Create(fields(place)),
    };
    let receipt = service.execute_party(&cmd, &keys()).await.unwrap();
    assert_eq!(receipt.reference.unwrap().revision, 1);
    Ok(())
}
#[tokio::test]
#[ignore = "requires marker-owned native cluster"]
async fn community_policy_missing_member_block_and_unavailable_deny() -> anyhow::Result<()> {
    let pool = owned::owned_pool().await?;
    let event = Uuid::new_v4();
    let owner = Uuid::new_v4();
    let place = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Policy fixture','OPEN')").bind(event).bind(format!("shared-{event}")).execute(&pool).await?;
    sqlx::query("insert into community_test_authority(event,actor,allowed) values($1,$2,true)")
        .bind(event)
        .bind(owner)
        .execute(&pool)
        .await?;
    sqlx::query("insert into community_test_places(event,place,allowed) values($1,$2,true)")
        .bind(event)
        .bind(place)
        .execute(&pool)
        .await?;
    let service = PgCommunity::new(
        pool.clone(),
        SharedCommunityAuthority::new(
            PgRelationshipPolicy::new(pool.clone(), SyntheticOwner),
            SyntheticResources,
        ),
    );
    let cmd = PartyCommand {
        actor: owner,
        event,
        key: Uuid::new_v4(),
        party: Uuid::nil(),
        action: PartyAction::Create(fields(place)),
    };
    assert_eq!(
        service.execute_party(&cmd, &keys()).await,
        Err(CommunityError::Command(CommandError::Denied))
    );
    member(&pool, event, owner).await?;
    service.execute_party(&cmd, &keys()).await.unwrap();
    let id = cmd.party_id();
    let unknown = PgCommunity::new(
        pool.clone(),
        SharedCommunityAuthority::new(
            PgRelationshipPolicy::new(
                pool.clone(),
                concert_service::relationship::UnavailableAuthority,
            ),
            SyntheticResources,
        ),
    );
    assert_eq!(
        unknown.party(owner, event, id).await,
        Err(CommunityError::Command(CommandError::Unavailable))
    );
    let other = Uuid::new_v4();
    member(&pool, event, other).await?;
    let policy = PgRelationshipPolicy::new(pool.clone(), SyntheticOwner);
    let query = PolicyQuery {
        event: EventId::from_uuid(event),
        actor: AuthUserId::from_uuid(owner),
        subject: AuthUserId::from_uuid(other),
        field: Field::PublicProfile,
    };
    let mut tx = pool.begin().await?;
    policy.authorize_in(&mut tx, query, None).await.unwrap();
    let (low, high) = if owner < other {
        (owner, other)
    } else {
        (other, owner)
    };
    sqlx::query("update concert_relationship_pairs set low_blocks_high=true,revision=revision+1 where event_id=$1 and low_user=$2 and high_user=$3").bind(event).bind(low).bind(high).execute(&mut *tx).await?;
    tx.commit().await?;
    assert_eq!(
        service.party(other, event, id).await,
        Err(CommunityError::Command(CommandError::Denied))
    );
    sqlx::query(
        "update concert_events set phase='READ_ONLY',revision=revision+1 where event_id=$1",
    )
    .bind(event)
    .execute(&pool)
    .await?;
    let cancel = PartyCommand {
        actor: owner,
        event,
        key: Uuid::new_v4(),
        party: id,
        action: PartyAction::Cancel { expected: 1 },
    };
    service.execute_party(&cancel, &keys()).await.unwrap();
    assert_eq!(
        service
            .execute_party(&cmd, &keys())
            .await
            .unwrap()
            .reference
            .unwrap()
            .resource,
        id
    );
    Ok(())
}
#[tokio::test]
#[ignore = "requires marker-owned native cluster"]
async fn community_commit_deadline_rejects_delayed_mutation() -> anyhow::Result<()> {
    let pool = owned::owned_pool().await?;
    let mut tx = pool.begin().await?;
    sqlx::query("select set_config('mcr.community_effect_until',(clock_timestamp()-interval '1 second')::text,true)").execute(&mut *tx).await?;
    // Trigger event on an existing stable party; no changed aggregate is needed to prove rollback.
    sqlx::query("update concert_community_parties set aggregate=aggregate where event_id=$1")
        .bind(Uuid::from_u128(39001))
        .execute(&mut *tx)
        .await?;
    // If restart fixture has not run yet, build a dedicated row then test in a new transaction.
    let affected: i64 =
        sqlx::query_scalar("select count(*) from concert_community_parties where event_id=$1")
            .bind(Uuid::from_u128(39001))
            .fetch_one(&mut *tx)
            .await?;
    if affected == 0 {
        tx.rollback().await?;
        policy_restart_fixture().await?;
        let mut tx = pool.begin().await?;
        sqlx::query("select set_config('mcr.community_effect_until',(clock_timestamp()-interval '1 second')::text,true)").execute(&mut *tx).await?;
        sqlx::query("update concert_community_parties set aggregate=aggregate where event_id=$1")
            .bind(Uuid::from_u128(39001))
            .execute(&mut *tx)
            .await?;
        assert!(tx.commit().await.is_err());
    } else {
        assert!(tx.commit().await.is_err());
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires marker-owned native cluster"]
async fn community_policy_restart_receipt() -> anyhow::Result<()> {
    policy_restart_fixture().await
}

#[tokio::test]
#[ignore = "requires marker-owned native cluster"]
async fn community_policy_cross_party_leave_remove_share_sorted_pairs() -> anyhow::Result<()> {
    let pool = owned::owned_pool().await?;
    let event = Uuid::new_v4();
    let a = Uuid::from_u128(49001);
    let b = Uuid::from_u128(49002);
    let place = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Pair ordering','OPEN')").bind(event).bind(format!("pairs-{event}")).execute(&pool).await?;
    member(&pool, event, a).await?;
    member(&pool, event, b).await?;
    sqlx::query("insert into community_test_places(event,place,allowed) values($1,$2,true)")
        .bind(event)
        .bind(place)
        .execute(&pool)
        .await?;
    let service = PgCommunity::new(
        pool.clone(),
        SharedCommunityAuthority::new(
            PgRelationshipPolicy::new(pool.clone(), SyntheticOwner),
            SyntheticResources,
        ),
    );
    let keys = keys();
    let mut parties = vec![];
    for (owner, user) in [(a, b), (b, a)] {
        let create = PartyCommand {
            actor: owner,
            event,
            key: Uuid::new_v4(),
            party: Uuid::nil(),
            action: PartyAction::Create(fields(place)),
        };
        let party = create.party_id();
        service.execute_party(&create, &keys).await.unwrap();
        let apply = PartyCommand {
            actor: user,
            event,
            key: Uuid::new_v4(),
            party,
            action: PartyAction::Apply {
                party_revision: 1,
                note: String::new(),
            },
        };
        let application = apply.target();
        service.execute_party(&apply, &keys).await.unwrap();
        service
            .execute_party(
                &PartyCommand {
                    actor: owner,
                    event,
                    key: Uuid::new_v4(),
                    party,
                    action: PartyAction::Decide {
                        application,
                        expected: 1,
                        party_revision: 1,
                        approve: true,
                    },
                },
                &keys,
            )
            .await
            .unwrap();
        parties.push(party);
    }
    let leave = PartyCommand {
        actor: b,
        event,
        key: Uuid::new_v4(),
        party: parties[0],
        action: PartyAction::Remove {
            user: b,
            expected: 1,
            party_revision: 2,
        },
    };
    let remove = PartyCommand {
        actor: b,
        event,
        key: Uuid::new_v4(),
        party: parties[1],
        action: PartyAction::Remove {
            user: a,
            expected: 1,
            party_revision: 2,
        },
    };
    let (l, r) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::join!(
            service.execute_party(&leave, &keys),
            service.execute_party(&remove, &keys)
        )
    })
    .await?;
    assert!(l.is_ok(), "{l:?}");
    assert!(r.is_ok(), "{r:?}");
    Ok(())
}
