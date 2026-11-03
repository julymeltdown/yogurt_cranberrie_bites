"""Qualification failures cannot leak secrets or turn into successful receipts."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

import browser_auth_smoke as smoke


class BrowserTests(unittest.TestCase):
    def test_only_explicit_loopback_targets_are_accepted_before_connecting(self):
        with patch.object(smoke.http.client, 'HTTPConnection') as connection:
            for origin in ('https://hostile.test', 'http://localhost', 'http://user@localhost:5', 'http://127.0.0.1:5/path'):
                with self.subTest(origin=origin), self.assertRaises(smoke.dev.SmokeError):
                    smoke.Browser(origin, {'statuses': {}})
            connection.assert_not_called()

    def response(self, *, status=200, headers=None, payload=b'{}'):
        response = Mock()
        response.status = status
        response.read.return_value = payload
        response.getheaders.return_value = list((headers or {
            'cache-control': 'no-store', 'x-correlation-id': 'be03-check',
            'referrer-policy': 'no-referrer'}).items())
        return response

    def test_observed_failure_records_status_but_not_private_body_and_closes_connection(self):
        report = {'statuses': {}}
        connection = Mock()
        connection.getresponse.return_value = self.response(status=500, payload=b'private-password-reset-token')
        with patch.object(smoke.http.client, 'HTTPConnection', return_value=connection):
            with self.assertRaisesRegex(smoke.CheckFailure, '^check:status$'):
                smoke.Browser('http://127.0.0.1:12345', report).request('check', '/api/auth/login')
        connection.close.assert_called_once()
        self.assertEqual(report, {'statuses': {'check': 500}})

    def test_success_requires_cache_correlation_referrer_and_no_redirect(self):
        for headers in [
            {'cache-control':'public','x-correlation-id':'be03-check','referrer-policy':'no-referrer'},
            {'cache-control':'no-store','x-correlation-id':'wrong','referrer-policy':'no-referrer'},
            {'cache-control':'no-store','x-correlation-id':'be03-check'},
            {'cache-control':'no-store','x-correlation-id':'be03-check','referrer-policy':'no-referrer','location':'https://hostile.test'},
        ]:
            connection = Mock()
            connection.getresponse.return_value = self.response(headers=headers)
            with patch.object(smoke.http.client, 'HTTPConnection', return_value=connection), self.assertRaises(smoke.CheckFailure):
                smoke.Browser('http://127.0.0.1:12345', {'statuses':{}}).request('check', '/api/auth/login')
            connection.close.assert_called_once()

    def test_error_trace_must_match_safe_header(self):
        connection = Mock()
        connection.getresponse.return_value = self.response(status=401, payload=b'{"traceId":"wrong"}')
        with patch.object(smoke.http.client, 'HTTPConnection', return_value=connection), self.assertRaisesRegex(smoke.CheckFailure, 'error-correlation'):
            smoke.Browser('http://127.0.0.1:12345', {'statuses':{}}).request('check','/api/auth/refresh',expected=401)

    def test_cookie_attributes_are_checked_and_only_cookie_pair_is_reused(self):
        value = smoke.refresh_cookie({'set-cookie':'refresh_token=private; HttpOnly; Path=/; SameSite=Lax'})
        self.assertEqual(value, 'refresh_token=private')
        with self.assertRaises(smoke.CheckFailure):
            smoke.refresh_cookie({'set-cookie':'refresh_token=private; Path=/'})

    def test_auth_state_proof_requires_owned_cluster_before_any_query(self):
        cluster = Mock(spec=smoke.dev.migration.Cluster)
        cluster.assert_owned.side_effect = smoke.dev.SmokeError('ownership', 'foreign')
        with self.assertRaises(smoke.dev.SmokeError): smoke.auth_snapshot(cluster)
        cluster.sql.assert_not_called()


class ReceiptTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='mcr-be03-test-')
        self.addCleanup(self.temp.cleanup)
        self.report = Path(self.temp.name) / 'report.json'
        self.args = ['--bin-dir','/missing','--report',str(self.report)]

    def test_existing_evidence_is_never_overwritten_or_started(self):
        self.report.write_text('preserved')
        with patch.object(smoke.dev,'DevStack') as stack, contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit): smoke.main(self.args)
        stack.assert_not_called()
        self.assertEqual(self.report.read_text(), 'preserved')

    def test_startup_exception_still_cleans_owned_resources_without_exception_text(self):
        stack = Mock()
        stack.report = {'cleanup':'PASS'}
        stack.start.side_effect = RuntimeError('private-password-token')
        with patch.object(smoke.dev,'DevStack',return_value=stack), contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(smoke.main(self.args),1)
        stack.close.assert_called_once()
        report = json.loads(self.report.read_bytes())
        self.assertEqual(report['failure'],'RuntimeError')
        self.assertNotIn('private-password-token',self.report.read_text())
        self.assertEqual(self.report.stat().st_mode & 0o777,0o600)

    def test_cleanup_failure_overrides_exercise_success(self):
        stack = Mock()
        stack.report = {'cleanup':'FAIL'}
        with patch.object(smoke.dev,'DevStack',return_value=stack), patch.object(smoke,'exercise'), contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(smoke.main(self.args),1)
        self.assertEqual(json.loads(self.report.read_bytes())['result'],'FAIL')
        stack.close.assert_called_once()


if __name__ == '__main__': unittest.main()
