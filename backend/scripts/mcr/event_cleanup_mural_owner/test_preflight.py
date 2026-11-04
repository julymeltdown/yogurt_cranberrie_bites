"""Negative admission fixtures; no executable, Cargo or PostgreSQL is launched."""
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import freeze
import run


class FrozenAdmission(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="mcr-mural-owner-preflight-", dir="/private/tmp")
        self.root = Path(self.temp.name)
        (self.root / "bin").mkdir()
        (self.root / "migrations").mkdir()
        self.exe = self.root / "bin/event_cleanup_mural_owner"
        self.exe.write_bytes(b"explicit negative test fixture")
        (self.root / "migrations/0019.sql").write_text("-- fixture only\n")
        (self.root / "cargo.jsonl").write_text("{}\n")
        st = self.exe.stat()
        receipt = {
            "accepted_offer": "negative-fixture",
            "binary": {"sha256": freeze.sha(self.exe), "size": st.st_size, "device": st.st_dev, "inode": st.st_ino},
            "migrations": {"0019.sql": freeze.sha(self.root / "migrations/0019.sql")},
            "cargo_json_sha256": freeze.sha(self.root / "cargo.jsonl"),
        }
        (self.root / "freeze.json").write_text(json.dumps(receipt))

    def tearDown(self):
        self.temp.cleanup()

    def refused_before_cluster(self):
        with patch.object(sys, "argv", ["run.py", "--frozen", str(self.root), "--accepted-offer", "negative-fixture"]), patch.object(run.tempfile, "mkdtemp", side_effect=AssertionError("cluster must not be constructed")) as cluster:
            with self.assertRaises(AssertionError):
                run.main()
            cluster.assert_not_called()

    def test_replaced_inode(self):
        # Retain old inode while creating replacement, avoiding inode reuse.
        self.exe.rename(self.exe.with_suffix(".old"))
        self.exe.write_bytes(b"explicit negative test fixture")
        self.refused_before_cluster()

    def test_changed_executable(self):
        self.exe.write_bytes(b"changed negative test fixture")
        self.refused_before_cluster()

    def test_changed_migration(self):
        (self.root / "migrations/0019.sql").write_text("-- altered fixture\n")
        self.refused_before_cluster()


if __name__ == "__main__":
    unittest.main()
