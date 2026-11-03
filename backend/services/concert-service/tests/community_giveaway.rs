//! Test-only native gift fixtures; no production authority or official place data.
#[path = "support/community/owned.rs"]
mod owned;
use async_trait::async_trait;
use chrono::{Duration, Utc};
use concert_application::{commands::*, community::giveaway::*};
use concert_domain::community::giveaway::*;
use concert_service::community::{giveaway::*, CommunityAuthority};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;
struct SyntheticAuthority;
#[async_trait]
impl CommunityAuthority for SyntheticAuthority {
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        actor: Uuid,
        event: Uuid,
        _: Uuid,
        _: bool,
    ) -> Result<(), CommandError> {
        let allowed: Option<bool> = sqlx::query_scalar(
            "select allowed from community_test_authority where actor=$1 and event=$2 for share",
        )
        .bind(actor)
        .bind(event)
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
}
#[async_trait]
impl GiftAuthority for SyntheticAuthority {
    async fn settings_in(
        &self,
        _: &mut PgConnection,
        _: Uuid,
    ) -> Result<GiftSettings, CommandError> {
        Ok(GiftSettings {
            pickup_grace_seconds: 60,
            event_cutoff: "2099-01-01T00:00:00Z".parse().unwrap(),
        })
    }
    async fn media_in(
        &self,
        _: &mut PgConnection,
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
fn keys() -> FingerprintKeys {
    FingerprintKeys::new(SigningKey::new("test-receipts", &[62; 32]).unwrap(), vec![]).unwrap()
}
fn code_keys() -> PickupKeys {
    PickupKeys::new(PickupKey::new("test-pickup", &[73; 32]).unwrap(), vec![]).unwrap()
}
async fn allow(pool: &PgPool, event: Uuid, user: Uuid) -> anyhow::Result<()> {
    sqlx::query("insert into community_test_authority(event,actor,allowed) values($1,$2,true) on conflict do nothing").bind(event).bind(user).execute(pool).await?;
    Ok(())
}
fn command(actor: Uuid, event: Uuid, giveaway: Uuid, action: GiftAction) -> GiftCommand {
    GiftCommand {
        actor,
        event,
        giveaway,
        action,
        key: Uuid::new_v4(),
        code: None,
    }
}
async fn fixture(total: i32) -> anyhow::Result<(PgPool, Uuid, Uuid, Uuid)> {
    let pool = owned::owned_pool().await?;
    let event = Uuid::new_v4();
    let owner = Uuid::new_v4();
    let place = Uuid::new_v4();
    sqlx::query(
        "insert into concert_events(event_id,slug,title,phase) values($1,$2,'Gift fixture','OPEN')",
    )
    .bind(event)
    .bind(format!("gifts-{event}"))
    .execute(&pool)
    .await?;
    allow(&pool, event, owner).await?;
    sqlx::query("insert into community_test_places(event,place,allowed) values($1,$2,true)")
        .bind(event)
        .bind(place)
        .execute(&pool)
        .await?;
    let fields = GiftFields {
        title: "Owned giveaway".into(),
        description: String::new(),
        image_media_id: None,
        quantity_total: total,
        pickup_starts_at: Utc::now() - Duration::minutes(1),
        pickup_ends_at: Utc::now() + Duration::hours(1),
        public_place_id: place,
    };
    let c = command(
        owner,
        event,
        Uuid::nil(),
        GiftAction::Create(fields.clone()),
    );
    let id = c.giveaway_id();
    let service = PgGifts::new(pool.clone(), SyntheticAuthority);
    let keys = keys();
    let codes = code_keys();
    service
        .execute(&c, &keys, &codes, None)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let m = PickupMeeting {
        place_id: place,
        map_revision: 1,
        place_revision: 1,
        instructions: "Synthetic private pickup".into(),
        identification: "Test identifier".into(),
        expires_at: fields.pickup_ends_at + Duration::seconds(60),
    };
    service
        .execute(
            &command(
                owner,
                event,
                id,
                GiftAction::SetMeeting {
                    expected: 1,
                    meeting: m,
                    physical_acknowledged: true,
                },
            ),
            &keys,
            &codes,
            None,
        )
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    service
        .execute(
            &command(
                owner,
                event,
                id,
                GiftAction::Update {
                    expected: 2,
                    fields,
                    state: GiftState::Open,
                },
            ),
            &keys,
            &codes,
            None,
        )
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    Ok((pool, event, owner, id))
}
#[tokio::test]
#[ignore = "requires marker-owned native cluster"]
async fn community_gift_five_buyers_one_unit_and_exact_replay() -> anyhow::Result<()> {
    let (pool, event, _, gift) = fixture(1).await?;
    let service = PgGifts::new(pool.clone(), SyntheticAuthority);
    let keys = keys();
    let codes = code_keys();
    let mut commands = vec![];
    for _ in 0..5 {
        let user = Uuid::new_v4();
        allow(&pool, event, user).await?;
        commands.push(command(
            user,
            event,
            gift,
            GiftAction::Reserve {
                giveaway_revision: 3,
            },
        ));
    }
    let (a, b, c, d, e) = tokio::join!(
        service.execute(&commands[0], &keys, &codes, None),
        service.execute(&commands[1], &keys, &codes, None),
        service.execute(&commands[2], &keys, &codes, None),
        service.execute(&commands[3], &keys, &codes, None),
        service.execute(&commands[4], &keys, &codes, None)
    );
    let results = vec![a, b, c, d, e];
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    let winner = results.iter().position(|r| r.is_ok()).unwrap();
    assert_eq!(
        service
            .execute(&commands[winner], &keys, &codes, None)
            .await
            .unwrap()
            .receipt,
        results[winner].as_ref().unwrap().receipt
    );
    let loser = (winner + 1) % 5;
    commands[loser].action = GiftAction::Reserve {
        giveaway_revision: 4,
    };
    commands[loser].key = Uuid::new_v4();
    assert!(matches!(
        service.execute(&commands[loser], &keys, &codes, None).await,
        Err(GiftServiceError::Gift(GiftError::SoldOut))
    ));
    let state:(i32,i32)=sqlx::query_as("select reserved_unclaimed,quantity_claimed from concert_community_giveaways where event_id=$1 and giveaway_id=$2").bind(event).bind(gift).fetch_one(&pool).await?;
    assert_eq!(state, (1, 0));
    Ok(())
}
#[tokio::test]
#[ignore = "requires marker-owned native cluster"]
async fn community_gift_code_rotation_throttle_and_one_time_pickup() -> anyhow::Result<()> {
    let (pool, event, owner, gift) = fixture(1).await?;
    let user = Uuid::new_v4();
    allow(&pool, event, user).await?;
    let service = PgGifts::new(pool.clone(), SyntheticAuthority);
    let keys = keys();
    let codes = code_keys();
    let context = TrustedPickupContext::from_trusted_keyed_ip_bucket([19; 32]);
    let reserve = command(
        user,
        event,
        gift,
        GiftAction::Reserve {
            giveaway_revision: 3,
        },
    );
    let reservation = reserve.target();
    service
        .execute(&reserve, &keys, &codes, None)
        .await
        .unwrap();
    let issue = command(
        user,
        event,
        gift,
        GiftAction::IssueCode {
            reservation,
            expected: 1,
        },
    );
    let result = service.execute(&issue, &keys, &codes, None).await.unwrap();
    let first = result.issued_code.unwrap();
    assert_eq!(first.revision, 2);
    assert!(service
        .execute(&issue, &keys, &codes, None)
        .await
        .unwrap()
        .issued_code
        .is_none());
    let second = service
        .execute(
            &command(
                user,
                event,
                gift,
                GiftAction::IssueCode {
                    reservation,
                    expected: 2,
                },
            ),
            &keys,
            &codes,
            None,
        )
        .await
        .unwrap()
        .issued_code
        .unwrap();
    let mut old = command(
        owner,
        event,
        gift,
        GiftAction::Claim {
            reservation,
            expected: 3,
        },
    );
    old.code = Some(first.code);
    assert!(matches!(
        service.execute(&old, &keys, &codes, Some(&context)).await,
        Err(GiftServiceError::Gift(GiftError::CodeInvalid))
    ));
    let mut claim = command(
        owner,
        event,
        gift,
        GiftAction::Claim {
            reservation,
            expected: 3,
        },
    );
    claim.code = Some(second.code);
    let receipt = service
        .execute(&claim, &keys, &codes, Some(&context))
        .await
        .unwrap()
        .receipt;
    assert_eq!(
        service
            .execute(&claim, &keys, &codes, Some(&context))
            .await
            .unwrap()
            .receipt,
        receipt
    );
    claim.key = Uuid::new_v4();
    assert_eq!(
        service
            .execute(&claim, &keys, &codes, Some(&context))
            .await
            .unwrap()
            .receipt
            .reference,
        receipt.reference
    );
    let r = service
        .reservation(user, event, gift, reservation)
        .await
        .unwrap();
    assert_eq!(r.state, ReservationState::Claimed);
    assert!(r.claim_receipt_id.is_some());
    assert!(r.code_expires_at.is_none());
    assert!(service
        .pickup(user, event, gift, reservation)
        .await
        .is_err());
    let counter:(i32,i32)=sqlx::query_as("select reserved_unclaimed,quantity_claimed from concert_community_giveaways where event_id=$1 and giveaway_id=$2").bind(event).bind(gift).fetch_one(&pool).await?;
    assert_eq!(counter, (0, 1));
    Ok(())
}
#[tokio::test]
#[ignore = "requires marker-owned native cluster"]
async fn community_gift_guesses_persist_and_expiry_cancel_race_release_once() -> anyhow::Result<()>
{
    let (pool, event, owner, gift) = fixture(1).await?;
    let user = Uuid::new_v4();
    allow(&pool, event, user).await?;
    let service = PgGifts::new(pool.clone(), SyntheticAuthority);
    let keys = keys();
    let codes = code_keys();
    let context = TrustedPickupContext::from_trusted_keyed_ip_bucket([29; 32]);
    let reserve = command(
        user,
        event,
        gift,
        GiftAction::Reserve {
            giveaway_revision: 3,
        },
    );
    let reservation = reserve.target();
    service
        .execute(&reserve, &keys, &codes, None)
        .await
        .unwrap();
    let issued = service
        .execute(
            &command(
                user,
                event,
                gift,
                GiftAction::IssueCode {
                    reservation,
                    expected: 1,
                },
            ),
            &keys,
            &codes,
            None,
        )
        .await
        .unwrap()
        .issued_code
        .unwrap();
    let wrong = if issued.code.expose() == "000000" {
        "111111"
    } else {
        "000000"
    };
    for _ in 0..5 {
        let mut claim = command(
            owner,
            event,
            gift,
            GiftAction::Claim {
                reservation,
                expected: 2,
            },
        );
        claim.code = Some(PickupCode::parse(wrong.into()).unwrap());
        assert!(matches!(
            service.execute(&claim, &keys, &codes, Some(&context)).await,
            Err(GiftServiceError::Gift(GiftError::CodeInvalid))
        ));
    }
    let mut claim = command(
        owner,
        event,
        gift,
        GiftAction::Claim {
            reservation,
            expected: 2,
        },
    );
    claim.code = Some(issued.code);
    assert!(matches!(
        service.execute(&claim, &keys, &codes, Some(&context)).await,
        Err(GiftServiceError::Command(CommandError::Capacity))
    ));
    // Explicit test-only time injection rewrites both root/index expiry to the DB past.
    sqlx::query("update concert_community_giveaways set aggregate=jsonb_set(aggregate,'{reservations,0,expires_at}',to_jsonb(to_char(clock_timestamp()-interval '1 second','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'))),next_action_at=clock_timestamp()-interval '1 second' where event_id=$1 and giveaway_id=$2").bind(event).bind(gift).execute(&pool).await?;
    let cancel = command(
        user,
        event,
        gift,
        GiftAction::Cancel {
            reservation,
            expected: 2,
        },
    );
    let (cancelled, swept) = tokio::join!(
        service.execute(&cancel, &keys, &codes, None),
        service.sweep(100)
    );
    assert!(swept.is_ok());
    assert!(
        cancelled.is_ok()
            || matches!(
                cancelled,
                Err(GiftServiceError::Gift(
                    GiftError::VersionConflict | GiftError::InvalidTransition
                ))
            )
    );
    service.sweep(100).await.unwrap();
    let counter:(i32,i32)=sqlx::query_as("select reserved_unclaimed,quantity_claimed from concert_community_giveaways where event_id=$1 and giveaway_id=$2").bind(event).bind(gift).fetch_one(&pool).await?;
    assert_eq!(counter, (0, 0));
    let secret: i64 = sqlx::query_scalar(
        "select count(*) from concert_community_pickup_secrets where event_id=$1",
    )
    .bind(event)
    .fetch_one(&pool)
    .await?;
    assert_eq!(secret, 0);
    Ok(())
}

#[tokio::test]
#[ignore = "requires marker-owned native cluster"]
async fn community_gift_restart_expiry() -> anyhow::Result<()> {
    let pool = owned::owned_pool().await?;
    let sentinel = Uuid::from_u128(29001);
    let existing: Option<Uuid> =
        sqlx::query_scalar("select giveaway_id from concert_community_giveaways where owner_id=$1")
            .bind(sentinel)
            .fetch_optional(&pool)
            .await?;
    if let Some(gift) = existing {
        let service = PgGifts::new(pool.clone(), SyntheticAuthority);
        service.sweep(100).await.unwrap();
        let held: i32 = sqlx::query_scalar(
            "select reserved_unclaimed from concert_community_giveaways where giveaway_id=$1",
        )
        .bind(gift)
        .fetch_one(&pool)
        .await?;
        assert_eq!(held, 0);
    } else {
        let (pool, event, owner, gift) = fixture(1).await?;
        let user = Uuid::new_v4();
        allow(&pool, event, user).await?;
        let service = PgGifts::new(pool.clone(), SyntheticAuthority);
        service
            .execute(
                &command(
                    user,
                    event,
                    gift,
                    GiftAction::Reserve {
                        giveaway_revision: 3,
                    },
                ),
                &keys(),
                &code_keys(),
                None,
            )
            .await
            .unwrap();
        // Test-only owner sentinel and expiry injection; no live table is used.
        sqlx::query("update concert_community_giveaways set owner_id=$1,aggregate=jsonb_set(jsonb_set(aggregate,'{owner}',to_jsonb($1::text)),'{reservations,0,expires_at}',to_jsonb(to_char(clock_timestamp()-interval '1 second','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'))),next_action_at=clock_timestamp()-interval '1 second' where event_id=$2 and giveaway_id=$3").bind(sentinel).bind(event).bind(gift).execute(&pool).await?;
        let _ = owner;
    }
    Ok(())
}
