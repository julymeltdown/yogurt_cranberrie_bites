use super::transport::{root, tls, Running};
use super::*;
use auth_service::{
    adapters::out::{
        cache::session_store::PgRefreshTokenStore, persistence::user_repository::PgUserRepository,
        runtime::SystemAuthRuntime,
    },
    application::ports::{RefreshTokenStorePort, UserRepositoryPort},
    domain::session::RefreshTokenRecord,
};
use friend_service::{
    adapters::out::persistence::follow_repository::PgFollowRepository,
    application::ports::{RelationshipPolicyRepositoryPort, UserLifecycleRepositoryPort},
};
use sqlx::{PgPool, Row};
use std::str::FromStr;

async fn pool(owner: InvalidationOwner) -> anyhow::Result<PgPool> {
    let root = root()?;
    let name = match owner {
        InvalidationOwner::Auth => "mcr_invalidation_client_auth",
        InvalidationOwner::Friend => "mcr_invalidation_client_friend",
    };
    let key = match owner {
        InvalidationOwner::Auth => "MCR_INVALIDATION_CLIENT_AUTH_DATABASE_URL",
        InvalidationOwner::Friend => "MCR_INVALIDATION_CLIENT_FRIEND_DATABASE_URL",
    };
    let options = sqlx::postgres::PgConnectOptions::from_str(&std::env::var(key)?)?;
    anyhow::ensure!(
        options.get_username() == "mcr_owner"
            && options.get_database() == Some(name)
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_options().is_none(),
        "unsafe database options"
    );
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options.application_name("mcr_invalidation_client_fixture"))
        .await?;
    let r=sqlx::query("select system_identifier::text identity,current_database() db,current_setting('server_version_num')::int version,current_setting('listen_addresses') listeners,current_setting('data_directory') data from pg_control_system()").fetch_one(&pool).await?;
    anyhow::ensure!(
        r.get::<String, _>("identity")
            == std::env::var("MCR_INVALIDATION_CLIENT_SYSTEM_IDENTIFIER")?
            && r.get::<String, _>("db") == name
            && r.get::<i32, _>("version") / 10000 == 16
            && r.get::<String, _>("listeners").is_empty()
            && r.get::<String, _>("data") == root.join("data").to_str().unwrap(),
        "foreign cluster"
    );
    Ok(pool)
}
async fn migrate(pool: &PgPool, owner: InvalidationOwner) -> anyhow::Result<()> {
    let service = match owner {
        InvalidationOwner::Auth => "auth-service",
        InvalidationOwner::Friend => "friend-service",
    };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(service)
        .join("migrations");
    let mut connection = pool.acquire().await?.detach();
    sqlx::migrate::Migrator::new(path.as_path())
        .await?
        .run(&mut connection)
        .await?;
    drop(connection);
    Ok(())
}
fn store(pool: &PgPool) -> PgRefreshTokenStore {
    PgRefreshTokenStore::new(pool.clone(), Arc::new(SystemAuthRuntime))
}
async fn user(pool: &PgPool) -> anyhow::Result<Uuid> {
    let user = Uuid::new_v4();
    sqlx::query("insert into users(id,primary_email,password_hash,status) values($1,$2,'private-fixture','ACTIVE')").bind(user).bind(format!("{user}@fixture.invalid")).execute(pool).await?;
    Ok(user)
}
async fn session(pool: &PgPool, user: Uuid, family: Uuid) -> anyhow::Result<String> {
    let record = RefreshTokenRecord {
        user_id: user,
        family_id: family,
        jti: Uuid::new_v4().to_string(),
        token_hash: "private-fixture".into(),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
    };
    store(pool).store(&(&record).into()).await?;
    Ok(record.jti)
}
async fn replay(
    source: &InvalidationSource,
    consumer: Uuid,
    head: OwnerCursor,
) -> anyhow::Result<Vec<VerifiedSignal>> {
    let mut after = OwnerCursor::new(head.owner(), head.epoch(), 0)?;
    let mut signals = Vec::new();
    let mut pages = 0;
    loop {
        let page = source.read_after(consumer, after, 256, &budget()).await?;
        assert_eq!(page.head(), head);
        assert_eq!(page.consumer_id(), consumer);
        pages += 1;
        signals.extend_from_slice(page.signals());
        after = page.next();
        if !page.has_more() {
            break;
        }
        assert_eq!(page.signals().len(), 256);
        assert!(pages < 10);
    }
    assert_eq!(after, head);
    if signals.len() > 256 {
        assert!(pages > 1);
    }
    Ok(signals)
}
fn snapshot(signals: &[VerifiedSignal]) -> serde_json::Value {
    serde_json::Value::Array(signals.iter().map(|s|{
        let scope=match s.scope(){SignalScope::User{user}=>serde_json::json!(["user",user]),SignalScope::Family{user,family}=>serde_json::json!(["family",user,family]),SignalScope::Pair{actor,subject}=>serde_json::json!(["pair",actor,subject])};
        serde_json::json!({"owner":format!("{:?}",s.owner()),"epoch":s.cursor().epoch(),"position":s.cursor().sequence(),"id":s.signal_id(),"reason":format!("{:?}",s.reason()),"scope":scope})
    }).collect())
}
async fn save(pool: &PgPool, head: OwnerCursor, signals: &[VerifiedSignal]) -> anyhow::Result<()> {
    sqlx::query("create table invalidation_client_restart_fixture(singleton bool primary key,epoch uuid,head bigint,signals jsonb)").execute(pool).await?;
    sqlx::query("insert into invalidation_client_restart_fixture values(true,$1,$2,$3)")
        .bind(head.epoch())
        .bind(head.sequence())
        .bind(snapshot(signals))
        .execute(pool)
        .await?;
    Ok(())
}
fn metadata<T>(message: T, role: Workload) -> Request<T> {
    let mut r = Request::new(message);
    r.set_timeout(Duration::from_millis(500));
    r.metadata_mut()
        .insert("x-service-caller", role.as_str().parse().unwrap());
    r.metadata_mut().insert(
        "x-service-token",
        format!("private-fixture-{}", role.as_str())
            .parse()
            .unwrap(),
    );
    r
}
async fn authentication(server: &Running, owner: InvalidationOwner) -> anyhow::Result<()> {
    let consumer = Uuid::new_v4();
    for (cert, token) in [
        (Workload::Concert, "wrong"),
        (Workload::Gateway, "private-fixture-concert"),
    ] {
        let source = InvalidationSource::new(owner, &server.endpoint, token, &tls(cert)?)?;
        assert_eq!(
            source.read_head(consumer, &budget()).await.unwrap_err(),
            ReadError::Unauthenticated
        );
    }
    let expected = match owner {
        InvalidationOwner::Auth => Workload::Auth,
        InvalidationOwner::Friend => Workload::Friend,
    };
    let channel = tls(Workload::Gateway)?
        .connect(&server.endpoint, expected)
        .await?;
    let query = proto::ReadInvalidationHeadRequest {
        consumer_id: consumer.to_string(),
        request_id: Uuid::new_v4().to_string(),
    };
    let after = proto::ReadInvalidationsRequest {
        consumer_id: consumer.to_string(),
        request_id: Uuid::new_v4().to_string(),
        after: Some(proto::InvalidationCursor {
            owner_epoch: Uuid::new_v4().to_string(),
            sequence: 0,
        }),
        limit: 1,
    };
    let (a, b) = match owner {
        InvalidationOwner::Auth => {
            let mut c=proto::auth_concert_invalidation_service_client::AuthConcertInvalidationServiceClient::new(channel);
            (
                c.read_head(metadata(query, Workload::Gateway))
                    .await
                    .unwrap_err(),
                c.read_after(metadata(after, Workload::Gateway))
                    .await
                    .unwrap_err(),
            )
        }
        InvalidationOwner::Friend => {
            let mut c=proto::friend_concert_invalidation_service_client::FriendConcertInvalidationServiceClient::new(channel);
            (
                c.read_head(metadata(query, Workload::Gateway))
                    .await
                    .unwrap_err(),
                c.read_after(metadata(after, Workload::Gateway))
                    .await
                    .unwrap_err(),
            )
        }
    };
    assert_eq!(a.code(), tonic::Code::PermissionDenied);
    assert_eq!(b.code(), tonic::Code::PermissionDenied);
    let public = tonic::transport::Endpoint::from_shared(server.endpoint.clone())?.tls_config(
        tonic::transport::ClientTlsConfig::new()
            .ca_certificate(tonic::transport::Certificate::from_pem(std::fs::read(
                root()?.join("pki/ca.crt"),
            )?))
            .domain_name(format!("{}.svc.internal", expected.as_str())),
    )?;
    if let Ok(Ok(channel)) =
        tokio::time::timeout(Duration::from_millis(500), public.connect()).await
    {
        let query = proto::ReadInvalidationHeadRequest {
            consumer_id: consumer.to_string(),
            request_id: Uuid::new_v4().to_string(),
        };
        let result=tokio::time::timeout(Duration::from_millis(500),async {match owner {
            InvalidationOwner::Auth=>proto::auth_concert_invalidation_service_client::AuthConcertInvalidationServiceClient::new(channel).read_head(metadata(query,Workload::Concert)).await,
            InvalidationOwner::Friend=>proto::friend_concert_invalidation_service_client::FriendConcertInvalidationServiceClient::new(channel).read_head(metadata(query,Workload::Concert)).await,
        }}).await;
        assert!(!matches!(result, Ok(Ok(_))));
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires invalidation_clients_smoke.py owned PG16 and TLS"]
async fn both_real_owner_mutations_replay_as_sealed_pages() -> anyhow::Result<()> {
    let auth = pool(InvalidationOwner::Auth).await?;
    let friend = pool(InvalidationOwner::Friend).await?;
    migrate(&auth, InvalidationOwner::Auth).await?;
    migrate(&friend, InvalidationOwner::Friend).await?;
    let a = Running::actual(InvalidationOwner::Auth, auth.clone()).await?;
    let f = Running::actual(InvalidationOwner::Friend, friend.clone()).await?;
    authentication(&a, InvalidationOwner::Auth).await?;
    authentication(&f, InvalidationOwner::Friend).await?;
    let ac = a.source(InvalidationOwner::Auth)?;
    let fc = f.source(InvalidationOwner::Friend)?;
    let consumer = Uuid::new_v4();
    let ah = ac.read_head(consumer, &budget()).await?.cursor();
    let fh = fc.read_head(consumer, &budget()).await?.cursor();
    assert_eq!(ah.sequence(), 0);
    assert_eq!(fh.sequence(), 0);
    assert_ne!(ah.epoch(), fh.epoch());
    assert!(ac
        .read_after(consumer, ah, 256, &budget())
        .await?
        .signals()
        .is_empty());
    assert_eq!(
        fc.read_after(consumer, ah, 1, &budget()).await.unwrap_err(),
        ReadError::OwnerMismatch
    );
    let wrong_server = a.source(InvalidationOwner::Friend)?;
    assert!(wrong_server.read_head(consumer, &budget()).await.is_err());
    drop(wrong_server);
    let u = user(&auth).await?;
    let family = Uuid::new_v4();
    let jti = session(&auth, u, family).await?;
    let before = ac.read_head(consumer, &budget()).await?.cursor();
    store(&auth).revoke(&jti).await?;
    let logout = ac.read_after(consumer, before, 256, &budget()).await?;
    assert!(logout
        .signals()
        .iter()
        .any(|s| s.reason() == SignalReason::AuthSessionChanged
            && s.scope() == SignalScope::Family { user: u, family }));
    let repeated = ac.read_after(consumer, before, 256, &budget()).await?;
    assert_eq!(logout.signals(), repeated.signals());
    assert_ne!(logout.query().request_id(), repeated.query().request_id());
    for (x, y) in logout.signals().iter().zip(repeated.signals()) {
        x.check_redelivery(y)?;
    }
    let second = Uuid::new_v4();
    session(&auth, u, second).await?;
    store(&auth).revoke_family(second).await?;
    PgUserRepository::new(auth.clone(), Arc::new(SystemAuthRuntime))
        .update_password_hash(u, "changed-private-fixture")
        .await?;
    sqlx::query("update users set status='SUSPENDED' where id=$1")
        .bind(u)
        .execute(&auth)
        .await?;
    sqlx::query("delete from users where id=$1")
        .bind(u)
        .execute(&auth)
        .await?;
    let repository = PgFollowRepository::new(friend.clone());
    let actor = Uuid::new_v4();
    let subject = Uuid::new_v4();
    repository.block(actor, subject, chrono::Utc::now()).await?;
    repository.unblock(actor, subject).await?;
    repository
        .apply_user_deletion(
            Uuid::new_v4(),
            subject,
            actor,
            chrono::Utc::now() + chrono::Duration::days(30),
        )
        .await?;
    // Both journals must hide uncommitted source changes and roll back with them.
    let rollback_user = user(&auth).await?;
    for (pool, source, sql, id) in [
        (
            &auth,
            &ac,
            "update users set status='SUSPENDED' where id=$1",
            rollback_user,
        ),
        (
            &friend,
            &fc,
            "insert into user_blocks(blocker_id,blocked_id,created_at) values($1,$2,now())",
            Uuid::new_v4(),
        ),
    ] {
        let before = source.read_head(consumer, &budget()).await?.cursor();
        let mut tx = pool.begin().await?;
        let mut query = sqlx::query(sql).bind(id);
        if source.owner() == InvalidationOwner::Friend {
            query = query.bind(Uuid::new_v4());
        }
        query.execute(&mut *tx).await?;
        assert_eq!(
            source.read_head(consumer, &budget()).await?.cursor(),
            before
        );
        tx.rollback().await?;
        assert_eq!(
            source.read_head(consumer, &budget()).await?.cursor(),
            before
        );
    }
    // Actual owning repository writes, never journal inserts, cross a full page.
    for _ in 0..260 {
        session(&auth, rollback_user, Uuid::new_v4()).await?;
        repository
            .block(actor, Uuid::new_v4(), chrono::Utc::now())
            .await?;
    }
    for (pool, source) in [(&auth, &ac), (&friend, &fc)] {
        let head = source.read_head(consumer, &budget()).await?.cursor();
        let signals = replay(source, consumer, head).await?;
        assert!(signals.len() > 256);
        let reasons: Vec<_> = signals.iter().map(VerifiedSignal::reason).collect();
        if source.owner() == InvalidationOwner::Auth {
            for reason in [
                SignalReason::AuthSessionChanged,
                SignalReason::AuthCredentialsChanged,
                SignalReason::AuthUserAuthorityChanged,
                SignalReason::AuthUserRemoved,
            ] {
                assert!(reasons.contains(&reason));
            }
        } else {
            assert!(reasons.contains(&SignalReason::FriendRelationshipChanged));
            assert!(reasons.contains(&SignalReason::FriendUserRemoved));
        }
        for cursor in [
            OwnerCursor::new(source.owner(), Uuid::new_v4(), 0)?,
            OwnerCursor::new(source.owner(), head.epoch(), head.sequence() + 1)?,
        ] {
            assert_eq!(
                source
                    .read_after(consumer, cursor, 256, &budget())
                    .await
                    .unwrap_err(),
                ReadError::CheckpointMismatch
            );
        }
        save(pool, head, &signals).await?;
    }
    drop(ac);
    drop(fc);
    a.close().await?;
    f.close().await?;
    auth.close().await;
    friend.close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires physical restart by invalidation_clients_smoke.py"]
async fn restart_preserves_both_owner_epochs_positions_and_signal_content() -> anyhow::Result<()> {
    for owner in [InvalidationOwner::Auth, InvalidationOwner::Friend] {
        let pool = pool(owner).await?;
        let server = Running::actual(owner, pool.clone()).await?;
        let source = server.source(owner)?;
        let consumer = Uuid::new_v4();
        let saved = sqlx::query(
            "select epoch,head,signals from invalidation_client_restart_fixture where singleton",
        )
        .fetch_one(&pool)
        .await?;
        let head = source.read_head(consumer, &budget()).await?.cursor();
        assert_eq!(head.epoch(), saved.get::<Uuid, _>("epoch"));
        assert_eq!(head.sequence(), saved.get::<i64, _>("head"));
        assert_eq!(
            snapshot(&replay(&source, consumer, head).await?),
            saved.get::<serde_json::Value, _>("signals")
        );
        drop(source);
        server.close().await?;
        pool.close().await;
    }
    Ok(())
}
