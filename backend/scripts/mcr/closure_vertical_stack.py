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
import json
import secrets
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor
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
        raw = failure.read()
        try:
            return failure.code, json.loads(raw or b"{}"), dict(failure.headers)
        except ValueError:
            return failure.code, {"raw": raw[:200].decode("utf-8", "replace")}, dict(failure.headers)


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


IN02_PLACE = "11111111-1111-4111-8111-111111111111"


def in02_provision(cluster, event, users):
    """Development-only event policy and a synthetic reviewed physical place."""
    now = datetime.now(timezone.utc)
    admissions = []
    for user in users:
        admissions += [
            "INSERT INTO concert_event_admissions(event_id,user_id,state) VALUES("
            + ",".join(map(prep.quote, [event, user, "ACTIVE"])) + ") ON CONFLICT DO NOTHING",
            "INSERT INTO concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) VALUES("
            + ",".join(map(prep.quote, [event, user, "REVENGE"])) + ",false,false,false) ON CONFLICT DO NOTHING",
            "INSERT INTO concert_avatar_settings(event_id,user_id,contract_version,representation,preset_id,manifest_id,manifest_version) VALUES("
            + ",".join(map(prep.quote, [event, user, "1.1.0", "whole_character",
                                        "photo01_front_01", "mcr_selected_static_20261103"]))
            + ",1) ON CONFLICT DO NOTHING",
        ]
    cluster.sql(
        "mcr_concert",
        "SELECT set_config('mcr.operations_authority_until',(clock_timestamp()+interval '1 hour')::text,false);"
        + ";".join(admissions) + ";"
        "INSERT INTO concert_event_settings(event_id,parties,giveaways,intros,social,mural,"
        "beacon_ttl_seconds,pickup_grace_seconds,social_grant_ttl_seconds,mural_cooldown_ms) VALUES("
        + prep.quote(event) + ",true,true,false,false,false,900,0,3600,1200) ON CONFLICT(event_id) DO NOTHING;"
        "INSERT INTO concert_meeting_settings(event_id,beacon_ttl_seconds,event_cutoff) VALUES("
        + prep.quote(event) + ",900,clock_timestamp()+interval '2 days') ON CONFLICT(event_id) DO NOTHING;",
    )
    provenance = {
        "source_id": "mcr_in02_dev",
        "source_kind": "PLACE_REFERENCE",
        "source_revision": "in02-development-v1",
        "source_uri": "https://example.test/in02-development-place",
        "author": "MCR development gate",
        "license": "Synthetic development metadata only; not a venue claim",
        "review_state": "REVIEWED",
        "reviewed_by": "development-operator",
        "reviewed_at": now.isoformat(),
        "photo_permission": "NOT_APPLICABLE",
    }
    canonical = json.dumps({"map_id": "map_in02_dev", "map_revision": 1},
                           separators=(",", ":")).encode()
    import hashlib
    digest = "sha256:" + hashlib.sha256(canonical).hexdigest()
    cluster.sql(
        "mcr_concert",
        "INSERT INTO concert_map_releases(map_id,map_revision,content_hash,geometry_hash,canonical_bytes) VALUES("
        "'map_in02_dev',1," + prep.quote(digest) + "," + prep.quote(digest)
        + ",decode('" + canonical.hex() + "','hex')) ON CONFLICT DO NOTHING;"
        "INSERT INTO concert_map_release_places(map_id,map_revision,place_id,label,place_kind,"
        "verification_state,accessibility_notes,source_kind,source_uri,review_state,provenance) VALUES("
        "'map_in02_dev',1," + prep.quote(IN02_PLACE)
        + ",'Development meeting point','PHYSICAL','VERIFIED','Step-free development fixture',"
        "'PLACE_REFERENCE'," + prep.quote(provenance["source_uri"]) + ",'REVIEWED',"
        + prep.quote(json.dumps(provenance, separators=(",", ":"))) + ") ON CONFLICT DO NOTHING;"
        "INSERT INTO concert_events_map_releases(event_id,catalog_revision,map_id,map_revision) VALUES("
        + prep.quote(event) + ",1,'map_in02_dev',1) ON CONFLICT(event_id) DO NOTHING;"
        "INSERT INTO concert_place_current_state(map_id,map_revision,place_id,closure_state) VALUES("
        "'map_in02_dev',1," + prep.quote(IN02_PLACE) + ",'OPEN') ON CONFLICT DO NOTHING;",
    )
    return IN02_PLACE


def in02_gate(stack, gateway, origin, event, host, alice, bob, report, record):
    """Drive the real meetup capacity and gift stock/pickup invariants over HTTP."""
    base = f"{gateway}/api/concert/events/{event}"
    checks = {}

    def call(method, path, payload, token, key=None):
        extra = None if method == "GET" else {"Idempotency-Key": key or str(uuid4())}
        status, body, _ = request(base + path, method, payload, token, origin, True, extra)
        return status, body

    def concurrent(first, second):
        barrier = threading.Barrier(2)

        def run(fn):
            barrier.wait()
            return fn()
        with ThreadPoolExecutor(max_workers=2) as pool:
            futures = [pool.submit(run, first), pool.submit(run, second)]
            return [future.result() for future in futures]

    now = datetime.now(timezone.utc)
    def stamp(value):
        return value.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%fZ")
    starts = stamp(now - timedelta(minutes=1))
    ends = stamp(now + timedelta(hours=2))
    meeting_starts = stamp(now + timedelta(minutes=5))
    meeting_ends = stamp(now + timedelta(hours=4))

    warm = []
    for _ in range(2):
        status, body = call("GET", "/parties", None, host[1])
        warm.append({"status": status, "body": body})
    record("party_warmup", {"runs": warm})

    party_payload = {
        "title": "IN02 development meetup",
        "description": "Capacity and pickup invariant gate",
        "ticket_section": "A",
        "capacity": 2,
        "meeting_starts_at": meeting_starts,
        "meeting_ends_at": meeting_ends,
        "requirements": "Development fixture only",
        "public_place_id": IN02_PLACE,
    }
    party_key = str(uuid4())
    record("in02_provision_rows", {
        "event": stack.cluster.sql("mcr_concert", "SELECT phase FROM concert_events WHERE event_id=" + prep.quote(event)),
        "participants": stack.cluster.sql("mcr_concert", "SELECT count(*) FROM concert_participants WHERE event_id=" + prep.quote(event)),
        "admissions": stack.cluster.sql("mcr_concert", "SELECT count(*) FROM concert_event_admissions WHERE event_id=" + prep.quote(event) + " AND state='ACTIVE'"),
        "settings": stack.cluster.sql("mcr_concert", "SELECT parties,giveaways FROM concert_event_settings WHERE event_id=" + prep.quote(event)),
        "places": stack.cluster.sql("mcr_concert", "SELECT count(*) FROM concert_map_release_places WHERE map_id='map_in02_dev' AND map_revision=1"),
        "parties": stack.cluster.sql("mcr_concert", "SELECT count(*) FROM concert_community_parties WHERE event_id=" + prep.quote(event)),
        "giveaways": stack.cluster.sql("mcr_concert", "SELECT count(*) FROM concert_community_giveaways WHERE event_id=" + prep.quote(event)),
    })
    for attempt in range(4):
        status, party = call("POST", "/parties", party_payload, host[1], party_key)
        if status != 504:
            break
        time.sleep(1)
    record("party_create", {"status": status, "body": party})
    report["steps"]["party_create"]["receipts"] = stack.cluster.sql(
        "mcr_concert",
        "SELECT operation_id,state,failure_reason,result_status FROM concert_command_receipts WHERE event_id="
        + prep.quote(event) + " ORDER BY created_at",
    )
    report["steps"]["party_create"]["event_rows"] = stack.cluster.sql(
        "mcr_concert",
        "SELECT phase FROM concert_events WHERE event_id=" + prep.quote(event),
    )
    checks["party_create_200"] = status == 200 and isinstance(party.get("party_id"), str)
    if not checks["party_create_200"]:
        return checks, "party creation did not reach the Party producer"
    party_id, party_revision = party["party_id"], party["revision"]

    status, alice_apply = call("POST", f"/parties/{party_id}/applications",
                               {"party_revision": party_revision, "note": "alice"}, alice[1])
    record("party_apply_alice", {"status": status, "body": alice_apply})
    status2, bob_apply = call("POST", f"/parties/{party_id}/applications",
                              {"party_revision": party_revision, "note": "bob"}, bob[1])
    record("party_apply_bob", {"status": status2, "body": bob_apply})
    checks["party_applies_200"] = status == 200 and status2 == 200
    if not checks["party_applies_200"]:
        return checks, "party applications were refused"

    def approve(application):
        return call("POST", f"/party-applications/{application}/decision",
                    {"expected_revision": 1, "party_revision": party_revision,
                     "decision": "APPROVED"}, host[1])

    outcomes = concurrent(lambda: approve(alice_apply["application_id"]),
                          lambda: approve(bob_apply["application_id"]))
    record("party_decisions_concurrent", {
        "alice": {"status": outcomes[0][0], "body": outcomes[0][1]},
        "bob": {"status": outcomes[1][0], "body": outcomes[1][1]},
    })
    winners = [index for index, outcome in enumerate(outcomes) if outcome[0] == 200]
    checks["party_one_concurrent_approval"] = len(winners) == 1
    if not checks["party_one_concurrent_approval"]:
        return checks, "capacity did not admit exactly one concurrent applicant"
    loser = 1 - winners[0]
    loser_app = (alice_apply if loser == 0 else bob_apply)["application_id"]
    status, rejected = call("POST", f"/party-applications/{loser_app}/decision",
                            {"expected_revision": 1, "party_revision": 2,
                             "decision": "APPROVED"}, host[1])
    record("party_second_approval", {"status": status, "body": rejected})
    checks["party_capacity_rejected"] = status == 409 and \
        (rejected.get("details") or {}).get("reason") == "PARTY_FULL"

    status, party_after = call("GET", f"/parties/{party_id}", None, host[1])
    record("party_read", {"status": status, "body": party_after})
    checks["party_member_count_two"] = status == 200 and party_after.get("member_count") == 2
    report["steps"]["party_rows"] = {
        "party": stack.cluster.sql("mcr_concert",
            "SELECT state,revision,(aggregate->'fields'->>'capacity') FROM concert_community_parties WHERE event_id="
            + prep.quote(event) + " AND party_id=" + prep.quote(party_id)),
        "members": stack.cluster.sql("mcr_concert",
            "SELECT user_id,is_owner,state FROM concert_community_members WHERE event_id="
            + prep.quote(event) + " AND party_id=" + prep.quote(party_id) + " ORDER BY is_owner DESC,user_id"),
        "applications": stack.cluster.sql("mcr_concert",
            "SELECT application_id,state,revision FROM concert_community_applications WHERE event_id="
            + prep.quote(event) + " AND party_id=" + prep.quote(party_id) + " ORDER BY application_id"),
    }

    status, giveaway = call("POST", "/giveaways", {
        "title": "IN02 development giveaway",
        "description": "One unit stock gate",
        "image_media_id": None,
        "quantity_total": 1,
        "pickup_starts_at": starts,
        "pickup_ends_at": ends,
        "public_place_id": IN02_PLACE,
    }, host[1])
    record("giveaway_create", {"status": status, "body": giveaway})
    checks["giveaway_create_200"] = status == 200 and isinstance(giveaway.get("giveaway_id"), str)
    if not checks["giveaway_create_200"]:
        return checks, "giveaway creation did not reach the Gift producer"
    giveaway_id, gift_revision = giveaway["giveaway_id"], giveaway["revision"]

    status, meeting = call("PUT", f"/giveaways/{giveaway_id}/pickup-meeting", {
        "expected_revision": gift_revision,
        "physical_presence_acknowledged": True,
        "meeting": {
            "place_id": IN02_PLACE,
            "map_revision": 1,
            "place_revision": 1,
            "instructions": "Development collection point",
            "identification": "Show the six digit code",
            "expires_at": stamp(now + timedelta(hours=3)),
        },
    }, host[1])
    record("giveaway_meeting", {"status": status, "body": meeting})
    checks["giveaway_meeting_200"] = status == 200
    if not checks["giveaway_meeting_200"]:
        return checks, "pickup meeting was refused"
    gift_revision = meeting.get("revision", gift_revision + 1)

    status, opened = call("PATCH", f"/giveaways/{giveaway_id}", {
        "expected_revision": gift_revision,
        "title": "IN02 development giveaway",
        "description": "One unit stock gate",
        "image_media_id": None,
        "quantity_total": 1,
        "pickup_starts_at": starts,
        "pickup_ends_at": ends,
        "public_place_id": IN02_PLACE,
        "state": "OPEN",
    }, host[1])
    record("giveaway_open", {"status": status, "body": opened})
    checks["giveaway_open_200"] = status == 200
    if not checks["giveaway_open_200"]:
        return checks, "giveaway could not be opened"
    gift_revision = opened["revision"]

    def reserve(token):
        return call("POST", f"/giveaways/{giveaway_id}/reservations",
                    {"giveaway_revision": gift_revision}, token)

    outcomes = concurrent(lambda: reserve(alice[1]), lambda: reserve(bob[1]))
    record("reservations_concurrent", {
        "alice": {"status": outcomes[0][0], "body": outcomes[0][1]},
        "bob": {"status": outcomes[1][0], "body": outcomes[1][1]},
    })
    winners = [index for index, outcome in enumerate(outcomes) if outcome[0] == 200]
    checks["stock_one_concurrent_reservation"] = len(winners) == 1
    if not checks["stock_one_concurrent_reservation"]:
        return checks, "stock did not admit exactly one concurrent reservation"
    winner = winners[0]
    reservation_id = outcomes[winner][1]["reservation_id"]
    latest = int(stack.cluster.sql(
        "mcr_concert", "SELECT revision FROM concert_community_giveaways WHERE event_id="
        + prep.quote(event) + " AND giveaway_id=" + prep.quote(giveaway_id)))
    status, sold_out = call("POST", f"/giveaways/{giveaway_id}/reservations",
                            {"giveaway_revision": latest},
                            (alice if winner == 1 else bob)[1])
    record("stock_second_attempt", {"status": status, "body": sold_out, "revision": latest})
    checks["stock_sold_out"] = status == 409 and \
        (sold_out.get("details") or {}).get("reason") == "GIVEAWAY_SOLD_OUT"
    report["steps"]["stock_rows"] = {
        "giveaway": stack.cluster.sql("mcr_concert",
            "SELECT state,revision,quantity_total,reserved_unclaimed,quantity_claimed FROM concert_community_giveaways WHERE event_id="
            + prep.quote(event) + " AND giveaway_id=" + prep.quote(giveaway_id)),
        "reservations": stack.cluster.sql("mcr_concert",
            "SELECT reservation_id,recipient_id,state,revision FROM concert_community_reservations WHERE event_id="
            + prep.quote(event) + " AND giveaway_id=" + prep.quote(giveaway_id) + " ORDER BY reservation_id"),
    }

    winner_token = (alice if winner == 0 else bob)[1]
    status, issued = call("POST", f"/reservations/{reservation_id}/pickup-code",
                           {"expected_revision": 1}, winner_token)
    record("pickup_code_issue", {"status": status, "body": issued})
    checks["pickup_code_issued"] = status == 200 and isinstance(issued.get("confirmation_code"), str)
    if not checks["pickup_code_issued"]:
        return checks, "pickup code was not issued"
    code = issued["confirmation_code"]
    reservation_revision = issued["reservation_revision"]
    report["steps"]["pickup_secret_issued"] = {
        "secret": stack.cluster.sql("mcr_concert",
            "SELECT generation,expires_at,length(keyed_hash) FROM concert_community_pickup_secrets WHERE event_id="
            + prep.quote(event) + " AND reservation_id=" + prep.quote(reservation_id)),
        "reservation": stack.cluster.sql("mcr_concert",
            "SELECT state,revision FROM concert_community_reservations WHERE event_id="
            + prep.quote(event) + " AND reservation_id=" + prep.quote(reservation_id)),
    }

    wrong = f"{(int(code) + 1) % 1000000:06d}"
    status, wrong_body = call("POST", f"/reservations/{reservation_id}/claim",
                              {"expected_revision": reservation_revision,
                               "confirmation_code": wrong}, host[1])
    record("pickup_wrong_code", {"status": status, "body": wrong_body})
    checks["wrong_code_403"] = status == 403 and \
        (wrong_body.get("details") or {}).get("reason") == "PICKUP_CODE_INVALID"

    claim_key = str(uuid4())
    claim_payload = {"expected_revision": reservation_revision, "confirmation_code": code}
    status, claimed = call("POST", f"/reservations/{reservation_id}/claim", claim_payload,
                           host[1], claim_key)
    record("pickup_claim", {"status": status, "body": claimed})
    checks["pickup_claim_200"] = status == 200 and claimed.get("state") == "CLAIMED"
    if not checks["pickup_claim_200"]:
        return checks, "valid pickup code was refused"
    claim_receipt = claimed["receipt_id"]

    status, same_key = call("POST", f"/reservations/{reservation_id}/claim", claim_payload,
                            host[1], claim_key)
    record("pickup_claim_same_key_replay", {"status": status, "body": same_key})
    checks["same_key_replay_same_receipt"] = status == 200 and same_key.get("receipt_id") == claim_receipt

    status, replay = call("POST", f"/reservations/{reservation_id}/claim", claim_payload, host[1])
    record("pickup_claim_new_key_replay", {"status": status, "body": replay})
    checks["same_code_new_key_replay_fails"] = status == 403 and \
        (replay.get("details") or {}).get("reason") == "PICKUP_CODE_INVALID"

    report["steps"]["pickup_rows"] = {
        "giveaway": stack.cluster.sql("mcr_concert",
            "SELECT state,revision,quantity_total,reserved_unclaimed,quantity_claimed FROM concert_community_giveaways WHERE event_id="
            + prep.quote(event) + " AND giveaway_id=" + prep.quote(giveaway_id)),
        "reservation": stack.cluster.sql("mcr_concert",
            "SELECT state,revision,expires_at FROM concert_community_reservations WHERE event_id="
            + prep.quote(event) + " AND reservation_id=" + prep.quote(reservation_id)),
        "pickup_secret": stack.cluster.sql("mcr_concert",
            "SELECT generation,expires_at,length(keyed_hash) FROM concert_community_pickup_secrets WHERE event_id="
            + prep.quote(event) + " AND reservation_id=" + prep.quote(reservation_id)),
        "receipts": stack.cluster.sql("mcr_concert",
            "SELECT operation_id,state,failure_reason,resource_revision FROM concert_command_receipts WHERE event_id="
            + prep.quote(event) + " ORDER BY created_at"),
        "outbox": stack.cluster.sql("mcr_concert",
            "SELECT count(*) FROM concert_outbox WHERE event_id=" + prep.quote(event)),
    }
    return checks, None


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


def log_excerpts(stack, names=("concert-service", "auth-service", "friend-service", "api-gateway")):
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


def new_log(stack, offsets, names=("api-gateway", "concert-service"), limit=1200):
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
    parser.add_argument("--in02", action="store_true")
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

        relationship, self_profile, decision, _post_read, _post_callback = world.joined.keys(cluster, 5)
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
            CONCERT_NOTIFICATION_GRPC_ENDPOINT=urls["notification"],
            CONCERT_INTRO_CURSOR_KEY_HEX=secrets.token_hex(32),
            CONCERT_INTRO_RECEIPT_KEYS_JSON=json.dumps(
                [{"id": "closure_dev", "secret_hex": secrets.token_hex(32)}]
            ),
            CONCERT_PICKUP_KEYS_JSON=json.dumps(
                [{"id": "closure_dev", "secret_hex": secrets.token_hex(32)}]
            ),
            CONCERT_PARTY_MODE="live",
            CONCERT_PARTY_CURSOR_KEY_HEX=secrets.token_hex(32),
            CONCERT_PARTY_RECEIPT_KEYS_JSON=json.dumps(
                [{"id": "closure_dev", "secret_hex": secrets.token_hex(32)}]
            ),
            CONCERT_GIFT_MODE="live",
            CONCERT_GIFT_CURSOR_KEY_HEX=secrets.token_hex(32),
            CONCERT_GIFT_RECEIPT_KEYS_JSON=json.dumps(
                [{"id": "closure_dev", "secret_hex": secrets.token_hex(32)}]
            ),
        )
        envs["api-gateway"]["MCR_GIFT_NETWORK_BUCKET_KEY"] = secrets.token_hex(32)
        inspected = migration.execute(
            [str(args.bin_dir / "concert-service"), "--inspect-product-release"],
            env=envs["concert-service"], cwd=cluster.root, phase="closure-release-inspection",
        )
        digest = inspected.stdout.strip().split("=", 1)[1]
        envs["concert-service"]["CONCERT_PARTICIPANT_RELEASE_DIGEST"] = digest
        envs["api-gateway"]["GATEWAY_CONCERT_GRPC_ENDPOINT"] = urls["concert"]
        envs["api-gateway"]["GATEWAY_REQUEST_TIMEOUT_MS"] = "5000"
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
        def enroll(ordinal):
            last = None
            for attempt in range(4):
                try:
                    return principal(gateway, stack.mail, policy, ordinal, origin)
                except StepFailure as failure:
                    last = failure
                    if attempt == 3:
                        raise
                    time.sleep(1)
            raise last

        operator = enroll(1)
        member = enroll(2)
        initialize_profile(gateway, operator[0], operator[1], 1, origin)
        initialize_profile(gateway, member[0], member[1], 2, origin)
        third = enroll(3) if args.in02 else None
        if third is not None:
            initialize_profile(gateway, third[0], third[1], 3, origin)
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
        if args.in02:
            in02_provision(stack.cluster, event, (operator[0], member[0], third[0]))
            checks, blocker = in02_gate(stack, gateway, origin, event, operator, member,
                                        third, report, record)
            report["checks"] = checks
            if blocker:
                report["in02_blocker"] = blocker
                report["result"] = "FAIL"
            else:
                failed = {name: value for name, value in checks.items() if value is not True}
                report["result"] = "PASS" if not failed else "FAIL"
                if failed:
                    report["failed_checks"] = failed
            if stack.cluster is not None:
                report["logs"] = log_excerpts(stack)
            return {"PASS": 0}.get(report.get("result"), 1)
        closure = {"confirmation": "CLOSE_EVENT", "expected_revision": 1,
                   "reason": "Development closure proof"}
        closure_url = f"{gateway}/api/concert/events/{event}/admin/closure"
        status, body, _ = request(closure_url, "POST", closure, operator[1], origin, True,
                                  {"Idempotency-Key": str(uuid4())})
        record("operator_closure", {"status": status, "body": body})
        status, body, _ = request(closure_url, "POST", closure, member[1], origin, True,
                                  {"Idempotency-Key": str(uuid4())})
        record("member_closure", {"status": status, "body": body})
        # The feed-post route is mounted by the Gateway but no Product operation
        # classifies it (`product_rpc::operation` refuses it as "Unsupported
        # product operation"), so it is recorded as an observation and never
        # used as the 409 proof.
        status, body, _ = request(
            f"{gateway}/api/concert/events/{event}/posts", "POST",
            {"text": "post-closure feed write must be refused", "mediaIds": []},
            operator[1], origin, True, {"Idempotency-Key": str(uuid4())},
        )
        record("feed_post_write", {"status": status, "body": body})
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
        phase = report["steps"]["closure_rows"]["event"].split("|")[0]
        checks = report["checks"]
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
        checks["forged_token_401"] = report["steps"]["forged_closure"]["status"] == 401
        checks["expired_token_401"] = report["steps"]["expired_closure"]["status"] == 401
        archive_status = report["steps"]["archive"]["status"]
        if phase == "ARCHIVED":
            checks["archive_public_200"] = archive_status == 200
            shape = report["steps"]["archive"]["shape"]
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
                " (ClosureProducer uses UnavailableLater).",
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
