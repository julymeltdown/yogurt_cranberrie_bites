import json
import unittest
import product_api_smoke as smoke


class ProductProbeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.contract = json.loads(smoke.CONTRACT.read_text())

    def response(self, status=200, value=None, content_type="application/json", extra=None):
        headers = {"cache-control": "no-store", "content-type": content_type, **(extra or {})}
        return smoke.validate_response(self.contract, "/api/concert/events/{eventId}/feed", "GET",
                                       status, headers, smoke.encode(value if value is not None else {"items": [], "next_cursor": None}))

    def test_exact_feed_schema_accepts_empty_page_without_claiming_owner_rows(self):
        self.assertEqual(self.response(), {"items": [], "next_cursor": None})

    def test_unknown_private_fields_and_undocumented_status_fail(self):
        for value in ({"items": [], "next_cursor": None, "private_email": "canary"},
                      {"items": [{"event_id": "foreign"}], "next_cursor": None}):
            with self.assertRaises(smoke.ProbeError):
                self.response(value=value)
        with self.assertRaises(smoke.ProbeError):
            self.response(status=201)

    def test_error_reason_must_match_exact_endpoint_status(self):
        value = {"code": "CONCERT_403", "message": "Unavailable", "details": {"reason": "PERMISSION_DENIED"}}
        self.assertEqual(self.response(status=403, value=value), value)
        value["details"]["reason"] = "IDEMPOTENCY_CONFLICT"
        with self.assertRaises(smoke.ProbeError):
            self.response(status=403, value=value)

    def test_duplicate_json_and_cacheable_private_responses_fail(self):
        headers = {"cache-control": "no-store", "content-type": "application/json"}
        with self.assertRaises(smoke.ProbeError):
            smoke.validate_response(self.contract, "/api/concert/events/{eventId}/feed", "GET", 200,
                                    headers, b'{"items":[],"items":[],"next_cursor":null}')
        with self.assertRaises(smoke.ProbeError):
            self.response(extra={"cache-control": "public, max-age=3600"})

    def test_private_content_requires_all_protection_headers(self):
        headers = {"cache-control": "no-store", "content-type": "image/png",
                   "x-content-type-options": "nosniff", "content-disposition": "inline",
                   "cross-origin-resource-policy": "same-origin"}
        path = "/api/concert/events/{eventId}/media/{mediaId}/content"
        self.assertEqual(smoke.validate_response(self.contract, path, "GET", 200, headers, b"sanitized"), b"sanitized")
        for key in headers:
            with self.assertRaises(smoke.ProbeError):
                smoke.validate_response(self.contract, path, "GET", 200,
                                        {k: v for k, v in headers.items() if k != key}, b"sanitized")

    def test_probe_refuses_credential_urls_plain_remote_http_and_redirects(self):
        for value in ("http://example.com", "https://user:secret@example.com", "https://example.com/private",
                      "https://example.com?token=private", "file:///private"):
            with self.assertRaises(smoke.ProbeError):
                smoke.base_url(value)
        self.assertEqual(smoke.base_url("http://127.0.0.1:3000/"), "http://127.0.0.1:3000")
        self.assertIsNone(smoke.NoRedirect().redirect_request(None, None, None, None, None, None))


if __name__ == "__main__":
    unittest.main()
