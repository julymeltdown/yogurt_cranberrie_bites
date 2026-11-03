//! Cross-service default-deny matrix for every served gRPC method.
//!
//! The mutual-TLS TCP matrix in crates/platform/tests/grpc_tls.rs proves that
//! the guard admits a call exactly when MethodPolicy::allows does, with zero
//! service effects on denial. This test closes the coverage half: every
//! Proto service has exactly one method table, every table lists exactly its
//! Proto methods, and for every method and every workload the decision equals
//! the declared caller list. A new Proto service or method without a table
//! entry fails here. No database, network or process is involved.
use platform::grpc_auth::Workload;
use platform::grpc_method_policy::MethodPolicy;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

fn policies() -> Vec<&'static MethodPolicy> {
    use api_gateway::adapters::r#in::grpc as gateway;
    use friend_service::adapters::r#in::grpc as friend;
    use post_service::adapters::r#in::grpc as post;
    use profile_service::adapters::r#in::grpc as profile;
    vec![
        &auth_service::adapters::r#in::grpc::method_policy::AUTH_METHOD_POLICY,
        &notification_service::adapters::r#in::grpc::method_policy::NOTIFICATION_METHOD_POLICY,
        &profile::method_policy::PROFILE_METHOD_POLICY,
        &profile::author_authority::PROFILE_AUTHOR_POLICY,
        &friend::method_policy::FRIEND_METHOD_POLICY,
        &friend::cache_projection::CACHE_PROJECTION_POLICY,
        &friend::fanout_projection::FANOUT_PROJECTION_POLICY,
        &friend::notification_authority::NOTIFICATION_POLICY_POLICY,
        &friend::notification_authority::NOTIFICATION_PROJECTION_POLICY,
        &post::method_policy::POST_METHOD_POLICY,
        &post::cache_authority::CACHE_OWNER_POLICY,
        &post::fanout_authority::FANOUT_OWNER_POLICY,
        &post::notification_authority::NOTIFICATION_OWNER_POLICY,
        &post::resource_authority::POST_RESOURCE_AUTHORITY_POLICY,
        &gateway::authority_exchange::AUTHORIZATION_EXCHANGE_POLICY,
        &gateway::cache_authority::CACHE_GRANT_POLICY,
        &gateway::fanout_authority::FANOUT_GRANT_POLICY,
        &gateway::notification_authority::NOTIFICATION_GRANT_POLICY,
    ]
}

/// Remove // and /* */ comments so commented-out declarations do not count.
fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'/') {
            for next in chars.by_ref() {
                if next == '\n' {
                    out.push('\n');
                    break;
                }
            }
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut previous = '\0';
            for next in chars.by_ref() {
                if previous == '*' && next == '/' {
                    break;
                }
                previous = next;
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Fully qualified service name to its rpc names, for every .proto file.
fn proto_services(root: &Path) -> BTreeMap<String, BTreeSet<String>> {
    let mut services = BTreeMap::new();
    let mut files: Vec<_> = std::fs::read_dir(root)
        .expect("proto root")
        .flat_map(|package| std::fs::read_dir(package.expect("proto package").path().join("v1")))
        .flatten()
        .map(|entry| entry.expect("proto entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "proto"))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no Proto files found under {}",
        root.display()
    );
    for file in files {
        let source = strip_comments(&std::fs::read_to_string(&file).expect("proto source"));
        let package = source
            .split("package ")
            .nth(1)
            .and_then(|rest| rest.split(';').next())
            .map(str::trim)
            .unwrap_or_else(|| panic!("{} declares no package", file.display()))
            .to_owned();
        for block in source.split("service ").skip(1) {
            let name = block.split_whitespace().next().expect("service name");
            let body = block.split_once('{').expect("service body").1;
            let mut depth = 1usize;
            let mut end = body.len();
            for (index, c) in body.char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = index;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let rpcs: BTreeSet<String> = body[..end]
                .split("rpc ")
                .skip(1)
                .map(|rpc| {
                    rpc.split(|c: char| c == '(' || c.is_whitespace())
                        .next()
                        .unwrap()
                        .to_owned()
                })
                .collect();
            let qualified = format!("{package}.{name}");
            assert!(!rpcs.is_empty(), "{qualified} declares no rpc");
            assert!(
                services.insert(qualified.clone(), rpcs).is_none(),
                "duplicate Proto service {qualified}"
            );
        }
    }
    services
}

#[test]
fn every_served_method_is_default_deny_for_every_undeclared_workload() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let proto = proto_services(&workspace.join("proto"));
    let policies = policies();

    let mut by_service = BTreeMap::new();
    for policy in &policies {
        assert!(
            by_service.insert(policy.service, *policy).is_none(),
            "{} has more than one method table",
            policy.service
        );
    }
    let served: BTreeSet<_> = by_service.keys().map(|name| name.to_string()).collect();
    let declared: BTreeSet<_> = proto.keys().cloned().collect();
    assert_eq!(
        served, declared,
        "Proto services and method tables must correspond one to one"
    );

    let (mut methods, mut allowed, mut denied, mut actor_bound, mut uncallable) = (0, 0, 0, 0, 0);
    for (service, rpcs) in &proto {
        let policy = by_service[service.as_str()];
        let listed: BTreeSet<_> = policy.method_names().map(str::to_owned).collect();
        assert_eq!(
            &listed, rpcs,
            "{service}: method table must list exactly its Proto rpcs"
        );
        for method in rpcs {
            methods += 1;
            let path = format!("/{service}/{method}");
            let callers = policy.callers(method).expect("listed method");
            if callers.is_empty() {
                uncallable += 1;
            }
            for workload in Workload::ALL {
                let expected = callers.contains(&workload);
                assert_eq!(
                    policy.allows(&path, workload),
                    expected,
                    "{path} for {workload:?} must be {}",
                    if expected { "allowed" } else { "denied" }
                );
                if expected {
                    allowed += 1;
                } else {
                    denied += 1;
                }
                for confused in [
                    format!("/{service}/{method}X"),
                    format!("/{service}X/{method}"),
                    format!("{service}/{method}"),
                    format!("/{service}/{method}/"),
                    format!("/other.v1.Service/{method}"),
                ] {
                    assert!(
                        !policy.allows(&confused, workload),
                        "{confused} must deny {workload:?}"
                    );
                }
            }
            if policy.actor_fields_for(&path).is_some() {
                actor_bound += 1;
                assert_eq!(
                    callers,
                    &[Workload::Gateway],
                    "{path} acts for a user and must be Gateway-only"
                );
            }
        }
        for (actor_method, _) in policy.actor_fields {
            assert!(
                rpcs.contains(*actor_method),
                "{service}: actor fields name unknown method {actor_method}"
            );
        }
        assert!(!policy.allows(&format!("/{service}/NotARealMethod"), Workload::Gateway));
    }
    assert!(methods > 0 && denied > 0);
    eprintln!(
        "SECURITY_METHOD_MATRIX {{\"services\":{},\"methods\":{methods},\"allowedPairs\":{allowed},\"deniedPairs\":{denied},\"actorBound\":{actor_bound},\"uncallable\":{uncallable}}}",
        proto.len()
    );
}

/// How a production binary wraps one policy table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GuardKind {
    Guard,
    WithForwarding,
    WithoutDelegation,
}

/// One guarded service a production binary actually serves.
#[derive(Clone, Debug, PartialEq, Eq)]
struct GuardCall {
    kind: GuardKind,
    policy_static: String,
    forwarding_static: Option<String>,
}

/// Production guard registration of one served policy table.
struct Registration {
    kind: GuardKind,
    forwarding: &'static [platform::grpc_method_policy::ForwardedMethod],
}

/// The Rust value of a forwarding static named in a production guard call.
/// The call site, not this table, decides which policy the value guards; an
/// unknown name fails so a new table cannot be served unexamined.
fn forwarding_static(name: &str) -> &'static [platform::grpc_method_policy::ForwardedMethod] {
    match name {
        "FRIEND_FORWARDING" => {
            friend_service::adapters::r#in::grpc::method_policy::FRIEND_FORWARDING
        }
        "PROFILE_AUTHOR_FORWARDING" => {
            profile_service::adapters::r#in::grpc::author_authority::PROFILE_AUTHOR_FORWARDING
        }
        other => panic!("production guard passes unknown forwarding table {other}"),
    }
}

/// The text between the parenthesis at `open` and its matching close.
fn balanced(source: &str, open: usize) -> &str {
    assert_eq!(source.as_bytes()[open], b'(');
    let mut depth = 0usize;
    for (offset, byte) in source.as_bytes()[open..].iter().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return &source[open + 1..open + offset];
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced call in production wiring")
}

/// Top-level comma-separated arguments of a call.
fn arguments(call: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    for c in call.chars() {
        match c {
            '(' | '[' | '{' | '<' => depth += 1,
            ')' | ']' | '}' | '>' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(current.trim().to_owned());
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_owned());
    }
    parts
}

fn last_segment(path: &str) -> String {
    path.trim()
        .trim_start_matches('&')
        .rsplit("::")
        .next()
        .expect("path")
        .trim()
        .to_owned()
}

const GUARD_PREFIX: &str = "platform::grpc_method_policy::";

/// Offset of the first guard function call in `text`, skipping other items
/// of the module such as the `GuardedService` return type.
fn find_guard_call(text: &str) -> Option<usize> {
    text.match_indices(GUARD_PREFIX)
        .map(|(index, _)| index)
        .find(|index| {
            let rest = &text[index + GUARD_PREFIX.len()..];
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            name.starts_with("guard") && rest[name.len()..].trim_start().starts_with('(')
        })
}

/// Parses the one guard call that starts at `text`.
fn guard_call(text: &str) -> GuardCall {
    let rest = text
        .trim_start()
        .strip_prefix(GUARD_PREFIX)
        .unwrap_or_else(|| panic!("not a guard call: {text}"));
    let name_end = rest.find('(').expect("guard call arguments");
    let kind = match rest[..name_end].trim() {
        "guard" => GuardKind::Guard,
        "guard_with_forwarding" => GuardKind::WithForwarding,
        "guard_without_delegation" => GuardKind::WithoutDelegation,
        other => panic!("unknown production guard {other}"),
    };
    let args = arguments(balanced(rest, name_end));
    GuardCall {
        kind,
        policy_static: last_segment(&args[0]),
        forwarding_static: (kind == GuardKind::WithForwarding).then(|| last_segment(&args[1])),
    }
}

/// Every guarded service registered with `.add_service(...)` in `main`.
/// Health services are skipped; a bare variable is followed to its `let` and
/// the helper function it calls in `helpers`. Any other form fails, so a new
/// registration style cannot silently drop out of the inventory.
fn guard_calls(main: &str, helpers: &str) -> Vec<GuardCall> {
    let main = strip_comments(main);
    let helpers = strip_comments(helpers);
    let mut calls = Vec::new();
    for (index, _) in main.match_indices(".add_service(") {
        let open = index + ".add_service".len();
        let argument = balanced(&main, open).trim();
        if argument.contains(".grpc_service(") {
            continue;
        }
        if find_guard_call(argument) == Some(0) {
            calls.push(guard_call(argument));
            continue;
        }
        assert!(
            argument
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "unrecognised production registration: {argument}"
        );
        let binding = format!("let {argument} =");
        let expression = main
            .split(binding.as_str())
            .nth(1)
            .and_then(|tail| tail.split(';').next())
            .unwrap_or_else(|| panic!("no binding for registered service {argument}"));
        if let Some(position) = find_guard_call(expression) {
            calls.push(guard_call(&expression[position..]));
            continue;
        }
        let callee = last_segment(expression.split('(').next().expect("helper call"));
        let body = helpers
            .split(format!("pub fn {callee}(").as_str())
            .nth(1)
            .and_then(|tail| tail.split("\npub ").next())
            .unwrap_or_else(|| panic!("no helper {callee} for registered service {argument}"));
        let position = find_guard_call(body)
            .unwrap_or_else(|| panic!("helper {callee} registers no guarded service"));
        calls.push(guard_call(&body[position..]));
    }
    calls
}

/// Production main.rs files and the source their registrations may call.
const PRODUCTION_BINARIES: [(&str, &str); 6] = [
    ("services/auth-service/src/main.rs", ""),
    ("services/notification-service/src/main.rs", ""),
    ("services/profile-service/src/main.rs", ""),
    ("services/friend-service/src/main.rs", ""),
    ("services/post-service/src/main.rs", ""),
    (
        "services/api-gateway/src/main.rs",
        "services/api-gateway/src/config/mod.rs",
    ),
];

/// Service name to how a production binary guards it. A table absent here is
/// compiled and tested but not served.
fn production_registrations(workspace: &Path) -> BTreeMap<String, Registration> {
    let read = |path: &str| std::fs::read_to_string(workspace.join(path)).expect(path);
    let services_by_static: BTreeMap<String, String> = policy_statics(workspace)
        .into_iter()
        .map(|(service, name)| (name, service))
        .collect();
    let mut registrations = BTreeMap::new();
    for (main, helpers) in PRODUCTION_BINARIES {
        let helpers = if helpers.is_empty() {
            String::new()
        } else {
            read(helpers)
        };
        for call in guard_calls(&read(main), &helpers) {
            let service = services_by_static
                .get(&call.policy_static)
                .unwrap_or_else(|| panic!("{main} guards unknown policy {}", call.policy_static))
                .clone();
            let registration = Registration {
                kind: call.kind,
                forwarding: call
                    .forwarding_static
                    .as_deref()
                    .map(forwarding_static)
                    .unwrap_or(&[]),
            };
            assert!(
                registrations
                    .insert(service.clone(), registration)
                    .is_none(),
                "{service} is registered more than once"
            );
        }
    }
    registrations
}

#[test]
fn guard_registration_parser_follows_actual_wiring() {
    let forwarded = r#"builder
        .add_service(readiness.grpc_service(registry.clone())?)
        .add_service(platform::grpc_method_policy::guard_with_forwarding(
            &a::FRIEND_METHOD_POLICY, a::FRIEND_FORWARDING, Server::new(state), r, d,
        )?)"#;
    assert_eq!(
        guard_calls(forwarded, ""),
        vec![GuardCall {
            kind: GuardKind::WithForwarding,
            policy_static: "FRIEND_METHOD_POLICY".into(),
            forwarding_static: Some("FRIEND_FORWARDING".into()),
        }]
    );
    // Swapping in the plain guard drops child-proof enforcement; the parsed
    // registration must lose its forwarding table rather than keep it.
    let plain = forwarded
        .replace("guard_with_forwarding", "guard")
        .replace(" a::FRIEND_FORWARDING,", "");
    assert_eq!(guard_calls(&plain, "")[0].forwarding_static, None);
    assert_eq!(guard_calls(&plain, "")[0].kind, GuardKind::Guard);
    // A service built in a helper counts only while main still adds it.
    let main = "let exchange_service =\n    api::config::exchange_service(&config)?;\nbuilder.add_service(exchange_service)";
    let helpers = "pub fn exchange_service(config: &C) -> anyhow::Result<platform::grpc_method_policy::GuardedService<S>> {\n    platform::grpc_method_policy::guard_without_delegation(&EXCHANGE_POLICY, S::new(), r)\n}\npub fn other() {}";
    assert_eq!(
        guard_calls(main, helpers)[0].policy_static,
        "EXCHANGE_POLICY"
    );
    let removed = main.replace("builder.add_service(exchange_service)", "builder");
    assert!(guard_calls(&removed, helpers).is_empty());
    let commented = main.replace("builder.add_service", "builder // .add_service");
    assert!(guard_calls(&commented, helpers).is_empty());
    assert!(std::panic::catch_unwind(|| guard_calls("b.add_service(make())", "")).is_err());
}

/// Service name to the static that declares its MethodPolicy, from source.
fn policy_statics(workspace: &Path) -> BTreeMap<String, String> {
    fn walk(directory: &Path, files: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(directory).expect("source directory") {
            let path = entry.expect("source entry").path();
            if path.is_dir() {
                walk(&path, files);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    let mut files = Vec::new();
    for service in std::fs::read_dir(workspace.join("services")).expect("services") {
        let source = service.expect("service").path().join("src");
        if source.is_dir() {
            walk(&source, &mut files);
        }
    }
    let mut statics = BTreeMap::new();
    for file in files {
        let source = strip_comments(&std::fs::read_to_string(&file).expect("source"));
        for declaration in source.split("pub static ").skip(1) {
            let Some((name, rest)) = declaration.split_once(':') else {
                continue;
            };
            let Some(body) = rest
                .trim_start()
                .strip_prefix("MethodPolicy = MethodPolicy {")
            else {
                continue;
            };
            let service = body
                .split("service: \"")
                .nth(1)
                .and_then(|tail| tail.split('"').next())
                .expect("policy service");
            assert!(
                statics
                    .insert(service.to_owned(), name.trim().to_owned())
                    .is_none(),
                "{service} is declared by more than one static"
            );
        }
    }
    statics
}

/// The subject binding the guard enforces before any handler runs. For a
/// served table it follows the production guard call; for a table that only
/// tests compose it follows the policy's own actor fields.
fn subject_binding(
    policy: &MethodPolicy,
    method: &str,
    registration: Option<&Registration>,
) -> String {
    let path = format!("/{}/{method}", policy.service);
    let delegated =
        registration.is_none_or(|registration| registration.kind != GuardKind::WithoutDelegation);
    let mut parts = Vec::new();
    if let Some(fields) = policy.actor_fields_for(&path) {
        assert!(
            delegated,
            "{path}: actor fields on a guard without delegation"
        );
        parts.push(format!("gateway actor delegation; actor fields {fields:?}"));
    }
    if let Some(registration) = registration {
        parts.extend(
            registration
                .forwarding
                .iter()
                .filter(|entry| entry.method == method)
                .map(|entry| {
                    format!(
                        "{} child authority issued by the Gateway; actor fields {:?}",
                        entry.caller.as_str(),
                        entry.actor_fields
                    )
                }),
        );
    }
    if parts.is_empty() {
        "workload identity only".to_owned()
    } else {
        parts.join("; ")
    }
}

#[derive(PartialEq, Debug, Clone)]
struct InventoryRow {
    method: String,
    callers: Vec<String>,
    subject: String,
    production: bool,
    resource: String,
    reason: String,
}

impl InventoryRow {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "method": self.method,
            "callers": self.callers,
            "subject": self.subject,
            "production": self.production,
            "resource": self.resource,
            "reason": self.reason,
        })
    }

    fn from_json(value: &serde_json::Value) -> Self {
        let text = |name: &str| {
            value[name]
                .as_str()
                .unwrap_or_else(|| panic!("inventory field {name} must be a string: {value}"))
                .to_owned()
        };
        Self {
            method: text("method"),
            callers: value["callers"]
                .as_array()
                .expect("callers array")
                .iter()
                .map(|caller| caller.as_str().expect("caller").to_owned())
                .collect(),
            subject: text("subject"),
            production: value["production"].as_bool().expect("production flag"),
            resource: text("resource"),
            reason: text("reason"),
        }
    }
}

fn derived_rows(workspace: &Path) -> Vec<InventoryRow> {
    let registered = production_registrations(workspace);
    let mut rows = Vec::new();
    for policy in policies() {
        for method in policy.method_names() {
            rows.push(InventoryRow {
                method: format!("/{}/{method}", policy.service),
                callers: policy
                    .callers(method)
                    .expect("listed")
                    .iter()
                    .map(|workload| workload.as_str().to_owned())
                    .collect(),
                subject: subject_binding(policy, method, registered.get(policy.service)),
                production: registered.contains_key(policy.service),
                resource: String::new(),
                reason: String::new(),
            });
        }
    }
    rows.sort_by(|left, right| left.method.cmp(&right.method));
    rows
}

const INVENTORY: &str = "docs/security/rpc-authority-inventory.json";

#[test]
fn every_method_has_a_current_authority_inventory_row_with_a_reason() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let derived = derived_rows(&workspace);
    if std::env::var_os("WRITE_RPC_AUTHORITY_SKELETON").is_some() {
        let path = workspace.join("target/rpc-authority-skeleton.json");
        let rows: Vec<_> = derived.iter().map(InventoryRow::to_json).collect();
        std::fs::write(&path, serde_json::to_string_pretty(&rows).unwrap()).unwrap();
        eprintln!("wrote {}", path.display());
        return;
    }
    let document: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(workspace.join(INVENTORY)).expect(INVENTORY))
            .expect("inventory JSON");
    assert_eq!(document["version"], 1);
    let rows: Vec<InventoryRow> = document["methods"]
        .as_array()
        .expect("inventory rows")
        .iter()
        .map(InventoryRow::from_json)
        .collect();
    let by_method: BTreeMap<_, _> = rows.iter().map(|row| (row.method.clone(), row)).collect();
    assert_eq!(by_method.len(), rows.len(), "duplicate inventory rows");
    let expected: BTreeSet<_> = derived.iter().map(|row| row.method.clone()).collect();
    let listed: BTreeSet<_> = by_method.keys().cloned().collect();
    assert_eq!(
        listed, expected,
        "inventory must list exactly every served Proto method"
    );
    for derived in &derived {
        let row = by_method[&derived.method];
        assert_eq!(
            row.callers, derived.callers,
            "{}: callers drifted from source",
            row.method
        );
        assert_eq!(
            row.subject, derived.subject,
            "{}: subject binding drifted",
            row.method
        );
        assert_eq!(
            row.production, derived.production,
            "{}: registration drifted",
            row.method
        );
        assert!(
            row.resource.trim().len() >= 12,
            "{}: resource check missing",
            row.method
        );
        assert!(
            row.reason.trim().len() >= 12,
            "{}: allow reason missing",
            row.method
        );
        if row.callers.is_empty() {
            assert!(
                row.reason.to_lowercase().contains("no caller"),
                "{}",
                row.method
            );
        }
    }
    eprintln!(
        "RPC_AUTHORITY_INVENTORY {{\"methods\":{},\"production\":{}}}",
        rows.len(),
        rows.iter().filter(|row| row.production).count()
    );
}
