#!/usr/bin/env python3
"""Harness refusal/cleanup checks; no database, network or application proof."""
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import friend_invalidation_smoke as smoke
import migration_smoke as migration

class HarnessTests(unittest.TestCase):
    def test_existing_directory_refused(self):
        with tempfile.TemporaryDirectory() as name:
            with self.assertRaises(FileExistsError):
                migration.Cluster({}, Path(name))
            self.assertTrue(Path(name).is_dir())

    def test_replaced_marker_refuses_cleanup(self):
        cluster = migration.Cluster({})
        marker = cluster.marker.read_bytes()
        try:
            cluster.marker.write_text('{}')
            with self.assertRaises(migration.SmokeError):
                cluster.cleanup()
            self.assertTrue(cluster.root.exists())
        finally:
            cluster.marker.write_bytes(marker)
            cluster.cleanup()
        self.assertFalse(cluster.root.exists())

    def test_target_binary_refused_even_when_executable(self):
        with tempfile.TemporaryDirectory() as name:
            binary = Path(name) / 'target' / 'debug' / 'test'
            binary.parent.mkdir(parents=True)
            binary.write_text('not executed')
            binary.chmod(0o700)
            with self.assertRaises(smoke.CheckFailure):
                smoke.frozen_executable(binary)

    def test_relative_binary_refused(self):
        with self.assertRaises(smoke.CheckFailure):
            smoke.frozen_executable(Path('test'))

    def test_failure_still_cleans_owned_cluster(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            binary = root / 'binary'
            binary.write_text('not executed')
            binary.chmod(0o700)
            tools = root / 'pg'
            tools.mkdir()
            for tool in migration.PG_TOOLS:
                p = tools / tool
                p.write_text('not executed')
                p.chmod(0o700)
            report = root / 'report.json'
            with patch.object(smoke, 'exercise', side_effect=smoke.CheckFailure('forced-before-init')):
                self.assertEqual(smoke.main(['--binary', str(binary), '--repository-test', str(binary), '--relationship-test', str(binary), '--pg-bin-dir', str(tools), '--report', str(report)]), 1)
            result = json.loads(report.read_text())
            self.assertEqual(result['cleanup'], 'PASS')
            self.assertFalse(Path(result['owned_directory']).exists())
            self.assertEqual(result['failure'], 'forced-before-init')
            self.assertFalse(result['secrets_recorded'])

if __name__ == '__main__':
    unittest.main()
