//! Bundle the fixed checked-in local index. Never fetch arbitrary references.
//!
//! The resolver is shared with the repository contract tests through
//! `api-contracts` so the served document and the asserted document cannot
//! drift into different assemblies of the same checked-in sources.
const INDEX: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/api/openapi.json"
));
const CORE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/api/openapi-core.json"
));

pub(super) fn bundled_contract() -> Result<&'static str, &'static str> {
    api_contracts::bundle_once(INDEX, CORE).map_err(api_contracts::BundleError::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn embedded_bundle_preserves_core_and_adds_auth_resend() {
        let document: Value = serde_json::from_str(bundled_contract().unwrap()).unwrap();
        let core: Value = serde_json::from_str(CORE).unwrap();
        for (path, value) in core["paths"].as_object().unwrap() {
            assert_eq!(&document["paths"][path], value);
        }
        let count: usize = document["paths"]
            .as_object()
            .unwrap()
            .values()
            .map(|item| item.as_object().unwrap().len())
            .sum();
        assert_eq!(count, 92);
        assert_eq!(
            document["paths"]["/auth/email/resend"]["post"]["security"],
            serde_json::json!([])
        );
        assert!(document["paths"]["/auth/email/resend"]["post"]["responses"]["202"].is_object());
        api_contracts::validate_references(&document, &document).unwrap();
    }

    #[test]
    fn external_cross_path_and_missing_targets_are_rejected() {
        let core = serde_json::json!({"paths": {"/good": {"get": {}}}});
        for reference in [
            "https://untrusted.invalid/schema",
            "./openapi-core.json#/paths/~1other",
            "#/paths/~1good",
        ] {
            let mut value = serde_json::json!({"$ref": reference});
            assert!(api_contracts::resolve_alias(&mut value, &core, "/paths/~1good").is_err());
        }
        let mut value = serde_json::json!({"$ref":"./openapi-core.json#/paths/~1missing"});
        assert!(api_contracts::resolve_alias(&mut value, &core, "/paths/~1missing").is_err());
    }
}
