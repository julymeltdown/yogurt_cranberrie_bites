//! CR03 only: unfinished ACTIVE and genuinely SEALED roots survive physical death.
//! Adapted from frozen CR01 public Product/reader fixture; see private draft provenance.
//! No phase/ACK writes, successful fake, prepared rollback or RootFinished promotion.
use super::*;

#[derive(Clone, Copy)]
enum Variant {
    Active,
    Sealed,
}
impl Variant {
    fn selected() -> anyhow::Result<Self> {
        match std::env::var("MCR_PROCESS_CASE")?.as_str() {
            "CR03_ACTIVE" => Ok(Self::Active),
            "CR03_SEALED" => Ok(Self::Sealed),
            _ => anyhow::bail!("explicit CR03 variant required"),
        }
    }
    fn phase(self) -> ScopePhase {
        match self {
            Self::Active => ScopePhase::Active,
            Self::Sealed => ScopePhase::Sealed,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Sealed => "SEALED",
        }
    }
    fn ready(self) -> &'static str {
        match self {
            Self::Active => "MCR_CR03_ACTIVE_OLD_READY",
            Self::Sealed => "MCR_CR03_SEALED_OLD_READY",
        }
    }
    fn observed(self) -> &'static str {
        match self {
            Self::Active => "MCR_CR03_ACTIVE_RESTART_OBSERVED",
            Self::Sealed => "MCR_CR03_SEALED_RESTART_OBSERVED",
        }
    }
}
struct Acquired {
    f: fixture::Fixture,
    scope: Arc<OwnerGuardScope>,
    view: authority::RootReadView,
    capability: proto::ResponseDelivery,
    actor: Uuid,
    subject: Uuid,
    key: ScopeKey,
}
// Flatten preserves the parent's controller identity fields and load() checks.
// Every field stays in the create-new 0600 handoff, never stdout or reports.
#[derive(Serialize, Deserialize)]
struct UnfinishedHandoff {
    #[serde(flatten)]
    common: Handoff,
    cr03_variant: String,
    native_receipts: serde_json::Value,
}
fn save_unfinished(value: &UnfinishedHandoff) -> anyhow::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(state_path(value.common.run)?)?;
    file.write_all(&serde_json::to_vec(value)?)?;
    file.sync_all()?;
    Ok(())
}
fn load_unfinished(run: Uuid) -> anyhow::Result<UnfinishedHandoff> {
    // Reuse exact parent's regular-file/no-symlink/uid/0600 and run checks.
    let _checked = load(run)?;
    let result: UnfinishedHandoff = serde_json::from_slice(&std::fs::read(state_path(run)?)?)?;
    anyhow::ensure!(result.common.run == run, "CR03 handoff run mismatch");
    Ok(result)
}
async fn acquire(instance: Uuid) -> anyhow::Result<Acquired> {
    let f = fixture::Fixture::new(instance, false).await?;
    for pool in [&f.auth, &f.friend, &f.concert, &f.profile] {
        anyhow::ensure!(
            prepared(pool).await?.is_empty(),
            "clean isolated databases required"
        );
    }
    let (actor, family, subject) = f.seed().await?;
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
    Ok(Acquired {
        f,
        scope,
        view,
        capability,
        actor,
        subject,
        key,
    })
}
async fn snapshot_prepared(f: &fixture::Fixture) -> anyhow::Result<Vec<Prepared>> {
    let mut result = Vec::new();
    for (owner, pool) in [
        ("auth", &f.auth),
        ("friend", &f.friend),
        ("concert", &f.concert),
        ("profile", &f.profile),
    ] {
        let native = prepared(pool).await?;
        anyhow::ensure!(
            native.len() == 1,
            "CR03 requires one actual prepared guard per store"
        );
        for (gid, xid) in native {
            result.push(Prepared {
                owner: owner.into(),
                gid,
                xid,
            });
        }
    }
    Ok(result)
}
async fn exact_prepared(f: &fixture::Fixture, saved: &[Prepared]) -> anyhow::Result<()> {
    anyhow::ensure!(saved.len() == 4, "four saved native identities required");
    for (owner, pool) in [
        ("auth", &f.auth),
        ("friend", &f.friend),
        ("concert", &f.concert),
        ("profile", &f.profile),
    ] {
        let expected: Vec<_> = saved.iter().filter(|v| v.owner == owner).collect();
        anyhow::ensure!(expected.len() == 1, "exact owner native identity required");
        let actual = prepared(pool).await?;
        anyhow::ensure!(
            actual.len() == 1 && actual[0].0 == expected[0].gid && actual[0].1 == expected[0].xid,
            "CR03 native identity or exclusion changed"
        );
    }
    Ok(())
}
async fn native_receipts(
    f: &fixture::Fixture,
    key: &ScopeKey,
    saved: &[Prepared],
) -> anyhow::Result<serde_json::Value> {
    let mut result = serde_json::Map::new();
    for (owner, pool) in [("auth", &f.auth), ("friend", &f.friend)] {
        let native = saved
            .iter()
            .find(|v| v.owner == owner)
            .ok_or_else(|| anyhow::anyhow!("owner identity missing"))?;
        // Retain actual full preparer XID/PID/start, root binding and guard identity.
        // Only legitimate owner state/seal timestamps may change after recover().
        let value:serde_json::Value=sqlx::query_scalar(
            "select to_jsonb(l)-'state'-'sealed_at'-'completed_at'-'scope_settlement' from public.concert_owner_guard_leases l where prepared_gid=$1 and binding_digest=$2")
            .bind(&native.gid).bind(key.binding_digest.as_slice()).fetch_one(pool).await?;
        result.insert(owner.into(), value);
    }
    let rows: Vec<serde_json::Value> = sqlx::query_scalar(
        "select to_jsonb(c) from public.concert_public_profile_children c order by child_id",
    )
    .fetch_all(&f.profile)
    .await?;
    anyhow::ensure!(
        rows.len() == 1,
        "one exact actual Profile native receipt required"
    );
    anyhow::ensure!(
        rows[0]["state"] == "REGISTERED" && rows[0]["full_xid"].as_str().is_some(),
        "Profile must remain genuinely prepared and REGISTERED"
    );
    result.insert("profile".into(), rows.into_iter().next().unwrap());
    Ok(serde_json::Value::Object(result))
}
async fn owner_unfinished(
    pool: &PgPool,
    key: &ScopeKey,
    gid: &str,
    after_recovery: bool,
) -> anyhow::Result<()> {
    let rows:Vec<(String,String,Vec<u8>,bool,bool)>=sqlx::query_as(
        "select prepared_gid,state,decode(response_scope#>>'{}','base64'),scope_settlement is null,completed_at is null from public.concert_owner_guard_leases where binding_digest=$1")
        .bind(key.binding_digest.as_slice()).fetch_all(pool).await?;
    let mut exact = Vec::new();
    for row in rows {
        if owner_scope_matches(&row.2, key)? {
            exact.push(row);
        }
    }
    anyhow::ensure!(
        exact.len() == 1 && exact[0].0 == gid && exact[0].3 && exact[0].4,
        "unfinished owner acquired completion or wrong root receipt"
    );
    anyhow::ensure!(
        if after_recovery {
            exact[0].1 == "SEALED"
        } else {
            matches!(exact[0].1.as_str(), "ACTIVE" | "SEALED")
        },
        "actual expired-owner sweep did not retain quarantine"
    );
    let capacity:i64=sqlx::query_scalar(
        "select count(*) from public.concert_owner_guard_leases where state not in ('COMPLETED','ABANDONED')")
        .fetch_one(pool).await?;
    anyhow::ensure!(capacity == 1, "owner quarantine must consume real capacity");
    Ok(())
}
async fn quarantine(
    f: &fixture::Fixture,
    key: &ScopeKey,
    variant: Variant,
    saved: &[Prepared],
    encrypted: &[Intent],
    receipts: &serde_json::Value,
    after_recovery: bool,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        f.ledger.resolve_scope(key).await? == variant.phase(),
        "unfinished root phase changed"
    );
    anyhow::ensure!(
        !f.ledger.delivery_completed(key).await?,
        "unfinished root gained DeliveryCompleted"
    );
    let tx:i64=sqlx::query_scalar(
        "select count(*) from concert_product_owner_transactions where instance_id=$1 and scope_id=$2 and binding_digest=$3")
        .bind(key.instance).bind(key.scope).bind(key.binding_digest.as_slice()).fetch_one(&f.concert).await?;
    anyhow::ensure!(
        tx == 0 && pending(f, key).await? == (2, 0),
        "unfinished root lost pending children or gained producer work"
    );
    anyhow::ensure!(
        intents(f, key).await?.as_slice() == encrypted,
        "unfinished root changed exact encrypted references"
    );
    anyhow::ensure!(
        acknowledgements(f, key)
            .await?
            .iter()
            .all(|(_, _, at)| at.is_none()),
        "unfinished child received an ACK"
    );
    exact_prepared(f, saved).await?;
    anyhow::ensure!(
        native_receipts(f, key, saved).await? == *receipts,
        "actual full native receipt identity changed"
    );
    for (owner, pool) in [("auth", &f.auth), ("friend", &f.friend)] {
        let native = saved
            .iter()
            .find(|v| v.owner == owner)
            .ok_or_else(|| anyhow::anyhow!("owner identity missing"))?;
        owner_unfinished(pool, key, &native.gid, after_recovery).await?;
    }
    // Exact production register() capacity predicate, read only. This observes
    // inclusion of this unfinished durable scope; it is not a 17-scope load test.
    let counted:i64=sqlx::query_scalar(
        "SELECT count(*) FROM concert_product_owner_scopes s WHERE instance_id=$1 AND NOT (phase='ROOT_FINISHED' AND (delivery_completed_at IS NOT NULL OR (delivery_secret_hash IS NULL AND serialized_at IS NULL AND NOT EXISTS (SELECT 1 FROM concert_product_owner_transactions t WHERE t.instance_id=s.instance_id AND t.scope_id=s.scope_id) AND NOT EXISTS (SELECT 1 FROM concert_product_child_intents c WHERE c.instance_id=s.instance_id AND c.scope_id=s.scope_id))))")
        .bind(key.instance).fetch_one(&f.concert).await?;
    anyhow::ensure!(
        counted == 1,
        "durable unfinished scope must remain capacity counted"
    );
    Ok(())
}
async fn old_unfinished(run: Uuid, instance: Uuid, variant: Variant) -> anyhow::Result<()> {
    let acquired = acquire(instance).await?;
    let Acquired {
        f,
        scope,
        view,
        capability,
        actor,
        subject,
        key,
    } = acquired;
    // Take the real admission immediately, while the original <=1.8s read is live.
    let permit = if matches!(variant, Variant::Sealed) {
        let permit = f.registry.lookup_root_read(&view)?;
        anyhow::ensure!(
            Arc::ptr_eq(&permit.scope(), &scope),
            "actual root permit mismatch"
        );
        Some(permit)
    } else {
        None
    };
    let encrypted = intents(&f, &key).await?;
    anyhow::ensure!(
        encrypted.len() == 2
            && encrypted[0].kind == "LOCAL_PUBLIC"
            && encrypted[1].kind == "PROFILE_PUBLIC",
        "two real local/Profile intents required"
    );
    enrolled_owner(&f.auth, &key, actor, subject).await?;
    enrolled_owner(&f.friend, &key, actor, subject).await?;
    let saved = snapshot_prepared(&f).await?;
    let receipts = native_receipts(&f, &key, &saved).await?;
    if let Some(permit) = permit {
        // Separate bounded guarded Delivery call. The held actual callback
        // permit prevents finish_root_work; no Auth or child fault is enabled.
        let code = complete(
            DeliveryPublisher::new(f.ledger.clone(), f.registry.clone()),
            capability.clone(),
        )
        .await?;
        anyhow::ensure!(
            code == 14,
            "held callback must leave terminal settlement Unavailable"
        );
        anyhow::ensure!(
            f.ledger.resolve_scope(&key).await? == ScopePhase::Sealed,
            "real callback drain did not leave genuine SEALED"
        );
        drop(permit); // Only after the actual finish failed. No retry in old PID.
    }
    // ACTIVE deliberately never invokes outer terminal delivery or finish.
    quarantine(&f, &key, variant, &saved, &encrypted, &receipts, false).await?;
    writers(&f, subject, true).await?;
    save_unfinished(&UnfinishedHandoff {
        common: Handoff {
            run,
            old_pid: std::process::id(),
            instance,
            subject,
            root_view: view.encode_to_vec(),
            delivery: capability.encode_to_vec(),
            prepared: saved,
            intents: encrypted,
        },
        cr03_variant: variant.label().into(),
        native_receipts: receipts,
    })?;
    line(variant.ready())?;
    std::future::pending::<()>().await;
    drop(scope);
    drop(f);
    Ok(())
}
async fn restart_unfinished(run: Uuid, instance: Uuid, variant: Variant) -> anyhow::Result<()> {
    let saved = load_unfinished(run)?;
    let state = &saved.common;
    anyhow::ensure!(
        state.instance == instance
            && state.old_pid != std::process::id()
            && saved.cr03_variant == variant.label(),
        "CR03 physical identity/variant mismatch"
    );
    let f = fixture::Fixture::new(instance, false).await?; // Empty registry; all fault flags false.
    let view = authority::RootReadView::decode(state.root_view.as_slice())?;
    let capability = proto::ResponseDelivery::decode(state.delivery.as_slice())?;
    let key = scope_key(
        view.scope
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("old scope missing"))?,
    )?;
    quarantine(
        &f,
        &key,
        variant,
        &state.prepared,
        &state.intents,
        &saved.native_receipts,
        false,
    )
    .await?;
    // Wait at least the full original <=1.8s monotonic content ceiling after
    // replacement startup AND past unchanged saved wall expiry+10ms. No clock,
    // view or deadline is rewritten/imported as fresh content authority.
    let monotonic = tokio::time::Instant::now();
    tokio::time::sleep(Duration::from_millis(1810)).await;
    anyhow::ensure!(
        monotonic.elapsed() >= Duration::from_millis(1800),
        "original monotonic ceiling not passed"
    );
    wait_past_saved_expiry(&view).await?;
    let publisher = DeliveryPublisher::new(f.ledger.clone(), f.registry.clone());
    old_content_denied(&f, &view, state.subject).await?;
    anyhow::ensure!(
        complete(publisher.clone(), capability.clone()).await? == 14,
        "lost registry must refuse unfinished delivery"
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
    for _ in 0..3 {
        let counts = worker.tick().await?;
        anyhow::ensure!(
            counts.selected == 0
                && counts.attempted == 0
                && counts.completed == 0
                && counts.pending == 0
                && counts.timed_out == 0,
            "unfinished root selected for child settlement"
        );
        quarantine(
            &f,
            &key,
            variant,
            &state.prepared,
            &state.intents,
            &saved.native_receipts,
            false,
        )
        .await?;
        writers(&f, state.subject, true).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    // Invoke the actual real-owner recovery APIs as a negative proof case.
    // Their normal CR01 helper name/comment does not grant RootFinished here.
    f.recover_owners().await?;
    quarantine(
        &f,
        &key,
        variant,
        &state.prepared,
        &state.intents,
        &saved.native_receipts,
        true,
    )
    .await?;
    writers(&f, state.subject, true).await?;
    old_content_denied(&f, &view, state.subject).await?;
    anyhow::ensure!(
        complete(publisher, capability).await? == 14,
        "owner sweep recreated unfinished Delivery"
    );
    quarantine(
        &f,
        &key,
        variant,
        &state.prepared,
        &state.intents,
        &saved.native_receipts,
        true,
    )
    .await?;
    // Four real native guards intentionally remain. The outer owned cluster
    // teardown is destruction of fixture data, never an actual settlement claim.
    line(variant.observed())?;
    Ok(())
}
#[tokio::test]
#[ignore = "CR03 requires staged frozen source/checkpoint and owned physical PG/mTLS runner; NOT_RUN"]
async fn process_root_child_recovery_cr03() -> anyhow::Result<()> {
    let run: Uuid = std::env::var("MCR_PROCESS_RUN")?.parse()?;
    let instance: Uuid = std::env::var("MCR_PROCESS_INSTANCE")?.parse()?;
    anyhow::ensure!(
        !run.is_nil() && !instance.is_nil(),
        "explicit CR03 run/instance required"
    );
    let variant = Variant::selected()?;
    match std::env::var(TEST_MODE)?.as_str() {
        "old" => old_unfinished(run, instance, variant).await,
        "restart" => {
            tokio::time::timeout(
                Duration::from_secs(90),
                restart_unfinished(run, instance, variant),
            )
            .await?
        }
        _ => anyhow::bail!("unsupported CR03 physical process mode"),
    }
}
