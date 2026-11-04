//! Uses actual art ca5d4bca through an explicitly frozen six-file readset.
//! PG pool ownership checks are reused read-only from the committed Wall fixture
//! at cd7f6584; no ambient database or synthetic successful Auth guard is used.
#[allow(dead_code)]
#[path = "mural_tools_b_support/environment.rs"]
mod environment;
use concert_service::mural_seed::*;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::path::PathBuf;
use uuid::Uuid;
const SOURCE: &str = "ca5d4bcad50d6159597309a961a29a8e4d1f3bb9";
const MANIFEST: &str = "3f1aa318f0e4b7d8fa7f079b1101f7386619291a8dbb3bb04ef70820952f34ce";
fn selected_hash() -> [u8; 32] {
    std::array::from_fn(|i| u8::from_str_radix(&MANIFEST[2 * i..2 * i + 2], 16).unwrap())
}
fn root() -> anyhow::Result<PathBuf> {
    Ok(PathBuf::from(std::env::var("MCR_MURAL_SEED_PACK")?))
}
fn pack() -> anyhow::Result<ValidatedPack> {
    Ok(load_pack(&root()?, selected_hash())?)
}
fn selection(event_id: Uuid) -> OperatorSelection {
    OperatorSelection {
        event_id,
        source_revision: SOURCE.into(),
        operator_binding: "owned-native-fixture:not-production-operator".into(),
        cooldown_ms: 1200,
        initial_state: InitialState::Editable,
    }
}
struct Bytes {
    manifest: Vec<u8>,
    source: Vec<u8>,
    pixels: Vec<u8>,
    palette: Vec<u8>,
    preview: Vec<u8>,
    evidence: Vec<u8>,
}
impl Bytes {
    fn read() -> anyhow::Result<Self> {
        let root = root()?;
        Ok(Self {
            manifest: std::fs::read(root.join("manifest.json"))?,
            source: std::fs::read(root.join("source.svg"))?,
            pixels: std::fs::read(root.join("pixels.bin"))?,
            palette: std::fs::read(root.join("palette.json"))?,
            preview: std::fs::read(root.join("preview.png"))?,
            evidence: std::fs::read(root.join("README.md"))?,
        })
    }
    fn validate(&self, hash: [u8; 32]) -> Result<ValidatedPack, SeedError> {
        validate_pack(
            PackBytes {
                manifest: &self.manifest,
                source: &self.source,
                pixels: &self.pixels,
                palette: &self.palette,
                preview: &self.preview,
                qualification_path: "README.md",
                qualification: &self.evidence,
            },
            hash,
        )
    }
    fn repin(&mut self, name: &str, bytes: &[u8]) {
        let mut m: Value = serde_json::from_slice(&self.manifest).unwrap();
        m["files"][name]["sha256"] = Value::String(
            Sha256::digest(bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        );
        self.manifest = serde_json::to_vec(&m).unwrap();
    }
    fn new_hash(&self) -> [u8; 32] {
        Sha256::digest(&self.manifest).into()
    }
}
#[test]
#[ignore = "Explicit frozen qualified art readset required; not a generated fixture"]
fn actual_pack_and_tamper_rejection() -> anyhow::Result<()> {
    let bytes = Bytes::read()?;
    let p = bytes.validate(selected_hash())?;
    assert_eq!(p.pixels().len(), 16384);
    assert_eq!(p.palette()[0], "#171617");
    assert_eq!(load_pack(&root()?, selected_hash())?.pixels(), p.pixels());
    assert!(bytes.validate([0; 32]).is_err());
    for kind in 0..9 {
        let mut b = Bytes::read()?;
        match kind {
            0 => b.source.push(b' '), // Original selection cannot accept edited bytes.
            1 => {
                b.pixels.pop();
                let v = b.pixels.clone();
                b.repin("pixels", &v);
            }
            2 => {
                b.pixels[0] = 16;
                let v = b.pixels.clone();
                b.repin("pixels", &v);
            }
            3 => {
                let mut v: Vec<String> = serde_json::from_slice(&b.palette)?;
                v[1] = "#abcdef".into();
                b.palette = serde_json::to_vec(&v)?;
                let v = b.palette.clone();
                b.repin("palette", &v);
            }
            4 => {
                b.pixels[0] = (b.pixels[0] + 1) % 16;
                let v = b.pixels.clone();
                b.repin("pixels", &v);
            } // PNG must still agree after all digests match.
            5 => {
                let mut m: Value = serde_json::from_slice(&b.manifest)?;
                m["invented_approval"] = true.into();
                b.manifest = serde_json::to_vec(&m)?;
            }
            6 => {
                let mut m: Value = serde_json::from_slice(&b.manifest)?;
                m["provenance"]["qualification_evidence"] = "../foreign.md".into();
                b.manifest = serde_json::to_vec(&m)?;
            }
            7 => {
                b.preview[0] ^= 1;
                let v = b.preview.clone();
                b.repin("preview", &v);
            }
            8 => {
                let text = String::from_utf8(b.manifest.clone())?;
                b.manifest = text
                    .replacen(
                        "\"asset_revision\": 1",
                        "\"asset_revision\": 1, \"asset_revision\": 1",
                        1,
                    )
                    .into_bytes();
            }
            _ => unreachable!(),
        }
        assert!(
            b.validate(b.new_hash()).is_err(),
            "tamper kind {kind} accepted"
        );
    }
    println!("MURAL_SEED_ART=actual ca5d4bca manifest {MANIFEST}; 10 rejection checks PASS");
    Ok(())
}
async fn event(pool: &PgPool) -> anyhow::Result<Uuid> {
    let event = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Owned seed SQL fixture','OPEN')")
        .bind(event).bind(format!("seed-{}",event.simple())).execute(pool).await?;
    Ok(event)
}
async fn absent(pool: &PgPool, event: Uuid) -> anyhow::Result<()> {
    for table in [
        "concert_mural_state",
        "concert_mural_tool_config",
        "concert_mural_seed_provenance",
    ] {
        let count: i64 =
            sqlx::query_scalar(&format!("select count(*) from {table} where event_id=$1"))
                .bind(event)
                .fetch_one(pool)
                .await?;
        assert_eq!(count, 0, "partial insert in {table}");
    }
    Ok(())
}
async fn fingerprint(pool: &PgPool, event: Uuid) -> anyhow::Result<Value> {
    Ok(sqlx::query_scalar("select jsonb_build_object('state',to_jsonb(s),'config',to_jsonb(c),'provenance',to_jsonb(p)) from concert_mural_state s join concert_mural_tool_config c using(event_id) join concert_mural_seed_provenance p using(event_id) where s.event_id=$1")
        .bind(event).fetch_one(pool).await?)
}
#[tokio::test]
#[ignore = "Explicit owned allocated PG16 + frozen actual art readset required"]
async fn native_seed_atomic_operator_binding_and_conflicts() -> anyhow::Result<()> {
    let pool = environment::pool("concert").await?;
    let p = pack()?;
    assert!(matches!(
        provision(&pool, &p, &selection(Uuid::nil())).await,
        Err(SeedError::InvalidSelection)
    ));
    assert!(matches!(
        provision(&pool, &p, &selection(Uuid::new_v4())).await,
        Err(SeedError::EventUnavailable)
    ));
    let ev = event(&pool).await?;
    let s = selection(ev);
    let a = provision(&pool, &p, &s).await?;
    assert_eq!(a.revision, 1);
    assert!(!a.exact_pristine_retry);
    let before = fingerprint(&pool, ev).await?;
    let b = provision(&pool, &p, &s).await?;
    assert!(b.exact_pristine_retry);
    assert_eq!(a.snapshot_id, b.snapshot_id);
    assert_eq!(a.provisioned_at, b.provisioned_at);
    for kind in 0..4 {
        let mut other = selection(ev);
        match kind {
            0 => other.operator_binding = "different-test-operator".into(),
            1 => other.source_revision = "a".repeat(40),
            2 => other.cooldown_ms += 1,
            3 => other.initial_state = InitialState::Paused,
            _ => unreachable!(),
        }
        assert!(matches!(
            provision(&pool, &p, &other).await,
            Err(SeedError::NotPristine)
        ));
        assert_eq!(before, fingerprint(&pool, ev).await?);
    }
    let row = sqlx::query("select pixels,palette from concert_mural_state where event_id=$1")
        .bind(ev)
        .fetch_one(&pool)
        .await?;
    assert_eq!(row.get::<Vec<u8>, _>("pixels"), p.pixels());
    assert_eq!(row.get::<Vec<String>, _>("palette"), p.palette());
    let failed = event(&pool).await?;
    // Actual deferred commit failure occurs after all three INSERTs. The entire
    // transaction must roll back, not leave an apparently seeded partial root.
    sqlx::raw_sql("create function seed_test_commit_failure() returns trigger language plpgsql as $$ begin raise exception 'injected seed commit failure'; end $$; create constraint trigger seed_test_commit_failure after insert on concert_mural_seed_provenance deferrable initially deferred for each row execute function seed_test_commit_failure();").execute(&pool).await?;
    assert!(provision(&pool, &p, &selection(failed)).await.is_err());
    absent(&pool, failed).await?;
    sqlx::raw_sql("drop trigger seed_test_commit_failure on concert_mural_seed_provenance; drop function seed_test_commit_failure();").execute(&pool).await?;
    let recovered = provision(&pool, &p, &selection(failed)).await?;
    assert!(!recovered.exact_pristine_retry);
    let foreign = event(&pool).await?;
    sqlx::query("insert into concert_mural_state(event_id,snapshot_id,state,pixels,palette,cooldown_ms) values($1,$2,'EDITABLE',$3,$4,1200)")
        .bind(foreign).bind(Uuid::new_v4()).bind(p.pixels()).bind(p.palette()).execute(&pool).await?;
    assert!(matches!(
        provision(&pool, &p, &selection(foreign)).await,
        Err(SeedError::NotPristine)
    ));
    println!("MURAL_SEED_NATIVE=atomic rollback + exact operator/source retry + foreign root refusal PASS");
    Ok(())
}
#[tokio::test]
#[ignore = "Explicit owned allocated PG16 + frozen actual art readset required"]
async fn native_seed_concurrency_history_and_immutable_provenance() -> anyhow::Result<()> {
    let pool = environment::pool("concert").await?;
    let p = pack()?;
    let ev = event(&pool).await?;
    let s = selection(ev);
    let (a, b) = tokio::join!(provision(&pool, &p, &s), provision(&pool, &p, &s));
    let a = a?;
    let b = b?;
    assert_eq!(a.snapshot_id, b.snapshot_id);
    assert_ne!(a.exact_pristine_retry, b.exact_pristine_retry);
    for statement in [
        "update concert_mural_seed_provenance set operator_binding='rewritten' where event_id=$1",
        "delete from concert_mural_seed_provenance where event_id=$1",
    ] {
        assert!(sqlx::query(statement)
            .bind(ev)
            .execute(&pool)
            .await
            .is_err());
    }
    assert!(sqlx::query("truncate concert_mural_seed_provenance")
        .execute(&pool)
        .await
        .is_err());
    // The real snapshot reader creates this immutable checkpoint. A seeded
    // revision with any checkpoint is no longer provisioner's pristine input.
    sqlx::query("insert into concert_mural_snapshots(snapshot_id,event_id,revision,state,pixels,palette,created_at) select snapshot_id,event_id,revision,state,pixels,palette,snapshot_created_at from concert_mural_state where event_id=$1")
        .bind(ev).execute(&pool).await?;
    let before = fingerprint(&pool, ev).await?;
    assert!(matches!(
        provision(&pool, &p, &s).await,
        Err(SeedError::NotPristine)
    ));
    assert_eq!(before, fingerprint(&pool, ev).await?);
    let edited = event(&pool).await?;
    let selected = selection(edited);
    provision(&pool, &p, &selected).await?;
    sqlx::query("update concert_mural_state set revision=2,snapshot_id=$2,reset_revision=2,pixels=set_byte(pixels,0,1) where event_id=$1")
        .bind(edited).bind(Uuid::new_v4()).execute(&pool).await?;
    assert!(matches!(
        provision(&pool, &p, &selected).await,
        Err(SeedError::NotPristine)
    ));
    let prepared: i64 = sqlx::query_scalar("select count(*) from pg_prepared_xacts")
        .fetch_one(&pool)
        .await?;
    assert_eq!(prepared, 0);
    println!("MURAL_SEED_NATIVE=concurrent exact retries + checkpoint/mutated-root refusal + immutable full provenance PASS");
    Ok(())
}

#[tokio::test]
#[ignore = "Controller-owned bootstrap event and explicitly selected local operator run only"]
async fn operator_provision_bootstrap_event() -> anyhow::Result<()> {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    use std::{os::unix::fs::MetadataExt, str::FromStr};
    let root = environment::root()?;
    let options = PgConnectOptions::from_str(&std::env::var("MCR_CANONICAL_CONCERT_URL")?)?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_concert")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "Foreign operator database"
    );
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await?;
    let row=sqlx::query("select current_setting('data_directory') data,current_setting('listen_addresses') listeners,(select system_identifier::text from pg_control_system()) system").fetch_one(&pool).await?;
    anyhow::ensure!(
        row.get::<String, _>("data") == root.join("data").to_string_lossy()
            && row.get::<String, _>("listeners").is_empty()
            && row.get::<String, _>("system") == std::env::var("MCR_CANONICAL_SYSTEM")?,
        "Foreign actual operator cluster"
    );
    // The controller's owning bootstrap returns this canonical event. It must
    // also match the actual selected release, never an invented production UUID.
    let input: Value =
        serde_json::from_slice(&std::fs::read(root.join("in01-concert-inputs.json"))?)?;
    let event = Uuid::parse_str(
        input["CONCERT_WORLD_EVENT_ID"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Bootstrap event missing"))?,
    )?;
    let release: Value = serde_json::from_slice(&std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../releases/operator-bundle/quality-v6/bootstrap.json"),
    )?)?;
    anyhow::ensure!(
        release["event"]["event_id"].as_str() == Some(event.to_string().as_str()),
        "Bootstrap/source event mismatch"
    );
    let metadata = std::fs::metadata(&root)?;
    let mut hash = Sha256::new();
    hash.update(b"mcr-mural-owned-operator-run/1\0");
    hash.update(metadata.dev().to_be_bytes());
    hash.update(metadata.ino().to_be_bytes());
    hash.update(metadata.uid().to_be_bytes());
    hash.update(std::fs::read(root.join("mcr-owner.json"))?);
    let binding: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    let mut selected = selection(event);
    selected.operator_binding = format!("local-owned-run:sha256:{binding}");
    let p = pack()?;
    let result = provision(&pool, &p, &selected).await?;
    anyhow::ensure!(
        !result.exact_pristine_retry,
        "First owning bootstrap seed must be new"
    );
    let retry = provision(&pool, &p, &selected).await?;
    anyhow::ensure!(
        retry.exact_pristine_retry && retry.snapshot_id == result.snapshot_id,
        "Exact source/operator retry changed seed"
    );
    let row = sqlx::query("select pixels,palette from concert_mural_state where event_id=$1")
        .bind(event)
        .fetch_one(&pool)
        .await?;
    assert_eq!(row.get::<Vec<u8>, _>("pixels"), p.pixels());
    assert_eq!(row.get::<Vec<String>, _>("palette"), p.palette());
    println!(
        "MURAL_SEED_PROVISION={}",
        serde_json::to_string(
            &serde_json::json!({"result":result,"source_revision":SOURCE,
        "operator_binding":selected.operator_binding,"cooldown_ms":selected.cooldown_ms,"initial_state":"EDITABLE",
        "pixel_sha256":row.get::<Vec<u8>,_>("pixels").iter().fold(Sha256::new(),|mut h,b|{h.update([*b]);h}).finalize().iter().map(|b|format!("{b:02x}")).collect::<String>(),
        "limits":["Isolated operator run, not a production Ops role grant","No authenticated server C03/snapshot or browser/two-client IN04 in this local producer test"]})
        )?
    );
    Ok(())
}
