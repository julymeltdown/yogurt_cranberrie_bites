//! Marker-owned Auth + Concert stores; actual authenticated Auth source RPC.
//! Provider verification is deliberately absent: this is internal erasure.
#[path = "../../auth-service/tests/support/auth_fixture_config.rs"]
mod auth_fixture;
#[path = "canonical_owner_composition_support/environment.rs"]
mod environment;
use auth_service::{
    adapters::out::persistence::user_deletion_saga_repository::PgUserDeletionSagaRepository,
    application::{dto::auth_dto::DeleteUserDto, ports::UserDeletionSagaPort},
};
use concert_service::{
    account_private_data::{PrivateDataCategory, PrivateDataCategoryOwner},
    adapters::out::{
        age_provider::account_deletion::{AgeAccountDeletion, AGE_DELETION_PAGE},
        relationship_authority::deletion_source::AuthDeletionSource,
    },
    world::account_deletion::{
        lock_user, DeletionAuthority, WorldDeletionCommand, WorldDeletionTarget,
    },
};
use platform::{
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, Workload},
};
use sqlx::PgPool;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::time::Instant;
use uuid::Uuid;

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(4)
}

// Fault injection changes the REAL Auth repository immediately before the
// owner's final source check; the RPC itself remains the production adapter.
struct RevokeBeforeCommit {
    source: Arc<AuthDeletionSource>,
    auth: PgPool,
    calls: AtomicUsize,
}
#[async_trait::async_trait]
impl DeletionAuthority for RevokeBeforeCommit {
    async fn verify(
        &self,
        c: &WorldDeletionCommand,
        d: Instant,
    ) -> Result<
        concert_service::world::account_deletion::VerifiedDeletionSource,
        concert_service::world::account_deletion::DeletionFailure,
    > {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 1 {
            sqlx::query("update user_deletion_sagas set actor_id=$2 where id=$1")
                .bind(c.saga_id)
                .bind(Uuid::new_v4())
                .execute(&self.auth)
                .await
                .map_err(|_| {
                    concert_service::world::account_deletion::DeletionFailure::Unavailable
                })?;
        }
        self.source.verify(c, d).await
    }
}

fn saved_command(c: &WorldDeletionCommand) -> serde_json::Value {
    serde_json::json!({"saga":c.saga_id,"user":c.user_id,"actor":c.actor_id,"retention":c.retention_until})
}
fn load_command(value: &serde_json::Value) -> WorldDeletionCommand {
    WorldDeletionCommand {
        saga_id: value["saga"].as_str().unwrap().parse().unwrap(),
        user_id: value["user"].as_str().unwrap().parse().unwrap(),
        actor_id: value["actor"].as_str().unwrap().parse().unwrap(),
        retention_until: value["retention"].as_str().unwrap().parse().unwrap(),
        target: WorldDeletionTarget::PrivateData,
    }
}

async fn identity(auth: &PgPool) -> anyhow::Result<WorldDeletionCommand> {
    let mut config = auth_fixture::test_config(std::env::var("MCR_CANONICAL_AUTH_URL")?)?;
    config.outbound_identity = "auth-fixture".into();
    config.tls_material = Some(environment::tls(Workload::Auth)?);
    for endpoint in [
        &mut config.profile_grpc_endpoint,
        &mut config.friend_grpc_endpoint,
        &mut config.notification_grpc_endpoint,
        &mut config.post_grpc_endpoint,
    ] {
        *endpoint = "https://127.0.0.1:1".into();
    }
    let settings = auth_service::config::concert_deletion::ConcertDeletionSettings::from_values(
        Some("off"),
        None,
        None,
    )?;
    let state =
        auth_service::config::build_state_with_deletion_runtime(&config, auth.clone(), settings)
            .await?;
    let email = format!("age-delete-{}@example.invalid", Uuid::new_v4().simple());
    let user = state
        .signup
        .signup_email(&email, "Owned-age-deletion-42!")
        .await?
        .user_id;
    let code: String = sqlx::query_scalar("select code from email_verifications where user_id=$1")
        .bind(user)
        .fetch_one(auth)
        .await?;
    state.signup.verify_email(&email, &code).await?;
    let dto = state
        .delete_user
        .execute(DeleteUserDto {
            user_id: user.to_string(),
            actor_id: user.to_string(),
            request_id: None,
            ip_address: None,
            user_agent: None,
        })
        .await?;
    let saga = PgUserDeletionSagaRepository::new(auth.clone())
        .find_by_saga_id(dto.saga_id.parse()?)
        .await?
        .unwrap();
    Ok(WorldDeletionCommand {
        saga_id: saga.id,
        user_id: saga.user_id,
        actor_id: saga.actor_id,
        retention_until: saga.retention_until,
        target: WorldDeletionTarget::PrivateData,
    })
}

async fn populated(pool: &PgPool, user: Uuid) -> anyhow::Result<Uuid> {
    let event = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Age erasure fixture','ARCHIVED')")
        .bind(event).bind(format!("age-delete-{event}")).execute(pool).await?;
    sqlx::query("insert into concert_age_owners(user_id) values($1)")
        .bind(user)
        .execute(pool)
        .await?;
    let receipt = Uuid::new_v4();
    // Persisted BE15 audit fixture, deliberately retained by the erasure owner.
    sqlx::query("insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state) values($1,$2,$3,'startConcertAgeVerification',$4,$5,$6,'PENDING')")
        .bind(receipt).bind(user).bind(event).bind(Uuid::new_v4())
        .bind(format!("hmac-sha256:v1:fixture:{}", "a".repeat(64))).bind(Uuid::new_v4()).execute(pool).await?;
    for n in 0..(AGE_DELETION_PAGE + 3) {
        let id = Uuid::new_v4();
        sqlx::query("insert into concert_age_verifications(verification_id,user_id,event_id,session_id,verifier_id,source_revision,state_digest,return_target,created_at,expires_at,state) values($1,$2,$3,$4,'fixture','v1',decode(repeat('ab',32),'hex'),'AGE_PRIVACY',clock_timestamp(),clock_timestamp()+interval '5 minutes','PENDING')")
            .bind(id).bind(user).bind(event).bind(Uuid::new_v4()).execute(pool).await?;
        sqlx::query("insert into concert_age_claims(claim_id,user_id,event_id,verifier_id,source_revision,status,verified_at,expires_at,age_at_verification,next_birthday_at) values($1,$2,$3,'fixture','v1',$4,clock_timestamp()-interval '1 hour',clock_timestamp()+interval '1 day',25,(clock_timestamp() at time zone 'Asia/Seoul')::date+1)")
            .bind(id).bind(user).bind(event).bind(if n==0 {"VERIFIED"} else {"REPLACED"}).execute(pool).await?;
    }
    Ok(receipt)
}

async fn snapshot(pool: &PgPool, user: Uuid) -> anyhow::Result<serde_json::Value> {
    Ok(sqlx::query_scalar(
        "select to_jsonb(p) from concert_age_deletion_progress p where user_id=$1",
    )
    .bind(user)
    .fetch_one(pool)
    .await?)
}

#[tokio::test]
#[ignore = "finite named allocation: marker-owned stores, mTLS, physical restart, fresh test PID"]
async fn age_deletion_populated_auth_retry_cancel_restart() -> anyhow::Result<()> {
    let auth = environment::pool("auth").await?;
    let pool = environment::pool("concert").await?;
    let registry = CallerRegistry::new().register(Workload::Concert, "concert-fixture")?;
    let service = auth_service::adapters::r#in::grpc::concert_deletion_source::guarded_server(
        auth.clone(),
        registry,
    )?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("https://{}", listener.local_addr()?);
    let incoming = futures::stream::unfold(listener, |l| async {
        Some((l.accept().await.map(|(s, _)| s), l))
    });
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tonic::transport::Server::builder()
        .tls_config(environment::tls(Workload::Auth)?.server_config()?)?
        .layer(CallDeadlineLayer)
        .add_service(service);
    let server = tokio::spawn(server.serve_with_incoming_shutdown(incoming, async {
        let _ = stopped.await;
    }));
    let authority = Arc::new(AuthDeletionSource::connect(
        &endpoint,
        &environment::tls(Workload::Concert)?,
        "concert-fixture",
    )?);
    let leaf = AgeAccountDeletion::new(pool.clone(), authority.clone());
    let saved = environment::root()?.join("age-deletion-restart.json");
    if std::env::var("MCR_AGE_DELETION_PHASE")? == "RESTARTED" {
        let value: serde_json::Value = serde_json::from_slice(&std::fs::read(saved)?)?;
        let c = WorldDeletionCommand {
            saga_id: value["saga"].as_str().unwrap().parse()?,
            user_id: value["user"].as_str().unwrap().parse()?,
            actor_id: value["actor"].as_str().unwrap().parse()?,
            retention_until: value["retention"].as_str().unwrap().parse()?,
            target: WorldDeletionTarget::PrivateData,
        };
        let d = deadline();
        let source = authority.verify(&c, d).await.unwrap();
        let ack = leaf.apply_category(&c, &source, d).await.unwrap();
        assert!(ack.complete && ack.progress_revision > 0);
        let before = snapshot(&pool, c.user_id).await?;
        let d = deadline();
        let source = authority.verify(&c, d).await.unwrap();
        assert_eq!(
            leaf.apply_category(&c, &source, d)
                .await
                .unwrap()
                .progress_revision,
            ack.progress_revision
        );
        assert_eq!(snapshot(&pool, c.user_id).await?, before);
        assert_eq!(value["terminal"], before);
        let resume = load_command(&value["resume"]);
        let d = deadline();
        let source = authority.verify(&resume, d).await.unwrap();
        let ack = leaf.apply_category(&resume, &source, d).await.unwrap();
        assert!(ack.complete && ack.progress_revision == 2);
        let retained = snapshot(&pool, resume.user_id).await?;
        let d = deadline();
        let source = authority.verify(&resume, d).await.unwrap();
        assert_eq!(
            leaf.apply_category(&resume, &source, d)
                .await
                .unwrap()
                .progress_revision,
            2
        );
        assert_eq!(snapshot(&pool, resume.user_id).await?, retained);
    } else {
        let c = identity(&auth).await?;
        let audit = populated(&pool, c.user_id).await?;
        let d = deadline();
        let source = authority.verify(&c, d).await.unwrap();
        let revoker = AgeAccountDeletion::new(
            pool.clone(),
            Arc::new(RevokeBeforeCommit {
                source: authority.clone(),
                auth: auth.clone(),
                calls: AtomicUsize::new(0),
            }),
        );
        assert!(revoker.apply_category(&c, &source, d).await.is_err());
        assert!(!sqlx::query_scalar::<_,bool>("select exists(select 1 from concert_world_account_deletion_fences where user_id=$1)")
            .bind(c.user_id).fetch_one(&pool).await?);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "select count(*) from concert_age_claims where user_id=$1"
            )
            .bind(c.user_id)
            .fetch_one(&pool)
            .await?,
            AGE_DELETION_PAGE + 3
        );
        sqlx::query("update user_deletion_sagas set actor_id=$2 where id=$1")
            .bind(c.saga_id)
            .bind(c.actor_id)
            .execute(&auth)
            .await?;
        let d = deadline();
        let source = authority.verify(&c, d).await.unwrap();
        // Cancellation while waiting on the FIRST user fence rolls back all effects.
        let mut blocker = pool.begin().await?;
        lock_user(&mut blocker, c.user_id).await?;
        let cancelled = tokio::time::timeout(
            Duration::from_millis(80),
            leaf.apply_category(&c, &source, d),
        )
        .await;
        assert!(cancelled.is_err());
        blocker.rollback().await?;
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "select count(*) from concert_age_claims where user_id=$1"
            )
            .bind(c.user_id)
            .fetch_one(&pool)
            .await?,
            AGE_DELETION_PAGE + 3
        );
        let d = deadline();
        let source = authority.verify(&c, d).await.unwrap();
        let first = leaf.apply_category(&c, &source, d).await.unwrap();
        assert!(!first.complete && first.progress_revision == 0);
        assert_eq!(first.category, PrivateDataCategory::AgeClaims);
        assert!(first.verifies(&c, source.source_binding()));
        let progress = snapshot(&pool, c.user_id).await?;
        assert_eq!(progress["erased_verifications"], AGE_DELETION_PAGE);
        // Rebound source is rejected even with the exact command and deadline.
        let bad = concert_service::world::account_deletion::VerifiedDeletionSource::from_verified_auth_source(&c,d,[7;32]).unwrap();
        assert!(leaf.apply_category(&c, &bad, d).await.is_err());
        assert_eq!(snapshot(&pool, c.user_id).await?, progress);
        for change in 0..5 {
            let mut rebound = c.clone();
            match change {
                0 => rebound.actor_id = Uuid::new_v4(),
                1 => rebound.user_id = Uuid::new_v4(),
                2 => rebound.saga_id = Uuid::new_v4(),
                3 => rebound.target = WorldDeletionTarget::Tickets,
                _ => rebound.retention_until += chrono::Duration::nanoseconds(1),
            }
            assert!(leaf.apply_category(&rebound, &source, d).await.is_err());
        }
        let d = deadline();
        let source = authority.verify(&c, d).await.unwrap();
        assert!(leaf
            .apply_category(&c, &source, d + Duration::from_millis(1))
            .await
            .is_err());
        let done = leaf.apply_category(&c, &source, d).await.unwrap();
        assert!(done.complete && done.progress_revision == 2);
        assert!(done.verifies(&c, source.source_binding()));
        assert!(
            !sqlx::query_scalar::<_, bool>(
                "select exists(select 1 from concert_age_owners where user_id=$1)"
            )
            .bind(c.user_id)
            .fetch_one(&pool)
            .await?
        );
        assert!(
            sqlx::query("insert into concert_age_owners(user_id) values($1)")
                .bind(c.user_id)
                .execute(&pool)
                .await
                .is_err()
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "select count(*) from concert_age_claims where user_id=$1"
            )
            .bind(c.user_id)
            .fetch_one(&pool)
            .await?,
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "select count(*) from concert_age_verifications where user_id=$1"
            )
            .bind(c.user_id)
            .fetch_one(&pool)
            .await?,
            0
        );
        assert!(
            sqlx::query_scalar::<_, bool>(
                "select exists(select 1 from concert_command_receipts where receipt_id=$1)"
            )
            .bind(audit)
            .fetch_one(&pool)
            .await?
        );
        // Direct storage recreation (late callback) is rejected by the same fence.
        let event: Uuid =
            sqlx::query_scalar("select event_id from concert_command_receipts where receipt_id=$1")
                .bind(audit)
                .fetch_one(&pool)
                .await?;
        assert!(sqlx::query("insert into concert_age_verifications(verification_id,user_id,event_id,session_id,verifier_id,source_revision,state_digest,return_target,created_at,expires_at,state) values($1,$2,$3,$4,'fixture','v1',decode(repeat('ab',32),'hex'),'AGE_PRIVACY',clock_timestamp(),clock_timestamp()+interval '5 minutes','PENDING')")
            .bind(Uuid::new_v4()).bind(c.user_id).bind(event).bind(Uuid::new_v4()).execute(&pool).await.is_err());
        assert!(
            sqlx::query("delete from concert_age_deletion_progress where user_id=$1")
                .bind(c.user_id)
                .execute(&pool)
                .await
                .is_err()
        );
        assert!(sqlx::query("update concert_age_deletion_progress set progress_revision=progress_revision+1 where user_id=$1").bind(c.user_id).execute(&pool).await.is_err());
        let terminal = snapshot(&pool, c.user_id).await?;
        // Authoritative source corruption/revocation cannot replay a terminal ACK.
        sqlx::query("update user_deletion_sagas set actor_id=$2 where id=$1")
            .bind(c.saga_id)
            .bind(Uuid::new_v4())
            .execute(&auth)
            .await?;
        assert!(leaf.apply_category(&c, &source, d).await.is_err());
        sqlx::query("update user_deletion_sagas set actor_id=$2 where id=$1")
            .bind(c.saga_id)
            .bind(c.actor_id)
            .execute(&auth)
            .await?;
        assert_eq!(snapshot(&pool, c.user_id).await?, terminal);
        let resume = identity(&auth).await?;
        populated(&pool, resume.user_id).await?;
        let d = deadline();
        let source = authority.verify(&resume, d).await.unwrap();
        assert!(
            !leaf
                .apply_category(&resume, &source, d)
                .await
                .unwrap()
                .complete
        );
        let mut value = saved_command(&c);
        value["terminal"] = terminal;
        value["resume"] = saved_command(&resume);
        std::fs::write(saved, serde_json::to_vec(&value)?)?;
    }
    stop.send(()).ok();
    tokio::time::timeout(Duration::from_secs(3), server).await???;
    Ok(())
}
