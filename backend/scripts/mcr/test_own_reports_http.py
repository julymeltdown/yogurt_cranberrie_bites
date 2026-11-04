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
            with self.assertRaisesRegex(ValueError, "all-schema-format-plugins-required"):
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
