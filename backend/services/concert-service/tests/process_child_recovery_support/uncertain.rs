// Authoring core: e34b90dd8616bc2c51a66d1acaf3521d4dd142c7; source-only draft, compile/runtime NOT_RUN.
//! CR02 lost actual Profile reply and CR05 actual SDK future cancellation.
//! Source-only fixture; compilation/native acceptance NOT_RUN. use parent helpers.
use super::profile_read_fault::Mode;
use super::*;
use concert_service::adapters::out::relationship_authority::guarded::ChildRecoveryReference;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Case {
    LostReply,
    CancelledBeforeDispatch,
}
impl Case {
    fn name(self) -> &'static str {
        match self {
            Self::LostReply => "CR02",
            Self::CancelledBeforeDispatch => "CR05",
        }
    }
    fn mode(self) -> Mode {
        match self {
            Self::LostReply => Mode::LostReply,
            Self::CancelledBeforeDispatch => Mode::BeforeDispatch,
        }
    }
    fn profile_prepared(self) -> usize {
        if self == Self::LostReply {
            1
        } else {
            0
        }
    }
    fn terminal(self) -> &'static str {
        if self == Self::LostReply {
            "SETTLED"
        } else {
            "CANCELLED"
        }
    }
    fn ready(self) -> &'static str {
        if self == Self::LostReply {
            "MCR_CR02_OLD_READY"
        } else {
            "MCR_CR05_OLD_READY"
        }
    }
    fn observed(self) -> &'static str {
        if self == Self::LostReply {
            "MCR_CR02_RESTART_OBSERVED"
        } else {
            "MCR_CR05_RESTART_OBSERVED"
        }
    }
}
// No Debug: the exact child_id/reference contain private completion material.
#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct NativeOwner {
    system: String,
    database: i64,
    owner: i64,
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct ProfileReceipt {
    child_id: String,
    digest: String,
    native: NativeOwner,
    full_xid: Option<String>,
    state: String,
    created_at: String,
    settled_at: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct Evidence {
    run: Uuid,
    instance: Uuid,
    case: String,
    profile: ChildRecoveryReference,
    local: ChildRecoveryReference,
    native: NativeOwner,
    original: Option<ProfileReceipt>,
}
fn evidence_path(run: Uuid) -> anyhow::Result<PathBuf> {
    Ok(environment::root()?.join(format!("uncertain-{}.private.json", run.simple())))
}
fn save_evidence(evidence: &Evidence) -> anyhow::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(evidence_path(evidence.run)?)?;
    file.write_all(&serde_json::to_vec(evidence)?)?;
    file.sync_all()?;
    Ok(())
}
fn load_evidence(run: Uuid) -> anyhow::Result<Evidence> {
    let path = evidence_path(run)?;
    let meta = std::fs::symlink_metadata(&path)?;
    let root = std::fs::metadata(environment::root()?)?;
    anyhow::ensure!(
        meta.is_file()
            && !meta.file_type().is_symlink()
            && meta.uid() == root.uid()
            && meta.permissions().mode() & 0o777 == 0o600,
        "private owned uncertainty handoff required"
    );
    let evidence: Evidence = serde_json::from_slice(&std::fs::read(path)?)?;
    anyhow::ensure!(evidence.run == run, "private handoff run mismatch");
    Ok(evidence)
}
async fn native_owner(pool: &PgPool) -> anyhow::Result<NativeOwner> {
    let (system, database, owner): (String, i64, i64) = sqlx::query_as(
        "select (pg_control_system()).system_identifier::text,(select oid::bigint from pg_database where datname=current_database()),(select oid::bigint from pg_roles where rolname=current_user)")
        .fetch_one(pool).await?;
    Ok(NativeOwner {
        system,
        database,
        owner,
    })
}
async fn receipt(pool: &PgPool, child: &str) -> anyhow::Result<Option<ProfileReceipt>> {
    type Row = (
        String,
        String,
        String,
        i64,
        i64,
        Option<String>,
        String,
        String,
        Option<String>,
    );
    let row: Option<Row> = sqlx::query_as("select child_id,completion_digest,system_identifier,database_oid,owner_oid,full_xid,state,created_at::text,settled_at::text from public.concert_public_profile_children where child_id=$1")
        .bind(child).fetch_optional(pool).await?;
    Ok(row.map(
        |(child_id, digest, system, database, owner, full_xid, state, created_at, settled_at)| {
            ProfileReceipt {
                child_id,
                digest,
                native: NativeOwner {
                    system,
                    database,
                    owner,
                },
                full_xid,
                state,
                created_at,
                settled_at,
            }
        },
    ))
}
fn profile_parts(e: &Evidence) -> anyhow::Result<(&str, &str)> {
    match &e.profile {
        ChildRecoveryReference::ProfilePublic {
            child_id,
            completion_secret,
            ..
        } => Ok((child_id, completion_secret)),
        _ => anyhow::bail!("exact Profile reference required"),
    }
}
async fn expected_digest(pool: &PgPool, e: &Evidence) -> anyhow::Result<String> {
    let (child, secret) = profile_parts(e)?;
    // Identical source-bound completion_bytes to Profile persistence.rs. Read-only.
    Ok(sqlx::query_scalar(
        "select translate(rtrim(encode(sha256($1::bytea),'base64'),'='),'+/','-_')",
    )
    .bind(format!("mcr public Profile completion v1|{child}|{secret}").into_bytes())
    .fetch_one(pool)
    .await?)
}
fn bind_evidence(
    e: &Evidence,
    state: &Handoff,
    view: &authority::RootReadView,
    case: Case,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        e.instance == state.instance && e.run == state.run && e.case == case.name(),
        "private uncertainty case identity mismatch"
    );
    let key = scope_key(
        view.scope
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("scope missing"))?,
    )?;
    let binding = view
        .binding
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("binding missing"))?;
    for reference in [&e.local, &e.profile] {
        let q = reference.query();
        anyhow::ensure!(
            q.actor.to_string() == binding.actor_id
                && q.event.to_string() == binding.event_id
                && q.family.to_string() == binding.session_family_id
                && q.request.to_string() == binding.request_id
                && q.subject == state.subject
                && q.expires_unix_micros == binding.expires_unix_micros
                && q.binding_digest == key.binding_digest,
            "full original child/root binding mismatch"
        );
        anyhow::ensure!(
            state
                .intents
                .iter()
                .filter(
                    |i| i.kind == reference.kind_name() && Some(i.id) == reference.intent_id().ok()
                )
                .count()
                == 1,
            "exact durable intent nonce missing"
        );
    }
    let (child, secret) = profile_parts(e)?;
    let q = e.profile.query();
    let binding = profile_service::concert_public_profile::ReadBinding {
        actor: q.actor,
        event: q.event,
        subject: q.subject,
        family: q.family,
        request: q.request,
    };
    binding
        .validate_completion(child, secret)
        .map_err(|_| anyhow::anyhow!("actual Profile binding validation failed"))?;
    anyhow::ensure!(
        matches!(&e.local, ChildRecoveryReference::LocalPublic { .. }),
        "Local reference missing"
    );
    Ok(())
}
async fn check_initial(
    f: &fixture::Fixture,
    evidence: &Evidence,
    case: Case,
) -> anyhow::Result<()> {
    let (child, _) = profile_parts(evidence)?;
    let actual = receipt(&f.profile, child).await?;
    let total: i64 =
        sqlx::query_scalar("select count(*) from public.concert_public_profile_children")
            .fetch_one(&f.profile)
            .await?;
    anyhow::ensure!(
        native_owner(&f.profile).await? == evidence.native,
        "Profile native owner changed"
    );
    anyhow::ensure!(
        total == case.profile_prepared() as i64 && actual == evidence.original,
        "original Profile receipt changed or unexpected receipt created"
    );
    if let ChildRecoveryReference::LocalPublic { native, .. } = &evidence.local {
        let owner = native_owner(&f.concert).await?;
        let low = native.full_xid.parse::<u64>()? % 4_294_967_296;
        let held = prepared(&f.concert).await?;
        anyhow::ensure!(
            held.len() == 1
                && held[0].1 == low.to_string()
                && owner.system == native.system_identifier
                && owner.database == i64::from(native.database_oid)
                && native.producer_instance_id == evidence.instance,
            "actual Local full native identity mismatch"
        );
    } else {
        anyhow::bail!("actual Local typed reference missing");
    }
    if case == Case::LostReply {
        let r = actual
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("actual REGISTERED receipt missing"))?;
        anyhow::ensure!(
            r.state == "REGISTERED"
                && r.settled_at.is_none()
                && r.native == evidence.native
                && r.digest == expected_digest(&f.profile, evidence).await?,
            "registered Profile receipt mismatch"
        );
        let xid = r
            .full_xid
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("actual full XID missing"))?;
        let low = xid.parse::<u64>()? % 4_294_967_296;
        let gid = format!("{}:{}", r.child_id, r.digest);
        anyhow::ensure!(
            prepared(&f.profile).await? == vec![(gid, low.to_string())],
            "exact registered native Profile transaction missing"
        );
    } else {
        anyhow::ensure!(
            actual.is_none() && prepared(&f.profile).await?.is_empty(),
            "cancelled read dispatched to actual Profile producer"
        );
    }
    Ok(())
}
async fn check_terminal(
    f: &fixture::Fixture,
    e: &Evidence,
    case: Case,
) -> anyhow::Result<ProfileReceipt> {
    let (child, _) = profile_parts(e)?;
    let r = receipt(&f.profile, child)
        .await?
        .ok_or_else(|| anyhow::anyhow!("real Profile terminal receipt missing"))?;
    let total: i64 =
        sqlx::query_scalar("select count(*) from public.concert_public_profile_children")
            .fetch_one(&f.profile)
            .await?;
    anyhow::ensure!(
        total == 1
            && r.child_id == child
            && r.digest == expected_digest(&f.profile, e).await?
            && r.native == e.native
            && r.native == native_owner(&f.profile).await?
            && r.state == case.terminal()
            && r.settled_at.is_some(),
        "exact authenticated Profile terminal receipt mismatch"
    );
    if case == Case::LostReply {
        let old = e
            .original
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("original registered receipt missing"))?;
        anyhow::ensure!(
            r.full_xid == old.full_xid && r.created_at == old.created_at && r.full_xid.is_some(),
            "Profile terminal native identity changed"
        );
        let status: Option<String> = sqlx::query_scalar("select pg_xact_status($1::text::xid8)")
            .bind(r.full_xid.as_deref())
            .fetch_one(&f.profile)
            .await?;
        anyhow::ensure!(
            status.as_deref() == Some("aborted"),
            "actual Profile abort proof missing"
        );
    } else {
        anyhow::ensure!(
            e.original.is_none() && r.full_xid.is_none(),
            "CANCELLED receipt must attest pre-dispatch absence, without dummy native XID"
        );
    }
    if let ChildRecoveryReference::LocalPublic { native, .. } = &e.local {
        let owner = native_owner(&f.concert).await?;
        anyhow::ensure!(
            owner.system == native.system_identifier
                && owner.database == i64::from(native.database_oid)
                && native.producer_instance_id == e.instance,
            "exact Local native identity changed"
        );
        let status: Option<String> = sqlx::query_scalar("select pg_xact_status($1::text::xid8)")
            .bind(&native.full_xid)
            .fetch_one(&f.concert)
            .await?;
        anyhow::ensure!(
            status.as_deref() == Some("aborted"),
            "actual Local abort proof missing"
        );
    }
    Ok(r)
}

async fn old_uncertain(run: Uuid, instance: Uuid, case: Case) -> anyhow::Result<()> {
    let f = fixture::Fixture::with_read_fault(instance, case.mode()).await?;
    for pool in [&f.auth, &f.friend, &f.concert, &f.profile] {
        anyhow::ensure!(
            prepared(pool).await?.is_empty(),
            "clean isolated databases required"
        );
    }
    let (actor, family, subject) = f.seed().await?;
    let producer = Arc::new(uncertain_producer::UncertainProducer {
        reader: f.reader.clone(),
        fault: f.read_fault.clone(),
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
        output.status == 503 && output.content_type == "application/json",
        "actual closed produced refusal required"
    );
    let document: serde_json::Value = serde_json::from_slice(&output.body)?;
    anyhow::ensure!(
        document["details"]["reason"] == "DEPENDENCY_UNAVAILABLE",
        "actual negative Product contract required"
    );
    anyhow::ensure!(
        !String::from_utf8_lossy(&output.body).contains("uncertain actual Profile acquisition")
            && !String::from_utf8_lossy(&output.body).contains("private-url-canary")
            && !String::from_utf8_lossy(&output.body).contains("actual SDK read future cancelled")
            && !String::from_utf8_lossy(&output.body).contains("completion_secret")
            && document.get("user_id").is_none(),
        "produced error disclosed private producer content"
    );
    anyhow::ensure!(
        f.read_fault.observed.load(Ordering::SeqCst) == 1,
        "exact actual read fault observation required"
    );
    anyhow::ensure!(
        f.read_fault.handler_calls.load(Ordering::SeqCst) == case.profile_prepared(),
        "wrong actual Profile handler dispatch count"
    );
    let dropped = if case == Case::CancelledBeforeDispatch {
        1
    } else {
        0
    };
    anyhow::ensure!(
        f.read_fault.read_future_dropped.load(Ordering::SeqCst) == dropped,
        "actual SDK read future cancellation missing"
    );
    let receipt_count: i64 =
        sqlx::query_scalar("select count(*) from concert_public_profile_children")
            .fetch_one(&f.profile)
            .await?;
    anyhow::ensure!(
        receipt_count == case.profile_prepared() as i64
            && prepared(&f.profile).await?.len() == case.profile_prepared(),
        "wrong actual Profile receipt/PREPARE before terminal Root handling"
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
    invariant(&f, &key).await?;
    // Terminal Root proof is real, but child completion remains unavailable.
    // CR02 actually ran ReadCurrent; CR05 cancelled before producer dispatch. Now only its exact Complete method returns a
    // negative transport fault; Local settlement is negatively unavailable.
    // All replacement-process settlement paths use the actual adapters.
    let before_fault = intents(&f, &key).await?;
    f.make_child_settlement_unavailable();
    let mut unavailable_worker = ChildRecoveryWorker::new(
        f.ledger.clone(),
        ChildRecoverySettings::new(
            2,
            Duration::from_secs(2),
            Duration::from_secs(6),
            Duration::from_millis(100),
        )?,
    )?;
    let unavailable = unavailable_worker.tick().await?;
    anyhow::ensure!(
        unavailable.selected == 2
            && unavailable.attempted == 2
            && unavailable.completed == 0
            && unavailable.pending == 2,
        "both actual children must survive unavailable settlement"
    );
    anyhow::ensure!(
        f.profile_completion_failures.load(Ordering::SeqCst) == 1,
        "actual Profile Complete transport fault was not exercised"
    );
    anyhow::ensure!(
        intents(&f, &key).await? == before_fault,
        "unavailable child settlement changed immutable encrypted references"
    );
    invariant(&f, &key).await?;
    anyhow::ensure!(
        pending(&f, &key).await? == (2, 0),
        "both real children must remain pending"
    );
    let kinds:Vec<String>=sqlx::query_scalar("select kind from concert_product_child_intents where instance_id=$1 and scope_id=$2 order by kind")
        .bind(instance).bind(key.scope).fetch_all(&f.concert).await?;
    anyhow::ensure!(
        kinds == vec!["LOCAL_PUBLIC".to_string(), "PROFILE_PUBLIC".to_string()],
        "wrong real child kinds"
    );
    // References are observed only after production decrypt/validation in the old
    // actual worker; no test decryptor or caller-invented nonce is substituted.
    let references = f.observed_child_references()?;
    anyhow::ensure!(
        references.len() == 2,
        "two actual typed references required"
    );
    for reference in &references {
        reference.validate_for(scope.binding())?;
    }
    let profile = references
        .iter()
        .find(|r| matches!(r, ChildRecoveryReference::ProfilePublic { .. }))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("actual Profile reference missing"))?;
    let local = references
        .iter()
        .find(|r| matches!(r, ChildRecoveryReference::LocalPublic { .. }))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("actual Local reference missing"))?;
    let mut evidence = Evidence {
        run,
        instance,
        case: case.name().into(),
        profile,
        local,
        native: native_owner(&f.profile).await?,
        original: None,
    };
    let (child, _) = profile_parts(&evidence)?;
    evidence.original = receipt(&f.profile, child).await?;
    check_initial(&f, &evidence, case).await?;
    enrolled_owner(&f.auth, &key, actor, subject).await?;
    enrolled_owner(&f.friend, &key, actor, subject).await?;
    local_writer(&f, subject, true).await?;
    profile_writer(&f, subject, case.profile_prepared() == 1).await?;
    let mut saved = Vec::new();
    for (name, pool) in [
        ("auth", &f.auth),
        ("friend", &f.friend),
        ("concert", &f.concert),
        ("profile", &f.profile),
    ] {
        let rows = prepared(pool).await?;
        let expected = if name == "profile" {
            case.profile_prepared()
        } else {
            1
        };
        anyhow::ensure!(
            rows.len() == expected,
            "exact real prepared owner/child count required"
        );
        for (gid, xid) in rows {
            saved.push(Prepared {
                owner: name.into(),
                gid,
                xid,
            });
        }
    }
    let handoff = Handoff {
        run,
        old_pid: std::process::id(),
        instance,
        subject,
        root_view: view.encode_to_vec(),
        delivery: capability.encode_to_vec(),
        prepared: saved,
        intents: before_fault,
    };
    bind_evidence(&evidence, &handoff, &view, case)?;
    save_evidence(&evidence)?;
    save(&handoff)?;
    line(case.ready())?;
    // Keep f, its actual Arc scopes, reader and servers alive until controller SIGKILL.
    std::future::pending::<()>().await;
    drop(scope);
    drop(f);
    Ok(())
}

async fn restart_uncertain(run: Uuid, instance: Uuid, case: Case) -> anyhow::Result<()> {
    let state = load(run)?;
    let evidence = load_evidence(run)?;
    let view = authority::RootReadView::decode(state.root_view.as_slice())?;
    bind_evidence(&evidence, &state, &view, case)?;
    let f = fixture::Fixture::new(instance, false).await?;
    anyhow::ensure!(
        f.read_fault.mode == Mode::None,
        "replacement must restore actual Profile producer"
    );
    check_initial(&f, &evidence, case).await?;
    // Full existing CR01 verifier, with only explicit physical outcome parameters.
    // No dummy PREPARE or weakened one-prepared/SETTLED default for CR01/CR04.
    restart_expected(
        run,
        instance,
        &f,
        RestartExpectation {
            profile_prepared: case.profile_prepared(),
            profile_terminal: case.terminal(),
        },
    )
    .await?;
    anyhow::ensure!(
        f.read_fault.complete_calls.load(Ordering::SeqCst) > 0,
        "actual configured Profile Complete RPC was not exercised"
    );
    let terminal = check_terminal(&f, &evidence, case).await?;
    let key = scope_key(
        view.scope
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("scope missing"))?,
    )?;
    let acknowledged = acknowledgements(&f, &key).await?;
    // Fresh production worker also checks cursor-independent repeated completion.
    let mut worker = ChildRecoveryWorker::new(
        f.ledger.clone(),
        ChildRecoverySettings::new(
            2,
            Duration::from_secs(2),
            Duration::from_secs(6),
            Duration::from_millis(100),
        )?,
    )?;
    for _ in 0..2 {
        let c = worker.tick().await?;
        anyhow::ensure!(
            c.selected == 0 && c.attempted == 0 && c.completed == 0 && c.pending == 0,
            "completed uncertainty intent was retried"
        );
        anyhow::ensure!(
            check_terminal(&f, &evidence, case).await? == terminal,
            "repeated production drain rewrote actual Profile terminal receipt"
        );
    }
    anyhow::ensure!(
        intents(&f, &key).await? == state.intents
            && acknowledgements(&f, &key).await? == acknowledged
            && pending(&f, &key).await? == (0, 2),
        "uncertainty retry changed immutable references or durable ACK timestamps"
    );
    for pool in [&f.auth, &f.friend, &f.concert, &f.profile] {
        anyhow::ensure!(
            prepared(pool).await?.is_empty(),
            "normal actual settlement must leave no native guards"
        );
    }
    anyhow::ensure!(
        f.read_fault.handler_calls.load(Ordering::SeqCst) == 0,
        "replacement unexpectedly acquired new Profile read authority"
    );
    invariant(&f, &key).await?;
    old_content_denied(&f, &view, state.subject).await?;
    let capability = proto::ResponseDelivery::decode(state.delivery.as_slice())?;
    anyhow::ensure!(
        complete(
            DeliveryPublisher::new(f.ledger.clone(), f.registry.clone()),
            capability
        )
        .await?
            == 14,
        "exact child/owner settlement cannot manufacture lost-registry Delivery"
    );
    line(case.observed())?;
    Ok(())
}

pub(super) async fn run(case: Case) -> anyhow::Result<()> {
    let run: Uuid = std::env::var("MCR_PROCESS_RUN")?.parse()?;
    let instance: Uuid = std::env::var("MCR_PROCESS_INSTANCE")?.parse()?;
    anyhow::ensure!(
        !run.is_nil() && !instance.is_nil() && std::env::var("MCR_PROCESS_CASE")? == case.name(),
        "explicit matching process case/identity required"
    );
    match std::env::var(TEST_MODE)?.as_str() {
        "old" => old_uncertain(run, instance, case).await,
        "restart" => {
            tokio::time::timeout(
                Duration::from_secs(90),
                restart_uncertain(run, instance, case),
            )
            .await?
        }
        _ => anyhow::bail!("unsupported actual uncertainty process mode"),
    }
}
