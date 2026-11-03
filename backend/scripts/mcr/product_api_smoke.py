#!/usr/bin/env python3
"""Probe actual configured C04 HTTP owners; never creates a fixture authority.

This is HTTP evidence only. Owner-fence crash/revocation/rollback qualification
requires the separate native producer gates; this script never certifies them.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import urllib.error
import urllib.parse
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[3]
CONTRACT = ROOT / "backend/docs/api/mcr-domain.openapi.json"
MAX_BYTES = 10 * 1024 * 1024


class ProbeError(Exception):
    """Fixed diagnostics only: no bodies, credentials, private URLs or SQL."""


def require(condition, message):
    if not condition:
        raise ProbeError(message)


def canonical_uuid(value):
    try:
        parsed = uuid.UUID(value)
        require(parsed.int != 0 and str(parsed) == value, "Noncanonical probe UUID")
        return value
    except (ValueError, AttributeError, TypeError):
        raise ProbeError("Invalid probe UUID") from None


def base_url(value):
    parsed = urllib.parse.urlsplit(value)
    require(parsed.scheme in ("http", "https") and parsed.hostname
            and not parsed.username and not parsed.password and not parsed.query
            and not parsed.fragment and parsed.path in ("", "/"), "Unsafe probe base URL")
    require(parsed.scheme == "https" or parsed.hostname in ("127.0.0.1", "localhost", "::1"),
            "Plain HTTP probes require loopback")
    return value.rstrip("/")


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, "Duplicate JSON response field")
        result[key] = value
    return result


def validate_response(document, path, method, status, headers, body):
    """Use the exact frozen endpoint/status schema, including local references."""
    require(0 < len(body) <= MAX_BYTES, "Invalid response body size")
    require(headers.get("cache-control") == "no-store", "Missing no-store response")
    operation = document["paths"][path][method.lower()]
    response = operation["responses"].get(str(status))
    require(response is not None, "Undocumented HTTP status")
    content_type = headers.get("content-type", "").split(";", 1)[0]
    content = response.get("content", {})
    require(content_type in content, "Undocumented response content type")
    if content_type.startswith("image/"):
        require(status == 200 and headers.get("x-content-type-options") == "nosniff"
                and headers.get("content-disposition") == "inline"
                and headers.get("cross-origin-resource-policy") == "same-origin",
                "Unsafe private image response")
        return body
    require(content_type == "application/json", "Unexpected representation")
    try:
        value = json.loads(body, object_pairs_hook=unique_object)
    except (ValueError, UnicodeError):
        raise ProbeError("Malformed JSON response") from None
    import jsonschema
    schema = {"$schema": "https://json-schema.org/draft/2020-12/schema",
              "components": document["components"], **content[content_type]["schema"]}
    validator = jsonschema.Draft202012Validator(schema, format_checker=jsonschema.FormatChecker())
    require(validator.is_valid(value), "Frozen C04 response schema mismatch")
    return value


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


class Client:
    def __init__(self, base, token, document, origin=None):
        self.base = base_url(base)
        require(token and not any(char.isspace() for char in token), "Missing/invalid probe access token")
        self.token = token
        self.document = document
        self.origin = origin or self.base
        self.opener = urllib.request.build_opener(NoRedirect())
        self.observations = []

    def call(self, method, target, contract_path, *, body=None, content_type=None, key=None,
             expected=(200,)):
        require(target.startswith("/api/concert/events/") and not target.startswith("//"),
                "Unexpected probe target")
        headers = {"Authorization": "Bearer " + self.token}
        if method in ("POST", "PUT"):
            require(key is not None and content_type is not None and body is not None,
                    "Incomplete mutation probe")
            headers.update({"Origin": self.origin, "X-MCR-Intent": "browser",
                            "Idempotency-Key": canonical_uuid(key), "Content-Type": content_type})
        request = urllib.request.Request(self.base + target, data=body, headers=headers, method=method)
        try:
            response = self.opener.open(request, timeout=12)
        except urllib.error.HTTPError as error:
            response = error
        except (OSError, ValueError):
            raise ProbeError("HTTP probe transport failed") from None
        with response:
            status = response.code
            response_headers = {name.lower(): value for name, value in response.headers.items()}
            raw = response.read(MAX_BYTES + 1)
        require(status in expected, "Unexpected probe HTTP status")
        value = validate_response(self.document, contract_path, method, status, response_headers, raw)
        self.observations.append({"operation": self.document["paths"][contract_path][method.lower()]["operationId"],
                                  "status": status, "bytes": len(raw)})
        return value


def encode(value):
    return json.dumps(value, separators=(",", ":"), allow_nan=False).encode()


def probe(args):
    event = canonical_uuid(args.event)
    document = json.loads(CONTRACT.read_text())
    client = Client(args.base_url, os.environ.get(args.access_token_env), document, args.origin)
    root = "/api/concert/events/" + event
    template = "/api/concert/events/{eventId}"
    report = {"result": "IN_PROGRESS", "contract_sha256": hashlib.sha256(CONTRACT.read_bytes()).hexdigest(),
              "revocation_ordering": "NOT_RUN", "rollback": "NOT_RUN", "owner_crash_settlement": "NOT_RUN"}
    require(args.expected_post or args.upload_file, "Choose a real Feed row or upload probe")
    if args.expected_post:
        expected = canonical_uuid(args.expected_post)
        page = client.call("GET", root + "/feed?limit=100", template + "/feed")
        require(any(row["post_id"] == expected for row in page["items"]), "Expected owner Feed row not observed")
        require(all(row["event_id"] == event for row in page["items"]), "Cross-event Feed response")
        report["feed_owner_row"] = "OBSERVED"
    if args.upload_file:
        require(args.foreign_access_token_env, "Owner upload probe requires foreign-account denial probe")
        raw = args.upload_file.read_bytes()
        require(0 < len(raw) <= MAX_BYTES, "Invalid upload fixture size")
        require(args.mime in ("image/png", "image/jpeg", "image/webp"), "Invalid upload fixture MIME")
        keys = [str(uuid.uuid4()) for _ in range(3)]
        intent = client.call("POST", root + "/media/uploads", template + "/media/uploads",
                             body=encode({"purpose": "POST", "content_type": args.mime,
                                          "byte_length": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}),
                             content_type="application/json", key=keys[0])
        media = canonical_uuid(intent["media_id"])
        target = root + "/media/" + media
        media_template = template + "/media/{mediaId}"
        uploaded = client.call("PUT", target + "/bytes", media_template + "/bytes", body=raw,
                               content_type=args.mime, key=keys[1])
        finalize = encode({"expected_revision": uploaded["revision"]})
        ready = client.call("POST", target + "/finalize", media_template + "/finalize", body=finalize,
                            content_type="application/json", key=keys[2])
        replay = client.call("POST", target + "/finalize", media_template + "/finalize", body=finalize,
                             content_type="application/json", key=keys[2])
        require(ready == replay and ready["state"] == "READY", "Finalize replay changed result or is not READY")
        receipt = client.call("GET", root + "/commands?" + urllib.parse.urlencode({
            "operation_id": "finalizeConcertUpload", "idempotency_key": keys[2]}), template + "/commands")
        require(receipt["state"] == "SUCCEEDED", "Finalized command did not recover SUCCEEDED")
        client.call("GET", target, media_template)
        client.call("GET", target + "/content", media_template + "/content")
        foreign = Client(args.base_url, os.environ.get(args.foreign_access_token_env), document, args.origin)
        foreign.call("GET", target, media_template, expected=(403,))
        foreign.call("GET", target + "/content", media_template + "/content", expected=(403,))
        report["foreign_account"] = foreign.observations
        report["media_replay_and_recovery"] = "OBSERVED"
    report.update(result="HTTP_SMOKE_PASS", observations=client.observations)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", required=True)
    parser.add_argument("--event", required=True)
    parser.add_argument("--access-token-env", default="MCR_PRODUCT_ACCESS_TOKEN")
    parser.add_argument("--foreign-access-token-env")
    parser.add_argument("--origin")
    parser.add_argument("--expected-post")
    parser.add_argument("--upload-file", type=Path)
    parser.add_argument("--mime", default="image/png")
    args = parser.parse_args()
    try:
        report = probe(args)
    except ProbeError as error:
        print(json.dumps({"result": "FAIL", "failure": str(error)}))
        return 1
    except Exception:
        print(json.dumps({"result": "FAIL", "failure": "Probe configuration or dependency failure"}))
        return 1
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
