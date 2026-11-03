"""Owner relationship harness refusal, cleanup and private failure receipts."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch
import owner_relationship_smoke as smoke

class HarnessTests(unittest.TestCase):
    def fake_cluster(self):
        cluster = Mock()
        cluster.root.exists.return_value = False
        return cluster
    def test_existing_report_is_never_overwritten_or_started(self):
        with tempfile.TemporaryDirectory() as folder:
            report=Path(folder)/"receipt.json";report.write_text("preserved")
            with patch.object(smoke,"run") as run,contextlib.redirect_stderr(io.StringIO()),self.assertRaises(SystemExit):
                smoke.main(["--cargo-json","/missing","--pg-bin","/missing","--report",str(report)])
            run.assert_not_called();self.assertEqual(report.read_text(),"preserved")
    def test_each_relative_input_is_refused_before_cluster_creation(self):
        for values in [("relative","/pg"),("/json","relative")]:
            report={}
            with patch.object(smoke.migration,"Cluster") as cluster:
                smoke.run(*(Path(p) for p in values),report)
            cluster.assert_not_called();self.assertEqual(report["result"],"FAIL")
    def test_exception_and_interrupt_cleanup_are_sanitized(self):
        for error in (RuntimeError("private-token"),KeyboardInterrupt()):
            cluster=self.fake_cluster();report={}
            with patch.object(smoke,"build_artifacts",return_value={"auth-bin":Path("/binary")}),patch.object(smoke.migration,"executable",side_effect=lambda p:p),patch.object(smoke.migration,"Cluster",return_value=cluster),patch.object(smoke,"digest",return_value="hash"),patch.object(smoke,"copy_artifacts",side_effect=lambda artifacts,*_:artifacts),patch.object(smoke,"exercise",side_effect=error):
                smoke.run(Path("/json"),Path("/pg"),report)
            cluster.cleanup.assert_called_once();self.assertEqual(report["cleanup"],"PASS");self.assertNotIn("private-token",json.dumps(report))
    def test_cleanup_failure_or_remaining_directory_revokes_success(self):
        for remains in (False,True):
            cluster=self.fake_cluster();cluster.root.exists.return_value=remains
            if not remains:cluster.cleanup.side_effect=RuntimeError("private-token")
            report={}
            def passed(*args):report["result"]="PASS"
            with patch.object(smoke,"build_artifacts",return_value={"auth-bin":Path("/binary")}),patch.object(smoke.migration,"executable",side_effect=lambda p:p),patch.object(smoke.migration,"Cluster",return_value=cluster),patch.object(smoke,"digest",return_value="hash"),patch.object(smoke,"copy_artifacts",side_effect=lambda artifacts,*_:artifacts),patch.object(smoke,"exercise",side_effect=passed):
                smoke.run(Path("/json"),Path("/pg"),report)
            self.assertEqual((report["result"],report["cleanup"]),("FAIL","FAIL"))
    def test_metadata_failure_always_finalizes_private_failure_report(self):
        with tempfile.TemporaryDirectory() as folder:
            report=Path(folder)/"receipt.json"
            with patch.object(smoke,"digest",side_effect=OSError("private-token")),contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(smoke.main(["--cargo-json","/missing","--pg-bin","/missing","--report",str(report)]),1)
            value=json.loads(report.read_text());self.assertEqual(value["failure"],"OSError");self.assertNotIn("private-token",report.read_text());self.assertEqual(report.stat().st_mode & 0o777,0o600)

    def test_symlink_report_is_not_followed(self):
        with tempfile.TemporaryDirectory() as folder:
            target=Path(folder)/"original";target.write_text("preserved")
            link=Path(folder)/"receipt.json";link.symlink_to(target)
            with patch.object(smoke,"run") as run,contextlib.redirect_stderr(io.StringIO()),self.assertRaises(SystemExit):
                smoke.main(["--cargo-json","/missing","--pg-bin","/missing","--report",str(link)])
            run.assert_not_called();self.assertEqual(target.read_text(),"preserved")

    def test_failed_or_partial_cargo_receipt_cannot_select_artifacts(self):
        for rows in ([{"reason":"build-finished","success":False}], [{"reason":"build-finished","success":True}], []):
            with tempfile.TemporaryDirectory() as folder:
                path=Path(folder)/"cargo.json";path.write_text("\n".join(json.dumps(row) for row in rows))
                with self.assertRaises(Exception):smoke.build_artifacts(path)

    def test_skipped_or_missing_native_result_cannot_qualify(self):
        import subprocess
        cluster=Mock();cluster.root=Path("/owned");cluster.env={};cluster.password="private-secret"
        cluster.url.side_effect=lambda db:"postgresql://private/"+db
        cluster.sql.return_value="160014"
        artifacts={key:Path("/"+key) for key in smoke.ARTIFACTS}
        for stdout in (b"0 passed; 0 failed; 1 ignored;",b"1 passed; 0 failed; 1 ignored;",b"test result: ok. 0 passed; 0 failed; 0 ignored;"):
            report={"commands":{}}
            with patch.object(smoke,"command",return_value=subprocess.CompletedProcess([],0,stdout,b"")),patch.object(smoke.migration,"migration_manifest",side_effect=lambda _,owner:{i:{} for i in range(1,15 if owner=="auth-service" else 16)}),patch.object(smoke.migration,"check_ledger",side_effect=lambda _,db,_manifest:14 if db=="mcr_auth_relationship" else 15),self.assertRaises(Exception):
                smoke.exercise(cluster,artifacts,report)
            self.assertNotIn("native_tests",report)

    def test_binary_change_during_pinning_is_refused(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder);source=root/"binary";source.write_bytes(b"compiled-owner")
            hashes={"owner":smoke.digest(source)}
            pinned=root/"pinned";pinned.mkdir()
            copies=smoke.copy_artifacts({"owner":source},pinned,hashes)
            self.assertEqual(copies["owner"].read_bytes(),b"compiled-owner")
            self.assertEqual(copies["owner"].stat().st_mode & 0o777,0o700)
            source.write_bytes(b"different-checkout")
            with self.assertRaises(Exception):smoke.copy_artifacts({"owner":source},pinned,hashes)

if __name__=="__main__":unittest.main()
