//! Actual Auth/Friend/Profile processes, actual held scopes and native effects.
//! Initial rows are fixtures; the release uses the real source-bound constructor.
use super::{
    auth::GrpcCanonicalAuth,
    coordinator::ParticipantProfileJournal,
    coordinator_tests::owned_pool,
    decision_endpoint_tests::fixture_tls,
    decision_rpc::{ParticipantDecisionService, DECISION_POLICY},
    profile::GrpcCanonicalSelfProfile,
    recovery::ParticipantRecovery,
    service::GuardedParticipantService,
    settlement::ParticipantNativeResolver,
};
use crate::{
    adapters::out::{relationship_authority::guarded::*, repository::avatar::AvatarPersistence},
    bootstrap::{BootstrapFeatures, BootstrapPostgresRepository, ProductReleaseBundle},
    participant::{proto, Operation, POLICY},
    product_rpc::{delivery, proto as product, scope_ledger::*, settlement},
};
use concert_application::bootstrap::{BootstrapAdmission, BootstrapRepository};
use platform::{
    actor_delegation::{
        concert_self_profile::ConcertSelfProfileSigner,
        participant_decision::ParticipantDecisionVerifier,
        relationship_read::RelationshipReadSigner, request_for, response_scope::RetainedScopeLayer,
        ActorDelegationClient, ActorDelegationSigner, ActorDelegationVerifier,
    },
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, ServiceIdentityClientInterceptor, Workload},
    grpc_method_policy::guard,
};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tonic::{service::interceptor::InterceptedService, transport::Channel, Request, Status};
use uuid::Uuid;

fn env(key: &str) -> anyhow::Result<String> {
    Ok(std::env::var(key)?)
}
fn registry() -> anyhow::Result<CallerRegistry> {
    let mut result = CallerRegistry::new();
    for (role, key) in [
        (Workload::Gateway, "GATEWAY"),
        (Workload::Auth, "AUTH"),
        (Workload::Friend, "FRIEND"),
        (Workload::Profile, "PROFILE"),
    ] {
        result = result.register(role, &env(&format!("{key}_SERVICE_CREDENTIAL"))?)?;
    }
    Ok(result)
}
struct NoPublicChildren;
#[async_trait::async_trait]
impl ChildIntentSettler for NoPublicChildren {
    async fn recover_one(&self, _: &ChildRecoveryReference) -> Result<(), Status> {
        Err(Status::unavailable(
            "Public children are outside the SELF join",
        ))
    }
}
type ClientTransport =
    InterceptedService<ActorDelegationClient<Channel>, ServiceIdentityClientInterceptor>;
struct Client {
    rpc:
        proto::concert_participant_service_client::ConcertParticipantServiceClient<ClientTransport>,
    completion: product::product_delivery_service_client::ProductDeliveryServiceClient<
        InterceptedService<Channel, ServiceIdentityClientInterceptor>,
    >,
    actor: Uuid,
    event: Uuid,
    family: Uuid,
    release: String,
}
impl Client {
    async fn call(
        &mut self,
        operation: Operation,
        body: Value,
        id: Uuid,
    ) -> anyhow::Result<proto::ParticipantResponse> {
        let now = chrono::Utc::now().timestamp_micros();
        let (method, suffix) = match operation {
            Operation::GetParticipant => ("GET", ""),
            Operation::SetAvatar => ("PUT", "/avatar"),
            Operation::UpdatePreferences => ("PATCH", ""),
        };
        let message = proto::ParticipantRequest {
            user_id: self.actor.to_string(),
            event_id: self.event.to_string(),
            session_family_id: self.family.to_string(),
            request_id: id.to_string(),
            http_method: method.into(),
            http_target: format!("/api/concert/events/{}/me{suffix}", self.event),
            request_json: if operation == Operation::GetParticipant {
                Vec::new()
            } else {
                serde_json::to_vec(&body)?
            },
            release_digest: self.release.clone(),
            authority_expires_unix_micros: now + 1_800_000,
            auth_observed_at_unix_micros: now,
            profile_observed_at_unix_micros: now,
        };
        let mut request = request_for(message, &self.actor.to_string())?;
        request.set_timeout(Duration::from_secs(2));
        let response = match operation {
            Operation::GetParticipant => self.rpc.get_participant(request).await?,
            Operation::SetAvatar => self.rpc.set_avatar(request).await?,
            Operation::UpdatePreferences => self.rpc.update_preferences(request).await?,
        };
        anyhow::ensure!(
            response
                .metadata()
                .get("cache-control")
                .and_then(|v| v.to_str().ok())
                == Some("no-store")
        );
        let value = response.into_inner();
        anyhow::ensure!(
            value.content_expires_unix_micros > chrono::Utc::now().timestamp_micros()
                && value.content_expires_unix_micros <= now + 1_800_000,
            "actual response-wide disclosure ceiling required"
        );
        let capability = value
            .delivery
            .clone()
            .ok_or_else(|| anyhow::anyhow!("actual delivery capability required"))?;
        // Native Gateway-role client has consumed the whole RPC body. Actual
        // public HTTP terminal-body wiring remains a separate root obligation.
        let stop = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let pending = match self
                .completion
                .complete_response(Request::new(capability.clone()))
                .await
            {
                Ok(result) if result.get_ref().completed => break,
                Ok(_) => "completed=false".to_owned(),
                Err(status) => format!("RPC {:?}", status.code()),
            };
            if tokio::time::Instant::now() >= stop {
                anyhow::bail!("actual owner/native delivery settlement did not complete for {operation:?}: {pending}; participant failure {:?}", value.failure);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Ok(value)
    }
    async fn document(&mut self, operation: Operation, body: Value) -> anyhow::Result<Value> {
        let value = self.call(operation, body, Uuid::new_v4()).await?;
        anyhow::ensure!(
            value.failure.is_none(),
            "actual joined request failed: {:?}",
            value.failure
        );
        Ok(serde_json::from_slice(&value.response_json)?)
    }
}

async fn profile_pool(concert: &PgPool) -> anyhow::Result<PgPool> {
    // Concert's full marker/socket/role/server checks ran first. Compare the
    // second actual store to that same owned cluster, not an arbitrary URL.
    let profile = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&env("MCR_PARTICIPANT_LIVE_PROFILE_URL")?)
        .await?;
    let sql="SELECT current_setting('data_directory'),current_setting('unix_socket_directories'),current_setting('listen_addresses'),current_user";
    let actual: (String, String, String, String) = sqlx::query_as(sql).fetch_one(&profile).await?;
    let owner: (String, String, String, String) = sqlx::query_as(sql).fetch_one(concert).await?;
    anyhow::ensure!(actual == owner && actual.2.is_empty() && actual.3 == "mcr_owner");
    let db: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&profile)
        .await?;
    anyhow::ensure!(db == "mcr_participant_live_profile");
    Ok(profile)
}

#[tokio::test]
#[ignore = "actual external Auth/Friend/Profile binaries and owned PG16.14 join harness only"]
async fn native_actual_owner_profile_participant_save_reload_and_abort() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let profile_db = profile_pool(&pool).await?;
    let root = PathBuf::from(env("MCR_PARTICIPANT_COORDINATOR_ROOT")?);
    let tls = fixture_tls(&root, "CONCERT")?.ok_or_else(|| anyhow::anyhow!("mTLS required"))?;
    let gateway_tls =
        fixture_tls(&root, "GATEWAY")?.ok_or_else(|| anyhow::anyhow!("mTLS required"))?;
    let actor: Uuid = env("MCR_PARTICIPANT_LIVE_ACTOR")?.parse()?;
    let event: Uuid = env("MCR_PARTICIPANT_LIVE_EVENT")?.parse()?;
    let family: Uuid = env("MCR_PARTICIPANT_LIVE_FAMILY")?.parse()?;
    anyhow::ensure!(!actor.is_nil() && !event.is_nil() && !family.is_nil());
    let repo = PathBuf::from(env("MCR_PARTICIPANT_LIVE_REPO_ROOT")?);
    anyhow::ensure!(repo.is_absolute() && repo.canonicalize()? == repo);
    let release_root = repo.join("backend/releases/operator-bundle/quality-v6");
    let release = ProductReleaseBundle::from_source_bound_files(
        &release_root.join("bootstrap.json"),
        &repo.join("contracts/fixtures/assets/selected-static/avatar-manifest.json"),
        &repo.join("backend/releases/reference-scene-v2/quality-v6/map-manifest.json"),
        [
            &release_root.join("theme-danger.json"),
            &release_root.join("theme-parade.json"),
            &release_root.join("theme-revenge.json"),
        ],
        BootstrapFeatures::default(),
        &repo.join("frontend/public"),
        &repo.join("backend/releases/reference-scene-v2/quality-v6/source-binding.json"),
    )
    .map_err(|_| anyhow::anyhow!("actual source-bound release constructor must validate"))?;
    sqlx::query("INSERT INTO concert_events(event_id,slug,title,phase) VALUES($1,$2,'Actual owner SELF join','OPEN')")
        .bind(event).bind(format!("join-{event}")).execute(&pool).await?;
    let admission = BootstrapAdmission::from_verified(
        concert_domain::AuthUserId::from_uuid(actor),
        concert_domain::EventId::from_uuid(event),
        family,
        Uuid::new_v4(),
        "Actual native SELF".into(),
        Some("Native bio".into()),
        chrono::Utc::now(),
    )
    .map_err(|_| anyhow::anyhow!("seed admission"))?;
    BootstrapPostgresRepository::new(pool.clone())
        .bootstrap(&admission, release.release().defaults())
        .await
        .map_err(|_| anyhow::anyhow!("seed participant rows"))?;
    let instance = Uuid::new_v4();
    let ledger = PgScopeLedger::new(
        pool.clone(),
        instance,
        Arc::new(ChildRecoveryKeys::new(
            ChildRecoveryKey::new("native_join", &[97; 32])?,
            vec![],
        )?),
        Arc::new(NoPublicChildren),
    )?;
    let scopes = ScopeRegistry::new(instance, Arc::new(ledger.clone()))?;
    let publisher = delivery::DeliveryPublisher::new(ledger.clone(), scopes.clone());
    let gateway = ActorDelegationVerifier::from_env(true, Workload::Concert)?;
    let relation = RelationshipReadSigner::from_env(&gateway)?;
    let token = env("CONCERT_SERVICE_CREDENTIAL")?;
    let auth_endpoint = env("MCR_PARTICIPANT_LIVE_AUTH_ENDPOINT")?;
    let friend_endpoint = env("MCR_PARTICIPANT_LIVE_FRIEND_ENDPOINT")?;
    let fence = Arc::new(CanonicalOwnerFence::connect(
        &auth_endpoint,
        &friend_endpoint,
        &token,
        &tls,
        relation.clone(),
        scopes,
    )?);
    let auth = GrpcCanonicalAuth::connect(&auth_endpoint, &token, Some(&tls), relation)?;
    let profile = GrpcCanonicalSelfProfile::connect(
        &env("MCR_PARTICIPANT_LIVE_PROFILE_ENDPOINT")?,
        &token,
        Some(&tls),
        ConcertSelfProfileSigner::from_env(&gateway)?,
    )?;
    let journal = Arc::new(
        ParticipantProfileJournal::from_product(pool.clone(), &release)
            .map_err(|_| anyhow::anyhow!("journal release binding"))?,
    );
    let store = Arc::new(
        AvatarPersistence::from_product(pool.clone(), release.clone())
            .map_err(|_| anyhow::anyhow!("actual product persistence constructor"))?,
    );
    let service = guard(
        &POLICY,
        GuardedParticipantService::new(
            store,
            auth,
            profile.clone(),
            fence,
            journal.clone(),
            Arc::new(publisher.clone()),
        )
        .server(),
        registry()?,
        gateway.clone(),
    )?;
    let decision = ParticipantDecisionVerifier::configured_from_env(&gateway)?
        .ok_or_else(|| anyhow::anyhow!("dedicated decision verifier required"))?;
    let decision = guard(
        &DECISION_POLICY,
        ParticipantDecisionService::new(pool.clone()).server(),
        registry()?,
        decision.into_inner(),
    )?;
    let settlement = guard(
        &settlement::POLICY,
        settlement::ProducerSettlementService::new(
            ledger.clone(),
            Arc::new(ParticipantNativeResolver::new(pool.clone(), instance)?),
        )
        .server(),
        registry()?,
        gateway.clone(),
    )?;
    let completion = guard(
        &delivery::POLICY,
        product::product_delivery_service_server::ProductDeliveryServiceServer::new(
            delivery::DeliveryService::new(publisher),
        ),
        registry()?,
        gateway,
    )?;
    let address: std::net::SocketAddr = env("MCR_PARTICIPANT_LIVE_CONCERT_ADDR")?.parse()?;
    anyhow::ensure!(address.ip().is_loopback() && address.port() > 1024);
    let listener = tokio::net::TcpListener::bind(address).await?;
    let incoming = futures::stream::unfold(listener, |listener| async move {
        Some((listener.accept().await.map(|(stream, _)| stream), listener))
    });
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let builder = tonic::transport::Server::builder().tls_config(tls.server_config()?)?;
    let server = tokio::spawn(async move {
        builder
            .layer(RetainedScopeLayer)
            .layer(CallDeadlineLayer)
            .add_service(service)
            .add_service(decision)
            .add_service(settlement)
            .add_service(completion)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = stopped.await;
            })
            .await
    });
    let (shutdown, recovery_stop) = tokio::sync::watch::channel(false);
    let recovery = tokio::spawn(ParticipantRecovery::new(journal, profile).run(recovery_stop));
    let endpoint = format!("https://{address}");
    let channel = gateway_tls
        .endpoint(&endpoint, Workload::Concert)?
        .connect_lazy();
    let interceptor = ServiceIdentityClientInterceptor::new(
        Workload::Gateway,
        &env("GATEWAY_SERVICE_CREDENTIAL")?,
    )?;
    let rpc=proto::concert_participant_service_client::ConcertParticipantServiceClient::with_interceptor(
        ActorDelegationClient::new(channel.clone(),ActorDelegationSigner::from_env(true,Workload::Gateway)?,Workload::Concert),interceptor.clone());
    let completion =
        product::product_delivery_service_client::ProductDeliveryServiceClient::with_interceptor(
            channel,
            interceptor,
        );
    let mut client = Client {
        rpc,
        completion,
        actor,
        event,
        family,
        release: release.source_digest().into(),
    };
    let exercise=async {
        let initial=client.document(Operation::GetParticipant,Value::Null).await?;
        anyhow::ensure!(initial["profile"]["display_name"]=="Actual native SELF" && initial["revision"]==1);
        let config=initial["profile"]["avatar"]["config"].clone();
        let avatar=client.document(Operation::SetAvatar,json!({"expected_revision":1,"config":config})).await?;
        anyhow::ensure!(avatar["revision"]==2);
        let preferences=json!({"theme":"DANGER","low_power":true,"reduced_motion":true,"sound_enabled":false});
        let saved=client.document(Operation::UpdatePreferences,json!({"expected_revision":1,"preferences":preferences})).await?;
        anyhow::ensure!(saved["revision"]==2 && saved["preferences"]==preferences);
        let full=client.document(Operation::UpdatePreferences,json!({"expected_revision":2,"display_name":"Joined saved name","bio":null})).await?;
        anyhow::ensure!(full["revision"]==3 && full["profile"]["display_name"]=="Joined saved name" && full["profile"]["bio"].is_null());
        let reloaded=client.document(Operation::GetParticipant,Value::Null).await?;
        anyhow::ensure!(reloaded==full,"actual owner and native store reload mismatch");
        // Independently prove each real committed producer identity, then
        // refuse rebinding every native/request identity dimension.
        use crate::product_rpc::settlement::NativeTransactionResolver;
        use crate::adapters::out::relationship_authority::guarded::proto as lease;
        let resolver=ParticipantNativeResolver::new(pool.clone(),instance)?;
        let registrations=sqlx::query("SELECT s.binding,t.identity FROM concert_product_owner_transactions t JOIN concert_product_owner_scopes s USING(instance_id,scope_id,binding_digest) WHERE t.instance_id=$1 AND t.identity->>'producer'='CONCERT'")
            .bind(instance).fetch_all(&pool).await?;
        anyhow::ensure!(registrations.len()==3);
        for row in registrations {
            let binding:platform::actor_delegation::owner_binding::OwnerBinding=serde_json::from_value(row.try_get("binding")?)?;
            let identity:crate::product_rpc::scope_ledger::ProducerTransaction=serde_json::from_value(row.try_get("identity")?)?;
            let proved=resolver.resolve(&binding,&identity).await?;
            anyhow::ensure!(proved.status==lease::TransactionStatus::Committed && !proved.receipt_evidence.is_empty());
            for dimension in 0..8 {
                let mut foreign=identity.clone();
                match dimension {
                    0=>foreign.system_identifier="1".into(),
                    1=>foreign.database_oid=1,
                    2=>foreign.full_xid=(foreign.full_xid.parse::<u64>()?+1).to_string(),
                    3=>foreign.backend_pid+=1,
                    4=>foreign.backend_started_unix_micros+=1,
                    5=>foreign.effect_binding_digest[0]^=1,
                    6=>foreign.producer_instance=Uuid::new_v4(),
                    _=>foreign.settlement_reference=Uuid::new_v4().as_bytes().to_vec(),
                }
                anyhow::ensure!(resolver.resolve(&binding,&foreign).await?.status==lease::TransactionStatus::Unknown);
            }
            let mut foreign=binding.clone(); foreign.body_digest[0]^=1;
            anyhow::ensure!(resolver.resolve(&foreign,&identity).await?.status==lease::TransactionStatus::Unknown);
        }
        let stale=client.call(Operation::UpdatePreferences,json!({"expected_revision":1,"display_name":"Must not prepare"}),Uuid::new_v4()).await?;
        anyhow::ensure!(stale.response_json.is_empty() && stale.failure.as_ref().is_some_and(|e|e.grpc_code==tonic::Code::Aborted as u32 && e.core_reason=="VERSION_CONFLICT"));
        // A real effect-transaction barrier AFTER canonical Profile PREPARE.
        // No synthetic decision, owner view, prepared identity or outcome.
        sqlx::raw_sql("CREATE FUNCTION public.participant_join_pause() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_lock(1296257616); PERFORM pg_advisory_unlock(1296257616); RETURN NEW; END $$; CREATE TRIGGER participant_join_pause BEFORE UPDATE ON public.concert_participants FOR EACH ROW EXECUTE FUNCTION public.participant_join_pause();").execute(&pool).await?;
        let mut barrier=pool.acquire().await?;
        sqlx::query("SELECT pg_advisory_lock(1296257616)").execute(&mut *barrier).await?;
        let operation=Uuid::new_v4();
        let mut pending=Box::pin(client.call(Operation::UpdatePreferences,json!({"expected_revision":3,"display_name":"Must roll back"}),operation));
        let gid=format!("mcr_sp_{}",operation.simple());
        let stop=tokio::time::Instant::now()+Duration::from_millis(1700);
        loop {
            let prepared:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_prepared_xacts WHERE database=current_database() AND gid=$1)").bind(&gid).fetch_one(&profile_db).await?;
            if prepared {break}
            anyhow::ensure!(tokio::time::Instant::now()<stop,"actual Profile did not PREPARE before bounded barrier");
            tokio::select! {
                result=&mut pending => anyhow::bail!("request ended before actual PREPARE: {}",result.is_ok()),
                _=tokio::time::sleep(Duration::from_millis(10))=>{}
            }
        }
        let invisible:String=sqlx::query_scalar("SELECT nickname FROM profiles WHERE user_id=$1").bind(actor).fetch_one(&profile_db).await?;
        anyhow::ensure!(invisible=="Joined saved name","prepared Profile content became visible");
        anyhow::ensure!(pending.await.is_err(),"blocked request must end without a fabricated delivery envelope");
        sqlx::query("SELECT pg_advisory_unlock(1296257616)").execute(&mut *barrier).await?;
        drop(barrier);
        let stop=tokio::time::Instant::now()+Duration::from_secs(8);
        loop {
            let resolved:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM public.concert_participant_profile_terminal_receipts r JOIN public.concert_participant_profile_operations o USING(operation_id) WHERE o.operation_id=$1 AND o.decision='PENDING' AND r.outcome='ABORTED' AND pg_xact_status(o.concert_full_xid::xid8)='aborted')")
                .bind(operation).fetch_one(&pool).await?;
            if resolved {break}
            anyhow::ensure!(tokio::time::Instant::now()<stop,"actual expired original operation did not reconcile ABORT");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let visible:String=sqlx::query_scalar("SELECT nickname FROM profiles WHERE user_id=$1").bind(actor).fetch_one(&profile_db).await?;
        let revision:i64=sqlx::query_scalar("SELECT revision FROM concert_participants WHERE event_id=$1 AND user_id=$2").bind(event).bind(actor).fetch_one(&pool).await?;
        anyhow::ensure!(visible=="Joined saved name" && revision==3);
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM concert_participant_effect_witnesses").fetch_one(&pool).await?;
        anyhow::ensure!(count==2,"exact successful avatar and preference witnesses required");
        anyhow::Ok(())
    }.await;
    drop(client);
    let _ = shutdown.send(true);
    let _ = stop.send(());
    tokio::time::timeout(Duration::from_secs(3), recovery).await??;
    tokio::time::timeout(Duration::from_secs(3), server).await???;
    profile_db.close().await;
    pool.close().await;
    exercise
}
