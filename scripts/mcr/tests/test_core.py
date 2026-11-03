import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import unittest

from jsonschema import Draft202012Validator, ValidationError

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("mcr_validate_core", ROOT / "scripts/mcr/validate-core.py")
core = importlib.util.module_from_spec(spec)
spec.loader.exec_module(core)


class CoreContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.document = core.generator.build_document()

    def fixture(self, name):
        return json.loads((ROOT / "contracts/fixtures/core" / (name + ".json")).read_text())

    def reject(self, value, schema, **context):
        with self.assertRaises((ValueError, ValidationError)):
            core.validate(value, schema, self.document, **context)

    def test_all_indexed_golden_success_and_failure_envelopes(self):
        self.assertEqual(core.validate_fixtures(self.document), 33)

    def test_auth_operation_map_uses_final_generator(self):
        source = core.generator.inherited_document()
        actual = {}
        for path, methods in self.document["paths"].items():
            for method, operation in methods.items():
                if "x-mcr-upstream" in operation:
                    upstream = operation["x-mcr-upstream"]
                    expected = source["paths"][upstream["path"]][method]
                    self.assertEqual(path, "/api" + upstream["path"])
                    self.assertEqual(operation["operationId"], expected["operationId"])
                    self.assertEqual(upstream["method"], method.upper())
                    actual[operation["operationId"]] = operation
        self.assertEqual(set(actual), core.generator.AUTH_IDS)
        self.assertNotIn("/api/auth/email/check", self.document["paths"])
        self.assertEqual(actual["logout"]["security"], [])

    def test_target_routes_never_claim_runtime_implementation(self):
        ids = []
        for methods in self.document["paths"].values():
            for method, operation in methods.items():
                ids.append(operation["operationId"])
                self.assertIn(operation["x-mcr-runtime-status"], {"NOT_IMPLEMENTED", "ADAPTATION_REQUIRED"})
                if method in {"post", "delete", "patch", "put"} and "callback" not in operation["operationId"].lower():
                    if operation["operationId"] in {"receiveAppleOAuthFormPost", "completeAppleOAuthFormPost"}:
                        continue
                    self.assertTrue(any(p.get("name") == "X-MCR-Intent" for p in operation.get("parameters", [])))
        self.assertEqual(len(ids), len(set(ids)))

    def test_schema_definitions_and_all_local_refs_resolve(self):
        for schema in self.document["components"]["schemas"].values():
            Draft202012Validator.check_schema(schema)
        def walk(value):
            if isinstance(value, dict):
                if "$ref" in value:
                    reference = value["$ref"]
                    self.assertTrue(reference.startswith("#/"), reference)
                    target = self.document
                    for component in reference[2:].split("/"):
                        target = target[component.replace("~1", "/").replace("~0", "~")]
                for child in value.values():
                    walk(child)
            elif isinstance(value, list):
                for child in value:
                    walk(child)
        walk(self.document)

    def test_public_profile_rejects_private_fields(self):
        for name in ["email", "exact_age", "age_claims", "social_handles", "preferences", "meeting", "access_token"]:
            with self.subTest(name=name):
                value = self.fixture("public-profile")
                value[name] = "private"
                self.reject(value, "PublicProfile")

    def test_private_identity_and_event_context_are_bound(self):
        value = self.fixture("private-participant")
        other = "00000000-0000-4000-8000-000000000999"
        self.reject(value, "PrivateParticipant", actor_id=other)
        self.reject(value, "PrivateParticipant", event_id=other)
        value["profile"]["user_id"] = other
        self.reject(value, "PrivateParticipant")

    def test_bootstrap_detects_cross_event_and_avatar_projection(self):
        value = self.fixture("bootstrap-open")
        value["participant"]["event_id"] = "00000000-0000-4000-8000-000000000999"
        self.reject(value, "Bootstrap")
        value = self.fixture("bootstrap-open")
        value["avatar"]["revision"] += 1
        self.reject(value, "Bootstrap")

    def test_bootstrap_rejects_unknown_avatar_item_even_with_consistent_projection(self):
        value = self.fixture("bootstrap-open")
        value["avatar"]["config"]["item_ids"]["hair"] = "unknown_hair"
        value["participant"]["profile"]["avatar"] = copy.deepcopy(value["avatar"])
        self.reject(value, "Bootstrap")

    def test_read_only_event_cannot_enable_writes(self):
        value = self.fixture("bootstrap-read-only")
        value["capabilities"]["mutations"] = {"available": True, "reason": None}
        self.reject(value, "Bootstrap")

    def test_capabilities_are_unique_and_truthfully_disabled(self):
        value = self.fixture("auth-capabilities-disabled")
        value["providers"][0]["provider"] = "X"
        self.reject(value, "AuthCapabilities")
        value = self.fixture("auth-capabilities-disabled")
        value["providers"][0]["enabled"] = True
        self.reject(value, "AuthCapabilities")

    def test_public_discovery_never_rotates_credentials(self):
        operation = self.document["paths"]["/api/auth/capabilities"]["get"]
        self.assertEqual(operation["security"], [])
        self.assertEqual(operation["x-mcr-retry"], "PUBLIC_READ_NO_AUTH_REFRESH")

    def test_origin_denial_is_typed_and_documented_for_resend(self):
        value = self.fixture("error-origin-not-allowed")
        core.validate(value, "OriginErrorResponse", self.document,
                      operation_id="resendVerificationEmail", http_status=403)
        value["details"]["private_handle"] = "private"
        self.reject(value, "OriginErrorResponse")
        self.reject(value, "AuthForbiddenResponse")
        core.validate({"code": "GATEWAY_403", "message": "Forbidden", "details": None},
                      "AuthForbiddenResponse", self.document)

    def test_all_guarded_operations_document_origin_failures(self):
        for methods in self.document["paths"].values():
            for operation in methods.values():
                if "x-mcr-origin-policy" in operation:
                    denial = operation["x-mcr-origin-rejection"]
                    self.assertEqual(denial["status"], 403)
                    self.assertFalse(denial["downstream_dispatch"])
                    self.assertIn("403", operation["responses"])
                    self.assertEqual(denial["schema"]["$ref"], "#/components/schemas/OriginErrorResponse")

    def test_core_transport_failures_keep_status_and_safe_reason(self):
        for status in [413, 415, 422, 500, 502, 504]:
            value = self.fixture(f"core-transport-{status}")
            core.validate(value, "CoreErrorResponse", self.document,
                          operation_id="setConcertAvatar", http_status=status)
            value["details"]["reason"] = "VERSION_CONFLICT"
            self.reject(value, "CoreErrorResponse", operation_id="setConcertAvatar", http_status=status)

    def test_place_and_public_profile_request_identity_are_bound(self):
        other = "00000000-0000-4000-8000-000000000999"
        for name, schema in [("place", "PlaceResponse"), ("place-list", "PlaceList")]:
            self.reject(self.fixture(name), schema, event_id=other)
        self.reject(self.fixture("place"), "PlaceResponse", place_id=other)
        self.reject(self.fixture("public-profile"), "PublicProfile", target_user_id=other)
        # A public target profile is allowed to differ from the authenticated caller.
        core.validate(self.fixture("public-profile"), "PublicProfile", self.document, actor_id=other)

    def test_core_error_reason_and_safe_details_are_required(self):
        for details in [None, {}, {"reason": []}, {"reason": "UNREGISTERED_REASON"},
                        {"reason": "VERSION_CONFLICT", "private_handle": "private"}]:
            value = self.fixture("error-version-conflict")
            value["details"] = details
            self.reject(value, "CoreErrorResponse")

    def test_error_example_is_checked_against_actual_operation_status(self):
        self.reject(self.fixture("error-version-conflict"), "CoreErrorResponse",
                    operation_id="setConcertAvatar", http_status=404)
        self.reject(self.fixture("error-version-conflict"), "CoreErrorResponse",
                    operation_id="setConcertAvatar", http_status=418)

    def test_bootstrap_delivers_three_consistent_theme_manifests(self):
        value = self.fixture("bootstrap-open")
        self.assertEqual({t["theme_id"] for t in value["theme_manifests"]}, {"REVENGE", "PARADE", "DANGER"})
        value["theme_manifests"][0]["map_revision"] += 1
        value["theme_manifests"][0]["content_hash"] = core.assets.content_hash(value["theme_manifests"][0])
        self.reject(value, "Bootstrap")
        value = self.fixture("bootstrap-open")
        value["theme_manifests"][0] = copy.deepcopy(value["theme_manifests"][1])
        self.reject(value, "Bootstrap")

    def test_browser_cannot_supply_asset_url_or_private_auth_credential(self):
        value = self.fixture("set-avatar-request")
        value["config"]["asset_url"] = "https://example.invalid/image.png"
        self.reject(value, "SetAvatarRequest")
        value = self.fixture("auth-success")
        value["refreshToken"] = "must-not-leak"
        self.reject(value, "AuthResponse")

    def test_utf8_password_boundaries_match_source_bytes(self):
        for password in ["1234567", "가" * 43, "a" * 129, "\ud800invalid"]:
            self.reject({"email": "test@example.invalid", "password": password}, "EmailSignupRequest")
        for password in ["가나다", "😀😀", "a" * 128]:
            core.validate({"email": "test@example.invalid", "password": password}, "EmailSignupRequest", self.document)

    def test_revision_and_nonempty_profile_update_are_required(self):
        self.reject({"display_name": "Fan"}, "UpdateParticipantRequest")
        self.reject({"expected_revision": 1}, "UpdateParticipantRequest")
        value = self.fixture("set-avatar-request")
        value["expected_revision"] = 0
        self.reject(value, "SetAvatarRequest")

    def test_utc_and_places_are_unambiguous(self):
        value = self.fixture("bootstrap-open")
        value["server_time"] = "2026-11-03T09:00:00+09:00"
        self.reject(value, "Bootstrap")
        value = self.fixture("place-list")
        value["items"].append(copy.deepcopy(value["items"][0]))
        self.reject(value, "PlaceList")

    def test_generated_artifact_is_deterministic_and_current(self):
        expected = json.dumps(self.document, ensure_ascii=False, indent=2) + "\n"
        self.assertEqual(expected, core.generator.OUTPUT.read_text())
        self.assertEqual(self.document, core.generator.build_document())
        result = subprocess.run([sys.executable, str(ROOT / "backend/scripts/docs/generate-mcr-core.py"), "--check"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_deletion_step_accepts_concert_targets_and_rejects_unknown(self):
        step = {
            "target": "PROFILE", "status": "PENDING", "attemptCount": 0,
            "nextAttemptAt": "2026-11-04T00:00:00Z", "lastError": None,
            "completedAt": None,
        }
        targets = ["PROFILE", "FRIEND", "POST", "NOTIFICATION", "CONCERT_TICKETS",
                   "CONCERT_SESSIONS", "CONCERT_SHARING", "CONCERT_PRIVATE_DATA"]
        actual = self.document["components"]["schemas"]["UserDeletionStep"]["properties"]["target"]["enum"]
        self.assertEqual(actual, targets)
        for target in targets:
            with self.subTest(target=target):
                core.validate({**step, "target": target}, "UserDeletionStep", self.document)
        self.reject({**step, "target": "CONCERT_ALL"}, "UserDeletionStep")


if __name__ == "__main__":
    unittest.main()
