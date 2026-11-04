//! Explicit synthetic privilege and owner sources for durable-engine qualification.
//! These are NOT production role, reviewed-retention, mural or archive adapters.
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::{
    commands::CommandError,
    event_cleanup::*,
    event_lifecycle::{Actor, CloseCommand},
};
use concert_domain::event_lifecycle::{Retention, Step};
use concert_service::{event_cleanup_live::*, event_lifecycle::*};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};
use tokio::sync::Notify;
use uuid::Uuid;
fn db(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
pub async fn pool() -> PgPool {
    let socket = std::env::var("MCR_EVENT_CLEANUP_SOCKET").expect("owned run.py required");
    assert!(socket.starts_with("/private/tmp/mcr-event-cleanup-"));
    let options = sqlx::postgres::PgConnectOptions::new()
        .host(&socket)
        .port(55446)
        .database("mcr_event_cleanup_fixture")
        .username("mcr_event_cleanup_fixture");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await
        .unwrap();
    let actual: String = sqlx::query_scalar("select owner_token from event_cleanup_fixture_marker")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(actual, std::env::var("MCR_EVENT_CLEANUP_OWNER").unwrap());
    pool
}
pub struct Authority {
    pub milliseconds: i64,
}
#[async_trait]
impl FencedLifecycleAuthority for Authority {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        q: AuthorityQuery,
        now: DateTime<Utc>,
    ) -> Result<AuthorityFence, CommandError> {
        let row=sqlx::query("select allowed,revision from event_cleanup_fixture_authority where event_id=$1 and actor_id=$2 for share").bind(q.actor.event).bind(q.actor.actor).fetch_optional(c).await.map_err(db)?.ok_or(CommandError::Denied)?;
        if !row.get::<bool, _>("allowed") {
            return Err(CommandError::Denied);
        }
        Ok(AuthorityFence {
            query: q,
            auth_revision: row.get("revision"),
            role_revision: row.get("revision"),
            policy_revision: 1,
            checked_at: now,
            expires_at: now + Duration::milliseconds(self.milliseconds),
        })
    }
    async fn retention_in(
        &self,
        c: &mut PgConnection,
        _: Actor,
        closed_at: DateTime<Utc>,
    ) -> Result<Retention, CommandError> {
        let revision: i64 = sqlx::query_scalar(
            "select revision from event_cleanup_fixture_policy where singleton=true for share",
        )
        .fetch_one(c)
        .await
        .map_err(db)?;
        Ok(Retention {
            policy_revision: revision,
            closed_at,
            cleanup_after: closed_at + Duration::hours(1),
        })
    }
}
struct ReadOnly;
#[async_trait]
impl LifecycleOwners for ReadOnly {
    async fn apply_in(
        &self,
        c: &mut PgConnection,
        key: EffectKey,
        _: Retention,
    ) -> Result<Applied, CommandError> {
        concert_service::event_lifecycle::read_only::apply_read_only_in(c, key).await
    }
}
pub async fn seed(pool: &PgPool, count: i64) -> (Actor, Uuid) {
    let actor = Actor {
        actor: Uuid::new_v4(),
        event: Uuid::new_v4(),
    };
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Cleanup engine fixture','LIVE')").bind(actor.event).bind(actor.event.to_string()).execute(pool).await.unwrap();
    sqlx::query("insert into event_cleanup_fixture_authority(event_id,actor_id,allowed,revision) values($1,$2,true,1)").bind(actor.event).bind(actor.actor).execute(pool).await.unwrap();
    let keys = concert_application::commands::FingerprintKeys::new(
        concert_application::commands::SigningKey::new("event_cleanup_fixture", &[0x46; 32])
            .unwrap(),
        vec![],
    )
    .unwrap();
    let service = PgEventLifecycle::new(
        pool.clone(),
        Authority {
            milliseconds: 10000,
        },
        ReadOnly,
    );
    let receipt = service
        .command(
            actor,
            Uuid::new_v4(),
            CloseCommand::Start {
                expected_revision: 1,
                reason: "Owned durable-engine fixture".into(),
            },
            &keys,
        )
        .await
        .unwrap();
    let job = receipt.reference.unwrap().resource;
    for id in 1..=count {
        sqlx::query("insert into event_cleanup_fixture_items(event_id,id,payload) values($1,$2,'private fixture location recipient handle')").bind(actor.event).bind(id).execute(pool).await.unwrap();
    }
    (actor, job)
}
#[derive(Default)]
pub struct OwnerFixture {
    /// 1: fail after SQL, 2: pending after SQL, 3: fail verification, 4: pause.
    pub mode: AtomicU8,
    pub entered: Notify,
    pub release: Notify,
}
#[async_trait]
impl LocalOwner for OwnerFixture {
    fn owner(&self) -> Owner {
        Owner::Mural
    }
    async fn apply_in(
        &self,
        c: &mut PgConnection,
        request: &BatchRequest,
    ) -> Result<Outcome, CommandError> {
        let after = request
            .cursor()
            .map(|c| i64::from_be_bytes(c.bytes().try_into().unwrap()))
            .unwrap_or(0);
        let ids:Vec<i64>=sqlx::query_scalar("select id from event_cleanup_fixture_items where event_id=$1 and id>$2 and payload is not null order by id limit $3 for update")
            .bind(request.scope().event).bind(after).bind(i64::from(request.limit())).fetch_all(&mut *c).await.map_err(db)?;
        for id in &ids {
            sqlx::query(
                "update event_cleanup_fixture_items set payload=null where event_id=$1 and id=$2",
            )
            .bind(request.scope().event)
            .bind(id)
            .execute(&mut *c)
            .await
            .map_err(db)?;
        }
        let mode = self.mode.load(Ordering::SeqCst);
        if mode == 1 {
            return Err(CommandError::Unavailable);
        }
        if mode == 2 {
            return Ok(Outcome::Pending);
        }
        if mode == 4 {
            self.entered.notify_one();
            self.release.notified().await;
        }
        let more:bool=sqlx::query_scalar("select exists(select 1 from event_cleanup_fixture_items where event_id=$1 and payload is not null)").bind(request.scope().event).fetch_one(&mut *c).await.map_err(db)?;
        let receipt = Receipt {
            binding: request.binding(),
            operation: request.operation(),
            id: Uuid::new_v4(),
            revision: 1,
            processed: ids.len().try_into().unwrap(),
        };
        sqlx::query("insert into event_cleanup_fixture_effects(operation_id,event_id,binding,receipt_id,processed,complete) values($1,$2,$3,$4,$5,$6)")
            .bind(receipt.operation).bind(request.scope().event).bind(receipt.binding.to_vec()).bind(receipt.id).bind(receipt.processed as i16).bind(!more).execute(c).await.map_err(db)?;
        if more {
            Ok(Outcome::Page {
                receipt,
                next: Cursor::new(ids.last().unwrap().to_be_bytes().to_vec()).unwrap(),
            })
        } else {
            Ok(Outcome::Converged { receipt })
        }
    }
    async fn verify_in(
        &self,
        c: &mut PgConnection,
        request: &BatchRequest,
        receipt: Receipt,
    ) -> Result<(), CommandError> {
        use sqlx::TransactionManager;
        assert!(sqlx::postgres::PgTransactionManager::get_transaction_depth(c) > 0);
        if self.mode.load(Ordering::SeqCst) == 3 {
            return Err(CommandError::Unavailable);
        }
        let row = sqlx::query(
            "select * from event_cleanup_fixture_effects where operation_id=$1 for share",
        )
        .bind(request.operation())
        .fetch_one(&mut *c)
        .await
        .map_err(db)?;
        if row.get::<Uuid, _>("event_id") != request.scope().event
            || row.get::<Vec<u8>, _>("binding") != request.binding()
            || row.get::<Uuid, _>("receipt_id") != receipt.id
            || row.get::<i16, _>("processed") != receipt.processed as i16
        {
            return Err(CommandError::InvalidState);
        }
        if row.get::<bool, _>("complete") {
            let remains:bool=sqlx::query_scalar("select exists(select 1 from event_cleanup_fixture_items where event_id=$1 and payload is not null)").bind(request.scope().event).fetch_one(c).await.map_err(db)?;
            if remains {
                return Err(CommandError::InvalidState);
            }
        }
        Ok(())
    }
}
pub fn runtime(pool: &PgPool, owner: Arc<OwnerFixture>, millis: i64) -> CleanupRuntime {
    CleanupRuntime::new(
        pool.clone(),
        Some(Arc::new(Authority {
            milliseconds: millis,
        })),
        Owners::new(vec![owner]).unwrap(),
        2,
    )
    .unwrap()
}
pub async fn remaining(pool: &PgPool, event: Uuid) -> i64 {
    sqlx::query_scalar("select count(*) from event_cleanup_fixture_items where event_id=$1 and payload is not null").bind(event).fetch_one(pool).await.unwrap()
}
pub async fn parent_pending(pool: &PgPool, job: Uuid) {
    let value: serde_json::Value =
        sqlx::query_scalar("select progress from concert_closure_jobs where job_id=$1")
            .bind(job)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(value["steps"][1]["state"], "PENDING");
    let effects: i64 = sqlx::query_scalar(
        "select count(*) from concert_closure_effects where job_id=$1 and step='FREEZE_MURAL'",
    )
    .bind(job)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(effects, 0);
    let success: i64 = sqlx::query_scalar(
        "select count(*) from concert_closure_audit where job_id=$1 and outcome='SUCCEEDED'",
    )
    .bind(job)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(success, 0);
}
