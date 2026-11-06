#!/usr/bin/env python3
"""Development-only World-capable vertical stack for the closure flow.

Banner: this is not a QA08 capacity run, not IN03 cohort admission and not a
release qualification. It starts this checkout's native binaries on an owned
private PG16 cluster, drives the public HTTP closure route and records the
truthful status of every step. A step the composed product cannot reach is
reported with its exact status and consumer, never simulated.
"""
from __future__ import annotations

import argparse
import base64
import http.client
import json
import secrets
import subprocess
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime, timedelta, timezone
from pathlib import Path
from unittest.mock import patch
from uuid import UUID, uuid4

sys.path.insert(0, str(Path(__file__).resolve().parent))
import browser_refresh  # noqa: E402
import dev_stack  # noqa: E402
import migration_smoke as migration  # noqa: E402
import runtime_smoke as runtime  # noqa: E402
import world_in01_live as world  # noqa: E402
import world_in01_prepare as prep  # noqa: E402

BANNER = {
    "scope": "development vertical flow",
    "release": "development release bundle, NOT_ADMITTED",
    "qa08": "NOT_RUN",
    "in03_cohort": "NOT_CLAIMED",
    "release_qualification": "NOT_CLAIMED",
}
# The public archive projection may name only these fields. Identity, handle and
# media storage references stay server-private even to an authenticated reader.
PUBLIC_ARCHIVE_POST_FIELDS = {"post_id", "text", "media_ids", "created_at"}


class StepFailure(RuntimeError):
    pass


def require(condition, label):
    if not condition:
        raise StepFailure(label)


def request(url, method="GET", payload=None, token=None, origin=None, intent=True, extra=None,
            cookie=None):
    body = None if payload is None else json.dumps(payload).encode()
    headers = {}
    if payload is not None:
        headers["Content-Type"] = "application/json"
    if token:
        headers["Authorization"] = "Bearer " + token
    if origin:
        headers["Origin"] = origin
    if intent:
        headers["X-MCR-Intent"] = "browser"
    if cookie:
        headers["Cookie"] = cookie
    if extra:
        headers.update(extra)
    req = urllib.request.Request(url, data=body, method=method, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=30) as response:
            return response.status, json.loads(response.read() or b"{}"), dict(response.headers)
    except urllib.error.HTTPError as failure:
        try:
            raw = failure.read()
        except http.client.IncompleteRead as error:
            raw = error.partial
        try:
            return failure.code, json.loads(raw or b"{}"), dict(failure.headers)
        except ValueError:
            return failure.code, {"raw": raw[:200].decode("utf-8", "replace")}, dict(failure.headers)
    except http.client.IncompleteRead as failure:
        return 0, {"raw": failure.partial[:200].decode("utf-8", "replace")}, {}
    except Exception as failure:  # noqa: BLE001 - transport failure is a recorded status
        return 0, {"raw": type(failure).__name__}, {}


def cookie_pair(headers, name):
    for key, value in headers.items():
        if key.lower() == "set-cookie" and value.startswith(name + "="):
            return value.split(";", 1)[0]
    return None


class EnrollmentMailSink(dev_stack.MailSink):
    """Accepts the canonical enrollment payload in addition to legacy mail."""

    ENROLLMENT_FIELDS = {"email", "verification_code", "expires_at",
                         "enrollment_attempt_id", "verification_link"}

    def accept(self, path, delivery_id, payload):
        if path != "/verify" or not isinstance(payload, dict) or set(payload) != self.ENROLLMENT_FIELDS:
            return super().accept(path, delivery_id, payload)
        self.cluster.assert_owned()
        if self.directory.is_symlink() or not self.directory.is_dir():
            raise migration.SmokeError("mail", "private inbox was replaced")
        if not all(isinstance(value, str) and 0 < len(value) <= 4096 for value in payload.values()):
            raise ValueError
        target = self.directory / (str(UUID(delivery_id)) + ".json")
        value = {"kind": "VERIFY_EMAIL", "delivery_id": str(UUID(delivery_id)),
                 "payload": payload}
        data = dev_stack.encode(value)
        if target.exists() or target.is_symlink():
            return 204 if not target.is_symlink() and target.read_bytes() == data else 409
        if self.count >= 1000:
            return 507
        runtime.private_file(target, data)
        self.count += 1
        return 204


def initialize_profile(gateway, user, token, ordinal, origin):
    status, body, _ = request(
        f"{gateway}/api/profile/{user}", "PUT",
        {"username": "closure_" + secrets.token_hex(6),
         "nickname": f"Closure participant {ordinal}",
         "avatarUrl": "/world/avatars/derived-v1/photo01-front-idle-64-e124c88815b22220.png",
         "bio": "Development closure participant"},
        token=token, origin=origin,
    )
    require(status == 200 and body.get("userId") == user,
            f"profile {ordinal} initialization failed: {status}")


def publish_enrollment_policy(cluster):
    """Explicit development operator publication; no default policy is seeded."""
    now = datetime.now(timezone.utc)
    revision = str(uuid4())
    publication = {
        "revision": revision,
        "documents": [{
            "id": "development-terms",
            "title": "Development terms",
            "url": "https://example.test/development-terms",
            "sha256": secrets.token_hex(32),
            "requiredAcknowledgementId": "development_terms_v1",
        }],
        "approvalReference": "development-not-admitted",
        "publishedAt": (now - timedelta(minutes=1)).isoformat(),
        "expiresAt": (now + timedelta(days=1)).isoformat(),
    }
    cluster.sql(
        "mcr_auth",
        "BEGIN; SELECT singleton FROM mcr_enrollment_current WHERE singleton FOR UPDATE;"
        "INSERT INTO mcr_enrollment_policies(revision,publication,published_at,expires_at) VALUES("
        + prep.quote(revision) + "," + prep.quote(json.dumps(publication, separators=(",", ":")))
        + "::jsonb,clock_timestamp(),clock_timestamp()+interval '1 day');"
        "UPDATE mcr_enrollment_current SET revision=" + prep.quote(revision)
        + " WHERE singleton;COMMIT;",
    )
    return publication


def principal(gateway, mail, policy, ordinal, origin):
    status, body, headers = request(gateway + "/api/auth/enrollment/attempts", "POST",
                                    {"policyRevision": policy["revision"]}, origin=origin)
    require(status == 201, f"enrollment attempt {ordinal} failed: {status}")
    attempt = body["attemptId"]
    cookie = cookie_pair(headers, "mcr-enrollment-dev")
    require(cookie is not None, f"enrollment cookie {ordinal} missing")
    email = f"closure-{secrets.token_hex(8)}@example.test"
    password = secrets.token_urlsafe(20)
    acknowledgements = [document["requiredAcknowledgementId"] for document in policy["documents"]]
    status, body, _ = request(
        gateway + "/api/auth/enrollment/signup", "POST",
        {"attemptId": attempt, "policyRevision": policy["revision"],
         "acknowledgementIds": acknowledgements, "idempotencyKey": str(uuid4()),
         "email": email, "password": password},
        origin=origin, cookie=cookie,
    )
    require(status == 202, f"enrollment signup {ordinal} failed: {status}")
    code, deadline = None, time.monotonic() + 25
    while code is None and time.monotonic() < deadline:
        for path in mail.directory.glob("*.json"):
            message = json.loads(path.read_bytes())
            payload = message.get("payload", {})
            if (message["kind"] == "VERIFY_EMAIL" and payload.get("email") == email
                    and payload.get("enrollment_attempt_id") == attempt):
                code = payload["verification_code"]
        time.sleep(0.1)
    require(code is not None, f"enrollment mail {ordinal} missing")
    status, body, _ = request(gateway + "/api/auth/enrollment/verify", "POST",
                              {"attemptId": attempt, "code": code}, origin=origin, cookie=cookie)
    require(status == 200 and body.get("status") == "VERIFIED", f"enrollment verify {ordinal}: {status}")
    status, body, _ = request(gateway + "/api/auth/login", "POST",
                              {"email": email, "password": password}, origin=origin)
    require(status == 200 and body.get("accessToken"), f"login {ordinal} failed: {status}")
    user, token = body["userId"], body["accessToken"]
    status, admission, _ = request(gateway + "/api/auth/enrollment/admission", "GET", None,
                                   token=token, origin=origin)
    require(status == 200, f"admission {ordinal} failed: {status}")
    if admission.get("decision") == "CONSENT_REQUIRED":
        status, current, _ = request(gateway + "/api/auth/enrollment/policy", "GET", origin=origin)
        require(status == 200, f"renewed policy {ordinal} failed: {status}")
        current_acknowledgements = [document["requiredAcknowledgementId"]
                                    for document in current["documents"]]
        status, consent, _ = request(
            gateway + "/api/auth/enrollment/consent", "POST",
            {"policyRevision": current["revision"],
             "acknowledgementIds": current_acknowledgements,
             "idempotencyKey": str(uuid4())},
            token=token, origin=origin,
        )
        require(status == 200 and consent.get("decision") == "ADMITTED",
                f"enrollment consent {ordinal}: {status}")
        status, admission, _ = request(gateway + "/api/auth/enrollment/admission", "GET", None,
                                       token=token, origin=origin)
    require(admission.get("decision") == "ADMITTED", f"principal {ordinal} not admitted")
    return user, token


def b64url(raw):
    return base64.urlsafe_b64encode(raw).rstrip(b"=").decode()


def expired_access_token(cluster, private_pem, subject):
    """A genuinely signed, expired access token. The verifier is not changed."""
    now = int(time.time())
    header = {"alg": "RS256", "typ": "JWT", "kid": "mcr-runtime-jwt"}
    claims = {
        "sub": subject,
        "iss": "mcr-runtime",
        "aud": "mcr-runtime-api",
        "iat": now - 120,
        "exp": now - 60,
        "jti": str(uuid4()),
        "token_type": "access",
        "roles": [],
    }
    signing = (b64url(json.dumps(header, separators=(",", ":")).encode())
               + "." + b64url(json.dumps(claims, separators=(",", ":")).encode()))
    key = cluster.root / "expired-token-key.pem"
    key.write_bytes(private_pem.encode())
    key.chmod(0o600)
    try:
        signed = subprocess.run(
            ["/usr/bin/openssl", "dgst", "-sha256", "-sign", str(key)],
            input=signing.encode(), capture_output=True, check=True,
            env={**cluster.env, "OPENSSL_CONF": "/dev/null"}, timeout=10,
        ).stdout
    finally:
        key.unlink(missing_ok=True)
    require(len(signed) == 256, "expired token signature shape")
    return signing + "." + b64url(signed)


def concert_readiness(status, body, service):
    """Concert's operational listener probes only its own database."""
    require(service == "concert-service", "concert readiness service")
    try:
        value = json.loads(body)
        require(all(type(value[k]) is bool for k in ("ready", "degraded", "draining")),
                "concert readyz flags")
        require(isinstance(value["checks"], dict) and set(value["checks"]) == {"database"},
                "concert readyz checks")
        check = value["checks"]["database"]
        require(type(check["required"]) is bool and check["required"] is True
                and check["status"] in ("up", "down", "timed_out", "not_checked"), "concert readyz database")
        computed = not value["draining"] and check["status"] == "up"
        require(value["ready"] == computed and status == (200 if computed else 503)
                and value["degraded"] == (check["status"] != "up"), "concert readyz consistency")
        return {"http_status": status, "ready": value["ready"], "degraded": value["degraded"],
                "draining": value["draining"],
                "checks": {"database": {"required": True, "status": check["status"]}}}
    except (ValueError, KeyError, TypeError):
        raise StepFailure("concert readyz malformed") from None


def readiness(status, body, service):
    if service == "concert-service":
        return concert_readiness(status, body, service)
    return browser_refresh.current_readiness(status, body, service)


def log_excerpts(stack, names=("concert-service", "post-service", "auth-service", "friend-service", "api-gateway")):
    excerpts = {}
    for name in names:
        child = stack.children.get(name)
        if child is None:
            continue
        try:
            excerpts[name] = child.diagnostic()
        except Exception as failure:  # noqa: BLE001 - evidence collection never hides the flow
            excerpts[name] = f"unavailable: {type(failure).__name__}"
    return excerpts


def new_log(stack, offsets, names=("api-gateway", "concert-service", "post-service"), limit=1200):
    """Bytes each owned child logged since the previous step, redacted.

    A step that produced no line reports that fact instead of a fabricated
    excerpt; the step's HTTP status, body and durable row stay the evidence.
    """
    excerpt = {}
    for name in names:
        child = stack.children.get(name)
        if child is None:
            continue
        try:
            if child.log.is_symlink():
                excerpt[name] = "refusing replaced child log"
                continue
            with child.log.open("rb") as stream:
                stream.seek(offsets.get(name, 0))
                chunk = stream.read(limit)
                offsets[name] = stream.tell()
            text = migration.redact(chunk.decode("utf-8", "replace"), child.secrets).strip()
            if text:
                excerpt[name] = text
        except Exception as failure:  # noqa: BLE001 - evidence collection never hides the flow
            excerpt[name] = f"unavailable: {type(failure).__name__}"
    return excerpt or {"note": "no new child log bytes"}


def archive_shape(body):
    """Return the fields the public projection actually contained."""
    if not isinstance(body, dict):
        return {"shape": "non-object"}
    if "items" in body:
        items = body.get("items") or []
        fields = sorted({key for item in items if isinstance(item, dict) for key in item})
        return {"shape": "projection", "fields": fields,
                "items": len(items),
                "media_ids_empty": all(not item.get("media_ids") for item in items if isinstance(item, dict))}
    return {"shape": "error", "fields": sorted(body)}


def feed_rows(cluster, event):
    """Durable feed receipt/work/outbox and Post owner rows for one event."""
    e = prep.quote(event)

    def count(database, sql):
        return int(cluster.sql(database, sql))

    return {
        "concert_command_receipts": count(
            "mcr_concert",
            f"SELECT count(*) FROM concert_command_receipts WHERE event_id={e}",
        ),
        "concert_feed_owner_work": count(
            "mcr_concert",
            f"SELECT count(*) FROM concert_feed_owner_work WHERE event_id={e}",
        ),
        "concert_outbox": count(
            "mcr_concert", f"SELECT count(*) FROM concert_outbox WHERE event_id={e}"
        ),
        "post_posts": count(
            "mcr_post", f"SELECT count(*) FROM post_concert_posts WHERE event_id={e}"
        ),
        "post_receipts": count(
            "mcr_post", f"SELECT count(*) FROM post_concert_receipts WHERE event_id={e}"
        ),
        "post_outbox": count(
            "mcr_post", f"SELECT count(*) FROM post_concert_outbox WHERE event_id={e}"
        ),
    }


def feed_proof(stack, gateway, origin, event, operator, member, record, checks):
    """Authenticated C04 feed mutations through the real Gateway/Concert/Post path."""
    base = f"{gateway}/api/concert/events/{event}/posts"

    def call(name, method, url, payload, token, key):
        before = feed_rows(stack.cluster, event)
        status, body, _ = request(
            url, method, payload, token, origin, True,
            {"Idempotency-Key": key} if key else None,
        )
        after = feed_rows(stack.cluster, event)
        record(name, {"status": status, "body": body,
                      "rows_before": before, "rows_after": after})
        return status, body, before, after

    def unchanged(before, after):
        return before == after

    def call_retry(name, method, url, payload, token, key):
        status, body, before, after = call(name, method, url, payload, token, key)
        if status in (0, 502, 503, 504):
            status, body, before, after = call(
                name + "_retry", method, url, payload, token, key
            )
        return status, body, before, after

    create = {"text": "feed proof post", "media_ids": [], "links": []}
    create_key = str(uuid4())
    status, body, before, after = call_retry(
        "feed_create", "POST", base, create, member[1], create_key
    )
    post = body.get("post_id")
    revision = body.get("revision")
    checks["feed_create_200"] = status == 200 and isinstance(post, str) and revision == 1
    if not checks["feed_create_200"]:
        return

    status, replay, _, replay_after = call_retry(
        "feed_create_replay", "POST", base, create, member[1], create_key
    )
    checks["feed_replay_stored"] = (
        status == 200
        and replay.get("post_id") == post
        and replay.get("revision") == revision
        and replay.get("text") == create["text"]
        and replay_after == after
    )

    conflict = {"text": "feed proof conflict", "media_ids": [], "links": []}
    status, conflict_body, conflict_before, conflict_after = call(
        "feed_conflict", "POST", base, conflict, member[1], create_key
    )
    checks["feed_conflict_409"] = (
        status == 409
        and (conflict_body.get("details") or {}).get("reason") == "IDEMPOTENCY_CONFLICT"
        and unchanged(conflict_before, conflict_after)
    )

    edit = {"expected_revision": revision, "text": "feed proof edited",
            "media_ids": [], "links": []}
    status, edited, _, _ = call_retry(
        "feed_edit", "PATCH", f"{base}/{post}", edit, member[1], str(uuid4())
    )
    edited_revision = edited.get("revision")
    checks["feed_edit_200"] = (
        status == 200
        and edited.get("post_id") == post
        and edited.get("text") == edit["text"]
        and edited_revision == revision + 1
    )

    status, deleted, _, _ = call_retry(
        "feed_delete", "DELETE", f"{base}/{post}?expected_revision={edited_revision}",
        None, member[1], str(uuid4()),
    )
    checks["feed_delete_200"] = (
        status == 200 and deleted.get("resource_id") == post
        and deleted.get("state") == "DELETED"
    )

    status, second, _, _ = call_retry(
        "feed_second_create", "POST", base,
        {"text": "feed proof second", "media_ids": [], "links": []}, member[1], str(uuid4()),
    )
    second_post = second.get("post_id")
    second_revision = second.get("revision")
    checks["feed_second_create_200"] = (
        status == 200 and isinstance(second_post, str) and second_revision == 1
    )
    if not checks["feed_second_create_200"]:
        return

    comment = {"text": "feed proof comment", "post_revision": second_revision}
    status, comment_body, _, _ = call_retry(
        "feed_comment", "POST", f"{base}/{second_post}/comments", comment,
        member[1], str(uuid4()),
    )
    checks["feed_comment_200"] = (
        status == 200 and isinstance(comment_body.get("comment_id"), str)
        and comment_body.get("post_id") == second_post
    )

    status, reaction, _, _ = call_retry(
        "feed_reaction", "PUT", f"{base}/{second_post}/reaction", {},
        member[1], str(uuid4()),
    )
    checks["feed_reaction_200"] = (
        status == 200 and reaction.get("post_id") == second_post
        and reaction.get("reacted_by_me") is True
        and reaction.get("reaction_count") == 1
    )

    status, unauthorized, before, after = call(
        "feed_unauthorized", "PATCH", f"{base}/{second_post}", edit, operator[1], str(uuid4())
    )
    checks["feed_unauthorized_403_zero_rows"] = (
        status == 403 and unchanged(before, after)
    )

    status, missing, before, after = call(
        "feed_missing", "PATCH", f"{base}/{uuid4()}", edit, operator[1], str(uuid4())
    )
    checks["feed_missing_404_zero_rows"] = (
        status == 404 and unchanged(before, after)
    )

    stack.cluster.sql(
        "mcr_friend",
        "INSERT INTO user_blocks(blocker_id,blocked_id,created_at) VALUES("
        + prep.quote(member[0]) + "," + prep.quote(operator[0]) + ",clock_timestamp())"
        + " ON CONFLICT (blocker_id,blocked_id) DO UPDATE SET deleted_at=NULL,created_at=clock_timestamp()",
    )
    status, blocked, before, after = call(
        "feed_blocked", "POST", f"{base}/{second_post}/comments",
        {"text": "feed proof blocked", "post_revision": second_revision},
        operator[1], str(uuid4()),
    )
    checks["feed_blocked_403_or_404_zero_rows"] = (
        status in (403, 404) and unchanged(before, after)
    )
    stack.cluster.sql(
        "mcr_friend",
        "UPDATE user_blocks SET deleted_at=clock_timestamp() WHERE blocker_id="
        + prep.quote(member[0]) + " AND blocked_id=" + prep.quote(operator[0]),
    )
    record("feed_block_cleared", {
        "blocks": stack.cluster.sql(
            "mcr_friend",
            "SELECT count(*) FROM user_blocks WHERE blocker_id=" + prep.quote(member[0])
            + " AND blocked_id=" + prep.quote(operator[0]) + " AND deleted_at IS NULL",
        ),
    })


def venue_seed(event):
    place_id = str(uuid4())
    map_id = "closure_dev_map"
    cutoff = (datetime.now(timezone.utc) + timedelta(hours=24)).strftime("%Y-%m-%dT%H:%M:%SZ")
    provenance = {
        "source_id": "closure_dev",
        "source_kind": "PLACE_REFERENCE",
        "source_revision": "closure-dev-1",
        "source_uri": "https://development.invalid/venue-place",
        "author": "development fixture",
        "license": "development only",
        "review_state": "REVIEWED",
        "reviewed_by": "development reviewer",
        "reviewed_at": "2026-11-06T00:00:00Z",
        "photo_permission": "NOT_APPLICABLE",
    }
    statements = [
        "INSERT INTO concert_meeting_settings(event_id,event_cutoff) VALUES("
        + prep.quote(event) + "," + prep.quote(cutoff) + ")",
        "INSERT INTO concert_map_releases(map_id,map_revision,content_hash,geometry_hash,canonical_bytes) VALUES("
        + prep.quote(map_id) + ",1,'sha256:" + "0" * 64 + "','sha256:" + "0" * 64
        + "',decode('00','hex'))",
        "INSERT INTO concert_map_release_places(map_id,map_revision,place_id,label,place_kind,"
        "verification_state,accessibility_notes,source_kind,source_uri,review_state,provenance) VALUES("
        + ",".join(map(prep.quote, [map_id, "1", place_id, "Closure development place", "PHYSICAL",
                                    "VERIFIED", "Development-only reviewed place", "PLACE_REFERENCE",
                                    "https://development.invalid/venue-place", "REVIEWED"]))
        + "," + prep.quote(json.dumps(provenance)) + "::jsonb)",
        "INSERT INTO concert_place_current_state(map_id,map_revision,place_id,closure_state) VALUES("
        + ",".join(map(prep.quote, [map_id, "1", place_id, "OPEN"])) + ")",
        "INSERT INTO concert_events_map_releases(event_id,catalog_revision,map_id,map_revision) VALUES("
        + ",".join(map(prep.quote, [event, "1", map_id, "1"])) + ")",
    ]
    return place_id, statements


def venue_friend_pair(gateway, origin, first, second, record):
    for follower, target, target_token in ((first, second[0], second[1]), (second, first[0], first[1])):
        status, body, _ = request(
            f"{gateway}/api/follows", "POST", {"targetUserId": target}, follower[1], origin, True
        )
        record("venue_follow", {"status": status, "body": body})
        require(status == 200, f"venue friendship follow failed: {status} {body}")
        if body.get("following") is not True and body.get("requestId"):
            accepted, accepted_body, _ = request(
                f"{gateway}/api/follows/requests/{body['requestId']}/accept",
                "POST", None, target_token, origin, True,
            )
            record("venue_follow_accept", {"status": accepted, "body": accepted_body})
            require(accepted == 200, f"venue follow accept failed: {accepted} {accepted_body}")


def venue_proof(stack, gateway, origin, event, place_id, member, operator, stranger, record, checks):
    base = f"{gateway}/api/concert/events/{event}/venue"
    now = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%fZ")

    def call(name, method, path, payload, token, key=None, auth=True):
        extra = {"Idempotency-Key": key} if key else None
        status, body, _ = request(
            f"{base}{path}", method, payload, token if auth else None, origin, True, extra
        )
        record(name, {"status": status, "body": body})
        return status, body

    status, sharing = call(
        "venue_sharing", "PUT", "/location/sharing",
        {"expected_revision": 0, "state": "ACTIVE", "viewer_ids": [operator[0]]},
        member[1], str(uuid4()),
    )
    checks["venue_sharing_200"] = status == 200 and sharing.get("operation") == "setVenueLocationSharing"
    status, fix = call(
        "venue_fix", "PUT", "/location/fix",
        {"expected_sharing_revision": 1, "source": "DEVICE_GEOLOCATION_REPORTED",
         "latitude": 37.5665, "longitude": 126.9780, "accuracy_m": 12.0,
         "captured_at": now},
        member[1], str(uuid4()),
    )
    checks["venue_fix_200"] = status == 200 and fix.get("resource_kind") == "PHONE_FIX"
    status, snapshot = call("venue_location_friend", "GET", "/location", None, operator[1])
    friends = snapshot.get("friends") if isinstance(snapshot, dict) else None
    checks["venue_location_friend_200"] = (
        status == 200 and isinstance(friends, list) and len(friends) == 1
        and friends[0].get("user_id") == member[0]
        and (friends[0].get("position") or {}).get("latitude") == 37.5665
    )
    status, own = call("venue_location_no_consent", "GET", "/location", None, member[1])
    checks["venue_location_no_consent_empty"] = (
        status == 200 and isinstance(own, dict) and own.get("friends") == []
        and own.get("self_location", {}).get("position", {}).get("latitude") == 37.5665
    )
    status, block_body, _ = request(
        f"{gateway}/api/relationships/blocks/{operator[0]}", "POST", None, member[1], origin, True
    )
    record("venue_block_create", {"status": status, "body": block_body})
    require(status == 200, f"venue block failed: {status} {block_body}")
    status, blocked = call("venue_location_blocked", "GET", "/location", None, operator[1])
    checks["venue_location_blocked_no_location"] = (
        status == 200 and isinstance(blocked, dict) and blocked.get("friends") == []
    )
    status, unblock_body, _ = request(
        f"{gateway}/api/relationships/blocks/{operator[0]}", "DELETE", None, member[1], origin, True
    )
    record("venue_block_delete", {"status": status, "body": unblock_body})
    require(status == 200, f"venue unblock failed: {status} {unblock_body}")
    venue_friend_pair(gateway, origin, member, operator, record)
    status, _ = call("venue_location_unauth", "GET", "/location", None, None, auth=False)
    checks["venue_location_unauth_401"] = status == 401

    note_body = {
        "text": "Development venue note",
        "audience": "FRIENDS",
        "expiry": "ONE_HOUR",
        "landmark_selection": {
            "landmark_id": place_id, "catalog_revision": 1, "landmark_revision": 1
        },
    }
    note_key = str(uuid4())
    status, created = call(
        "venue_note_create", "POST", f"/landmarks/{place_id}/notes",
        note_body, member[1], note_key,
    )
    note_id = ((created.get("resource") or {}).get("id") if isinstance(created, dict) else None)
    note_revision = ((created.get("resource") or {}).get("revision") if isinstance(created, dict) else None)
    checks["venue_note_create_201"] = (
        status == 201 and created.get("state") == "SUCCEEDED"
        and isinstance(note_id, str) and created.get("operation_id") == "createVenuePlaceNote"
    )
    status, replay = call(
        "venue_note_create_replay", "POST", f"/landmarks/{place_id}/notes",
        note_body, member[1], note_key,
    )
    checks["venue_note_create_replay_stable"] = (
        status == 201 and (replay.get("resource") or {}).get("id") == note_id
        and (replay.get("resource") or {}).get("revision") == note_revision
    )
    changed_note = dict(note_body)
    changed_note["text"] = "Development venue note changed"
    status, conflict = call(
        "venue_note_create_conflict", "POST", f"/landmarks/{place_id}/notes",
        changed_note, member[1], note_key,
    )
    checks["venue_note_create_conflict_409"] = (
        status == 409
        and (conflict.get("details") or {}).get("reason") == "VERSION_CONFLICT"
    )
    status_list, _ = call(
        "venue_note_read_list_retention", "GET", f"/landmarks/{place_id}/notes", None, operator[1]
    )
    status_thread, _ = call(
        "venue_note_read_thread_retention", "GET",
        f"/landmarks/{place_id}/notes/{note_id}", None, operator[1],
    )
    status_replies, _ = call(
        "venue_note_read_replies_retention", "GET",
        f"/landmarks/{place_id}/notes/{note_id}/replies", None, operator[1],
    )
    checks["venue_note_reads_503"] = (
        status_list == 503 and status_thread == 503 and status_replies == 503
    )
    reply_key = str(uuid4())
    reply_body = {"text": "Development reply", "expected_parent_revision": note_revision}
    status, reply = call(
        "venue_note_reply", "POST", f"/landmarks/{place_id}/notes/{note_id}/replies",
        reply_body, operator[1], reply_key,
    )
    reply_id = ((reply.get("resource") or {}).get("id") if isinstance(reply, dict) else None)
    checks["venue_note_reply_201"] = (
        status == 201 and reply.get("operation_id") == "createVenuePlaceReply"
        and isinstance(reply_id, str)
    )
    status, reply_replay = call(
        "venue_note_reply_replay", "POST", f"/landmarks/{place_id}/notes/{note_id}/replies",
        reply_body, operator[1], reply_key,
    )
    checks["venue_note_reply_replay_stable"] = (
        status == 201 and (reply_replay.get("resource") or {}).get("id") == reply_id
    )
    changed_reply = dict(reply_body)
    changed_reply["text"] = "Development reply changed"
    status, reply_conflict = call(
        "venue_note_reply_conflict", "POST", f"/landmarks/{place_id}/notes/{note_id}/replies",
        changed_reply, operator[1], reply_key,
    )
    checks["venue_note_reply_conflict_409"] = (
        status == 409
        and (reply_conflict.get("details") or {}).get("reason") == "VERSION_CONFLICT"
    )
    status, _ = call(
        "venue_note_stranger_reply", "POST", f"/landmarks/{place_id}/notes/{note_id}/replies",
        {"text": "Denied reply", "expected_parent_revision": 2}, stranger[1], str(uuid4()),
    )
    checks["venue_note_stranger_reply_403"] = status == 403
    status, _ = call(
        "venue_note_foreign_withdraw", "DELETE",
        f"/landmarks/{place_id}/notes/{note_id}?expected_revision=2", None, operator[1], str(uuid4()),
    )
    checks["venue_note_foreign_withdraw_403"] = status == 403
    withdraw_key = str(uuid4())
    status, withdrawn = call(
        "venue_note_withdraw", "DELETE",
        f"/landmarks/{place_id}/notes/{note_id}?expected_revision=2", None, member[1], withdraw_key,
    )
    withdraw_revision = ((withdrawn.get("resource") or {}).get("revision") if isinstance(withdrawn, dict) else None)
    checks["venue_note_withdraw_200"] = (
        status == 200 and withdrawn.get("operation_id") == "withdrawVenuePlaceNote"
    )
    status, withdraw_replay = call(
        "venue_note_withdraw_replay", "DELETE",
        f"/landmarks/{place_id}/notes/{note_id}?expected_revision=2", None, member[1], withdraw_key,
    )
    checks["venue_note_withdraw_replay_stable"] = (
        status == 200
        and (withdraw_replay.get("resource") or {}).get("revision") == withdraw_revision
    )
    status, withdraw_conflict = call(
        "venue_note_withdraw_conflict", "DELETE",
        f"/landmarks/{place_id}/notes/{note_id}?expected_revision=1", None, member[1], withdraw_key,
    )
    checks["venue_note_withdraw_conflict_409"] = (
        status == 409
        and (withdraw_conflict.get("details") or {}).get("reason") == "VERSION_CONFLICT"
    )

    context_path = f"{gateway}/api/concert/events/{event}/venue-meeting/context?selectedFriendId={{}}"
    status, context, _ = request(context_path.format(member[0]), "GET", None, operator[1], origin)
    record("venue_meeting_context_friend", {"status": status, "body": context})
    checks["venue_meeting_context_friend_200"] = (
        status == 200 and isinstance(context, dict) and context.get("canRead") is True
    )
    status, context, _ = request(context_path.format(member[0]), "GET", None, stranger[1], origin)
    record("venue_meeting_context_stranger", {"status": status, "body": context})
    checks["venue_meeting_context_stranger_404"] = status == 404
    status, sent, _ = request(
        f"{gateway}/api/concert/events/{event}/venue-meeting/messages", "POST",
        {"selection": {"selectedFriendId": member[0]}, "body": "Development venue message"},
        operator[1], origin, True, {"Idempotency-Key": str(uuid4())},
    )
    record("venue_meeting_send", {"status": status, "body": sent})
    checks["venue_meeting_send_200"] = (
        status == 200 and sent.get("state") == "APPLIED"
        and (sent.get("result") or {}).get("kind") == "MESSAGE"
    )
    status, messages, _ = request(
        f"{gateway}/api/concert/events/{event}/venue-meeting/messages?selectedFriendId={operator[0]}",
        "GET", None, member[1], origin,
    )
    record("venue_meeting_messages", {"status": status, "body": messages})
    checks["venue_meeting_messages_200"] = (
        status == 200 and isinstance(messages, dict) and any(
            item.get("body") == "Development venue message" for item in messages.get("items", [])
        )
    )
    meeting_base = f"{gateway}/api/concert/events/{event}/venue-meeting"
    status, invitations, _ = request(
        f"{meeting_base}/invitations?direction=OUTGOING", "GET", None, operator[1], origin
    )
    record("venue_meeting_invitations", {"status": status, "body": invitations})
    checks["venue_meeting_invitation_list_200"] = status == 200 and isinstance(invitations, dict)
    invitation_body = {
        "selection": {
            "selectedFriendId": member[0],
            "landmark": {"landmarkId": place_id, "catalogRevision": 1, "landmarkRevision": 1},
        }
    }
    status, created_invitation, _ = request(
        f"{meeting_base}/invitations", "POST", invitation_body, operator[1], origin, True,
        {"Idempotency-Key": str(uuid4())},
    )
    record("venue_meeting_invitation_create", {"status": status, "body": created_invitation})
    invitation = ((created_invitation.get("result") or {}).get("invitation") or {})
    invitation_id = invitation.get("id")
    checks["venue_meeting_invitation_create_200"] = (
        status == 200 and created_invitation.get("state") == "APPLIED"
        and isinstance(invitation_id, str)
    )
    if invitation_id:
        detail = (
            f"{meeting_base}/invitations/{invitation_id}?selectedFriendId={member[0]}"
            f"&landmarkId={place_id}&catalogRevision=1&landmarkRevision=1"
        )
        status, detail_body, _ = request(detail, "GET", None, operator[1], origin)
        record("venue_meeting_invitation_detail", {"status": status, "body": detail_body})
        checks["venue_meeting_invitation_get_200"] = status == 200 and detail_body.get("id") == invitation_id
        decision = {
            "selection": {
                "selectedFriendId": operator[0],
                "landmark": {"landmarkId": place_id, "catalogRevision": 1, "landmarkRevision": 1},
            },
            "expectedRevision": 1,
            "decision": "ACCEPT",
        }
        status, decided, _ = request(
            f"{meeting_base}/invitations/{invitation_id}/decision", "POST", decision,
            member[1], origin, True, {"Idempotency-Key": str(uuid4())},
        )
        record("venue_meeting_invitation_decide", {"status": status, "body": decided})
        checks["venue_meeting_invitation_decide_200"] = (
            status == 200 and decided.get("state") == "APPLIED"
        )
        status, stranger_detail, _ = request(detail, "GET", None, stranger[1], origin)
        record("venue_meeting_invitation_stranger_detail", {"status": status, "body": stranger_detail})
        checks["venue_meeting_stranger_denied"] = status == 404
        status, _, _ = request(
            f"{meeting_base}/invitations", "POST", invitation_body, stranger[1], origin, True,
            {"Idempotency-Key": str(uuid4())},
        )
        checks["venue_meeting_stranger_denied"] = checks["venue_meeting_stranger_denied"] and status == 404
        status, _, _ = request(
            f"{meeting_base}/invitations/{invitation_id}/decision", "POST", decision,
            stranger[1], origin, True, {"Idempotency-Key": str(uuid4())},
        )
        checks["venue_meeting_stranger_denied"] = checks["venue_meeting_stranger_denied"] and status == 404
        status, _, _ = request(
            f"{meeting_base}/messages", "POST",
            {"selection": {"selectedFriendId": member[0]}, "body": "Denied message"},
            stranger[1], origin, True, {"Idempotency-Key": str(uuid4())},
        )
        checks["venue_meeting_stranger_denied"] = checks["venue_meeting_stranger_denied"] and status == 404
        status, _, _ = request(
            f"{meeting_base}/commands/{uuid4()}?selectedFriendId={member[0]}",
            "GET", None, stranger[1], origin,
        )
        checks["venue_meeting_stranger_denied"] = checks["venue_meeting_stranger_denied"] and status == 404
        status, command, _ = request(
            f"{meeting_base}/commands/{uuid4()}?selectedFriendId={member[0]}",
            "GET", None, operator[1], origin,
        )
        record("venue_meeting_command", {"status": status, "body": command})
        checks["venue_meeting_command_200"] = (
            status == 200 and command.get("state") == "UNKNOWN"
        )
    else:
        checks["venue_meeting_invitation_get_200"] = False
        checks["venue_meeting_invitation_decide_200"] = False
        checks["venue_meeting_stranger_denied"] = False
        checks["venue_meeting_command_200"] = False
    report_rows = stack.cluster.sql(
        "mcr_concert",
        "SELECT (SELECT count(*) FROM concert_venue_location_sharing WHERE event_id="
        + prep.quote(event) + "),(SELECT count(*) FROM concert_venue_location_fixes WHERE event_id="
        + prep.quote(event) + "),(SELECT count(*) FROM concert_place_notes WHERE event_id="
        + prep.quote(event) + "),(SELECT count(*) FROM concert_place_notes WHERE event_id="
        + prep.quote(event) + " AND parent_id IS NOT NULL),(SELECT count(*) FROM concert_venue_meeting_messages WHERE event_id="
        + prep.quote(event) + "),(SELECT count(*) FROM concert_place_note_receipts WHERE event_id="
        + prep.quote(event) + ")",
    )
    record("venue_rows", {"sharing|fixes|notes|replies|messages|receipts": report_rows})


def hold(stack, seconds):
    print("HOLD_BEGIN", flush=True)
    for name in sorted(stack.children):
        for kind in ("grpc", "http", "metrics", "world"):
            try:
                print(f"HOLD_PORT {name}:{kind} {stack.ports.address(f'{name}:{kind}')}", flush=True)
            except Exception:  # noqa: BLE001
                pass
    print(f"HOLD_ROOT {stack.cluster.root}", flush=True)
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            time.sleep(0.25)
        except KeyboardInterrupt:
            break
    print("HOLD_END", flush=True)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--pg-bin-dir", type=Path, default=Path("/opt/homebrew/opt/postgresql@16/bin"))
    parser.add_argument("--redis-binary", type=Path, default=Path("/opt/homebrew/bin/redis-server"))
    parser.add_argument("--openssl-binary", type=Path, default=Path("/opt/homebrew/bin/openssl"))
    parser.add_argument("--hold-on-failure-seconds", type=int, default=0)
    parser.add_argument("--venue-only", action="store_true")
    args = parser.parse_args(argv)
    migration.SERVICES["concert-service"] = "CONCERT_DATABASE_URL"
    runtime.PREFIX.update({name: variable.removesuffix("_DATABASE_URL")
                           for name, variable in migration.SERVICES.items()})
    runtime.START_ORDER = ("profile-service", "notification-service", "friend-service", "post-service",
                           "concert-service", "auth-service", "api-gateway")
    runtime.PEERS["concert-service"] = ("auth-service", "friend-service", "profile-service")
    runtime.EXTRA_LISTENERS["concert-service"] = ("world",)
    # Concert exposes no HTTP listener: its operational listener is CONCERT_METRICS_ADDR
    # and its product transport is gRPC. The shared harness assumes an HTTP kind for
    # every service; owner-qualify only the listeners this binary actually opens.
    original_child_init = runtime.Child.__init__

    def child_init(self, cluster, name, argv, env, ports, reservations, secrets_to_redact=()):
        if name == "concert-service":
            reservations = tuple(key for key in reservations if key != "concert-service:http")
        original_child_init(self, cluster, name, argv, env, ports, reservations, secrets_to_redact)

    runtime.Child.__init__ = child_init
    report = {"banner": BANNER, "steps": {}, "checks": {}}
    args.report.write_text(json.dumps(report, indent=2))
    stack = dev_stack.DevStack(args.bin_dir, args.pg_bin_dir, args.redis_binary, args.openssl_binary, None)
    base_environments = dev_stack.development_environments
    origin = None
    auth_private_key = None

    def environments(cluster, ports, keys, credentials, redis_password, mail_token, browser_origin=None):
        nonlocal origin, auth_private_key
        envs = base_environments(cluster, ports, keys, credentials, redis_password, mail_token,
                                 browser_origin)
        auth_private_key = keys["AUTH_JWT_PRIVATE_KEY_PEM"]
        # The canonical owner guard keeps prepared transactions; Auth/Friend
        # verify that capacity before serving, exactly as acceptance does.
        cluster.assert_owned()
        cluster.stop()
        with (cluster.data / "postgresql.conf").open("a") as stream:
            stream.write("\nmax_prepared_transactions=256\n")
        cluster.start()
        require(cluster.sql("postgres", "SHOW max_prepared_transactions") == "256",
                "owned-prepared-config")
        world.pki(cluster)
        for service in envs:
            envs[service].update(world.tls(cluster, runtime.PREFIX[service].lower()))
        origin = envs["api-gateway"]["GATEWAY_CORS_ALLOWED_ORIGINS"].split(",")[0]
        envs["auth-service"]["AUTH_ENROLLMENT_VERIFICATION_URL"] = origin + "/verify-email"
        release = prep.release_inputs()
        template = json.loads(Path(release["CONCERT_BOOTSTRAP_FILE"]).read_bytes())
        event = str(template["event"]["event_id"])
        rooms = [{"room_id": str(uuid4()), "capacity": 4}]
        owner = str(uuid4())
        avatar = template["avatar_manifest"]
        mapped = template["map_manifest"]
        statements = ["INSERT INTO concert_events(event_id,slug,title,phase) VALUES("
                      + ",".join(map(prep.quote, [event, "closure-dev-" + secrets.token_hex(8),
                                                  "Closure development flow", "OPEN"])) + ")"]
        venue_place, venue_statements = venue_seed(event)
        statements.extend(venue_statements)
        report["venue_place"] = venue_place
        for room in rooms:
            statements.append(
                "INSERT INTO concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,"
                "avatar_manifest_id,avatar_manifest_version,is_default) VALUES("
                + ",".join(map(prep.quote, [event, room["room_id"], "Closure dev room", room["capacity"],
                                            mapped["map_id"], mapped["map_revision"],
                                            avatar["manifest_id"], avatar["manifest_version"]]))
                + ",true)")
        cluster.sql("mcr_concert", ";".join(statements))
        world_address = ports.address("concert-service:world")
        envs["concert-service"].update(
            {
                "CONCERT_RUNTIME_MODE": "local",
                "CONCERT_BOOTSTRAP_FILE": release["CONCERT_BOOTSTRAP_FILE"],
                "CONCERT_AVATAR_MANIFEST_FILE": release["CONCERT_AVATAR_MANIFEST_FILE"],
                "CONCERT_MAP_MANIFEST_FILE": release["CONCERT_MAP_MANIFEST_FILE"],
                "CONCERT_REVENGE_MANIFEST_FILE": release["CONCERT_REVENGE_MANIFEST_FILE"],
                "CONCERT_PARADE_MANIFEST_FILE": release["CONCERT_PARADE_MANIFEST_FILE"],
                "CONCERT_DANGER_MANIFEST_FILE": release["CONCERT_DANGER_MANIFEST_FILE"],
                "CONCERT_WORLD_LISTEN_ADDR": world_address,
                "CONCERT_WORLD_ALLOWED_ORIGINS": json.dumps([origin]),
                "CONCERT_WORLD_EVENT_ID": event,
                "CONCERT_WORLD_OWNER_ID": owner,
                # The bridge requires the replay consumer to be the same owner.
                "CONCERT_WORLD_INVALIDATION_CONSUMER_ID": owner,
                "CONCERT_WORLD_ROOMS_JSON": json.dumps(rooms),
                "CONCERT_WORLD_RESOURCE_ROOT": release["asset_root"],
                "CONCERT_WORLD_SOURCE_BINDING_FILE": release["source_binding"],
                "CONCERT_CLOSURE_MODE": "operator",
                "CONCERT_CLOSURE_RETENTION_HOURS": "720",
                "CONCERT_CLOSURE_POLICY_REVISION": "1",
                "CONCERT_CLOSURE_RECEIPT_KEY_HEX": secrets.token_hex(32),
                "CONCERT_CLOSURE_ARCHIVE_CURSOR_KEY_HEX": secrets.token_hex(32),
            }
        )

        relationship, self_profile, decision, post_read, post_callback, feed_command = world.joined.keys(cluster, 6)
        urls = {role: "https://" + ports.address(role + "-service:grpc")
                for role in ("auth", "friend", "profile", "concert", "notification", "post")}
        for env in envs.values():
            for key, value in list(env.items()):
                if key.endswith("_GRPC_ENDPOINT") and value.startswith("http://127.0.0.1:"):
                    env[key] = value.replace("http://", "https://", 1)
            env["CONCERT_RELATIONSHIP_VERIFYING_KEYS"] = relationship[1]
        for role in ("auth", "friend"):
            envs[role + "-service"].update(
                CONCERT_OWNER_AUTHORITY_ENABLED="true",
                AUTHORITY_CONCERT_SETTLEMENT_ENDPOINT=urls["concert"],
                AUTHORITY_PROFILE_SETTLEMENT_ENDPOINT=urls["profile"],
            )
        envs["profile-service"].update(
            CONCERT_PROFILE_SELF_VERIFYING_KEYS=self_profile[1],
            PROFILE_PARTICIPANT_DECISION_SIGNING_SEED=decision[0],
            PROFILE_SELF_AUTH_GRPC_ENDPOINT=urls["auth"],
            PROFILE_SELF_FRIEND_GRPC_ENDPOINT=urls["friend"],
            PROFILE_SELF_CONCERT_GRPC_ENDPOINT=urls["concert"],
        )
        envs["concert-service"].update(
            CONCERT_RELATIONSHIP_SIGNING_SEED=relationship[0],
            CONCERT_PROFILE_SELF_SIGNING_SEED=self_profile[0],
            PROFILE_PARTICIPANT_DECISION_VERIFYING_KEYS=decision[1],
            CONCERT_PARTICIPANT_MODE="live",
            CONCERT_PRODUCER_INSTANCE_ID=str(uuid4()),
            CONCERT_PARTICIPANT_RECOVERY_KEYS_JSON=json.dumps(
                [{"id": "closure_dev", "secret_hex": secrets.token_hex(32)}]
            ),
        )
        for role in ("auth", "friend", "profile"):
            envs["concert-service"]["CONCERT_PARTICIPANT_" + role.upper() + "_GRPC_ENDPOINT"] = urls[role]
            envs["concert-service"]["CONCERT_" + role.upper() + "_GRPC_ENDPOINT"] = urls[role]
        envs["concert-service"].update(
            CONCERT_INTRO_MODE="live",
            CONCERT_PUBLIC_FEED_MODE="read",
            CONCERT_NOTIFICATION_GRPC_ENDPOINT=urls["notification"],
            CONCERT_INTRO_CURSOR_KEY_HEX=secrets.token_hex(32),
            CONCERT_INTRO_RECEIPT_KEYS_JSON=json.dumps(
                [{"id": "closure_dev", "secret_hex": secrets.token_hex(32)}]
            ),
            CONCERT_PICKUP_KEYS_JSON=json.dumps(
                [{"id": "closure_dev", "secret_hex": secrets.token_hex(32)}]
            ),
            CONCERT_VENUE_MODE="live",
            CONCERT_VENUE_POLICY_JSON=json.dumps({
                "venue_key": "closure-dev-venue",
                "provider_id": "closure-dev-provider",
                "source_revision": "closure-dev-1",
                "location_revision": 1,
                "meeting_revision": 1,
                "fresh_ms": 30000,
                "last_known_ms": 120000,
                "fix_max_age_ms": 120000,
                "future_skew_ms": 5000,
                "sharing_lease_ms": 3600000,
                "request_ttl_ms": 60000,
                "check_in_ttl_ms": 60000,
                "coordinate_retention_ms": 300000,
                "command_retention_ms": 60000,
                "max_accuracy_m": 50.0,
                "max_viewers": 10,
                "invitation_ms": 86400000,
                "content_ms": 3600000,
                "purge_ms": 604800000,
            }),
            CONCERT_POST_GRPC_ENDPOINT=urls["post"],
            CONCERT_POST_READ_SIGNING_SEED=post_read[0],
            POST_CONCERT_ROOT_READ_VERIFYING_KEYS=post_callback[1],
            CONCERT_FEED_CURSOR_KEY_HEX=secrets.token_hex(32),
            CONCERT_FEED_COMMAND_MODE="on",
            CONCERT_POST_COMMAND_SIGNING_SEED=feed_command[0],
            CONCERT_FEED_COMMAND_KEYS_JSON=json.dumps(
                [{"id": "closure_feed", "secret_hex": secrets.token_hex(32)}]
            ),
        )
        envs["post-service"].update(
            POST_CONCERT_READ_MODE="read",
            POST_CONCERT_READ_AUTH_GRPC_ENDPOINT=urls["auth"],
            POST_FRIEND_GRPC_ENDPOINT=urls["friend"],
            POST_CONCERT_ROOT_GRPC_ENDPOINT=urls["concert"],
            CONCERT_POST_READ_VERIFYING_KEYS=post_read[1],
            POST_CONCERT_ROOT_READ_SIGNING_SEED=post_callback[0],
            POST_CONCERT_COMMAND_MODE="on",
            CONCERT_POST_COMMAND_VERIFYING_KEYS=feed_command[1],
        )
        inspected = migration.execute(
            [str(args.bin_dir / "concert-service"), "--inspect-product-release"],
            env=envs["concert-service"], cwd=cluster.root, phase="closure-release-inspection",
        )
        digest = inspected.stdout.strip().split("=", 1)[1]
        envs["concert-service"]["CONCERT_PARTICIPANT_RELEASE_DIGEST"] = digest
        envs["api-gateway"]["GATEWAY_CONCERT_GRPC_ENDPOINT"] = urls["concert"]
        envs["api-gateway"]["GATEWAY_CONCERT_RELEASE_DIGEST"] = digest
        report["event"] = event
        return envs

    try:
        with patch.object(dev_stack, "development_environments", environments), \
                patch.object(dev_stack, "MailSink", EnrollmentMailSink), \
                patch.object(runtime, "readiness_value", readiness):
            stack.start()
        require(auth_private_key is not None, "auth signing key captured")
        report["stack"] = stack.descriptor()
        report["stack"].pop("owner", None)
        gateway = stack.ports.url("api-gateway:http")
        policy = publish_enrollment_policy(stack.cluster)
        report["enrollment_policy_revision"] = policy["revision"]
        operator = principal(gateway, stack.mail, policy, 1, origin)
        member = principal(gateway, stack.mail, policy, 2, origin)
        stranger = principal(gateway, stack.mail, policy, 3, origin)
        initialize_profile(gateway, operator[0], operator[1], 1, origin)
        initialize_profile(gateway, member[0], member[1], 2, origin)
        initialize_profile(gateway, stranger[0], stranger[1], 3, origin)
        event = report["event"]
        stack.cluster.sql(
            "mcr_concert",
            "INSERT INTO concert_event_operator_roles(event_id,user_id,role,granted_by) VALUES("
            + prep.quote(event) + "," + prep.quote(operator[0]) + ",'OPERATOR'," + prep.quote(operator[0]) + ")",
        )
        report["steps"]["operator_role"] = stack.cluster.sql(
            "mcr_concert",
            "SELECT role,revision,revoked_at FROM concert_event_operator_roles WHERE event_id="
            + prep.quote(event) + " AND user_id=" + prep.quote(operator[0]),
        )
        # Each step records the bytes its owned children logged since the prior
        # step. Status, response body and durable row remain the primary proof.
        log_offsets = {}

        def record(name, value):
            value["log"] = new_log(stack, log_offsets)
            report["steps"][name] = value
            return value

        checks = report["checks"]
        for user in (operator[0], member[0], stranger[0]):
            stack.cluster.sql(
                "mcr_concert",
                "INSERT INTO concert_event_admissions(event_id,user_id,state) VALUES("
                + prep.quote(event) + "," + prep.quote(user) + ",'ACTIVE') ON CONFLICT DO NOTHING",
            )
            stack.cluster.sql(
                "mcr_concert",
                "INSERT INTO concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) VALUES("
                + prep.quote(event) + "," + prep.quote(user)
                + ",'REVENGE',false,false,false) ON CONFLICT DO NOTHING",
            )
            stack.cluster.sql(
                "mcr_concert",
                "INSERT INTO concert_avatar_settings(event_id,user_id,contract_version,representation,preset_id,manifest_id,manifest_version) VALUES("
                + prep.quote(event) + "," + prep.quote(user)
                + ",'1.1.0','whole_character','photo01_front_01','mcr_selected_static_20261103',1) ON CONFLICT DO NOTHING",
            )
        report["steps"]["feed_admissions"] = stack.cluster.sql(
            "mcr_concert",
            "SELECT user_id,state FROM concert_event_admissions WHERE event_id="
            + prep.quote(event) + " ORDER BY user_id",
        )
        for name in (
            "feed_create_200", "feed_replay_stored", "feed_conflict_409",
            "feed_edit_200", "feed_delete_200", "feed_second_create_200",
            "feed_comment_200", "feed_reaction_200",
            "feed_unauthorized_403_zero_rows", "feed_missing_404_zero_rows",
            "feed_blocked_403_or_404_zero_rows", "feed_post_closure_409",
        ):
            checks[name] = False
        venue_place = report["venue_place"]
        venue_friend_pair(gateway, origin, member, operator, record)
        for name in (
            "venue_sharing_200", "venue_fix_200", "venue_location_friend_200",
            "venue_location_no_consent_empty", "venue_location_blocked_no_location",
            "venue_location_unauth_401", "venue_note_create_201",
            "venue_note_create_replay_stable", "venue_note_create_conflict_409",
            "venue_note_reads_503", "venue_note_reply_201",
            "venue_note_reply_replay_stable", "venue_note_reply_conflict_409",
            "venue_note_stranger_reply_403", "venue_note_foreign_withdraw_403",
            "venue_note_withdraw_200", "venue_note_withdraw_replay_stable",
            "venue_note_withdraw_conflict_409", "venue_meeting_context_friend_200",
            "venue_meeting_context_stranger_404", "venue_meeting_send_200",
            "venue_meeting_messages_200", "venue_meeting_invitation_list_200",
            "venue_meeting_invitation_create_200", "venue_meeting_invitation_get_200",
            "venue_meeting_invitation_decide_200", "venue_meeting_command_200",
            "venue_meeting_stranger_denied",
        ):
            checks[name] = False
        venue_proof(
            stack, gateway, origin, event, venue_place, member, operator, stranger,
            record, checks,
        )
        if args.venue_only:
            venue_checks = {name: value for name, value in checks.items() if name.startswith("venue_")}
            failed = {name: value for name, value in venue_checks.items() if value is not True}
            report["result"] = "PASS" if not failed else "FAIL"
            report["checks"] = venue_checks
            report["venue_only"] = True
            if failed:
                report["failed_checks"] = failed
            return {"PASS": 0}.get(report["result"], 1)
        feed_proof(stack, gateway, origin, event, operator, member, record, checks)
        closure = {"confirmation": "CLOSE_EVENT", "expected_revision": 1,
                   "reason": "Development closure proof"}
        closure_url = f"{gateway}/api/concert/events/{event}/admin/closure"
        status, body, _ = request(closure_url, "POST", closure, operator[1], origin, True,
                                  {"Idempotency-Key": str(uuid4())})
        record("operator_closure", {"status": status, "body": body})
        status, body, _ = request(closure_url, "POST", closure, member[1], origin, True,
                                  {"Idempotency-Key": str(uuid4())})
        record("member_closure", {"status": status, "body": body})
        # The World authority task renews room leases on a 1 s supervised tick.
        # The closure retired those leases and closed the phase, so one tick
        # must pass with the service still up before the post-closure probes.
        # Time is the behavior here: wait past a full interval, bounded.
        settle_started = time.monotonic()
        while time.monotonic() - settle_started < 2.0:
            time.sleep(0.05)
        concert_child = stack.children.get("concert-service")
        concert_alive = concert_child is not None and concert_child.process.poll() is None
        settle_offset = log_offsets.get("concert-service", 0)
        record("post_closure_settle", {
            "waited_seconds": round(time.monotonic() - settle_started, 3),
            "concert_alive": concert_alive,
        })
        # Scan the whole post-closure byte range, not the truncated step excerpt.
        settle_bytes = ""
        if concert_child is not None and not concert_child.log.is_symlink():
            with concert_child.log.open("rb") as stream:
                stream.seek(settle_offset)
                settle_bytes = stream.read(65536).decode("utf-8", errors="replace")
        checks["world_survives_closure"] = (
            concert_alive
            and "World authority maintenance failed" not in settle_bytes
            and "CONCERT_COMPONENT_FAILURE" not in settle_bytes
        )
        # The Gateway mount and the Concert product classifier now both resolve
        # this route. READ_ONLY must refuse it with 409 and leave no feed rows.
        before = feed_rows(stack.cluster, event)
        status, body, _ = request(
            f"{gateway}/api/concert/events/{event}/posts", "POST",
            {"text": "post-closure feed write must be refused", "media_ids": [], "links": []},
            member[1], origin, True, {"Idempotency-Key": str(uuid4())},
        )
        after = feed_rows(stack.cluster, event)
        record("feed_post_write", {"status": status, "body": body,
                                   "rows_before": before, "rows_after": after})
        feed_post = report["steps"]["feed_post_write"]
        checks["feed_post_closure_409"] = (
            feed_post["status"] == 409
            and (feed_post["body"].get("details") or {}).get("reason") == "EVENT_READ_ONLY"
            and feed_post["rows_before"] == feed_post["rows_after"])
        # A supported Product write after READ_ONLY must be refused with 409, not
        # accepted or silently dropped. The same closure command under a fresh
        # idempotency key is the exact transition probe for that contract.
        status, body, _ = request(closure_url, "POST", closure, operator[1], origin, True,
                                  {"Idempotency-Key": str(uuid4())})
        record("post_closure_write", {"status": status, "body": body})
        archive_url = f"{gateway}/api/concert/events/{event}/archive"
        status, body, _ = request(archive_url, "GET", None, member[1], origin)
        record("archive", {"status": status, "body": body, "shape": archive_shape(body)})
        status, body, _ = request(archive_url, "GET", None, None, origin)
        record("archive_anonymous", {"status": status, "body": body,
                                    "shape": archive_shape(body)})
        status, body, _ = request(closure_url, "POST", closure, "forged.development.token", origin,
                                  True, {"Idempotency-Key": str(uuid4())})
        record("forged_closure", {"status": status, "body": body})
        expired = expired_access_token(stack.cluster, auth_private_key, member[0])
        status, body, _ = request(closure_url, "POST", closure, expired, origin, True,
                                  {"Idempotency-Key": str(uuid4())})
        record("expired_closure", {"status": status, "body": body})
        report["steps"]["closure_rows"] = {
            "event": stack.cluster.sql(
                "mcr_concert", "SELECT phase,revision FROM concert_events WHERE event_id="
                + prep.quote(event)
            ),
            "job": stack.cluster.sql(
                "mcr_concert", "SELECT job_id,actor_id,retention_revision,progress->>'state' "
                "FROM concert_closure_jobs WHERE event_id=" + prep.quote(event)
            ),
            "receipt": stack.cluster.sql(
                "mcr_concert", "SELECT state,result_status FROM concert_command_receipts WHERE event_id="
                + prep.quote(event) + " AND operation_id='closeConcertEvent'",
            ),
            "effects": stack.cluster.sql(
                "mcr_concert", "SELECT step,owner_revision FROM concert_closure_effects WHERE event_id="
                + prep.quote(event) + " ORDER BY step",
            ),
            "archive_rows": stack.cluster.sql(
                "mcr_concert", "SELECT count(*) FROM concert_closure_archives WHERE event_id="
                + prep.quote(event),
            ),
            "public_post_rows": stack.cluster.sql(
                "mcr_post", "SELECT count(*) FROM post_concert_posts WHERE event_id="
                + prep.quote(event),
            ),
        }
        # Post-probe liveness: the authority tick can land after every HTTP probe.
        # A crash there must fail the run, not disappear into teardown.
        concert_child = stack.children.get("concert-service")
        concert_alive_after = concert_child is not None and concert_child.process.poll() is None
        whole_run = ""
        if concert_child is not None and not concert_child.log.is_symlink():
            whole_run = concert_child.log.read_bytes().decode("utf-8", errors="replace")
        record("post_probe_liveness", {"concert_alive": concert_alive_after})
        checks = report["checks"]
        checks["concert_alive_after_probes"] = concert_alive_after
        # Whole-run stderr scan, not just the post-closure interval.
        checks["no_component_failure"] = (
            "CONCERT_COMPONENT_FAILURE" not in whole_run
            and "World authority maintenance failed" not in whole_run)
        phase = report["steps"]["closure_rows"]["event"].split("|")[0]
        checks["operator_closure_ack"] = (
            report["steps"]["operator_closure"]["status"] == 200
            and isinstance(report["steps"]["operator_closure"]["body"], dict))
        checks["closure_job_persisted"] = bool(
            report["steps"]["closure_rows"]["job"] and report["steps"]["closure_rows"]["receipt"])
        checks["event_read_only"] = phase == "READ_ONLY"
        checks["member_closure_forbidden"] = report["steps"]["member_closure"]["status"] == 403
        post_closure = report["steps"]["post_closure_write"]
        checks["post_closure_conflict"] = (
            post_closure["status"] == 409
            and (post_closure["body"].get("details") or {}).get("reason")
            == "INVALID_TRANSITION")
        feed_post = report["steps"]["feed_post_write"]
        checks["feed_post_closure_409"] = (
            feed_post["status"] == 409
            and (feed_post["body"].get("details") or {}).get("reason") == "EVENT_READ_ONLY"
            and feed_post["rows_before"] == feed_post["rows_after"])
        checks["forged_token_401"] = report["steps"]["forged_closure"]["status"] == 401
        checks["expired_token_401"] = report["steps"]["expired_closure"]["status"] == 401
        archive = report["steps"]["archive"]
        archive_anonymous = report["steps"]["archive_anonymous"]

        def archive_status_matches(status, body):
            reason = (body.get("details") or {}).get("reason")
            if phase == "ARCHIVED":
                return status == 200
            if phase == "READ_ONLY":
                return status == 404 and reason == "RESOURCE_UNAVAILABLE"
            return False

        # Every phase constrains the observed archive status. The `None` projection
        # checks below only cover the ARCHIVED-only 200 body, never the status.
        checks["archive_status_matches_phase"] = (
            archive_status_matches(archive["status"], archive["body"])
            and archive_status_matches(archive_anonymous["status"], archive_anonymous["body"]))
        if phase == "ARCHIVED":
            checks["archive_public_200"] = archive["status"] == 200
            shape = archive["shape"]
            checks["archive_public_fields"] = (
                shape.get("shape") == "projection"
                and set(shape.get("fields") or []) <= PUBLIC_ARCHIVE_POST_FIELDS
                and shape.get("media_ids_empty") is True)
        else:
            # The public archive serves only a completed ARCHIVED event. Until the
            # later closure steps have real owners, the truthful answer is 404.
            checks["archive_public_200"] = None
            checks["archive_public_fields"] = None
            report["limitations"] = [
                "Archive PUBLIC 200 NOT_REACHED: event phase is " + phase
                + "; PgPublicArchive::page refuses non-ARCHIVED events with 404 and the later"
                " closure steps (FREEZE_MURAL..CLEANUP) have no composed LifecycleOwners"
                " (ClosureProducer uses UnavailableLater). The observed archive status is"
                " asserted by archive_status_matches_phase.",
            ]
        failed = {name: value for name, value in checks.items() if value is False}
        report["result"] = "PASS" if not failed and all(v is not None for v in checks.values()) else (
            "PARTIAL" if not failed else "FAIL")
        if failed:
            report["failed_checks"] = failed
        if stack.cluster is not None:
            report["logs"] = log_excerpts(stack)
        if report["result"] != "PASS" and args.hold_on_failure_seconds > 0:
            hold(stack, args.hold_on_failure_seconds)
    except Exception as failure:  # noqa: BLE001 - owned development driver
        report["result"] = "FAIL"
        detail = failure.detail if isinstance(failure, migration.SmokeError) else ""
        report["failure"] = f"{type(failure).__name__}: {failure} {detail}"
        if stack.cluster is not None:
            report["logs"] = log_excerpts(stack)
        if args.hold_on_failure_seconds > 0 and stack.cluster is not None:
            hold(stack, args.hold_on_failure_seconds)
    finally:
        try:
            stack.close()
            report["cleanup"] = "PASS"
        except Exception as failure:  # noqa: BLE001
            report["cleanup"] = f"FAIL: {failure}"
        args.report.write_text(json.dumps(report, indent=2))
    return {"PASS": 0, "PARTIAL": 2}.get(report.get("result"), 1)


if __name__ == "__main__":
    sys.exit(main())
