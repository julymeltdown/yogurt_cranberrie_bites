"""Behavioral tests for the deterministic OpenAPI assembler."""

from __future__ import annotations

import importlib.util
import subprocess
import sys
import unittest
from pathlib import Path
from types import ModuleType
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
GENERATOR = ROOT / "scripts/docs/generate-openapi.py"


def load_generator() -> ModuleType:
    """Load the CLI script without invoking its command-line entry point."""
    spec = importlib.util.spec_from_file_location("generate_openapi", GENERATOR)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"could not load {GENERATOR}")
    module = importlib.util.module_from_spec(spec)
    sys.path.insert(0, str(GENERATOR.parent))
    try:
        spec.loader.exec_module(module)
    finally:
        sys.path.pop(0)
    return module


def collect_registrations(registrar) -> list[tuple[str, str, dict]]:
    """Capture the observable registrations exported by one operation family."""
    registrations = []

    def add(path: str, method: str, **kwargs) -> None:
        registrations.append((path, method, kwargs))

    registrar(add)
    return registrations


def isolated_content() -> bytes:
    """Assemble serialized content in a fresh Python interpreter."""
    code = (
        "import runpy, sys; "
        f"sys.path.insert(0, {str(GENERATOR.parent)!r}); "
        f"module = runpy.run_path({str(GENERATOR)!r}); "
        "sys.stdout.buffer.write(module['build_content']().encode('utf-8'))"
    )
    return subprocess.run(
        [sys.executable, "-c", code],
        cwd=ROOT,
        check=True,
        capture_output=True,
    ).stdout


class GenerateOpenApiTests(unittest.TestCase):
    def test_modules_export_registrars_and_assemble_complete_document(self) -> None:
        # Given: the operation-family exports used by the CLI assembler
        generator = load_generator()
        registrars = (
            generator.platform_docs.add_operations,
            generator.auth.add_operations,
            generator.profile_social.add_operations,
            generator.post_notification.add_operations,
        )

        # When: each exported registrar and the real assembler are exercised
        family_registrations = [collect_registrations(value) for value in registrars]
        document = generator.build_document()

        # Then: every export contributes operations to the complete contract
        self.assertTrue(all(callable(value) for value in registrars))
        self.assertTrue(all(family_registrations))
        self.assertEqual(91, sum(map(len, family_registrations)))
        self.assertEqual(92, sum(map(len, document["paths"].values())))
        self.assertIn("/admin/post-events/dead-letters/{event_id}/replay", document["paths"])
        self.assertNotIn("/metrics", document["paths"])

    def test_async_acceptance_contract_is_limited_to_receipt_backed_routes(self) -> None:
        document = load_generator().build_document()
        expected_async_routes = {
            ("/api/follows", "post"),
            ("/api/follows/{target_user_id}", "delete"),
            ("/api/posts", "post"),
            ("/api/posts/{post_id}", "delete"),
            ("/api/posts/{post_id}/comments", "post"),
            ("/api/posts/{post_id}/likes", "post"),
            ("/api/posts/{post_id}/likes", "delete"),
            ("/api/profile/privacy", "put"),
            ("/api/profile/{user_id}", "put"),
            ("/api/moderation/reports", "post"),
        }

        documented_async_routes = {
            (path, method)
            for path, path_item in document["paths"].items()
            for method, operation in path_item.items()
            if "202" in operation["responses"]
            and path.startswith(
                ("/api/follows", "/api/posts", "/api/profile", "/api/moderation/reports")
            )
        }

        self.assertEqual(expected_async_routes, documented_async_routes)
        for path, method in expected_async_routes:
            accepted = document["paths"][path][method]["responses"]["202"]
            self.assertEqual(
                "#/components/schemas/AsyncOperationAccepted",
                accepted["content"]["application/json"]["schema"]["$ref"],
            )
            self.assertTrue(
                {"Cache-Control", "Location", "Retry-After", "X-Operation-Id"}
                .issubset(accepted["headers"])
            )

        caller_state = document["components"]["schemas"]["CallerOperationStatus"]
        self.assertIn("QUEUED", caller_state["properties"]["state"]["enum"])

    def test_real_assembly_rejects_duplicate_operation_ids(self) -> None:
        # Given: the real auth registrar followed by one sorted duplicate ID
        generator = load_generator()
        original = generator.auth.add_operations

        def add_duplicate(add) -> None:
            original(add)
            add("/zz-duplicate", "get", operation_id="login")

        # When/Then: the real build rejects the duplicate before serialization
        with patch.object(generator.auth, "add_operations", add_duplicate):
            with self.assertRaisesRegex(ValueError, "duplicate operationId: login"):
                generator.build_document()

    def test_real_assembly_rejects_reversed_registrations(self) -> None:
        # Given: a real operation family whose exported registrations are reversed
        generator = load_generator()
        original = generator.profile_social.add_operations

        def add_reversed(add) -> None:
            for path, method, kwargs in reversed(collect_registrations(original)):
                add(path, method, **kwargs)

        # When/Then: pre-sort assembly validation rejects the unstable order
        with patch.object(generator.profile_social, "add_operations", add_reversed):
            with self.assertRaisesRegex(
                ValueError, "operation registrations must be sorted"
            ):
                generator.build_document()

    def test_isolated_assembly_is_byte_deterministic(self) -> None:
        # Given: the checked-in byte contract
        expected = (ROOT / "docs/api/openapi.json").read_bytes()

        # When: two independent interpreters assemble and serialize it
        first = isolated_content()
        second = isolated_content()

        # Then: both isolated results exactly match each other and the contract
        self.assertEqual(expected, first)
        self.assertEqual(first, second)

    def test_oauth_state_security_is_provider_bound(self) -> None:
        # Given: the generated OAuth callback and logical state-cookie scheme
        document = load_generator().build_document()
        callback = document["paths"]["/auth/oauth/{provider}/callback"]["get"]
        scheme = document["components"]["securitySchemes"]["OAuthStateCookie"]

        # When: their machine-consumed security metadata is inspected
        security = callback["security"]
        provider_cookie_names = scheme["x-provider-cookie-names"]

        # Then: one provider-bound requirement replaces the wrong-provider OR list
        self.assertEqual([{"OAuthStateCookie": []}], security)
        self.assertEqual(
            {
                "google": "oauth_state_google",
                "apple": "oauth_state_apple",
                "x": "oauth_state_x",
            },
            provider_cookie_names,
        )

    def test_json_extractor_errors_are_declared_only_for_json_body_operations(self) -> None:
        # Given: the complete generated public Gateway contract.
        document = load_generator().build_document()
        expected_schema = {"$ref": "#/components/schemas/ErrorResponse"}

        # When: JSON-specific extractor statuses are inspected for every operation.
        extractor_responses = [
            (path, method, status, operation, operation["responses"].get(status))
            for path, path_item in document["paths"].items()
            for method, operation in path_item.items()
            for status in ("415", "422")
        ]

        # Then: JSON requests document their rejection envelope, while bodyless and
        # multipart operations do not promise JSON-only extractor failures.
        self.assertTrue(extractor_responses)
        for path, method, status, operation, response in extractor_responses:
            has_json_body = "application/json" in operation.get("requestBody", {}).get(
                "content", {}
            )
            with self.subTest(path=path, method=method, status=status):
                if has_json_body:
                    self.assertIsNotNone(response)
                    self.assertEqual(
                        expected_schema,
                        response["content"]["application/json"]["schema"],
                    )
                else:
                    self.assertIsNone(response)


if __name__ == "__main__":
    unittest.main()
