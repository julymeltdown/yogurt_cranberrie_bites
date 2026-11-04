"""Source-only guardian checks. These start no Cargo, PostgreSQL or browser."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from resource_guard import NativeBudget
from migration_smoke import SmokeError

class OwnedDirectory:
    def __init__(self, root):
        self.root=Path(root).resolve()
        self.identity=self.root.stat().st_dev,self.root.stat().st_ino
    def assert_owned(self):
        stat=self.root.stat()
        if self.identity!=(stat.st_dev,stat.st_ino):raise RuntimeError("replaced owned directory")

class NativeResourceGuard(unittest.TestCase):
    def test_requires_fresh_explicit_limits_and_current_floor(self):
        for deadline,floor,ceiling in [(time.time()-1,16*1024**3,1024**3),(time.time()+60,8*1024**3,1024**3),(time.time()+60,16*1024**3,1)]:
            with self.assertRaises(SmokeError):NativeBudget("accepted-test",deadline,floor,ceiling)
        budget=NativeBudget("accepted-test",time.time()+60,16*1024**3,1024**3)
        with patch("resource_guard.shutil.disk_usage",return_value=SimpleNamespace(free=15*1024**3)):
            with self.assertRaises(SmokeError):budget.check()
        self.assertEqual(budget.commands,[])

    def test_floor_breach_stops_only_owned_live_process_and_retains_failure(self):
        peer=subprocess.Popen([sys.executable,"-c","import time;time.sleep(30)"],start_new_session=True)
        try:
            with tempfile.TemporaryDirectory(prefix="wall-guardian-test-") as root:
                cluster=OwnedDirectory(root)
                budget=NativeBudget("accepted-test",time.time()+60,16*1024**3,1024**3)
                observations=[SimpleNamespace(free=n*1024**3) for n in (32,32,15)]
                with patch("resource_guard.shutil.disk_usage",side_effect=observations):
                    with self.assertRaises(SmokeError):
                        budget.execute([sys.executable,"-c","import time;print('owned-child-started',flush=True);time.sleep(30)"],cluster=cluster,env={"PATH":"/usr/bin:/bin"},phase="floor-breach",cwd=cluster.root)
                self.assertIsNone(peer.poll())
                self.assertEqual(len(budget.commands),1)
                receipt=budget.commands[0]
                self.assertLess(receipt["exit_code"],0)
                self.assertEqual(receipt["guardian_failure"]["phase"],"resource")
                self.assertIn("owned-child-started",receipt["stdout"])
        finally:
            peer.terminate();peer.wait(timeout=3)

if __name__=="__main__":unittest.main()
