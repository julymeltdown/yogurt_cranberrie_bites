//! Reuses CF01 capture/environment and production owner adapters at 145a8e9.
//! Adds the actual Notification native settlement endpoint to their configured
//! producer map. Negative fault injection refuses Complete before forwarding;
//! every successful decision/settlement comes from the real owning service.
mod owners {
    //! CF01 adapted from owner_authority_support/owners.rs at 591d908a85a01fe0a494e04c0109bb0bec64a3e1; original SHA256 b9bbf048a2d236f759ac5eb5d935d3ccb0d7e9d76343428f23415d5fd3a37a00.
    //! Production owner stores, production guards and actual authenticated settlement
    //! clients. Main-process startup itself is qualified by the owner-store runner.
    use crate::{capture, environment};
    use platform::{
        actor_delegation::{
            relationship_read::{RelationshipReadSigner, RelationshipReadVerifier},
            ActorDelegationVerifier,
        },
        grpc_auth::Workload,
    };
    use sqlx::PgPool;
    use std::sync::Arc;

    pub(super) struct Running {
        pub(super) url: String,
        task: tokio::task::JoinHandle<()>,
    }
    impl Drop for Running {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    pub(super) fn signer() -> anyhow::Result<RelationshipReadSigner> {
        signer_for_gateway(capture::gateway()?.public_key())
    }
    pub(super) fn signer_for_gateway(key: [u8; 32]) -> anyhow::Result<RelationshipReadSigner> {
        let gateway = ActorDelegationVerifier::new(Workload::Concert, &[key])?;
        RelationshipReadSigner::from_seed(&[75; 32], &gateway)
    }
    fn verifier(owner: Workload, key: [u8; 32]) -> anyhow::Result<RelationshipReadVerifier> {
        RelationshipReadVerifier::new(
            owner,
            &[signer_for_gateway(key)?.public_key()],
            &ActorDelegationVerifier::new(owner, &[key])?,
        )
    }
    fn incoming(
        listener: tokio::net::TcpListener,
    ) -> impl futures::Stream<Item = Result<tokio::net::TcpStream, std::io::Error>> + Send + 'static
    {
        futures::stream::unfold(listener, |listener| async {
            let result = listener.accept().await.map(|(stream, _)| stream);
            Some((result, listener))
        })
    }

    pub(super) async fn auth(
        pool: PgPool,
        producer_url: &str,
        notification_url: &str,
    ) -> anyhow::Result<Running> {
        auth_for_gateway(
            pool,
            producer_url,
            notification_url,
            capture::gateway()?.public_key(),
        )
        .await
    }
    pub(super) async fn auth_for_gateway(
        pool: PgPool,
        producer_url: &str,
        notification_url: &str,
        key: [u8; 32],
    ) -> anyhow::Result<Running> {
        use auth_service::concert_authority::{proto, PgOwnerAuthority, RpcSettlementResolver};
        let tls = environment::tls(Workload::Auth)?;
        let resolver = RpcSettlementResolver::new(
            &tls,
            "auth-fixture",
            &[
                (proto::Producer::Concert, producer_url.to_owned()),
                (proto::Producer::Notification, notification_url.to_owned()),
            ],
        )?;
        let store = PgOwnerAuthority::new(pool, Arc::new(resolver))?;
        let server = auth_service::adapters::r#in::grpc::concert_authority::guarded_server(
            store,
            auth_service::concert_authority::owner_caller_registry()?,
            verifier(Workload::Auth, key)?,
        )?;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("https://{}", listener.local_addr()?);
        let server = tonic::transport::Server::builder()
            .tls_config(tls.server_config()?)?
            .add_service(server);
        let task = tokio::spawn(async move {
            server
                .serve_with_incoming(incoming(listener))
                .await
                .expect("actual Auth authority server");
        });
        Ok(Running { url, task })
    }
    pub(super) async fn friend(
        pool: PgPool,
        producer_url: &str,
        notification_url: &str,
    ) -> anyhow::Result<Running> {
        friend_for_gateway(
            pool,
            producer_url,
            notification_url,
            capture::gateway()?.public_key(),
        )
        .await
    }
    pub(super) async fn friend_for_gateway(
        pool: PgPool,
        producer_url: &str,
        notification_url: &str,
        key: [u8; 32],
    ) -> anyhow::Result<Running> {
        use friend_service::concert_authority::{proto, PgOwnerAuthority, RpcSettlementResolver};
        let tls = environment::tls(Workload::Friend)?;
        let resolver = RpcSettlementResolver::new(
            &tls,
            "friend-fixture",
            &[
                (proto::Producer::Concert, producer_url.to_owned()),
                (proto::Producer::Notification, notification_url.to_owned()),
            ],
        )?;
        let store = PgOwnerAuthority::new(pool, Arc::new(resolver))?;
        let server = friend_service::adapters::r#in::grpc::concert_authority::guarded_server(
            store,
            friend_service::concert_authority::owner_caller_registry()?,
            verifier(Workload::Friend, key)?,
        )?;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("https://{}", listener.local_addr()?);
        let server = tonic::transport::Server::builder()
            .tls_config(tls.server_config()?)?
            .add_service(server);
        let task = tokio::spawn(async move {
            server
                .serve_with_incoming(incoming(listener))
                .await
                .expect("actual Friend authority server");
        });
        Ok(Running { url, task })
    }
}
use crate::{capture, environment, target_client::RootTargetClient};
use concert_service::{
    adapters::out::relationship_authority::guarded::{ChildRecoveryReference, OwnerGuardScope},
    config::canonical_runtime::CanonicalRuntimeCore,
    notification_live::{
        proto,
        target_authority::{TargetAuthorityService, TargetReadSettler, POLICY},
        NotificationClient,
    },
    product_rpc::{
        child_recovery::ChildRecoverySettings,
        child_settlement::{NotificationInboxIntentSettler, NotificationTargetIntentSettler},
        scope_ledger::{
            ChildIntentSettler, ChildRecoveryKey, ChildRecoveryKeys, ScopeKey, ScopePhase,
        },
        settlement::PgNativeAbortResolver,
    },
};
use platform::{
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::guard_without_delegation,
};
use sqlx::PgPool;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
    time::Duration,
};
use tonic::{Request, Response, Status};
use uuid::Uuid;
struct Children {
    target: TargetReadSettler,
    notification: OnceLock<Arc<NotificationClient>>,
}
#[async_trait::async_trait]
impl ChildIntentSettler for Children {
    async fn recover_one(&self, r: &ChildRecoveryReference) -> Result<(), Status> {
        match r {
            ChildRecoveryReference::TargetRead { .. } => self.target.recover_target_read(r).await,
            ChildRecoveryReference::NotificationInbox { .. } => {
                self.notification
                    .get()
                    .ok_or_else(|| Status::unavailable("Notification fixture not composed"))?
                    .recover_notification_inbox(r)
                    .await
            }
            _ => Err(Status::failed_precondition("Unsupported fixture child")),
        }
    }
}
struct RefuseCompletion {
    inner: notification_service::concert::authority::NotificationInboxService,
    deny: Arc<AtomicBool>,
}
#[tonic::async_trait]
impl notification_service::concert::transport::proto::concert_inbox_service_server::ConcertInboxService for RefuseCompletion{
    async fn execute(&self,r:Request<notification_service::concert::transport::proto::ExecuteRequest>)->Result<Response<notification_service::concert::transport::proto::ExecuteResponse>,Status>{self.inner.execute(r).await}
    async fn check(&self,r:Request<notification_service::concert::transport::proto::GuardRequest>)->Result<Response<notification_service::concert::transport::proto::GuardReply>,Status>{self.inner.check(r).await}
    async fn complete(&self,r:Request<notification_service::concert::transport::proto::GuardRequest>)->Result<Response<notification_service::concert::transport::proto::GuardReply>,Status>{
        if self.deny.load(Ordering::SeqCst){return Err(Status::unavailable("Injected pre-forward completion refusal"));}
        self.inner.complete(r).await
    }
}
struct Tasks(Vec<tokio::task::JoinHandle<()>>);
impl Drop for Tasks {
    fn drop(&mut self) {
        for task in &self.0 {
            task.abort();
        }
    }
}
fn incoming(
    listener: tokio::net::TcpListener,
) -> impl futures::Stream<Item = Result<tokio::net::TcpStream, std::io::Error>> + Send + 'static {
    futures::stream::unfold(listener, |l| async {
        Some((l.accept().await.map(|(s, _)| s), l))
    })
}
fn spawn(
    f: impl std::future::Future<Output = Result<(), tonic::transport::Error>> + Send + 'static,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        f.await.expect("actual owned target service");
    })
}
async fn prepared(pool: &PgPool) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "select count(*) from pg_prepared_xacts where database=current_database()",
    )
    .fetch_one(pool)
    .await?)
}
async fn target_held(pool: &PgPool) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar("select count(*) from pg_prepared_xacts where database=current_database() and gid like 'mcr-notification-target:%'").fetch_one(pool).await?)
}
fn scope_key(scope: &OwnerGuardScope) -> anyhow::Result<ScopeKey> {
    let p = scope.producer_scope();
    Ok(ScopeKey {
        instance: p.instance_id.parse()?,
        scope: p.scope_id.parse()?,
        binding_digest: p
            .binding_digest
            .try_into()
            .map_err(|_| anyhow::anyhow!("digest"))?,
    })
}
async fn writer_refused(pool: &PgPool, event: Uuid, actor: Uuid) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("set local lock_timeout='50ms'")
        .execute(&mut *tx)
        .await?;
    let result=sqlx::query("update concert_event_admissions set restriction_revision=restriction_revision+1 where event_id=$1 and user_id=$2")
        .bind(event).bind(actor).execute(&mut *tx).await;
    anyhow::ensure!(result.is_err(), "actor-policy exclusion was released early");
    tx.rollback().await?;
    Ok(())
}
pub(super) async fn run() -> anyhow::Result<()> {
    use notification_service::concert::authority::{
        NotificationInboxService, INBOX_POLICY, SETTLEMENT_POLICY,
    };
    let auth = environment::pool("auth").await?;
    let friend = environment::pool("friend").await?;
    let concert = environment::pool("concert").await?;
    let notification = environment::pool("notification").await?;
    for pool in [&auth, &friend, &concert, &notification] {
        assert_eq!(prepared(pool).await?, 0);
    }
    let root_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let root_url = format!("https://{}", root_listener.local_addr()?);
    let notification_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let notification_url = format!("https://{}", notification_listener.local_addr()?);
    let auth_owner = owners::auth(auth.clone(), &root_url, &notification_url).await?;
    let friend_owner = owners::friend(friend.clone(), &root_url, &notification_url).await?;
    let instance = Uuid::new_v4();
    let children = Arc::new(Children {
        target: TargetReadSettler::new(concert.clone(), instance)?,
        notification: OnceLock::new(),
    });
    let core = CanonicalRuntimeCore::new(
        concert.clone(),
        instance,
        Arc::new(ChildRecoveryKeys::new(
            ChildRecoveryKey::new("target-test", &[113; 32])?,
            vec![],
        )?),
        children.clone(),
    )?;
    let client = Arc::new(NotificationClient::connect(
        &notification_url,
        "concert-fixture",
        Some(&environment::tls(Workload::Concert)?),
        Some(Arc::new(core.ledger())),
    )?);
    children
        .notification
        .set(client.clone())
        .map_err(|_| anyhow::anyhow!("single composition"))?;
    let fence = core.owner_fence(
        &auth_owner.url,
        &friend_owner.url,
        "concert-fixture",
        &environment::tls(Workload::Concert)?,
        owners::signer()?,
    )?;
    let callback = guard_without_delegation(
        &POLICY,
        TargetAuthorityService::new(concert.clone(), core.registry()).server(),
        CallerRegistry::new().register(Workload::Notification, "notification-fixture")?,
    )?;
    let settlement = guard_without_delegation(
        &concert_service::product_rpc::settlement::POLICY,
        core.settlement_service(Arc::new(PgNativeAbortResolver::new(
            concert.clone(),
            instance,
        )?))
        .server(),
        CallerRegistry::new()
            .register(Workload::Auth, "auth-fixture")?
            .register(Workload::Friend, "friend-fixture")?,
    )?;
    let mut tasks = Tasks(vec![spawn(
        tonic::transport::Server::builder()
            .tls_config(environment::tls(Workload::Concert)?.server_config()?)?
            .layer(CallDeadlineLayer)
            .add_service(callback)
            .add_service(settlement)
            .serve_with_incoming(incoming(root_listener)),
    )]);
    let targets = Arc::new(RootTargetClient::connect(
        &root_url,
        "notification-fixture",
        &environment::tls(Workload::Notification)?,
    )?);
    let inbox = NotificationInboxService::connect(
        notification.clone(),
        &auth_owner.url,
        &friend_owner.url,
        "notification-fixture",
        Some(&environment::tls(Workload::Notification)?),
        Some(targets.clone()),
    )
    .await?;
    let notification_settlement = guard_without_delegation(
        &SETTLEMENT_POLICY,
        inbox.settlement_server(),
        CallerRegistry::new()
            .register(Workload::Auth, "auth-fixture")?
            .register(Workload::Friend, "friend-fixture")?,
    )?;
    let deny = Arc::new(AtomicBool::new(true));
    let inbox_server=guard_without_delegation(&INBOX_POLICY,
        notification_service::concert::transport::proto::concert_inbox_service_server::ConcertInboxServiceServer::new(RefuseCompletion{inner:inbox,deny:deny.clone()}),
        CallerRegistry::new().register(Workload::Concert,"concert-fixture")?)?;
    tasks.0.push(spawn(
        tonic::transport::Server::builder()
            .tls_config(environment::tls(Workload::Notification)?.server_config()?)?
            .layer(CallDeadlineLayer)
            .add_service(inbox_server)
            .add_service(notification_settlement)
            .serve_with_incoming(incoming(notification_listener)),
    ));
    let (actor, family, _) = environment::identities(&auth, 0).await?;
    let event = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Target native fixture','OPEN')").bind(event).bind(format!("target-{}",event.simple())).execute(&concert).await?;
    sqlx::query(
        "insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')",
    )
    .bind(event)
    .bind(actor)
    .execute(&concert)
    .await?;
    sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(event).bind(actor).execute(&concert).await?;
    // Actual mTLS callback refusals must leave no target native child behind.
    // Use a separate original scope so these adversarial calls cannot consume
    // the successful dispatch's original deadline.
    let negative_scope = fence
        .acquire(capture::binding(actor, family, event, true).await?)
        .await?;
    let (_negative_delivery, _negative_serialized) =
        core.delivery().publish(negative_scope.clone()).await?;
    {
        use concert_application::notification::{Candidate, Context};
        use concert_domain::notification::{Target, TargetType};
        use notification_service::concert::authority::RootTargetSnapshotOwner;
        use prost::Message;
        let wire = proto::ParentScope {
            binding: Some(negative_scope.wire_binding()),
            auth_view: Some(negative_scope.auth_view()),
            friend_view: Some(negative_scope.friend_view()),
            root_view: Some(negative_scope.root_read_view()),
        };
        let parent = notification_service::concert::transport::proto::ParentScope::decode(
            wire.encode_to_vec().as_slice(),
        )?;
        let b = negative_scope.binding();
        let context = Context {
            actor_id: actor,
            event_id: event,
            session_family_id: family,
            request_id: b.request_id(),
            authority_digest: b.digest(),
            observed_at: chrono::Utc::now(),
            expires_at: chrono::DateTime::from_timestamp_micros(b.deadline().expires_micros())
                .unwrap(),
        };
        for kind in [Some(TargetType::Post), Some(TargetType::Report), None] {
            let candidate = Candidate {
                notification_id: Uuid::new_v4(),
                source_actor_id: None,
                target: kind.map(|target_type| Target {
                    target_type,
                    target_id: Uuid::new_v4(),
                }),
            };
            let refused = platform::call_deadline::scope(
                b.deadline().stop().into_std(),
                targets.authorize(&parent, context, &[candidate], chrono::Utc::now()),
            )
            .await;
            assert!(
                refused.is_err(),
                "unknown owner must never produce Visible/Stale"
            );
        }
        let mut foreign = context;
        foreign.actor_id = Uuid::new_v4();
        assert!(platform::call_deadline::scope(
            b.deadline().stop().into_std(),
            targets.authorize(&parent, foreign, &[], chrono::Utc::now())
        )
        .await
        .is_err());
        let mut rebound = parent.clone();
        rebound.root_view.as_mut().unwrap().view_secret[0] ^= 1;
        assert!(platform::call_deadline::scope(
            b.deadline().stop().into_std(),
            targets.authorize(&rebound, context, &[], chrono::Utc::now())
        )
        .await
        .is_err());
        let wrong_token = RootTargetClient::connect(
            &root_url,
            "invalid-notification-credential",
            &environment::tls(Workload::Notification)?,
        )?;
        assert!(platform::call_deadline::scope(
            b.deadline().stop().into_std(),
            wrong_token.authorize(&parent, context, &[], chrono::Utc::now())
        )
        .await
        .is_err());
        assert_eq!(target_held(&concert).await?, 0);
    }
    negative_scope.mark_serialized().await?;
    negative_scope.finish_response().await?;
    drop(negative_scope);
    let scope = fence
        .acquire(capture::binding(actor, family, event, true).await?)
        .await?;
    let (delivery, _serialized_token) = core.delivery().publish(scope.clone()).await?;
    let k = scope_key(&scope)?;
    let reply = platform::call_deadline::scope(
        scope.binding().deadline().stop().into_std(),
        client.execute(
            scope.clone(),
            proto::InboxOperation::UnreadCount,
            proto::execute_request::Input::Count(proto::CountInput {}),
        ),
    )
    .await?;
    assert!(matches!(
        reply.output,
        Some(proto::execute_response::Output::Count(proto::UnreadCount {
            unread_total: 0,
            ..
        }))
    ));
    assert_eq!(
        target_held(&concert).await?,
        1,
        "empty snapshot must retain actor/event policy"
    );
    writer_refused(&concert, event, actor).await?;
    let count:i64=sqlx::query_scalar("select count(*) from concert_product_child_intents where instance_id=$1 and scope_id=$2 and completed_at is null")
        .bind(instance).bind(k.scope).fetch_one(&concert).await?;
    assert_eq!(count, 2, "separate Inbox and Target read intents");
    scope.mark_serialized().await?;
    assert!(
        scope.finish_response().await.is_err(),
        "injected Notification completion must keep Root unresolved"
    );
    assert_eq!(
        core.ledger().resolve_scope(&k).await?,
        ScopePhase::RootFinished
    );
    assert_eq!(target_held(&concert).await?, 1);
    // Preserve locks even after the original wall/monotonic disclosure expires.
    tokio::time::sleep_until(scope.binding().deadline().stop() + Duration::from_millis(20)).await;
    let mut worker = core.child_recovery_worker(ChildRecoverySettings::default())?;
    worker.tick().await?;
    assert_eq!(
        target_held(&concert).await?,
        1,
        "sweeper may not bypass pending Inbox"
    );
    writer_refused(&concert, event, actor).await?;
    deny.store(false, Ordering::SeqCst);
    for _ in 0..3 {
        worker.tick().await?;
    }
    assert_eq!(
        target_held(&concert).await?,
        0,
        "actual native abort should release Target locks"
    );
    let pending:i64=sqlx::query_scalar("select count(*) from concert_product_child_intents where instance_id=$1 and scope_id=$2 and completed_at is null")
        .bind(instance).bind(k.scope).fetch_one(&concert).await?;
    assert_eq!(pending, 0);
    for pool in [&auth, &friend, &concert, &notification] {
        assert_eq!(prepared(pool).await?, 0, "all actual native guards settled");
    }
    assert_eq!(delivery.instance_id, instance.to_string());
    drop(scope);
    drop(tasks);
    drop(auth_owner);
    drop(friend_owner);
    Ok(())
}
