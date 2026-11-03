#!/usr/bin/env python3
"""Frozen runner preflight checks. No PostgreSQL, Cargo or target access."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import feed_live_read_smoke as smoke

class AdapterRunnerTests(unittest.TestCase):
    def test_relative_metadata_is_refused(self):
        with self.assertRaises(smoke.PreflightError):
            smoke.artifacts(Path("metadata.json"))

    def test_symlink_metadata_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve();actual=root/"actual.json";actual.write_text("{}")
            link=root/"link.json";link.symlink_to(actual)
            with self.assertRaises(smoke.PreflightError):smoke.artifacts(link)

    def test_other_scope_or_incomplete_inventory_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            metadata=Path(directory).resolve()/"metadata.json"
            for value in [{"scope":"live","artifacts":{name:{} for name in smoke.REQUIRED}},
                          {"scope":"adapter-only","artifacts":{}}]:
                metadata.write_text(json.dumps(value))
                with self.assertRaises(smoke.PreflightError):smoke.artifacts(metadata)

    def test_changed_source_is_refused_before_any_binary_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            metadata=Path(directory).resolve()/"metadata.json"
            metadata.write_text(json.dumps({"scope":"adapter-only","artifacts":{name:{} for name in smoke.REQUIRED},"source_sha256":{"old":"hash"}}))
            with patch.object(smoke,"sources",return_value={"new":"hash"}):
                with self.assertRaises(smoke.PreflightError):smoke.artifacts(metadata)

    def test_replaced_binary_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve();binary=root/"binary";binary.write_bytes(b"changed");binary.chmod(0o700)
            metadata=root/"metadata.json"
            metadata.write_text(json.dumps({"scope":"adapter-only","artifacts":{name:{"path":str(binary),"sha256":"old"} for name in smoke.REQUIRED},"source_sha256":{}}))
            with patch.object(smoke,"sources",return_value={}):
                with self.assertRaises(smoke.PreflightError):smoke.artifacts(metadata)

    def test_skips_filtered_cases_and_wrong_inventory_are_not_success(self):
        for output in ["test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out;",
                       "test result: ok. 8 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out;",
                       "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;"]:
            with self.assertRaises(smoke.PreflightError):
                smoke.test_result(subprocess.CompletedProcess([],0,stdout=output),8)

if __name__ == "__main__":unittest.main()
