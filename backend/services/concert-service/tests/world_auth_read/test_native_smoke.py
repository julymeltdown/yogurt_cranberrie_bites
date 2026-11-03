"""Fail-closed receipt/summary guards; never launches Cargo, binaries or PostgreSQL."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("world_auth_smoke", Path(__file__).with_name("native_smoke.py"))
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


class EvidenceGuards(unittest.TestCase):
    def receipt(self, rows):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "receipt.jsonl"
            path.write_text("\n".join(json.dumps(row) for row in rows))
            with self.assertRaises(Exception):
                smoke.artifacts(path)

    def test_failed_build_cannot_supply_artifacts(self):
        self.receipt([{"reason": "build-finished", "success": False}])

    def test_finished_build_without_exact_executables_is_not_evidence(self):
        self.receipt([{"reason": "build-finished", "success": True}] * 2)

    def test_foreign_worktree_artifact_is_refused(self):
        self.receipt([{"reason": "compiler-artifact", "executable": "/usr/bin/true",
                       "target": {"name": "concert_service", "kind": ["lib"]},
                       "profile": {"test": True}, "manifest_path": "/tmp/foreign/Cargo.toml"}])

    def test_missing_skipped_or_failed_native_tests_cannot_pass(self):
        for raw in (b"", b"test result: ok. 0 passed; 0 failed; 2 ignored;\n",
                    b"test result: ok. 1 passed; 0 failed; 0 ignored;\n",
                    b"test result: FAILED. 1 passed; 1 failed; 0 ignored;\n"):
            with self.assertRaises(Exception):
                smoke.summary(raw, 2)
        smoke.summary(b"test result: ok. 2 passed; 0 failed; 0 ignored;\n", 2)


if __name__ == "__main__":
    unittest.main()
