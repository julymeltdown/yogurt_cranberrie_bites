"""Guard and failure-path fixtures; these do not substitute for native PG execution."""
import json
from contextlib import redirect_stderr
import io
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import migration_smoke as smoke


class IsolationGuards(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.parent = Path(self.temp.name)
        self.cluster = smoke.Cluster({}, self.parent / "owned")

    def pidfile(self):
        self.cluster.data.mkdir(exist_ok=True)
        path = self.cluster.data / "postmaster.pid"
        path.write_text(f"12345\n{self.cluster.data}\n0\n5432\n{self.cluster.socket}\n\nready\n")
        return path

    def test_existing_foreign_root_is_rejected_without_touching_content(self):
        foreign = self.parent / "foreign"
        foreign.mkdir()
        (foreign / "sentinel").write_text("preserve")
        with self.assertRaises(FileExistsError):
            smoke.Cluster({}, foreign)
        self.assertEqual((foreign / "sentinel").read_text(), "preserve")

    def test_partial_constructor_failure_removes_only_its_new_root(self):
        root = self.parent / "constructor-failure"
        with patch.object(Path, "chmod", side_effect=OSError("fixture")):
            with self.assertRaises(OSError):
                smoke.Cluster({}, root)
        self.assertFalse(root.exists())

    def test_existing_data_directory_refused_before_initdb(self):
        self.cluster.data.mkdir()
        (self.cluster.data / "sentinel").write_text("preserve")
        with patch.object(smoke, "execute") as command:
            with self.assertRaises(smoke.SmokeError):
                self.cluster.init()
            command.assert_not_called()
        self.assertEqual((self.cluster.data / "sentinel").read_text(), "preserve")

    def test_marker_tampering_refuses_stop_and_removal(self):
        self.cluster.marker.write_text('{"token":"foreign"}')
        with patch.object(smoke, "execute") as command:
            with self.assertRaises(smoke.SmokeError):
                self.cluster.cleanup()
            command.assert_not_called()
        self.assertTrue(self.cluster.root.exists())

    def test_symlink_data_directory_is_never_followed_for_cleanup(self):
        foreign = self.parent / "foreign"
        foreign.mkdir()
        self.cluster.data.symlink_to(foreign, target_is_directory=True)
        with self.assertRaises(smoke.SmokeError):
            self.cluster.cleanup()
        self.assertTrue(foreign.exists())

    def test_foreign_postmaster_pidfile_refuses_pg_ctl(self):
        path = self.pidfile()
        path.write_text(path.read_text().replace(str(self.cluster.data), "/foreign/data"))
        with patch.object(smoke, "execute") as command:
            with self.assertRaises(smoke.SmokeError):
                self.cluster.stop()
            command.assert_not_called()

    def test_stop_failure_never_removes_the_data_directory(self):
        self.pidfile()
        self.cluster.tools = {"pg_ctl": Path("/fake/pg_ctl")}
        with patch.object(smoke, "execute", side_effect=smoke.SmokeError("stop", "fixture")):
            with self.assertRaises(smoke.SmokeError):
                self.cluster.cleanup()
        self.assertTrue(self.cluster.data.exists())

    def test_generated_url_uses_only_owned_socket_and_mcr_database(self):
        value = self.cluster.url("mcr_auth")
        self.cluster.validate_url(value, "mcr_auth")
        self.assertIn("host=", value)
        self.assertNotIn(self.cluster.password, self.cluster.env.values())
        self.assertFalse(any(k.endswith("DATABASE_URL") for k in self.cluster.env))

    def test_default_foreign_and_ambiguous_urls_rejected(self):
        good = self.cluster.url("mcr_auth")
        for value in ["postgres://postgres:postgres@localhost:5432/auth_db", good.split("?")[0],
                      good + "&host=%2Fforeign", good.replace("mcr_auth", "postgres"),
                      good.replace("localhost", "remote.example"),
                      good.replace(self.cluster.password, "foreign")]:
            with self.subTest(value=smoke.redact(value)):
                with self.assertRaises(smoke.SmokeError):
                    self.cluster.validate_url(value, "mcr_auth")
        with self.assertRaises(smoke.SmokeError):
            self.cluster.url("postgres")

    def test_ambient_database_and_provider_environment_is_not_inherited(self):
        with patch.dict(os.environ, {"PGHOST": "/foreign", "DATABASE_URL": "foreign",
                                     "AUTH_DATABASE_URL": "foreign", "AUTH_GOOGLE_SECRET": "secret"}):
            cluster = smoke.Cluster({}, self.parent / "other")
        self.assertFalse(any(k in cluster.env for k in ["PGHOST", "DATABASE_URL",
                                                       "AUTH_DATABASE_URL", "AUTH_GOOGLE_SECRET", "HOME"]))
        self.assertEqual(cluster.client_env("mcr_auth")["PGHOST"], str(cluster.socket))
        cluster.cleanup()

    def test_initdb_failure_removes_password_file(self):
        self.cluster.tools = {"initdb": Path("/fake/initdb")}
        with patch.object(smoke, "execute", side_effect=smoke.SmokeError("initdb", "failed")) as command:
            with self.assertRaises(smoke.SmokeError):
                self.cluster.init()
            self.assertEqual(command.call_args.kwargs["cwd"], self.cluster.root)
        self.assertFalse((self.cluster.root / "init-password").exists())
        self.cluster.cleanup()
        self.assertFalse(self.cluster.root.exists())


class FailureReporting(unittest.TestCase):
    def test_missing_binary_fails_before_any_cluster_is_created(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(smoke, "Cluster") as cluster:
            report = smoke.run(Path(directory), Path(directory))
        cluster.assert_not_called()
        self.assertEqual(report["result"], "FAIL")
        self.assertEqual(report["failure"]["phase"], "preflight")
        self.assertEqual(report["services"], [])

    def test_relative_binary_directory_rejected(self):
        with self.assertRaises(smoke.SmokeError):
            smoke.preflight(Path("relative"), Path("/fake"))

    def test_empty_migration_set_is_not_success(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(smoke.SmokeError):
                smoke.migration_manifest(Path(directory), "auth-service")

    def test_duplicate_migration_versions_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "services/auth-service/migrations"
            path.mkdir(parents=True)
            (path / "0001_one.sql").write_text("select 1;")
            (path / "0001_two.sql").write_text("select 2;")
            with self.assertRaises(smoke.SmokeError):
                smoke.migration_manifest(root, "auth-service")

    def test_ledger_requires_exact_successful_versions_and_checksums(self):
        from unittest.mock import Mock
        cluster = Mock()
        for rows in ["", "1|f|abc", "1|t|wrong", "1|t|abc\n2|t|def", "not-a-row"]:
            cluster.sql.return_value = rows
            with self.subTest(rows=rows), self.assertRaises(smoke.SmokeError):
                smoke.check_ledger(cluster, "mcr_auth", {1: "abc"})
        cluster.sql.return_value = "1|t|abc"
        self.assertEqual(smoke.check_ledger(cluster, "mcr_auth", {1: "abc"}), 1)

    def test_subprocess_failure_is_redacted_and_never_passes(self):
        output = subprocess.CompletedProcess([], 9, "", "denied password SECRET postgres://a:b@remote/db")
        with patch.object(subprocess, "run", return_value=output):
            with self.assertRaises(smoke.SmokeError) as failure:
                smoke.execute(["/fake"], env={"PGPASSWORD": "SECRET"}, phase="fixture")
        text = json.dumps(failure.exception.detail)
        self.assertNotIn("SECRET", text)
        self.assertNotIn("a:b", text)
        self.assertEqual(failure.exception.detail["exit_code"], 9)

    def test_execute_forwards_explicit_working_directory(self):
        with patch.object(subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "", "")) as command:
            smoke.execute(["/fake"], env={}, phase="fixture", cwd=Path("/owned-fixture"))
        self.assertEqual(command.call_args.kwargs["cwd"], Path("/owned-fixture"))

    def test_timeout_is_an_explicit_failure(self):
        with patch.object(subprocess, "run", side_effect=subprocess.TimeoutExpired("fake", 1)):
            with self.assertRaises(smoke.SmokeError) as failure:
                smoke.execute(["/fake"], env={}, phase="timeout")
        self.assertEqual(failure.exception.detail["diagnostic"], "command timed out")

    def test_failed_qualification_still_cleans_owned_cluster(self):
        with patch.object(smoke, "preflight", return_value=({}, {}, {})), \
             patch.object(smoke, "Cluster") as constructor, \
             patch.object(smoke, "qualify", side_effect=smoke.SmokeError("migrate", "fixture failure")):
            report = smoke.run(Path("/fake"), Path("/fake"))
        constructor.return_value.cleanup.assert_called_once()
        self.assertEqual(report["result"], "FAIL")
        self.assertEqual(report["cleanup"], "PASS")
        self.assertEqual(report["failure"]["phase"], "migrate")

    def test_failed_service_command_is_marked_fail_with_its_exit_code(self):
        from unittest.mock import Mock
        cluster = Mock(spec=smoke.Cluster)
        cluster.assert_postmaster.return_value = 12345
        cluster.sql.return_value = "fixture"
        cluster.env = {}
        cluster.root = Path("/owned-fixture")
        cluster.tools = {"createdb": Path("/fake/createdb")}
        cluster.url.return_value = "postgresql://mcr_owner:secret@localhost/mcr_api_gateway"
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "fixture-service"
            binary.write_bytes(b"fixture")
            report = {"services": []}
            with patch.object(smoke, "execute", side_effect=[None, smoke.SmokeError("migrate", "fixture", 7)]) as command:
                with self.assertRaises(smoke.SmokeError):
                    smoke.qualify(cluster, {"api-gateway": binary}, {"api-gateway": {1: "abc"}}, report)
            self.assertTrue(all(call.kwargs["cwd"] == cluster.root for call in command.call_args_list))
        self.assertEqual(report["services"][0]["migrate"], "FAIL")
        self.assertEqual(report["services"][0]["migrate_exit_code"], 7)
        self.assertEqual(report["services"][0]["verify"], "NOT_RUN")

    def test_unexpected_exception_still_reports_failure_and_cleans_up(self):
        with patch.object(smoke, "preflight", return_value=({}, {}, {})), \
             patch.object(smoke, "Cluster") as constructor, \
             patch.object(smoke, "qualify", side_effect=RuntimeError("sensitive fixture")):
            report = smoke.run(Path("/fake"), Path("/fake"))
        constructor.return_value.cleanup.assert_called_once()
        self.assertEqual(report["result"], "FAIL")
        self.assertEqual(report["failure"]["diagnostic"], "RuntimeError")
        self.assertNotIn("sensitive fixture", json.dumps(report))

    def test_cleanup_failure_overrides_success(self):
        with patch.object(smoke, "preflight", return_value=({}, {}, {})), \
             patch.object(smoke, "Cluster") as constructor, patch.object(smoke, "qualify"):
            constructor.return_value.root = Path("/retained-owned-fixture")
            constructor.return_value.cleanup.side_effect = smoke.SmokeError("stop", "fixture failure")
            report = smoke.run(Path("/fake"), Path("/fake"))
        self.assertEqual(report["result"], "FAIL")
        self.assertEqual(report["cleanup"], "FAIL")
        self.assertEqual(report["retained_owned_directory"], "/retained-owned-fixture")

    def test_report_path_never_overwrites_an_existing_file(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(smoke, "run") as run:
            report = Path(directory) / "existing.json"
            report.write_text("preserve")
            with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                smoke.main(["--bin-dir", "/fake", "--report", str(report)])
            self.assertEqual(report.read_text(), "preserve")
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
