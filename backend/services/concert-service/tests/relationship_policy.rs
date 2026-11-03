//! Real owned PostgreSQL tests. All authority below is explicitly test-only.
#[path = "support/relationship/mod.rs"]
mod support;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::{commands::*, relationship::RelationshipPolicy};
use concert_domain::{relationship::*, AuthUserId, EventId, Revision};
use concert_service::{
    adapters::out::repository::receipt::PgCommandRepository,
    relationship::{commands::BlockCommand, *},
};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

struct TestAuthority;
#[async_trait]
impl FencedRelationshipAuthority for TestAuthority {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        q: PolicyQuery,
        now: DateTime<Utc>,
    ) -> Result<FreshAuthority, RelationshipError> {
        let rows=sqlx::query("select user_id,allowed,inherited_block,revision from relationship_test_authority where event_id=$1 and user_id in ($2,$3) order by user_id for share").bind(q.event.uuid()).bind(q.actor.uuid()).bind(q.subject.uuid()).fetch_all(c).await.map_err(|_|RelationshipError::Unavailable)?;
        let actor = rows
            .iter()
            .find(|r| r.get::<Uuid, _>("user_id") == q.actor.uuid())
            .ok_or(RelationshipError::Unavailable)?;
        let subject = rows
            .iter()
            .find(|r| r.get::<Uuid, _>("user_id") == q.subject.uuid())
            .ok_or(RelationshipError::Unavailable)?;
        Ok(FreshAuthority {
            query: q,
            checked_at: now,
            valid_until: now + Duration::seconds(5),
            revision: Revision::new(
                actor.get::<i64, _>("revision") + subject.get::<i64, _>("revision"),
            )
            .unwrap(),
            actor_allowed: actor.get("allowed"),
            subject_allowed: subject.get("allowed"),
            inherited_block: actor.get::<bool, _>("inherited_block")
                || subject.get::<bool, _>("inherited_block"),
        })
    }
}
async fn fixture() -> anyhow::Result<(PgPool, PolicyQuery)> {
    let p = support::owned_pool().await?;
    let event = Uuid::new_v4();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Relationship test','LIVE')").bind(event).bind(format!("test-{event}")).execute(&p).await?;
    for u in [a, b] {
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(event).bind(u).execute(&p).await?;
        sqlx::query("insert into relationship_test_authority(event_id,user_id,allowed,inherited_block,revision) values($1,$2,true,false,1)").bind(event).bind(u).execute(&p).await?;
    }
    Ok((
        p,
        PolicyQuery {
            event: EventId::from_uuid(event),
            actor: AuthUserId::from_uuid(a),
            subject: AuthUserId::from_uuid(b),
            field: Field::PublicProfile,
        },
    ))
}
fn request(q: PolicyQuery, blocked: bool, key: Uuid) -> CanonicalRequest {
    let scope = Scope {
        actor: q.actor.uuid(),
        event: q.event.uuid(),
        operation: RegisteredOperation::find(if blocked {
            "blockConcertUser"
        } else {
            "unblockConcertUser"
        })
        .unwrap(),
        key,
    };
    CanonicalRequest::json(
        scope,
        &[
            ("eventId", &scope.event.to_string()),
            ("userId", &q.subject.uuid().to_string()),
        ],
        &[],
        None,
    )
    .unwrap()
}
fn keys() -> FingerprintKeys {
    FingerprintKeys::new(SigningKey::new("test", &[37; 32]).unwrap(), vec![]).unwrap()
}
async fn block(
    p: &PgPool,
    q: PolicyQuery,
    blocked: bool,
    key: Uuid,
) -> Result<SafeReceipt, CommandError> {
    let policy = PgRelationshipPolicy::new(p.clone(), TestAuthority);
    PgCommandRepository::new(p.clone())
        .execute(
            &request(q, blocked, key),
            q.subject.uuid(),
            &keys(),
            &BlockCommand {
                policy: &policy,
                query: q,
                blocked,
            },
        )
        .await
}
#[tokio::test]
async fn both_directions_unblock_and_stale_projection() -> anyhow::Result<()> {
    let (p, q) = fixture().await?;
    let policy = PgRelationshipPolicy::new(p.clone(), TestAuthority);
    let initial = policy.authorize(q).await?;
    let r = block(&p, q, true, Uuid::new_v4()).await?;
    assert_eq!(r.state, ReceiptState::Succeeded);
    assert_eq!(policy.authorize(q).await, Err(RelationshipError::Denied));
    let reverse = PolicyQuery {
        actor: q.subject,
        subject: q.actor,
        ..q
    };
    assert_eq!(
        policy.authorize(reverse).await,
        Err(RelationshipError::Denied)
    );
    block(&p, q, false, Uuid::new_v4()).await?;
    let current = policy.authorize(q).await?;
    assert_ne!(initial.stamp(), current.stamp());
    assert_eq!(
        initial.ensure_current(q, current.stamp(), Utc::now()),
        Err(RelationshipError::Stale)
    );
    let mut tx = p.begin().await?;
    assert_eq!(
        policy.authorize_in(&mut tx, q, Some(initial.stamp())).await,
        Err(RelationshipError::Stale)
    );
    tx.rollback().await?;
    let n: i64 = sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
        .bind(q.event.uuid())
        .fetch_one(&p)
        .await?;
    assert_eq!(n, 2);
    Ok(())
}
#[tokio::test]
async fn duplicate_block_is_idempotent_and_reverse_block_survives() -> anyhow::Result<()> {
    let (p, q) = fixture().await?;
    let key = Uuid::new_v4();
    let first = block(&p, q, true, key).await?;
    let repeated = block(&p, q, true, key).await?;
    assert_eq!(first, repeated);
    let reverse = PolicyQuery {
        actor: q.subject,
        subject: q.actor,
        ..q
    };
    block(&p, reverse, true, Uuid::new_v4()).await?;
    block(&p, q, false, Uuid::new_v4()).await?;
    assert_eq!(
        PgRelationshipPolicy::new(p.clone(), TestAuthority)
            .authorize(q)
            .await,
        Err(RelationshipError::Denied)
    );
    Ok(())
}
#[tokio::test]
async fn unknown_suspension_inherited_block_and_event_denials() -> anyhow::Result<()> {
    let (p, q) = fixture().await?;
    let policy = PgRelationshipPolicy::new(p.clone(), TestAuthority);
    for column in ["allowed", "inherited_block"] {
        let sql=format!("update relationship_test_authority set {column}=$3,revision=revision+1 where event_id=$1 and user_id=$2");
        sqlx::query(&sql)
            .bind(q.event.uuid())
            .bind(q.subject.uuid())
            .bind(column == "inherited_block")
            .execute(&p)
            .await?;
        assert_eq!(policy.authorize(q).await, Err(RelationshipError::Denied));
        sqlx::query(&sql)
            .bind(q.event.uuid())
            .bind(q.subject.uuid())
            .bind(column == "allowed")
            .execute(&p)
            .await?;
    }
    sqlx::query("delete from relationship_test_authority where event_id=$1 and user_id=$2")
        .bind(q.event.uuid())
        .bind(q.subject.uuid())
        .execute(&p)
        .await?;
    assert_eq!(
        policy.authorize(q).await,
        Err(RelationshipError::Unavailable)
    );
    assert_eq!(
        PgRelationshipPolicy::new(p.clone(), UnavailableAuthority)
            .authorize(q)
            .await,
        Err(RelationshipError::Unavailable)
    );
    Ok(())
}
#[tokio::test]
async fn self_safety_in_readonly_and_no_generic_private_grant() -> anyhow::Result<()> {
    let (p, q) = fixture().await?;
    let policy = PgRelationshipPolicy::new(p.clone(), TestAuthority);
    sqlx::query(
        "update concert_events set phase='READ_ONLY',revision=revision+1 where event_id=$1",
    )
    .bind(q.event.uuid())
    .execute(&p)
    .await?;
    assert!(policy
        .authorize(PolicyQuery {
            subject: q.actor,
            field: Field::SafetySelf,
            ..q
        })
        .await
        .is_ok());
    assert_eq!(
        policy
            .authorize(PolicyQuery {
                field: Field::Interaction,
                ..q
            })
            .await,
        Err(RelationshipError::Denied)
    );
    assert_eq!(
        policy
            .authorize(PolicyQuery {
                field: Field::PrivateHandle,
                ..q
            })
            .await,
        Err(RelationshipError::Denied)
    );
    block(&p, q, true, Uuid::new_v4()).await?;
    block(&p, q, false, Uuid::new_v4()).await?;
    Ok(())
}
#[tokio::test]
async fn transaction_fences_revocation_and_waiter_rechecks() -> anyhow::Result<()> {
    let (p, q) = fixture().await?;
    let policy = PgRelationshipPolicy::new(p.clone(), TestAuthority);
    let mut tx = p.begin().await?;
    policy.authorize_in(&mut tx, q, None).await?;
    let other = p.clone();
    let mut pending = tokio::spawn(async move { block(&other, q, true, Uuid::new_v4()).await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut pending)
            .await
            .is_err()
    );
    tx.commit().await?;
    pending.await??;
    assert_eq!(policy.authorize(q).await, Err(RelationshipError::Denied));
    Ok(())
}
#[tokio::test]
async fn missing_membership_foreign_event_and_failed_transaction_leave_no_receipts(
) -> anyhow::Result<()> {
    let (p, q) = fixture().await?;
    let policy = PgRelationshipPolicy::new(p.clone(), TestAuthority);
    let foreign = PolicyQuery {
        event: EventId::from_uuid(Uuid::new_v4()),
        ..q
    };
    assert_eq!(
        policy.authorize(foreign).await,
        Err(RelationshipError::Denied)
    );
    sqlx::query("delete from concert_participants where event_id=$1 and user_id=$2")
        .bind(q.event.uuid())
        .bind(q.actor.uuid())
        .execute(&p)
        .await?;
    assert_eq!(
        block(&p, q, true, Uuid::new_v4()).await,
        Err(CommandError::Denied)
    );
    let n: i64 =
        sqlx::query_scalar("select count(*) from concert_command_receipts where event_id=$1")
            .bind(q.event.uuid())
            .fetch_one(&p)
            .await?;
    assert_eq!(n, 0);
    Ok(())
}

use concert_application::relationship::social::*;
use concert_domain::{relationship::social::*, ParticipantKey};
use concert_service::relationship::{crypto::*, social::*};
struct TestSharing;
#[async_trait]
impl SocialEligibility for TestSharing {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        context: SharingContext,
        now: DateTime<Utc>,
    ) -> Result<FreshSharing, RelationshipError> {
        let row = sqlx::query(
            "select * from relationship_test_intro where event_id=$1 and intro_id=$2 for share",
        )
        .bind(context.event.uuid())
        .bind(context.intro_id)
        .fetch_optional(c)
        .await
        .map_err(|_| RelationshipError::Unavailable)?
        .ok_or(RelationshipError::Denied)?;
        if row.get::<Uuid, _>("requester_id") != context.requester.uuid()
            || row.get::<Uuid, _>("recipient_id") != context.recipient.uuid()
            || row.get::<i64, _>("revision") != context.intro_revision.get()
        {
            return Err(RelationshipError::Denied);
        }
        Ok(FreshSharing {
            context,
            checked_at: now,
            expires_at: row.get("expires_at"),
            allowed: row.get("allowed"),
        })
    }
}
fn handle_keys() -> HandleKeys {
    HandleKeys::new(
        ManagedHandleKey::new("test-key", &[71; 32]).unwrap(),
        vec![],
    )
    .unwrap()
}
fn social(p: &PgPool) -> SocialService<TestAuthority, TestSharing> {
    SocialService::new(p.clone(), TestAuthority, TestSharing, handle_keys(), 3600).unwrap()
}
fn actor(q: PolicyQuery) -> ParticipantKey {
    ParticipantKey {
        event_id: q.event,
        user_id: q.actor,
    }
}
fn recipient(q: PolicyQuery) -> ParticipantKey {
    ParticipantKey {
        event_id: q.event,
        user_id: q.subject,
    }
}
async fn social_fixture() -> anyhow::Result<(PgPool, PolicyQuery, CreateSharing)> {
    let (p, q) = fixture().await?;
    let service = social(&p);
    let platform = Platform::Instagram;
    service
        .set_handle(
            actor(q),
            platform,
            0,
            PrivateHandle::normalize(platform, "Requester.Handle").unwrap(),
            Uuid::new_v4(),
            &keys(),
        )
        .await?;
    service
        .set_handle(
            recipient(q),
            platform,
            0,
            PrivateHandle::normalize(platform, "Recipient.Handle").unwrap(),
            Uuid::new_v4(),
            &keys(),
        )
        .await?;
    let context = SharingContext {
        event: q.event,
        requester: q.actor,
        recipient: q.subject,
        intro_id: Uuid::new_v4(),
        intro_revision: Revision::initial(),
    };
    sqlx::query("insert into relationship_test_intro(event_id,intro_id,requester_id,recipient_id,revision,allowed,expires_at) values($1,$2,$3,$4,1,true,clock_timestamp()+interval '1 hour')").bind(q.event.uuid()).bind(context.intro_id).bind(q.actor.uuid()).bind(q.subject.uuid()).execute(&p).await?;
    Ok((
        p,
        q,
        CreateSharing {
            context,
            consents: vec![ConsentBinding {
                platform,
                requester_version: Revision::initial(),
                recipient_version: Revision::initial(),
                requester_consents: true,
            }],
        },
    ))
}
async fn approved(p: &PgPool, q: PolicyQuery, input: CreateSharing) -> anyhow::Result<Uuid> {
    let service = social(p);
    let receipt = service
        .create(actor(q), input, Uuid::new_v4(), &keys())
        .await?;
    let id = receipt.reference.unwrap().resource;
    let r = service
        .decide(
            recipient(q),
            id,
            SharingDecision {
                expected: Revision::initial(),
                approve: true,
                approved_platforms: vec![Platform::Instagram],
                recipient_consents: true,
            },
            Uuid::new_v4(),
            &keys(),
        )
        .await?;
    assert_eq!(r.state, ReceiptState::Succeeded);
    Ok(id)
}
#[tokio::test]
async fn social_private_ciphertext_reciprocal_reveal_and_idempotent_decision() -> anyhow::Result<()>
{
    let (p, q, input) = social_fixture().await?;
    let service = social(&p);
    let key = Uuid::new_v4();
    let request = service
        .create(actor(q), input.clone(), key, &keys())
        .await?;
    assert_eq!(
        request,
        service.create(actor(q), input, key, &keys()).await?
    );
    let id = request.reference.unwrap().resource;
    let n: i64 = sqlx::query_scalar("select count(*) from concert_social_grants where event_id=$1")
        .bind(q.event.uuid())
        .fetch_one(&p)
        .await?;
    assert_eq!(n, 0);
    assert!(service
        .reveal(actor(q), id, Platform::Instagram)
        .await
        .is_err());
    let decision = || SharingDecision {
        expected: Revision::initial(),
        approve: true,
        approved_platforms: vec![Platform::Instagram],
        recipient_consents: true,
    };
    let key = Uuid::new_v4();
    let a = service
        .decide(recipient(q), id, decision(), key, &keys())
        .await?;
    let b = service
        .decide(recipient(q), id, decision(), key, &keys())
        .await?;
    assert_eq!(a, b);
    let reveal = service.reveal(actor(q), id, Platform::Instagram).await?;
    assert_eq!(reveal.handle.expose_for_reveal(), "recipient.handle");
    assert_eq!(reveal.context.owner, q.subject);
    assert_eq!(
        service
            .reveal(recipient(q), id, Platform::Instagram)
            .await?
            .handle
            .expose_for_reveal(),
        "requester.handle"
    );
    assert!(!format!("{reveal:?}").contains("recipient.handle"));
    assert_eq!(PrivateReveal::CACHE_CONTROL, "no-store");
    let bytes: Vec<Vec<u8>> =
        sqlx::query_scalar("select ciphertext from concert_social_handles where event_id=$1")
            .bind(q.event.uuid())
            .fetch_all(&p)
            .await?;
    for b in bytes {
        assert!(!b.windows(9).any(|v| v == b"recipient" || v == b"requester"));
    }
    let outbox: String = sqlx::query_scalar(
        "select string_agg(row_to_json(o)::text,'') from concert_outbox o where event_id=$1",
    )
    .bind(q.event.uuid())
    .fetch_one(&p)
    .await?;
    assert!(!outbox.contains(".handle"));
    Ok(())
}
#[tokio::test]
async fn social_no_consent_wrong_actor_and_stale_versions_deny() -> anyhow::Result<()> {
    let (p, q, mut input) = social_fixture().await?;
    let service = social(&p);
    input.consents[0].requester_consents = false;
    assert!(service
        .create(actor(q), input.clone(), Uuid::new_v4(), &keys())
        .await
        .is_err());
    input.consents[0].requester_consents = true;
    input.consents[0].recipient_version = Revision::new(2)?;
    assert!(service
        .create(actor(q), input.clone(), Uuid::new_v4(), &keys())
        .await
        .is_err());
    input.consents[0].recipient_version = Revision::initial();
    let receipt = service
        .create(actor(q), input, Uuid::new_v4(), &keys())
        .await?;
    let id = receipt.reference.unwrap().resource;
    let bad = SharingDecision {
        expected: Revision::initial(),
        approve: true,
        approved_platforms: vec![Platform::Instagram],
        recipient_consents: false,
    };
    assert!(service
        .decide(recipient(q), id, bad, Uuid::new_v4(), &keys())
        .await
        .is_err());
    let decision = SharingDecision {
        expected: Revision::initial(),
        approve: true,
        approved_platforms: vec![Platform::Instagram],
        recipient_consents: true,
    };
    assert!(service
        .decide(actor(q), id, decision, Uuid::new_v4(), &keys())
        .await
        .is_err());
    let stranger = ParticipantKey {
        event_id: q.event,
        user_id: AuthUserId::from_uuid(Uuid::new_v4()),
    };
    assert!(service
        .reveal(stranger, id, Platform::Instagram)
        .await
        .is_err());
    assert!(service
        .reveal(
            ParticipantKey {
                event_id: EventId::from_uuid(Uuid::new_v4()),
                user_id: q.actor
            },
            id,
            Platform::Instagram
        )
        .await
        .is_err());
    assert!(service.reveal(actor(q), id, Platform::X).await.is_err());
    Ok(())
}
#[tokio::test]
async fn social_block_unblock_never_restores_prior_consent() -> anyhow::Result<()> {
    let (p, q, input) = social_fixture().await?;
    let id = approved(&p, q, input).await?;
    let service = social(&p);
    block(&p, q, true, Uuid::new_v4()).await?;
    assert_eq!(
        service
            .reveal(actor(q), id, Platform::Instagram)
            .await
            .unwrap_err(),
        RelationshipError::Denied
    );
    block(&p, q, false, Uuid::new_v4()).await?;
    assert_eq!(
        service
            .reveal(actor(q), id, Platform::Instagram)
            .await
            .unwrap_err(),
        RelationshipError::Denied
    );
    let n: i64 = sqlx::query_scalar(
        "select count(*) from concert_social_grants where event_id=$1 and state='ACTIVE'",
    )
    .bind(q.event.uuid())
    .fetch_one(&p)
    .await?;
    assert_eq!(n, 0);
    Ok(())
}
#[tokio::test]
async fn social_replacement_and_removal_leave_monotone_tombstones() -> anyhow::Result<()> {
    let (p, q, input) = social_fixture().await?;
    let id = approved(&p, q, input).await?;
    let service = social(&p);
    let platform = Platform::Instagram;
    service
        .set_handle(
            recipient(q),
            platform,
            1,
            PrivateHandle::normalize(platform, "new.handle").unwrap(),
            Uuid::new_v4(),
            &keys(),
        )
        .await?;
    assert!(service.reveal(actor(q), id, platform).await.is_err());
    service
        .remove_handle(
            recipient(q),
            platform,
            Revision::new(2)?,
            Uuid::new_v4(),
            &keys(),
        )
        .await?;
    let row=sqlx::query("select revision,ciphertext,key_id,nonce from concert_social_handles where event_id=$1 and user_id=$2 and platform='INSTAGRAM'").bind(q.event.uuid()).bind(q.subject.uuid()).fetch_one(&p).await?;
    assert_eq!(row.get::<i64, _>("revision"), 3);
    assert!(row.get::<Option<Vec<u8>>, _>("ciphertext").is_none());
    assert!(row.get::<Option<String>, _>("key_id").is_none());
    assert!(row.get::<Option<Vec<u8>>, _>("nonce").is_none());
    service
        .set_handle(
            recipient(q),
            platform,
            3,
            PrivateHandle::normalize(platform, "again.handle").unwrap(),
            Uuid::new_v4(),
            &keys(),
        )
        .await?;
    assert!(service.reveal(actor(q), id, platform).await.is_err());
    Ok(())
}
#[tokio::test]
async fn social_revoke_duplicate_is_atomic_and_readonly_safety_works() -> anyhow::Result<()> {
    let (p, q, input) = social_fixture().await?;
    let id = approved(&p, q, input).await?;
    let service = social(&p);
    sqlx::query(
        "update concert_events set phase='READ_ONLY',revision=revision+1 where event_id=$1",
    )
    .bind(q.event.uuid())
    .execute(&p)
    .await?;
    let key = Uuid::new_v4();
    let a = service
        .revoke(actor(q), id, Revision::new(2)?, key, &keys())
        .await?;
    assert_eq!(
        a,
        service
            .revoke(actor(q), id, Revision::new(2)?, key, &keys())
            .await?
    );
    assert!(service
        .reveal(actor(q), id, Platform::Instagram)
        .await
        .is_err());
    let n: i64 = sqlx::query_scalar(
        "select count(*) from concert_social_grants where event_id=$1 and state='ACTIVE'",
    )
    .bind(q.event.uuid())
    .fetch_one(&p)
    .await?;
    assert_eq!(n, 0);
    Ok(())
}
#[tokio::test]
async fn social_expired_claim_and_inherited_restriction_revision_deny() -> anyhow::Result<()> {
    let (p, q, input) = social_fixture().await?;
    let ctx = input.context;
    let id = approved(&p, q, input).await?;
    let service = social(&p);
    sqlx::query("update relationship_test_intro set expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and intro_id=$2").bind(q.event.uuid()).bind(ctx.intro_id).execute(&p).await?;
    assert!(service
        .reveal(actor(q), id, Platform::Instagram)
        .await
        .is_err());
    sqlx::query("update relationship_test_intro set expires_at=clock_timestamp()+interval '1 hour' where event_id=$1 and intro_id=$2").bind(q.event.uuid()).bind(ctx.intro_id).execute(&p).await?;
    sqlx::query("update relationship_test_authority set inherited_block=true,revision=revision+1 where event_id=$1 and user_id=$2").bind(q.event.uuid()).bind(q.subject.uuid()).execute(&p).await?;
    assert!(service
        .reveal(actor(q), id, Platform::Instagram)
        .await
        .is_err());
    sqlx::query("update relationship_test_authority set inherited_block=false,revision=revision+1 where event_id=$1 and user_id=$2").bind(q.event.uuid()).bind(q.subject.uuid()).execute(&p).await?;
    assert!(service
        .reveal(actor(q), id, Platform::Instagram)
        .await
        .is_err());
    Ok(())
}
#[tokio::test]
async fn social_concurrent_approval_block_and_duplicate_decision() -> anyhow::Result<()> {
    let (p, q, input) = social_fixture().await?;
    let service = social(&p);
    let r = service
        .create(actor(q), input, Uuid::new_v4(), &keys())
        .await?;
    let id = r.reference.unwrap().resource;
    let other = p.clone();
    let approval = tokio::spawn(async move {
        social(&other)
            .decide(
                recipient(q),
                id,
                SharingDecision {
                    expected: Revision::initial(),
                    approve: true,
                    approved_platforms: vec![Platform::Instagram],
                    recipient_consents: true,
                },
                Uuid::new_v4(),
                &keys(),
            )
            .await
    });
    block(&p, q, true, Uuid::new_v4()).await?;
    let _ = approval.await?;
    assert!(service
        .reveal(actor(q), id, Platform::Instagram)
        .await
        .is_err());
    let n: i64 = sqlx::query_scalar(
        "select count(*) from concert_social_grants where event_id=$1 and state='ACTIVE'",
    )
    .bind(q.event.uuid())
    .fetch_one(&p)
    .await?;
    assert_eq!(n, 0);
    Ok(())
}
#[tokio::test]
async fn social_outbox_failure_rolls_back_handle_and_receipt() -> anyhow::Result<()> {
    let (p, q, input) = social_fixture().await?;
    let id = approved(&p, q, input).await?;
    let service = social(&p);
    sqlx::query("insert into relationship_test_outbox_fail(event_id) values($1)")
        .bind(q.event.uuid())
        .execute(&p)
        .await?;
    let before: i64 =
        sqlx::query_scalar("select count(*) from concert_command_receipts where event_id=$1")
            .bind(q.event.uuid())
            .fetch_one(&p)
            .await?;
    assert!(service
        .set_handle(
            recipient(q),
            Platform::Instagram,
            1,
            PrivateHandle::normalize(Platform::Instagram, "must.rollback").unwrap(),
            Uuid::new_v4(),
            &keys()
        )
        .await
        .is_err());
    let after: i64 =
        sqlx::query_scalar("select count(*) from concert_command_receipts where event_id=$1")
            .bind(q.event.uuid())
            .fetch_one(&p)
            .await?;
    assert_eq!(before, after);
    assert_eq!(
        service
            .reveal(actor(q), id, Platform::Instagram)
            .await?
            .handle
            .expose_for_reveal(),
        "recipient.handle"
    );
    Ok(())
}
#[tokio::test]
async fn social_key_rotation_aad_and_corruption_fail_closed() -> anyhow::Result<()> {
    let (p, q, input) = social_fixture().await?;
    let id = approved(&p, q, input).await?;
    let rotated = SocialService::new(
        p.clone(),
        TestAuthority,
        TestSharing,
        HandleKeys::new(
            ManagedHandleKey::new("new-key", &[81; 32])?,
            vec![ManagedHandleKey::new("test-key", &[71; 32])?],
        )?,
        3600,
    )?;
    assert_eq!(
        rotated
            .reveal(actor(q), id, Platform::Instagram)
            .await?
            .handle
            .expose_for_reveal(),
        "recipient.handle"
    );
    let no_old = SocialService::new(
        p.clone(),
        TestAuthority,
        TestSharing,
        HandleKeys::new(ManagedHandleKey::new("new-key", &[81; 32])?, vec![])?,
        3600,
    )?;
    assert_eq!(
        no_old
            .reveal(actor(q), id, Platform::Instagram)
            .await
            .unwrap_err(),
        RelationshipError::Unavailable
    );
    // Ciphertext swap preserves SQL shape and versions but AAD binds owner identity.
    let mut corruption = p.begin().await?;
    sqlx::query("select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '1 minute')::text,true)").execute(&mut *corruption).await?;
    sqlx::query("update concert_social_handles set ciphertext=(select ciphertext from concert_social_handles where event_id=$1 and user_id=$2 and platform='INSTAGRAM'),revision=revision+1 where event_id=$1 and user_id=$3 and platform='INSTAGRAM'").bind(q.event.uuid()).bind(q.actor.uuid()).bind(q.subject.uuid()).execute(&mut *corruption).await?;
    corruption.commit().await?;
    assert!(rotated
        .reveal(actor(q), id, Platform::Instagram)
        .await
        .is_err());
    Ok(())
}

#[tokio::test]
async fn social_http_allowlist_no_store_success_and_denial() -> anyhow::Result<()> {
    use concert_service::relationship::http::reveal_response;
    use http_body_util::BodyExt;
    let (p, q, input) = social_fixture().await?;
    let id = approved(&p, q, input).await?;
    let service = social(&p);
    let response = reveal_response(&service, actor(q), id, Platform::Instagram).await;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let value: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await?.to_bytes())?;
    let mut actual: Vec<_> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    actual.sort_unstable();
    assert_eq!(
        actual,
        vec![
            "expires_at",
            "grant_revision",
            "handle",
            "handle_version",
            "owner_id",
            "platform",
            "request_id"
        ]
    );
    assert_eq!(value["handle"], "recipient.handle");
    let denied = reveal_response(&service, actor(q), Uuid::new_v4(), Platform::Instagram).await;
    assert_eq!(denied.status(), axum::http::StatusCode::NOT_FOUND);
    assert_eq!(denied.headers()["cache-control"], "no-store");
    let bytes = denied.into_body().collect().await?.to_bytes();
    let text = std::str::from_utf8(&bytes)?;
    assert!(!text.contains(".handle"));
    assert!(!text.contains(&q.subject.uuid().to_string()));
    Ok(())
}
#[tokio::test]
async fn social_actual_grant_expiry_denies_reveal() -> anyhow::Result<()> {
    let (p, q, input) = social_fixture().await?;
    let service = SocialService::new(p.clone(), TestAuthority, TestSharing, handle_keys(), 1)?;
    let request = service
        .create(actor(q), input, Uuid::new_v4(), &keys())
        .await?;
    let id = request.reference.unwrap().resource;
    service
        .decide(
            recipient(q),
            id,
            SharingDecision {
                expected: Revision::initial(),
                approve: true,
                approved_platforms: vec![Platform::Instagram],
                recipient_consents: true,
            },
            Uuid::new_v4(),
            &keys(),
        )
        .await?;
    assert!(service
        .reveal(actor(q), id, Platform::Instagram)
        .await
        .is_ok());
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    assert_eq!(
        service
            .reveal(actor(q), id, Platform::Instagram)
            .await
            .unwrap_err(),
        RelationshipError::Denied
    );
    Ok(())
}
#[tokio::test]
async fn social_concurrent_grant_revoke_has_no_false_success() -> anyhow::Result<()> {
    let (p, q, input) = social_fixture().await?;
    let service = social(&p);
    let request = service
        .create(actor(q), input, Uuid::new_v4(), &keys())
        .await?;
    let id = request.reference.unwrap().resource;
    let other = p.clone();
    let approval = tokio::spawn(async move {
        social(&other)
            .decide(
                recipient(q),
                id,
                SharingDecision {
                    expected: Revision::initial(),
                    approve: true,
                    approved_platforms: vec![Platform::Instagram],
                    recipient_consents: true,
                },
                Uuid::new_v4(),
                &keys(),
            )
            .await
    });
    let revoked = service
        .revoke(actor(q), id, Revision::initial(), Uuid::new_v4(), &keys())
        .await;
    let approved = approval.await?;
    match revoked {
        Ok(receipt) => {
            assert_eq!(receipt.state, ReceiptState::Succeeded);
            assert!(approved.is_err());
        }
        Err(CommandError::Conflict) => {
            assert!(approved.is_ok());
            service
                .revoke(actor(q), id, Revision::new(2)?, Uuid::new_v4(), &keys())
                .await?;
        }
        Err(other) => panic!("unexpected coarse outcome {other:?}"),
    }
    assert!(service
        .reveal(actor(q), id, Platform::Instagram)
        .await
        .is_err());
    let n: i64 = sqlx::query_scalar(
        "select count(*) from concert_social_grants where event_id=$1 and state='ACTIVE'",
    )
    .bind(q.event.uuid())
    .fetch_one(&p)
    .await?;
    assert_eq!(n, 0);
    Ok(())
}

#[tokio::test]
async fn social_sql_terminal_states_and_immutable_scope_reject_resurrection() -> anyhow::Result<()>
{
    let (p, q, input) = social_fixture().await?;
    let id = approved(&p, q, input).await?;
    let service = social(&p);
    let metadata = service.own_handles(actor(q)).await?;
    assert_eq!(metadata.len(), 2);
    assert!(metadata
        .iter()
        .any(|h| h.platform == Platform::X && !h.configured && h.version == 0));
    assert!(metadata
        .iter()
        .any(|h| h.platform == Platform::Instagram && h.configured && h.version == 1));
    assert!(sqlx::query("update concert_social_handles set user_id=$3,revision=revision+1 where event_id=$1 and user_id=$2").bind(q.event.uuid()).bind(q.actor.uuid()).bind(Uuid::new_v4()).execute(&p).await.is_err());
    service
        .revoke(actor(q), id, Revision::new(2)?, Uuid::new_v4(), &keys())
        .await?;
    assert!(sqlx::query("update concert_social_requests set state='APPROVED',revision=revision+1 where event_id=$1 and request_id=$2").bind(q.event.uuid()).bind(id).execute(&p).await.is_err());
    assert!(sqlx::query("update concert_social_grants set state='ACTIVE',revision=revision+1 where event_id=$1 and request_id=$2").bind(q.event.uuid()).bind(id).execute(&p).await.is_err());
    Ok(())
}

struct ShortAuthority;
#[async_trait]
impl FencedRelationshipAuthority for ShortAuthority {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        q: PolicyQuery,
        now: DateTime<Utc>,
    ) -> Result<FreshAuthority, RelationshipError> {
        let mut proof = TestAuthority.check_in(c, q, now).await?;
        proof.valid_until = now + Duration::milliseconds(100);
        Ok(proof)
    }
}
#[tokio::test]
async fn authority_expiry_at_commit_rolls_back_effect_receipt_and_outbox() -> anyhow::Result<()> {
    let (p, q) = fixture().await?;
    sqlx::query("insert into relationship_test_outbox_delay(event_id) values($1)")
        .bind(q.event.uuid())
        .execute(&p)
        .await?;
    let service = SocialService::new(p.clone(), ShortAuthority, TestSharing, handle_keys(), 3600)?;
    assert!(service
        .set_handle(
            actor(q),
            Platform::Instagram,
            0,
            PrivateHandle::normalize(Platform::Instagram, "expire.before.commit").unwrap(),
            Uuid::new_v4(),
            &keys()
        )
        .await
        .is_err());
    for table in [
        "concert_social_handles",
        "concert_command_receipts",
        "concert_outbox",
        "concert_relationship_pairs",
    ] {
        let sql = format!("select count(*) from {table} where event_id=$1");
        let n: i64 = sqlx::query_scalar(&sql)
            .bind(q.event.uuid())
            .fetch_one(&p)
            .await?;
        assert_eq!(n, 0, "{table}");
    }
    Ok(())
}

#[tokio::test]
async fn actor_only_guard_has_no_pair_side_effect_or_target_permission() -> anyhow::Result<()> {
    let (p, q) = fixture().await?;
    let policy = PgRelationshipPolicy::new(p.clone(), TestAuthority);
    let mut tx = p.begin().await?;
    let result = policy.authorize_actor_in(&mut tx, actor(q)).await?;
    assert_eq!(result.key(), actor(q));
    assert!(result.valid_until() > result.checked_at());
    let pairs: i64 =
        sqlx::query_scalar("select count(*) from concert_relationship_pairs where event_id=$1")
            .bind(q.event.uuid())
            .fetch_one(&mut *tx)
            .await?;
    assert_eq!(pairs, 0);
    tx.commit().await?;
    let mut tx = p.begin().await?;
    assert_eq!(
        PgRelationshipPolicy::new(p.clone(), UnavailableAuthority)
            .authorize_actor_in(&mut tx, actor(q))
            .await,
        Err(RelationshipError::Unavailable)
    );
    tx.rollback().await?;
    Ok(())
}
#[tokio::test]
async fn actor_guard_then_root_avoids_root_then_self_pair_deadlock() -> anyhow::Result<()> {
    let (p, q) = fixture().await?;
    sqlx::query("insert into relationship_test_roots(event_id) values($1)")
        .bind(q.event.uuid())
        .execute(&p)
        .await?;
    let mut writer = p.begin().await?;
    sqlx::query("select event_id from concert_events where event_id=$1 for share")
        .bind(q.event.uuid())
        .fetch_one(&mut *writer)
        .await?;
    sqlx::query("select event_id from relationship_test_roots where event_id=$1 for update")
        .bind(q.event.uuid())
        .fetch_one(&mut *writer)
        .await?;
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let other = p.clone();
    let reader = tokio::spawn(async move {
        let policy = PgRelationshipPolicy::new(other.clone(), TestAuthority);
        let mut tx = other.begin().await?;
        policy.authorize_actor_in(&mut tx, actor(q)).await?;
        let _ = ready_tx.send(());
        sqlx::query("select event_id from relationship_test_roots where event_id=$1 for share")
            .bind(q.event.uuid())
            .fetch_one(&mut *tx)
            .await?;
        policy.authorize_in(&mut tx, q, None).await?;
        tx.commit().await?;
        Ok::<_, anyhow::Error>(())
    });
    ready_rx.await?;
    let policy = PgRelationshipPolicy::new(p.clone(), TestAuthority);
    let completion = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        policy.authorize_in(
            &mut writer,
            PolicyQuery {
                subject: q.actor,
                field: Field::SafetySelf,
                ..q
            },
            None,
        ),
    )
    .await;
    if completion.is_err() {
        reader.abort();
        writer.rollback().await?;
        anyhow::bail!("actor guard acquired a self pair before the resource root")
    }
    completion??;
    writer.commit().await?;
    reader.await??;
    Ok(())
}
