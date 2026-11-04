#!/usr/bin/env python3
"""Frozen runner preflight checks. No PostgreSQL, Cargo or target access."""
import json
import os
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

    def test_native_configuration_must_be_private_and_explicit(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory).resolve()/"native-url"
            path.write_text("postgresql://localhost/feed_native");path.chmod(0o644)
            with self.assertRaises(smoke.PreflightError):smoke.native_environment(path)
            path.chmod(0o600);path.write_text("postgresql://localhost/feed_native\nextra")
            with self.assertRaises(smoke.PreflightError):smoke.native_environment(path)

    def test_native_missing_inventory_never_executes_a_case(self):
        listed=subprocess.CompletedProcess([],0,stdout="0 tests, 0 benchmarks\n")
        with patch.object(smoke.subprocess,"run",return_value=listed) as run:
            with self.assertRaises(smoke.PreflightError):smoke.native_checks(Path("/tmp/frozen-native"),{})
            self.assertEqual(run.call_count,1)

    def test_native_replaced_controller_identity_and_foreign_socket_are_refused(self):
        from urllib.parse import urlencode
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve();socket=root/"socket";socket.mkdir(mode=0o700)
            token="a"*48
            (root/"mcr-owner.json").write_text(json.dumps({"token":token,"uid":os.getuid()}))
            info=root.stat();path=root/"config.json"
            value={"root":str(root),"identity":[info.st_dev,info.st_ino],"token":token,
                   "url":"postgresql://mcr_owner:fixture@localhost/mcr_feed_native?"+urlencode({"host":str(socket),"port":5432,"sslmode":"disable"})}
            path.write_text(json.dumps(value));path.chmod(0o600)
            self.assertEqual(smoke.native_environment(path)["MCR_FEED_NATIVE_OWNER_ROOT"],str(root))
            for changes in [{"identity":[info.st_dev,info.st_ino+1]}, {"token":"b"*48},
                            {"url":"postgresql://mcr_owner:fixture@localhost/mcr_feed_native?host=127.0.0.1&port=5432&sslmode=disable"}]:
                path.write_text(json.dumps({**value,**changes}))
                with self.assertRaises(smoke.PreflightError):smoke.native_environment(path)

    def test_native_filtered_or_ignored_case_cannot_qualify_storage(self):
        listed=subprocess.CompletedProcess([],0,stdout="\n".join(f"{name}: test" for name in smoke.NATIVE_CASES))
        for result in ["test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 100 filtered out;",
                       "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 100 filtered out;"]:
            run_result=subprocess.CompletedProcess([],0,stdout=result)
            with patch.object(smoke.subprocess,"run",side_effect=[listed,run_result]):
                with self.assertRaises(smoke.PreflightError):smoke.native_checks(Path("/tmp/frozen-native"),{})

if __name__ == "__main__":unittest.main()
