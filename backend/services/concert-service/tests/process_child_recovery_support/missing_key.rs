//! CR04 draft: omitted-key replacement observes quarantine; restored replacement
//! reuses the actual CR01 restart verification. Compilation/native: NOT_RUN.
use super::*;
use concert_service::product_rpc::scope_ledger::{ChildRecoveryKey, ChildRecoveryKeys};

async fn omitted_key(run: Uuid, instance: Uuid) -> anyhow::Result<()> {
    let state = load(run)?;
    anyhow::ensure!(
        state.instance == instance && state.old_pid != std::process::id(),
        "distinct physical replacement identity required"
    );
    let keys = Arc::new(ChildRecoveryKeys::new(
        ChildRecoveryKey::new("next-key-without-retained-original", &[90; 32])?,
        vec![],
    )?);
    let f = fixture::Fixture::with_keys(instance, false, keys).await?;
    let view = authority::RootReadView::decode(state.root_view.as_slice())?;
    let capability = proto::ResponseDelivery::decode(state.delivery.as_slice())?;
    let key = scope_key(
        view.scope
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("scope missing"))?,
    )?;
    wait_past_saved_expiry(&view).await?;
    invariant(&f, &key).await?;
    anyhow::ensure!(
        pending(&f, &key).await? == (2, 0),
        "pending original intents required"
    );
    anyhow::ensure!(
        intents(&f, &key).await? == state.intents,
        "omitted-key restart changed immutable references"
    );
    anyhow::ensure!(
        state.intents.len() == 2 && state.intents.iter().all(|i| i.key_id == "public-native-v1"),
        "fixture must omit the actual original required key"
    );
    for observation in &state.prepared {
        let pool = match observation.owner.as_str() {
            "auth" => &f.auth,
            "friend" => &f.friend,
            "concert" => &f.concert,
            "profile" => &f.profile,
            _ => anyhow::bail!("unknown exact prepared owner"),
        };
        anyhow::ensure!(
            prepared(pool)
                .await?
                .iter()
                .any(|(gid, xid)| gid == &observation.gid && xid == &observation.xid),
            "original native guard lost"
        );
    }
    writers(&f, state.subject, true).await?;
    old_content_denied(&f, &view, state.subject).await?;
    let publisher = DeliveryPublisher::new(f.ledger.clone(), f.registry.clone());
    anyhow::ensure!(
        complete(publisher.clone(), capability.clone()).await? == 14,
        "omitted key cannot restore lost-registry Delivery"
    );
    let mut worker = ChildRecoveryWorker::new(
        f.ledger.clone(),
        ChildRecoverySettings::new(
            2,
            Duration::from_secs(2),
            Duration::from_secs(6),
            Duration::from_millis(100),
        )?,
    )?;
    let mut attempted = 0;
    for _ in 0..3 {
        let counts = worker.tick().await?;
        attempted += counts.attempted;
        anyhow::ensure!(
            counts.completed == 0 && counts.timed_out == 0 && counts.pending == counts.attempted,
            "missing-key drain must retain every attempted intent"
        );
    }
    anyhow::ensure!(
        attempted == 4,
        "bounded retry must wrap and retry both unreadable intents"
    );
    anyhow::ensure!(
        pending(&f, &key).await? == (2, 0) && intents(&f, &key).await? == state.intents,
        "unreadable ciphertext was changed or acknowledged"
    );
    for pool in [&f.auth, &f.friend, &f.concert, &f.profile] {
        anyhow::ensure!(
            prepared(pool).await?.len() == 1,
            "missing-key retry released a native guard"
        );
    }
    writers(&f, state.subject, true).await?;
    invariant(&f, &key).await?;
    old_content_denied(&f, &view, state.subject).await?;
    anyhow::ensure!(
        complete(publisher, capability).await? == 14,
        "unreadable child retry fabricated DeliveryCompleted"
    );
    // Exit normally with all prepared work deliberately retained. Controller
    // reaps this exact PID, then starts another distinct replacement containing
    // the retained original key and actual typed adapters. No force rollback.
    line("MCR_CR04_KEY_OMITTED_OBSERVED")?;
    Ok(())
}

pub(super) async fn run() -> anyhow::Result<()> {
    let run: Uuid = std::env::var("MCR_PROCESS_RUN")?.parse()?;
    let instance: Uuid = std::env::var("MCR_PROCESS_INSTANCE")?.parse()?;
    anyhow::ensure!(
        !run.is_nil() && !instance.is_nil(),
        "explicit nonzero identities required"
    );
    match std::env::var(TEST_MODE)?.as_str() {
        "old" => old(run, instance).await,
        "missing-key" => {
            tokio::time::timeout(Duration::from_secs(90), omitted_key(run, instance)).await?
        }
        "restart" => {
            let keys = Arc::new(ChildRecoveryKeys::new(
                ChildRecoveryKey::new("next-key-without-retained-original", &[90; 32])?,
                vec![ChildRecoveryKey::new("public-native-v1", &[89; 32])?],
            )?);
            let f = fixture::Fixture::with_keys(instance, false, keys).await?;
            tokio::time::timeout(
                Duration::from_secs(90),
                restart_with_fixture(run, instance, f),
            )
            .await?
        }
        _ => anyhow::bail!("unsupported CR04 physical process mode"),
    }
}
