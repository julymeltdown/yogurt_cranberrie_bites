//! Owned process fixture for native acceptance; Docker remains the default.
use anyhow::{ensure, Result};
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;
use test_support::redis::RedisContainer;
use uuid::Uuid;

pub enum RedisFixture {
    Docker(Box<RedisContainer>),
    Native(NativeRedis),
}

impl RedisFixture {
    pub async fn new() -> Result<Self> {
        if let Ok(root) = std::env::var("POST_TEST_REDIS_FIXTURE_ROOT") {
            let root = PathBuf::from(root);
            ensure!(root.is_dir(), "owned Redis fixture root must already exist");
            let id = Uuid::new_v4();
            let directory = root.join(id.simple().to_string());
            fs::create_dir(&directory)?;
            let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
            let port = listener.local_addr()?.port();
            drop(listener);
            let logs = PathBuf::from(std::env::var("POST_TEST_REDIS_LOG_DIRECTORY")?);
            ensure!(logs.is_dir(), "owned Redis log directory required");
            let fixture = Self::Native(NativeRedis {
                directory,
                log: logs.join(format!("{id}.log")),
                port,
                child: Mutex::new(None),
            });
            fixture.start().await?;
            fixture.wait_ready(Duration::from_secs(5)).await?;
            Ok(fixture)
        } else {
            Ok(Self::Docker(Box::new(
                test_support::redis::start_redis().await?,
            )))
        }
    }

    pub async fn rm(self) -> Result<()> {
        match self {
            Self::Docker(container) => Ok(container.rm().await?),
            Self::Native(native) => {
                native.stop()?;
                drop(native);
                Ok(())
            }
        }
    }

    pub async fn get_host(&self) -> Result<String> {
        match self {
            Self::Docker(container) => Ok(container.get_host().await?.to_string()),
            Self::Native(_) => Ok("127.0.0.1".into()),
        }
    }

    pub async fn get_host_port_ipv4(&self, port: u16) -> Result<u16> {
        match self {
            Self::Docker(container) => Ok(container.get_host_port_ipv4(port).await?),
            Self::Native(native) => {
                ensure!(port == 6379, "unexpected Redis container port");
                Ok(native.port)
            }
        }
    }

    pub async fn stop_with_timeout(&self, timeout: Option<i32>) -> Result<()> {
        match self {
            Self::Docker(container) => Ok(container.stop_with_timeout(timeout).await?),
            Self::Native(native) => native.stop(),
        }
    }

    pub async fn start(&self) -> Result<()> {
        match self {
            Self::Docker(container) => Ok(container.start().await?),
            Self::Native(native) => {
                let mut child = native.child.lock().expect("owned Redis process lock");
                ensure!(child.is_none(), "owned Redis process is already started");
                let log = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&native.log)?;
                *child = Some(
                    Command::new("redis-server")
                        .args([
                            "--bind",
                            "127.0.0.1",
                            "--protected-mode",
                            "yes",
                            "--port",
                            &native.port.to_string(),
                            "--dir",
                        ])
                        .arg(&native.directory)
                        .args(["--appendonly", "no", "--save", ""])
                        .stdout(Stdio::from(log.try_clone()?))
                        .stderr(Stdio::from(log))
                        .spawn()?,
                );
                Ok(())
            }
        }
    }

    pub async fn wait_ready(&self, timeout: Duration) -> Result<()> {
        let client = redis::Client::open(format!(
            "redis://{}:{}/0",
            self.get_host().await?,
            self.get_host_port_ipv4(6379).await?
        ))?;
        tokio::time::timeout(timeout, async {
            loop {
                if let Ok(mut connection) = client.get_multiplexed_async_connection().await {
                    if let Ok(info) = redis::cmd("INFO").arg("server").query_async::<String>(&mut connection).await {
                        if let Self::Native(native) = self {
                            let child = native.child.lock().expect("owned Redis process lock");
                            let pid = child.as_ref().ok_or_else(|| anyhow::anyhow!("owned Redis process stopped during readiness"))?.id();
                            let observed = info.lines().find_map(|l| l.strip_prefix("process_id:"));
                            ensure!(observed == Some(pid.to_string().as_str()), "Redis port belongs to another process");
                            let run_id = info.lines().find_map(|l| l.strip_prefix("run_id:")).ok_or_else(|| anyhow::anyhow!("Redis run ID absent"))?;
                            println!("POST_REDIS_FIXTURE {}", serde_json::json!({"directory":native.directory,"log":native.log,"pid":pid,"runId":run_id,"port":native.port}));
                        }
                        return Ok::<(), anyhow::Error>(());
                    }
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        }).await??;
        Ok(())
    }
}

pub struct NativeRedis {
    directory: PathBuf,
    log: PathBuf,
    port: u16,
    child: Mutex<Option<Child>>,
}

impl NativeRedis {
    fn stop(&self) -> Result<()> {
        if let Some(mut child) = self.child.lock().expect("owned Redis process lock").take() {
            let pid = child.id();
            if child.try_wait()?.is_none() {
                child.kill()?;
            }
            let status = child.wait()?;
            println!(
                "POST_REDIS_REAPED {}",
                serde_json::json!({"directory":self.directory,"pid":pid,"exitCode":status.code()})
            );
        }
        Ok(())
    }
}

impl Drop for NativeRedis {
    fn drop(&mut self) {
        let result = self
            .stop()
            .and_then(|()| Ok(fs::remove_dir_all(&self.directory)?));
        println!(
            "POST_REDIS_DISPOSED {}",
            serde_json::json!({"directory":self.directory,"removed":!self.directory.exists(),"error":result.err().map(|e|e.to_string())})
        );
    }
}
