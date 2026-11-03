//! Owned process handles and incremental evidence for socket acceptance tests.
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::File,
    net::TcpListener,
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;
#[derive(Clone)]
pub(super) struct Evidence(Arc<Mutex<(PathBuf, Value)>>);

impl Evidence {
    pub(super) fn new(directory: PathBuf, boundary: &str) -> Result<Self> {
        std::fs::create_dir_all(&directory)?;
        let proof = Self(Arc::new(Mutex::new((
            directory.join("process-proof.json"),
            json!({
                "status":"RUNNING", "events":[], "boundary":
                boundary
            }),
        ))));
        proof.event(json!({"stage":"started"}))?;
        Ok(proof)
    }
    pub(super) fn event(&self, mut event: Value) -> Result<()> {
        event["observedAt"] = json!(chrono::Utc::now());
        let mut record = self.0.lock().expect("proof lock");
        record.1["events"]
            .as_array_mut()
            .expect("events")
            .push(event);
        std::fs::write(&record.0, serde_json::to_vec_pretty(&record.1)?)?;
        Ok(())
    }
    pub(super) fn finish(&self, status: &str) -> Result<()> {
        let mut record = self.0.lock().expect("proof lock");
        record.1["status"] = json!(status);
        std::fs::write(&record.0, serde_json::to_vec_pretty(&record.1)?)?;
        Ok(())
    }
}

pub(super) struct ProofGuard(pub(super) Evidence);
impl Drop for ProofGuard {
    fn drop(&mut self) {
        let record = self.0 .0.lock().expect("proof lock");
        let passed = record.1["status"] == "PASS";
        drop(record);
        if !passed {
            let _ = self.0.finish("INCOMPLETE");
        }
    }
}

pub(super) struct OwnedChild {
    child: Option<Child>,
    name: String,
    nonce: Uuid,
    proof: Evidence,
}
impl OwnedChild {
    pub(super) fn spawn(
        binary: &Path,
        name: &str,
        env: &BTreeMap<String, String>,
        directory: &Path,
        proof: &Evidence,
    ) -> Result<Self> {
        let nonce = Uuid::new_v4();
        let log = directory.join(format!("{name}-{nonce}.log"));
        let stream = File::create(&log)?;
        let child = Command::new(binary)
            .env_clear()
            .envs(env)
            .stdout(Stdio::from(stream.try_clone()?))
            .stderr(Stdio::from(stream))
            .spawn()
            .with_context(|| format!("start {name}"))?;
        let mut owned = Self {
            child: Some(child),
            name: name.into(),
            nonce,
            proof: proof.clone(),
        };
        // Own the handle before fallible evidence I/O, so failed recording also reaps it.
        if let Err(error) = proof.event(json!({"stage":"spawn", "name":name,"nonce":nonce,
            "pid":owned.child.as_ref().expect("child").id(), "log":log}))
        {
            let _ = owned.stop();
            return Err(error);
        }
        Ok(owned)
    }
    pub(super) fn alive(&mut self) -> Result<()> {
        anyhow::ensure!(
            self.child
                .as_mut()
                .context("missing child")?
                .try_wait()?
                .is_none(),
            "{} exited; inspect process log",
            self.name
        );
        Ok(())
    }
    pub(super) fn stop(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            let pid = child.id();
            let existing = child.try_wait()?;
            let status = match existing {
                Some(status) => status,
                None => {
                    child.kill()?;
                    child.wait()?
                }
            };
            self.proof.event(json!({"stage":"reaped", "name":self.name,"nonce":self.nonce,
                "pid":pid,"signal":status.signal(),"exitCode":status.code(),"killIssued":existing.is_none()}))?;
            anyhow::ensure!(
                existing.is_none() && status.signal() == Some(9),
                "owned process did not remain live until SIGKILL"
            );
        }
        Ok(())
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub(super) fn address() -> Result<String> {
    Ok(TcpListener::bind("127.0.0.1:0")?.local_addr()?.to_string())
}
pub(super) fn put(env: &mut BTreeMap<String, String>, key: &str, value: impl ToString) {
    env.insert(key.into(), value.to_string());
}
pub(super) fn tls(
    env: &mut BTreeMap<String, String>,
    prefix: &str,
    leaf: &test_support::tls::LeafPaths,
) {
    for (key, value) in test_support::tls::env_names(prefix)
        .into_iter()
        .zip(leaf.env_values())
    {
        put(env, &key, value);
    }
}
pub(super) async fn live(child: &mut OwnedChild, metrics: &str) -> Result<()> {
    let http = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(1))
        .build()?;
    let start = Instant::now();
    loop {
        child.alive()?;
        if http
            .get(format!("http://{metrics}/livez"))
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
        {
            return Ok(());
        }
        anyhow::ensure!(
            start.elapsed() < Duration::from_secs(30),
            "liveness timeout"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
