//! The observer is deliberately synthetic. It checks retained hook timing and
//! rollback, not actual Auth/Friend Track or canonical settlement.
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::commands::{FingerprintKeys, SigningKey};
use concert_domain::{
    relationship::{social::*, FreshAuthority, PolicyQuery, RelationshipError},
    AuthUserId, EventId, ParticipantKey, Revision,
};
use concert_service::relationship::{
    crypto::{HandleKeys, ManagedHandleKey},
    social::{SocialEffect, SocialService, SocialTransactionHooks, UnavailableSocialEligibility},
    FencedRelationshipAuthority,
};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

struct RegisteredFixtureAuthority;
#[async_trait]
impl FencedRelationshipAuthority for RegisteredFixtureAuthority {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        q: PolicyQuery,
        now: DateTime<Utc>,
    ) -> Result<FreshAuthority, RelationshipError> {
        let registered: Option<String> =
            sqlx::query_scalar("select current_setting('mcr.fixture_social_registered',true)")
                .fetch_one(&mut *c)
                .await
                .map_err(|_| RelationshipError::Unavailable)?;
        let xid: String = sqlx::query_scalar("select pg_current_xact_id()::text")
            .fetch_one(&mut *c)
            .await
            .map_err(|_| RelationshipError::Unavailable)?;
        if registered.as_deref() != Some(&xid) {
            return Err(RelationshipError::Unavailable);
        }
        Ok(FreshAuthority {
            query: q,
            checked_at: now,
            valid_until: now + Duration::seconds(5),
            revision: Revision::initial(),
            actor_allowed: true,
            subject_allowed: true,
            inherited_block: false,
        })
    }
}
struct Hooks {
    actor: Uuid,
    reject_register: bool,
    reject_success: bool,
}
#[async_trait]
impl SocialTransactionHooks for Hooks {
    async fn register(&self, c: &mut PgConnection) -> Result<(), RelationshipError> {
        if self.reject_register {
            return Err(RelationshipError::Unavailable);
        }
        sqlx::query(
            "select set_config('mcr.fixture_social_registered',pg_current_xact_id()::text,true)",
        )
        .execute(&mut *c)
        .await
        .map_err(|_| RelationshipError::Unavailable)?;
        Ok(())
    }
    async fn succeeded(
        &self,
        c: &mut PgConnection,
        e: SocialEffect,
    ) -> Result<(), RelationshipError> {
        if !e.projection {
            let revision:Option<i64>=sqlx::query_scalar("select revision from concert_social_handles where user_id=$1 and revision=$2 and public.concert_social_own_visible_xmin(xmin)")
                .bind(self.actor).bind(e.revision).fetch_optional(&mut *c).await.map_err(|_|RelationshipError::Unavailable)?;
            assert_eq!(
                revision,
                Some(e.revision),
                "must run AFTER actual retained mutation"
            );
        }
        sqlx::query("insert into fixture_social_hook_events(actor,full_xid,operation,resource,revision,projection) values($1,pg_current_xact_id()::text,$2,$3,$4,$5)")
            .bind(self.actor).bind(e.operation).bind(e.resource).bind(e.revision).bind(e.projection).execute(&mut *c).await.map_err(|_|RelationshipError::Unavailable)?;
        if self.reject_success {
            return Err(RelationshipError::Unavailable);
        }
        Ok(())
    }
}
async fn setup() -> (PgPool, ParticipantKey) {
    let p = crate::owned::pool().await;
    let actor = ParticipantKey {
        event_id: EventId::from_uuid(Uuid::new_v4()),
        user_id: AuthUserId::from_uuid(Uuid::new_v4()),
    };
    sqlx::query("create table if not exists fixture_social_hook_events(actor uuid,full_xid text,operation text,resource uuid,revision bigint,projection boolean)").execute(&p).await.unwrap();
    sqlx::query(
        "insert into concert_events(event_id,slug,title,phase) values($1,$2,'Hook timing','LIVE')",
    )
    .bind(actor.event_id.uuid())
    .bind(actor.event_id.uuid().to_string())
    .execute(&p)
    .await
    .unwrap();
    sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(actor.event_id.uuid()).bind(actor.user_id.uuid()).execute(&p).await.unwrap();
    (p, actor)
}
fn service(
    p: &PgPool,
    actor: ParticipantKey,
    reject_register: bool,
    reject_success: bool,
) -> SocialService<RegisteredFixtureAuthority, UnavailableSocialEligibility> {
    SocialService::new(
        p.clone(),
        RegisteredFixtureAuthority,
        UnavailableSocialEligibility,
        HandleKeys::new(ManagedHandleKey::new("fixture", &[71; 32]).unwrap(), vec![]).unwrap(),
        3600,
    )
    .unwrap()
    .with_transaction_hooks(Arc::new(Hooks {
        actor: actor.user_id.uuid(),
        reject_register,
        reject_success,
    }))
}
fn keys() -> FingerprintKeys {
    FingerprintKeys::new(SigningKey::new("fixture", &[37; 32]).unwrap(), vec![]).unwrap()
}
#[tokio::test]
#[ignore = "owned social_live native runner required"]
async fn hook_register_precedes_policy_and_verified_replay_has_safe_projection() {
    let (p, a) = setup().await;
    let service = service(&p, a, false, false);
    let key = Uuid::new_v4();
    let keys = keys();
    for _ in 0..2 {
        service
            .set_handle(
                a,
                Platform::Instagram,
                0,
                PrivateHandle::normalize(Platform::Instagram, "never.in.witness").unwrap(),
                key,
                &keys,
            )
            .await
            .unwrap();
    }
    let rows=sqlx::query("select full_xid,projection,revision from fixture_social_hook_events where actor=$1 order by ctid").bind(a.user_id.uuid()).fetch_all(&p).await.unwrap();
    assert_eq!(rows.len(), 3);
    assert!(!rows[0].get::<bool, _>("projection"));
    assert!(rows[1].get::<bool, _>("projection"));
    assert!(rows[2].get::<bool, _>("projection"));
    assert_eq!(
        rows[0].get::<String, _>("full_xid"),
        rows[1].get::<String, _>("full_xid")
    );
    assert_ne!(
        rows[0].get::<String, _>("full_xid"),
        rows[2].get::<String, _>("full_xid")
    );
    assert!(rows.iter().all(|r| r.get::<i64, _>("revision") == 1));
    let handles = service.own_handles(a).await.unwrap();
    assert_eq!(handles.iter().filter(|h| h.configured).count(), 1);
    service
        .remove_handle(
            a,
            Platform::Instagram,
            Revision::initial(),
            Uuid::new_v4(),
            &keys,
        )
        .await
        .unwrap();
    assert!(!service
        .own_handles(a)
        .await
        .unwrap()
        .iter()
        .any(|h| h.configured));
    p.close().await;
}
#[tokio::test]
#[ignore = "owned social_live native runner required"]
async fn failed_registration_or_success_sink_rolls_back_retained_effect_receipt_and_witness() {
    let (p, a) = setup().await;
    for (register, success) in [(true, false), (false, true)] {
        assert!(service(&p, a, register, success)
            .set_handle(
                a,
                Platform::X,
                0,
                PrivateHandle::normalize(Platform::X, "secret_handle").unwrap(),
                Uuid::new_v4(),
                &keys()
            )
            .await
            .is_err());
        for table in ["concert_social_handles", "concert_command_receipts"] {
            let column = if table == "concert_social_handles" {
                "user_id"
            } else {
                "actor_id"
            };
            let n: i64 =
                sqlx::query_scalar(&format!("select count(*) from {table} where {column}=$1"))
                    .bind(a.user_id.uuid())
                    .fetch_one(&p)
                    .await
                    .unwrap();
            assert_eq!(n, 0);
        }
        let n: i64 =
            sqlx::query_scalar("select count(*) from fixture_social_hook_events where actor=$1")
                .bind(a.user_id.uuid())
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(n, 0);
    }
    p.close().await;
}
