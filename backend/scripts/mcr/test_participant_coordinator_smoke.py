"""Artifact refusal boundaries; never starts a database or compiler."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import participant_coordinator_sql_smoke as smoke


class NativeArtifactBoundary(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.binary = self.root / "private" / "native-tests"
        self.binary.parent.mkdir()
        self.binary.write_bytes(b"explicit test fixture")
        self.binary.chmod(0o700)
        self.receipt = self.root / "artifact.json"
        self.value = {
            "copy": str(self.binary), "sha256": hashlib.sha256(self.binary.read_bytes()).hexdigest(),
            "source_commit": "a" * 40, "source_sha256": {"owned.rs": "b" * 64},
            "compile_exit": 0, "rustc": "1.88.0", "incremental": "0", "debug": "0",
            "jobs": 2, "locked": True, "offline": True,
            "artifact": {"reason": "compiler-artifact", "target": {"name": "concert_service", "kind": ["lib"]},
                         "profile": {"test": True},
                         "manifest_path": str(self.root / "backend/services/concert-service/Cargo.toml"),
                         "executable": str(self.root / "build/debug/deps/concert_service-exact")}}

    def validate(self):
        self.receipt.write_text(json.dumps(self.value))
        with patch.object(smoke, "required_sources", return_value={"owned.rs": "b" * 64}), \
             patch.object(smoke.subprocess, "check_output", return_value="a" * 40 + "\n"):
            return smoke.validate_binary(self.binary, self.receipt, self.root)

    def test_exact_private_copy_is_accepted_without_starting_pg(self):
        with patch.object(smoke.migration, "Cluster") as cluster:
            self.assertEqual(self.validate()["compile_exit"], 0)
            cluster.assert_not_called()

    def test_modified_binary_is_refused_before_pg(self):
        self.binary.write_bytes(b"different binary")
        with patch.object(smoke.migration, "Cluster") as cluster:
            with self.assertRaises(smoke.migration.SmokeError):
                self.validate()
            cluster.assert_not_called()

    def test_stale_commit_source_or_wrong_build_is_refused(self):
        for key, wrong in [("source_commit", "c" * 40), ("source_sha256", {}),
                           ("compile_exit", 101), ("rustc", "stable"),
                           ("incremental", "1"), ("debug", "2"), ("jobs", 8),
                           ("locked", False), ("offline", False)]:
            with self.subTest(key=key):
                original = self.value[key]
                self.value[key] = wrong
                with self.assertRaises(smoke.migration.SmokeError):
                    self.validate()
                self.value[key] = original

    def test_other_target_or_shared_binary_is_refused(self):
        for key, wrong in [("target", {"name": "concert_service", "kind": ["bin"]}),
                           ("manifest_path", "/foreign/Cargo.toml"), ("profile", {"test": False}),
                           ("executable", str(self.binary.parent / "debug/deps/not-copied"))]:
            with self.subTest(key=key):
                original = self.value["artifact"][key]
                self.value["artifact"][key] = wrong
                with self.assertRaises(smoke.migration.SmokeError):
                    self.validate()
                self.value["artifact"][key] = original


if __name__ == "__main__":
    unittest.main()
