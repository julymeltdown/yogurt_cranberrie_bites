import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import unittest

from jsonschema import Draft202012Validator, FormatChecker, ValidationError

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("mcr_domain_bundle", ROOT / "backend/scripts/docs/generate-mcr-domain.py")
bundle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bundle)


class DomainBundleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.document = bundle.build_document()
        cls.core = bundle.core_document()
        cls.rows = json.loads((ROOT / "contracts/domain/v1/operations.json").read_text())
        cls.world = json.loads((ROOT / "contracts/realtime/v1/operations.json").read_text())

    def operation(self, row):
        return self.document["paths"][row["path"]][row["method"]]

    def validate(self, schema, value):
        Draft202012Validator({"components": self.document["components"], **schema},
                             format_checker=FormatChecker()).validate(value)

    def test_every_registered_http_operation_once_and_no_fake_websocket_endpoint(self):
        ids = [o["operationId"] for methods in self.document["paths"].values() for o in methods.values()]
        self.assertEqual(len(ids), 191)
        self.assertEqual(len(set(ids)), 191)
        for row in self.rows + self.world:
            self.assertEqual(self.operation(row)["operationId"], row["operationId"])
        self.assertFalse(any("websocket" in p or p.endswith("/world") for p in self.document["paths"]))
        self.assertEqual(self.document["x-mcr-contract-state"], "REVIEW_CANDIDATE")

    def test_core_paths_and_definitions_are_unchanged(self):
        for path, methods in self.core["paths"].items():
            self.assertEqual(self.document["paths"][path], methods)
        for name, schema in self.core["components"]["schemas"].items():
            self.assertEqual(self.document["components"]["schemas"][name], schema)
        self.assertEqual(json.loads((ROOT / "contracts/generated/mcr-core.openapi.json").read_text()), self.core)

    def test_event_user_moderation_cannot_silently_become_global_authority(self):
        row = next(row for row in self.rows if row["operationId"] == "moderateConcertUser")
        self.assertEqual(self.operation(row)["x-mcr-authority-scope"], "PATH_EVENT_ONLY")
        for changed in ({"authority_scope": "GLOBAL_AUTH"},
                        {"path": "/api/admin/users/{userId}/moderation"},
                        {"actor": "AUTH_ADMIN"}):
            with self.subTest(changed=changed), self.assertRaisesRegex(ValueError, "event-local authority"):
                bundle.domain_operation({**row, **changed}, self.document, {}, [])

    def test_all_schemas_valid_and_all_references_local_and_resolved(self):
        bundle.validate_document(self.document)
        for schema in self.document["components"]["schemas"].values():
            Draft202012Validator.check_schema(schema)

    def test_source_definition_translation_preserves_every_assertion(self):
        document = copy.deepcopy(self.core)
        references = bundle.add_schemas(document, ROOT)
        counts = {}
        for family in ("domain", "realtime"):
            counts[family] = 0
            for path in (ROOT / "contracts" / family / "v1").glob("*.schema.json"):
                source = json.loads(path.read_text())
                for name, schema in source["$defs"].items():
                    absolute = f'{source["$id"]}#/$defs/{name}'
                    self.assertEqual(bundle.resolve(document, references[absolute]),
                                     bundle.rewrite(schema, references, source["$id"]))
                    counts[family] += 1
        self.assertEqual(counts, {"domain": 214, "realtime": 24})

    def test_query_projection_is_exact_closed_and_delete_has_no_body(self):
        for row in self.rows:
            target = self.operation(row)
            query = bundle.resolve(self.document, target["x-mcr-query-schema"]["$ref"])
            parameters = {p["name"]: p for p in target["parameters"] if p["in"] == "query"}
            self.assertEqual(set(parameters), set(query["properties"]))
            self.assertTrue(target["x-mcr-closed-query"])
            for name, parameter in parameters.items():
                self.assertEqual(parameter["schema"], query["properties"][name])
                self.assertEqual(parameter["required"], name in query.get("required", []))
            if row["method"] in {"get", "delete"}:
                self.assertNotIn("requestBody", target)
            required = query.get("required", [])
            self.validate(target["x-mcr-query-schema"], {name: 1 for name in required}
                          if row["query_schema"] in {"common.RevisionRequest", "party.RemoveMember", "mural.RevisionQuery"}
                          else {} if not required else self.required_query(row))

    def required_query(self, row):
        if row["query_schema"] == "social.RevealQuery":
            return {"platform": "X"}
        return {"operation_id": "createConcertParty", "idempotency_key": "00000000-0000-4000-8000-000000000001"}

    def test_path_query_and_json_body_are_not_conflated(self):
        for row in self.rows:
            target = self.operation(row)
            actual = {p["name"] for p in target["parameters"] if p["in"] == "path"}
            self.assertEqual(actual, set(row["path_parameters"]))
            if row["request_location"] == "body" and row["request"] != "BINARY":
                self.assertEqual(target["requestBody"]["content"]["application/json"]["schema"],
                                 bundle.domain_ref(row["request"]))

    def test_error_status_reason_matrix_is_not_widened(self):
        base = {"code": "domain_error", "message": "Request unavailable"}
        for row in self.rows:
            target = self.operation(row)
            for code, reasons in row["errors"].items():
                schema = target["responses"][code]["content"]["application/json"]["schema"]
                for reason in reasons:
                    details = {"reason": reason}
                    if reason in {"RATE_LIMITED", "MURAL_COOLDOWN"}:
                        details["retry_after_ms"] = 1000
                    self.validate(schema, {**base, "details": details})
                wrong = "INVALID_INPUT" if "INVALID_INPUT" not in reasons else "PERMISSION_DENIED"
                with self.assertRaises(ValidationError):
                    self.validate(schema, {**base, "details": {"reason": wrong}})

    def test_pending_receipts_origin_idempotency_and_public_security(self):
        for row in self.rows:
            target = self.operation(row)
            headers = {p["name"]: p for p in target["parameters"] if p["in"] == "header"}
            self.assertEqual("Idempotency-Key" in headers, row["idempotency_key"])
            self.assertEqual("X-MCR-Intent" in headers, row["method"] != "get")
            self.assertEqual(target["security"], [] if row["actor"] == "PUBLIC" else [{"BearerAuth": []}])
            self.assertEqual("202" in target["responses"], row["pending_response"] is not None)
            if "202" in target["responses"]:
                self.assertEqual(target["responses"]["202"]["content"]["application/json"]["schema"],
                                 bundle.domain_ref(row["pending_response"]))
            for response in target["responses"].values():
                self.assertEqual(response["headers"]["Cache-Control"]["schema"]["const"], "no-store")
            self.assertEqual(target["x-mcr-runtime-status"], "NOT_IMPLEMENTED")

    def test_binary_media_never_becomes_json_and_keeps_allowed_types(self):
        put = next(row for row in self.rows if row["request"] == "BINARY")
        get = next(row for row in self.rows if row["response"] == "BINARY")
        expected = {"image/jpeg", "image/png", "image/webp"}
        self.assertEqual(set(self.operation(put)["requestBody"]["content"]), expected)
        success = self.operation(get)["responses"]["200"]
        self.assertEqual(set(success["content"]), expected)
        self.assertEqual(success["headers"]["X-Content-Type-Options"]["schema"]["const"], "nosniff")

    def test_receipt_projection_rejects_fingerprints_on_every_pending_command(self):
        receipt = json.loads((ROOT / 'contracts/fixtures/domain/command-pending.json').read_text())
        for row in self.rows:
            if row['pending_response'] != 'common.CommandReceipt':
                continue
            schema = self.operation(row)['responses'][str(row['pending_status'])]['content']['application/json']['schema']
            self.validate(schema, receipt)
            for field in ('request_hash', 'fingerprint', 'key_id', 'mac_secret', 'canonical_body'):
                with self.subTest(operation=row['operationId'], field=field), self.assertRaises(ValidationError):
                    self.validate(schema, {**receipt, field: 'synthetic-forbidden'})

    def test_world_ticket_stays_single_use_nonreplayable_without_command_receipt(self):
        for row in self.world:
            target = self.operation(row)
            self.assertEqual(target["x-mcr-retry"], row["retry"])
            self.assertNotIn("202", target["responses"])
            self.assertFalse(any(p["name"] == "Idempotency-Key" for p in target["parameters"]))
        ticket = self.operation(self.world[1])
        schema = ticket["responses"]["403"]["content"]["application/json"]["schema"]
        fixture = json.loads((ROOT / "contracts/fixtures/core/error-origin-not-allowed.json").read_text())
        self.validate(schema, fixture)

    def test_positive_domain_payload_fixtures_survive_relocation(self):
        for entry in json.loads((ROOT / "contracts/fixtures/domain/index.json").read_text()):
            if entry["expected"] == "accept":
                self.validate(bundle.domain_ref(entry["schema"]),
                              json.loads((ROOT / "contracts/fixtures/domain" / entry["file"]).read_text()))

    def test_closure_start_and_resume_remain_closed_after_relocation(self):
        entries = json.loads((ROOT / "contracts/fixtures/domain/index.json").read_text())
        for entry in entries:
            if entry["schema"] != "admin.CloseEvent":
                continue
            value = json.loads((ROOT / "contracts/fixtures/domain" / entry["file"]).read_text())
            with self.subTest(fixture=entry["file"]):
                if entry["expected"] == "accept":
                    self.validate(bundle.domain_ref("admin.CloseEvent"), value)
                else:
                    with self.assertRaises(ValidationError):
                        self.validate(bundle.domain_ref("admin.CloseEvent"), value)
        row = next(row for row in self.rows if row["operationId"] == "closeConcertEvent")
        operation = self.operation(row)
        self.assertEqual(operation["x-mcr-command-modes"], row["command_modes"])
        self.assertEqual(operation["x-mcr-retry"], "STATUS_THEN_IDENTICAL_COMMAND")
        self.assertEqual(row["command_modes"]["RESUME_CLOSURE"]["revision_subject"], "CLOSURE_JOB")

    def test_duplicate_registration_and_equivalent_path_names_fail(self):
        document = copy.deepcopy(self.document)
        first = self.rows[0]
        operation = copy.deepcopy(self.operation(first))
        document["paths"]["/new"] = {"get": operation}
        with self.assertRaisesRegex(ValueError, "Duplicate operationId"):
            bundle.validate_document(document)
        document = copy.deepcopy(self.document)
        document["paths"][first["path"].replace("{eventId}", "{otherId}")] = {}
        with self.assertRaisesRegex(ValueError, "Equivalent path templates"):
            bundle.validate_document(document)

    def test_unresolved_external_and_component_collisions_fail(self):
        for reference in ("https://unregistered.invalid/schema", "#/components/schemas/NoSuchSchema"):
            document = copy.deepcopy(self.document)
            document["components"]["schemas"]["Broken"] = {"$ref": reference}
            with self.assertRaises(ValueError):
                bundle.validate_document(document)
        document = copy.deepcopy(self.core)
        document["components"]["schemas"]["Domain_common_Uuid"] = {}
        with self.assertRaisesRegex(ValueError, "Component collision"):
            bundle.add_schemas(document, ROOT)
        with self.assertRaisesRegex(ValueError, "Unregistered source reference"):
            bundle.rewrite({"$ref": "#/$defs/unknown"}, {}, "https://example.invalid")

    def test_deterministic_artifact_and_check_mode(self):
        self.assertEqual(self.document, bundle.build_document())
        self.assertEqual(bundle.OUTPUT.read_text(), json.dumps(self.document, ensure_ascii=False, indent=2) + "\n")
        result = subprocess.run([sys.executable, str(ROOT / "backend/scripts/docs/generate-mcr-domain.py"), "--check"],
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
