"""Public CLI regression: a positive run must pass before negative exits count."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
spec = importlib.util.spec_from_file_location("qa01_run", HERE / "run.py")
coverage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(coverage)


def command(*args):
    process = subprocess.run([sys.executable, "-B", str(HERE / "run.py"), *args],
                             cwd=ROOT, capture_output=True, text=True, timeout=180)
    return process, json.loads(process.stdout)


class CoverageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        process, receipt = command()
        if process.returncode or receipt["result"] != "PASS":
            raise RuntimeError("positive baseline must pass before negatives: " + process.stdout + process.stderr)
        cls.receipt = receipt
        cls.results = [{"command": "run.py", "exit": process.returncode, "receipt": receipt}]

    @classmethod
    def tearDownClass(cls):
        if os.environ.get("QA01_WRITE_TEST_RECEIPT") == "1":
            output = ROOT / "docs/mcr/evidence/contracts/qa01-coverage/negative-results.json"
            output.write_text(json.dumps(cls.results, indent=2) + "\n")

    def test_positive_public_entrypoint_is_nonempty(self):
        self.assertGreater(self.receipt["fixtures"], 0)
        self.assertGreater(self.receipt["responses"], 0)
        self.assertEqual(self.receipt["operations"], len(coverage.read(coverage.PIN)["operations"]))

    def test_public_cli_rejects_real_mutations(self):
        errors = {
            "broken-schema": "SchemaError",
            "unknown-enum": "schema",
            "empty-index": "empty fixture index",
            "missing-fixture": "unindexed/missing fixture",
            "unindexed-fixture": "unindexed/missing fixture",
            "missing-operation": "missing/unexpected operation",
            "missing-response": "generated OpenAPI drift",
            "drift": "generated OpenAPI drift",
            "broken-ref": "Unresolvable",
            "empty-responses": "empty response coverage",
            "empty-generated": "empty generated operation coverage",
            "pinned-input-drift": "pinned input drift",
        }
        for probe in coverage.PROBES:
            with self.subTest(probe=probe):
                process, receipt = command("--probe", probe)
                self.results.append({"command": "run.py --probe " + probe,
                                     "exit": process.returncode, "receipt": receipt})
                self.assertEqual(process.returncode, 1, process.stdout + process.stderr)
                self.assertEqual(receipt["result"], "FAIL")
                self.assertIn(errors[probe], receipt["error"])

    def test_missing_dependencies_are_not_silent_success(self):
        process = subprocess.run([sys.executable, "-B", "-S", str(HERE / "run.py")],
                                 cwd=ROOT, capture_output=True, text=True, timeout=30)
        self.assertNotEqual(process.returncode, 0)
        self.assertNotIn('"result": "PASS"', process.stdout)

    def test_missing_optional_format_plugins_fail_closed(self):
        assets = coverage.load("asset_no_formats", "scripts/mcr/validate-assets.py")
        from jsonschema import FormatChecker
        with patch.object(assets, "FormatChecker", lambda: FormatChecker(formats=())):
            with self.assertRaisesRegex(assets.AssetError, "format checkers unavailable"):
                assets.validators()

    def test_catalog_duplicate_and_traversal_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "duplicate"):
            coverage.verify_catalog([{"file": "a.json"}] * 2, ["a.json"], "test")
        with self.assertRaisesRegex(ValueError, "unsafe"):
            coverage.verify_catalog([{"file": "../a.json"}], ["a.json"], "test")

    def test_missing_schema_family_cannot_be_an_empty_pass(self):
        with self.assertRaisesRegex(ValueError, "empty source schema"):
            coverage.check_schemas([])

    def test_duplicate_and_nonfinite_json_are_rejected(self):
        # Test the public reader without writing outside the exclusive lease.
        class Source:
            def __init__(self, value): self.value = value
            def read_text(self, **_kwargs): return self.value
        for payload in ('{"x":1,"x":2}', '{"x":NaN}', '{"x":Infinity}'):
            with self.subTest(payload=payload), self.assertRaises(ValueError):
                coverage.read(Source(payload))


if __name__ == "__main__":
    unittest.main(verbosity=2)
