//! Adapter persistence tests require a runner-owned real PostgreSQL cluster.
#[path = "event_lifecycle_support/migrations.rs"]
mod migrations;
#[path = "support/event_lifecycle/mod.rs"]
mod support;
use chrono::Duration;
use concert_application::{commands::*, event_lifecycle::CloseCommand};
use concert_domain::event_lifecycle::{JobState, Step};
use concert_service::event_lifecycle::*;
use std::sync::atomic::Ordering;
use support::*;
use uuid::Uuid;
fn start() -> CloseCommand {
    CloseCommand::Start {
        expected_revision: 1,
        reason: "Owned fixture closure".into(),
    }
}
fn resume(id: Uuid, rev: i64) -> CloseCommand {
    CloseCommand::Resume {
        job_id: id,
        expected_revision: rev,
        reason: "Explicit fixture retry".into(),
    }
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture"]
async fn event_lifecycle_idempotent_start_receipt_is_admission_only() {
    let pool = pool().await;
    let actor = actor(&pool).await;
    let key = Uuid::new_v4();
    let service = PgEventLifecycle::new(
        pool.clone(),
        Authority {
            delay: Duration::zero(),
        },
        Owners::healthy(),
    );
    let first = service.command(actor, key, start(), &keys()).await.unwrap();
    assert_eq!(first.state, ReceiptState::Succeeded);
    assert_eq!(first.result_status, Some(200));
    let jobid = first.reference.unwrap().resource;
    assert_eq!(jobid, start_job_id(actor, key));
    let replay = service.command(actor, key, start(), &keys()).await.unwrap();
    assert_eq!(first, replay);
    let job = service.job(actor, jobid).await.unwrap();
    assert_eq!(job.state, JobState::Pending);
    assert_eq!(job.next(), Some(Step::FreezeMural));
    let phase: String = sqlx::query_scalar("select phase from concert_events where event_id=$1")
        .bind(actor.event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(phase, "READ_ONLY");
    let outbox: i64 = sqlx::query_scalar("select count(*) from concert_outbox where receipt_id=$1")
        .bind(first.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(outbox, 1);
    let other = service
        .command(actor, Uuid::new_v4(), start(), &keys())
        .await
        .unwrap();
    assert_eq!(other.failure, Some(SafeFailure::InvalidTransition));
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture"]
async fn event_lifecycle_partial_failure_rolls_back_and_explicit_resume_skips_success() {
    let pool = pool().await;
    let actor = actor(&pool).await;
    let owners = Owners::healthy();
    owners.fail_snapshot.store(true, Ordering::SeqCst);
    let service = PgEventLifecycle::new(
        pool.clone(),
        Authority {
            delay: Duration::zero(),
        },
        owners.clone(),
    );
    let start_key = Uuid::new_v4();
    let admission = service
        .command(actor, start_key, start(), &keys())
        .await
        .unwrap();
    let jobid = admission.reference.unwrap().resource;
    service.run_next(actor, jobid).await.unwrap();
    let failed = service.run_next(actor, jobid).await.unwrap();
    assert_eq!(failed.state, JobState::PartialFailure);
    assert_eq!(count(&pool, actor.event, "FREEZE_MURAL").await, 1);
    assert_eq!(count(&pool, actor.event, "SNAPSHOT").await, 0);
    assert_eq!(
        service
            .command(actor, start_key, start(), &keys())
            .await
            .unwrap(),
        admission
    );
    assert_eq!(
        service.job(actor, jobid).await.unwrap(),
        failed,
        "START replay must not retry a failed job"
    );
    assert_eq!(
        service
            .command(actor, start_key, resume(jobid, failed.revision), &keys())
            .await
            .unwrap_err(),
        CommandError::Conflict
    );
    assert_eq!(
        service.run_next(actor, jobid).await.unwrap_err(),
        CommandError::InvalidState
    );
    let stale = service
        .command(
            actor,
            Uuid::new_v4(),
            resume(jobid, failed.revision - 1),
            &keys(),
        )
        .await
        .unwrap();
    assert_eq!(stale.failure, Some(SafeFailure::VersionConflict));
    owners.fail_snapshot.store(false, Ordering::SeqCst);
    let retrykey = Uuid::new_v4();
    let request = resume(jobid, failed.revision);
    let accepted = service
        .command(actor, retrykey, request.clone(), &keys())
        .await
        .unwrap();
    assert_eq!(accepted.reference.unwrap().revision, failed.revision + 1);
    assert_eq!(
        accepted,
        service
            .command(actor, retrykey, request, &keys())
            .await
            .unwrap()
    );
    while service.job(actor, jobid).await.unwrap().state != JobState::Completed {
        service.run_next(actor, jobid).await.unwrap();
    }
    assert_eq!(count(&pool, actor.event, "FREEZE_MURAL").await, 1);
    assert_eq!(count(&pool, actor.event, "SNAPSHOT").await, 1);
    let phase: String = sqlx::query_scalar("select phase from concert_events where event_id=$1")
        .bind(actor.event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(phase, "ARCHIVED");
    service.run_next(actor, jobid).await.unwrap();
    assert_eq!(count(&pool, actor.event, "CLEANUP").await, 1);
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture"]
async fn event_lifecycle_retention_wait_and_current_authority_fail_closed() {
    let pool = pool().await;
    let actor = actor(&pool).await;
    let key = Uuid::new_v4();
    let service = PgEventLifecycle::new(
        pool.clone(),
        Authority {
            delay: Duration::hours(1),
        },
        Owners::healthy(),
    );
    let first = service.command(actor, key, start(), &keys()).await.unwrap();
    let job = first.reference.unwrap().resource;
    for _ in 0..5 {
        service.run_next(actor, job).await.unwrap();
    }
    let waiting = service.run_next(actor, job).await.unwrap();
    assert_eq!(waiting.next(), Some(Step::Cleanup));
    assert_ne!(waiting.state, JobState::Completed);
    assert_eq!(count(&pool, actor.event, "CLEANUP").await, 0);
    assert_eq!(waiting, service.run_next(actor, job).await.unwrap());
    sqlx::query(
        "update be41_fixture_authority set allowed=false,revision=revision+1 where event_id=$1",
    )
    .bind(actor.event)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        service
            .command(actor, key, start(), &keys())
            .await
            .unwrap_err(),
        CommandError::Denied
    );
    assert_eq!(
        service.job(actor, job).await.unwrap_err(),
        CommandError::Denied
    );
    assert_eq!(
        service.run_next(actor, job).await.unwrap_err(),
        CommandError::Denied
    );
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture"]
async fn event_lifecycle_concurrent_workers_and_wrong_receipt() {
    let pool = pool().await;
    let actor = actor(&pool).await;
    let owners = Owners::healthy();
    let service = PgEventLifecycle::new(
        pool.clone(),
        Authority {
            delay: Duration::zero(),
        },
        owners.clone(),
    );
    let job = service
        .command(actor, Uuid::new_v4(), start(), &keys())
        .await
        .unwrap()
        .reference
        .unwrap()
        .resource;
    let (a, b) = tokio::join!(service.run_next(actor, job), service.run_next(actor, job));
    a.unwrap();
    b.unwrap();
    assert_eq!(count(&pool, actor.event, "FREEZE_MURAL").await, 1);
    assert_eq!(count(&pool, actor.event, "SNAPSHOT").await, 1);
    owners.wrong_receipt.store(true, Ordering::SeqCst);
    let failed = service.run_next(actor, job).await.unwrap();
    assert_eq!(failed.state, JobState::PartialFailure);
    assert_eq!(count(&pool, actor.event, "EXPIRE_GRANTS").await, 0);
    let mut wrong = actor;
    wrong.actor = Uuid::new_v4();
    assert_eq!(
        service.job(wrong, job).await.unwrap_err(),
        CommandError::Denied
    );
}

#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture"]
async fn event_lifecycle_restart_seed() {
    let pool = pool().await;
    let actor = actor(&pool).await;
    let owners = Owners::healthy();
    owners.fail_snapshot.store(true, Ordering::SeqCst);
    let service = PgEventLifecycle::new(
        pool.clone(),
        Authority {
            delay: Duration::zero(),
        },
        owners,
    );
    let job = service
        .command(actor, Uuid::new_v4(), start(), &keys())
        .await
        .unwrap()
        .reference
        .unwrap()
        .resource;
    service.run_next(actor, job).await.unwrap();
    let failed = service.run_next(actor, job).await.unwrap();
    assert_eq!(failed.state, JobState::PartialFailure);
    sqlx::raw_sql("CREATE TABLE be41_fixture_restart(event_id uuid,actor_id uuid,job_id uuid);")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("insert into be41_fixture_restart values($1,$2,$3)")
        .bind(actor.event)
        .bind(actor.actor)
        .bind(job)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
}
#[tokio::test]
#[ignore = "runner must restart PostgreSQL after restart_seed"]
async fn event_lifecycle_restart_resume() {
    use sqlx::Row;
    let pool = pool().await;
    let row = sqlx::query("select * from be41_fixture_restart")
        .fetch_one(&pool)
        .await
        .unwrap();
    let actor = concert_application::event_lifecycle::Actor {
        actor: row.get("actor_id"),
        event: row.get("event_id"),
    };
    let job: Uuid = row.get("job_id");
    let service = PgEventLifecycle::new(
        pool.clone(),
        Authority {
            delay: Duration::zero(),
        },
        Owners::healthy(),
    );
    let failed = service.job(actor, job).await.unwrap();
    assert_eq!(failed.state, JobState::PartialFailure);
    assert_eq!(failed.next(), Some(Step::Snapshot));
    assert_eq!(count(&pool, actor.event, "FREEZE_MURAL").await, 1);
    assert_eq!(count(&pool, actor.event, "SNAPSHOT").await, 0);
    service
        .command(actor, Uuid::new_v4(), resume(job, failed.revision), &keys())
        .await
        .unwrap();
    while service.job(actor, job).await.unwrap().state != JobState::Completed {
        service.run_next(actor, job).await.unwrap();
    }
    assert_eq!(count(&pool, actor.event, "FREEZE_MURAL").await, 1);
    assert_eq!(count(&pool, actor.event, "SNAPSHOT").await, 1);
}

struct ActualPrivacy(Owners);
#[async_trait::async_trait]
impl LifecycleOwners for ActualPrivacy {
    async fn apply_in(
        &self,
        c: &mut sqlx::PgConnection,
        key: EffectKey,
        retention: concert_domain::event_lifecycle::Retention,
    ) -> Result<Applied, CommandError> {
        match key.step {
            Step::ExpireGrants => privacy::expire_grants_in(c, key).await,
            Step::ExpireBeacons => {
                privacy::expire_beacons_in(c, key).await?;
                self.0.apply_in(c, key, retention).await
            }
            Step::Cleanup => {
                privacy::cleanup_privacy_in(c, key, retention).await?;
                self.0.apply_in(c, key, retention).await
            }
            _ => self.0.apply_in(c, key, retention).await,
        }
    }
}
async fn seed_privacy(pool: &sqlx::PgPool, actor: concert_application::event_lifecycle::Actor) {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("set local mcr.relationship_authority_until='2100-01-01T00:00:00Z'")
        .execute(&mut *tx)
        .await
        .unwrap();
    let peer = Uuid::new_v4();
    let request = Uuid::new_v4();
    sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(actor.event).bind(actor.actor).execute(&mut *tx).await.unwrap();
    sqlx::query("insert into concert_social_handles(event_id,user_id,platform,key_id,nonce,ciphertext) values($1,$2,'X','fixture',decode(repeat('00',12),'hex'),decode(repeat('00',17),'hex'))").bind(actor.event).bind(actor.actor).execute(&mut *tx).await.unwrap();
    sqlx::query("insert into concert_social_requests(event_id,request_id,requester_id,recipient_id,intro_id,intro_revision,authority_revision,relationship_revision,state,expires_at) values($1,$2,$3,$4,$5,1,1,1,'PENDING',clock_timestamp()+interval '1 hour')").bind(actor.event).bind(request).bind(actor.actor).bind(peer).bind(Uuid::new_v4()).execute(&mut *tx).await.unwrap();
    sqlx::query("update concert_social_requests set state='APPROVED',revision=revision+1 where event_id=$1 and request_id=$2").bind(actor.event).bind(request).execute(&mut *tx).await.unwrap();
    sqlx::query("insert into concert_social_bindings values($1,$2,'X',1,1,true,true)")
        .bind(actor.event)
        .bind(request)
        .execute(&mut *tx)
        .await
        .unwrap();
    for (owner, viewer) in [(actor.actor, peer), (peer, actor.actor)] {
        sqlx::query("insert into concert_social_grants(event_id,request_id,owner_id,viewer_id,platform,handle_version,peer_handle_version,state,expires_at) values($1,$2,$3,$4,'X',1,1,'ACTIVE',(select expires_at from concert_social_requests where event_id=$1 and request_id=$2))").bind(actor.event).bind(request).bind(owner).bind(viewer).execute(&mut *tx).await.unwrap();
    }
    sqlx::query("insert into concert_meeting_settings(event_id,event_cutoff) values($1,clock_timestamp()+interval '1 hour')").bind(actor.event).execute(&mut *tx).await.unwrap();
    sqlx::query("insert into concert_meeting_beacons(beacon_id,event_id,owner_id,place_id,map_revision,place_revision,selected_catalog_revision,instructions,identification,audience,expires_at) values($1,$2,$3,$4,1,1,1,'Owned private fixture description','Owned private fixture identification',$5,clock_timestamp()+interval '120 seconds')")
        .bind(Uuid::new_v4()).bind(actor.event).bind(actor.actor).bind(Uuid::new_v4()).bind(serde_json::json!({"kind":"EXPLICIT_USERS","user_ids":[peer]})).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture"]
async fn event_lifecycle_actual_privacy_sql_expires_and_cleans_only_owned_event() {
    let pool = pool().await;
    let actor = actor(&pool).await;
    let foreign = support::actor(&pool).await;
    seed_privacy(&pool, actor).await;
    seed_privacy(&pool, foreign).await;
    let service = PgEventLifecycle::new(
        pool.clone(),
        Authority {
            delay: Duration::zero(),
        },
        ActualPrivacy(Owners::healthy()),
    );
    let job = service
        .command(actor, Uuid::new_v4(), start(), &keys())
        .await
        .unwrap()
        .reference
        .unwrap()
        .resource;
    for _ in 0..3 {
        service.run_next(actor, job).await.unwrap();
    }
    // Unexpected late writes must not silently renew event-close retention.
    let mut late = pool.begin().await.unwrap();
    sqlx::query("set local mcr.relationship_authority_until='2100-01-01T00:00:00Z'")
        .execute(&mut *late)
        .await
        .unwrap();
    sqlx::query("update concert_social_handles set revision=revision+1,ciphertext=decode(repeat('01',17),'hex') where event_id=$1").bind(actor.event).execute(&mut *late).await.unwrap();
    late.commit().await.unwrap();
    let active: i64 = sqlx::query_scalar(
        "select count(*) from concert_social_grants where event_id=$1 and state='ACTIVE'",
    )
    .bind(actor.event)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(active, 0);
    service.run_next(actor, job).await.unwrap();
    let beacons: i64 = sqlx::query_scalar(
        "select count(*) from concert_meeting_beacons where event_id=$1 and state='ACTIVE'",
    )
    .bind(actor.event)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(beacons, 0);
    while service.job(actor, job).await.unwrap().state != JobState::Completed {
        service.run_next(actor, job).await.unwrap();
    }
    let retained:i64=sqlx::query_scalar("select (select count(*) from concert_social_requests where event_id=$1)+(select count(*) from concert_meeting_beacons where event_id=$1)+(select count(*) from concert_social_handles where event_id=$1 and ciphertext is not null)").bind(actor.event).fetch_one(&pool).await.unwrap();
    assert_eq!(retained, 0);
    let untouched: i64 = sqlx::query_scalar(
        "select count(*) from concert_social_grants where event_id=$1 and state='ACTIVE'",
    )
    .bind(foreign.event)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(untouched, 2);
    let receipts: i64 = sqlx::query_scalar(
        "select count(*) from concert_closure_privacy_effects where event_id=$1",
    )
    .bind(actor.event)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(receipts, 3);
}

#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture"]
async fn event_lifecycle_full_migration_ledger_and_reapply() -> anyhow::Result<()> {
    let pool = pool().await;
    migrations::qualify(&pool).await
}

/// Route-level proof over the published admin/closure template: the real HTTP
/// handler admits one durable job, an identical same-key request is transport
/// replay, every later closure write is refused, and ARCHIVED is only reached
/// after each required step commits.
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture"]
async fn event_lifecycle_admin_closure_route_admits_read_only_and_refuses_rewrite() {
    use axum::{
        body::{to_bytes, Bytes},
        extract::{Path, State},
        http::{header, HeaderMap, StatusCode},
        response::{IntoResponse, Response},
        routing::post,
        Router,
    };
    use concert_application::event_lifecycle::Actor;
    use concert_service::event_lifecycle::http::{close_response, job_response};
    use std::sync::Arc;
    use tower::ServiceExt;

    #[derive(Clone)]
    struct Route {
        service: Arc<PgEventLifecycle<Authority, Owners>>,
        actor: Actor,
        keys: Arc<FingerprintKeys>,
    }
    async fn post_closure(
        State(route): State<Route>,
        _: Path<Uuid>,
        headers: HeaderMap,
        body: Bytes,
    ) -> Response {
        let Some(key) = headers
            .get("idempotency-key")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| Uuid::parse_str(value).ok())
        else {
            return (StatusCode::BAD_REQUEST, "missing idempotency key").into_response();
        };
        close_response(&route.service, route.actor, key, &body, &route.keys).await
    }
    async fn get_job(State(route): State<Route>, Path((_, job_id)): Path<(Uuid, Uuid)>) -> Response {
        job_response(&route.service, route.actor, job_id).await
    }
    let pool = pool().await;
    let actor = actor(&pool).await;
    let service = Arc::new(PgEventLifecycle::new(
        pool.clone(),
        Authority {
            delay: Duration::zero(),
        },
        Owners::healthy(),
    ));
    let keys = Arc::new(keys());
    let route = Route {
        service: service.clone(),
        actor,
        keys: keys.clone(),
    };
    async fn fetch_job(route: &Route, job: Uuid) -> Response {
        job_response(&route.service, route.actor, job).await
    }
    let router = Router::new()
        .route(
            "/api/concert/events/:event_id/admin/closure",
            post(post_closure),
        )
        .route(
            "/api/concert/events/:event_id/admin/closure/:job_id",
            axum::routing::get(get_job),
        )
        .with_state(route.clone());
    let body = serde_json::json!({
        "confirmation": "CLOSE_EVENT",
        "expected_revision": 1,
        "reason": "Route proof closure"
    })
    .to_string();
    let call = |key: Uuid, body: String| {
        axum::http::Request::builder()
            .method("POST")
            .uri(format!("/api/concert/events/{}/admin/closure", actor.event))
            .header(header::CONTENT_TYPE, "application/json")
            .header("idempotency-key", key.to_string())
            .body(axum::body::Body::from(body))
            .unwrap()
    };
    let key = Uuid::new_v4();
    let admitted = router
        .clone()
        .oneshot(call(key, body.clone()))
        .await
        .unwrap();
    assert_eq!(admitted.status(), StatusCode::OK);
    assert_eq!(admitted.headers()[header::CACHE_CONTROL], "no-store");
    let admitted = to_bytes(admitted.into_body(), usize::MAX).await.unwrap();
    let job: serde_json::Value = serde_json::from_slice(&admitted).unwrap();
    let job_id: Uuid = job["job_id"].as_str().unwrap().parse().unwrap();
    assert_eq!(job["job_id"], serde_json::json!(start_job_id(actor, key)));
    assert_eq!(job["state"], "PENDING");
    let phase: String = sqlx::query_scalar("select phase from concert_events where event_id=$1")
        .bind(actor.event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(phase, "READ_ONLY");

    // Identical same-key request is transport replay of the admission.
    let replay = router
        .clone()
        .oneshot(call(key, body.clone()))
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = to_bytes(replay.into_body(), usize::MAX).await.unwrap();
    assert_eq!(admitted, replay);

    // A second closure write after READ_ONLY must be refused.
    let refused = router
        .clone()
        .oneshot(call(Uuid::new_v4(), body.clone()))
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::CONFLICT);

    // Status reads stay available and never execute a step.
    let status = fetch_job(&route, job_id).await;
    assert_eq!(status.status(), StatusCode::OK);
    let status = to_bytes(status.into_body(), usize::MAX).await.unwrap();
    let status: serde_json::Value = serde_json::from_slice(&status).unwrap();
    assert_eq!(status["state"], "PENDING");
    assert_eq!(
        fetch_job(&route, Uuid::new_v4()).await.status(),
        StatusCode::NOT_FOUND
    );

    // Every required step commits before ARCHIVED; the route then refuses.
    let mut job = service.job(actor, job_id).await.unwrap();
    for _ in 0..7 {
        if job.state == JobState::Completed {
            break;
        }
        job = service.run_next(actor, job_id).await.unwrap();
        assert_ne!(job.state, JobState::PartialFailure);
    }
    assert_eq!(job.state, JobState::Completed);
    let phase: String = sqlx::query_scalar("select phase from concert_events where event_id=$1")
        .bind(actor.event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(phase, "ARCHIVED");
    assert_eq!(
        router
            .clone()
            .oneshot(call(Uuid::new_v4(), body))
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
}

