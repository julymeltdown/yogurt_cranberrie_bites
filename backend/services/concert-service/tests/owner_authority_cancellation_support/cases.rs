use super::{capture, environment, fixture::Fixture};
use concert_service::adapters::out::relationship_authority::guarded::OwnerGuardScope;
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use tokio::sync::Notify;
use tonic::Status;
use uuid::Uuid;

async fn evidence(
    pool: &PgPool,
    scope: &OwnerGuardScope,
) -> anyhow::Result<(String, bool, bool, bool, bool)> {
    let producer = scope.producer_scope();
    Ok(sqlx::query_as("SELECT phase,response_abandoned_at IS NOT NULL,cancellation_settled_at IS NOT NULL,serialized_at IS NOT NULL,delivery_completed_at IS NOT NULL FROM concert_product_owner_scopes WHERE instance_id=$1 AND scope_id=$2")
        .bind(Uuid::parse_str(&producer.instance_id)?).bind(Uuid::parse_str(&producer.scope_id)?).fetch_one(pool).await?)
}
async fn prepared(pool: &PgPool) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM pg_prepared_xacts WHERE database=current_database()",
    )
    .fetch_one(pool)
    .await?)
}
async fn wait_phase(f: &Fixture, scope: &OwnerGuardScope, phase: &str) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if evidence(&f.concert, scope).await?.0 == phase {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await??;
    Ok(())
}
async fn setup() -> anyhow::Result<(Fixture, Arc<OwnerGuardScope>)> {
    let f = Fixture::with_native_abort().await?;
    let (actor, family, _) = environment::identities(&f.auth, 0).await?;
    let scope = f
        .fence
        .acquire(capture::binding(actor, family).await?)
        .await?;
    Ok((f, scope))
}

#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS CR08 controller"]
async fn returned_error_records_abandonment_without_delivery() -> anyhow::Result<()> {
    let (f, scope) = setup().await?;
    let publisher = f.delivery_publisher();
    let owned = scope.clone();
    let error = scope
        .run_owned(async move {
            let capability = publisher.publish(owned).await?;
            drop(capability);
            Err::<(), _>(Status::unavailable("actual producer returned error"))
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), tonic::Code::Unavailable);
    assert_eq!(
        evidence(&f.concert, &scope).await?,
        ("ROOT_FINISHED".into(), true, true, false, false)
    );
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (0, 0)
    );
    assert!(scope.mark_serialized().await.is_err());
    assert!(
        scope.finish_response().await.is_err(),
        "abandonment never certifies HTTP delivery"
    );
    assert!(
        scope.run_owned(async { Ok(()) }).await.is_err(),
        "one supervisor only"
    );
    f.assert_no_recovery_calls();
    Ok(())
}

struct ActualWorkDrop(Arc<Notify>);
impl Drop for ActualWorkDrop {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}

#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS CR08 controller"]
async fn caller_cancellation_destroys_actual_work_before_owner_release() -> anyhow::Result<()> {
    let (f, scope) = setup().await?;
    let publisher = f.delivery_publisher();
    let owned = scope.clone();
    let task_scope = scope.clone();
    let entered = Arc::new(Notify::new());
    let observed = entered.clone();
    let dropped = Arc::new(Notify::new());
    let drop_signal = dropped.clone();
    let original = std::time::Instant::now() + Duration::from_millis(1800);
    let task = tokio::spawn(platform::call_deadline::scope(original, async move {
        task_scope
            .run_owned(async move {
                let _actual = ActualWorkDrop(drop_signal);
                assert_eq!(platform::call_deadline::current(), Some(original));
                let _capability = publisher.publish(owned).await?;
                observed.notify_one();
                std::future::pending::<Result<(), Status>>().await
            })
            .await
    }));
    tokio::time::timeout(Duration::from_secs(1), entered.notified()).await?;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(1), dropped.notified()).await?;
    wait_phase(&f, &scope, "ROOT_FINISHED").await?;
    tokio::time::timeout(Duration::from_secs(3), async {
        while !evidence(&f.concert, &scope).await?.2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    assert_eq!(
        evidence(&f.concert, &scope).await?,
        ("ROOT_FINISHED".into(), true, true, false, false)
    );
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (0, 0)
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS CR08 controller"]
async fn unresolved_callback_and_expiry_never_prove_abandonment() -> anyhow::Result<()> {
    let (f, scope) = setup().await?;
    let view = scope.root_read_view();
    let callback = f.registry.lookup_root_read(&view)?;
    let publisher = f.delivery_publisher();
    let owned = scope.clone();
    let result = scope
        .run_owned(async move {
            let _capability = publisher.publish(owned).await?;
            Err::<(), _>(Status::cancelled(
                "actual producer failure with admitted callback",
            ))
        })
        .await;
    assert!(result.is_err());
    assert_eq!(
        evidence(&f.concert, &scope).await?,
        ("SEALED".into(), false, false, false, false)
    );
    assert!(
        f.registry.lookup_root_read(&view).is_err(),
        "late callback admission closed"
    );
    tokio::time::sleep_until(scope.binding().deadline().stop() + Duration::from_millis(40)).await;
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (1, 1)
    );
    drop(callback);
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(
        evidence(&f.concert, &scope).await?,
        ("SEALED".into(), false, false, false, false)
    );
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (1, 1)
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS CR08 controller"]
async fn active_native_transaction_prevents_cancellation_settlement() -> anyhow::Result<()> {
    let (f, scope) = setup().await?;
    // Actual remote/native work may outlive the local handler. Its exact durable
    // registration must keep owner fences even after truthful root abandonment.
    let mut native = f.concert.begin().await?;
    scope
        .register_transaction_in(&mut native, [77; 32], Uuid::new_v4().as_bytes().to_vec())
        .await?;
    let publisher = f.delivery_publisher();
    let owned = scope.clone();
    assert!(scope
        .run_owned(async move {
            let _capability = publisher.publish(owned).await?;
            Err::<(), _>(Status::cancelled("actual root error with live native work"))
        })
        .await
        .is_err());
    assert_eq!(
        evidence(&f.concert, &scope).await?,
        ("ROOT_FINISHED".into(), true, false, false, false)
    );
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (1, 1)
    );
    native.rollback().await?;
    // Actual native rollback is cleanup, not an invented cancellation receipt.
    assert!(!evidence(&f.concert, &scope).await?.2);
    Ok(())
}

#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS CR08 controller"]
async fn successful_handoff_retains_existing_unserialized_quarantine() -> anyhow::Result<()> {
    let (f, scope) = setup().await?;
    let publisher = f.delivery_publisher();
    let owned = scope.clone();
    let capability = scope
        .run_owned(async move { publisher.publish(owned).await })
        .await?;
    drop(capability);
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert_eq!(
        evidence(&f.concert, &scope).await?,
        ("ACTIVE".into(), false, false, false, false)
    );
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (1, 1)
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS CR08 controller"]
async fn drained_late_callback_allows_true_abandonment() -> anyhow::Result<()> {
    let (f, scope) = setup().await?;
    let view = scope.root_read_view();
    let callback = f.registry.lookup_root_read(&view)?;
    let publisher = f.delivery_publisher();
    let owned = scope.clone();
    let task_scope = scope.clone();
    let task = tokio::spawn(async move {
        task_scope
            .run_owned(async move {
                let _capability = publisher.publish(owned).await?;
                Err::<(), _>(Status::cancelled(
                    "root work ended before admitted callback",
                ))
            })
            .await
    });
    wait_phase(&f, &scope, "SEALED").await?;
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (1, 1)
    );
    assert!(!evidence(&f.concert, &scope).await?.1);
    let late = callback.scope();
    assert!(late
        .public_profile_subject(late.binding().actor())
        .await
        .is_err());
    drop(callback);
    assert!(task.await?.is_err());
    assert_eq!(
        evidence(&f.concert, &scope).await?,
        ("ROOT_FINISHED".into(), true, true, false, false)
    );
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (0, 0)
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS CR08 controller"]
async fn expired_original_deadline_never_polls_ready_producer() -> anyhow::Result<()> {
    let (f, scope) = setup().await?;
    let polled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed = polled.clone();
    let error = platform::call_deadline::scope(
        std::time::Instant::now() - Duration::from_millis(1),
        scope.run_owned(async move {
            observed.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), tonic::Code::DeadlineExceeded);
    assert!(!polled.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(
        evidence(&f.concert, &scope).await?,
        ("ROOT_FINISHED".into(), true, true, false, false)
    );
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (0, 0)
    );
    Ok(())
}

struct PanickingWorkDrop(Arc<Notify>);
impl Drop for PanickingWorkDrop {
    fn drop(&mut self) {
        self.0.notify_one();
        panic!("negative actual producer destructor fault");
    }
}
#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS CR08 controller"]
async fn failed_work_destruction_never_reaches_terminal_proof() -> anyhow::Result<()> {
    let (f, scope) = setup().await?;
    let publisher = f.delivery_publisher();
    let owned = scope.clone();
    let task_scope = scope.clone();
    let entered = Arc::new(Notify::new());
    let observed = entered.clone();
    let dropped = Arc::new(Notify::new());
    let drop_signal = dropped.clone();
    let task = tokio::spawn(async move {
        task_scope
            .run_owned(async move {
                let _actual = PanickingWorkDrop(drop_signal);
                let _capability = publisher.publish(owned).await?;
                observed.notify_one();
                std::future::pending::<Result<(), Status>>().await
            })
            .await
    });
    tokio::time::timeout(Duration::from_secs(1), entered.notified()).await?;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(1), dropped.notified()).await?;
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(
        evidence(&f.concert, &scope).await?,
        ("ACTIVE".into(), false, false, false, false)
    );
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (1, 1)
    );
    Ok(())
}

struct RejectChild;
#[async_trait::async_trait]
impl concert_service::product_rpc::scope_ledger::ChildIntentSettler for RejectChild {
    async fn recover_one(
        &self,
        _: &concert_service::adapters::out::relationship_authority::guarded::ChildRecoveryReference,
    ) -> Result<(), Status> {
        Err(Status::unavailable("no actual child in capacity fixture"))
    }
}
#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS CR08 controller"]
async fn unpublished_unsettled_abandonment_remains_in_durable_capacity() -> anyhow::Result<()> {
    use concert_service::product_rpc::scope_ledger::{
        ChildRecoveryKey, ChildRecoveryKeys, PgScopeLedger, ScopeKey,
    };
    let (f, scope) = setup().await?;
    // A real database lock prevents actual Auth Complete from changing its
    // durable lease. No synthetic success/failure owner resolver is installed.
    let mut blocker = f.auth.begin().await?;
    sqlx::query("LOCK TABLE concert_owner_guard_leases IN SHARE MODE NOWAIT")
        .execute(&mut *blocker)
        .await?;
    assert!(scope
        .run_owned(async { Err::<(), _>(Status::cancelled("unpublished owned work failed")) })
        .await
        .is_err());
    assert_eq!(
        evidence(&f.concert, &scope).await?,
        ("ROOT_FINISHED".into(), true, false, false, false)
    );
    let value = scope.producer_scope();
    let instance = Uuid::parse_str(&value.instance_id)?;
    let (binding,hash):(serde_json::Value,Option<Vec<u8>>)=sqlx::query_as("SELECT binding,delivery_secret_hash FROM concert_product_owner_scopes WHERE instance_id=$1 AND scope_id=$2")
        .bind(instance).bind(Uuid::parse_str(&value.scope_id)?).fetch_one(&f.concert).await?;
    assert!(
        hash.is_none(),
        "unpublished cancellation has no delivery capability"
    );
    let binding: platform::actor_delegation::owner_binding::OwnerBinding =
        serde_json::from_value(binding)?;
    let ledger = PgScopeLedger::new(
        f.concert.clone(),
        instance,
        Arc::new(ChildRecoveryKeys::new(
            ChildRecoveryKey::new("canonical-fixture-v1", &[88; 32])?,
            vec![],
        )?),
        Arc::new(RejectChild),
    )?;
    // This replacement ledger/empty registry exercises durable admission only;
    // no raw row is promoted into a positive owner or native completion proof.
    let _empty_registry =
        concert_service::adapters::out::relationship_authority::guarded::ScopeRegistry::new(
            instance,
            Arc::new(ledger.clone()),
        )?;
    for _ in 0..15 {
        ledger
            .register(
                &ScopeKey {
                    instance,
                    scope: Uuid::new_v4(),
                    binding_digest: scope.binding().digest(),
                },
                &binding,
            )
            .await?;
    }
    let denied = ledger
        .register(
            &ScopeKey {
                instance,
                scope: Uuid::new_v4(),
                binding_digest: scope.binding().digest(),
            },
            &binding,
        )
        .await
        .unwrap_err();
    assert_eq!(denied.code(), tonic::Code::ResourceExhausted);
    assert_eq!(
        (prepared(&f.auth).await?, prepared(&f.friend).await?),
        (1, 1)
    );
    blocker.rollback().await?;
    Ok(())
}
