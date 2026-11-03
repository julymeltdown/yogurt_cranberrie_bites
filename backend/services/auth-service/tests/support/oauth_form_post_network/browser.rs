//! Bounded child lifetime; Unix descendants share a new, fixture-owned group.
use std::process::{Output, Stdio};
use std::time::Duration;
use tokio::process::Command;

pub async fn run(mut command: Command, deadline: Duration) -> anyhow::Result<Output> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
    command
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command.spawn()?;
    let pid = child
        .id()
        .ok_or_else(|| anyhow::anyhow!("fixture child PID missing"))?;
    let mut output = Box::pin(child.wait_with_output());
    match tokio::time::timeout(deadline, &mut output).await {
        Ok(result) => Ok(result?),
        Err(_) => {
            #[cfg(unix)]
            let _ = Command::new("/bin/kill")
                .args(["-KILL", &format!("-{pid}")])
                .status()
                .await?;
            #[cfg(windows)]
            let _ = Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .status()
                .await?;
            // Keep owning the wait future until the terminated child is reaped.
            let reaped = tokio::time::timeout(Duration::from_secs(5), &mut output).await??;
            anyhow::bail!(
                "fixture browser deadline exceeded; child terminated and reaped; progress={}",
                String::from_utf8_lossy(&reaped.stderr)
            )
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn deadline_terminates_and_reaps_the_owned_child_and_its_pipe_holding_descendant() {
    // A surviving descendant holds stdout open, so successful wait_with_output
    // completion requires terminating that descendant as well as its parent.
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "sleep 60 & wait"]);
    let result = run(command, Duration::from_millis(100)).await;
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("child terminated and reaped"));
}
