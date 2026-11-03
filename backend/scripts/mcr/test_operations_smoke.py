"""Refusal and cleanup boundaries for the isolated BE36/38 harness."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock,patch
import operations_smoke as smoke

class HarnessTests(unittest.TestCase):
    def test_existing_report_not_overwritten_or_started(self):
        with tempfile.TemporaryDirectory() as folder:
            path=Path(folder)/"evidence.json";path.write_text("preserve")
            with patch.object(smoke,"run") as run,contextlib.redirect_stderr(io.StringIO()),self.assertRaises(SystemExit):
                smoke.main(["--binary","/missing","--report-test","/missing","--operations-test","/missing","--pg-bin","/missing","--report",str(path)])
            run.assert_not_called();self.assertEqual(path.read_text(),"preserve")
    def test_relative_binary_refused_before_cluster_creation(self):
        report={}
        with patch.object(smoke.migration,"Cluster") as cluster:
            smoke.run(Path("relative"),Path("/report"),Path("/operations"),Path("/pg"),report)
        cluster.assert_not_called();self.assertEqual(report["result"],"FAIL")
    def test_failure_and_interrupt_cleanup_hide_secrets(self):
        for error in (RuntimeError("private-token"),KeyboardInterrupt()):
            cluster=Mock();cluster.root.exists.return_value=False;report={}
            with patch.object(smoke.migration,"executable",side_effect=lambda p:p),patch.object(smoke.migration,"Cluster",return_value=cluster),patch.object(smoke,"digest",return_value="hash"),patch.object(smoke,"exercise",side_effect=error):
                smoke.run(Path("/service"),Path("/report"),Path("/operations"),Path("/pg"),report)
            cluster.cleanup.assert_called_once();self.assertEqual(report["cleanup"],"PASS");self.assertNotIn("private-token",json.dumps(report))
    def test_cleanup_failure_revokes_success(self):
        cluster=Mock();cluster.root.exists.return_value=True;report={}
        def passed(*_):report.update(result="PASS")
        with patch.object(smoke.migration,"executable",side_effect=lambda p:p),patch.object(smoke.migration,"Cluster",return_value=cluster),patch.object(smoke,"digest",return_value="hash"),patch.object(smoke,"exercise",side_effect=passed):
            smoke.run(Path("/service"),Path("/report"),Path("/operations"),Path("/pg"),report)
        self.assertEqual((report["result"],report["cleanup"]),("FAIL","FAIL"))
    def test_metadata_failure_finalizes_private_failure_report(self):
        with tempfile.TemporaryDirectory() as folder:
            path=Path(folder)/"evidence.json"
            with patch.object(smoke,"digest",side_effect=OSError("private-token")),contextlib.redirect_stdout(io.StringIO()):
                result=smoke.main(["--binary","/missing","--report-test","/missing","--operations-test","/missing","--pg-bin","/missing","--report",str(path)])
            self.assertEqual(result,1);self.assertEqual(path.stat().st_mode & 0o777,0o600);self.assertNotIn("private-token",path.read_text())

if __name__=="__main__":unittest.main()
