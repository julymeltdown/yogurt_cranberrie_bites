#!/usr/bin/env python3
"""Small filesystem/process fixtures only; never opens the shared Cargo target."""
import errno
import json
import os
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import gift_public_build as build
import gift_public_guard as guard


class AllocatedBytesTests(unittest.TestCase):
    def test_allocated_inode_union_without_following_symlink(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            child = root / "child"
            child.mkdir()
            data = child / "data"
            data.write_bytes(b"x" * 8192)
            os.link(data, root / "hardlink")
            link = root / "external"
            link.symlink_to("/not-a-readable-fixture-target")
            expected = sum(p.lstat().st_blocks * 512 for p in (root, child, data, link))
            self.assertEqual(guard.allocated_bytes(root), expected)

    def test_disappeared_enumerated_child_is_tolerated(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            (root / "rotated").write_bytes(b"x" * 8192)
            real_stat = os.stat

            def removed(path, *args, **kwargs):
                if path == "rotated" and "dir_fd" in kwargs:
                    raise FileNotFoundError(errno.ENOENT, "concurrent unlink")
                return real_stat(path, *args, **kwargs)

            with patch.object(guard.os, "stat", side_effect=removed):
                self.assertEqual(guard.allocated_bytes(root), root.lstat().st_blocks * 512)

    def test_permission_failure_is_not_treated_as_rotation(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            (root / "denied").touch()
            with patch.object(guard.os, "stat", side_effect=PermissionError(errno.EACCES, "denied")):
                with self.assertRaises(PermissionError):
                    guard.allocated_bytes(root)

    def test_missing_root_fails_closed(self):
        with tempfile.TemporaryDirectory() as name:
            with self.assertRaises(FileNotFoundError):
                guard.allocated_bytes(Path(name) / "missing")

    def test_symlink_root_fails_closed(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            (root / "link").symlink_to(root, target_is_directory=True)
            with self.assertRaises(ValueError):
                guard.allocated_bytes(root / "link")

    def test_replaced_root_fails_closed(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            original = os.lstat
            calls = 0

            def changed(path, *args, **kwargs):
                nonlocal calls
                result = original(path, *args, **kwargs)
                if path == str(root):
                    calls += 1
                    if calls == 2:
                        return SimpleNamespace(st_mode=result.st_mode, st_dev=result.st_dev,
                                               st_ino=result.st_ino + 1)
                return result

            with patch.object(guard.os, "lstat", side_effect=changed):
                with self.assertRaisesRegex(ValueError, "replaced"):
                    guard.allocated_bytes(root)

    def test_sample_deadline_fails_closed(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            (root / "child").touch()
            with patch.object(guard.time, "monotonic", side_effect=[0, 6]):
                with self.assertRaises(TimeoutError):
                    guard.allocated_bytes(root)


class CleanupTests(unittest.TestCase):
    def test_permission_error_is_retained_and_direct_child_is_reaped(self):
        process = unittest.mock.Mock(pid=123)
        remaining = unittest.mock.Mock(side_effect=[[123], [], []])
        signal_group = unittest.mock.Mock(side_effect=PermissionError(errno.EPERM, "denied"))
        errors = build.stop_owned_process_group(process, remaining, signal_group)
        self.assertEqual(len(errors), 1)
        self.assertIn("PermissionError", errors[0])
        process.wait.assert_called_once_with(timeout=5)

    def test_reap_timeout_is_recorded(self):
        process = unittest.mock.Mock(pid=123)
        process.wait.side_effect = subprocess.TimeoutExpired("fixture", 5)
        errors = build.stop_owned_process_group(process, lambda: [])
        self.assertIn("TimeoutExpired", errors[0])

    def test_guardian_failed_exit_retains_failure_details(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            instance = guard.IndependentGiftGuardian.__new__(guard.IndependentGiftGuardian)
            instance.process = unittest.mock.Mock()
            instance.process.wait.return_value = 1
            instance.stop_path = root / "stop"
            instance.result_path = root / "result"
            instance.result_path.write_text(json.dumps({"result": "FAIL", "failure": "disk sample"}))
            result = instance.stop()
            self.assertEqual(result["result"], "FAIL")
            self.assertEqual(result["failure"], "disk sample")
            self.assertEqual(result["guardian_exit_code"], 1)


if __name__ == "__main__":
    unittest.main()
