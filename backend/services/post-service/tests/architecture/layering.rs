#[test]
fn domain_and_application_are_framework_and_persistence_free() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for layer in ["domain", "application"] {
        assert_no_forbidden_boundary_references(&src.join(layer));
        assert_no_forbidden_boundary_references(
            &src.join(format!("../../../crates/post-{layer}/src")),
        );
    }
}

const FORBIDDEN_BOUNDARY_PATTERNS: &[&str] = &[
    "platform::",
    "use axum",
    "axum::",
    "use tonic",
    "tonic::",
    "use tokio",
    "tokio::",
    "use sqlx",
    "sqlx::",
];

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
