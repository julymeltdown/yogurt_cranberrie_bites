#!/usr/bin/env python3
"""Frozen runner preflight checks. No PostgreSQL, Cargo or target access."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import party_live_smoke as smoke


class AdapterRunnerTests(unittest.TestCase):
    def test_relative_metadata_is_refused(self):
        with self.assertRaises(smoke.PreflightError):
            smoke.artifacts(Path("metadata.json"))

    def test_symlink_metadata_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            actual = root / "actual.json"
            actual.write_text("{}")
            link = root / "link.json"
            link.symlink_to(actual)
            with self.assertRaises(smoke.PreflightError):
                smoke.artifacts(link)

    def test_other_scope_or_incomplete_inventory_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            metadata = Path(directory).resolve() / "metadata.json"
            for value in [
                {"scope": "live", "artifacts": {name: {} for name in smoke.REQUIRED}},
                {"scope": "adapter-only", "artifacts": {}},
            ]:
                metadata.write_text(json.dumps(value))
                with self.assertRaises(smoke.PreflightError):
                    smoke.artifacts(metadata)

    def test_changed_source_is_refused_before_binary_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            metadata = Path(directory).resolve() / "metadata.json"
            metadata.write_text(
                json.dumps(
                    {
                        "scope": "adapter-only",
                        "artifacts": {name: {} for name in smoke.REQUIRED},
                        "source_sha256": {"old": "hash"},
                    }
                )
            )
            with patch.object(smoke, "sources", return_value={"new": "hash"}):
                with self.assertRaises(smoke.PreflightError):
                    smoke.artifacts(metadata)

    def test_replaced_binary_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            binary = root / "binary"
            binary.write_bytes(b"changed")
            binary.chmod(0o700)
            metadata = root / "metadata.json"
            metadata.write_text(
                json.dumps(
                    {
                        "scope": "adapter-only",
                        "artifacts": {
                            name: {"path": str(binary), "sha256": "old"}
                            for name in smoke.REQUIRED
                        },
                        "source_sha256": {},
                    }
                )
            )
            with patch.object(smoke, "sources", return_value={}):
                with self.assertRaises(smoke.PreflightError):
                    smoke.artifacts(metadata)

    def test_filtered_or_ignored_cases_are_not_success(self):
        for output in [
            "test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out;",
            "test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out;",
            "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;",
        ]:
            with self.assertRaises(smoke.PreflightError):
                smoke.test_result(
                    subprocess.CompletedProcess([], 0, stdout=output),
                    3,
                )

    def test_native_runner_requires_frozen_binary_and_new_absolute_report(self):
        import party_live_native_smoke as native

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            binary = root / "binary"
            binary.write_bytes(b"frozen")
            binary.chmod(0o500)
            self.assertEqual(native.regular(binary, executable=True), binary)
            with self.assertRaises(RuntimeError):
                native.regular(Path("relative"), executable=True)
            binary.chmod(0o600)
            with self.assertRaises(RuntimeError):
                native.regular(binary, executable=True)
            with self.assertRaises(SystemExit):
                native.main(
                    [
                        "--test-binary",
                        str(binary),
                        "--pg-bin",
                        str(root),
                        "--report",
                        "relative.json",
                    ]
                )

    def test_native_runner_refuses_shared_target_path(self):
        import party_live_native_smoke as native

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            shared = root / "mcr-build" / "backend"
            shared.mkdir(parents=True)
            binary = shared / "party_live_native"
            binary.write_bytes(b"frozen")
            binary.chmod(0o700)
            with self.assertRaises(RuntimeError):
                native.regular(binary, executable=True)


if __name__ == "__main__":
    unittest.main()
