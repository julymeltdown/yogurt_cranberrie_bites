"""Refuse unbound peer artifacts before starting an owned process/database."""
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

import participant_live_join_smoke as smoke


class PeerArtifactBoundary(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="mcr-peer-boundary-", dir="/private/tmp")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.metadata = self.root / "metadata.json"
        self.content = b"committed immutable source\n"
        self.source = {"source_commit": "a" * 40,
                       "source_sha256": {"backend/owned.rs": hashlib.sha256(self.content).hexdigest()}}
        self.binary = self.root / "native"
        self.binary.write_bytes(b"explicit native artifact fixture")
        self.binary.chmod(0o700)
        self.record = {"path": str(self.binary), "sha256": smoke.runtime.digest(self.binary)}

    def snapshot(self, payload=None):
        self.metadata.write_text(json.dumps(self.source))
        if payload is None:
            payload = b"b" * 40 + b" blob " + str(len(self.content)).encode() + b"\n" + self.content + b"\n"
        with patch.object(smoke.subprocess, "run", return_value=SimpleNamespace(stdout=payload)), \
             patch.object(smoke.migration, "Cluster") as cluster:
            result = smoke.snapshot(self.metadata)
            cluster.assert_not_called()
            return result

    def test_exact_committed_snapshot_accepts_without_a_database(self):
        self.assertEqual(self.snapshot()[1], "a" * 40)

    def test_qualified_compile_pin_is_used_instead_of_later_publication_pin(self):
        self.source["compiled_source_commit"] = "c" * 40
        self.assertEqual(self.snapshot()[1], "c" * 40)

    def test_missing_or_modified_source_cannot_qualify(self):
        for payload in [b"a:missing missing\n", b"b" * 40 + b" blob 5\nother\n"]:
            with self.subTest(payload=payload):
                with self.assertRaises(smoke.migration.SmokeError):
                    self.snapshot(payload)

    def test_bad_revision_or_injected_source_path_is_refused_before_git(self):
        for key, value in [("source_commit", "a" * 40 + "\nHEAD"),
                           ("source_sha256", {"/absolute.rs": "b" * 64}),
                           ("source_sha256", {"../foreign.rs": "b" * 64}),
                           ("source_sha256", {"owned\nHEAD:foreign": "b" * 64})]:
            original = self.source[key]
            self.source[key] = value
            self.metadata.write_text(json.dumps(self.source))
            with patch.object(smoke.subprocess, "run") as command:
                with self.assertRaises(smoke.migration.SmokeError):
                    smoke.snapshot(self.metadata)
                command.assert_not_called()
            self.source[key] = original

    def test_modified_or_symlink_peer_binary_is_refused(self):
        self.assertEqual(smoke.copied_binary(self.record), self.binary)
        self.binary.write_bytes(b"changed after qualification")
        with self.assertRaises(smoke.migration.SmokeError):
            smoke.copied_binary(self.record)
        link = self.root / "alias"
        link.symlink_to(self.binary)
        with self.assertRaises(smoke.migration.SmokeError):
            smoke.copied_binary({"path": str(link), "sha256": smoke.runtime.digest(self.binary)})


if __name__ == "__main__":
    unittest.main()
