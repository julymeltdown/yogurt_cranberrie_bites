import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import owner_media_live_smoke as smoke

class ArtifactBoundary(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.binary = self.root / "private-test"
        self.binary.write_bytes(b"test binary")
        self.source = self.root / "source.rs"
        self.source.write_text("pinned source")
        self.receipt = self.root / "receipt.json"
        self.value = {"exit_code": 0, "private_artifact": str(self.binary),
                      "artifact_sha256": smoke.digest(self.binary),
                      "cargo_artifact": {"reason": "compiler-artifact", "target": {"name": "owner_media_live"}, "profile": {"test": True}, "executable": "/build/owner_media_live-exact"},
                      "rustc_version": "rustc 1.88.0 (fixture)", "incremental": "0", "debug": "0", "jobs": 2,
                      "locked": True, "offline": True, "source_revision": "a" * 40,
                      "source_sha256": {"source.rs": smoke.digest(self.source)}}
    def validate(self):
        self.receipt.write_text(json.dumps(self.value))
        with patch.object(smoke, "required_sources", return_value={"source.rs": smoke.digest(self.source)}), patch.object(smoke.subprocess, "run") as run:
            run.return_value.returncode = 0
            return smoke.validate_artifact(self.binary, self.receipt, self.root)
    def test_accepts_exact_private_artifact_and_complete_source_manifest(self):
        self.assertEqual(self.validate()["exit_code"], 0)
    def test_refuses_stale_binary(self):
        self.binary.write_bytes(b"changed")
        with self.assertRaisesRegex(RuntimeError, "artifact digest differs"):
            self.validate()
    def test_refuses_missing_required_source_pins(self):
        self.value["source_sha256"] = {}
        with self.assertRaisesRegex(RuntimeError, "pins incomplete"):
            self.validate()
    def test_refuses_source_drift(self):
        self.source.write_text("changed source")
        with self.assertRaisesRegex(RuntimeError, "source changed"):
            self.validate()
    def test_refuses_wrong_test_artifact_and_build_constraints(self):
        for field, wrong in [("cargo_artifact", {}), ("exit_code", 1), ("incremental", "1"), ("jobs", 3), ("offline", False)]:
            with self.subTest(field=field):
                old = self.value[field]
                self.value[field] = wrong
                with self.assertRaises(RuntimeError):
                    self.validate()
                self.value[field] = old
    def test_refuses_checkout_escape(self):
        self.value["source_sha256"]["../escape"] = "irrelevant"
        with self.assertRaisesRegex(RuntimeError, "escapes checkout"):
            self.validate()

if __name__ == "__main__":
    unittest.main()
