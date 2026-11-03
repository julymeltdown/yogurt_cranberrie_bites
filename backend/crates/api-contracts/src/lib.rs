//! Fixed checked-in OpenAPI bundling shared by the Gateway runtime and the
//! repository contract tests.
//!
//! The checked-in `docs/api/openapi.json` is an index whose path items and
//! components are local aliases into `docs/api/openapi-core.json`. Runtime
//! serving and the tests must resolve those aliases with identical rules so a
//! contract assertion cannot pass against a differently assembled document.
//! Remote references are never fetched.

use serde_json::Value;
use std::sync::OnceLock;

/// Marker describing why a fixed local reference could not be resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleError {
    InvalidIndex,
    InvalidCore,
    MissingPaths,
    MissingComponents,
    UnsupportedAlias,
    MissingTarget,
    InvalidTarget,
    UnresolvedReference,
    SerializationFailed,
}

impl BundleError {
    pub fn as_str(self) -> &'static str {
        match self {
            BundleError::InvalidIndex => "invalid OpenAPI index",
            BundleError::InvalidCore => "invalid OpenAPI core",
            BundleError::MissingPaths => "missing OpenAPI paths",
            BundleError::MissingComponents => "missing OpenAPI components",
            BundleError::UnsupportedAlias => "unsupported OpenAPI alias",
            BundleError::MissingTarget => "missing OpenAPI alias target",
            BundleError::InvalidTarget => "recursive or invalid OpenAPI alias target",
            BundleError::UnresolvedReference => "missing internal OpenAPI reference",
            BundleError::SerializationFailed => "OpenAPI serialization failed",
        }
    }
}

fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// Resolve one `./openapi-core.json#<pointer>` alias in place.
pub fn resolve_alias(value: &mut Value, core: &Value, pointer: &str) -> Result<(), BundleError> {
    if value.get("$ref").is_none() {
        return Ok(());
    }
    let expected = format!("./openapi-core.json#{pointer}");
    if value.get("$ref").and_then(Value::as_str) != Some(expected.as_str())
        || value.as_object().is_none_or(|object| object.len() != 1)
    {
        return Err(BundleError::UnsupportedAlias);
    }
    let target = core.pointer(pointer).ok_or(BundleError::MissingTarget)?;
    if !target.is_object() || target.get("$ref").is_some() {
        return Err(BundleError::InvalidTarget);
    }
    *value = target.clone();
    Ok(())
}

/// Reject every reference that is not a resolvable internal `#/` pointer.
pub fn validate_references(value: &Value, root: &Value) -> Result<(), BundleError> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if key == "$ref" {
                    let reference = child.as_str().ok_or(BundleError::UnresolvedReference)?;
                    let pointer = reference
                        .strip_prefix('#')
                        .ok_or(BundleError::UnresolvedReference)?;
                    if !pointer.starts_with('/') || root.pointer(pointer).is_none() {
                        return Err(BundleError::UnresolvedReference);
                    }
                } else {
                    validate_references(child, root)?;
                }
            }
        }
        Value::Array(values) => {
            for child in values {
                validate_references(child, root)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Resolve every checked-in alias and return the assembled document.
pub fn bundle(index_text: &str, core_text: &str) -> Result<String, BundleError> {
    let mut index: Value =
        serde_json::from_str(index_text).map_err(|_| BundleError::InvalidIndex)?;
    let core: Value = serde_json::from_str(core_text).map_err(|_| BundleError::InvalidCore)?;
    let paths = index
        .get_mut("paths")
        .and_then(Value::as_object_mut)
        .ok_or(BundleError::MissingPaths)?;
    for (path, value) in paths {
        resolve_alias(value, &core, &format!("/paths/{}", escape(path)))?;
    }
    let components = index
        .get_mut("components")
        .and_then(Value::as_object_mut)
        .ok_or(BundleError::MissingComponents)?;
    for (category, entries) in components {
        let entries = entries
            .as_object_mut()
            .ok_or(BundleError::MissingComponents)?;
        for (name, value) in entries {
            resolve_alias(
                value,
                &core,
                &format!("/components/{}/{}", escape(category), escape(name)),
            )?;
        }
    }
    validate_references(&index, &index)?;
    serde_json::to_string(&index).map_err(|_| BundleError::SerializationFailed)
}

/// Cache one bundled document for the process lifetime.
pub fn bundle_once(index_text: &str, core_text: &str) -> Result<&'static str, BundleError> {
    static BUNDLE: OnceLock<Result<String, BundleError>> = OnceLock::new();
    BUNDLE
        .get_or_init(|| bundle(index_text, core_text))
        .as_ref()
        .map(String::as_str)
        .map_err(|error| *error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository_documents() -> (String, String) {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/api");
        (
            std::fs::read_to_string(format!("{root}/openapi.json")).expect("index"),
            std::fs::read_to_string(format!("{root}/openapi-core.json")).expect("core"),
        )
    }

    #[test]
    fn checked_in_documents_bundle_without_external_references() {
        let (index, core) = repository_documents();
        let document: Value = serde_json::from_str(&bundle(&index, &core).unwrap()).unwrap();
        let core_document: Value = serde_json::from_str(&core).unwrap();
        for (path, value) in core_document["paths"].as_object().unwrap() {
            assert_eq!(
                &document["paths"][path], value,
                "path {path} must match core"
            );
        }
        validate_references(&document, &document).unwrap();
    }

    #[test]
    fn unresolvable_aliases_are_rejected() {
        let core = serde_json::json!({"paths": {"/good": {"get": {}}}});
        for reference in [
            "https://untrusted.invalid/schema",
            "./openapi-core.json#/paths/~1other",
            "#/paths/~1good",
        ] {
            let mut value = serde_json::json!({"$ref": reference});
            assert!(resolve_alias(&mut value, &core, "/paths/~1good").is_err());
        }
        let mut value = serde_json::json!({"$ref":"./openapi-core.json#/paths/~1missing"});
        assert!(resolve_alias(&mut value, &core, "/paths/~1missing").is_err());
    }
}
