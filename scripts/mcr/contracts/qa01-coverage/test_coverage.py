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


class C07NamespaceTests(unittest.TestCase):
    def test_c07_separation_is_exact_not_a_global_exclusion(self):
        for name in ("contracts/fixtures/c07/a.json", "contracts/generated/api-client/nested/a.json"):
            self.assertFalse(coverage.legacy_path(ROOT / name))
        for name in ("contracts/fixtures/c07-other/a.json", "contracts/generated/api-client-other/a.json",
                     "contracts/fixtures/domain/c07/a.json", "contracts/core/v1/core.schema.json"):
            self.assertTrue(coverage.legacy_path(ROOT / name))

    def test_non_c07_unclassified_files_still_fail_discovery(self):
        import tempfile
        with tempfile.TemporaryDirectory(prefix="mcr-c07-qa01-") as directory:
            root = Path(directory)
            for name in ("contracts/fixtures/c07/catalog.json", "contracts/fixtures/c07/unknown.txt",
                         "contracts/fixtures/domain/valid.json"):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("{}")
            with patch.object(coverage, "ROOT", root):
                self.assertEqual(coverage.fixture_paths(), [root / "contracts/fixtures/domain/valid.json"])
                # A new canonical JSON input remains visible and therefore fails
                # the unchanged exact pins comparison in the ordinary run.
                canonical = root / "contracts/fixtures/domain/new.json"
                canonical.write_text("{}")
                self.assertIn(canonical, coverage.fixture_paths())
                bad = root / "contracts/fixtures/c07-other/unknown.txt"
                bad.parent.mkdir(parents=True)
                bad.write_text("unclassified")
                with self.assertRaisesRegex(ValueError, "unclassified non-JSON fixture file"):
                    coverage.fixture_paths()



class C07MotionCoverageTests(unittest.TestCase):
    def test_candidate_and_reviewed_fixtures_are_explicitly_classified(self):
        motion = coverage.load("qa01_motion_regression", "scripts/mcr/contracts/c07/motion_coverage.py")
        fixtures = {coverage.relative(p): coverage.read(p) for p in
                    (ROOT / "contracts/fixtures/assets/motion-v2").rglob("*.json")}
        inventory = []
        motion.check_motion_fixtures(ROOT, coverage.read, fixtures, inventory)
        self.assertEqual({row["file"] for row in inventory}, set(fixtures))
        for qualification in ("candidate_SCHEMA_ONLY", "reviewed_SCHEMA_ONLY"):
            rows = [r for r in inventory if r["qualification"] == qualification]
            self.assertEqual(sum(r["expected"] == "accept" for r in rows), 15)
            self.assertEqual(sum(r["expected"] == "reject" for r in rows), 16)
            positive = next(r["file"] for r in rows if r["expected"] == "accept")
            negative = next(r["file"] for r in rows if r["expected"] == "reject")
            tampered = dict(fixtures); tampered[positive] = {"__unknown": True}
            with self.assertRaisesRegex(ValueError, "expectation mismatch"):
                motion.check_motion_fixtures(ROOT, coverage.read, tampered, [])
            tampered = dict(fixtures); tampered[negative] = fixtures[positive]
            with self.assertRaisesRegex(ValueError, "expectation mismatch"):
                motion.check_motion_fixtures(ROOT, coverage.read, tampered, [])

    def test_root_only_schemas_are_checked_without_definitions(self):
        motion = coverage.load("qa01_motion_roots", "scripts/mcr/contracts/c07/motion_coverage.py")
        documents = [coverage.read(p) for p in motion.motion_schema_paths(ROOT)]
        self.assertEqual(len(documents), 4)
        self.assertEqual(sum("$defs" not in d for d in documents), 2)
        self.assertGreater(coverage.check_schemas(documents), 0)
        broken = dict(documents[0]); broken["type"] = "UNKNOWN_TYPE"
        from jsonschema.exceptions import SchemaError
        with self.assertRaises(SchemaError): coverage.check_schemas([broken])


if __name__ == "__main__":
    unittest.main(verbosity=2)
