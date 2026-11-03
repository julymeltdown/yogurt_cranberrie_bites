"""Public resend contract and local bundle regressions; no Rust/DB execution claim."""

from __future__ import annotations

from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
DOCS = ROOT / "scripts/docs"
sys.path.insert(0, str(DOCS))
try:
    import openapi_core
    from openapi.verification import PATH, extend_document
    from openapi_bundle import bundle_document, core_reference, load_document, split_document
finally:
    sys.path.pop(0)


def checker_module():
    spec = importlib.util.spec_from_file_location("verification_openapi_checker", DOCS / "check-openapi.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    sys.path.insert(0, str(DOCS))
    try:
        spec.loader.exec_module(module)
    finally:
        sys.path.pop(0)
    return module


class VerificationOpenApiTests(unittest.TestCase):
    def setUp(self) -> None:
        self.core = openapi_core.build_document()
        self.document = extend_document(self.core)
        self.index = split_document(self.core, self.document)

    def test_existing_operations_and_components_are_unchanged(self):
        for path, value in self.core["paths"].items():
            self.assertEqual(value, self.document["paths"][path])
        for category, entries in self.core["components"].items():
            for name, value in entries.items():
                self.assertEqual(value, self.document["components"][category][name])
        self.assertEqual(92, sum(map(len, self.document["paths"].values())))

    def test_checked_in_artifacts_round_trip(self):
        self.assertEqual(self.document, load_document(ROOT / "docs/api/openapi.json"))
        self.assertEqual(self.document, bundle_document(self.index, self.core))

    def test_request_and_response_are_typed_and_minimal(self):
        operation = self.document["paths"][PATH]["post"]
        self.assertEqual([], operation["security"])
        self.assertEqual(4096, operation["x-max-body-bytes"])
        schemas = self.document["components"]["schemas"]
        self.assertEqual(["email"], schemas["ResendVerificationRequest"]["required"])
        self.assertFalse(schemas["ResendVerificationRequest"]["additionalProperties"])
        self.assertEqual(["ACCEPTED"], schemas["ResendVerificationResponse"]["properties"]["status"]["enum"])
        self.assertEqual({"status"}, set(schemas["ResendVerificationResponse"]["properties"]))
        self.assertEqual({"202"}, {key for key in operation["responses"] if key.startswith("2")})

    def test_all_resend_responses_are_non_cacheable(self):
        responses = self.document["paths"][PATH]["post"]["responses"]
        self.assertTrue({"400", "413", "415", "422", "429", "500", "502", "503", "504"} <= set(responses))
        for response in responses.values():
            self.assertEqual(["no-store"], response["headers"]["Cache-Control"]["schema"]["enum"])
            self.assertIn("X-Correlation-Id", response["headers"])

    def test_untrusted_or_cross_path_aliases_are_rejected(self):
        for reference in ["https://untrusted.invalid/schema", "../other.json#/paths/a", core_reference("paths", "/auth/login")]:
            index = deepcopy(self.index)
            index["paths"]["/api/feed"] = {"$ref": reference}
            with self.assertRaises(ValueError):
                bundle_document(index, self.core)

    def test_missing_or_recursive_targets_are_rejected(self):
        for target in [None, {"$ref": core_reference("paths", "/api/feed")}]:
            core = deepcopy(self.core)
            core["paths"]["/api/feed"] = target
            with self.assertRaises(ValueError):
                bundle_document(self.index, core)

    def test_reference_siblings_cannot_override_core(self):
        index = deepcopy(self.index)
        index["paths"]["/api/feed"]["get"] = {"operationId": "replacement"}
        with self.assertRaises(ValueError):
            bundle_document(index, self.core)

    def test_unresolved_nested_external_reference_is_rejected(self):
        index = deepcopy(self.index)
        index["components"]["schemas"]["ResendVerificationRequest"]["properties"]["email"] = {"$ref":"file:///private"}
        with self.assertRaises(ValueError):
            bundle_document(index, self.core)

    def test_new_handler_matches_actual_request_dto(self):
        checker = checker_module()
        contracts = checker.source_contracts()
        key = (PATH, "post")
        self.assertEqual("ResendVerificationRequest", contracts[key].json_request)
        checker.validate_against_handlers(self.document, {key: contracts[key]}, checker.rust_structs(checker.DTO_ROOT))
        checker.validate_structure(self.document)
        checker.validate_special_responses(self.document)
        checker.validate_expected_success_response_schemas(self.document)

    def test_handler_signature_scanner_does_not_confuse_lifetimes_with_char_literals(self):
        checker = checker_module()
        signatures = checker.handler_signatures(
            "async fn read_field(field: MultipartField<'_>) -> Result<(), Error> { }"
        )
        self.assertEqual("field: MultipartField<'_>", signatures["read_field"])

    def test_request_schema_drift_is_not_hidden_by_bundling(self):
        checker = checker_module()
        contracts = checker.source_contracts()
        document = deepcopy(self.document)
        document["components"]["schemas"]["ResendVerificationRequest"]["properties"]["userId"] = {"type":"string"}
        key = (PATH, "post")
        with self.assertRaisesRegex(ValueError, "request properties differ"):
            checker.validate_against_handlers(document, {key: contracts[key]}, checker.rust_structs(checker.DTO_ROOT))

    def test_new_response_dto_is_checked(self):
        checker = checker_module()
        with patch.object(checker, "RESPONSE_SCHEMA_STRUCTS", {"ResendVerificationResponse":"ResendVerificationResponse"}):
            checker.validate_response_structures(self.document, checker.rust_structs(checker.DTO_ROOT))
            document = deepcopy(self.document)
            document["components"]["schemas"]["ResendVerificationResponse"]["properties"]["delivered"] = {"type":"boolean"}
            with self.assertRaises(ValueError):
                checker.validate_response_structures(document, checker.rust_structs(checker.DTO_ROOT))

    def test_bundle_cli_returns_a_standalone_document(self):
        output = subprocess.run([sys.executable, str(DOCS / "generate-openapi.py"), "--bundle"], check=True, capture_output=True)
        self.assertEqual(self.document, json.loads(output.stdout))
        self.assertNotIn(b"./openapi-core.json#", output.stdout)


if __name__ == "__main__":
    unittest.main()
