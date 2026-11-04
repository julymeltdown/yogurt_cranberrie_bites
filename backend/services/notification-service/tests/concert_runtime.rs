//! Fail-closed opt-in parsing and actual pre-database startup refusal. Process
//! checks require an explicitly copied service executable from the finite lease.
use notification_service::config::concert_runtime::ConcertSettings;
use std::{collections::BTreeMap, time::Duration};
#[path = "concert_inbox_support/mod.rs"]
mod support;

fn values() -> BTreeMap<String, String> {
    [
        ("NOTIFICATION_CONCERT_MODE", "live"),
        (
            "NOTIFICATION_CONCERT_AUTH_GRPC_ENDPOINT",
            "https://auth.svc.internal:50101",
        ),
        (
            "NOTIFICATION_CONCERT_FRIEND_GRPC_ENDPOINT",
            "https://friend.svc.internal:50102",
        ),
        (
            "NOTIFICATION_CONCERT_ROOT_GRPC_ENDPOINT",
            "https://concert.svc.internal:50103",
        ),
        (
            "NOTIFICATION_SERVICE_CREDENTIAL",
            "test-notification-own-credential",
        ),
        ("CONCERT_SERVICE_CREDENTIAL", "test-concert-own-credential"),
        ("AUTH_SERVICE_CREDENTIAL", "test-auth-own-credential"),
        ("FRIEND_SERVICE_CREDENTIAL", "test-friend-own-credential"),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.into()))
    .collect()
}
#[test]
fn disabled_mode_does_not_read_unrelated_credentials_or_endpoints() -> anyhow::Result<()> {
    for mode in [None, Some("off")] {
        let mut reads = vec![];
        assert!(ConcertSettings::parse(
            |key| {
                reads.push(key.to_owned());
                Ok(mode.map(str::to_owned))
            },
            false
        )?
        .is_none());
        assert_eq!(reads, vec!["NOTIFICATION_CONCERT_MODE"]);
    }
    Ok(())
}
#[test]
fn live_mode_requires_tls_all_real_owners_and_distinct_credentials() -> anyhow::Result<()> {
    let configured = values();
    assert!(ConcertSettings::parse(|k| Ok(configured.get(k).cloned()), true)?.is_some());
    assert!(ConcertSettings::parse(|k| Ok(configured.get(k).cloned()), false).is_err());
    for key in configured.keys() {
        if key == "NOTIFICATION_CONCERT_MODE" {
            continue;
        }
        let mut incomplete = configured.clone();
        incomplete.remove(key);
        assert!(
            ConcertSettings::parse(|k| Ok(incomplete.get(k).cloned()), true).is_err(),
            "missing {key}"
        );
    }
    for origin in [
        "http://concert.svc.internal:50103",
        "https://concert.svc.internal/path",
        "https://user@concert.svc.internal",
        "https://concert.svc.internal/?query=1",
    ] {
        let mut invalid = configured.clone();
        invalid.insert(
            "NOTIFICATION_CONCERT_ROOT_GRPC_ENDPOINT".into(),
            origin.into(),
        );
        assert!(ConcertSettings::parse(|k| Ok(invalid.get(k).cloned()), true).is_err());
    }
    let mut invalid = configured.clone();
    invalid.insert("NOTIFICATION_CONCERT_MODE".into(), "mock".into());
    assert!(ConcertSettings::parse(|k| Ok(invalid.get(k).cloned()), true).is_err());
    let mut invalid = configured.clone();
    invalid.insert(
        "AUTH_SERVICE_CREDENTIAL".into(),
        configured["CONCERT_SERVICE_CREDENTIAL"].clone(),
    );
    assert!(ConcertSettings::parse(|k| Ok(invalid.get(k).cloned()), true).is_err());
    Ok(())
}
#[tokio::test]
async fn actual_binary_refuses_invalid_live_config_before_database_or_bind() -> anyhow::Result<()> {
    let binary = std::path::PathBuf::from(std::env::var("MCR_NOTIFICATION_SERVICE_BINARY")?);
    anyhow::ensure!(
        binary.is_absolute() && binary.is_file(),
        "explicit copied Notification binary required"
    );
    for (mode, expected) in [
        ("mock", "NOTIFICATION_CONCERT_MODE"),
        ("live", "requires mutual TLS"),
    ] {
        let mut command = tokio::process::Command::new(&binary);
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("NOTIFICATION_RUNTIME_MODE", "parity")
            .env("NOTIFICATION_CONCERT_MODE", mode)
            .env("NOTIFICATION_DATABASE_URL", "invalid-before-database")
            .env("NOTIFICATION_HTTP_ADDR", "127.0.0.1:0")
            .env("NOTIFICATION_METRICS_ADDR", "127.0.0.1:0")
            .env("NOTIFICATION_GRPC_ADDR", "127.0.0.1:0")
            .kill_on_drop(true);
        let result = tokio::time::timeout(Duration::from_secs(5), command.output()).await??;
        assert!(!result.status.success());
        let stderr = String::from_utf8(result.stderr)?;
        assert!(
            stderr.contains(expected),
            "startup did not reject Concert configuration first: {stderr}"
        );
        assert!(!stderr.contains("invalid-before-database"));
    }
    Ok(())
}

#[tokio::test]
async fn actual_main_mounts_live_delivery_and_replay_survives_process_restart() -> anyhow::Result<()>
{
    use notification_service::concert::transport::proto;
    use platform::{
        grpc_auth::{ServiceIdentityClientInterceptor, Workload},
        grpc_tls::TlsMaterial,
    };
    use tonic::{service::interceptor::InterceptedService, Request};
    use uuid::Uuid;
    let pool = support::owned_pool("mcr_concert_notification").await?;
    let root = std::path::PathBuf::from(std::env::var("CONCERT_TEST_OWNER_ROOT")?);
    let binary = std::path::PathBuf::from(std::env::var("MCR_NOTIFICATION_SERVICE_BINARY")?);
    anyhow::ensure!(
        binary.is_absolute() && binary.is_file(),
        "explicit copied service binary required"
    );
    let pki = root.join("pki");
    let tls = TlsMaterial::load(
        pki.join("concert.crt"),
        pki.join("concert.key"),
        pki.join("ca.crt"),
    )?;
    let outbox = Uuid::new_v4();
    let message = proto::DeliverRequest {
        envelope: Some(proto::DeliveryEnvelope {
            outbox_id: outbox.to_string(),
            receipt_id: Uuid::new_v4().to_string(),
            event_id: Uuid::new_v4().to_string(),
            resource_id: Uuid::new_v4().to_string(),
            resource_revision: 1,
        }),
        recipients: vec![proto::DeliveryRecipient {
            user_id: Uuid::new_v4().to_string(),
            source_actor_id: Some(Uuid::new_v4().to_string()),
            kind: 3,
            target: Some(proto::Target {
                target_type: 5,
                target_id: Uuid::new_v4().to_string(),
            }),
        }],
    };
    for _ in 0..2 {
        let reservations = (0..3)
            .map(|_| std::net::TcpListener::bind("127.0.0.1:0"))
            .collect::<std::io::Result<Vec<_>>>()?;
        let addresses = reservations
            .iter()
            .map(|l| l.local_addr())
            .collect::<std::io::Result<Vec<_>>>()?;
        let mut command = tokio::process::Command::new(&binary);
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("NOTIFICATION_RUNTIME_MODE", "parity")
            .env(
                "NOTIFICATION_DATABASE_URL",
                std::env::var("CONCERT_TEST_NOTIFICATION_DATABASE_URL")?,
            )
            .env("NOTIFICATION_HTTP_ADDR", addresses[0].to_string())
            .env("NOTIFICATION_METRICS_ADDR", addresses[1].to_string())
            .env("NOTIFICATION_GRPC_ADDR", addresses[2].to_string())
            .env("NOTIFICATION_RETENTION_PURGE_WORKER_ENABLED", "false")
            .env(
                "NOTIFICATION_GRPC_TLS_CERT_PATH",
                pki.join("notification.crt"),
            )
            .env(
                "NOTIFICATION_GRPC_TLS_KEY_PATH",
                pki.join("notification.key"),
            )
            .env("NOTIFICATION_GRPC_TLS_CA_PATH", pki.join("ca.crt"))
            .envs(values())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        drop(reservations);
        let mut child = command.spawn()?;
        let endpoint = format!("https://{}", addresses[2]);
        let channel = tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                anyhow::ensure!(
                    child.try_wait()?.is_none(),
                    "Notification main exited before readiness"
                );
                if let Ok(channel) = tls.connect(&endpoint, Workload::Notification).await {
                    break Ok::<_, anyhow::Error>(channel);
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await??;
        let identity = ServiceIdentityClientInterceptor::new(
            Workload::Concert,
            "test-concert-own-credential",
        )?;
        let transport = InterceptedService::new(channel, identity);
        let mut delivery =
            proto::concert_delivery_service_client::ConcertDeliveryServiceClient::new(
                transport.clone(),
            );
        for _ in 0..2 {
            let mut request = Request::new(message.clone());
            request.set_timeout(Duration::from_secs(2));
            assert_eq!(
                delivery
                    .deliver(request)
                    .await?
                    .into_inner()
                    .committed_envelope,
                message.envelope
            );
        }
        // The real Inbox route is mounted and rejects absent canonical scope.
        // Reachability is not proof of actual Auth/Friend/Root target policy.
        let mut inbox =
            proto::concert_inbox_service_client::ConcertInboxServiceClient::new(transport);
        let mut request = Request::new(proto::ExecuteRequest {
            operation: proto::InboxOperation::UnreadCount as i32,
            input: Some(proto::execute_request::Input::Count(proto::CountInput {})),
            ..Default::default()
        });
        request.set_timeout(Duration::from_secs(1));
        assert_eq!(
            inbox.execute(request).await.unwrap_err().code(),
            tonic::Code::PermissionDenied
        );
        drop((delivery, inbox));
        let status = tokio::process::Command::new("/bin/kill")
            .arg("-TERM")
            .arg(
                child
                    .id()
                    .ok_or_else(|| anyhow::anyhow!("Notification pid missing"))?
                    .to_string(),
            )
            .status()
            .await?;
        anyhow::ensure!(status.success(), "Notification shutdown signal failed");
        anyhow::ensure!(
            tokio::time::timeout(Duration::from_secs(5), child.wait())
                .await??
                .success(),
            "Notification graceful shutdown failed"
        );
    }
    let count =
        sqlx::query_scalar::<_, i64>("select count(*) from notifications where event_id=$1")
            .bind(outbox)
            .fetch_one(&pool)
            .await?;
    assert_eq!(count, 1);
    pool.close().await;
    Ok(())
}
