"""Negative bundle-admission tests. No target access, process or PG is allowed."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from moderation_live_support import run

class FrozenBundleAdmission(unittest.TestCase):
    def fixture(self, root):
        bundle=Path(root)/"bundle";bundle.mkdir(mode=0o700)
        binary=bundle/"fixture-not-executed";binary.write_bytes(b"never execute this test fixture");binary.chmod(0o700)
        migrations=bundle/"source/backend/services/concert-service/migrations";migrations.mkdir(parents=True)
        sql=migrations/"0043_moderation_runtime.sql";sql.write_text("-- deliberately inert preflight fixture\n")
        info=bundle.stat()
        pin={"root_device":info.st_dev,"root_inode":info.st_ino,"lease_id":"NEGATIVE_UNIT_FIXTURE_NOT_A_LEASE",
             "target":"/never-open-be39-target","test_binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),
             "source_commit":"0"*40,"migration_sha384":{"43":hashlib.sha384(sql.read_bytes()).hexdigest()}}
        return bundle,binary,pin

    def refused(self, mutation, expected):
        with tempfile.TemporaryDirectory(prefix="be39-preflight-") as root:
            bundle,binary,pin=self.fixture(root);mutation(pin)
            marker=bundle/"be39-build.json";marker.write_text(json.dumps(pin));marker.chmod(0o600)
            report={}
            with patch("moderation_live_support.migration.Cluster", side_effect=AssertionError("PG forbidden in negative test")) as cluster:
                run(binary,Path(pin["target"]),bundle,Path("/never-open-pg-bin"),"NEGATIVE_UNIT_FIXTURE_NOT_A_LEASE",report)
                cluster.assert_not_called()
            self.assertEqual(report["result"],"FAIL")
            self.assertEqual(report["failure"],expected)
            self.assertNotIn("owned_directory",report)

    def test_replaced_bundle_inode_refused_before_any_cluster(self):
        self.refused(lambda p:p.update(root_inode=p["root_inode"]+1),"frozen-lease-binding-mismatch")

    def test_altered_executable_refused_before_any_cluster(self):
        self.refused(lambda p:p.update(test_binary_sha256="0"*64),"frozen-executable-hash-mismatch")

    def test_altered_migration_snapshot_refused_before_any_cluster(self):
        self.refused(lambda p:p.update(migration_sha384={}),"frozen-migration-hash-mismatch")
