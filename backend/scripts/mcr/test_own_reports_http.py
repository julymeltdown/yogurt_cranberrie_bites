#!/usr/bin/env python3
"""Report HTTP guard regressions. These do not certify actual service behavior."""
from __future__ import annotations

import copy
import json
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch
from urllib.parse import urlsplit

import own_reports_http as report


ACTOR = "11111111-1111-4111-8111-111111111111"
EVENT = "22222222-2222-4222-8222-222222222222"
TARGET = "33333333-3333-4333-8333-333333333333"
RESOURCE = "44444444-4444-4444-8444-444444444444"
KEY = "55555555-5555-4555-8555-555555555555"


class PublicBoundary(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.oracle = report.contract()

    def setUp(self):
        self.result = {"statuses": {}}
        browser = SimpleNamespace(target=urlsplit("http://127.0.0.1:25001"), origin="http://127.0.0.1:5173")
        self.client = report.Http(browser, EVENT, self.result, self.oracle, report.time.time() + 30)
        self.safe = {"report_id": RESOURCE, "event_id": EVENT, "target_type": "USER", "target_id": TARGET,
                     "state": "OPEN", "revision": 1, "created_at": "2026-11-04T17:00:00Z"}

    def response(self, value, status=200, headers=None, raw=None):
        response = Mock(status=status)
        response.read.return_value = json.dumps(value).encode() if raw is None else raw
        values = {"Cache-Control": "no-store", "Content-Type": "application/json", **(headers or {})}
        response.getheader.side_effect = lambda key, default=None: values.get(key, default)
        connection = Mock()
        connection.getresponse.return_value = response
        return connection

    def call(self, connection, **kwargs):
        with patch.object(report.http.client, "HTTPConnection", return_value=connection):
            return self.client.call("test", "createConcertReport", ACTOR, "RAM_ONLY_TOKEN", "/reports",
                                    payload={"target_type": "USER"}, key=KEY, **kwargs)

    def test_actual_gateway_target_and_frontend_origin_remain_distinct(self):
        connection = self.response(self.safe)
        result = self.call(connection)
        self.assertEqual(result, self.safe)
        args = connection.request.call_args
        self.assertEqual(args.args[:2], ("POST", f"/api/concert/events/{EVENT}/reports"))
        self.assertEqual(args.kwargs["headers"]["Origin"], "http://127.0.0.1:5173")
        self.assertEqual(args.kwargs["headers"]["Idempotency-Key"], KEY)
        self.assertNotIn("RAM_ONLY_TOKEN", json.dumps(self.result))
        connection.close.assert_called_once_with()

    def test_safe_projection_rejects_evidence_reporter_text_and_foreign_event(self):
        for field, value in (("reporter_id", ACTOR), ("text", "private canary"),
                             ("reason", "HARASSMENT"), ("evidence_media_ids", []), ("event_id", TARGET)):
            with self.subTest(field=field):
                with self.assertRaises(ValueError):
                    self.call(self.response({**self.safe, field: value}))

    def test_utc_format_and_duplicate_keys_are_checked_by_strict_oracle(self):
        for value in ("invalid-time", "2026-99-99T17:00:00Z"):
            with self.assertRaises(ValueError):
                self.call(self.response({**self.safe, "created_at": value}))
        with self.assertRaises(report.launcher.InputRefusal):
            self.call(self.response({}, raw=b'{"state":"OPEN","state":"RESOLVED"}'))

    def test_terminal_receipt_cannot_be_relabelled_as_pending_202(self):
        receipt = {"receipt_id": RESOURCE, "event_id": EVENT, "actor_id": ACTOR,
                   "operation_id": "createConcertReport", "idempotency_key": KEY,
                   "state": "SUCCEEDED", "resource_id": RESOURCE, "resource_revision": 1,
                   "failure_reason": None, "created_at": "2026-11-04T17:00:00Z",
                   "expires_at": "2026-11-05T17:00:00Z", "result_status": 200}
        with self.assertRaisesRegex(ValueError, "202 must remain pending"):
            self.call(self.response(receipt, 202), expected=(202,))
        pending = {**receipt, "state": "PENDING", "resource_id": None,
                   "resource_revision": None, "result_status": None}
        self.assertEqual(self.call(self.response(pending, 202), expected=(202,)), pending)

    def test_status_cache_redirect_and_size_refusals_always_close_connection(self):
        for connection in (self.response(self.safe, 503), self.response(self.safe, headers={"Cache-Control": "public"}),
                           self.response(self.safe, headers={"Location": "https://elsewhere.invalid"}),
                           self.response(self.safe, raw=b"x" * 131073)):
            with self.assertRaises(report.launcher.InputRefusal):
                self.call(connection)
            connection.close.assert_called_once_with()

    def test_expired_work_does_not_open_any_connection(self):
        self.client.deadline = report.time.time() - 1
        with patch.object(report.http.client, "HTTPConnection", side_effect=AssertionError("late HTTP")):
            with self.assertRaisesRegex(ValueError, "http-work-deadline"):
                self.client.call("expired", "listOwnConcertReports", ACTOR, "TOKEN", "/me/reports")

    def test_incomplete_body_preserves_framing_without_private_partial_or_credentials(self):
        connection = self.response({}, headers={"Transfer-Encoding": "chunked"})
        connection.getresponse.return_value.read.side_effect = report.http.client.IncompleteRead(
            b'private reporter canary RAM_ONLY_TOKEN', 23)
        with self.assertRaises(report.http.client.IncompleteRead):
            self.call(connection)
        self.assertEqual(self.result["transport_failure"], {
            "label": "test", "operation_id": "createConcertReport", "type": "IncompleteRead",
            "response_status": 200, "partial_bytes": 38, "expected_remaining_bytes": 23,
            "content_length": None, "transfer_encoding": "CHUNKED",
        })
        self.assertEqual(self.result["statuses"], {})
        for secret in ("private reporter canary", "RAM_ONLY_TOKEN", KEY):
            self.assertNotIn(secret, json.dumps(self.result))
        connection.close.assert_called_once_with()

    def test_failure_probe_runs_after_socket_close_and_preserves_original_error(self):
        connection = self.response({})
        error = report.http.client.IncompleteRead(b"private", 7)
        connection.getresponse.return_value.read.side_effect = error
        counts = {key: 0 for key in report.FAILURE_COUNTS}
        counts.update(scopes=4, registered_native=4, confirmed_native=4, operations_witnesses=4)

        def probe(operation):
            connection.close.assert_called_once_with()
            self.assertEqual(operation, "createConcertReport")
            return counts

        self.client.failure_probe = probe
        with self.assertRaises(report.http.client.IncompleteRead) as raised:
            self.call(connection)
        self.assertIs(raised.exception, error)
        self.assertEqual(self.result["transport_failure"]["settlement_probe"], {
            "scope": "EVENT_OPERATION_AGGREGATE", "state": "CAPTURED", "counts": counts})
        self.assertEqual(self.result["statuses"], {})

    def test_unsafe_or_failed_probe_cannot_publish_data_or_mask_delivery_error(self):
        counts = {key: 0 for key in report.FAILURE_COUNTS}
        for unsafe in ({**counts, "binding": "PRIVATE_PROBE_CANARY"},
                       {**counts, "scopes": True}, {**counts, "scopes": -1},
                       {**counts, "scopes": "PRIVATE_PROBE_CANARY"},
                       RuntimeError("PRIVATE_PROBE_CANARY")):
            with self.subTest(kind=type(unsafe).__name__):
                connection = self.response({})
                error = report.http.client.IncompleteRead(b"private", 7)
                connection.getresponse.return_value.read.side_effect = error
                self.client.failure_probe = Mock(side_effect=unsafe) if isinstance(unsafe, Exception) else Mock(return_value=unsafe)
                with self.assertRaises(report.http.client.IncompleteRead) as raised:
                    self.call(connection)
                self.assertIs(raised.exception, error)
                self.assertEqual(self.result["transport_failure"]["settlement_probe"], {
                    "scope": "EVENT_OPERATION_AGGREGATE", "state": "UNAVAILABLE"})
                self.assertNotIn("PRIVATE_PROBE_CANARY", json.dumps(self.result))

    def test_failure_probe_does_not_start_when_original_work_budget_is_insufficient(self):
        connection = self.response({})
        connection.getresponse.return_value.read.side_effect = report.http.client.IncompleteRead(b"", 1)
        self.client.failure_probe = Mock(side_effect=AssertionError("late probe"))
        self.client.deadline = 1234.5
        with patch.object(report.time, "time", return_value=1234):
            with self.assertRaises(report.http.client.IncompleteRead):
                self.call(connection)
        self.client.failure_probe.assert_not_called()
        self.assertEqual(self.result["transport_failure"]["settlement_probe"]["state"], "SKIPPED_WORK_DEADLINE")


class FailureProbeBudget(unittest.TestCase):
    def test_probe_is_read_only_bounded_and_returns_only_the_json_counts(self):
        process = Mock(returncode=0)
        process.poll.return_value = 0
        counts = {key: 0 for key in report.FAILURE_COUNTS}
        process.communicate.return_value = (json.dumps(counts), "PRIVATE_STDERR_CANARY")
        with patch.object(report, "psql", return_value=process) as spawn, \
                patch.object(report.time, "time", return_value=1234):
            result = report.settlement_failure_counts(Mock(), EVENT, "createConcertReport", 1235)
        self.assertEqual(result, counts)
        self.assertEqual(process.communicate.call_args.kwargs["timeout"], .5)
        self.assertIn("BEGIN READ ONLY", spawn.call_args.args[3])
        self.assertIn("statement_timeout='250ms'", spawn.call_args.args[3])
        self.assertIn("ROLLBACK", spawn.call_args.args[3])
        process.kill.assert_not_called()
        process.stdout.close.assert_called_once_with()
        self.assertNotIn("PRIVATE_STDERR_CANARY", json.dumps(result))

    def test_timeout_kills_probe_and_reaps_it_without_a_new_query(self):
        process = Mock()
        process.communicate.side_effect = report.subprocess.TimeoutExpired("psql", .5)
        process.poll.side_effect = [None, -9]
        with patch.object(report, "psql", return_value=process) as spawn, \
                patch.object(report.time, "time", return_value=1234):
            with self.assertRaises(report.subprocess.TimeoutExpired):
                report.settlement_failure_counts(Mock(), EVENT, "createConcertReport", 1235)
        process.kill.assert_called_once_with()
        spawn.assert_called_once()
        process.stdout.close.assert_called_once_with()


class StartupAndLifecycle(unittest.TestCase):
    def test_report_user_keys_are_independent_of_existing_managed_features(self):
        envs = {name: {} for name in report.launcher.SERVICES if name != "concert-service"}
        concert = {}
        urls = {role: "https://127.0.0.1:" + str(2000 + n) for n, role in
                enumerate(("auth", "friend", "post", "notification", "concert"))}
        report.report_features(report.launcher.managed_features, envs, concert, urls,
                               (("post-seed", "post-public"), ("callback-seed", "callback-public")))
        self.assertEqual(concert["CONCERT_REPORT_MODE"], "user")
        self.assertNotEqual(concert["CONCERT_REPORT_CURSOR_KEY_HEX"],
                            json.loads(concert["CONCERT_REPORT_RECEIPT_KEYS_JSON"])[0]["secret_hex"])
        self.assertFalse(any("OPERATOR" in name or "AGE_PROVIDER" in name for name in concert))

    def test_missing_format_plugin_refuses_before_native_launch(self):
        original = report.importlib.util.module_from_spec

        def missing(spec):
            module = original(spec)
            if spec.name != "mcr_own_reports_c04":
                return module
            loader = spec.loader.exec_module

            def load(value):
                loader(value)
                value.FORMATS = copy.copy(value.FORMATS)
                value.FORMATS.checkers = dict(value.FORMATS.checkers)
                value.FORMATS.checkers.pop("date-time", None)
            spec.loader.exec_module = load
            return module

        with patch.object(report.importlib.util, "module_from_spec", side_effect=missing):
            with self.assertRaisesRegex(ValueError, "^all-schema-format-plugins-required:date-time$"):
                report.contract()

    def test_cli_selects_wrapped_runner_once_and_restores_launcher(self):
        original = report.launcher.run
        selected = []
        with patch.object(report.launcher, "main", side_effect=lambda argv: selected.append(report.launcher.run) or 2):
            self.assertEqual(report.main(["--invalid"]), 2)
        self.assertEqual(selected, [report.run])
        self.assertIs(report.launcher.run, original)
        self.assertIsNot(report.STACK_RUN, report.run)


if __name__ == "__main__":
    unittest.main()
