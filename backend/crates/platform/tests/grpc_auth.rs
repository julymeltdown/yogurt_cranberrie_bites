use platform::grpc_auth::{
    authorize_call, CallerPolicy, CallerRegistry, RequireRegisteredCallerInterceptor, RpcAction,
    ServiceIdentityClientInterceptor, VerifiedCaller, Workload,
};
use tonic::service::Interceptor;
use tonic::Request;

const GATEWAY_TOKEN: &str = "gateway-credential";
const AUTH_TOKEN: &str = "auth-credential";
const POST_TOKEN: &str = "post-credential";

fn registry() -> CallerRegistry {
    CallerRegistry::new()
        .register(Workload::Gateway, GATEWAY_TOKEN)
        .expect("gateway")
        .register(Workload::Auth, AUTH_TOKEN)
        .expect("auth")
        .register(Workload::Post, POST_TOKEN)
        .expect("post")
}

fn authenticated_request(workload: Workload, token: &str) -> Request<()> {
    let mut client = ServiceIdentityClientInterceptor::new(workload, token).expect("identity");
    let mut server =
        RequireRegisteredCallerInterceptor::new(registry()).expect("server interceptor");
    server
        .call(client.call(Request::new(())).expect("client request"))
        .expect("authenticated request")
}

#[test]
fn registry_accepts_only_the_matching_caller_credential() {
    for workload in Workload::ALL {
        let token = match workload {
            Workload::Gateway => GATEWAY_TOKEN,
            Workload::Auth => AUTH_TOKEN,
            Workload::Post => POST_TOKEN,
            // Unregistered workloads have no accepted credential.
            Workload::Profile | Workload::Friend | Workload::Notification => continue,
        };
        let request = authenticated_request(workload, token);
        assert_eq!(
            request
                .extensions()
                .get::<VerifiedCaller>()
                .expect("caller")
                .0,
            workload
        );
    }
}

#[test]
fn one_service_credential_cannot_impersonate_another() {
    // The gateway credential claims to be auth. Each caller has its own secret,
    // so a credential stolen from one workload does not authenticate another.
    let mut server =
        RequireRegisteredCallerInterceptor::new(registry()).expect("server interceptor");
    for (claimed, token) in [
        (Workload::Auth, GATEWAY_TOKEN),
        (Workload::Gateway, POST_TOKEN),
        (Workload::Post, AUTH_TOKEN),
    ] {
        let mut client =
            ServiceIdentityClientInterceptor::new(claimed, token).expect("client identity");
        let error = server
            .call(client.call(Request::new(())).expect("request"))
            .expect_err("cross-identity call must fail");
        assert_eq!(error.code(), tonic::Code::Unauthenticated);
    }
}

#[test]
fn unregistered_or_malformed_identity_is_rejected() {
    let mut server =
        RequireRegisteredCallerInterceptor::new(registry()).expect("server interceptor");
    // Unknown caller name with a valid-looking token.
    let mut request = Request::new(());
    request.metadata_mut().insert(
        platform::grpc_auth::SERVICE_CALLER_HEADER,
        "unknown".parse().unwrap(),
    );
    request
        .metadata_mut()
        .insert("x-service-token", GATEWAY_TOKEN.parse().unwrap());
    assert_eq!(
        server.call(request).expect_err("unknown caller").code(),
        tonic::Code::Unauthenticated
    );

    // Caller name present but credential missing.
    let mut request = Request::new(());
    request.metadata_mut().insert(
        platform::grpc_auth::SERVICE_CALLER_HEADER,
        "gateway".parse().unwrap(),
    );
    assert_eq!(
        server.call(request).expect_err("missing token").code(),
        tonic::Code::Unauthenticated
    );
}

#[test]
fn registry_rejects_duplicate_and_empty_registrations() {
    assert!(CallerRegistry::new()
        .register(Workload::Gateway, GATEWAY_TOKEN)
        .expect("first")
        .register(Workload::Gateway, "another")
        .is_err());
    assert!(CallerRegistry::new()
        .register(Workload::Gateway, "   ")
        .is_err());
    assert!(RequireRegisteredCallerInterceptor::new(CallerRegistry::new()).is_err());
    assert!(CallerRegistry::new()
        .register_with_previous(Workload::Gateway, GATEWAY_TOKEN, Some(GATEWAY_TOKEN))
        .is_err());
    assert!(CallerRegistry::new()
        .register_with_previous(Workload::Gateway, GATEWAY_TOKEN, Some("   "))
        .is_err());
}

#[test]
fn credential_rotation_accepts_current_and_previous_until_previous_is_removed() {
    let rotating = CallerRegistry::new()
        .register_with_previous(Workload::Gateway, "gateway-new", Some("gateway-old"))
        .expect("overlapping credentials");
    let mut server =
        RequireRegisteredCallerInterceptor::new(rotating).expect("rotating interceptor");

    for token in ["gateway-new", "gateway-old"] {
        let mut client =
            ServiceIdentityClientInterceptor::new(Workload::Gateway, token).expect("identity");
        assert!(server
            .call(client.call(Request::new(())).expect("request"))
            .is_ok());
    }

    let mut retired_server = RequireRegisteredCallerInterceptor::new(
        CallerRegistry::new()
            .register(Workload::Gateway, "gateway-new")
            .expect("new credential only"),
    )
    .expect("retired interceptor");
    let mut retired_client =
        ServiceIdentityClientInterceptor::new(Workload::Gateway, "gateway-old")
            .expect("old identity");
    assert_eq!(
        retired_server
            .call(retired_client.call(Request::new(())).expect("request"))
            .expect_err("retired credential")
            .code(),
        tonic::Code::Unauthenticated
    );
}

#[test]
fn destructive_operations_require_the_designated_caller() {
    // Account deletion is the auth coordinator's job alone.
    assert!(CallerPolicy::allows(
        Workload::Auth,
        RpcAction::ApplyUserDeletion
    ));
    for caller in [Workload::Gateway, Workload::Post, Workload::Friend] {
        assert!(
            !CallerPolicy::allows(caller, RpcAction::ApplyUserDeletion),
            "{} must not erase accounts",
            caller.as_str()
        );
    }
    // Moderation is an operator action delivered from the edge.
    assert!(CallerPolicy::allows(
        Workload::Gateway,
        RpcAction::ApplyContentModeration
    ));
    assert!(!CallerPolicy::allows(
        Workload::Post,
        RpcAction::ApplyContentModeration
    ));

    // Only the authenticated edge may carry an administrator identity into
    // PostService for outbox dead-letter recovery.
    assert!(CallerPolicy::allows(
        Workload::Gateway,
        RpcAction::ResolveDeadLetterReplay
    ));
    for caller in [Workload::Auth, Workload::Post, Workload::Friend] {
        assert!(!CallerPolicy::allows(
            caller,
            RpcAction::ResolveDeadLetterReplay
        ));
    }

    // Only the authenticated edge may carry an administrator identity into
    // AuthService to restart a deletion saga.
    assert!(CallerPolicy::allows(
        Workload::Gateway,
        RpcAction::RetryUserDeletion
    ));
    for caller in [Workload::Auth, Workload::Post, Workload::Friend] {
        assert!(!CallerPolicy::allows(caller, RpcAction::RetryUserDeletion));
    }
    assert!(CallerPolicy::allows(
        Workload::Gateway,
        RpcAction::InspectUserDeletions
    ));
    for caller in [Workload::Auth, Workload::Post, Workload::Friend] {
        assert!(!CallerPolicy::allows(
            caller,
            RpcAction::InspectUserDeletions
        ));
    }

    // Mutation receipt IDs are not bearer credentials and are visible only to
    // the Gateway, which separately exposes the status through admin auth.
    assert!(CallerPolicy::allows(
        Workload::Gateway,
        RpcAction::MutationReceiptLookup
    ));
    for caller in [Workload::Auth, Workload::Post, Workload::Friend] {
        assert!(!CallerPolicy::allows(
            caller,
            RpcAction::MutationReceiptLookup
        ));
    }
}

#[test]
fn authorize_call_reports_missing_and_insufficient_identity_differently() {
    // No verified identity: unauthenticated.
    let error = authorize_call(&Request::new(()), RpcAction::ApplyUserDeletion)
        .expect_err("anonymous call");
    assert_eq!(error.code(), tonic::Code::Unauthenticated);

    // Verified identity without authority: permission denied, so operators can
    // distinguish a misconfigured peer from a missing credential.
    let mut request = Request::new(());
    request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Post));
    let error =
        authorize_call(&request, RpcAction::ApplyUserDeletion).expect_err("unauthorized call");
    assert_eq!(error.code(), tonic::Code::PermissionDenied);

    let mut request = Request::new(());
    request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Auth));
    assert_eq!(
        authorize_call(&request, RpcAction::ApplyUserDeletion).expect("authorized"),
        Workload::Auth
    );
}
