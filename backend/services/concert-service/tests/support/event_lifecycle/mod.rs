//! Owned fixtures only. These authorities/owners are deliberately not production adapters.
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::{commands::*, event_lifecycle::Actor};
use concert_domain::event_lifecycle::{Retention, Step};
use concert_service::event_lifecycle::*;
use sqlx::{PgConnection, PgPool, Row};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use uuid::Uuid;
fn db(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
pub async fn pool() -> PgPool {
    let url = std::env::var("MCR_BE41_DATABASE_URL")
        .expect("run owned event_lifecycle_pg.py --native-binary");
    assert!(url.starts_with("postgresql:///mcr_be41_fixture?host=/"));
    let owner = std::env::var("MCR_BE41_FIXTURE_OWNER").unwrap();
    assert_eq!(owner.len(), 32);
    let pool = PgPool::connect(&url).await.unwrap();
    let actual: String = sqlx::query_scalar("select owner_token from be41_fixture_owner")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(owner, actual);
    sqlx::raw_sql("CREATE TABLE IF NOT EXISTS be41_fixture_authority(event_id uuid,actor_id uuid,allowed boolean NOT NULL,revision bigint NOT NULL DEFAULT 1,PRIMARY KEY(event_id,actor_id));CREATE TABLE IF NOT EXISTS be41_fixture_applied(event_id uuid,operation_id uuid PRIMARY KEY,step text,applied integer NOT NULL);").execute(&pool).await.unwrap();
    pool
}
pub fn keys() -> FingerprintKeys {
    FingerprintKeys::new(
        SigningKey::new("be41_fixture", &[0x41; 32]).unwrap(),
        vec![],
    )
    .unwrap()
}
pub async fn actor(pool: &PgPool) -> Actor {
    let actor = Actor {
        actor: Uuid::new_v4(),
        event: Uuid::new_v4(),
    };
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'BE41 owned fixture','LIVE')").bind(actor.event).bind(format!("be41-{}",actor.event)).execute(pool).await.unwrap();
    sqlx::query("insert into be41_fixture_authority(event_id,actor_id,allowed) values($1,$2,true)")
        .bind(actor.event)
        .bind(actor.actor)
        .execute(pool)
        .await
        .unwrap();
    actor
}
pub struct Authority {
    pub delay: Duration,
}
#[async_trait]
impl FencedLifecycleAuthority for Authority {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        q: AuthorityQuery,
        now: DateTime<Utc>,
    ) -> Result<AuthorityFence, CommandError> {
        let row=sqlx::query("select allowed,revision from be41_fixture_authority where event_id=$1 and actor_id=$2 for share").bind(q.actor.event).bind(q.actor.actor).fetch_optional(c).await.map_err(db)?.ok_or(CommandError::Denied)?;
        if !row.get::<bool, _>("allowed") {
            return Err(CommandError::Denied);
        }
        Ok(AuthorityFence {
            query: q,
            auth_revision: row.get("revision"),
            role_revision: row.get("revision"),
            policy_revision: 1,
            checked_at: now,
            expires_at: now + Duration::seconds(10),
        })
    }
    async fn retention_in(
        &self,
        _: &mut PgConnection,
        _: Actor,
        at: DateTime<Utc>,
    ) -> Result<Retention, CommandError> {
        Ok(Retention {
            policy_revision: 1,
            closed_at: at,
            cleanup_after: at + self.delay,
        })
    }
}
#[derive(Clone)]
pub struct Owners {
    pub fail_snapshot: Arc<AtomicBool>,
    pub wrong_receipt: Arc<AtomicBool>,
}
impl Owners {
    pub fn healthy() -> Self {
        Self {
            fail_snapshot: Arc::new(AtomicBool::new(false)),
            wrong_receipt: Arc::new(AtomicBool::new(false)),
        }
    }
}
#[async_trait]
impl LifecycleOwners for Owners {
    async fn apply_in(
        &self,
        c: &mut PgConnection,
        key: EffectKey,
        retention: Retention,
    ) -> Result<Applied, CommandError> {
        if key.step == Step::Cleanup {
            let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
                .fetch_one(&mut *c)
                .await
                .map_err(db)?;
            assert!(now >= retention.cleanup_after);
        }
        sqlx::query("insert into be41_fixture_applied(event_id,operation_id,step,applied) values($1,$2,$3,1)")
            .bind(key.event).bind(key.operation_id()).bind(key.step.name()).execute(c).await.map_err(db)?;
        if key.step == Step::Snapshot && self.fail_snapshot.load(Ordering::SeqCst) {
            return Err(CommandError::Unavailable);
        }
        Ok(Applied {
            key,
            receipt: if self.wrong_receipt.load(Ordering::SeqCst) {
                Uuid::nil()
            } else {
                key.operation_id()
            },
            revision: 1,
        })
    }
}
pub async fn count(pool: &PgPool, event: Uuid, step: &str) -> i64 {
    sqlx::query_scalar("select coalesce(sum(applied),0)::bigint from be41_fixture_applied where event_id=$1 and step=$2").bind(event).bind(step).fetch_one(pool).await.unwrap()
}
