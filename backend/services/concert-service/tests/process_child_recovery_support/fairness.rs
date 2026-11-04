//! CR06 private source: two canonical terminal roots and concurrent bounded recovery.
//! Compile/native NOT_RUN. No database lifecycle/ACK writes or successful doubles.
use super::*;
use concert_service::adapters::out::relationship_authority::guarded::ChildRecoveryReference;

struct Acquired {
    root: Root,
    _scope: Arc<OwnerGuardScope>,
}
// Private capabilities are deliberately not Debug and never printed.
#[derive(Serialize, Deserialize)]
struct Root {
    actor: Uuid,
    subject: Uuid,
    root_view: Vec<u8>,
    delivery: Vec<u8>,
    intents: Vec<Intent>,
    auth_gid: String,
    friend_gid: String,
}
#[derive(Serialize, Deserialize)]
struct MultiHandoff {
    run: Uuid,
    old_pid: u32,
    instance: Uuid,
    roots: Vec<Root>,
    prepared: Vec<Prepared>,
}
fn decode_root(
    root: &Root,
) -> anyhow::Result<(authority::RootReadView, proto::ResponseDelivery, ScopeKey)> {
    let view = authority::RootReadView::decode(root.root_view.as_slice())?;
    let delivery = proto::ResponseDelivery::decode(root.delivery.as_slice())?;
    let key = scope_key(
        view.scope
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("saved scope missing"))?,
    )?;
    Ok((view, delivery, key))
}
fn save_multi(state: &MultiHandoff) -> anyhow::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(state_path(state.run)?)?;
    file.write_all(&serde_json::to_vec(state)?)?;
    file.sync_all()?;
    Ok(())
}
fn load_multi(run: Uuid) -> anyhow::Result<MultiHandoff> {
    let path = state_path(run)?;
    let meta = std::fs::symlink_metadata(&path)?;
    let owner = std::fs::metadata(environment::root()?)?;
    anyhow::ensure!(
        meta.is_file()
            && !meta.file_type().is_symlink()
            && meta.uid() == owner.uid()
            && meta.permissions().mode() & 0o777 == 0o600
            && meta.len() <= 1_000_000,
        "private exact CR06 handoff required"
    );
    let state: MultiHandoff = serde_json::from_slice(&std::fs::read(path)?)?;
    anyhow::ensure!(
        state.run == run && state.roots.len() == 2 && state.prepared.len() == 8,
        "exact two-root CR06 handoff required"
    );
    Ok(state)
}
async fn owner_gid(
    pool: &PgPool,
    key: &ScopeKey,
    actor: Uuid,
    subject: Uuid,
) -> anyhow::Result<String> {
    let rows:Vec<(String,Vec<Uuid>,Vec<u8>)> = sqlx::query_as(
        "select prepared_gid,enrolled_subjects,decode(response_scope#>>'{}','base64') from public.concert_owner_guard_leases where binding_digest=$1 and state='ACTIVE'")
        .bind(key.binding_digest.as_slice()).fetch_all(pool).await?;
    let mut exact = Vec::new();
    for (gid, subjects, encoded) in rows {
        if owner_scope_matches(&encoded, key)? {
            anyhow::ensure!(
                subjects.len() == 2 && subjects.contains(&actor) && subjects.contains(&subject),
                "real enrolled owner subjects required"
            );
            anyhow::ensure!(
                prepared(pool)
                    .await?
                    .iter()
                    .any(|(actual, _)| actual == &gid),
                "real matching owner PREPARE required"
            );
            exact.push(gid);
        }
    }
    anyhow::ensure!(exact.len() == 1, "exact canonical owner required");
    Ok(exact.remove(0))
}
async fn acquire(
    f: &fixture::Fixture,
    actor: Uuid,
    family: Uuid,
    subject: Uuid,
) -> anyhow::Result<Acquired> {
    let producer = Arc::new(ReadProducer {
        reader: f.reader.clone(),
        subject,
        captured: Mutex::new(None),
    });
    let publisher = DeliveryPublisher::new(f.ledger.clone(), f.registry.clone());
    let owner = ScopedProductOwner::new(
        Arc::new(f.fence.clone()),
        publisher.clone(),
        producer.clone(),
    );
    let signer = capture::gateway()?;
    let service = guard(
        &product_rpc::POLICY,
        proto::product_service_server::ProductServiceServer::new(ProductRpcService::new(Arc::new(
            owner,
        ))),
        CallerRegistry::new().register(Workload::Gateway, "gateway-fixture")?,
        ActorDelegationVerifier::new(Workload::Concert, &[signer.public_key()])?,
    )?;
    let service =
        platform::call_deadline::CallDeadlineLayer.layer(RetainedScopeLayer.layer(service));
    let now = chrono::Utc::now().timestamp_micros();
    let message = proto::ProductRequest {
        user_id: actor.to_string(),
        event_id: f.event.to_string(),
        session_family_id: family.to_string(),
        request_id: Uuid::new_v4().to_string(),
        http_method: "GET".into(),
        http_target: format!("/api/concert/events/{}/feed", f.event),
        http_body: Vec::new(),
        content_type: String::new(),
        idempotency_key: String::new(),
        authority_expires_unix_micros: now + 1_800_000,
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
        access_expires_unix_micros: now + 60_000_000,
        ..Default::default()
    };
    let bytes = frame(&message);
    let proof = signer.issue(
        Workload::Concert,
        DISPATCH,
        actor,
        None,
        &bytes,
        chrono::Utc::now().timestamp(),
    );
    let mut req = request(DISPATCH, bytes)?;
    req.headers_mut().insert(HEADER, proof.parse()?);
    let response = service.oneshot(req).await?;
    let headers = response.headers().clone();
    // Actual Concert gRPC body EOF marks serialization through production Serialized.
    let body = response
        .into_body()
        .collect()
        .await
        .map_err(|_| anyhow::anyhow!("actual retained Product body failed"))?;
    anyhow::ensure!(
        grpc_code(&headers, body.trailers())? == 0,
        "guarded Product RPC denied"
    );
    let output: proto::ProductResponse = decode(&body.to_bytes())?;
    anyhow::ensure!(
        output.status == 403 && output.content_type == "application/json",
        "actual closed produced refusal required"
    );
    let document: serde_json::Value = serde_json::from_slice(&output.body)?;
    anyhow::ensure!(
        document["details"]["reason"] == "PERMISSION_DENIED",
        "actual negative Product contract required"
    );
    anyhow::ensure!(
        !String::from_utf8_lossy(&output.body).contains("private CR01 producer diagnostic")
            && !String::from_utf8_lossy(&output.body).contains("private-url-canary")
            && document.get("user_id").is_none(),
        "produced error disclosed private producer content"
    );
    let capability = output
        .delivery
        .ok_or_else(|| anyhow::anyhow!("delivery missing"))?;
    let scope = producer
        .captured
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| anyhow::anyhow!("scope missing"))?;
    let view = scope.root_read_view();
    let key = scope_key(&scope.producer_scope())?;
    anyhow::ensure!(
        f.ledger.resolve_scope(&key).await? == ScopePhase::Active,
        "EOF prematurely finished root"
    );
    let observed = Arc::new(AtomicI32::new(-1));
    let terminal = Arc::new(Terminal {
        publisher: publisher.clone(),
        capability: capability.clone(),
        observed: observed.clone(),
    });
    let outer = tower::service_fn(move |_: ()| {
        let token = ResponseScopeToken::new(terminal.clone());
        let bytes = output.body.clone();
        async move {
            let mut response = axum::http::Response::new(Full::new(Bytes::from(bytes)));
            response.extensions_mut().insert(token);
            Ok::<_, std::convert::Infallible>(response)
        }
    });
    let response = RetainedScopeLayer.layer(outer).oneshot(()).await?;
    anyhow::ensure!(
        response.into_body().collect().await.is_err(),
        "negative terminal fault missing"
    );
    anyhow::ensure!(
        observed.load(Ordering::SeqCst) == 14,
        "terminal callback did not return Unavailable"
    );
    invariant(f, &key).await?;
    let root = Root {
        actor,
        subject,
        root_view: view.encode_to_vec(),
        delivery: capability.encode_to_vec(),
        intents: intents(f, &key).await?,
        auth_gid: owner_gid(&f.auth, &key, actor, subject).await?,
        friend_gid: owner_gid(&f.friend, &key, actor, subject).await?,
    };
    anyhow::ensure!(
        root.intents.len() == 2 && pending(f, &key).await? == (2, 0),
        "two real pending children required"
    );
    Ok(Acquired {
        root,
        _scope: scope,
    })
}

async fn old_multi(run: Uuid, instance: Uuid) -> anyhow::Result<()> {
    let f = fixture::Fixture::new(instance, true).await?;
    for pool in [&f.auth, &f.friend, &f.concert, &f.profile] {
        anyhow::ensure!(
            prepared(pool).await?.is_empty(),
            "clean four-database fixture required"
        );
    }
    // Both data seeds precede every native guard: seed() updates event revision.
    let a = f.seed().await?;
    let b = f.seed().await?;
    anyhow::ensure!(
        a.0 != b.0 && a.2 != b.2,
        "distinct real fixture identities required"
    );
    let first = acquire(&f, a.0, a.1, a.2).await?;
    let second = acquire(&f, b.0, b.1, b.2).await?;
    f.make_child_settlement_unavailable();
    let mut worker = ChildRecoveryWorker::new(
        f.ledger.clone(),
        ChildRecoverySettings::new(
            4,
            Duration::from_secs(2),
            Duration::from_secs(10),
            Duration::from_millis(100),
        )?,
    )?;
    let unavailable = worker.tick().await?;
    anyhow::ensure!(
        unavailable.selected == 4
            && unavailable.attempted == 4
            && unavailable.completed == 0
            && unavailable.pending == 4,
        "all four actual children must survive settlement outage"
    );
    anyhow::ensure!(
        f.profile_completion_failures.load(Ordering::SeqCst) == 2,
        "both actual Profile Complete outage calls required"
    );
    for acquired in [&first, &second] {
        let (_, _, key) = decode_root(&acquired.root)?;
        invariant(&f, &key).await?;
        anyhow::ensure!(
            intents(&f, &key).await? == acquired.root.intents,
            "settlement outage changed immutable ciphertext"
        );
        writers(&f, acquired.root.subject, true).await?;
    }
    let registered: i64 = sqlx::query_scalar(
        "select count(*) from concert_public_profile_children where state='REGISTERED'",
    )
    .fetch_one(&f.profile)
    .await?;
    anyhow::ensure!(registered == 2, "two real Profile native receipts required");
    let mut native = Vec::new();
    for (owner, pool) in [
        ("auth", &f.auth),
        ("friend", &f.friend),
        ("concert", &f.concert),
        ("profile", &f.profile),
    ] {
        let rows = prepared(pool).await?;
        anyhow::ensure!(
            rows.len() == 2,
            "two actual native guards required in each database"
        );
        for (gid, xid) in rows {
            native.push(Prepared {
                owner: owner.into(),
                gid,
                xid,
            });
        }
    }
    save_multi(&MultiHandoff {
        run,
        old_pid: std::process::id(),
        instance,
        roots: vec![first.root, second.root],
        prepared: native,
    })?;
    line("MCR_CR06_OLD_READY")?;
    // _scope Arcs and services remain owned by this exact PID until SIGKILL.
    std::future::pending::<()>().await;
    drop(first._scope);
    drop(second._scope);
    drop(f);
    Ok(())
}

fn settings() -> anyhow::Result<ChildRecoverySettings> {
    Ok(ChildRecoverySettings::new(
        1,
        Duration::from_millis(250),
        Duration::from_secs(1),
        Duration::from_millis(100),
    )?)
}
fn bounded(counts: &product_rpc::child_recovery::ChildRecoveryCounts) -> anyhow::Result<()> {
    anyhow::ensure!(
        counts.selected <= 1
            && counts.attempted <= 1
            && counts.completed <= counts.attempted
            && counts.timed_out <= counts.attempted,
        "single-candidate configured budget exceeded"
    );
    Ok(())
}
async fn unique_acks(f: &fixture::Fixture, state: &MultiHandoff) -> anyhow::Result<i64> {
    let mut result = 0;
    for root in &state.roots {
        let (_, _, key) = decode_root(root)?;
        let (pending, complete) = super::pending(f, &key).await?;
        anyhow::ensure!(pending + complete == 2, "child row identity count changed");
        result += complete;
    }
    Ok(result)
}
async fn intact(f: &fixture::Fixture, state: &MultiHandoff) -> anyhow::Result<()> {
    for root in &state.roots {
        let (view, delivery, key) = decode_root(root)?;
        invariant(f, &key).await?;
        anyhow::ensure!(
            intents(f, &key).await? == root.intents,
            "key/nonce/identity/ciphertext changed across bounded recovery"
        );
        old_content_denied(f, &view, root.subject).await?;
        anyhow::ensure!(
            complete(
                DeliveryPublisher::new(f.ledger.clone(), f.registry.clone()),
                delivery
            )
            .await?
                == 14,
            "lost registry or child ACK manufactured DeliveryCompleted"
        );
    }
    Ok(())
}
async fn ack_snapshot(
    f: &fixture::Fixture,
    state: &MultiHandoff,
) -> anyhow::Result<Vec<Vec<(String, Uuid, Option<String>)>>> {
    let mut result = Vec::new();
    for root in &state.roots {
        let (_, _, key) = decode_root(root)?;
        result.push(acknowledgements(f, &key).await?);
    }
    Ok(result)
}

async fn restart_multi(run: Uuid, instance: Uuid) -> anyhow::Result<()> {
    let state = load_multi(run)?;
    anyhow::ensure!(
        state.instance == instance && state.old_pid != std::process::id(),
        "distinct physical replacement identity required"
    );
    let f = fixture::Fixture::new(instance, false).await?; // Fresh EMPTY registry.
    for (owner, pool) in [
        ("auth", &f.auth),
        ("friend", &f.friend),
        ("concert", &f.concert),
        ("profile", &f.profile),
    ] {
        let rows = prepared(pool).await?;
        let saved: Vec<_> = state.prepared.iter().filter(|p| p.owner == owner).collect();
        anyhow::ensure!(
            rows.len() == 2
                && saved.len() == 2
                && saved
                    .iter()
                    .all(|p| rows.iter().any(|(gid, xid)| gid == &p.gid && xid == &p.xid)),
            "exact original native GID/XID must survive process death"
        );
    }
    for root in &state.roots {
        let (_, _, key) = decode_root(root)?;
        anyhow::ensure!(
            pending(&f, &key).await? == (2, 0),
            "pending intent lost across physical restart"
        );
        anyhow::ensure!(
            owner_gid(&f.auth, &key, root.actor, root.subject).await? == root.auth_gid
                && owner_gid(&f.friend, &key, root.actor, root.subject).await? == root.friend_gid,
            "canonical owner identity changed"
        );
        writers(&f, root.subject, true).await?;
    }
    intact(&f, &state).await?;
    let first: (Uuid, Vec<u8>, String, Uuid) = sqlx::query_as(
        "select scope_id,binding_digest,kind,intent_id from concert_product_child_intents where instance_id=$1 and completed_at is null order by scope_id,kind,intent_id limit 1")
        .bind(instance).fetch_one(&f.concert).await?;
    anyhow::ensure!(
        first.2 == "LOCAL_PUBLIC",
        "first SQL-ordered actual child must be LocalPublic"
    );
    let digest: [u8; 32] = first.1.as_slice().try_into()?;
    let target = ScopeKey {
        instance,
        scope: first.0,
        binding_digest: digest,
    };
    let target_root = state
        .roots
        .iter()
        .find(|root| {
            decode_root(root).is_ok_and(|(_, _, key)| {
                key.scope == target.scope && key.binding_digest == target.binding_digest
            })
        })
        .ok_or_else(|| anyhow::anyhow!("first candidate not one of exact saved roots"))?;
    *f.hung_intent
        .lock()
        .map_err(|_| anyhow::anyhow!("fault target mutex"))? = Some((digest, first.3));
    let mut one = ChildRecoveryWorker::new(f.ledger.clone(), settings()?)?;
    let mut two = ChildRecoveryWorker::new(f.ledger.clone(), settings()?)?;
    let started = tokio::time::Instant::now();
    // Hold a real adapter future, after production decryption/parent validation.
    // Keep that future pending while probing the exact parent row separately.
    let (a, b) = {
        let first_tick = one.tick();
        tokio::pin!(first_tick);
        tokio::select! {
            _ = f.hung_entered.notified() => (),
            _ = &mut first_tick => anyhow::bail!("worker did not enter actual selected hung adapter"),
            _ = tokio::time::sleep(Duration::from_secs(2)) => anyhow::bail!("hung adapter entry deadline exceeded"),
        }
        let mut probe = f.concert.begin().await?;
        let lock = sqlx::query("select scope_id from concert_product_owner_scopes where instance_id=$1 and scope_id=$2 and binding_digest=$3 for update nowait")
            .bind(instance).bind(target.scope).bind(digest.as_slice()).fetch_one(&mut *probe).await;
        let rollback = probe.rollback().await;
        rollback?;
        anyhow::ensure!(
            lock.is_ok(),
            "ledger transaction held across real pending child call"
        );
        tokio::time::timeout(Duration::from_secs(2), async {
            tokio::join!(&mut first_tick, two.tick())
        })
        .await?
    };
    anyhow::ensure!(
        started.elapsed() < Duration::from_secs(3),
        "first concurrent configured cycles unbounded"
    );
    let a = a?;
    let b = b?;
    bounded(&a)?;
    bounded(&b)?;
    anyhow::ensure!(
        a.timed_out == 1 && a.completed == 0 && a.pending == 1,
        "actual first unresolved candidate must time out without ACK"
    );
    let mut reported_ack_successes = a.completed + b.completed;
    for _ in 0..10 {
        if unique_acks(&f, &state).await? == 3 {
            break;
        }
        let start = tokio::time::Instant::now();
        let (a, b) = tokio::time::timeout(Duration::from_secs(3), async {
            tokio::join!(one.tick(), two.tick())
        })
        .await?;
        anyhow::ensure!(
            start.elapsed() < Duration::from_secs(3),
            "bounded concurrent iteration deadline exceeded"
        );
        let a = a?;
        let b = b?;
        bounded(&a)?;
        bounded(&b)?;
        reported_ack_successes += a.completed + b.completed;
    }
    anyhow::ensure!(
        unique_acks(&f, &state).await? == 3 && reported_ack_successes >= 3,
        "unresolved first candidate starved later real children"
    );
    anyhow::ensure!(
        pending(&f, &target).await? == (1, 1),
        "first unknown intent was acknowledged"
    );
    let actual = f
        .hung_reference
        .lock()
        .map_err(|_| anyhow::anyhow!("captured actual reference mutex"))?
        .clone()
        .ok_or_else(|| anyhow::anyhow!("production-validated original reference missing"))?;
    let ChildRecoveryReference::LocalPublic { native, .. } = &actual else {
        anyhow::bail!("unknown reference was not LocalPublic");
    };
    anyhow::ensure!(
        actual.query().binding_digest == target.binding_digest
            && actual.intent_id()? == first.3
            && actual.query().subject == target_root.subject,
        "hang target did not match immutable production-validated reference"
    );
    let gid = actual.local_prepared_gid()?;
    let current = prepared(&f.concert).await?;
    anyhow::ensure!(
        current.len() == 1
            && current[0].0 == gid
            && current[0].1 == (native.full_xid.parse::<u64>()? as u32).to_string()
            && state
                .prepared
                .iter()
                .any(|p| p.owner == "concert" && p.gid == current[0].0 && p.xid == current[0].1),
        "unknown native child no longer has its original exact GID/XID"
    );
    anyhow::ensure!(
        prepared(&f.profile).await?.is_empty(),
        "later Profile guards failed real settlement"
    );
    for root in &state.roots {
        local_writer(&f, root.subject, root.subject == target_root.subject).await?;
        profile_writer(&f, root.subject, false).await?;
    }
    for pool in [&f.auth, &f.friend] {
        anyhow::ensure!(
            prepared(pool).await?.len() == 2,
            "child-only drain released canonical owners early"
        );
    }
    let settled: i64 = sqlx::query_scalar(
        "select count(*) from concert_public_profile_children where state='SETTLED'",
    )
    .fetch_one(&f.profile)
    .await?;
    anyhow::ensure!(settled == 2, "two actual Profile SETTLED receipts required");
    intact(&f, &state).await?;
    let unchanged = ack_snapshot(&f, &state).await?;
    for _ in 0..4 {
        let (a, b) = tokio::time::timeout(Duration::from_secs(3), async {
            tokio::join!(one.tick(), two.tick())
        })
        .await?;
        let a = a?;
        let b = b?;
        bounded(&a)?;
        bounded(&b)?;
        anyhow::ensure!(
            a.completed == 0 && b.completed == 0,
            "completed child repeated after immutable ACK"
        );
    }
    anyhow::ensure!(
        ack_snapshot(&f, &state).await? == unchanged
            && unique_acks(&f, &state).await? == 3
            && f.hung_attempts.load(Ordering::SeqCst) >= 2,
        "concurrent retries duplicated or changed immutable ACKs"
    );
    // Restore the actual adapter on the SAME composed ledger, never another ledger.
    *f.hung_intent
        .lock()
        .map_err(|_| anyhow::anyhow!("fault target mutex"))? = None;
    let mut restored = ChildRecoveryWorker::new(f.ledger.clone(), settings()?)?;
    let final_tick = restored.tick().await?;
    bounded(&final_tick)?;
    anyhow::ensure!(
        final_tick.completed == 1 && unique_acks(&f, &state).await? == 4,
        "restored actual native adapter did not ACK the original unresolved intent"
    );
    anyhow::ensure!(
        prepared(&f.concert).await?.is_empty(),
        "restored actual Local guard remains"
    );
    for root in &state.roots {
        writers(&f, root.subject, false).await?;
    }
    let final_acks = ack_snapshot(&f, &state).await?;
    for _ in 0..2 {
        let tick = restored.tick().await?;
        anyhow::ensure!(
            tick.selected == 0 && tick.attempted == 0 && tick.completed == 0,
            "final completed rows reselected"
        );
    }
    anyhow::ensure!(
        ack_snapshot(&f, &state).await? == final_acks,
        "final ACK timestamps changed"
    );
    for root in &state.roots {
        let (view, _, _) = decode_root(root)?;
        wait_past_saved_expiry(&view).await?;
    }
    f.recover_owners().await?;
    for root in &state.roots {
        let (_, _, key) = decode_root(root)?;
        completed_owner(&f.auth, &key, &root.auth_gid).await?;
        completed_owner(&f.friend, &key, &root.friend_gid).await?;
    }
    for pool in [&f.auth, &f.friend, &f.concert, &f.profile] {
        anyhow::ensure!(
            prepared(pool).await?.is_empty(),
            "normal settlement must release all eight native guards"
        );
    }
    intact(&f, &state).await?;
    // The unique SQL rows, rather than overlapping worker success counts, certify effects.
    anyhow::ensure!(
        unique_acks(&f, &state).await? == 4 && ack_snapshot(&f, &state).await? == final_acks,
        "owner recovery changed unique child receipts"
    );
    let path = environment::root()?.join(format!("process-cr06-{}-counters.json", run.simple()));
    let mut counters = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    counters.write_all(&serde_json::to_vec(&serde_json::json!({
        "run": run,
        "replacement_pid": std::process::id(),
        "ack_success_count_before_restore": reported_ack_successes,
        "unique_ack_rows_before_restore": 3,
        "restored_actual_adapter_ack_success_count": final_tick.completed,
        "unique_ack_rows_after_restore": 4,
        "hung_adapter_attempts": f.hung_attempts.load(Ordering::SeqCst),
        "per_tick_max_selected": 1,
        "per_tick_max_attempted": 1,
        "per_child_timeout_millis": 250,
        "system_batch_budget_millis": 1000,
        "normal_native_settlement": true,
        "ledger_row_nowait_while_child_pending": "PASS"
    }))?)?;
    counters.sync_all()?;
    line("MCR_CR06_RESTART_OBSERVED")?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires owned PG16 and source-bound CR06 physical process controller; NOT_RUN"]
async fn process_root_child_recovery_cr06() -> anyhow::Result<()> {
    let run: Uuid = std::env::var("MCR_PROCESS_RUN")?.parse()?;
    let instance: Uuid = std::env::var("MCR_PROCESS_INSTANCE")?.parse()?;
    anyhow::ensure!(
        !run.is_nil() && !instance.is_nil(),
        "nonzero physical run identity required"
    );
    match std::env::var(TEST_MODE)?.as_str() {
        "old" => old_multi(run, instance).await,
        "restart" => {
            tokio::time::timeout(Duration::from_secs(90), restart_multi(run, instance)).await?
        }
        _ => anyhow::bail!("explicit CR06 process mode required"),
    }
}
