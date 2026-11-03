//! Real Kafka process fixture; the normal testcontainers path remains available.
use anyhow::{ensure, Result};
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;
use test_support::kafka::KafkaContainer;
use uuid::Uuid;

pub enum KafkaFixture {
    Docker(Box<KafkaContainer>),
    Native(NativeKafka),
}

impl KafkaFixture {
    pub async fn new() -> Result<Self> {
        let Ok(root) = std::env::var("POST_TEST_KAFKA_FIXTURE_ROOT") else {
            return Ok(Self::Docker(Box::new(
                test_support::kafka::start_kafka().await?,
            )));
        };
        let root = PathBuf::from(root);
        ensure!(root.is_dir(), "owned Kafka root must already exist");
        let directory = root.join(Uuid::new_v4().simple().to_string());
        fs::create_dir(&directory)?;
        let broker = std::net::TcpListener::bind("127.0.0.1:0")?;
        let controller = std::net::TcpListener::bind("127.0.0.1:0")?;
        let port = broker.local_addr()?.port();
        let controller_port = controller.local_addr()?.port();
        let config = directory.join("server.properties");
        fs::write(&config, format!("process.roles=broker,controller\nnode.id=1\ncontroller.listener.names=CONTROLLER\ncontroller.quorum.bootstrap.servers=127.0.0.1:{controller_port}\nlisteners=PLAINTEXT://127.0.0.1:{port},CONTROLLER://127.0.0.1:{controller_port}\nadvertised.listeners=PLAINTEXT://127.0.0.1:{port}\nlistener.security.protocol.map=PLAINTEXT:PLAINTEXT,CONTROLLER:PLAINTEXT\ninter.broker.listener.name=PLAINTEXT\nlog.dirs={}\noffsets.topic.replication.factor=1\ntransaction.state.log.replication.factor=1\ntransaction.state.log.min.isr=1\nshare.coordinator.state.topic.replication.factor=1\nshare.coordinator.state.topic.min.isr=1\ngroup.initial.rebalance.delay.ms=0\nnum.partitions=1\nauto.create.topics.enable=true\n", directory.join("data").display()))?;
        let log_root = PathBuf::from(std::env::var("POST_TEST_KAFKA_LOG_DIRECTORY")?);
        ensure!(log_root.is_dir(), "owned Kafka log directory required");
        let log_path = log_root.join(format!("{}.log", Uuid::new_v4()));
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let generated = Command::new("kafka-storage").arg("random-uuid").output()?;
        ensure!(
            generated.status.success(),
            "Kafka cluster ID generation failed"
        );
        let cluster_id = String::from_utf8(generated.stdout)?.trim().to_owned();
        ensure!(
            !cluster_id.is_empty() && !cluster_id.contains('\n'),
            "invalid generated cluster ID"
        );
        let formatted = Command::new("kafka-storage")
            .args([
                "format",
                "--standalone",
                "--cluster-id",
                &cluster_id,
                "--config",
            ])
            .arg(&config)
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .status()?;
        ensure!(formatted.success(), "owned Kafka storage format failed");
        drop(broker);
        drop(controller);
        let fixture = Self::Native(NativeKafka {
            directory,
            config,
            log: log_path,
            cluster_id,
            port,
            child: Mutex::new(None),
        });
        fixture.start().await?;
        fixture.wait_ready(Duration::from_secs(60)).await?;
        Ok(fixture)
    }

    pub async fn bootstrap(&self) -> Result<String> {
        match self {
            Self::Docker(container) => test_support::kafka::kafka_bootstrap(container).await,
            Self::Native(native) => Ok(format!("127.0.0.1:{}", native.port)),
        }
    }

    pub async fn wait_ready(&self, timeout: Duration) -> Result<()> {
        if let Self::Native(native) = self {
            tokio::time::timeout(timeout, async {
                loop {
                    let pid = {
                        let mut guard = native.child.lock().expect("owned Kafka process lock");
                        let child = guard.as_mut().ok_or_else(|| anyhow::anyhow!("owned Kafka is stopped"))?;
                        ensure!(child.try_wait()?.is_none(), "owned Kafka exited during startup; see {}", native.log.display());
                        child.id()
                    };
                    let listeners = Command::new("lsof").args(["-nP", "-a", "-p", &pid.to_string(), "-i", &format!("TCP:{}", native.port), "-sTCP:LISTEN", "-Fp"]).output()?;
                    if String::from_utf8_lossy(&listeners.stdout).lines().any(|line| line == format!("p{pid}")) {
                        println!("POST_KAFKA_FIXTURE {}", serde_json::json!({"directory":native.directory,"config":native.config,"log":native.log,"pid":pid,"port":native.port,"clusterId":native.cluster_id,"listenerOwnedByChild":true}));
                        return Ok::<(), anyhow::Error>(());
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }).await??;
        }
        test_support::kafka::await_kafka_ready(&self.bootstrap().await?, timeout).await
    }

    pub async fn start(&self) -> Result<()> {
        match self {
            Self::Docker(container) => Ok(container.start().await?),
            Self::Native(native) => {
                let mut guard = native.child.lock().expect("owned Kafka process lock");
                ensure!(guard.is_none(), "owned Kafka is already started");
                let log = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&native.log)?;
                *guard = Some(
                    Command::new("kafka-server-start")
                        .arg(&native.config)
                        .env("KAFKA_HEAP_OPTS", "-Xms128m -Xmx512m")
                        .stdout(Stdio::from(log.try_clone()?))
                        .stderr(Stdio::from(log))
                        .spawn()?,
                );
                Ok(())
            }
        }
    }

    pub async fn stop_with_timeout(&self, timeout: Option<i32>) -> Result<()> {
        match self {
            Self::Docker(container) => Ok(container.stop_with_timeout(timeout).await?),
            Self::Native(native) => native.stop(),
        }
    }
}

pub struct NativeKafka {
    directory: PathBuf,
    config: PathBuf,
    log: PathBuf,
    cluster_id: String,
    port: u16,
    child: Mutex<Option<Child>>,
}

impl NativeKafka {
    fn stop(&self) -> Result<()> {
        if let Some(mut child) = self.child.lock().expect("owned Kafka process lock").take() {
            let pid = child.id();
            if child.try_wait()?.is_none() {
                child.kill()?;
            }
            let status = child.wait()?;
            println!(
                "POST_KAFKA_REAPED {}",
                serde_json::json!({"directory":self.directory,"pid":pid,"exitCode":status.code()})
            );
        }
        Ok(())
    }
}

impl Drop for NativeKafka {
    fn drop(&mut self) {
        let result = self
            .stop()
            .and_then(|()| Ok(fs::remove_dir_all(&self.directory)?));
        println!(
            "POST_KAFKA_DISPOSED {}",
            serde_json::json!({"directory":self.directory,"removed":!self.directory.exists(),"error":result.err().map(|e|e.to_string())})
        );
    }
}
