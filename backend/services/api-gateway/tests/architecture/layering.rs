#[test]
fn domain_and_application_are_framework_and_persistence_free() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for layer in ["domain", "application"] {
        assert_no_forbidden_boundary_references(&src.join(layer));
        assert_no_forbidden_boundary_references(
            &src.join("../../../crates")
                .join(format!("gateway-{layer}"))
                .join("src"),
        );
    }
    assert_no_application_adapter_references(&src.join("application"));
    assert_no_application_adapter_references(&src.join("../../../crates/gateway-application/src"));
}

#[test]
fn governance_routes_delegate_through_owned_services_and_single_repository_ports() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let moderation_routes = read_source(&src.join("adapters/in/http/moderation_routes.rs"));
    let admin_routes = read_source(&src.join("adapters/in/http/admin_routes.rs"));
    let moderation_service = read_source(
        &src.join("../../../crates/gateway-application/src/service/moderation_service.rs"),
    );
    let policy_service =
        read_source(&src.join("../../../crates/gateway-application/src/service/policy_service.rs"));

    assert!(moderation_routes.contains(".moderation_service"));
    assert!(admin_routes.contains(".policy_service"));
    for (path, source, forbidden) in [
        (
            "moderation routes",
            moderation_routes.as_str(),
            &[
                "sqlx::query",
                "sqlx::query_as",
                "PgModerationRepository",
                "ModerationRepositoryPort",
            ][..],
        ),
        (
            "policy routes",
            admin_routes.as_str(),
            &[
                "PgPolicyRepository",
                "PolicyRepositoryPort",
                "RolloutRepositoryPort",
            ][..],
        ),
        (
            "moderation service",
            moderation_service.as_str(),
            &["sqlx::", "crate::adapters", "PgModerationRepository"][..],
        ),
        (
            "policy service",
            policy_service.as_str(),
            &["sqlx::", "crate::adapters", "PgPolicyRepository"][..],
        ),
    ] {
        for pattern in forbidden {
            assert!(
                !source.contains(pattern),
                "{path} must not bypass its application/repository boundary through `{pattern}`"
            );
        }
    }

    assert!(moderation_service.contains("ModerationRepositoryPort"));
    assert!(policy_service.contains("PolicyRepositoryPort"));
    assert!(policy_service.contains("RolloutRepositoryPort"));

    let repositories = src.join("adapters/out/repository");
    for port in [
        "ModerationRepositoryPort",
        "PolicyRepositoryPort",
        "RolloutRepositoryPort",
    ] {
        let implementations = rust_files_under(&repositories)
            .into_iter()
            .map(|path| read_source(&path))
            .map(|source| source.matches(&format!("impl {port} for")).count())
            .sum::<usize>();
        assert_eq!(
            implementations, 1,
            "{port} must have exactly one in-process persistence implementation"
        );
    }
}

const FORBIDDEN_BOUNDARY_PATTERNS: &[&str] = &[
    "use axum",
    "axum::",
    "use tonic",
    "tonic::",
    "use tokio",
    "tokio::",
    "use sqlx",
    "sqlx::",
    "platform::",
];

#[test]
fn incoming_adapters_cannot_import_sql_drivers_or_outgoing_implementations() {
    let incoming = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/adapters/in");
    let files = rust_files_under(&incoming);
    assert!(!files.is_empty(), "incoming architecture scope must exist");
    for path in files {
        assert!(
            !bypasses_incoming_repository_boundary(&read_source(&path)),
            "{} bypasses the application repository boundary",
            path.display()
        );
    }
}

// Conservative source gate: opaque PgPool compatibility parameters remain,
// but other SQLx imports/aliases and explicit outgoing references are forbidden.
// This is not an exhaustive interprocedural physical-effect analysis.
fn bypasses_incoming_repository_boundary(source: &str) -> bool {
    let compact: String = source.chars().filter(|c| !c.is_whitespace()).collect();
    // Accept only bare imports and qualified parameter/field types. Reject
    // qualified constructors, import renames and type aliases before removing
    // the SQLx name; otherwise the exception could hide direct connection I/O.
    for (offset, _) in compact.match_indices("sqlx::PgPool") {
        let before = &compact[..offset];
        let after = &compact[offset + "sqlx::PgPool".len()..];
        let type_position = before.ends_with(':') || before.ends_with('&');
        let bare_import = before.ends_with("use") && after.starts_with(';');
        if (!type_position && !bare_import) || after.starts_with("::") || after.starts_with("as") {
            return true;
        }
    }
    let without_pool_types = compact.replace("sqlx::PgPool", "");
    without_pool_types.contains("sqlx")
        || compact.contains("PgPool::")
        || compact.contains("=PgPool")
        || [
            ".acquire(",
            ".begin(",
            ".execute(",
            ".fetch_one(",
            ".fetch_all(",
            ".fetch_optional(",
        ]
        .iter()
        .any(|method| compact.contains(method))
        || compact.contains("adapters::out")
        || compact.contains("adapters::r#out")
        || compact.contains("acquire_database_connection")
}

#[test]
fn incoming_gate_rejects_query_import_alias_and_whitespace_bypasses() {
    for source in [
        "sqlx::query(\"insert\")",
        "sqlx :: query_as::<_, Row>(\"select\")",
        "use sqlx::query as read; read(\"select\")",
        "use sqlx as database; database::query(\"select\")",
        "use crate :: adapters :: out :: repository :: ConcreteRepository;",
        "use crate::adapters::r#out::repository::ConcreteRepository;",
        "config::acquire_database_connection(pool).await",
        "sqlx::PgPool::connect(url).await",
        "use sqlx::PgPool; PgPool::connect(url).await",
        "use sqlx::PgPool as Database; Database::connect(url).await",
        "type Database = sqlx::PgPool; Database::connect(url).await",
        "use sqlx::PgPool; type Database = PgPool; Database::connect(url).await",
        "fn worker(pool: &sqlx::PgPool) { pool.acquire().await; }",
        "fn worker(pool: &sqlx::PgPool) { pool.begin().await; }",
    ] {
        assert!(bypasses_incoming_repository_boundary(source), "{source}");
    }
    assert!(!bypasses_incoming_repository_boundary(
        "fn worker(pool: &sqlx :: PgPool) { state.repository.record(command); }"
    ));
}

const FORBIDDEN_APPLICATION_PATTERNS: &[&str] = &["crate::adapters", "super::adapters"];

fn assert_no_forbidden_boundary_references(root: &std::path::Path) {
    assert!(
        root.is_dir(),
        "expected architecture root `{}` to exist",
        root.display()
    );

    let files = rust_files_under(root);
    assert!(
        !files.is_empty(),
        "expected architecture root `{}` to contain at least one Rust file",
        root.display()
    );

    for path in files {
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read `{}`: {}", path.display(), error));

        for pattern in FORBIDDEN_BOUNDARY_PATTERNS {
            assert!(
                !content.contains(pattern),
                "`{}` contains forbidden boundary pattern `{}`",
                path.display(),
                pattern
            );
        }
    }
}

fn assert_no_application_adapter_references(root: &std::path::Path) {
    for path in rust_files_under(root) {
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read `{}`: {}", path.display(), error));

        for pattern in FORBIDDEN_APPLICATION_PATTERNS {
            assert!(
                !content.contains(pattern),
                "`{}` contains forbidden application adapter pattern `{}`",
                path.display(),
                pattern
            );
        }
    }
}

fn rust_files_under(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files);
    files.sort();
    files
}

fn collect_rust_files(directory: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
    let entries = std::fs::read_dir(directory).unwrap_or_else(|error| {
        panic!(
            "failed to read directory `{}`: {}",
            directory.display(),
            error
        )
    });

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!(
                "failed to read directory entry in `{}`: {}",
                directory.display(),
                error
            )
        });
        let path = entry.path();
        let file_type = entry.file_type().unwrap_or_else(|error| {
            panic!(
                "failed to read metadata for `{}`: {}",
                path.display(),
                error
            )
        });

        if file_type.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

fn read_source(path: &std::path::Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read `{}`: {}", path.display(), error))
}
