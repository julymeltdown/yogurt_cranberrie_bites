"""BE33 harness refusals. Native PostgreSQL evidence is a separate command."""
import os
from pathlib import Path
import tempfile
import unittest
import notification_smoke as smoke
import migration_smoke as migration


class SmokeSafety(unittest.TestCase):
    def test_binary_requires_matching_hash_and_regular_executable(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "test"
            path.write_text("fixture")
            path.chmod(0o700)
            cluster = migration.Cluster({})
            try:
                with self.assertRaisesRegex(RuntimeError, "hash-mismatch"):
                    smoke.pin_binary(path, "00" * 32, cluster, "copy")
                pinned = smoke.pin_binary(path, smoke.digest(path), cluster, "copy")
                self.assertEqual(smoke.digest(pinned), smoke.digest(path))
                link = Path(directory) / "link"
                link.symlink_to(path)
                with self.assertRaisesRegex(RuntimeError, "regular-executable"):
                    smoke.pin_binary(link, smoke.digest(path), cluster, "copy2")
            finally:
                cluster.cleanup()

    def test_owned_cluster_rejects_replaced_marker_and_foreign_url(self):
        cluster = migration.Cluster({})
        original = cluster.marker.read_text()
        try:
            cluster.marker.write_text("{}")
            with self.assertRaises(migration.SmokeError):
                cluster.assert_owned()
            cluster.marker.write_text(original)
            with self.assertRaises(migration.SmokeError):
                cluster.validate_url("postgresql://foreign@localhost/postgres", smoke.NOTIFICATION_DB)
            cluster.validate_url(cluster.url(smoke.NOTIFICATION_DB), smoke.NOTIFICATION_DB)
        finally:
            cluster.marker.write_text(original)
            cluster.cleanup()

    def test_no_arbitrary_database_fallback_or_skipped_native_mode(self):
        source = Path(smoke.__file__).read_text()
        self.assertIn('"both-real-test-binaries-required"', source)
        self.assertIn('"CONCERT_TEST_DATABASE_URL": cluster.url(database)', source)
        self.assertIn('"CONCERT_TEST_NOTIFICATION_DATABASE_URL": cluster.url(NOTIFICATION_DB)', source)
        self.assertNotIn('os.environ', source)
        self.assertNotIn('getenv', source)


if __name__ == "__main__":
    unittest.main()
