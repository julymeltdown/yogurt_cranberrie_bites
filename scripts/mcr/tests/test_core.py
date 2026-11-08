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
                self.assertIn(operation["x-mcr-runtime-status"],
                              {"NOT_IMPLEMENTED", "ADAPTATION_REQUIRED", "IMPLEMENTATION_IN_PROGRESS"})
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

    def test_bootstrap_selected_static_reuses_canonical_union_and_theme_pins(self):
        value = self.fixture("bootstrap-open")
        directory = ROOT / "contracts/fixtures/assets/selected-static"
        manifest = json.loads((directory / "avatar-manifest.json").read_text())
        config = json.loads((directory / "photo01_front_06.json").read_text())
        value["avatar_manifest"] = manifest
        value["avatar"]["config"] = config
        value["participant"]["profile"]["avatar"] = copy.deepcopy(value["avatar"])
        for theme in value["theme_manifests"]:
            theme.update(avatar_manifest_id=manifest["manifest_id"], avatar_manifest_version=1,
                         avatar_content_hash=manifest["content_hash"])
            theme["content_hash"] = core.assets.content_hash(theme)
        core.validate(value, "Bootstrap", self.document)
        value["avatar"]["config"]["preset_id"] = "photo01_front_07"
        value["participant"]["profile"]["avatar"] = copy.deepcopy(value["avatar"])
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

    # --- WP-01 profile core contract (T-001, T-004, T-007) -----------------

    PROFILE_DEFINITIONS = ["OwnProfile", "PublicProfileV2", "IdentityVerification",
                           "StartIdentityVerification", "IdentityCallbackQuery",
                           "IdentityVerificationStart", "FavoriteItem", "UpdateProfile",
                           "SetFavorites", "CatalogQuery", "CatalogPage", "PeopleQuery",
                           "PeoplePage"]

    PROFILE_OPERATIONS = ["getOwnProfile", "updateOwnProfile", "setOwnFavorites", "getProfile",
                          "searchCatalog", "discoverProfiles", "getOwnIdentityVerification",
                          "startIdentityVerification", "completeIdentityVerification",
                          "disconnectIdentityVerification"]

    # Objects a viewer can receive for another member. None may carry a numeric age.
    VIEWER_FACING = ["PublicProfileV2", "PeopleCard", "CatalogItem", "CatalogPage", "PeoplePage"]

    def definitions(self):
        return self.document["components"]["schemas"]

    def test_profile_core_definitions_exist_and_are_closed(self):
        for name in self.PROFILE_DEFINITIONS:
            with self.subTest(name=name):
                schema = self.definitions()[name]
                self.assertIs(schema.get("additionalProperties"), False)
                self.assertIn("required", schema)
                self.assertLessEqual(set(schema["required"]), set(schema["properties"]))
                Draft202012Validator.check_schema(schema)

    def test_viewer_facing_objects_never_expose_a_numeric_age(self):
        forbidden = {"age", "minimum_age", "maximum_age_exclusive", "exact_age",
                     "age_at_verification", "birthyear", "birthday", "date_of_birth",
                     "verified_age", "age_bounds", "audience", "allowed_gender"}
        for name in self.VIEWER_FACING:
            with self.subTest(name=name):
                self.assertFalse(forbidden & set(self.definitions()[name]["properties"]))

    def test_public_profile_v2_rejects_age_and_audience_fields(self):
        value = {
            "profile_id": "00000000-0000-4000-8000-000000000002",
            "nickname": "Fixture fan",
            "bio": None,
            "profile_image_media_id": None,
            "avatar": {"revision": 1, "config": self.fixture("avatar-state")["config"]},
            "favorites_public": [],
            "shared_with_viewer": [],
            "interaction_style": None,
            "gender_bucket": None,
            "follow": {"following": False, "followed_by": False},
            "blocked": False,
            "room_available": True,
        }
        core.validate(value, "PublicProfileV2", self.document)
        for name, injected in [("age", 25), ("minimum_age", 18), ("maximum_age_exclusive", 30),
                               ("audience", {"allowed_gender": "any"}), ("allowed_gender", "female"),
                               ("exact_age", 25), ("date_of_birth", "2000-01-01")]:
            with self.subTest(field=name):
                self.reject({**value, name: injected}, "PublicProfileV2")
        value["gender_bucket"] = "female"
        core.validate(value, "PublicProfileV2", self.document)
        value["gender_bucket"] = "any"
        self.reject(value, "PublicProfileV2")

    def test_gender_bucket_appears_only_on_the_owner_claim_and_public_projection(self):
        holders = sorted(name for name, schema in self.definitions().items()
                         if "gender_bucket" in schema.get("properties", {}))
        self.assertEqual(holders, ["IdentityClaims", "PublicProfileV2"])
        for name in ["OwnProfile", "UpdateProfile", "PeopleCard", "CatalogItem"]:
            with self.subTest(name=name):
                self.assertNotIn("gender_bucket", self.definitions()[name]["properties"])

    def test_identity_verification_owner_view_and_claim_bounds(self):
        value = {
            "provider": "naver",
            "status": "VERIFIED",
            "verified_at": "2026-11-08T00:00:00Z",
            "expires_at": "2027-02-06T00:00:00Z",
            "claims": {"age": 25, "gender_bucket": "female"},
            "display_on_profile": False,
        }
        core.validate(value, "IdentityVerification", self.document)
        for claims in [{"age": 13, "gender_bucket": "female"},
                       {"age": 101, "gender_bucket": "female"},
                       {"age": 25, "gender_bucket": "any"},
                       {"age": 25}]:
            with self.subTest(claims=claims):
                self.reject({**value, "claims": claims}, "IdentityVerification")
        self.reject({**value, "provider": "google"}, "IdentityVerification")
        self.reject({**value, "status": "UNKNOWN"}, "IdentityVerification")

    def test_identity_start_return_path_is_same_origin_only(self):
        core.validate({"return_path": "/me/settings"}, "StartIdentityVerification", self.document)
        for path in ["//evil.example/steal", "https://evil.example/", "me/settings", "/a\nb", ""]:
            with self.subTest(path=path):
                self.reject({"return_path": path}, "StartIdentityVerification")

    def test_profile_operation_register_is_unique_and_event_free(self):
        register = json.loads((ROOT / "contracts/core/v1/operations.json").read_text())
        ids = [row["operationId"] for row in register]
        self.assertEqual(len(ids), len(set(ids)))
        routes = [(row["method"], row["path"]) for row in register]
        self.assertEqual(len(routes), len(set(routes)))
        self.assertEqual(len(register), 25)
        ids = {row["operationId"] for row in register}
        self.assertLessEqual(set(self.PROFILE_OPERATIONS), ids)
        for row in register:
            if row["operationId"] in self.PROFILE_OPERATIONS:
                with self.subTest(operation=row["operationId"]):
                    self.assertNotIn("/events/{eventId}", row["path"])

    def test_complete_identity_verification_is_the_only_303(self):
        redirects = []
        for path, methods in self.document["paths"].items():
            for method, operation in methods.items():
                if "303" in operation["responses"]:
                    redirects.append(operation["operationId"])
        self.assertEqual(redirects, ["completeIdentityVerification"])
        operation = self.document["paths"]["/api/identity/{provider}/callback"]["get"]
        self.assertEqual(operation["security"], [])
        self.assertNotIn("X-MCR-Intent", [p["name"] for p in operation["parameters"]])
        self.assertEqual(operation["x-mcr-redirect"], "SERVER_DERIVED_STORED_RETURN_PATH_ONLY")
        self.assertIs(operation["x-mcr-anonymous-provider-callback"], True)
        location = operation["responses"]["303"]["headers"]["Location"]
        self.assertIs(location["required"], True)
        self.assertNotIn("content", operation["responses"]["303"])

    def test_profile_operations_are_anonymous_only_for_the_provider_callback(self):
        anonymous = []
        for path, methods in self.document["paths"].items():
            for operation in methods.values():
                if operation["operationId"] in self.PROFILE_OPERATIONS and operation["security"] == []:
                    anonymous.append(operation["operationId"])
        self.assertEqual(anonymous, ["completeIdentityVerification"])

    def test_profile_operations_declare_closed_queries_and_no_unknown_fields(self):
        operation = self.document["paths"]["/api/catalog/{category}/search"]["get"]
        query = [p for p in operation["parameters"] if p["in"] == "query"]
        self.assertEqual([p["name"] for p in query], ["limit", "cursor", "q"])
        self.assertNotIn("requestBody", operation)
        category = next(p for p in operation["parameters"] if p["name"] == "category")
        self.assertEqual(category["schema"]["enum"], ["MUSIC", "ANIME", "GAME"])
        provider = self.document["paths"]["/api/me/identity/{provider}"]["get"]
        self.assertEqual(next(p for p in provider["parameters"] if p["name"] == "provider")["schema"]["enum"],
                         ["naver"])


if __name__ == "__main__":
    unittest.main()
