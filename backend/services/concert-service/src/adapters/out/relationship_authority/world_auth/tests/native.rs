//! Actual consumed ticket/current membership -> signed client -> owning Auth PG.
use super::*;
use crate::world::PgWorldRepository;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::{os::unix::fs::MetadataExt, path::PathBuf, str::FromStr};

async fn pool(owner: &str) -> anyhow::Result<PgPool> {
    anyhow::ensure!(matches!(owner, "auth" | "concert"), "foreign owner refused");
    let root = PathBuf::from(std::env::var("MCR_WORLD_AUTH_ROOT")?);
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        root.is_absolute()
            && root.canonicalize()? == root
            && info.is_dir()
            && info.mode() & 0o077 == 0,
        "private owned fixture required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == std::env::var("MCR_WORLD_AUTH_IDENTITY")?,
        "fixture inode changed"
    );
    let marker = root.join("mcr-owner.json");
    let mi = std::fs::symlink_metadata(&marker)?;
    anyhow::ensure!(
        mi.is_file()
            && !mi.file_type().is_symlink()
            && mi.mode() & 0o077 == 0
            && mi.uid() == info.uid(),
        "private marker required"
    );
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(marker)?)?;
    anyhow::ensure!(
        value["uid"].as_u64() == Some(info.uid() as u64)
            && value["token"].as_str() == Some(std::env::var("MCR_WORLD_AUTH_TOKEN")?.as_str()),
        "fixture owner mismatch"
    );
    for name in ["data", "socket"] {
        let child = root.join(name);
        let ci = std::fs::symlink_metadata(&child)?;
        anyhow::ensure!(
            ci.is_dir() && !ci.file_type().is_symlink() && child.canonicalize()? == child,
            "foreign fixture child refused"
        );
    }
    let options = PgConnectOptions::from_str(&std::env::var(format!(
        "MCR_WORLD_AUTH_{}_URL",
        owner.to_uppercase()
    ))?)?;
    let database = if owner == "concert" {
        "mcr_concert_world_auth".to_owned()
    } else {
        "mcr_world_auth_auth".to_owned()
    };
    anyhow::ensure!(
        options.get_database() == Some(database.as_str())
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_options().is_none()
            && options.get_port() == 5432,
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await?;
    let data: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        data == root.join("data").to_string_lossy(),
        "foreign data directory refused"
    );
    let identity: (String, String, String, i32) = sqlx::query_as(
        "select current_database(),current_user,current_setting('listen_addresses'),current_setting('server_version_num')::int"
    ).fetch_one(&pool).await?;
    anyhow::ensure!(
        identity == (database, "mcr_owner".into(), "".into(), 160014),
        "foreign server refused"
    );
    Ok(pool)
}
async fn native_server(pool: PgPool) -> Running {
    use auth_service::adapters::r#in::grpc::concert_relationship::{
        proto, RelationshipServer, POLICY,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let guarded = platform::grpc_method_policy::guard(
        &POLICY,
        proto::auth_concert_relationship_service_server::AuthConcertRelationshipServiceServer::new(
            RelationshipServer::from_pool(pool),
        ),
        CallerRegistry::new()
            .register(Workload::Concert, "world-auth-fixture-concert")
            .unwrap(),
        verifier(),
    )
    .unwrap();
    let (stop, rx) = oneshot::channel();
    let incoming = futures::stream::unfold(listener, |listener| async move {
        Some((listener.accept().await.map(|(s, _)| s), listener))
    });
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .layer(CallDeadlineLayer)
            .add_service(guarded)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = rx.await;
            })
            .await
            .unwrap();
    });
    Running {
        endpoint,
        stop: Some(stop),
        task,
    }
}
async fn fixture(auth: &PgPool) -> anyhow::Result<TicketClaim> {
    let c = claim();
    let user = c.record.subject.key.user_id.uuid();
    let family = Uuid::parse_str(c.record.subject.session_id.as_str())?;
    sqlx::query("insert into users(id,primary_email,password_hash,status) values($1,$2,'private-world-auth-password','ACTIVE')").bind(user).bind(format!("{user}@private-world-auth.invalid")).execute(auth).await?;
    sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values($1,'private-world-auth-token',$2,$3,clock_timestamp()+interval '1 hour')").bind(Uuid::new_v4().to_string()).bind(user).bind(family).execute(auth).await?;
    Ok(c)
}

#[tokio::test]
#[ignore = "requires explicitly marker/inode-owned Auth and Concert PostgreSQL"]
async fn native_consumed_ticket_and_current_membership_use_fresh_signed_auth() -> anyhow::Result<()>
{
    let auth = pool("auth").await?;
    let concert = pool("concert").await?;
    let c = fixture(&auth).await?;
    let repo = PgWorldRepository::new(concert.clone());
    let event = c.record.subject.event_id.uuid();
    let user = c.record.subject.key.user_id.uuid();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'World Auth fixture','OPEN')").bind(event).bind(format!("auth-{}",event.simple())).execute(&concert).await?;
    sqlx::query("insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,is_default) values($1,$2,'Fixture',2,'map_world_auth',1,'avatar_manifest',1,true)").bind(event).bind(c.record.pin.room_id.uuid()).execute(&concert).await?;
    assert_eq!(repo.issue(c.record.clone()).await?, IssueOutcome::Issued);
    let ConsumeOutcome::Consumed(consumed) = repo
        .consume(ConsumeClaim {
            digest: c.record.digest.clone(),
            connection_id: c.connection_id,
            requested: c.record.pin.clone(),
            now: Utc::now(),
        })
        .await?
    else {
        anyhow::bail!("real consume required")
    };
    assert!(repo
        .current_owned_membership(c.record.subject.key, Utc::now())
        .await?
        .is_none());
    let server = native_server(auth.clone()).await;
    let reader = client(&server.endpoint);
    let b = binding(&consumed);
    let observation = reader.observe(&b).await?;
    observation.check(&b)?;
    assert_eq!(
        observation.credential_expires_at(),
        consumed.record.access_expires_at
    );
    let initial_revision = observation.revision();
    let expected = OwnerIdentity::new("native-world-auth-owner".into(), Uuid::new_v4())?;
    repo.claim_owner(
        c.record.subject.event_id,
        c.record.pin.room_id,
        &expected,
        Utc::now(),
        chrono::Duration::seconds(20),
    )
    .await?;
    assert!(matches!(
        repo.admit(&expected, &consumed, Utc::now()).await?,
        AdmissionOutcome::Admitted(_)
    ));
    let current = repo
        .current_owned_membership(c.record.subject.key, Utc::now())
        .await?
        .expect("real current membership");
    let next = || {
        WorldOwnerRequest::from_current(
            &current,
            &expected,
            &c.record.subject,
            c.connection_id,
            Uuid::new_v4(),
            deadline(),
        )
        .unwrap()
    };
    assert_eq!(
        reader.observe(&next()).await?.credential_expires_at(),
        consumed.record.access_expires_at
    );
    sqlx::query("update refresh_tokens set revoked_at=clock_timestamp() where user_id=$1")
        .bind(user)
        .execute(&auth)
        .await?;
    assert_eq!(
        reader.observe(&next()).await.unwrap_err(),
        WorldError::Denied
    );
    sqlx::query("update refresh_tokens set revoked_at=null,expires_at=clock_timestamp()-interval '1 second' where user_id=$1").bind(user).execute(&auth).await?;
    assert_eq!(
        reader.observe(&next()).await.unwrap_err(),
        WorldError::Denied
    );
    sqlx::query(
        "update refresh_tokens set expires_at=clock_timestamp()+interval '1 hour' where user_id=$1",
    )
    .bind(user)
    .execute(&auth)
    .await?;
    for status in ["PENDING_VERIFICATION", "SUSPENDED", "DELETED"] {
        sqlx::query("update users set status=$2 where id=$1")
            .bind(user)
            .bind(status)
            .execute(&auth)
            .await?;
        assert_eq!(
            reader.observe(&next()).await.unwrap_err(),
            WorldError::Denied
        );
    }
    sqlx::query("update users set status='ACTIVE' where id=$1")
        .bind(user)
        .execute(&auth)
        .await?;
    let refreshed = reader.observe(&next()).await?;
    assert_eq!(refreshed.revision().epoch, initial_revision.epoch);
    assert!(refreshed.revision().revision > initial_revision.revision);
    let mut wrong = current.clone();
    wrong.membership.session_id = SessionId::new(Uuid::new_v4().to_string())?;
    let wrong_subject = WorldSubject {
        session_id: wrong.membership.session_id.clone(),
        ..c.record.subject.clone()
    };
    let wrong = WorldOwnerRequest::from_current(
        &wrong,
        &expected,
        &wrong_subject,
        c.connection_id,
        Uuid::new_v4(),
        deadline(),
    )?;
    // Even trusted server binding of an unknown family never fabricates owner liveness.
    assert_eq!(
        reader.observe(&wrong).await.unwrap_err(),
        WorldError::Denied
    );
    sqlx::query("delete from users where id=$1")
        .bind(user)
        .execute(&auth)
        .await?;
    assert_eq!(
        reader.observe(&next()).await.unwrap_err(),
        WorldError::Denied
    );
    assert_eq!(
        repo.consume(ConsumeClaim {
            digest: c.record.digest,
            connection_id: c.connection_id,
            requested: c.record.pin.clone(),
            now: Utc::now()
        })
        .await?,
        ConsumeOutcome::AlreadyConsumed
    );
    drop(server);
    auth.close().await;
    concert.close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires explicitly marker/inode-owned Auth PostgreSQL"]
async fn native_locked_statement_cancel_and_owner_outage_never_fabricate_facts(
) -> anyhow::Result<()> {
    let auth = pool("auth").await?;
    let c = fixture(&auth).await?;
    let server = native_server(auth.clone()).await;
    let reader = Arc::new(client(&server.endpoint));
    let mut lock = auth.begin().await?;
    sqlx::query("lock table users in access exclusive mode")
        .execute(&mut *lock)
        .await?;
    let bounded = AuthorityRpcDeadline::new(Utc::now().timestamp_micros() + 80_000, BUDGET)?;
    let b = WorldOwnerRequest::from_consumed(&c, Uuid::new_v4(), bounded)?;
    let start = std::time::Instant::now();
    assert!(reader.observe(&b).await.is_err());
    assert!(start.elapsed() < Duration::from_secs(1));
    let b = Arc::new(binding(&c));
    let r = reader.clone();
    let request = b.clone();
    let task = tokio::spawn(async move { r.observe(&request).await });
    tokio::time::sleep(Duration::from_millis(40)).await;
    b.cancel();
    assert!(task.await?.is_err());
    lock.rollback().await?;
    let recovered = reader.observe(&binding(&c)).await?;
    assert_eq!(
        recovered.credential_expires_at(),
        c.record.access_expires_at
    );
    sqlx::query("update users set status='SUSPENDED' where id=$1")
        .bind(c.record.subject.key.user_id.uuid())
        .execute(&auth)
        .await?;
    assert_eq!(
        reader.observe(&binding(&c)).await.unwrap_err(),
        WorldError::Denied
    );
    auth.close().await;
    assert_eq!(
        reader.observe(&binding(&c)).await.unwrap_err(),
        WorldError::Unavailable
    );
    drop(server);
    Ok(())
}
