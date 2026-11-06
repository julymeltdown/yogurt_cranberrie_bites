"""Development launcher ownership, private mail and actual control protocol regressions."""
import contextlib
import http.client
import io
import json
import os
from pathlib import Path
import socket
import tempfile
import threading
import unittest
from unittest.mock import Mock, patch
from uuid import uuid4

import dev_stack as dev
import migration_smoke as migration
import runtime_smoke as runtime


class DevTests(unittest.TestCase):
    def test_settlement_owner_starts_before_its_authority_dependents(self):
        order = runtime.start_order(("profile-service", "notification-service", "friend-service",
                                     "post-service", "concert-service", "auth-service", "api-gateway"))
        self.assertLess(order.index("concert-service"), order.index("auth-service"))
        self.assertLess(order.index("concert-service"), order.index("friend-service"))

    def test_start_order_without_a_settlement_owner_is_unchanged(self):
        self.assertEqual(runtime.start_order(), runtime.START_ORDER)
        self.assertEqual(runtime.start_order(("post-service", "api-gateway")), ("post-service", "api-gateway"))

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="mcr-dev-test-")
        self.addCleanup(self.temp.cleanup)
        self.cluster = migration.Cluster({}, Path(self.temp.name) / "owned")
        self.ports = runtime.Ports()
        self.addCleanup(self.ports.close)
        self.ports.reserve("mail")

    def sink(self):
        sink = dev.MailSink(self.cluster, self.ports, "private-test-mail-token")
        self.addCleanup(sink.close)
        return sink

    def post(self, sink, payload, *, token="private-test-mail-token", key=None, path="/verify", headers=None):
        conn = http.client.HTTPConnection("127.0.0.1", self.ports.ports["mail"], timeout=3)
        try:
            conn.request("POST", path, body=json.dumps(payload), headers={
                "Authorization": "Bearer " + token, "Idempotency-Key": key or str(uuid4()),
                "Content-Type": "application/json", **(headers or {})})
            response = conn.getresponse()
            response.read()
            return response.status
        finally:
            conn.close()

    def verification(self):
        return {"email": "synthetic@example.test", "verification_code": "073012", "expires_at": "2026-11-03T12:00:00Z"}

    def test_mail_capture_uses_owned_reservation_and_private_files(self):
        sink = self.sink()
        with contextlib.redirect_stderr(io.StringIO()) as errors, contextlib.redirect_stdout(io.StringIO()) as output:
            self.assertEqual(self.post(sink, self.verification()), 204)
        self.assertEqual(output.getvalue() + errors.getvalue(), "")
        paths = list(sink.directory.glob("*.json"))
        self.assertEqual(len(paths), 1)
        self.assertEqual(paths[0].stat().st_mode & 0o777, 0o600)
        self.assertEqual(sink.directory.stat().st_mode & 0o777, 0o700)
        self.assertEqual(json.loads(paths[0].read_text())["payload"], self.verification())
        self.assertNotIn("mail", self.ports.held)

    def test_mail_rejects_unauthenticated_unknown_malformed_or_extra_fields(self):
        sink = self.sink()
        self.assertEqual(self.post(sink, self.verification(), token="wrong"), 401)
        for options, payload in [({"path": "/other"}, self.verification()), ({"key": "../../escape"}, self.verification()),
                                 ({}, {**self.verification(), "password": "not-allowed"}), ({}, []),
                                 ({}, {**self.verification(), "verification_code": "１２３４５６"})]:
            with self.subTest(options=options):
                self.assertEqual(self.post(sink, payload, **options), 400)
        self.assertEqual(list(sink.directory.iterdir()), [])

    def test_mail_delivery_deduplicates_same_id_and_rejects_conflict(self):
        sink, key = self.sink(), str(uuid4())
        for _ in range(2):
            self.assertEqual(self.post(sink, self.verification(), key=key), 204)
        self.assertEqual(sink.count, 1)
        self.assertEqual(self.post(sink, {**self.verification(), "verification_code": "999999"}, key=key), 409)
        self.assertEqual(sink.count, 1)

    def test_mail_bounds_size_count_and_rejects_replaced_inbox(self):
        sink = self.sink()
        self.assertEqual(self.post(sink, {"oversized": "x" * 17000}), 400)
        sink.count = 1000
        self.assertEqual(self.post(sink, self.verification()), 507)
        sink.directory.rmdir()
        sink.directory.symlink_to(Path(self.temp.name), target_is_directory=True)
        self.assertEqual(self.post(sink, self.verification()), 400)

    def test_reset_delivery_records_actual_link_only_in_private_capture(self):
        sink = self.sink()
        self.assertEqual(self.post(sink, {"email": "synthetic@example.test", "reset_link": "http://127.0.0.1:43210/reset?token=private"}, path="/reset"), 204)
        value = json.loads(next(sink.directory.glob("*.json")).read_text())
        self.assertEqual(value["kind"], "PASSWORD_RESET")

    def assert_incomplete_mail_request_has_bounded_shutdown(self, payload):
        sink = self.sink()
        dispatched = threading.Event()
        stopped = threading.Event()
        errors = []
        finish_request = sink.server.finish_request

        def dispatch(*args):
            dispatched.set()
            return finish_request(*args)

        def close():
            try:
                sink.close()
            except BaseException as error:
                errors.append(error)
            finally:
                stopped.set()

        # Wait for actual acceptance so shutdown cannot race ahead of HTTP parsing.
        with patch.object(sink.server, "finish_request", side_effect=dispatch):
            client = socket.create_connection(("127.0.0.1", self.ports.ports["mail"]), timeout=3)
            closer = None
            try:
                if payload:
                    client.sendall(payload)
                self.assertTrue(dispatched.wait(2), "mail server did not accept client")
                closer = threading.Thread(target=close, daemon=True)
                closer.start()
                bounded = stopped.wait(4)
            finally:
                # A failed regression must release the blocked parser before test cleanup.
                client.close()
                if closer is not None:
                    closer.join(timeout=3)
        self.assertTrue(bounded, "mail shutdown blocked on incomplete HTTP request")
        self.assertEqual(errors, [])
        self.assertFalse(sink.thread.is_alive())
        self.assertEqual(list(sink.directory.iterdir()), [])

    def test_mail_idle_client_cannot_block_owned_shutdown(self):
        self.assert_incomplete_mail_request_has_bounded_shutdown(b"")

    def test_mail_partial_headers_cannot_block_owned_shutdown(self):
        self.assert_incomplete_mail_request_has_bounded_shutdown(
            b"POST /verify HTTP/1.1\r\nContent-Len")

    def test_origin_rejects_external_credentials_path_query_or_implicit_ports(self):
        for value in ("https://example.com", "http://localhost", "http://127.0.0.1:5/", "http://user@localhost:5",
                      "http://localhost:5?secret=x", "http://[", "http://0.0.0.0:8080", "http://127.0.0.1:70000"):
            with self.subTest(value=value), self.assertRaises(dev.SmokeError):
                dev.local_origin(value)
        self.assertEqual(dev.local_origin("http://localhost:5173"), "http://localhost:5173")

    def envs(self, browser_origin=None):
        for service in runtime.START_ORDER:
            for kind in ("http", "metrics", "grpc"):
                self.ports.reserve(f"{service}:{kind}")
        for name in ("redis", "disabled-egress"):
            self.ports.reserve(name)
        keys = {key: "fresh-test-key" for key in ("AUTH_JWT_PRIVATE_KEY_PEM", "ACTOR_DELEGATION_VERIFYING_KEYS",
                "ACTOR_DELEGATION_SIGNING_SEED", "RESOURCE_EVIDENCE_VERIFYING_KEYS", "RESOURCE_EVIDENCE_SIGNING_SEED")}
        return dev.development_environments(self.cluster, self.ports, keys, {}, "new-redis-password", "new-mail-token", browser_origin)

    def test_local_auth_configuration_keeps_gateway_boundary_and_owned_mail(self):
        with patch.dict(os.environ, {"HOME": "/foreign", "AUTH_DATABASE_URL": "postgres://foreign", "AUTH_GOOGLE_CLIENT_SECRET": "secret"}):
            envs = self.envs()
        for env in envs.values():
            self.assertNotIn("HOME", env)
            self.assertNotIn("AUTH_GOOGLE_CLIENT_SECRET", env)
        auth, gateway = envs["auth-service"], envs["api-gateway"]
        self.assertEqual(auth["AUTH_PUBLIC_APPLICATION_ROUTES_ENABLED"], "false")
        self.assertEqual(auth["AUTH_VERIFICATION_MAILER_ENDPOINT"], self.ports.url("mail") + "/verify")
        self.assertEqual(auth["AUTH_PASSWORD_RESET_MAILER_ENDPOINT"], self.ports.url("mail") + "/reset")
        self.assertEqual(gateway["GATEWAY_REFRESH_COOKIE_SECURE"], "false")
        self.assertEqual(gateway["GATEWAY_CORS_ALLOWED_ORIGINS"], self.ports.url("api-gateway:http"))
        self.assertEqual(auth["AUTH_PASSWORD_RESET_BASE_URL"], self.ports.url("api-gateway:http") + "/reset-password")

    def test_reset_link_targets_the_configured_browser_origin(self):
        origin = "http://127.0.0.1:5173"
        envs = self.envs(origin)
        self.assertEqual(envs["auth-service"]["AUTH_PASSWORD_RESET_BASE_URL"], origin + "/reset-password")
        self.assertEqual(envs["api-gateway"]["GATEWAY_CORS_ALLOWED_ORIGINS"], origin)
        self.assertEqual(envs["auth-service"]["AUTH_PASSWORD_RESET_MAILER_ENDPOINT"], self.ports.url("mail") + "/reset")

    def control(self):
        stack = Mock()
        stack.cluster = self.cluster
        stack.rows = {"profile-service": {"readiness": {"ready": True}}}
        stack.ports.url.return_value = "http://127.0.0.1:43210"
        control = dev.Control(stack)
        stop = threading.Event()
        def serve():
            while not stop.is_set():
                control.handle_request()
        thread = threading.Thread(target=serve, daemon=True)
        thread.start()
        def cleanup():
            stop.set()
            thread.join(timeout=3)
            control.server_close()
        self.addCleanup(cleanup)
        info = self.cluster.root.stat()
        descriptor = {"kind": "mcr-owned-dev-stack-v1", "uid": os.getuid(), "root": str(self.cluster.root),
                      "root_identity": [info.st_dev, info.st_ino], "owner": {"token": self.cluster.token, "uid": os.getuid()},
                      "socket": str(self.cluster.root / "control.sock")}
        path = Path(self.temp.name) / "ready.json"
        runtime.private_file(path, dev.encode(descriptor))
        return control, path

    def test_actual_control_status_and_stop_protocol(self):
        control, path = self.control()
        self.assertEqual(dev.control_request(path, "status")["state"], "READY")
        self.assertFalse(control.stopping)
        self.assertEqual(dev.control_request(path, "stop")["state"], "STOPPING")
        self.assertTrue(control.stopping)
        self.assertEqual(dev.control_request(path, "unknown")["state"], "REJECTED")

    def test_control_refuses_public_symlink_stale_and_foreign_descriptors(self):
        control, path = self.control()
        with patch.object(socket.socket, "connect") as connect:
            path.chmod(0o644)
            with self.assertRaises(dev.SmokeError): dev.control_request(path, "stop")
            path.chmod(0o600)
            link = path.with_suffix(".link")
            link.symlink_to(path)
            with self.assertRaises(dev.SmokeError): dev.control_request(link, "stop")
            self.cluster.marker.write_text("{}")
            with self.assertRaises(dev.SmokeError): dev.control_request(path, "stop")
            connect.assert_not_called()
        self.assertFalse(control.stopping)

    def test_rejected_control_response_never_exits_successfully(self):
        with patch.object(dev, "control_request", return_value={"state": "REJECTED"}), contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(dev.main(["status", "--ready-file", "/unused"]), 1)
            self.assertEqual(dev.main(["stop", "--ready-file", "/unused"]), 1)

    def test_missing_binaries_fail_before_resource_creation_with_new_report(self):
        report, ready = Path(self.temp.name) / "report.json", Path(self.temp.name) / "ready.json"
        with patch.object(migration, "Cluster") as cluster, contextlib.redirect_stdout(io.StringIO()):
            result = dev.main(["start", "--bin-dir", "/missing", "--report", str(report), "--ready-file", str(ready), "--duration-seconds", "1"])
        cluster.assert_not_called()
        self.assertEqual(result, 1)
        self.assertEqual(json.loads(report.read_text())["result"], "FAIL")
        self.assertEqual(report.stat().st_mode & 0o777, 0o600)

    def test_existing_reports_or_ready_files_never_overwritten(self):
        report, ready = Path(self.temp.name) / "report.json", Path(self.temp.name) / "ready.json"
        ready.write_text("preserved")
        with patch.object(dev, "DevStack") as stack, contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit):
                dev.main(["start", "--bin-dir", "/missing", "--report", str(report), "--ready-file", str(ready)])
        stack.assert_not_called()
        self.assertEqual(ready.read_text(), "preserved")
        self.assertEqual(json.loads(report.read_text())["cleanup"], "NOT_RUN")

    def test_failed_mail_stop_attempts_other_children_but_retains_root(self):
        stack = dev.DevStack(Path("/missing"), Path("/pg"), Path("/redis"), Path("/openssl"))
        stack.cluster = self.cluster
        stack.mail, child = Mock(), Mock()
        stack.mail.count = 0
        stack.mail.stop.side_effect = RuntimeError("private value must not be printed")
        child.stop.return_value = {"exit_code": 0, "forced": False}
        stack.children = {"service": child}
        with patch.object(self.cluster, "stop") as pgstop, patch.object(self.cluster, "cleanup") as remove:
            stack.close()
        child.stop.assert_called_once()
        pgstop.assert_called_once()
        remove.assert_not_called()
        self.assertEqual(stack.report["cleanup"], "FAIL")
        self.assertTrue(self.cluster.root.exists())
        self.assertNotIn("private value", json.dumps(stack.report))


class ImageContextTests(unittest.TestCase):
    def test_recipe_packs_runtime_sql_and_keeps_current_proto_layout(self):
        root = Path(__file__).resolve().parents[3]
        text = (root / "infra/mcr/backend/Dockerfile").read_text()
        self.assertIn("WORKDIR /mcr/backend", text)
        self.assertIn("COPY backend/ ./", text)
        self.assertIn("cargo +1.88.0 build --release --locked", text)
        self.assertIn("/mcr/backend/services/${SERVICE}/migrations /mcr/backend/services/${SERVICE}/migrations", text)
        self.assertIn("runtime-grants.sql", text)
        self.assertIn("USER mcr", text)
        for service in runtime.START_ORDER:
            self.assertTrue((root / "backend/services" / service / "migrations").is_dir())
            self.assertTrue((root / "backend/services" / service / "runtime-grants.sql").is_file())
        self.assertTrue((root / "backend/proto/authority/v1/authority.proto").is_file())


if __name__ == "__main__":
    unittest.main()
