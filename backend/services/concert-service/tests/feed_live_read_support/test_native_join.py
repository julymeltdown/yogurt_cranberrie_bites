"""Negative controller preflights; never start PostgreSQL or access Cargo targets."""
import hashlib
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

path = Path(__file__).resolve().with_name("native_join.py")
spec = importlib.util.spec_from_file_location("feed_native_join_preflight", path)
join = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = join
spec.loader.exec_module(join)


class NativeJoinPreflights(unittest.TestCase):
    def checkpoint(self):
        return {"scope": "feed-native-owner-join", "source_state": "clean",
                "source_commit": "a" * 40, "source_sha256": {}}

    def test_dirty_or_other_scope_cannot_create_a_cluster(self):
        for changes in [{"source_state": "dirty"}, {"scope": "adapter-only"}]:
            with self.subTest(changes=changes), patch.object(join, "Cluster") as cluster:
                with self.assertRaises(join.SmokeError):
                    join.qualify({**self.checkpoint(), **changes}, Path("/nonexistent"), {})
                cluster.assert_not_called()

    def test_changed_source_cannot_create_a_cluster(self):
        with patch.object(join, "source_manifest", return_value={"changed.rs": "new"}), \
             patch.object(join, "Cluster") as cluster:
            with self.assertRaises(join.SmokeError):
                join.qualify(self.checkpoint(), Path("/nonexistent"), {})
            cluster.assert_not_called()

    def test_substituted_executable_cannot_create_a_cluster(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory).resolve() / "copied-library"
            original = b"original compiled checkpoint"
            binary.write_bytes(b"substituted executable")
            binary.chmod(0o700)
            checkpoint = {**self.checkpoint(), "binary": str(binary),
                          "binary_sha256": hashlib.sha256(original).hexdigest()}
            with patch.object(join, "source_manifest", return_value={}), \
                 patch.object(join, "Cluster") as cluster:
                with self.assertRaises(join.SmokeError):
                    join.qualify(checkpoint, Path("/nonexistent"), {})
                cluster.assert_not_called()


if __name__ == "__main__":
    unittest.main()
