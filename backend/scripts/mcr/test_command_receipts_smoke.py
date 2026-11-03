"""BE15 harness refusal, cleanup and source/contract evidence checks."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch
import command_receipts_smoke as smoke

class HarnessTests(unittest.TestCase):
    def fake_cluster(self):
        cluster = Mock()
        cluster.root.exists.return_value = False
        return cluster
    def test_existing_report_is_never_overwritten_or_started(self):
        with tempfile.TemporaryDirectory() as folder:
            report=Path(folder)/"receipt.json";report.write_text("preserved")
            with patch.object(smoke,"run") as run,contextlib.redirect_stderr(io.StringIO()),self.assertRaises(SystemExit):
                smoke.main(["--binary","/missing","--repository-test","/missing","--pg-bin","/missing","--report",str(report)])
            run.assert_not_called();self.assertEqual(report.read_text(),"preserved")
    def test_relative_binary_is_refused_before_cluster_creation(self):
        report={}
        with patch.object(smoke.migration,"Cluster") as cluster:
            smoke.run(Path("relative"),Path("/test"),Path("/pg"),report)
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
            with patch.object(smoke,"digest",side_effect=OSError("private-token")),contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(smoke.main(["--binary","/missing","--repository-test","/missing","--pg-bin","/missing","--report",str(report)]),1)
            value=json.loads(report.read_text());self.assertEqual(value["failure"],"OSError");self.assertNotIn("private-token",report.read_text());self.assertEqual(report.stat().st_mode & 0o777,0o600)
    def test_pinned_contract_registry_matches_exact_command_set(self):
        root=smoke.ROOT
        self.assertEqual(tuple(smoke.digest(root/"contracts/domain/v1"/name) for name in smoke.CONTRACTS),smoke.PINNED)
        operations=json.loads((root/"contracts/domain/v1/operations.json").read_text())
        receipt=json.loads((root/"contracts/domain/v1/common.schema.json").read_text())["$defs"]["CommandReceipt"]
        self.assertNotIn("request_hash",receipt["properties"])
        self.assertNotIn("request_hash",receipt["required"])

        registered={x["operationId"] for x in operations if x["idempotency_key"]}
        import re
        source=(root/"backend/crates/concert-application/src/commands/registry.rs").read_text()
        self.assertEqual(set(re.findall(r'id: "([^"]+)"',source)),registered)
        migration=(root/"backend/services/concert-service/migrations/0002_command_receipts_outbox.sql").read_text()
        self.assertTrue(all("('"+name+"')" in migration for name in registered))

if __name__=="__main__":unittest.main()
