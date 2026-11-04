//! Canonical local rows; explicit synthetic authority, retention and preceding
//! step effects. No production privilege, archive or remote convergence claim.
use super::*;
use concert_domain::event_lifecycle::Job;
use concert_service::event_cleanup_live::local::{BeaconExpiryOwner, PrivateRetentionOwner};

pub struct LocalAuthority {
    pub relationship: bool,
}
#[async_trait]
impl FencedLifecycleAuthority for LocalAuthority {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        q: AuthorityQuery,
        now: DateTime<Utc>,
    ) -> Result<AuthorityFence, CommandError> {
        let fence = Authority {
            milliseconds: 20000,
        }
        .check_in(c, q, now)
        .await?;
        if self.relationship {
            sqlx::query("select set_config('mcr.relationship_authority_until',$1,true)")
                .bind(fence.expires_at.to_rfc3339())
                .execute(c)
                .await
                .map_err(db)?;
        }
        Ok(fence)
    }
    async fn retention_in(
        &self,
        c: &mut PgConnection,
        a: Actor,
        closed: DateTime<Utc>,
    ) -> Result<Retention, CommandError> {
        Authority {
            milliseconds: 20000,
        }
        .retention_in(c, a, closed)
        .await
    }
}
pub async fn fences(c: &mut PgConnection) {
    sqlx::query("select set_config('mcr.lifecycle_authority_until',(clock_timestamp()+interval '30 seconds')::text,true),set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '30 seconds')::text,true)").execute(c).await.unwrap();
}
pub fn owner(own: Owner) -> Arc<dyn LocalOwner> {
    match own {
        Owner::Beacons => Arc::new(BeaconExpiryOwner),
        Owner::PrivateRetention => Arc::new(PrivateRetentionOwner),
        _ => panic!("local owner"),
    }
}
pub fn run(pool: &PgPool, own: Arc<dyn LocalOwner>, relationship: bool) -> CleanupRuntime {
    CleanupRuntime::new(
        pool.clone(),
        Some(Arc::new(LocalAuthority { relationship })),
        Owners::new(vec![own]).unwrap(),
        2,
    )
    .unwrap()
}
pub struct Fixture {
    pub actor: Actor,
    pub job: Uuid,
    pub user: Uuid,
    pub beacons: Vec<Uuid>,
    pub media: Uuid,
}
pub async fn seed(
    pool: &PgPool,
    step: Step,
    due: bool,
    beacon_count: usize,
    social: bool,
) -> Fixture {
    let actor = Actor {
        actor: Uuid::new_v4(),
        event: Uuid::new_v4(),
    };
    let user = Uuid::new_v4();
    let peer = Uuid::new_v4();
    let media = Uuid::new_v4();
    let job_id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    fences(&mut tx).await;
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Native local owner fixture','LIVE')").bind(actor.event).bind(actor.event.to_string()).execute(&mut *tx).await.unwrap();
    sqlx::query("insert into event_cleanup_fixture_authority values($1,$2,true,1)")
        .bind(actor.event)
        .bind(actor.actor)
        .execute(&mut *tx)
        .await
        .unwrap();
    for u in [user, peer] {
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(actor.event).bind(u).execute(&mut *tx).await.unwrap();
    }
    sqlx::query("insert into concert_meeting_settings(event_id,event_cutoff) values($1,clock_timestamp()+interval '1 hour')").bind(actor.event).execute(&mut *tx).await.unwrap();
    let mut beacons = vec![];
    for _ in 0..beacon_count {
        let beacon = Uuid::new_v4();
        beacons.push(beacon);
        sqlx::query("insert into concert_meeting_beacons(beacon_id,event_id,owner_id,place_id,map_revision,place_revision,selected_catalog_revision,instructions,identification,audience,expires_at) values($1,$2,$3,$4,1,1,1,'Private meeting instructions','Private identification',$5,clock_timestamp()+interval '10 minutes')")
            .bind(beacon).bind(actor.event).bind(user).bind(Uuid::new_v4()).bind(serde_json::json!({"kind":"EXPLICIT_USERS","user_ids":[peer]})).execute(&mut *tx).await.unwrap();
    }
    if let Some(beacon) = beacons.first() {
        sqlx::query("insert into concert_media(media_id,event_id,owner_id,purpose,content_type,declared_bytes,input_sha256,policy_event_revision,policy_relationship_revision,policy_authority_revision,state,sanitized,width,height,target_id) values($1,$2,$3,'BEACON','image/png',1,decode(repeat('01',32),'hex'),1,1,1,'READY',decode('01','hex'),1,1,$4)")
            .bind(media).bind(actor.event).bind(user).bind(beacon).execute(&mut *tx).await.unwrap();
        sqlx::query("insert into concert_meeting_beacon_photos(event_id,beacon_id,owner_id,media_id,beacon_revision,media_revision,permission_acknowledged,expires_at) select event_id,beacon_id,owner_id,$2,revision,1,true,expires_at from concert_meeting_beacons where beacon_id=$1")
            .bind(beacon).bind(media).execute(&mut *tx).await.unwrap();
    }
    if social {
        for u in [user, peer] {
            sqlx::query("insert into concert_social_handles(event_id,user_id,platform,key_id,nonce,ciphertext) values($1,$2,'INSTAGRAM','fixture',decode(repeat('01',12),'hex'),decode(repeat('02',20),'hex'))").bind(actor.event).bind(u).execute(&mut *tx).await.unwrap();
        }
        let req = Uuid::new_v4();
        sqlx::query("insert into concert_social_requests(event_id,request_id,requester_id,recipient_id,intro_id,intro_revision,authority_revision,relationship_revision,state,expires_at) values($1,$2,$3,$4,$5,1,1,1,'PENDING',clock_timestamp()+interval '1 hour')").bind(actor.event).bind(req).bind(user).bind(peer).bind(Uuid::new_v4()).execute(&mut *tx).await.unwrap();
        sqlx::query("insert into concert_social_bindings values($1,$2,'INSTAGRAM',1,1,true,true)")
            .bind(actor.event)
            .bind(req)
            .execute(&mut *tx)
            .await
            .unwrap();
        for (o, v) in [(user, peer), (peer, user)] {
            sqlx::query("insert into concert_social_grants(event_id,request_id,owner_id,viewer_id,platform,handle_version,peer_handle_version,state,expires_at) select event_id,request_id,$3,$4,'INSTAGRAM',1,1,'ACTIVE',expires_at from concert_social_requests where event_id=$1 and request_id=$2").bind(actor.event).bind(req).bind(o).bind(v).execute(&mut *tx).await.unwrap();
        }
        sqlx::query("update concert_social_requests set state='REVOKED',revision=revision+1 where event_id=$1").bind(actor.event).execute(&mut *tx).await.unwrap();
        sqlx::query("update concert_social_grants set state='REVOKED',revision=revision+1 where event_id=$1").bind(actor.event).execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap(); // Photo binding validated while beacon is ACTIVE.
    let mut tx = pool.begin().await.unwrap();
    fences(&mut tx).await;
    if step == Step::Cleanup {
        sqlx::query("update concert_meeting_beacons set state='REVOKED',revision=revision+1 where event_id=$1").bind(actor.event).execute(&mut *tx).await.unwrap();
    }
    sqlx::query(
        "update concert_events set phase='READ_ONLY',revision=revision+1 where event_id=$1",
    )
    .bind(actor.event)
    .execute(&mut *tx)
    .await
    .unwrap();
    let mut job = Job::new(job_id, actor.event).unwrap();
    sqlx::query("insert into concert_closure_jobs(job_id,event_id,actor_id,start_key,original_event_revision,retention_revision,closed_at,cleanup_after,progress) values($1,$2,$3,$4,1,1,statement_timestamp()-make_interval(hours=>$6),statement_timestamp()+make_interval(hours=>1-$6),$5)")
        .bind(job_id).bind(actor.event).bind(actor.actor).bind(Uuid::new_v4()).bind(serde_json::to_value(&job).unwrap()).bind(if due {2i32}else{0i32}).execute(&mut *tx).await.unwrap();
    let applied = read_only::apply_read_only_in(
        &mut tx,
        EffectKey {
            event: actor.event,
            job: job_id,
            step: Step::ReadOnly,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "insert into concert_closure_effects values($1,$2,'READ_ONLY',$3,$4,clock_timestamp())",
    )
    .bind(actor.event)
    .bind(job_id)
    .bind(applied.receipt)
    .bind(applied.revision)
    .execute(&mut *tx)
    .await
    .unwrap();
    while job.next() != Some(step) {
        let s = job.next().unwrap();
        // Synthetic preceding step effect, never evidence for that owner.
        sqlx::query("insert into concert_closure_effects(event_id,job_id,step,owner_receipt,owner_revision) values($1,$2,$3,$4,1)").bind(actor.event).bind(job_id).bind(s.name()).bind(Uuid::new_v4()).execute(&mut *tx).await.unwrap();
        job.record(s, true).unwrap();
        sqlx::query("update concert_closure_jobs set revision=$2,progress=$3 where job_id=$1")
            .bind(job_id)
            .bind(job.revision)
            .bind(serde_json::to_value(&job).unwrap())
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();
    Fixture {
        actor,
        job: job_id,
        user,
        beacons,
        media,
    }
}
pub async fn counts(pool: &PgPool, f: &Fixture) -> (i64, i64) {
    let row=sqlx::query("select (select count(*) from concert_event_cleanup_owner_pages where job_id=$1) pages,(select count(*) from concert_event_cleanup_owner_effects e join concert_event_cleanup_owner_pages p using(operation_id) where p.job_id=$1) effects").bind(f.job).fetch_one(pool).await.unwrap();
    (row.get("pages"), row.get("effects"))
}

async fn beacon_rows(pool: &PgPool, event: Uuid) -> serde_json::Value {
    sqlx::query_scalar("select coalesce(jsonb_agg(to_jsonb(b)-ARRAY['state','revision'] order by beacon_id),'[]'::jsonb) from concert_meeting_beacons b where event_id=$1").bind(event).fetch_one(pool).await.unwrap()
}
async fn active(pool: &PgPool, event: Uuid) -> i64 {
    sqlx::query_scalar(
        "select count(*) from concert_meeting_beacons where event_id=$1 and state='ACTIVE'",
    )
    .bind(event)
    .fetch_one(pool)
    .await
    .unwrap()
}
#[tokio::test]
#[ignore = "owned fixture"]
async fn event_cleanup_local_beacons_preserve_guidance_photo_and_other_event() {
    let pool = pool().await;
    let f = seed(&pool, Step::ExpireBeacons, false, 3, false).await;
    let other = seed(&pool, Step::ExpireBeacons, false, 1, false).await;
    // An already TTL-expired ACTIVE source must still be revoked at close.
    let mut tx = pool.begin().await.unwrap();
    fences(&mut tx).await;
    sqlx::query("insert into concert_meeting_beacons(beacon_id,event_id,owner_id,place_id,map_revision,place_revision,selected_catalog_revision,instructions,identification,audience,expires_at) values($1,$2,$3,$4,1,1,1,'Expired private guidance','Expired identification',$5,clock_timestamp()+interval '200 milliseconds')")
        .bind(Uuid::new_v4()).bind(f.actor.event).bind(f.user).bind(Uuid::new_v4()).bind(serde_json::json!({"kind":"EXPLICIT_USERS","user_ids":[f.user]})).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    let before = beacon_rows(&pool, f.actor.event).await;
    let other_before = beacon_rows(&pool, other.actor.event).await;
    let photo: serde_json::Value = sqlx::query_scalar(
        "select to_jsonb(p) from concert_meeting_beacon_photos p where beacon_id=$1",
    )
    .bind(f.beacons[0])
    .fetch_one(&pool)
    .await
    .unwrap();
    let run = run(&pool, owner(Owner::Beacons), true);
    let first = run.run_owner(f.actor, f.job, Owner::Beacons).await.unwrap();
    assert_eq!((first.state, first.processed), (State::Pending, 2));
    let last = run.run_owner(f.actor, f.job, Owner::Beacons).await.unwrap();
    assert_eq!((last.state, last.processed), (State::Complete, 4));
    assert_eq!(
        run.run_owner(f.actor, f.job, Owner::Beacons).await.unwrap(),
        last
    );
    assert_eq!(active(&pool, f.actor.event).await, 0);
    assert_eq!(active(&pool, other.actor.event).await, 1);
    assert_eq!(before, beacon_rows(&pool, f.actor.event).await);
    assert_eq!(other_before, beacon_rows(&pool, other.actor.event).await);
    let after: serde_json::Value = sqlx::query_scalar(
        "select to_jsonb(p) from concert_meeting_beacon_photos p where beacon_id=$1",
    )
    .bind(f.beacons[0])
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(photo, after);
    assert_eq!(counts(&pool, &f).await, (2, 4));
    assert!(!run.ready(f.actor, f.job).await.unwrap());
    // World/community are still absent; no parent EXPIRE_BEACONS success.
    let state: String = sqlx::query_scalar(
        "select progress->'steps'->4->>'state' from concert_closure_jobs where job_id=$1",
    )
    .bind(f.job)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, "PENDING");
}

#[tokio::test]
#[ignore = "owned fixture"]
async fn event_cleanup_local_retention_is_bounded_leaf_first_and_preserves_media_audit() {
    let pool = pool().await;
    let f = seed(&pool, Step::Cleanup, true, 3, true).await;
    let media: serde_json::Value =
        sqlx::query_scalar("select to_jsonb(m) from concert_media m where media_id=$1")
            .bind(f.media)
            .fetch_one(&pool)
            .await
            .unwrap();
    let effects: i64 =
        sqlx::query_scalar("select count(*) from concert_closure_effects where job_id=$1")
            .bind(f.job)
            .fetch_one(&pool)
            .await
            .unwrap();
    let run = run(&pool, owner(Owner::PrivateRetention), true);
    let mut previous = 0;
    for _ in 0..10 {
        let p = run
            .run_owner(f.actor, f.job, Owner::PrivateRetention)
            .await
            .unwrap();
        assert_ne!(p.state, State::PartialFailure);
        assert!(p.processed > previous && p.processed - previous <= 2);
        previous = p.processed;
        if p.state == State::Complete {
            break;
        }
    }
    assert_eq!(previous, 10);
    let final_page = run
        .run_owner(f.actor, f.job, Owner::PrivateRetention)
        .await
        .unwrap();
    assert_eq!(final_page.state, State::Complete);
    assert_eq!(final_page.processed, 10);
    assert_eq!(counts(&pool, &f).await, (5, 10));
    let remain: bool =
        sqlx::query_scalar("select concert_event_cleanup_owner_remaining($1,'PRIVATE_RETENTION')")
            .bind(f.actor.event)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!remain);
    let handles:i64=sqlx::query_scalar("select count(*) from concert_social_handles where event_id=$1 and revision=2 and key_id is null and nonce is null and ciphertext is null").bind(f.actor.event).fetch_one(&pool).await.unwrap();
    assert_eq!(handles, 2);
    let after: serde_json::Value =
        sqlx::query_scalar("select to_jsonb(m) from concert_media m where media_id=$1")
            .bind(f.media)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(media, after);
    let after: i64 =
        sqlx::query_scalar("select count(*) from concert_closure_effects where job_id=$1")
            .bind(f.job)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(effects, after);
    assert!(!run.ready(f.actor, f.job).await.unwrap()); // Separate media/Post/World/community owners remain mandatory.
    let mut tx = pool.begin().await.unwrap();
    fences(&mut tx).await;
    sqlx::query("update concert_social_handles set key_id='fixture',nonce=decode(repeat('01',12),'hex'),ciphertext=decode(repeat('02',20),'hex'),revision=revision+1 where event_id=$1 and user_id=$2").bind(f.actor.event).bind(f.user).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    assert!(run
        .run_owner(f.actor, f.job, Owner::PrivateRetention)
        .await
        .is_err()); // Current source reconstruction invalidates terminal replay.
    assert_eq!(counts(&pool, &f).await, (5, 10));
}

#[tokio::test]
#[ignore = "owned fixture"]
async fn event_cleanup_local_missing_fence_not_due_and_active_sources_refuse_effects() {
    let pool = pool().await;
    // Empty canonical sets still require a bound native page and current fences.
    for (step, own) in [
        (Step::ExpireBeacons, Owner::Beacons),
        (Step::Cleanup, Owner::PrivateRetention),
    ] {
        let f = seed(&pool, step, true, 0, false).await;
        let run = run(&pool, owner(own), true);
        for _ in 0..2 {
            let p = run.run_owner(f.actor, f.job, own).await.unwrap();
            assert_eq!((p.state, p.processed), (State::Complete, 0));
        }
        assert_eq!(counts(&pool, &f).await, (1, 0));
        assert!(!run.ready(f.actor, f.job).await.unwrap());
    }
    let f = seed(&pool, Step::ExpireBeacons, false, 3, false).await;
    let p = run(&pool, owner(Owner::Beacons), false)
        .run_owner(f.actor, f.job, Owner::Beacons)
        .await
        .unwrap();
    assert_eq!(p.state, State::PartialFailure);
    assert_eq!(active(&pool, f.actor.event).await, 3);
    assert_eq!(counts(&pool, &f).await, (0, 0));
    let f = seed(&pool, Step::Cleanup, false, 1, true).await;
    let p = run(&pool, owner(Owner::PrivateRetention), true)
        .run_owner(f.actor, f.job, Owner::PrivateRetention)
        .await
        .unwrap();
    assert_eq!((p.state, p.processed), (State::Pending, 0));
    assert_eq!(counts(&pool, &f).await, (0, 0));
    let f = seed(&pool, Step::Cleanup, true, 1, true).await;
    // New ACTIVE source after the fixture's earlier close, not a forged success.
    let mut tx = pool.begin().await.unwrap();
    fences(&mut tx).await;
    sqlx::query("insert into concert_meeting_beacons(beacon_id,event_id,owner_id,place_id,map_revision,place_revision,selected_catalog_revision,instructions,identification,audience,expires_at) values($1,$2,$3,$4,1,1,1,'Late fixture','Late fixture',$5,clock_timestamp()+interval '10 minutes')")
        .bind(Uuid::new_v4()).bind(f.actor.event).bind(f.user).bind(Uuid::new_v4()).bind(serde_json::json!({"kind":"EXPLICIT_USERS","user_ids":[f.user]})).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let p = run(&pool, owner(Owner::PrivateRetention), true)
        .run_owner(f.actor, f.job, Owner::PrivateRetention)
        .await
        .unwrap();
    assert_eq!(p.state, State::PartialFailure);
    assert_eq!(counts(&pool, &f).await, (0, 0));
}

struct FailureOwner {
    inner: Arc<dyn LocalOwner>,
    mode: u8,
}
#[async_trait]
impl LocalOwner for FailureOwner {
    fn owner(&self) -> Owner {
        self.inner.owner()
    }
    async fn apply_in(
        &self,
        c: &mut PgConnection,
        r: &BatchRequest,
    ) -> Result<Outcome, CommandError> {
        let out = self.inner.apply_in(c, r).await?;
        match self.mode {
            1 => Err(CommandError::Unavailable),
            2 => Ok(Outcome::Pending),
            _ => Ok(out),
        }
    }
    async fn verify_in(
        &self,
        c: &mut PgConnection,
        r: &BatchRequest,
        receipt: Receipt,
    ) -> Result<(), CommandError> {
        self.inner.verify_in(c, r, receipt).await?;
        if self.mode == 3 {
            Err(CommandError::Unavailable)
        } else {
            Ok(())
        }
    }
}
#[tokio::test]
#[ignore = "owned fixture"]
async fn event_cleanup_local_actual_effects_verification_and_pending_roll_back_together() {
    let pool = pool().await;
    for (step, own) in [
        (Step::ExpireBeacons, Owner::Beacons),
        (Step::Cleanup, Owner::PrivateRetention),
    ] {
        let f = seed(&pool, step, true, 3, true).await;
        for mode in [1, 2, 3] {
            let run = run(
                &pool,
                Arc::new(FailureOwner {
                    inner: owner(own),
                    mode,
                }),
                true,
            );
            let p = run.run_owner(f.actor, f.job, own).await.unwrap();
            assert_eq!(p.processed, 0);
            assert_eq!(
                p.state,
                if mode == 2 {
                    State::Pending
                } else {
                    State::PartialFailure
                }
            );
            assert_eq!(counts(&pool, &f).await, (0, 0));
        }
        let run = run(&pool, owner(own), true);
        let p = run.run_owner(f.actor, f.job, own).await.unwrap();
        assert_eq!(p.processed, 2);
        assert_eq!(counts(&pool, &f).await, (1, 2));
        let row=sqlx::query("select p.*,j.retention_revision,j.closed_at,j.cleanup_after from concert_event_cleanup_owner_pages p join concert_closure_jobs j using(job_id) where p.job_id=$1").bind(f.job).fetch_one(&pool).await.unwrap();
        let r = BatchRequest::new(
            Scope {
                actor: f.actor.actor,
                event: f.actor.event,
                job: f.job,
                step,
                owner: own,
            },
            Retention {
                policy_revision: row.get("retention_revision"),
                closed_at: row.get("closed_at"),
                cleanup_after: row.get("cleanup_after"),
            },
            0,
            None,
            2,
        )
        .unwrap();
        assert_eq!(r.operation(), row.get::<Uuid, _>("operation_id"));
        assert_eq!(r.binding().as_slice(), row.get::<Vec<u8>, _>("binding"));
    }
}

async fn request(c: &mut PgConnection, f: &Fixture, own: Owner) -> BatchRequest {
    fences(c).await;
    let step = if own == Owner::Beacons {
        Step::ExpireBeacons
    } else {
        Step::Cleanup
    };
    sqlx::query("insert into concert_event_cleanup_progress(job_id,event_id,actor_id,step,owner,batch_limit) values($1,$2,$3,$4,$5,2)").bind(f.job).bind(f.actor.event).bind(f.actor.actor).bind(step.name()).bind(own.name()).execute(&mut *c).await.unwrap();
    let row=sqlx::query("select retention_revision,closed_at,cleanup_after from concert_closure_jobs where job_id=$1").bind(f.job).fetch_one(c).await.unwrap();
    BatchRequest::new(
        Scope {
            actor: f.actor.actor,
            event: f.actor.event,
            job: f.job,
            step,
            owner: own,
        },
        Retention {
            policy_revision: row.get("retention_revision"),
            closed_at: row.get("closed_at"),
            cleanup_after: row.get("cleanup_after"),
        },
        0,
        None,
        2,
    )
    .unwrap()
}
#[tokio::test]
#[ignore = "owned fixture"]
async fn event_cleanup_local_native_witness_requires_engine_append_and_is_immutable() {
    let pool = pool().await;
    for (step, own) in [
        (Step::ExpireBeacons, Owner::Beacons),
        (Step::Cleanup, Owner::PrivateRetention),
    ] {
        let f = seed(&pool, step, true, 3, true).await;
        let mut tx = pool.begin().await.unwrap();
        let r = request(&mut tx, &f, own).await;
        let out = owner(own).apply_in(&mut tx, &r).await.unwrap();
        let receipt = match out {
            Outcome::Page { receipt, .. } | Outcome::Converged { receipt } => receipt,
            _ => panic!("actual native effect"),
        };
        owner(own).verify_in(&mut tx, &r, receipt).await.unwrap();
        assert!(tx.commit().await.is_err());
        assert_eq!(counts(&pool, &f).await, (0, 0));
        // Even an exact bound ledger append cannot outlive its caller's fence.
        let mut tx = pool.begin().await.unwrap();
        let r = request(&mut tx, &f, own).await;
        let (receipt, next) = match owner(own).apply_in(&mut tx, &r).await.unwrap() {
            Outcome::Page { receipt, next } => (receipt, next),
            _ => panic!("bounded first page"),
        };
        sqlx::query("insert into concert_event_cleanup_batches(operation_id,job_id,event_id,actor_id,step,owner,ordinal,binding,cursor_in,cursor_out,outcome,receipt_id,owner_revision,processed,cumulative) values($1,$2,$3,$4,$5,$6,0,$7,NULL,$8,'PAGE',$9,1,$10,$10)")
            .bind(r.operation()).bind(f.job).bind(f.actor.event).bind(f.actor.actor).bind(step.name()).bind(own.name()).bind(r.binding().to_vec()).bind(next.bytes()).bind(receipt.id).bind(receipt.processed as i16).execute(&mut *tx).await.unwrap();
        sqlx::query("select set_config('mcr.relationship_authority_until',(clock_timestamp()-interval '1 second')::text,true)").execute(&mut *tx).await.unwrap();
        assert!(tx.commit().await.is_err());
        assert_eq!(counts(&pool, &f).await, (0, 0));
        run(&pool, owner(own), true)
            .run_owner(f.actor, f.job, own)
            .await
            .unwrap();
        for sql in ["update concert_event_cleanup_owner_pages set receipt_id=operation_id where job_id=$1","delete from concert_event_cleanup_owner_pages where job_id=$1","update concert_event_cleanup_owner_effects set previous_revision=previous_revision+1 where operation_id in(select operation_id from concert_event_cleanup_owner_pages where job_id=$1)","delete from concert_event_cleanup_owner_effects where operation_id in(select operation_id from concert_event_cleanup_owner_pages where job_id=$1)"] {
            assert!(sqlx::query(sql).bind(f.job).execute(&pool).await.is_err());
        }
        for sql in [
            "truncate concert_event_cleanup_owner_pages cascade",
            "truncate concert_event_cleanup_owner_effects",
        ] {
            assert!(sqlx::query(sql).execute(&pool).await.is_err());
        }
        assert_eq!(counts(&pool, &f).await, (1, 2));
    }
}

#[tokio::test]
#[ignore = "owned fixture"]
async fn event_cleanup_local_forged_binding_xid_receipt_and_effect_are_rejected() {
    let pool = pool().await;
    let f = seed(&pool, Step::ExpireBeacons, true, 3, false).await;
    for mode in 0..3 {
        let mut tx = pool.begin().await.unwrap();
        let r = request(&mut tx, &f, Owner::Beacons).await;
        let sql=match mode {
            0=>"insert into concert_event_cleanup_owner_pages(operation_id,job_id,event_id,actor_id,step,owner,ordinal,batch_limit,binding) values($1,$2,$3,$4,'EXPIRE_BEACONS','BEACONS',0,2,decode(repeat('00',32),'hex'))",
            1=>"insert into concert_event_cleanup_owner_pages(operation_id,job_id,event_id,actor_id,step,owner,ordinal,batch_limit,binding,native_xid) values($1,$2,$3,$4,'EXPIRE_BEACONS','BEACONS',0,2,$5,'1'::xid8)",
            _=>"insert into concert_event_cleanup_owner_pages(operation_id,job_id,event_id,actor_id,step,owner,ordinal,batch_limit,binding) values($1,$2,$3,$4,'EXPIRE_BEACONS','BEACONS',0,2,$5)"};
        let q = sqlx::query(sql)
            .bind(r.operation())
            .bind(f.job)
            .bind(f.actor.event)
            .bind(f.actor.actor);
        let result = if mode == 0 {
            q.execute(&mut *tx).await
        } else {
            q.bind(r.binding().to_vec()).execute(&mut *tx).await
        };
        if mode < 2 {
            assert!(result.is_err());
        } else {
            result.unwrap();
            assert!(sqlx::query("insert into concert_event_cleanup_owner_effects(operation_id,resource_key,previous_revision,next_revision) values($1,decode('0a','hex')||uuid_send($2),1,2)").bind(r.operation()).bind(f.beacons[0]).execute(&mut *tx).await.is_err());
        }
        tx.rollback().await.unwrap();
    }
    let mut tx = pool.begin().await.unwrap();
    let r = request(&mut tx, &f, Owner::Beacons).await;
    let out = owner(Owner::Beacons).apply_in(&mut tx, &r).await.unwrap();
    let mut receipt = match out {
        Outcome::Page { receipt, .. } => receipt,
        _ => panic!("page"),
    };
    receipt.id = Uuid::new_v4();
    assert!(owner(Owner::Beacons)
        .verify_in(&mut tx, &r, receipt)
        .await
        .is_err());
    tx.rollback().await.unwrap();
    assert_eq!(counts(&pool, &f).await, (0, 0));
}

#[tokio::test]
#[ignore = "owned fixture"]
async fn event_cleanup_local_open_witness_and_cascade_over_cap_cannot_commit() {
    let pool = pool().await;
    let f = seed(&pool, Step::Cleanup, true, 3, true).await;
    for cascade in [false, true] {
        let mut tx = pool.begin().await.unwrap();
        let r = request(&mut tx, &f, Owner::PrivateRetention).await;
        sqlx::query("insert into concert_event_cleanup_owner_pages(operation_id,job_id,event_id,actor_id,step,owner,ordinal,batch_limit,binding) values($1,$2,$3,$4,'CLEANUP','PRIVATE_RETENTION',0,2,$5)")
            .bind(r.operation()).bind(f.job).bind(f.actor.event).bind(f.actor.actor).bind(r.binding().to_vec()).execute(&mut *tx).await.unwrap();
        if cascade {
            sqlx::query("select set_config('mcr.cleanup_operation_id',$1,true)")
                .bind(r.operation().to_string())
                .execute(&mut *tx)
                .await
                .unwrap();
            // One parent statement would delete a request, binding and two
            // grants: automatic capture must reject its four actual row writes.
            assert!(
                sqlx::query("delete from concert_social_requests where event_id=$1")
                    .bind(f.actor.event)
                    .execute(&mut *tx)
                    .await
                    .is_err()
            );
            tx.rollback().await.unwrap();
        } else {
            assert!(tx.commit().await.is_err());
        }
        assert_eq!(counts(&pool, &f).await, (0, 0));
        let sources:i64=sqlx::query_scalar("select (select count(*) from concert_social_requests where event_id=$1)+(select count(*) from concert_social_bindings where event_id=$1)+(select count(*) from concert_social_grants where event_id=$1)").bind(f.actor.event).fetch_one(&pool).await.unwrap();
        assert_eq!(sources, 4);
    }
}

#[tokio::test]
#[ignore = "owned fixture; actual PostgreSQL restart follows"]
async fn event_cleanup_local_restart_prepare() {
    let pool = pool().await;
    sqlx::query("create table event_cleanup_fixture_local_restart(actor_id uuid,event_id uuid,job_id uuid,owner text,operation_id uuid)").execute(&pool).await.unwrap();
    for (step, own) in [
        (Step::ExpireBeacons, Owner::Beacons),
        (Step::Cleanup, Owner::PrivateRetention),
    ] {
        let f = seed(&pool, step, true, 3, true).await;
        let p = run(&pool, owner(own), true)
            .run_owner(f.actor, f.job, own)
            .await
            .unwrap();
        assert_eq!(p.processed, 2);
        sqlx::query("insert into event_cleanup_fixture_local_restart select $1,$2,$3,$4,operation_id from concert_event_cleanup_owner_pages where job_id=$3").bind(f.actor.actor).bind(f.actor.event).bind(f.job).bind(own.name()).execute(&pool).await.unwrap();
    }
}
#[tokio::test]
#[ignore = "owned fixture; after actual PostgreSQL restart"]
async fn event_cleanup_local_restart_verify() {
    let pool = pool().await;
    let rows = sqlx::query("select * from event_cleanup_fixture_local_restart")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        let actor = Actor {
            actor: row.get("actor_id"),
            event: row.get("event_id"),
        };
        let job: Uuid = row.get("job_id");
        let own = if row.get::<String, _>("owner") == "BEACONS" {
            Owner::Beacons
        } else {
            Owner::PrivateRetention
        };
        let run = run(&pool, owner(own), true);
        let p = run.run_owner(actor, job, own).await.unwrap();
        assert!(p.processed > 2 && p.processed <= 4);
        let mut complete = p.state == State::Complete;
        for _ in 0..8 {
            if complete {
                break;
            }
            complete = run.run_owner(actor, job, own).await.unwrap().state == State::Complete;
        }
        assert!(complete);
        assert_eq!(
            run.run_owner(actor, job, own).await.unwrap().state,
            State::Complete
        );
        let unchanged:bool=sqlx::query_scalar("select exists(select 1 from concert_event_cleanup_owner_pages where job_id=$1 and ordinal=0 and operation_id=$2)").bind(job).bind(row.get::<Uuid,_>("operation_id")).fetch_one(&pool).await.unwrap();
        assert!(unchanged);
        let duplicates:i64=sqlx::query_scalar("select count(*) from (select e.resource_key from concert_event_cleanup_owner_effects e join concert_event_cleanup_owner_pages p using(operation_id) where p.job_id=$1 group by e.resource_key having count(*)>1) d").bind(job).fetch_one(&pool).await.unwrap();
        assert_eq!(duplicates, 0);
    }
}
