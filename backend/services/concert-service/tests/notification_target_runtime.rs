//! Configuration and negative-only composition. Reuse the original CF01 private
//! PKI ownership guard; no database, listener or successful target grant is used.
#[path = "canonical_owner_composition_support/environment.rs"]
mod environment;
use concert_service::{
    adapters::out::relationship_authority::guarded::ChildRecoveryReference,
    config::{
        canonical_runtime::CanonicalRuntimeCore,
        notification_target_runtime::{NotificationTargetRuntimeSettings, POLICY},
    },
    notification_live::proto,
    product_rpc::scope_ledger::{ChildIntentSettler, ChildRecoveryKey, ChildRecoveryKeys},
};
use platform::{
    grpc_auth::{CallerRegistry, RequireRegisteredCallerInterceptor, VerifiedCaller, Workload},
    grpc_tls::TlsMaterial,
};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tonic::{service::Interceptor, Request, Status};
use uuid::Uuid;

fn values() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("CONCERT_NOTIFICATION_TARGET_MODE".into(), "live".into()),
        (
            "NOTIFICATION_SERVICE_CREDENTIAL".into(),
            "target-test-notification-current".into(),
        ),
        (
            "NOTIFICATION_SERVICE_CREDENTIAL_PREVIOUS".into(),
            "target-test-notification-previous".into(),
        ),
    ])
}
fn parse(
    values: &BTreeMap<String, String>,
    inbox: bool,
    shared: bool,
    tls: Option<&TlsMaterial>,
) -> anyhow::Result<Option<NotificationTargetRuntimeSettings>> {
    NotificationTargetRuntimeSettings::from_values(
        |name| Ok(values.get(name).cloned()),
        inbox,
        shared,
        tls,
    )
}
#[test]
fn off_ignores_global_credentials_but_rejects_a_live_inbox() -> anyhow::Result<()> {
    for mode in [None, Some("off")] {
        let mut read = vec![];
        let settings = NotificationTargetRuntimeSettings::from_values(
            |name| {
                read.push(name.to_owned());
                if name == "CONCERT_NOTIFICATION_TARGET_MODE" {
                    Ok(mode.map(str::to_owned))
                } else {
                    anyhow::bail!("off must not inspect global credentials or peer settings")
                }
            },
            false,
            false,
            None,
        )?;
        assert!(settings.is_none());
        assert_eq!(read, vec!["CONCERT_NOTIFICATION_TARGET_MODE"]);
        let mut globals = BTreeMap::from([
            (
                "NOTIFICATION_SERVICE_CREDENTIAL".into(),
                "unrelated-global-credential".into(),
            ),
            ("NOTIFICATION_SERVICE_CREDENTIAL_PREVIOUS".into(), "".into()),
        ]);
        if let Some(mode) = mode {
            globals.insert("CONCERT_NOTIFICATION_TARGET_MODE".into(), mode.into());
        }
        assert!(parse(&globals, false, false, None)?.is_none());
        assert!(parse(&globals, true, true, None).is_err());
        assert!(parse(&globals, true, false, None).is_err());
    }
    Ok(())
}
#[test]
fn live_requires_existing_inbox_shared_startup_and_real_tls() -> anyhow::Result<()> {
    let values = values();
    let tls = environment::tls(Workload::Concert)?;
    for (inbox, shared) in [(false, false), (true, false), (false, true)] {
        assert!(parse(&values, inbox, shared, Some(&tls)).is_err());
    }
    assert!(parse(&values, true, true, None).is_err());
    assert!(parse(
        &values,
        true,
        true,
        Some(&TlsMaterial::from_pem(
            b"bad-cert".to_vec(),
            b"bad-key".to_vec(),
            b"bad-ca".to_vec()
        ))
    )
    .is_err());
    assert!(parse(&values, true, true, Some(&tls))?.is_some());
    Ok(())
}
#[test]
fn mode_and_current_previous_credentials_fail_closed() -> anyhow::Result<()> {
    let tls = environment::tls(Workload::Concert)?;
    for mode in ["", "on", "true", "LIVE", " live", "fixture"] {
        let mut values = values();
        values.insert("CONCERT_NOTIFICATION_TARGET_MODE".into(), mode.into());
        assert!(parse(&values, true, true, Some(&tls)).is_err());
    }
    for token in [
        None,
        Some(""),
        Some(" "),
        Some(platform::config::LOCAL_PARITY_INTERNAL_GRPC_SERVICE_TOKEN),
        Some("invalid\ntoken"),
    ] {
        let mut values = values();
        values.remove("NOTIFICATION_SERVICE_CREDENTIAL");
        if let Some(token) = token {
            values.insert("NOTIFICATION_SERVICE_CREDENTIAL".into(), token.into());
        }
        assert!(parse(&values, true, true, Some(&tls)).is_err());
    }
    let mut equal = values();
    equal.insert(
        "NOTIFICATION_SERVICE_CREDENTIAL_PREVIOUS".into(),
        equal["NOTIFICATION_SERVICE_CREDENTIAL"].clone(),
    );
    assert!(parse(&equal, true, true, Some(&tls)).is_err());
    let mut current_only = values();
    current_only.remove("NOTIFICATION_SERVICE_CREDENTIAL_PREVIOUS");
    assert!(parse(&current_only, true, true, Some(&tls))?.is_some());
    Ok(())
}
#[test]
fn notification_credentials_cannot_reuse_any_other_workload_generation() -> anyhow::Result<()> {
    let tls = environment::tls(Workload::Concert)?;
    for role in Workload::ALL {
        if role == Workload::Notification {
            continue;
        }
        for key in [
            platform::config::service_credential_env_name(role),
            platform::config::previous_service_credential_env_name(role),
        ] {
            for notification_key in [
                "NOTIFICATION_SERVICE_CREDENTIAL",
                "NOTIFICATION_SERVICE_CREDENTIAL_PREVIOUS",
            ] {
                let mut values = values();
                values.insert(key.clone(), values[notification_key].clone());
                assert!(parse(&values, true, true, Some(&tls)).is_err());
            }
        }
    }
    Ok(())
}
struct UnavailableChildren;
#[async_trait::async_trait]
impl ChildIntentSettler for UnavailableChildren {
    async fn recover_one(&self, _: &ChildRecoveryReference) -> Result<(), Status> {
        Err(Status::unavailable("negative-only composition fixture"))
    }
}
fn authenticate(
    registry: &CallerRegistry,
    role: Workload,
    token: &str,
) -> Result<Request<()>, Status> {
    let mut interceptor = RequireRegisteredCallerInterceptor::new(registry.clone())
        .map_err(|_| Status::internal("caller registry"))?;
    let mut request = Request::new(());
    request
        .metadata_mut()
        .insert("x-service-caller", role.as_str().parse().unwrap());
    request
        .metadata_mut()
        .insert("x-service-token", token.parse().unwrap());
    interceptor.call(request)
}
#[tokio::test]
async fn composed_callback_uses_dedicated_callers_and_cannot_mint_a_root() -> anyhow::Result<()> {
    use proto::concert_notification_target_authority_server::ConcertNotificationTargetAuthority;
    let tls = environment::tls(Workload::Concert)?;
    let config = values();
    let settings = parse(&config, true, true, Some(&tls))?.unwrap();
    // Closed pool follows canonical_runtime_config.rs; cannot attach to ambient
    // PG defaults or return a successful owner/native policy result.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgres://mcr_target_startup@127.0.0.1:1/mcr_target_startup")?;
    pool.close().await;
    let instance = Uuid::new_v4();
    let keys = Arc::new(ChildRecoveryKeys::new(
        ChildRecoveryKey::new("target-startup", &[91; 32])?,
        vec![],
    )?);
    let core =
        CanonicalRuntimeCore::new(pool.clone(), instance, keys, Arc::new(UnavailableChildren))?;
    let original = core.registry();
    let inherited =
        CallerRegistry::new().register(Workload::Gateway, "target-test-gateway-current")?;
    let runtime = settings.compose(pool, &core);
    assert!(Arc::ptr_eq(&original, &core.registry()));
    assert_eq!(core.ledger().instance(), instance);
    assert!(original.lookup_root_read(&Default::default()).is_err());
    assert_eq!(
        POLICY.methods,
        &[("Authorize", &[Workload::Notification][..])]
    );
    for key in [
        "NOTIFICATION_SERVICE_CREDENTIAL",
        "NOTIFICATION_SERVICE_CREDENTIAL_PREVIOUS",
    ] {
        let authenticated = authenticate(&runtime.callers, Workload::Notification, &config[key])?;
        assert_eq!(
            authenticated.extensions().get::<VerifiedCaller>(),
            Some(&VerifiedCaller(Workload::Notification))
        );
        assert!(authenticate(&inherited, Workload::Notification, &config[key]).is_err());
    }
    assert!(authenticate(&inherited, Workload::Gateway, "target-test-gateway-current").is_ok());
    for role in Workload::ALL {
        if role != Workload::Notification {
            assert!(authenticate(
                &runtime.callers,
                role,
                &config["NOTIFICATION_SERVICE_CREDENTIAL"]
            )
            .is_err());
        }
    }
    let mut request = Request::new(proto::TargetAuthorityRequest::default());
    request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Notification));
    let result = platform::call_deadline::scope(
        Instant::now() + Duration::from_secs(1),
        runtime.service.authorize(request),
    )
    .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::Unavailable);
    let _guarded = platform::grpc_method_policy::guard_without_delegation(
        &POLICY,
        runtime.service.server(),
        runtime.callers,
    )?;
    Ok(())
}
