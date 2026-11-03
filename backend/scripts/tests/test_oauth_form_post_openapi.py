"""Form-media and child-handler drift checks; no browser/runtime claims."""
from copy import deepcopy
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from test_verification_openapi import checker_module


class OAuthFormContractTests(unittest.TestCase):
    def test_child_module_forms_are_resolved_and_wrong_media_is_rejected(self):
        checker = checker_module()
        contracts = checker.source_contracts()
        document = checker.load_document(checker.ROOT / "docs/api/openapi.json")
        for path in ("/auth/oauth/{provider}/callback", "/auth/oauth/{provider}/callback/complete"):
            key = (path, "post")
            source = contracts[key]
            self.assertTrue(source.form_request)
            self.assertIn("form_post::", source.handler)
            checker.validate_against_handlers(document, {key: source}, {})
            mutated = deepcopy(document)
            mutated["paths"][path]["post"]["requestBody"]["content"] = {"application/json": {"schema": {"type": "object"}}}
            with self.assertRaisesRegex(ValueError, "form handler must document"):
                checker.validate_against_handlers(mutated, {key: source}, {})
            del mutated["paths"][path]["post"]["requestBody"]
            with self.assertRaisesRegex(ValueError, "form handler must document"):
                checker.validate_against_handlers(mutated, {key: source}, {})

    def test_unresolved_child_handler_fails_instead_of_becoming_bodyless(self):
        checker = checker_module()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "routes.rs").write_text('fn router() { Router::new().route("/x", post(child::missing)) }')
            with patch.object(checker, "ROUTER_ROOT", root):
                with self.assertRaisesRegex(ValueError, "unresolved route handler"):
                    checker.source_contracts()


if __name__ == "__main__":
    unittest.main()
