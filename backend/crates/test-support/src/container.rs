use anyhow::Result;
use std::net::TcpListener;

pub(crate) const HOST_PORT_BIND_ATTEMPTS: usize = 5;

/// A loaded host makes fixture containers fail to come up for reasons that say
/// nothing about the code under test, and four shapes were observed while the
/// Docker VM was saturating this machine: the create call timing out, the
/// readiness log never appearing because the entrypoint was killed, a container
/// that ends up exposing no port, and a readiness probe that times out. All four
/// are transient infrastructure failures, so startup retries them a bounded
/// number of times with a backoff.
///
/// No assertion is relaxed: a container must still start and pass its readiness
/// probe, and an error that is not one of these shapes is returned immediately
/// so a genuine configuration or product fault cannot be hidden by retrying.
pub(crate) const CONTAINER_START_ATTEMPTS: usize = 5;

/// Delay between attempts. An immediate retry against host memory pressure tends
/// to fail identically, so give the machine a moment to recover first.
pub(crate) const CONTAINER_START_BACKOFF: std::time::Duration = std::time::Duration::from_secs(3);

/// Total time the retry loop may spend before it gives up and reports the last
/// transient failure.
///
/// Attempts alone do not bound wall time: the pinned client waits up to 60 s for
/// its own startup strategy, so five attempts plus backoff can hold a binary open
/// for roughly five minutes per fixture. With no container runtime reachable, a
/// whole test binary then looks like it is "still running" instead of failing.
/// That is the shape that made the aborted 2026-10-18 run hard to read. The budget
/// bounds the wait and keeps the last infrastructure error as the report.
pub(crate) const CONTAINER_START_TOTAL_BUDGET: std::time::Duration =
    std::time::Duration::from_secs(150);

/// Why the container-runtime endpoint is unusable, if it is.
///
/// Connecting to the socket is not enough: a half-started Docker Desktop accepts
/// the connection and then never answers, which is exactly the state observed on
/// 2026-10-18 at 11:42 UTC. A client that only connects therefore waits out its
/// own startup timeout, and with retries that holds a test binary open for
/// minutes while a log shows nothing but "running".
///
/// So this sends Docker's `/_ping` request and requires a response within a short
/// deadline. It does not pull an image or create a container, so a `None` result
/// is not a health proof; a `Some` reason is a definite, named blocker.
pub(crate) fn container_runtime_unavailable_reason() -> Option<String> {
    use std::io::{Read, Write};
    use std::time::Duration;

    // Generous enough that a busy engine still passes: the question is whether the
    // daemon answers at all, not how fast it is. A daemon that is not serving the
    // socket never answers, so it fails on the deadline rather than on speed.
    const PING_DEADLINE: Duration = Duration::from_secs(8);

    // `DOCKER_HOST` may name a non-socket transport, which this probe cannot
    // speak. Those are left to the client rather than reported as unavailable.
    let socket = match std::env::var("DOCKER_HOST") {
        Ok(value) => match value.strip_prefix("unix://") {
            Some(path) => std::path::PathBuf::from(path),
            None => return None,
        },
        Err(_) => std::path::PathBuf::from("/var/run/docker.sock"),
    };
    if !socket.exists() {
        return Some(format!(
            "container runtime socket {} does not exist",
            socket.display()
        ));
    }
    let Ok(mut stream) = std::os::unix::net::UnixStream::connect(&socket) else {
        return Some(format!(
            "container runtime socket {} is not accepting connections",
            socket.display()
        ));
    };
    let _ = stream.set_read_timeout(Some(PING_DEADLINE));
    let _ = stream.set_write_timeout(Some(PING_DEADLINE));
    if stream
        .write_all(b"GET /_ping HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return Some("container runtime did not accept a /_ping request".to_string());
    }
    let mut response = Vec::new();
    let mut buffer = [0_u8; 256];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                response.extend_from_slice(&buffer[..read]);
                if response.windows(5).any(|w| w == b"HTTP/") {
                    break;
                }
            }
            Err(_) => {
                return Some(format!(
                    "container runtime did not answer /_ping within {}s",
                    PING_DEADLINE.as_secs()
                ))
            }
        }
    }
    let text = String::from_utf8_lossy(&response);
    // Any HTTP status counts: the daemon is serving. Reading a specific code here
    // would turn an unrelated status into a "runtime unavailable" report.
    if text.starts_with("HTTP/") {
        None
    } else {
        Some("container runtime did not answer /_ping with an HTTP response".to_string())
    }
}

/// Run `start` until it succeeds, retrying only transient infrastructure
/// failures, and return the last such failure once the attempts are exhausted.
///
/// A non-transient error is returned immediately so a genuine configuration or
/// product fault is never masked by retrying. This is the control flow the
/// PostgreSQL and Redis fixtures use. It is generic over the start future and the
/// delay so the loop itself can be executed in a test without a container
/// runtime: the unit tests below drive it with a failing closure and assert the
/// attempt count, which is the part a Docker-dependent test cannot reach when no
/// engine is available.
pub(crate) async fn retry_transient_start<T, F, Fut>(
    attempts: usize,
    backoff: std::time::Duration,
    mut start: F,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    // A reachable socket is not a serving daemon: a half-started Docker Desktop
    // accepts the connection and never answers. Probing first turns that into an
    // immediate named error instead of one client startup timeout per attempt.
    if let Some(reason) = container_runtime_unavailable_reason() {
        return Err(anyhow::anyhow!("container runtime unavailable: {reason}"));
    }
    retry_transient_start_within(attempts, backoff, CONTAINER_START_TOTAL_BUDGET, &mut start).await
}

/// The retry loop with an explicit total budget, so the budget itself is
/// testable without waiting out the production value.
///
/// This variant deliberately does not probe the runtime, so its behavior depends
/// only on the injected start closure. The probe is a caller concern, which keeps
/// the loop's own tests independent of whether this host has a working engine.
pub(crate) async fn retry_transient_start_within<T, F, Fut>(
    attempts: usize,
    backoff: std::time::Duration,
    budget: std::time::Duration,
    start: &mut F,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let deadline = tokio::time::Instant::now() + budget;
    let mut last_transient: Option<anyhow::Error> = None;
    for index in 0..attempts.max(1) {
        if index > 0 {
            if tokio::time::Instant::now() + backoff > deadline {
                break;
            }
            tokio::time::sleep(backoff).await;
        }
        match start().await {
            Ok(value) => return Ok(value),
            Err(error) => {
                if !is_transient_container_start_failure(&error) {
                    return Err(error);
                }
                last_transient = Some(error);
            }
        }
        if tokio::time::Instant::now() >= deadline {
            break;
        }
    }
    Err(last_transient.unwrap_or_else(|| {
        anyhow::anyhow!(
            "container start exhausted its {}s budget before a single attempt",
            budget.as_secs()
        )
    }))
}

pub(crate) fn is_transient_container_start_failure(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        let message = cause.to_string().to_ascii_lowercase();
        let create_timed_out =
            message.contains("failed to create a container") && message.contains("timeout");
        // The entrypoint was killed before it announced readiness. On this host
        // that is the guest OOM killer firing while the host is loaded, which the
        // product cannot influence.
        let killed_before_ready = (message.contains("container is not ready")
            || message.contains("failed to wait for container log"))
            && (message.contains("killed") || message.contains("end of stream"));
        // The container came up without the port its fixture needs.
        let port_never_exposed = message.contains("does not expose port");
        // A readiness probe ran out of time. The runtime reports
        // "container startup timeout" when the wait strategy expires, which is a
        // different string from the fixture's own probe message, so both are
        // accepted rather than one guessed spelling.
        let readiness_timed_out = message.contains("did not become ready within")
            || (message.contains("container is not ready") && message.contains("timeout"));
        create_timed_out || killed_before_ready || port_never_exposed || readiness_timed_out
    })
}

pub(crate) fn reserve_loopback_host_port() -> Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

pub(crate) fn is_host_port_conflict(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        let message = cause.to_string().to_ascii_lowercase();
        message.contains("address already in use")
            && (message.contains("host port")
                || message.contains("ports are not available: exposing port tcp"))
    })
}

#[cfg(test)]
mod tests {
    use super::{
        is_host_port_conflict, is_transient_container_start_failure, retry_transient_start_within,
        CONTAINER_START_ATTEMPTS,
    };

    /// The retry loop is executed here, not merely classified.
    ///
    /// The classifier tests below prove which error strings count as transient.
    /// They cannot show that the loop then retries the right number of times, that
    /// it gives up with the last infrastructure error, or that it returns a real
    /// fault immediately. Those are the behaviors the fixtures depend on, and a
    /// Docker-backed test cannot exercise them on a host with no engine, so the
    /// loop takes an injectable start closure and delay and is driven directly.
    mod retry_loop {
        use super::*;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::time::Duration;

        const FAST: Duration = Duration::from_millis(1);
        const ROOMY: Duration = Duration::from_secs(30);
        const CREATE_TIMEOUT: &str = "failed to create a container: Timeout error";

        fn counter() -> Arc<AtomicUsize> {
            Arc::new(AtomicUsize::new(0))
        }

        /// Drive the loop with a closure that always fails the same way.
        async fn always_fails(
            attempts: usize,
            seen: Arc<AtomicUsize>,
            message: &'static str,
        ) -> anyhow::Result<()> {
            let mut start = move || {
                let seen = Arc::clone(&seen);
                async move {
                    seen.fetch_add(1, Ordering::SeqCst);
                    Err(anyhow::anyhow!(message))
                }
            };
            retry_transient_start_within(attempts, FAST, ROOMY, &mut start).await
        }

        #[tokio::test]
        async fn a_transient_failure_is_retried_until_it_succeeds() {
            let seen = counter();
            let observed = Arc::clone(&seen);
            let mut start = move || {
                let observed = Arc::clone(&observed);
                async move {
                    let attempt = observed.fetch_add(1, Ordering::SeqCst) + 1;
                    if attempt < 3 {
                        Err(anyhow::anyhow!(CREATE_TIMEOUT))
                    } else {
                        Ok("container")
                    }
                }
            };
            let result =
                retry_transient_start_within(CONTAINER_START_ATTEMPTS, FAST, ROOMY, &mut start)
                    .await;

            assert_eq!("container", result.expect("the third attempt must succeed"));
            assert_eq!(3, seen.load(Ordering::SeqCst), "two retries then a success");
        }

        #[tokio::test]
        async fn a_real_fault_is_not_retried_even_once() {
            let seen = counter();
            let error = always_fails(
                CONTAINER_START_ATTEMPTS,
                Arc::clone(&seen),
                "failed to start container: image not found",
            )
            .await
            .expect_err("a non-transient failure must be returned");

            assert!(error.to_string().contains("image not found"));
            assert_eq!(
                1,
                seen.load(Ordering::SeqCst),
                "a real fault must not be retried"
            );
        }

        #[tokio::test]
        async fn exhausting_the_attempts_returns_the_last_infrastructure_error() {
            let seen = counter();
            let error = always_fails(
                CONTAINER_START_ATTEMPTS,
                Arc::clone(&seen),
                "container is not ready: container startup timeout",
            )
            .await
            .expect_err("exhausted attempts must fail");

            assert!(
                error.to_string().contains("container startup timeout"),
                "the returned error must name the infrastructure cause, got {error}"
            );
            assert_eq!(CONTAINER_START_ATTEMPTS, seen.load(Ordering::SeqCst));
        }

        #[tokio::test]
        async fn a_killed_entrypoint_is_retried_then_reported() {
            // The shape that failed five profile-service tests on 2026-10-18.
            let seen = counter();
            let error = always_fails(
                2,
                Arc::clone(&seen),
                "app: container is not ready: failed to wait for container log: End of stream reached",
            )
            .await
            .expect_err("a pending entrypoint must not pass");

            assert!(error.to_string().contains("End of stream reached"));
            assert_eq!(2, seen.load(Ordering::SeqCst));
        }

        #[tokio::test]
        async fn a_zero_attempt_budget_still_runs_once() {
            // A zero budget would otherwise report a start failure for a container
            // that was never attempted, so the loop clamps to one attempt.
            let mut start = || async { Ok("container") };
            let value: &str = retry_transient_start_within(0, FAST, ROOMY, &mut start)
                .await
                .expect("a zero-attempt budget still runs once");
            assert_eq!("container", value);
        }

        #[tokio::test]
        async fn the_budget_stops_a_long_retry_chain() {
            // Attempts alone do not bound wall time. With a budget shorter than the
            // configured backoff, the loop must stop instead of sleeping through
            // every remaining attempt.
            let seen = counter();
            let observed = Arc::clone(&seen);
            let mut start = move || {
                let observed = Arc::clone(&observed);
                async move {
                    observed.fetch_add(1, Ordering::SeqCst);
                    Err(anyhow::anyhow!(CREATE_TIMEOUT))
                }
            };
            let long_backoff = Duration::from_secs(3600);
            let result: anyhow::Result<()> = retry_transient_start_within(
                CONTAINER_START_ATTEMPTS,
                long_backoff,
                FAST,
                &mut start,
            )
            .await;
            let error = result.expect_err("the budget must stop the chain");

            assert!(error.to_string().contains("Timeout error"));
            assert_eq!(
                1,
                seen.load(Ordering::SeqCst),
                "no attempt after the budget closed"
            );
        }
    }

    #[test]
    fn recognizes_a_docker_create_timeout_as_transient() {
        assert!(is_transient_container_start_failure(&anyhow::anyhow!(
            "failed to create a container: Timeout error"
        )));
    }

    #[test]
    fn recognizes_every_transient_startup_shape_observed_on_a_loaded_host() {
        // The entrypoint was killed before it announced readiness.
        assert!(is_transient_container_start_failure(&anyhow::anyhow!(
            "container is not ready: failed to wait for container log: End of stream reached before finding message: [\"docker-entrypoint.sh: line 92: 29 Killed initdb\"]"
        )));
        // The container came up without the port the fixture needs.
        assert!(is_transient_container_start_failure(&anyhow::anyhow!(
            "container 'abc' does not expose port 5432/tcp"
        )));
        // A readiness probe ran out of time.
        assert!(is_transient_container_start_failure(&anyhow::anyhow!(
            "Redis testcontainer did not become ready within 10s"
        )));
        // The container runtime's own wait-strategy message, which is a
        // different string from the fixture probe above and was observed
        // failing a whole integration binary before it was classified.
        assert!(is_transient_container_start_failure(&anyhow::anyhow!(
            "app: container is not ready: container startup timeout"
        )));
    }

    #[test]
    fn a_generic_failure_is_not_retried() {
        // Only the known transient shapes are retried, so a real fault is still
        // reported immediately instead of being masked by five attempts.
        assert!(!is_transient_container_start_failure(&anyhow::anyhow!(
            "product assertion failed: expected 1 destination effect"
        )));
    }

    #[test]
    fn does_not_treat_other_startup_failures_as_a_create_timeout() {
        assert!(!is_transient_container_start_failure(&anyhow::anyhow!(
            "failed to start container: image not found"
        )));
        assert!(!is_transient_container_start_failure(&anyhow::anyhow!(
            "failed to create a container: permission denied"
        )));
    }

    #[test]
    fn recognizes_testcontainers_host_port_conflicts() {
        assert!(is_host_port_conflict(&anyhow::anyhow!(
            "failed to bind host port for 0.0.0.0:63794: address already in use"
        )));
        assert!(is_host_port_conflict(&anyhow::anyhow!(
            "Docker responded with status code 500: Ports are not available: exposing port TCP 0.0.0.0:63876 -> 127.0.0.1:0: listen tcp 0.0.0.0:63876: bind: address already in use"
        )));
    }

    #[test]
    fn does_not_retry_other_startup_errors_or_application_socket_conflicts() {
        assert!(!is_host_port_conflict(&anyhow::anyhow!(
            "failed to bind host port: permission denied"
        )));
        assert!(!is_host_port_conflict(&anyhow::anyhow!(
            "address already in use while binding a process socket"
        )));
    }
}
