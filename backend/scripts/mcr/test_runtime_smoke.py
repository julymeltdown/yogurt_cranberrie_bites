"""Offline safety and failure semantics; no service builds or existing listeners."""
import contextlib
import copy
import io
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

import migration_smoke as migration
import runtime_smoke as runtime


class RuntimeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="mcr-runtime-test-")
        self.addCleanup(self.temp.cleanup)
        self.cluster = migration.Cluster({}, Path(self.temp.name) / "owned")
        self.ports = runtime.Ports()
        self.addCleanup(self.ports.close)
        for service in runtime.START_ORDER:
            for kind in ("http", "metrics", "grpc"):
                self.ports.reserve(f"{service}:{kind}")
        self.ports.reserve("redis")
        self.ports.reserve("disabled-egress")
        keys = {name: "fresh-generated-test-key" for name in (
            "AUTH_JWT_PRIVATE_KEY_PEM", "ACTOR_DELEGATION_VERIFYING_KEYS", "ACTOR_DELEGATION_SIGNING_SEED",
            "RESOURCE_EVIDENCE_SIGNING_SEED", "RESOURCE_EVIDENCE_VERIFYING_KEYS")}
        credentials = {prefix + "_SERVICE_CREDENTIAL": "fresh-" + prefix for prefix in runtime.PREFIX.values()}
        self.envs = runtime.environments(self.cluster, self.ports, keys, credentials, "fresh-redis-password")

    def assertRejected(self, envs):
        with self.assertRaises(runtime.SmokeError):
            runtime.validate_environments(self.cluster, self.ports, envs)

    def test_generated_configuration_passes_and_omits_ambient(self):
        with patch.dict(os.environ, {"HOME": "/foreign", "DATABASE_URL": "postgres://foreign/data",
                                     "HTTP_PROXY": "http://external:80", "AUTH_GOOGLE_CLIENT_SECRET": "foreign"}):
            runtime.validate_environments(self.cluster, self.ports, self.envs)
        for env in self.envs.values():
            self.assertFalse({"HOME", "DATABASE_URL", "HTTP_PROXY", "AUTH_GOOGLE_CLIENT_SECRET"} & env.keys())
            self.assertEqual(env["TMPDIR"], str(self.cluster.root))
        gateway = self.envs["api-gateway"]
        self.assertGreaterEqual(int(gateway["GATEWAY_JWT_KEYS_REFRESH_SECONDS"]), 5)
        self.assertLessEqual(int(gateway["GATEWAY_JWT_KEYS_MIN_REFRESH_SECONDS"]),
                             int(gateway["GATEWAY_JWT_KEYS_REFRESH_SECONDS"]))

    def test_rejects_default_database_urls_for_every_service(self):
        for service, variable in migration.SERVICES.items():
            with self.subTest(service=service):
                changed = copy.deepcopy(self.envs)
                changed[service][variable] = "postgres://postgres:postgres@localhost:5432/postgres"
                self.assertRejected(changed)

    def test_rejects_foreign_database_socket(self):
        self.envs["auth-service"]["AUTH_DATABASE_URL"] = self.cluster.url("mcr_auth").replace("mcr-pg", "foreign") + "&host=/tmp"
        self.assertRejected(self.envs)

    def test_rejects_foreign_or_existing_data_root(self):
        with self.assertRaises(FileExistsError):
            migration.Cluster({}, self.cluster.root)
        self.cluster.data.mkdir()
        with self.assertRaises(runtime.SmokeError):
            self.cluster.init()

    def test_rejects_foreign_marker_before_child_launch(self):
        self.cluster.marker.write_text("{}")
        with patch.object(subprocess, "Popen") as spawn:
            with self.assertRaises(runtime.SmokeError):
                runtime.Child(self.cluster, "test", ["/bin/false"], {}, self.ports, ())
            spawn.assert_not_called()

    def test_rejects_ambient_home_and_external_connectors(self):
        for key, value in (("HOME", "/tmp"), ("TMPDIR", "/tmp"), ("HTTP_PROXY", "http://127.0.0.1:9000"),
                           ("AUTH_VERIFICATION_MAILER_ENDPOINT", "http://127.0.0.1:9000"),
                           ("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:9000")):
            with self.subTest(key=key):
                changed = copy.deepcopy(self.envs)
                changed["auth-service"][key] = value
                self.assertRejected(changed)

    def test_rejects_foreign_listeners_and_peers(self):
        for key, value in (("POST_HTTP_ADDR", "0.0.0.0:8085"), ("POST_PROFILE_GRPC_ENDPOINT", "http://127.0.0.1:50052"),
                           ("POST_GATEWAY_GRPC_ENDPOINT", "http://upstream:50050"), ("POST_SEED_ENABLED", "true"),
                           ("POST_DATABASE_MIGRATION_MODE", "startup")):
            changed = copy.deepcopy(self.envs)
            changed["post-service"][key] = value
            self.assertRejected(changed)

    def test_rejects_old_redis_and_kafka(self):
        for key, value in (("POST_REDIS_URL", "redis://127.0.0.1:6379/"),
                           ("POST_EXTERNAL_PUBLISHER", "kafka"), ("POST_KAFKA_BOOTSTRAP_BROKERS", "external:9092")):
            changed = copy.deepcopy(self.envs)
            changed["post-service"][key] = value
            self.assertRejected(changed)

    def test_rejects_non_owned_provider_endpoint(self):
        self.envs["auth-service"]["AUTH_GOOGLE_TOKEN_URI"] = "https://accounts.google.com/token"
        self.assertRejected(self.envs)

    def test_missing_binaries_fails_before_any_resource(self):
        with patch.object(migration, "Cluster") as cluster:
            report = runtime.run(Path(self.temp.name) / "absent", Path("/opt/homebrew/bin"),
                                 Path("/opt/homebrew/bin/redis-server"), Path("/opt/homebrew/bin/openssl"))
        self.assertEqual(report["result"], "FAIL")
        self.assertEqual(report["failure"]["phase"], "preflight")
        self.assertEqual(report["cleanup"], "NOT_RUN")
        cluster.assert_not_called()

    def test_default_port_and_actual_collision_refused(self):
        with self.assertRaises(runtime.SmokeError):
            self.ports.reserve("bad", 6379)
        with socket.socket() as foreign:
            foreign.bind(("127.0.0.1", 0))
            foreign.listen()
            with self.assertRaises(runtime.SmokeError):
                self.ports.reserve("collision", foreign.getsockname()[1])
            # Foreign socket survives; no probe or kill was issued.
            self.assertGreater(foreign.fileno(), 0)

    def test_restarted_ports_are_fresh_without_socket_reuse(self):
        before = self.ports.ports.copy()
        for service in runtime.START_ORDER:
            for kind in ("http", "metrics", "grpc"):
                name = f"{service}:{kind}"
                self.ports.release(name)
                self.ports.reserve(name)
        restarted = {p for name, p in self.ports.ports.items() if name not in ("redis", "disabled-egress")}
        self.assertFalse(restarted & set(before.values()))
        self.assertEqual(self.ports.ports["redis"], before["redis"])

    def test_openssl_ignores_host_configuration(self):
        public = bytes.fromhex("302a300506032b6570032100") + bytes(32)
        with patch.object(migration, "execute") as execute, patch.object(Path, "read_bytes", return_value=public):
            runtime.generated_keys(self.cluster, Path("/openssl"))
        for call in execute.call_args_list:
            self.assertEqual(call.kwargs["env"]["OPENSSL_CONF"], "/dev/null")
            self.assertNotIn("HOME", call.kwargs["env"])

    def test_lsof_wrong_pid_and_nonloopback_rejected(self):
        self.assertEqual(runtime.listener_addresses("p123\nf10\nn127.0.0.1:49999\n", 123), {"127.0.0.1:49999"})
        for value in ("p124\nn127.0.0.1:49999\n", "p123\nn*:49999\n", "n127.0.0.1:49999\n"):
            with self.assertRaises(runtime.SmokeError):
                runtime.listener_addresses(value, 123)

    def test_probe_refuses_unowned_listener_without_connecting(self):
        child = Mock()
        child.listeners.return_value = False
        with patch.object(runtime.http.client, "HTTPConnection") as connect:
            with self.assertRaises(runtime.SmokeError):
                runtime.http_probe(child, "metrics", "/readyz")
            connect.assert_not_called()

    def test_readiness_requires_real_shape_and_status(self):
        value = {"ready": True, "degraded": False, "draining": False,
                 "checks": {"database": {"required": True, "status": "up"}}}
        result = runtime.readiness_value(200, json.dumps(value), "profile-service")
        self.assertTrue(result["ready"])
        for status, body in ((200, b"ok"), (200, b'{"ready":true}'), (503, json.dumps(value))):
            with self.assertRaises(runtime.SmokeError):
                runtime.readiness_value(status, body, "profile-service")
        value["checks"]["database"]["status"] = "down"
        with self.assertRaises(runtime.SmokeError):
            runtime.readiness_value(200, json.dumps(value), "profile-service")
        value.update(ready=False, degraded=True)
        self.assertFalse(runtime.readiness_value(503, json.dumps(value), "profile-service")["ready"])

    def test_null_or_missing_dependencies_cannot_pass(self):
        value = {"ready": True, "degraded": False, "draining": False,
                 "checks": {"database": {"required": True, "status": "up"}}}
        for service in ("api-gateway", "auth-service", "friend-service", "post-service"):
            with self.assertRaises(runtime.SmokeError):
                runtime.readiness_value(200, json.dumps(value), service)

    def readiness(self, service):
        checks = {"database": {"required": True, "status": "up"}}
        for peer in runtime.PEERS[service]:
            required = service != "auth-service" and not (
                service == "post-service" and peer in ("profile-service", "notification-service"))
            checks[runtime.PREFIX[service].lower() + "->" + runtime.PREFIX[peer].lower()] = {
                "required": required, "status": "up"}
        if service == "api-gateway":
            checks["signing-keys"] = {"required": True, "status": "up"}
        return {"ready": True, "degraded": False, "draining": False, "checks": checks}

    def test_gateway_signing_key_name_matches_actual_producer(self):
        value = self.readiness("api-gateway")
        self.assertTrue(runtime.readiness_value(200, json.dumps(value), "api-gateway")["ready"])
        value["checks"]["signing_keys"] = value["checks"].pop("signing-keys")
        with self.assertRaises(runtime.SmokeError):
            runtime.readiness_value(200, json.dumps(value), "api-gateway")

    def test_dependency_flags_and_degraded_status_cannot_be_forged(self):
        for service in runtime.START_ORDER:
            value = self.readiness(service)
            self.assertTrue(runtime.readiness_value(200, json.dumps(value), service)["ready"])
            for name in value["checks"]:
                changed = copy.deepcopy(value)
                changed["checks"][name]["required"] = not changed["checks"][name]["required"]
                with self.subTest(service=service, check=name), self.assertRaises(runtime.SmokeError):
                    runtime.readiness_value(200, json.dumps(changed), service)
        optional = self.readiness("auth-service")
        optional["checks"]["auth->profile"]["status"] = "down"
        optional["degraded"] = True
        self.assertTrue(runtime.readiness_value(200, json.dumps(optional), "auth-service")["ready"])
        optional["degraded"] = False
        with self.assertRaises(runtime.SmokeError):
            runtime.readiness_value(200, json.dumps(optional), "auth-service")

    def test_readiness_wrong_types_are_rejected(self):
        for value in (None, [], {}, {"ready": 1}, {**self.readiness("profile-service"), "checks": []},
                      {**self.readiness("profile-service"), "checks": {"database": None}}):
            with self.subTest(value=value), self.assertRaises(runtime.SmokeError):
                runtime.readiness_value(200, json.dumps(value), "profile-service")

    def test_signal_deferred_until_child_is_registered(self):
        registry = {}
        previous = {sig: signal.getsignal(sig) for sig in (signal.SIGINT, signal.SIGTERM)}
        process = Mock(pid=123)
        def spawn(*_args, **_kwargs):
            os.kill(os.getpid(), signal.SIGTERM)
            os.kill(os.getpid(), signal.SIGINT)
            return process
        with patch.object(subprocess, "Popen", side_effect=spawn):
            with self.assertRaises(KeyboardInterrupt):
                with runtime.protected_signals():
                    registry["test"] = runtime.Child(self.cluster, "test", ["/bin/false"],
                                                     self.cluster.env, self.ports, ())
        self.assertIs(registry["test"].process, process)
        for sig, handler in previous.items():
            self.assertEqual(signal.getsignal(sig), handler)

    def test_repeated_signals_do_not_abort_cleanup(self):
        with runtime.protected_signals(cleanup=True):
            for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGTERM):
                os.kill(os.getpid(), sig)

    def test_cleanup_attempts_all_children_and_stops_pg_without_removal_on_failure(self):
        broken, good, redis = Mock(), Mock(), Mock()
        broken.stop.side_effect = runtime.SmokeError("stop", "cannot confirm exit")
        good.stop.return_value = redis.stop.return_value = {"forced": False, "exit_code": 0}
        report = {"result": "PASS", "cleanup": "NOT_RUN", "process_stops": []}
        with patch.object(self.cluster, "stop") as pg_stop, patch.object(self.cluster, "cleanup") as remove:
            runtime.cleanup_resources(self.cluster, self.ports, {"good": good, "broken": broken}, redis, report)
        pg_stop.assert_called_once()
        good.stop.assert_called_once()
        redis.stop.assert_called_once()
        remove.assert_not_called()
        self.assertEqual((report["result"], report["cleanup"]), ("FAIL", "FAIL"))
        self.assertTrue(self.cluster.root.exists())

    def test_readiness_timeout_keeps_failure_evidence(self):
        child = Mock(name="profile-service")
        value = {"ready": False, "degraded": True, "draining": False,
                 "checks": {"database": {"required": True, "status": "down"}}}
        rows = {"profile-service": {}}
        with patch.object(runtime, "http_probe", return_value=(503, json.dumps(value))), \
             patch.object(runtime.time, "monotonic", side_effect=[0, 0, 2]), patch.object(runtime.time, "sleep"):
            with self.assertRaises(runtime.SmokeError):
                runtime.wait_ready({"profile-service": child}, rows, "initial", timeout=1)
        self.assertEqual(rows["profile-service"]["initial"]["http_status"], 503)

    def child(self):
        process = Mock()
        process.pid = 123
        process.poll.return_value = None
        process.returncode = 0
        with patch.object(subprocess, "Popen", return_value=process) as spawn:
            child = runtime.Child(self.cluster, "test", ["/bin/false"], self.cluster.env, self.ports, ())
            self.assertEqual(spawn.call_args.kwargs["cwd"], self.cluster.root)
            self.assertNotIn("HOME", spawn.call_args.kwargs["env"])
        return child, process

    def test_exit_failure_includes_code_and_redacts(self):
        child, process = self.child()
        child.secrets = ("sensitive-token",)
        child.log.write_text("bad sensitive-token postgres://user:password@host/db")
        process.poll.return_value = 7
        process.returncode = 7
        with self.assertRaises(runtime.SmokeError) as error:
            child.alive()
        self.assertEqual(error.exception.detail["exit_code"], 7)
        self.assertNotIn("sensitive-token", error.exception.detail["diagnostic"])
        self.assertNotIn("password", error.exception.detail["diagnostic"])

    def test_stop_timeout_only_kills_owned_child(self):
        child, process = self.child()
        process.wait.side_effect = [subprocess.TimeoutExpired("owned", 12), 0]
        stopped = child.stop()
        self.assertTrue(stopped["forced"])
        process.terminate.assert_called_once()
        process.kill.assert_called_once()

    def test_stop_refuses_replaced_ownership_marker(self):
        child, process = self.child()
        self.cluster.marker.write_text("{}")
        with self.assertRaises(runtime.SmokeError):
            child.stop()
        process.terminate.assert_not_called()
        process.kill.assert_not_called()

    def test_early_qualification_failure_still_cleans_owned_cluster(self):
        with patch.object(migration, "preflight", return_value=({}, {}, {})), \
             patch.object(migration, "executable", side_effect=lambda p: p), \
             patch.object(migration, "execute", return_value=Mock(stdout="test-revision")), \
             patch.object(migration, "Cluster", return_value=self.cluster), \
             patch.object(migration, "qualify", side_effect=runtime.SmokeError("ledger", "receipt mismatch")):
            report = runtime.run(Path("/binaries"), Path("/pg"), Path("/redis"), Path("/openssl"))
        self.assertEqual(report["result"], "FAIL")
        self.assertEqual(report["cleanup"], "PASS")
        self.assertFalse(self.cluster.root.exists())

    def test_cleanup_failure_cannot_report_success_or_remove_foreign_root(self):
        with patch.object(migration, "preflight", return_value=({}, {}, {})), \
             patch.object(migration, "executable", side_effect=lambda p: p), \
             patch.object(migration, "execute", return_value=Mock(stdout="test-revision")), \
             patch.object(migration, "Cluster", return_value=self.cluster), \
             patch.object(migration, "qualify", side_effect=runtime.SmokeError("ledger", "receipt mismatch")), \
             patch.object(self.cluster, "cleanup", side_effect=runtime.SmokeError("ownership", "marker changed")):
            report = runtime.run(Path("/binaries"), Path("/pg"), Path("/redis"), Path("/openssl"))
        self.assertEqual(report["result"], "FAIL")
        self.assertEqual(report["cleanup"], "FAIL")
        self.assertTrue(self.cluster.root.exists())

    def test_report_refuses_overwrite_without_running(self):
        path = Path(self.temp.name) / "evidence.json"
        path.write_text("preserved")
        with contextlib.redirect_stderr(io.StringIO()), patch.object(runtime, "run") as run:
            with self.assertRaises(SystemExit):
                runtime.main(["--bin-dir", "/missing", "--report", str(path)])
            run.assert_not_called()
        self.assertEqual(path.read_text(), "preserved")

    def test_truthful_failure_report_written_mode_600(self):
        path = Path(self.temp.name) / "failure.json"
        with contextlib.redirect_stdout(io.StringIO()):
            code = runtime.main(["--bin-dir", str(Path(self.temp.name) / "missing"), "--report", str(path)])
        self.assertEqual(code, 1)
        self.assertEqual(json.loads(path.read_text())["result"], "FAIL")
        self.assertEqual(path.stat().st_mode & 0o777, 0o600)


if __name__ == "__main__":
    unittest.main()
