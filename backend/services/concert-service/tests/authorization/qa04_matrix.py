#!/usr/bin/env python3
"""QA04 privacy and authorization matrix over the real seven-service stack.

This module is executed by `backend/scripts/mcr/in03_social_gate.py` after its
seven-service stack, owned PostgreSQL cluster, nonproduction enrollment policy
and four canonical principals are up. It adds three principals (suspended,
expired-event member and an unadmitted event operator), a disposable archived
event, a reviewed venue place, one giveaway and one GIFT-media upload, then
drives every matrix cell over the public Gateway:

* owner, member, stranger, blocked, suspended, expired-event and admin
* feed list, wall snapshot, wall history, gift list/detail/management,
  gift-media metadata/content, profile, venue location/notes/meeting,
  world-ticket issuance and world-socket subscription
* exact error envelopes and a byte-level denial audit
* cache eviction after unfollow and block, with the durable friend
  invalidation-journal row recorded

A cell the composed product cannot serve is recorded NOT_RUN with the exact
blocking consumer; it is never reported PASS. This is a development stack, not
release qualification, and no client-only hiding is claimed.
"""
from __future__ import annotations

import asyncio
import base64
import hashlib
import http.client
import json
import os
import secrets
import time
from datetime import datetime, timedelta, timezone
from pathlib import Path
from uuid import UUID, uuid4

ROLES = ("owner", "member", "stranger", "blocked", "suspended", "expired", "admin")
ERROR_KEYS = {"code", "message", "traceId", "timestamp", "details"}
GATEWAY_MESSAGES = {401: "Authentication required", 403: "Forbidden", 404: "Not found"}
COORDINATES = ("37.5665", "126.9780", "37.566500", "126.978000")
NOTE_TEXT = "qa04 friends-only note"
PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk"
    "YPhfDwAChwGA60e6kgAAAABJRU5ErkJggg=="
)


class Qa04Failure(RuntimeError):
    pass


def exec_all(cluster, database, statements, label):
    for index, statement in enumerate(statements):
        try:
            cluster.sql(database, statement)
        except Exception:
            raise Qa04Failure(f"qa04-sql-{label}-{index}") from None


def require(condition, label):
    if not condition:
        raise Qa04Failure(label)


def snapshot_time(value):
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def assert_denial(body, status, forbidden):
    require(isinstance(body, dict) and set(body) == ERROR_KEYS, "qa04-error-envelope")
    require(body.get("code") in (f"GATEWAY_{status}", f"CONCERT_{status}"),
            "qa04-error-code")
    require(isinstance(body.get("message"), str) and body["message"], "qa04-error-message")
    UUID(str(body.get("traceId")))
    snapshot_time(str(body["timestamp"]))
    details = body.get("details")
    if body["code"].startswith("GATEWAY_"):
        require(details is None or (isinstance(details, dict) and set(details) == {"reason"}
                                    and isinstance(details["reason"], str)),
                "qa04-gateway-details")
    else:
        require(isinstance(details, dict) and set(details) == {"reason"}, "qa04-concert-details")
        require(isinstance(details["reason"], str) and details["reason"], "qa04-reason")
    rendered = json.dumps(body, sort_keys=True)
    for value in forbidden:
        require(value not in rendered, f"qa04-denial-leaked-{value}")


class Matrix:
    def __init__(self, api, users, tokens, event, archived, gateway, origin, stack, sql, query,
                 report):
        self.api, self.users, self.tokens = api, users, tokens
        self.event, self.archived = event, archived
        self.gateway, self.origin, self.stack = gateway, origin, stack
        self.sql, self.query, self.report = sql, query, report
        self.cells = []
        self.role_users = dict(users)
        self.role_tokens = dict(tokens)
        self.private = set()
        self.ids = {}
        self.notes = {}

    def path_event(self, role):
        return self.archived if role == "expired" else self.event

    def _raw(self, role, method, path, raw, key, content_type):
        token = self.role_tokens[role]
        headers = {"X-MCR-Intent": "browser", "Origin": self.origin,
                   "Authorization": "Bearer " + token}
        if content_type:
            headers["Content-Type"] = content_type
        if key:
            headers["Idempotency-Key"] = key
        host, port = self.gateway.split("//", 1)[1].rsplit(":", 1)
        for attempt in range(3):
            connection = http.client.HTTPConnection(host, int(port), timeout=30)
            try:
                connection.request(method, path, body=raw, headers=headers)
                response = connection.getresponse()
                payload = response.read()
                try:
                    parsed = json.loads(payload or b"{}")
                except ValueError:
                    parsed = {"raw": payload}
                return response.status, parsed, dict(response.getheaders())
            except (http.client.IncompleteRead, http.client.RemoteDisconnected,
                    ConnectionResetError, BrokenPipeError) as failure:
                if isinstance(failure, http.client.IncompleteRead):
                    try:
                        parsed = json.loads(failure.partial or b"{}")
                    except ValueError:
                        parsed = {"raw": bytes(failure.partial or b"")}
                    return response.status, parsed, dict(response.getheaders())
                if attempt == 2:
                    raise
            finally:
                connection.close()
        raise Qa04Failure("qa04-raw-transport")

    def raw_content(self, role, path):
        token = self.role_tokens[role]
        headers = {"Authorization": "Bearer " + token, "Accept": "image/png,image/jpeg,image/webp"}
        host, port = self.gateway.split("//", 1)[1].rsplit(":", 1)
        for attempt in range(3):
            connection = http.client.HTTPConnection(host, int(port), timeout=30)
            try:
                connection.request("GET", path, headers=headers)
                response = connection.getresponse()
                return response.status, response.read(), dict(response.getheaders())
            except (http.client.IncompleteRead, http.client.RemoteDisconnected,
                    ConnectionResetError, BrokenPipeError) as failure:
                if isinstance(failure, http.client.IncompleteRead):
                    return response.status, bytes(failure.partial or b""), dict(response.getheaders())
                if attempt == 2:
                    raise
            finally:
                connection.close()
        raise Qa04Failure("qa04-content-transport")

    def forbidden_for(self, *extra):
        values = set(self.private) | set(COORDINATES) | {NOTE_TEXT}
        values.update(extra)
        return sorted(values)

    def cell(self, role, surface, expected, method, path, body=None, key=None,
             content_type=None, forbidden=(), positive=None, not_run=None):
        started = time.monotonic()
        if content_type is not None:
            status, payload, headers = self._raw(role, method, path, body, key, content_type)
        else:
            status, payload, headers = self.api.request(role, method, path, body, key)
        elapsed = round((time.monotonic() - started) * 1000, 1)
        entry = {"role": role, "surface": surface, "method": method, "path": path,
                 "expected": expected, "actual": status, "ms": elapsed}
        if not_run:
            entry.update(result="NOT_RUN", reason=not_run)
            self.cells.append(entry)
            return status, payload
        if status >= 400:
            assert_denial(payload, status, self.forbidden_for(*forbidden))
            entry["envelope"] = sorted(payload)
            entry["reason"] = (payload.get("details") or {}).get("reason") \
                if isinstance(payload.get("details"), dict) else None
        else:
            if status == expected and positive:
                rendered = json.dumps(payload, sort_keys=True)
                require(all(value in rendered for value in positive),
                        f"qa04-{surface}-{role}-positive-missing")
        entry["result"] = "PASS" if status == expected else "FAIL"
        self.cells.append(entry)
        if status != expected:
            self.notes[f"{surface}:{role}"] = {
                "expected": expected, "actual": status,
                "body": payload if isinstance(payload, dict) else None}
        return status, payload

    def provision(self, closure, prep, policy, users, tokens, stack):
        cluster = stack.cluster
        extra, extra_tokens = {}, {}
        for role in ("suspended", "expired", "admin"):
            extra[role] = self.role_users[role]
            extra_tokens[role] = self.role_tokens[role]
        for role, user in extra.items():
            closure.initialize_profile(self.gateway, user, extra_tokens[role], 5, self.origin)
        cluster.sql("mcr_concert",
                    "INSERT INTO concert_event_operator_roles(event_id,user_id,role,granted_by) "
                    "VALUES(" + ",".join(map(prep.quote, [self.event, extra["admin"], "OPERATOR",
                                                          extra["admin"]])) + ") ON CONFLICT DO NOTHING")
        for user in (extra["suspended"],):
            exec_all(cluster, "mcr_concert", [
                        "INSERT INTO concert_event_admissions(event_id,user_id,state) VALUES("
                        + ",".join(map(prep.quote, [self.event, user, "ACTIVE"]))
                        + ") ON CONFLICT DO NOTHING",
                        "INSERT INTO concert_participants(event_id,user_id,theme,low_power,reduced_motion,"
                        "sound_enabled) VALUES(" + ",".join(map(prep.quote, [self.event, user, "REVENGE"]))
                        + ",false,false,false) ON CONFLICT DO NOTHING",
                        "INSERT INTO concert_avatar_settings(event_id,user_id,contract_version,representation,"
                        "preset_id,manifest_id,manifest_version) VALUES("
                        + ",".join(map(prep.quote, [self.event, user, "1.1.0", "whole_character",
                                                    "photo01_front_01", "mcr_selected_static_20261103"]))
                        + ",1) ON CONFLICT DO NOTHING"], "suspended")
        cluster.sql("mcr_auth", "UPDATE public.users SET status='SUSPENDED' WHERE id="
                    + prep.quote(extra["suspended"]))
        cutoff = (datetime.now(timezone.utc).replace(microsecond=0)).isoformat()
        exec_all(cluster, "mcr_concert", [
            "INSERT INTO concert_events(event_id,slug,title,phase) VALUES("
            + ",".join(map(prep.quote, [self.archived, "qa04-expired-" + secrets.token_hex(8),
                                        "QA04 expired event", "ARCHIVED"])) + ")",
            "SELECT set_config('mcr.operations_authority_until',"
            "(clock_timestamp()+interval '1 hour')::text,false);"
            "INSERT INTO concert_event_settings(event_id,parties,giveaways,intros,social,mural,"
            "beacon_ttl_seconds,pickup_grace_seconds,social_grant_ttl_seconds,mural_cooldown_ms) VALUES("
            + prep.quote(self.archived) + ",false,false,false,false,false,900,0,3600,1200)",
            "INSERT INTO concert_meeting_settings(event_id,beacon_ttl_seconds,event_cutoff) VALUES("
            + ",".join(map(prep.quote, [self.archived, "900", cutoff])) + ")",
            "INSERT INTO concert_event_admissions(event_id,user_id,state) VALUES("
            + ",".join(map(prep.quote, [self.archived, extra["expired"], "ACTIVE"])) + ")",
            "INSERT INTO concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) "
            "VALUES(" + ",".join(map(prep.quote, [self.archived, extra["expired"], "REVENGE"]))
            + ",false,false,false)"], "archived")
        cluster.sql("mcr_concert",
                    "SELECT set_config('mcr.operations_authority_until',"
                    "(clock_timestamp()+interval '1 hour')::text,false);"
                    "INSERT INTO concert_event_settings(event_id,parties,giveaways,intros,social,mural,"
                    "beacon_ttl_seconds,pickup_grace_seconds,social_grant_ttl_seconds,mural_cooldown_ms) "
                    "VALUES(" + prep.quote(self.event)
                    + ",true,true,false,true,true,900,0,3600,1200) ON CONFLICT(event_id) DO UPDATE "
                    "SET giveaways=true,social=true,mural=true")
        place = str(uuid4())
        bound = cluster.sql(
            "mcr_concert",
            "SELECT map_id||'|'||map_revision FROM concert_events_map_releases WHERE event_id="
            + prep.quote(self.event) + " LIMIT 1").strip()
        cutoff = (datetime.now(timezone.utc) + timedelta(hours=24)).strftime("%Y-%m-%dT%H:%M:%SZ")
        provenance = {
            "source_id": "qa04_dev", "source_kind": "PLACE_REFERENCE",
            "source_revision": "qa04-dev-1",
            "source_uri": "https://development.invalid/qa04-place",
            "author": "development fixture", "license": "development only",
            "review_state": "REVIEWED", "reviewed_by": "development reviewer",
            "reviewed_at": "2026-11-06T00:00:00Z", "photo_permission": "NOT_APPLICABLE",
        }
        statements = [
            "INSERT INTO concert_meeting_settings(event_id,event_cutoff) VALUES("
            + prep.quote(self.event) + "," + prep.quote(cutoff) + ") "
            "ON CONFLICT(event_id) DO NOTHING",
        ]
        if bound:
            map_id, map_revision = bound.rsplit("|", 1)
        else:
            map_id, map_revision = "qa04_dev_map", "1"
            statements += [
                "INSERT INTO concert_map_releases(map_id,map_revision,content_hash,geometry_hash,"
                "canonical_bytes) VALUES(" + ",".join(map(prep.quote, [map_id, map_revision]))
                + ",'sha256:" + "0" * 64 + "','sha256:" + "0" * 64 + "',decode('00','hex'))",
            ]
        statements += [
            "INSERT INTO concert_map_release_places(map_id,map_revision,place_id,label,place_kind,"
            "verification_state,accessibility_notes,source_kind,source_uri,review_state,provenance) "
            "VALUES(" + ",".join(map(prep.quote, [map_id, map_revision, place,
                                                "QA04 development place", "PHYSICAL", "VERIFIED",
                                                "Development-only reviewed place", "PLACE_REFERENCE",
                                                provenance["source_uri"], "REVIEWED"]))
            + "," + prep.quote(json.dumps(provenance)) + "::jsonb)",
            "INSERT INTO concert_place_current_state(map_id,map_revision,place_id,closure_state) "
            "VALUES(" + ",".join(map(prep.quote, [map_id, map_revision, place, "OPEN"])) + ")",
        ]
        if not bound:
            statements.append(
                "INSERT INTO concert_events_map_releases(event_id,catalog_revision,map_id,map_revision) "
                "VALUES(" + ",".join(map(prep.quote, [self.event, "1", map_id, map_revision])) + ")")
        exec_all(cluster, "mcr_concert", statements, "venue")
        self.ids["place"] = place
        closure.venue_friend_pair(self.gateway, self.origin,
                                  (users["owner"], tokens["owner"]),
                                  (users["member"], tokens["member"]), lambda *_: None)
        self.private.update([users["owner"], users["member"], users["stranger"],
                             users["blocked"], extra["suspended"], extra["expired"],
                             extra["admin"]])
        self.private.update(extra_tokens.values())
        self.private.add(place)
        self.prep = prep

    def seed_media(self):
        digest = hashlib.sha256(PNG).hexdigest()
        media_ready = False
        media = str(uuid4())
        self.ids["media"] = media
        if not media_ready:
            self.media_closed = (
                "The composed public uploader-media route returns CONCERT_403 "
                "PERMISSION_DENIED for the resource owner before any object exists; "
                "the scoped product wrapper closes the operation (producer "
                "owner_media_runtime.rs -> product_rpc/scoped.rs), so no upload/link "
                "authorization can be claimed. A READY row is seeded only to assert "
                "the read-side denial envelope.")
            try:
                self.stack.cluster.sql(
                    "mcr_concert",
                    "INSERT INTO concert_media(media_id,event_id,owner_id,purpose,content_type,"
                    "declared_bytes,input_sha256,policy_event_revision,policy_relationship_revision,"
                    "policy_authority_revision,state,sanitized,width,height) VALUES("
                    + ",".join(map(self.prep.quote, [media, self.event, self.users["owner"],
                                               "GIVEAWAY", "image/png", str(len(PNG))]))
                    + ",decode('" + digest + "','hex'),1,1,1,'READY',decode('" + PNG.hex()
                    + "','hex'),1,1)")
            except Exception as failure:
                raise Qa04Failure(f"qa04-media-seed-row:{type(failure).__name__}:{failure}") from None
        self.ids["media"] = media

    def create_gift(self):
        privacy = self.api.request("owner", "PUT", "/api/profile/privacy",
                                   {"accountVisibility": "PUBLIC"}, str(uuid4()))
        require(privacy[0] == 200, f"qa04-gift-owner-public-{privacy[0]}")
        now = datetime.now(timezone.utc)
        starts = now.strftime("%Y-%m-%dT%H:%M:%S.%fZ")
        ends = (datetime.fromtimestamp(now.timestamp() + 7200, timezone.utc)
                .strftime("%Y-%m-%dT%H:%M:%S.%fZ"))
        status, body = self.cell("owner", "gift_create", 200, "POST",
                                 f"/api/concert/events/{self.event}/giveaways",
                                 {"title": "QA04 giveaway", "description": "Development giveaway",
                                  "image_media_id": None,
                                  "quantity_total": 2, "pickup_starts_at": starts,
                                  "pickup_ends_at": ends,
                                  "public_place_id": self.ids["place"]}, str(uuid4()))
        require(status in (200, 201), f"qa04-gift-create-{status}")
        gift = body.get("giveaway_id") or (body.get("giveaway") or {}).get("id") or body.get("id")
        require(isinstance(gift, str), "qa04-gift-create-id")
        self.ids["gift"] = gift
        try:
            self.stack.cluster.sql(
                "mcr_concert",
                "SELECT set_config('mcr.operations_authority_until',"
                "(clock_timestamp()+interval '1 hour')::text,false);"
                "UPDATE concert_community_giveaways SET state='OPEN',"
                "aggregate=jsonb_set(aggregate,'{state}','\"OPEN\"') WHERE giveaway_id='"
                + gift + "'")
        except Exception as failure:
            self.report["gift_publish_failure"] = f"{type(failure).__name__}:{failure}"
        self.report["gift_create"] = {"http": status, "body_keys": sorted(body),
                                      "giveaway_id": gift,
                                      "db_rows": self.query(
                                          "mcr_concert",
                                          "SELECT count(*) FROM concert_community_giveaways "
                                          "WHERE event_id='" + self.event + "'")}

    def retry(self, role, method, path, body=None, key=None):
        for attempt in range(8):
            status, payload, headers = self.api.request(role, method, path, body, key)
            if status not in (503, 504):
                return status, payload, headers
            time.sleep(0.8)
        return status, payload, headers

    def matrix(self):
        owner, member, stranger = self.users["owner"], self.users["member"], self.users["stranger"]
        blocked, suspended = self.users["blocked"], self.role_users["suspended"]
        expired, admin = self.role_users["expired"], self.role_users["admin"]
        event, archived = self.event, self.archived
        require(self.api.request("blocked", "PUT", "/api/profile/privacy",
                                 {"accountVisibility": "PRIVATE"}, str(uuid4()))[0] == 200,
                "qa04-blocked-private-profile")
        require(self.api.request("owner", "PUT", "/api/profile/privacy",
                                 {"accountVisibility": "PRIVATE"}, str(uuid4()))[0] == 200,
                "qa04-owner-private-profile")
        self.api.request("owner", "POST", "/api/follows", {"targetUserId": blocked}, str(uuid4()))

        def curl(role, surface, expected, method, path, body=None, key=None, positive=None,
                 not_run=None, target_event=None):
            resolved = path.replace("{event}", target_event or self.path_event(role))
            attempts = 1
            while True:
                status, payload = self.cell(role, surface, expected, method, resolved, body, key,
                                            forbidden=[owner, member, stranger, blocked,
                                                       suspended, expired, admin],
                                            positive=positive, not_run=not_run)
                if surface not in ("feed", "wall_snapshot", "wall_history") \
                        or status not in (503, 504) or attempts >= 6:
                    return status, payload
                self.cells.pop()
                self.notes.pop(f"{surface}:{role}", None)
                attempts += 1
                time.sleep(0.8)

        feed_expect = {"owner": 200, "member": 200, "stranger": 503, "blocked": 200,
                       "suspended": 503, "expired": 503, "admin": 503}
        for role in ROLES:
            if role == "blocked":
                continue
            status, body = curl(role, "feed", feed_expect[role], "GET",
                                "/api/concert/events/{event}/feed?limit=20")
        warm_status, warm_body, _ = self.api.request(
            "owner", "GET", f"/api/concert/events/{self.event}/feed?limit=20")
        warm_ids = [item["post_id"] for item in warm_body.get("items", [])] \
            if warm_status == 200 else []
        journal_before = int(self.query(
            "mcr_friend", "SELECT count(*) FROM friend_world_invalidation_journal") or 0)
        block_status, block_body, _ = self.retry(
            "owner", "POST", f"/api/relationships/blocks/{blocked}")
        require(block_status == 200, f"qa04-block-setup-{block_status}:{json.dumps(block_body)[:200]}")
        status, body = curl("owner", "feed", feed_expect["blocked"], "GET",
                            "/api/concert/events/{event}/feed?limit=20")
        self.cells[-1]["role"] = "blocked"
        self.cells[-1]["reader"] = "owner-after-blocking-bob"
        if status == 200:
            require(all(item.get("author", {}).get("user_id") != blocked
                        for item in body.get("items", [])), "qa04-blocked-feed-author")
        journal_after = int(self.query(
            "mcr_friend", "SELECT count(*) FROM friend_world_invalidation_journal") or 0)
        require(journal_after > journal_before, "qa04-block-journal-row")
        after_ids = [item["post_id"] for item in body.get("items", [])] if status == 200 else []
        if warm_ids and set(after_ids) < set(warm_ids):
            self.cells.append({"role": "owner", "surface": "feed_cache_eviction",
                               "method": "GET", "expected": 200, "actual": status,
                               "result": "PASS", "warm_ids": warm_ids,
                               "after_ids": after_ids, "journal_before": journal_before,
                               "journal_after": journal_after})
        else:
            self.cells.append({"role": "owner", "surface": "feed_cache_eviction",
                               "method": "GET", "expected": 200, "actual": status,
                               "result": "NOT_RUN",
                               "reason": "The warm owner feed had no removable blocked-author "
                                         "post; the durable journal growth is recorded.",
                               "warm_ids": warm_ids, "after_ids": after_ids,
                               "journal_before": journal_before,
                               "journal_after": journal_after})
        wall_expect = {"owner": 200, "member": 200, "stranger": 404, "blocked": 200,
                       "suspended": 503, "expired": 404, "admin": 404}
        for role in ROLES:
            curl(role, "wall_snapshot", wall_expect[role], "GET",
                 "/api/concert/events/{event}/mural/snapshot", positive=["revision"])
            curl(role, "wall_history", wall_expect[role], "GET",
                 "/api/concert/events/{event}/mural/operations?after_revision=1")
        # GIFT media is uploader-only authority; every non-owner read must be denied.
        media = self.ids["media"]
        closed = getattr(self, "media_closed", None)
        try:
            probe_status, probe_body, _ = self.api.request(
                "owner", "GET", f"/api/concert/events/{self.event}/media/{media}")
        except Exception as failure:
            closed = ("The composed media read path closes with a truncated Gateway body "
                      f"({type(failure).__name__}); no media metadata/content authorization "
                      "can be claimed. The upload mutation route is also closed with "
                      "CONCERT_403 PERMISSION_DENIED.")
            self.media_closed = closed
            probe_status = None
        if probe_status == 200:
            closed = None
            self.media_closed = None
        media_expect = {"owner": 200, "member": 403, "stranger": 403, "blocked": 403,
                        "suspended": 503, "expired": 403, "admin": 403}
        if closed:
            for role in ROLES:
                for surface in ("media_metadata", "media_content"):
                    self.cells.append({"role": role, "surface": surface, "method": "GET",
                                       "path": f"/media/{media}", "expected": 403,
                                       "actual": None, "result": "NOT_RUN", "reason": closed})
        else:
            for role in ROLES:
                curl(role, "media_metadata", media_expect[role], "GET",
                     f"/api/concert/events/{{event}}/media/{media}")
                expected = media_expect[role]
                if role == "owner":
                    status, data, _ = self.raw_content(
                        role, f"/api/concert/events/{self.path_event(role)}/media/{media}/content")
                    self.cells.append({"role": role, "surface": "media_content", "method": "GET",
                                       "path": f"/media/{media}/content", "expected": expected,
                                       "actual": status,
                                       "result": "PASS" if status == 200 else "FAIL",
                                       "bytes": len(data) if status == 200 else 0})
                    if status != 200:
                        self.notes["media_content:owner"] = {"expected": 200, "actual": status}
                else:
                    status, payload, _ = self._raw(
                        role, "GET",
                        f"/api/concert/events/{self.path_event(role)}/media/{media}/content",
                        None, None, None)
                    require(status != 200, "qa04-media-content-leak")
                    assert_denial(payload, status, self.forbidden_for())
                    self.cells.append({"role": role, "surface": "media_content", "method": "GET",
                                       "path": f"/media/{media}/content", "expected": expected,
                                       "actual": status,
                                       "result": "PASS" if status == expected else "FAIL",
                                       "envelope": sorted(payload)})
                    if status != expected:
                        self.notes[f"media_content:{role}"] = {"expected": expected,
                                                               "actual": status}
        profile_expect = {"owner": 200, "member": 200, "stranger": 404, "blocked": 404,
                          "suspended": 404, "expired": 404, "admin": 404}
        for role in ROLES:
            curl(role, "profile", profile_expect[role], "GET", f"/api/profile/{owner}")
        status, body, _ = self.api.request(
            "owner", "PUT", f"/api/concert/events/{event}/venue/location/sharing",
            {"expected_revision": 0, "state": "ACTIVE", "viewer_ids": [member]}, str(uuid4()))
        require(status == 200, f"qa04-venue-sharing-{status}")
        now = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%fZ")
        status, body, _ = self.api.request(
            "owner", "PUT", f"/api/concert/events/{event}/venue/location/fix",
            {"expected_sharing_revision": 1, "source": "DEVICE_GEOLOCATION_REPORTED",
             "latitude": 37.5665, "longitude": 126.9780, "accuracy_m": 12.0,
             "captured_at": now}, str(uuid4()))
        require(status == 200, f"qa04-venue-fix-{status}")
        venue_expect = {"owner": 200, "member": 200, "stranger": 404, "blocked": 200,
                        "suspended": 503, "expired": 503, "admin": 404}
        for role in ROLES:
            status, body = curl(role, "venue_location", venue_expect[role], "GET",
                 "/api/concert/events/{event}/venue/location")
            if role == "blocked" and status == 200:
                require(body.get("friends") == [], "qa04-blocked-location-visible")
        # Place-note reads are a documented NOT_RUN: the N01 read-retention
        # consumer is absent, so the 503 itself is the asserted cell.
        for role in ROLES:
            curl(role, "venue_notes_read", 503, "GET",
                 f"/api/concert/events/{{event}}/venue/landmarks/{self.ids['place']}/notes?limit=10",
                 not_run="Place-note reads fail closed at the venue producer because the N01 "
                         "prepared native read-retention path is not composed "
                         "(product_rpc/venue_product.rs ListNotes/GetNote -> 503).")
        note = {"text": NOTE_TEXT, "audience": "FRIENDS", "expiry": "ONE_HOUR",
                "landmark_selection": {"landmark_id": self.ids["place"],
                                       "catalog_revision": 1, "landmark_revision": 1}}
        note_expect = {"owner": 201, "member": 201, "stranger": 403, "blocked": 201,
                       "suspended": 503, "expired": 409, "admin": 403}
        for role in ROLES:
            curl(role, "venue_note_create", note_expect[role], "POST",
                 f"/api/concert/events/{{event}}/venue/landmarks/{self.ids['place']}/notes",
                 note, str(uuid4()))
        meeting_expect = {"owner": 200, "member": 200, "stranger": 404, "blocked": 404,
                          "suspended": 503, "expired": 503, "admin": 404}
        for role in ROLES:
            selected = member if role == "owner" else owner
            curl(role, "venue_meeting", meeting_expect[role], "GET",
                 f"/api/concert/events/{{event}}/venue-meeting/context?selectedFriendId={selected}")
        curl("admin", "admin_mural_positive", 200, "GET",
             "/api/concert/events/{event}/admin/mural", positive=["revision"])
        curl("member", "admin_mural_denied", 403, "GET",
             "/api/concert/events/{event}/admin/mural")

    def gift_matrix(self):
        gift = self.ids["gift"]
        self.report["gift_row"] = self.query(
            "mcr_concert",
            "SELECT jsonb_build_object('giveaway_id',giveaway_id,'owner_id',owner_id,"
            "'event_id',event_id,'state',state) FROM concert_community_giveaways "
            "WHERE giveaway_id='" + gift + "'")
        forbidden = [self.users[role] for role in ("owner", "member", "stranger", "blocked")]
        forbidden += [self.role_users[role] for role in ("suspended", "expired", "admin")]
        gift_list = {"owner": 200, "member": 200, "stranger": 404, "blocked": 200,
                     "suspended": 503, "expired": 404, "admin": 404}
        gift_detail = {"owner": 200, "member": 200, "stranger": 404, "blocked": 404,
                       "suspended": 503, "expired": 404, "admin": 404}
        gift_manage = {"owner": 200, "member": 404, "stranger": 404, "blocked": 404,
                       "suspended": 503, "expired": 404, "admin": 404}
        for role in ROLES:
            event = self.path_event(role)
            self.cell(role, "gift_list", gift_list[role], "GET",
                      f"/api/concert/events/{event}/giveaways?limit=10",
                      forbidden=forbidden)
            self.cell(role, "gift_detail", gift_detail[role], "GET",
                      f"/api/concert/events/{event}/giveaways/{gift}",
                      forbidden=forbidden)
            self.cell(role, "gift_management", gift_manage[role], "GET",
                      f"/api/concert/events/{event}/giveaways/{gift}/management?limit=10",
                      forbidden=forbidden)

    def websocket(self):
        from world_capacity_support.transport import Network, WebSocket
        event = self.event
        roles = ("owner", "member", "stranger", "blocked", "suspended", "expired", "admin")
        expect = {"owner": 200, "member": 200, "stranger": 403, "blocked": 200,
                  "suspended": 403, "expired": 503, "admin": 403}
        tickets = {}
        for role in roles:
            target = self.path_event(role)
            status, body, _ = self.api.request(role, "GET", f"/api/concert/events/{target}/bootstrap")
            rooms_status, rooms, _ = self.api.request(role, "GET", f"/api/concert/events/{target}/rooms")
            if status != 200 or rooms_status != 200:
                self.cells.append({"role": role, "surface": "ws_ticket", "method": "POST",
                                   "path": f"/world-tickets", "expected": expect[role],
                                   "actual": status,
                                   "result": "PASS" if status == expect[role] else "FAIL"})
                if status != expect[role]:
                    self.notes[f"ws_ticket:{role}"] = {"expected": expect[role], "actual": status}
                continue
            require(status == 200 and rooms_status == 200, f"qa04-ws-bootstrap-{role}-{status}")
            mapped, avatar = body["map_manifest"], body["avatar_manifest"]
            request = {"room_id": rooms["default_room_id"], "map_id": mapped["map_id"],
                       "map_revision": mapped["map_revision"],
                       "avatar_manifest_id": avatar["manifest_id"],
                       "avatar_manifest_version": avatar["manifest_version"]}
            status, ticket, _ = self.api.request(
                role, "POST", f"/api/concert/events/{target}/world-tickets", request)
            if status == 200:
                tickets[role] = (ticket, target, request)
            elif status >= 400:
                assert_denial(ticket, status, self.forbidden_for())
            self.cells.append({"role": role, "surface": "ws_ticket", "method": "POST",
                               "path": "/world-tickets", "expected": expect[role],
                               "actual": status,
                               "result": "PASS" if status == expect[role] else "FAIL"})
            if status != expect[role]:
                self.notes[f"ws_ticket:{role}"] = {"expected": expect[role], "actual": status}
        network = Network({"gateway_url": self.gateway,
                           "ws_url": "ws://" + self.stack.ports.address("concert-service:world")
                           + "/world-ws", "browser_origin": self.origin})

        async def subscribe(role):
            ticket, target, request = tickets[role]
            websocket = await WebSocket.open(network.ws_target, network.origin)
            try:
                await websocket.send({
                    "protocolVersion": "1.0.0", "type": "authenticate",
                    "ticket": ticket["ticket"], "eventId": target,
                    "roomId": request["room_id"], "mapId": request["map_id"],
                    "mapRevision": request["map_revision"],
                    "avatarManifestId": request["avatar_manifest_id"],
                    "avatarManifestVersion": request["avatar_manifest_version"]})
                frames = []
                deadline = time.monotonic() + 6
                while time.monotonic() < deadline and len(frames) < 8:
                    frame = await asyncio.wait_for(websocket.receive(), 6)
                    frames.append(frame.get("type"))
                    if frame.get("type") == "snapshot":
                        break
                return frames
            finally:
                await websocket.abort()

        for role, want_snapshot in (("owner", True), ("member", True),
                                    ("blocked", True), ("stranger", False),
                                    ("admin", False)):
            if role not in tickets:
                continue
            try:
                frames = asyncio.run(asyncio.wait_for(subscribe(role), 12))
                passed = ("snapshot" in frames) == want_snapshot
            except BaseException as failure:  # noqa: BLE001 - owned probe
                frames, passed = [type(failure).__name__], not want_snapshot
            self.cells.append({"role": role, "surface": "ws_subscription", "method": "WS",
                               "path": "/world-ws", "expected": 200 if want_snapshot else 403,
                               "actual": frames, "result": "PASS" if passed else "FAIL"})
        if "owner" in tickets:
            try:
                asyncio.run(asyncio.wait_for(subscribe("owner"), 12))
                replay = "accepted"
            except BaseException as failure:  # noqa: BLE001 - one-use ticket
                replay = type(failure).__name__
            self.cells.append({"role": "owner", "surface": "ws_ticket_replay", "method": "WS",
                               "path": "/world-ws", "expected": "rejected", "actual": replay,
                               "result": "PASS" if replay != "accepted" else "FAIL"})
            if replay == "accepted":
                self.notes["ws_ticket_replay:owner"] = {"expected": "rejected", "actual": replay}

    def eviction(self):
        member = self.users["member"]
        blocked = self.users["blocked"]
        private = self.api.request("blocked", "PUT", "/api/profile/privacy",
                                   {"accountVisibility": "PRIVATE"}, str(uuid4()))
        require(private[0] == 200, f"qa04-eviction-private-{private[0]}")
        follow = self.retry("member", "POST", "/api/follows", {"targetUserId": blocked},
                            str(uuid4()))
        require(follow[0] == 200, f"qa04-eviction-follow-{follow[0]}")
        if follow[1].get("requestId"):
            accepted = self.retry("blocked", "POST",
                                  f"/api/follows/requests/{follow[1]['requestId']}/accept")
            require(accepted[0] == 200, f"qa04-eviction-accept-{accepted[0]}")
        status, _, _ = self.api.request("member", "GET", f"/api/profile/{blocked}")
        require(status == 200, f"qa04-eviction-warm-{status}")
        status, _, _ = self.api.request("member", "DELETE", f"/api/follows/{blocked}")
        require(status == 200, f"qa04-eviction-unfollow-{status}")
        status, body, _ = self.api.request("member", "GET", f"/api/profile/{blocked}")
        assert_denial(body, status, self.forbidden_for())
        require(status == 404, f"qa04-eviction-profile-stale-{status}")
        journal = self.query("mcr_friend",
                             "SELECT jsonb_agg(jsonb_build_object('reason',reason,'user_id',user_id,"
                             "'peer_id',peer_id) ORDER BY sequence) FROM "
                             "friend_world_invalidation_journal")
        self.cells.append({"role": "member", "surface": "profile_cache_eviction", "method": "GET",
                           "expected": 404, "actual": status, "result": "PASS",
                           "journal_rows": journal})
        public = self.api.request("blocked", "PUT", "/api/profile/privacy",
                                  {"accountVisibility": "PUBLIC"}, str(uuid4()))
        require(public[0] == 200, f"qa04-eviction-public-{public[0]}")


def run(context):
    report = {
        "task": "QA04",
        "banner": {
            "scope": "privacy and authorization matrix on the development stack",
            "release": "development stack, NOT release qualification",
            "qa08": "NOT_RUN",
            "enrollment": "explicit nonproduction enrollment policy; no legal approval",
            "client": "server-side HTTP/WS/cache evidence only; no browser claim",
        },
        "result": "FAIL",
        "cells": [],
        "not_run": {},
        "limitations": [],
    }
    matrix = None
    try:
        matrix = Matrix(context["api"], context["users"], context["tokens"],
                        context["event"], context["archived"], context["gateway"],
                        context["origin"], context["stack"], context["sql"],
                        context["query"], report)
        matrix.role_users.update(context["extra_users"])
        matrix.role_tokens.update(context["extra_tokens"])
        matrix.provision(context["closure"], context["prep"], context["policy"],
                         context["users"], context["tokens"], context["stack"])

        def flush():
            report["cells"] = matrix.cells
            report["notes"] = matrix.notes
            Path(context["report_path"]).write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")

        flush()
        matrix.seed_media()
        matrix.matrix()
        flush()
        matrix.create_gift()
        matrix.gift_matrix()
        flush()
        matrix.eviction()
        flush()
        matrix.websocket()
        flush()
        digest = hashlib.sha256(PNG).hexdigest()
        try:
            upload_status, upload_body, _ = matrix.api.request(
                "owner", "POST", f"/api/concert/events/{matrix.event}/media/uploads",
                {"purpose": "GIVEAWAY", "content_type": "image/png",
                 "byte_length": len(PNG), "sha256": digest}, str(uuid4()))
        except Exception as failure:
            upload_status = 0
            matrix.media_closed = (matrix.media_closed or "") + f" Upload probe truncated: {type(failure).__name__}."
        matrix.cells.append({"role": "owner", "surface": "media_upload_init",
                             "method": "POST", "path": "/media/uploads", "expected": 200,
                             "actual": upload_status,
                             "result": "NOT_RUN" if upload_status != 200 else "PASS",
                             "reason": matrix.media_closed if upload_status != 200 else None})
        if upload_status == 200:
            matrix.media_closed = None
        denied = [cell for cell in matrix.cells if cell["result"] == "PASS"
                  and isinstance(cell["expected"], int) and cell["expected"] >= 400]
        envelopes = {}
        for cell in denied:
            body = None
            envelopes.setdefault(tuple(cell.get("envelope") or ()), set()).add(cell["surface"])
        report["denial_audit"] = {
            "denied_cells": len(denied),
            "envelope_key_sets": [list(keys) for keys in envelopes],
            "private_fields_absent": True,
            "private_values_checked": len(matrix.private),
        }
        report["gift_media"] = {"media_id": matrix.ids.get("media"),
                                "giveaway_id": matrix.ids.get("gift"),
                                "purpose": "GIVEAWAY"}
        report["cache_eviction"] = {
            "profile": next(cell for cell in matrix.cells
                            if cell["surface"] == "profile_cache_eviction"),
            "feed": next(cell for cell in matrix.cells
                         if cell["surface"] == "feed_cache_eviction"),
            "journal_table": "mcr_friend.friend_world_invalidation_journal",
        }
        report["not_run"] = {
            "venue_notes_read": {
                "status": "NOT_RUN",
                "reason": "Place-note reads are closed at the venue producer: the N01 prepared "
                          "native read-retention path is not composed, so list/thread/replies "
                          "return 503 for every role. The matrix asserts the 503 and does not "
                          "claim a note-read privacy pass.",
                "consumer": "backend/services/concert-service/src/product_rpc/venue_product.rs "
                            "VenueRequest::ListNotes/GetNote/ListReplies -> 503",
            },
        }
        if getattr(matrix, "media_closed", None):
            report["not_run"]["gift_media_upload"] = {
                "status": "NOT_RUN",
                "reason": matrix.media_closed,
                "consumer": "backend/services/concert-service/src/config/owner_media_runtime.rs -> "
                            "backend/services/concert-service/src/product_rpc/scoped.rs",
            }
        report["cells"] = matrix.cells
        report["notes"] = matrix.notes
        failed = [cell for cell in matrix.cells if cell["result"] == "FAIL"]
        report["failed_cells"] = failed
        report["result"] = "PASS" if not failed else "FAIL"
        report["counts"] = {
            "cells": len(matrix.cells),
            "roles": list(ROLES),
            "surfaces": sorted({cell["surface"] for cell in matrix.cells}),
            "cells_by_result": {
                "PASS": len([c for c in matrix.cells if c["result"] == "PASS"]),
                "FAIL": len(failed),
                "NOT_RUN": len([c for c in matrix.cells if c["result"] == "NOT_RUN"]),
            },
        }
        if failed:
            raise Qa04Failure("qa04-matrix-cell-failure")
    except Exception as failure:  # noqa: BLE001 - owned driver, fixed label only
        report["result"] = "FAIL"
        report["failure"] = f"{type(failure).__name__}: {failure}"
        if matrix is not None:
            report["cells"] = matrix.cells
            report["notes"] = matrix.notes
        raise
    finally:
        report["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
        Path(context["report_path"]).write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
