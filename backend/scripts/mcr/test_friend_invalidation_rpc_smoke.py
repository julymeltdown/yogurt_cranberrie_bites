#!/usr/bin/env python3
"""Focused harness preflight tests; never native transport acceptance."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import friend_invalidation_rpc_smoke as smoke

class RpcHarnessTests(unittest.TestCase):
    def test_relative_metadata_refused(self):
        with self.assertRaises(smoke.migration.SmokeError):
            smoke.binaries(Path('metadata.json'))

    def test_wrong_inventory_refused(self):
        with tempfile.TemporaryDirectory() as name:
            path = Path(name) / 'meta.json'
            path.write_text(json.dumps({'artifacts': {}}))
            with self.assertRaises(smoke.migration.SmokeError):
                smoke.binaries(path)

    def test_symlink_metadata_refused(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            actual = root / 'actual.json'
            actual.write_text('{}')
            path = root / 'meta.json'
            path.symlink_to(actual)
            with self.assertRaises(smoke.migration.SmokeError):
                smoke.binaries(path)

    def test_dirty_checkpoint_refused(self):
        with tempfile.TemporaryDirectory() as name:
            path = Path(name) / 'meta.json'
            path.write_text(json.dumps({'artifacts': {name: {} for name in smoke.REQUIRED}, 'source_commit': 'commit'}))
            with patch.object(smoke.subprocess, 'check_output', side_effect=['commit', 'modified']):
                with self.assertRaises(smoke.migration.SmokeError):
                    smoke.binaries(path)

    def test_owned_cleanup_on_native_failure(self):
        created = []
        original = smoke.migration.Cluster
        class RecordingCluster(original):
            def __init__(self, tools):
                super().__init__(tools)
                created.append(self.root)
            def init(self):
                raise smoke.migration.SmokeError('fixture', 'forced-before-init')
        with patch.object(smoke.migration, 'Cluster', RecordingCluster), patch.object(smoke.migration, 'executable', side_effect=lambda p: p):
            report = {}
            with self.assertRaises(smoke.migration.SmokeError):
                smoke.qualify({}, Path('/fixture-never-executed'), report)
        self.assertFalse(created[0].exists())
        self.assertEqual(report['cleanup']['status'], 'PASS')

if __name__ == '__main__':
    unittest.main()
