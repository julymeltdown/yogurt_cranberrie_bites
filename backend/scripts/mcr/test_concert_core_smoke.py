"""BE05 ownership, secret-free failures, and actual dependency/lockfile boundaries."""
import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest
from unittest.mock import Mock, patch
import concert_core_smoke as smoke

class HarnessTests(unittest.TestCase):
    def test_foreign_health_target_is_refused_before_connect(self):
        with patch.object(smoke.http.client,"HTTPConnection") as connect:
            for value in ("hostile.test:80","localhost:12","127.0.0.1:0","127.0.0.1:65536","127.0.0.1:1/path"):
                with self.subTest(value=value),self.assertRaises(smoke.CheckFailure):smoke.health(value,"/readyz")
            connect.assert_not_called()
    def test_health_body_is_bounded_and_connection_closed_on_failure(self):
        response=Mock();response.read.return_value=b"x"*65537
        connection=Mock();connection.getresponse.return_value=response
        with patch.object(smoke.http.client,"HTTPConnection",return_value=connection),self.assertRaisesRegex(smoke.CheckFailure,"body-bound"):
            smoke.health("127.0.0.1:12345","/readyz")
        connection.close.assert_called_once();response.read.assert_called_once_with(65537)
    def test_command_failure_and_redaction_never_record_body_or_credentials(self):
        for code,body,label in [(1,b"private-password-token","exit"),(0,b"private-password-token","redaction")]:
            report={"commands":{}};result=subprocess.CompletedProcess([],code,b"",body)
            with patch.object(smoke.subprocess,"run",return_value=result),self.assertRaisesRegex(smoke.CheckFailure,label):
                smoke.command(report,"check",["binary"],{},Path("/tmp"),secrets=("private-password-token",) if code==0 else ())
            self.assertEqual(report,{"commands":{"check":code}})
    def test_owned_service_never_signals_after_marker_identity_failure(self):
        cluster=Mock(spec=smoke.migration.Cluster);cluster.assert_owned.side_effect=RuntimeError("foreign")
        service=smoke.OwnedService(cluster,Path("/binary"),{});service.child=Mock()
        with self.assertRaises(RuntimeError):service.stop()
        service.child.terminate.assert_not_called();service.child.kill.assert_not_called()
    def test_log_is_closed_when_spawn_did_not_create_a_child(self):
        service=smoke.OwnedService(Mock(),Path("/binary"),{});service.log=Mock()
        self.assertTrue(service.stop());service.log.close.assert_called_once()
    def test_failure_still_cleans_cluster_and_records_only_exception_type(self):
        cluster=Mock();report={"commands":{}}
        def fail(*args):raise RuntimeError("private-password-token")
        with patch.object(smoke.migration,"executable",side_effect=lambda p:p),patch.object(smoke.migration,"Cluster",return_value=cluster),patch.object(smoke,"digest",return_value="hash"),patch.object(smoke,"exercise",side_effect=fail):
            smoke.run(Path("/binary"),Path("/tests"),report)
        cluster.cleanup.assert_called_once();self.assertEqual(report["failure"],"RuntimeError");self.assertNotIn("private-password-token",json.dumps(report,default=str));self.assertEqual(report["cleanup"],"PASS")
    def test_cleanup_failure_cannot_leave_a_successful_receipt(self):
        cluster=Mock();cluster.cleanup.side_effect=RuntimeError("private")
        report={"commands":{}}
        def passed(*args):report["result"]="PASS"
        with patch.object(smoke.migration,"executable",side_effect=lambda p:p),patch.object(smoke.migration,"Cluster",return_value=cluster),patch.object(smoke,"digest",return_value="hash"),patch.object(smoke,"exercise",side_effect=passed):smoke.run(Path("/binary"),Path("/tests"),report)
        self.assertEqual((report["result"],report["cleanup"]),("FAIL","FAIL"))
    def test_existing_receipt_is_never_overwritten_or_started(self):
        with tempfile.TemporaryDirectory() as folder:
            report=Path(folder)/"receipt.json";report.write_text("preserved")
            with patch.object(smoke,"run") as run,contextlib.redirect_stderr(io.StringIO()),self.assertRaises(SystemExit):
                smoke.main(["--binary","/missing","--repository-test","/missing","--report",str(report)])
            run.assert_not_called();self.assertEqual(report.read_text(),"preserved")
    def test_preflight_failure_writes_private_sanitized_failure_receipt(self):
        with tempfile.TemporaryDirectory() as folder:
            report=Path(folder)/"receipt.json"
            with patch.object(smoke.subprocess,"check_output",side_effect=RuntimeError("private-password-token")),contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(smoke.main(["--binary","/missing","--repository-test","/missing","--report",str(report)]),1)
            value=json.loads(report.read_text());self.assertEqual(value["result"],"FAIL");self.assertEqual(value["failure"],"RuntimeError");self.assertNotIn("private-password-token",report.read_text());self.assertEqual(report.stat().st_mode & 0o777,0o600)
    def test_unreadable_metadata_also_finalizes_a_failure_receipt(self):
        with tempfile.TemporaryDirectory() as folder:
            report=Path(folder)/"receipt.json"
            with patch.object(smoke,"digest",side_effect=OSError("private-password-token")),contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(smoke.main(["--binary","/missing","--repository-test","/missing","--report",str(report)]),1)
            value=json.loads(report.read_text());self.assertEqual((value["result"],value["failure"]),("FAIL","OSError"));self.assertNotIn("private-password-token",report.read_text())

class BoundaryTests(unittest.TestCase):
    def test_runtime_domain_application_dependencies_have_no_framework_or_persistence_edges(self):
        allowed={"concert-domain":{"chrono","uuid","thiserror","serde","serde_json","sha2","iri-string"},"concert-application":{"concert-domain","async-trait","chrono","thiserror","uuid","sha2","hmac"}}
        for name,edges in allowed.items():
            manifest=tomllib.loads((smoke.ROOT/"backend/crates"/name/"Cargo.toml").read_text())
            self.assertLessEqual(set(manifest["dependencies"]),edges)
            for dependency,settings in manifest["dependencies"].items():
                if "path" in settings:self.assertEqual((smoke.ROOT/"backend/crates"/name/settings["path"]).resolve(),(smoke.ROOT/"backend/crates/concert-domain").resolve())
    def test_workspace_change_retains_every_existing_package_resolution_and_dependency(self):
        original=tomllib.loads(subprocess.check_output(["git","show","f795ed4:backend/Cargo.lock"],cwd=smoke.ROOT,text=True))
        current=tomllib.loads((smoke.ROOT/"backend/Cargo.lock").read_text())
        def packages(value):return {(p["name"],p["version"],p.get("source")):p for p in value["package"]}
        old,new=packages(original),packages(current)
        for key,value in old.items():
            self.assertIn(key,new,f"existing package resolution disappeared: {key}")
            expected=dict(value)
            if key == ("auth-service","0.1.0",None):
                self.assertNotIn("profile-service",value["dependencies"])
                expected["dependencies"]=sorted([*value["dependencies"],"profile-service"])
            self.assertEqual(new[key],expected,f"existing package changed outside reviewed owner-test dependency: {key}")
        self.assertEqual({key[0] for key in new.keys()-old.keys()},{"concert-domain","concert-application","concert-service"})
        original=tomllib.loads(subprocess.check_output(["git","show","f795ed4:backend/Cargo.toml"],cwd=smoke.ROOT,text=True))
        current=tomllib.loads((smoke.ROOT/"backend/Cargo.toml").read_text())
        self.assertEqual(original["workspace"]["dependencies"],current["workspace"]["dependencies"])
        self.assertEqual(set(current["workspace"]["members"])-set(original["workspace"]["members"]),{"crates/concert-domain","crates/concert-application","services/concert-service"})

if __name__=="__main__":unittest.main()
