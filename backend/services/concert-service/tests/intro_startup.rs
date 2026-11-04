//! Actual production executable over owned CF02 Auth/Friend/Profile stores.
//! Requires an explicitly allocated private cluster and copied, hash-pinned main.
#[path = "shared_participant_composition_support/mod.rs"]
mod support;
use concert_service::{participant::proto as pp, product_rpc::proto};
use platform::{
    actor_delegation::{ActorDelegationSigner, HEADER},
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use prost::Message;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};
use support::{environment, fixture};
use tonic::{transport::Channel, Request};
use uuid::Uuid;

// Fixture-only encoding; production key decoding remains the platform parser.
fn b64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for part in bytes.chunks(3) {
        let value = ((part[0] as u32) << 16)
            | ((part.get(1).copied().unwrap_or(0) as u32) << 8)
            | part.get(2).copied().unwrap_or(0) as u32;
        for shift in [18, 12] {
            out.push(ALPHABET[((value >> shift) & 63) as usize] as char);
        }
        out.push(if part.len() > 1 {
            ALPHABET[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if part.len() > 2 {
            ALPHABET[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
fn settings(f: &fixture::Fixture, instance: Uuid) -> anyhow::Result<BTreeMap<String, String>> {
    let root = environment::root()?;
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap();
    let bundle = source.join("backend/releases/operator-bundle/quality-v6");
    let mut values: BTreeMap<String, String> = [
        (
            "CONCERT_DATABASE_URL",
            std::env::var("MCR_CANONICAL_CONCERT_URL")?,
        ),
        ("CONCERT_RUNTIME_MODE", "local".into()),
        ("CONCERT_MIGRATION_MODE", "external".into()),
        ("CONCERT_METRICS_ADDR", "127.0.0.1:0".into()),
        (
            "CONCERT_GRPC_ADDR",
            environment::address("concert")?.to_string(),
        ),
        ("CONCERT_PARTICIPANT_MODE", "live".into()),
        (
            "CONCERT_PARTICIPANT_AUTH_GRPC_ENDPOINT",
            environment::url("auth")?,
        ),
        (
            "CONCERT_PARTICIPANT_FRIEND_GRPC_ENDPOINT",
            environment::url("friend")?,
        ),
        (
            "CONCERT_PARTICIPANT_PROFILE_GRPC_ENDPOINT",
            environment::url("profile")?,
        ),
        ("CONCERT_PRODUCER_INSTANCE_ID", instance.to_string()),
        (
            "CONCERT_PARTICIPANT_RELEASE_DIGEST",
            f.product.source_digest().into(),
        ),
        (
            "CONCERT_PARTICIPANT_RECOVERY_KEYS_JSON",
            json!([{"id":"startup-recovery","secret_hex":"5d".repeat(32)}]).to_string(),
        ),
        ("GATEWAY_SERVICE_CREDENTIAL", "gateway-fixture".into()),
        ("AUTH_SERVICE_CREDENTIAL", "auth-fixture".into()),
        ("FRIEND_SERVICE_CREDENTIAL", "friend-fixture".into()),
        ("PROFILE_SERVICE_CREDENTIAL", "profile-fixture".into()),
        ("CONCERT_SERVICE_CREDENTIAL", "concert-fixture".into()),
        (
            "ACTOR_DELEGATION_VERIFYING_KEYS",
            b64(&ActorDelegationSigner::from_seed(Workload::Gateway, &[74; 32])?.public_key()),
        ),
        ("CONCERT_RELATIONSHIP_SIGNING_SEED", b64(&[75; 32])),
        ("CONCERT_PROFILE_SELF_SIGNING_SEED", b64(&[76; 32])),
        (
            "PROFILE_PARTICIPANT_DECISION_VERIFYING_KEYS",
            b64(&fixture::decision_key()?),
        ),
        ("CONCERT_INTRO_MODE", "live".into()),
        (
            "CONCERT_NOTIFICATION_GRPC_ENDPOINT",
            environment::url("notification")?,
        ),
        ("CONCERT_INTRO_CURSOR_KEY_HEX", "61".repeat(32)),
        (
            "CONCERT_INTRO_RECEIPT_KEYS_JSON",
            json!([{"id":"startup-receipt","secret_hex":"62".repeat(32)}]).to_string(),
        ),
        (
            "CONCERT_PICKUP_KEYS_JSON",
            json!([{"id":"startup-pickup","secret_hex":"63".repeat(32)}]).to_string(),
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v))
    .collect();
    for (key, path) in [
        ("CONCERT_BOOTSTRAP_FILE", bundle.join("bootstrap.json")),
        (
            "CONCERT_AVATAR_MANIFEST_FILE",
            source.join("contracts/fixtures/assets/selected-static/avatar-manifest.json"),
        ),
        (
            "CONCERT_MAP_MANIFEST_FILE",
            source.join("backend/releases/reference-scene-v2/quality-v6/map-manifest.json"),
        ),
        (
            "CONCERT_REVENGE_MANIFEST_FILE",
            bundle.join("theme-revenge.json"),
        ),
        (
            "CONCERT_PARADE_MANIFEST_FILE",
            bundle.join("theme-parade.json"),
        ),
        (
            "CONCERT_DANGER_MANIFEST_FILE",
            bundle.join("theme-danger.json"),
        ),
        ("CONCERT_GRPC_TLS_CERT_PATH", root.join("pki/concert.crt")),
        ("CONCERT_GRPC_TLS_KEY_PATH", root.join("pki/concert.key")),
        ("CONCERT_GRPC_TLS_CA_PATH", root.join("pki/ca.crt")),
    ] {
        values.insert(
            key.into(),
            path.to_str()
                .ok_or_else(|| anyhow::anyhow!("fixture path"))?
                .into(),
        );
    }
    Ok(values)
}
struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        // Only this owned child handle, including assertion-failure cleanup.
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
impl Running {
    fn start(values: &BTreeMap<String, String>, log: &str) -> anyhow::Result<Self> {
        let root = environment::root()?;
        let path = std::env::var("MCR_INTRO_CONCERT_BIN")?;
        let path = Path::new(&path);
        let metadata = std::fs::symlink_metadata(path)?;
        anyhow::ensure!(
            path.is_absolute() && metadata.is_file() && !metadata.file_type().is_symlink(),
            "copied actual main required"
        );
        let digest = format!("{:x}", Sha256::digest(std::fs::read(path)?));
        anyhow::ensure!(
            digest == std::env::var("MCR_INTRO_CONCERT_SHA256")?,
            "main hash changed"
        );
        let output = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(root.join(log))?;
        Ok(Self(
            Command::new(path)
                .env_clear()
                .envs(values)
                .stdin(Stdio::null())
                .stdout(output.try_clone()?)
                .stderr(output)
                .spawn()?,
        ))
    }
    async fn wait(&mut self) -> anyhow::Result<std::process::ExitStatus> {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(status) = self.0.try_wait()? {
                    return Ok(status);
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await?
    }
    async fn stop(mut self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.0.try_wait()?.is_none(),
            "main stopped before requested shutdown"
        );
        anyhow::ensure!(
            Command::new("/bin/kill")
                .args(["-TERM", &self.0.id().to_string()])
                .status()?
                .success(),
            "owned child shutdown signal"
        );
        anyhow::ensure!(
            self.wait().await?.success(),
            "actual main graceful shutdown failed"
        );
        Ok(())
    }
}
async fn channel(child: &mut Running) -> anyhow::Result<Channel> {
    let material = environment::tls(Workload::Gateway)?;
    let endpoint = environment::url("concert")?;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            anyhow::ensure!(
                child.0.try_wait()?.is_none(),
                "actual main exited during startup"
            );
            if let Ok(channel) = material.connect(&endpoint, Workload::Concert).await {
                return Ok(channel);
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await?
}
fn signed<T: Message>(message: T, actor: Uuid, method: &str) -> anyhow::Result<Request<T>> {
    let bytes = message.encode_to_vec();
    let mut frame = vec![0];
    frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    frame.extend(bytes);
    let signer = ActorDelegationSigner::from_seed(Workload::Gateway, &[74; 32])?;
    let proof = signer.issue(
        Workload::Concert,
        method,
        actor,
        None,
        &frame,
        chrono::Utc::now().timestamp(),
    );
    let mut request = Request::new(message);
    request.metadata_mut().insert(HEADER, proof.parse()?);
    request.metadata_mut().insert(
        platform::call_deadline::REMAINING_HEADER,
        "1950000u".parse()?,
    );
    Ok(request)
}
async fn complete(
    channel: Channel,
    f: &fixture::Fixture,
    cap: proto::ResponseDelivery,
) -> anyhow::Result<()> {
    let scope: Uuid = cap.scope_id.parse()?;
    let instance: Uuid = cap.instance_id.parse()?;
    let retained: bool = sqlx::query_scalar("select phase='ACTIVE' and serialized_at is not null and delivery_completed_at is null from concert_product_owner_scopes where instance_id=$1 and scope_id=$2").bind(instance).bind(scope).fetch_one(&f.concert).await?;
    anyhow::ensure!(retained, "inner gRPC EOF released the actual root early");
    let mut client =
        proto::product_delivery_service_client::ProductDeliveryServiceClient::with_interceptor(
            channel,
            ServiceIdentityClientInterceptor::new(Workload::Gateway, "gateway-fixture")?,
        );
    anyhow::ensure!(
        client.complete_response(cap).await?.into_inner().completed,
        "actual mounted delivery refused"
    );
    let phase: String = sqlx::query_scalar("select phase from concert_product_owner_scopes where instance_id=$1 and scope_id=$2 and delivery_completed_at is not null").bind(instance).bind(scope).fetch_one(&f.concert).await?;
    anyhow::ensure!(
        phase == "ROOT_FINISHED",
        "actual root not finished after response terminal"
    );
    let pending: i64 = sqlx::query_scalar("select count(*) from concert_product_child_intents where instance_id=$1 and scope_id=$2 and completed_at is null").bind(instance).bind(scope).fetch_one(&f.concert).await?;
    anyhow::ensure!(pending == 0, "response left child intents pending");
    Ok(())
}
async fn participant(channel: Channel, f: &fixture::Fixture) -> anyhow::Result<Uuid> {
    let now = chrono::Utc::now().timestamp_micros();
    let raw = pp::ParticipantRequest {
        user_id: f.actor.to_string(),
        event_id: f.event.to_string(),
        session_family_id: f.family.to_string(),
        request_id: Uuid::new_v4().to_string(),
        http_method: "GET".into(),
        http_target: format!("/api/concert/events/{}/me", f.event),
        request_json: vec![],
        release_digest: f.product.source_digest().into(),
        authority_expires_unix_micros: now + 1_950_000,
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
    };
    let mut client =
        pp::concert_participant_service_client::ConcertParticipantServiceClient::with_interceptor(
            channel.clone(),
            ServiceIdentityClientInterceptor::new(Workload::Gateway, "gateway-fixture")?,
        );
    let response = client
        .get_participant(signed(
            raw,
            f.actor,
            "/concert.participant.v1.ConcertParticipantService/GetParticipant",
        )?)
        .await?
        .into_inner();
    anyhow::ensure!(
        response.failure.is_none(),
        "real Participant returned failure"
    );
    let cap = response
        .delivery
        .ok_or_else(|| anyhow::anyhow!("Participant lacks delivery"))?;
    let instance = cap.instance_id.parse()?;
    complete(channel, f, cap).await?;
    Ok(instance)
}
async fn intro(
    channel: Channel,
    f: &fixture::Fixture,
    verb: &str,
    suffix: &str,
    body: Value,
) -> anyhow::Result<Value> {
    let now = chrono::Utc::now().timestamp_micros();
    let raw = proto::ProductRequest {
        user_id: f.actor.to_string(),
        event_id: f.event.to_string(),
        session_family_id: f.family.to_string(),
        request_id: Uuid::new_v4().to_string(),
        http_method: verb.into(),
        http_target: format!("/api/concert/events/{}/{suffix}", f.event),
        http_body: if verb == "GET" {
            vec![]
        } else {
            serde_json::to_vec(&body)?
        },
        content_type: if verb == "GET" {
            ""
        } else {
            "application/json"
        }
        .into(),
        idempotency_key: if verb == "GET" {
            String::new()
        } else {
            Uuid::new_v4().to_string()
        },
        authority_expires_unix_micros: now + 1_950_000,
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
        access_expires_unix_micros: now + 60_000_000,
        ..Default::default()
    };
    let mut client = proto::product_service_client::ProductServiceClient::with_interceptor(
        channel.clone(),
        ServiceIdentityClientInterceptor::new(Workload::Gateway, "gateway-fixture")?,
    );
    let result = client
        .dispatch(signed(
            raw,
            f.actor,
            "/concert.product.v1.ProductService/Dispatch",
        )?)
        .await?
        .into_inner();
    anyhow::ensure!(
        result.status == 200,
        "actual Intro {verb} {suffix} status {}",
        result.status
    );
    let value: Value = serde_json::from_slice(&result.body)?;
    let cap = result
        .delivery
        .ok_or_else(|| anyhow::anyhow!("Intro lacks delivery"))?;
    anyhow::ensure!(
        cap.instance_id == std::env::var("MCR_INTRO_EXPECTED_INSTANCE")?,
        "Intro uses a different root"
    );
    complete(channel, f, cap).await?;
    Ok(value)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires an allocated owned PostgreSQL cluster, mTLS and hash-pinned actual Concert main"]
async fn actual_main_mounts_six_intro_operations_with_shared_participant_and_delivery(
) -> anyhow::Result<()> {
    let f = fixture::Fixture::new().await?;
    sqlx::query("insert into concert_meeting_settings(event_id,event_cutoff) values($1,clock_timestamp()+interval '1 day')").bind(f.event).execute(&f.concert).await?;
    let instance: Uuid = std::env::var("MCR_INTRO_EXPECTED_INSTANCE")?.parse()?;
    let values = settings(&f, instance)?;
    let mut invalid = values.clone();
    invalid.remove("CONCERT_INTRO_CURSOR_KEY_HEX");
    let mut rejected = Running::start(&invalid, "intro-invalid-startup.log")?;
    anyhow::ensure!(
        !rejected.wait().await?.success(),
        "incomplete live configuration started"
    );
    anyhow::ensure!(
        tokio::net::TcpStream::connect(environment::address("concert")?)
            .await
            .is_err(),
        "invalid startup opened the Concert listener"
    );
    let mut main = Running::start(&values, "intro-actual-main.log")?;
    let channel = channel(&mut main).await?;
    anyhow::ensure!(
        participant(channel.clone(), &f).await? == instance,
        "Participant uses a different root"
    );
    let draft=intro(channel.clone(),&f,"PUT","me/intro",json!({"expected_revision":0,"text":"Actual production startup","age_bounds":{"minimum_age":null,"maximum_age_exclusive":null},"expires_at":null})).await?;
    anyhow::ensure!(draft["revision"] == 1, "draft revision");
    let published = intro(
        channel.clone(),
        &f,
        "POST",
        "me/intro/publish",
        json!({"expected_revision":1}),
    )
    .await?;
    anyhow::ensure!(published["revision"] == 2, "published revision");
    let own = intro(channel.clone(), &f, "GET", "me/intro", Value::Null).await?;
    anyhow::ensure!(own["revision"] == 2, "own revision");
    let id = own["intro_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("intro identity"))?;
    intro(channel.clone(), &f, "GET", "intros?limit=5", Value::Null).await?;
    intro(
        channel.clone(),
        &f,
        "GET",
        &format!("intros/{id}"),
        Value::Null,
    )
    .await?;
    let closed = intro(
        channel.clone(),
        &f,
        "POST",
        "me/intro/close",
        json!({"expected_revision":2}),
    )
    .await?;
    anyhow::ensure!(closed["revision"] == 3, "closed revision");
    anyhow::ensure!(
        participant(channel.clone(), &f).await? == instance,
        "Participant root changed"
    );
    anyhow::ensure!(
        f.prepared().await? == [0, 0, 0, 0],
        "native prepared transactions remain"
    );
    drop(channel);
    main.stop().await?;
    anyhow::ensure!(
        tokio::net::TcpStream::connect(environment::address("concert")?)
            .await
            .is_err(),
        "main listener survived shutdown"
    );
    println!("Actual production Concert main: six Intro operations, shared Participant, retained inner EOF, mounted delivery and graceful shutdown PASS");
    Ok(())
}
