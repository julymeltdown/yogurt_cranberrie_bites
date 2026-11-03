"""Auth admission RPC harness refusal, cleanup and private failure receipts."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch
import auth_admission_rpc_smoke as smoke

class HarnessTests(unittest.TestCase):
    def fake_cluster(self):
        cluster = Mock()
        cluster.root.exists.return_value = False
        return cluster
    def test_existing_report_is_never_overwritten_or_started(self):
        with tempfile.TemporaryDirectory() as folder:
            report=Path(folder)/"receipt.json";report.write_text("preserved")
            with patch.object(smoke,"run") as run,contextlib.redirect_stderr(io.StringIO()),self.assertRaises(SystemExit):
                smoke.main(["--binary","/missing","--rpc-test","/missing","--owner-test","/missing","--pg-bin","/missing","--report",str(report)])
            run.assert_not_called();self.assertEqual(report.read_text(),"preserved")
    def test_each_relative_input_is_refused_before_cluster_creation(self):
        for values in [("relative","/test","/pg"),("/binary","relative","/pg"),("/binary","/test","relative")]:
            report={}
            with patch.object(smoke.migration,"Cluster") as cluster:
                smoke.run(*(Path(p) for p in values),report)
            cluster.assert_not_called();self.assertEqual(report["result"],"FAIL")
    def test_exception_and_interrupt_cleanup_are_sanitized(self):
        for error in (RuntimeError("private-token"),KeyboardInterrupt()):
            cluster=self.fake_cluster();report={}
            with patch.object(smoke.migration,"executable",side_effect=lambda p:p),patch.object(smoke.migration,"Cluster",return_value=cluster),patch.object(smoke,"digest",return_value="hash"),patch.object(smoke,"exercise",side_effect=error):
                smoke.run(Path("/binary"),Path("/test"),Path("/pg"),report)
            cluster.cleanup.assert_called_once();self.assertEqual(report["cleanup"],"PASS");self.assertNotIn("private-token",json.dumps(report))
    def test_cleanup_failure_or_remaining_directory_revokes_success(self):
        for remains in (False,True):
            cluster=self.fake_cluster();cluster.root.exists.return_value=remains
            if not remains:cluster.cleanup.side_effect=RuntimeError("private-token")
            report={}
            def passed(*args):report["result"]="PASS"
            with patch.object(smoke.migration,"executable",side_effect=lambda p:p),patch.object(smoke.migration,"Cluster",return_value=cluster),patch.object(smoke,"digest",return_value="hash"),patch.object(smoke,"exercise",side_effect=passed):
                smoke.run(Path("/binary"),Path("/test"),Path("/pg"),report)
            self.assertEqual((report["result"],report["cleanup"]),("FAIL","FAIL"))
    def test_metadata_failure_always_finalizes_private_failure_report(self):
        with tempfile.TemporaryDirectory() as folder:
            report=Path(folder)/"receipt.json"
            with patch.object(smoke.migration,"executable",side_effect=lambda p:p),patch.object(smoke,"digest",side_effect=OSError("private-token")),contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(smoke.main(["--binary","/missing","--rpc-test","/missing","--owner-test","/missing","--pg-bin","/missing","--report",str(report)]),1)
            value=json.loads(report.read_text());self.assertEqual(value["failure"],"OSError");self.assertNotIn("private-token",report.read_text());self.assertEqual(report.stat().st_mode & 0o777,0o600)

    def test_symlink_report_is_not_followed(self):
        with tempfile.TemporaryDirectory() as folder:
            target=Path(folder)/"original";target.write_text("preserved")
            link=Path(folder)/"receipt.json";link.symlink_to(target)
            with patch.object(smoke,"run") as run,contextlib.redirect_stderr(io.StringIO()),self.assertRaises(SystemExit):
                smoke.main(["--binary","/missing","--rpc-test","/missing","--owner-test","/missing","--pg-bin","/missing","--report",str(link)])
            run.assert_not_called();self.assertEqual(target.read_text(),"preserved")


class TestOutputTests(unittest.TestCase):
    def test_summary_requires_exact_count_zero_failures_and_no_ignored(self):
        for raw in [b"test result: ok. 0 passed; 0 failed; 5 ignored;",b"test result: ok. 4 passed; 0 failed; 0 ignored;",b"test result: ok. 5 passed; 0 failed; 1 ignored;",b"test result: ok. 15 passed; 0 failed; 0 ignored;",b"not a Rust test result"]:
            with self.assertRaises(Exception):smoke.require_test_summary(raw,5)
        smoke.require_test_summary(b"test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured;",5)
    def test_native_count_comes_from_actual_test_listing(self):
        import subprocess
        result=subprocess.CompletedProcess([],0,b"native_one: test\ndeterministic_two: test\nnative_three: test\n",b"")
        with patch.object(smoke,"command",return_value=result):
            self.assertEqual(smoke.native_names(Path("/test"),{},Path("/owned"),(),{}),["native_one","native_three"])
        for raw in [b"deterministic_only: test\n",b"native_one: test\nnative_one: test\n"]:
            with patch.object(smoke,"command",return_value=subprocess.CompletedProcess([],0,raw,b"")),self.assertRaises(Exception):
                smoke.native_names(Path("/test"),{},Path("/owned"),(),{})

if __name__=="__main__":unittest.main()
