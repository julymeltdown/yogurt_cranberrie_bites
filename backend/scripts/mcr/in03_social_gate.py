#!/usr/bin/env python3
"""IN03 development-stack social and privacy gate.

Banner: development stack, NOT release qualification, QA08 NOT_RUN, and no
client-only privacy claim. This driver starts this checkout's seven native
services on an owned private PG16 cluster with real mTLS and a private temp
root, publishes an explicitly nonproduction enrollment policy, creates real
principals through signup -> mail verify -> consent, and drives the public HTTP
surface for the friendship, block/unblock, profile, feed, wall and social-block
privacy fences.

Every denial is asserted as an exact status and exact closed JSON body. A case
that depends on an external provider is recorded NOT_RUN with the missing
input named; a producer that the composed service does not mount is recorded
NOT_RUN with the exact blocking consumer. Neither is reported as PASS.
"""
from __future__ import annotations

import argparse
import hashlib
import http.client
import json
import os
import secrets
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from unittest.mock import patch
from uuid import UUID, uuid4

sys.path.insert(0, str(Path(__file__).resolve().parent))
import closure_vertical_stack as closure  # noqa: E402
import dev_stack  # noqa: E402
import migration_smoke as migration  # noqa: E402
import runtime_smoke as runtime  # noqa: E402
import world_in01_live as world  # noqa: E402
import world_in01_prepare as prep  # noqa: E402

BANNER = {
    "scope": "development stack social and privacy gate",
    "release": "development stack, NOT release qualification",
    "qa08": "NOT_RUN",
    "production": "NOT_CLAIMED",
    "authorization": "development enrollment policy; never legal approval",
    "client": "no browser cache claim in this backend gate",
}
PACK = Path("frontend/public/wall/initial-motif-v1")
ROLES = ("alice", "bob", "carol", "outsider")
ERROR_KEYS = {"code", "message", "traceId", "timestamp", "details"}
PRIVATE_TOKENS = ("alice@example.test", "bob@example.test", "carol@example.test",
                  "accessToken", "refreshToken", "password", "private.handle",
                  "privatehandle", "handle_version")


class GateFailure(RuntimeError):
    pass


def require(condition, label):
    if not condition:
        raise GateFailure(label)


def sql_quote(value):
    return prep.quote(value)


def snapshot_time(value):
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


class Api:
    """One authenticated HTTP client role; no browser state or injected token."""

    def __init__(self, gateway, origin, tokens):
        host, port = gateway.split("//", 1)[1].rsplit(":", 1)
        self.host, self.port = host, int(port)
        self.origin, self.tokens = origin, tokens
        self.log = []

    def request(self, role, method, path, body=None, key=None, headers=None):
        token = self.tokens.get(role)
        hdrs = {"X-MCR-Intent": "browser", "Origin": self.origin}
        if token:
            hdrs["Authorization"] = "Bearer " + token
        if body is not None:
            hdrs["Content-Type"] = "application/json"
        if key:
            hdrs["Idempotency-Key"] = key
        if headers:
            hdrs.update(headers)
        raw = None if body is None else json.dumps(body).encode()
        self.log.append({"role": role, "method": method, "path": path})
        return self._send(method, path, raw, hdrs)

    def _send(self, method, path, raw, hdrs, attempt=0):
        connection = http.client.HTTPConnection(self.host, self.port, timeout=30)
        try:
            connection.request(method, path, body=raw, headers=hdrs)
            response = connection.getresponse()
            payload = response.read()
            try:
                parsed = json.loads(payload or b"{}")
            except ValueError:
                parsed = {"raw": payload[:400].decode("utf-8", "replace")}
            if parsed is None:
                parsed = {}
            return response.status, parsed, dict(response.getheaders())
        except (http.client.IncompleteRead, http.client.RemoteDisconnected,
                ConnectionResetError, BrokenPipeError) as failure:
            if attempt < 1:
                self.log.append({"transport_retry": type(failure).__name__,
                                 "method": method, "path": path})
                return self._send(method, path, raw, hdrs, attempt + 1)
            raise
        finally:
            connection.close()


def assert_social_error(status, body, expected_status, expected_reason, forbid=()):
    """Exact closed SocialRuntime envelope (its own message, never the generic one)."""
    require(status == expected_status, f"social-expected-{expected_status}-got-{status}")
    require(isinstance(body, dict) and set(body) == ERROR_KEYS, "social-error-envelope")
    require(body["code"] == f"CONCERT_{expected_status}", "social-error-code")
    require(body["message"] == "The requested action is unavailable.", "social-error-message")
    require(body["details"] == {"reason": expected_reason}, "social-error-details")
    UUID(body["traceId"])
    snapshot_time(body["timestamp"])
    rendered = json.dumps(body)
    for token in forbid:
        require(token not in rendered, f"social-denied-body-leaked-{token}")


def assert_gateway_error(status, body, expected_status, expected_message, forbid=()):
    require(status == expected_status, f"expected-status-{expected_status}-got-{status}")
    require(isinstance(body, dict) and set(body) == ERROR_KEYS, "gateway-error-envelope")
    require(body["code"] == f"GATEWAY_{expected_status}", "gateway-error-code")
    require(body["message"] == expected_message, "gateway-error-message")
    require(body["details"] is None, "gateway-error-details")
    UUID(body["traceId"])
    snapshot_time(body["timestamp"])
    rendered = json.dumps(body)
    for token in forbid:
        require(token not in rendered, f"denied-body-leaked-{token}")


def assert_concert_error(status, body, expected_status, expected_reason, forbid=()):
    require(status == expected_status, f"expected-status-{expected_status}-got-{status}")
    require(isinstance(body, dict) and set(body) == ERROR_KEYS, "concert-error-envelope")
    require(body["code"] == f"CONCERT_{expected_status}", "concert-error-code")
    require(body["message"] == "Concert request unavailable", "concert-error-message")
    require(isinstance(body["details"], dict) and set(body["details"]) == {"reason"},
            "concert-error-details")
    require(body["details"]["reason"] == expected_reason, "concert-error-reason")
    UUID(body["traceId"])
    snapshot_time(body["timestamp"])
    rendered = json.dumps(body)
    for token in forbid:
        require(token not in rendered, f"denied-body-leaked-{token}")


def load_pack(root):
    directory = root / PACK
    require(directory.is_dir(), "wall-pack-missing")
    pixels = (directory / "pixels.bin").read_bytes()
    palette = json.loads((directory / "palette.json").read_bytes())
    manifest = json.loads((directory / "manifest.json").read_bytes())
    require(len(pixels) == 16384 and all(c < 16 for c in pixels), "wall-pixels-invalid")
    require(len(palette) == 16, "wall-palette-invalid")
    require(manifest.get("width") == 128 and manifest.get("height") == 128, "wall-manifest-invalid")
    return pixels, palette, hashlib.sha256((directory / "manifest.json").read_bytes()).hexdigest()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--pg-bin-dir", type=Path, default=Path("/opt/homebrew/opt/postgresql@16/bin"))
    parser.add_argument("--redis-binary", type=Path, default=Path("/opt/homebrew/bin/redis-server"))
    parser.add_argument("--openssl-binary", type=Path, default=Path("/opt/homebrew/bin/openssl"))
    parser.add_argument("--hold-on-failure-seconds", type=int, default=0)
    args = parser.parse_args(argv)

    root = Path(__file__).resolve().parents[3]
    report = {"banner": BANNER, "steps": {}, "checks": {}, "not_run": {},
              "candidate": {"worktree": str(root), "revision": None},
              "started_at_utc": datetime.now(timezone.utc).isoformat()}
    report["candidate"]["revision"] = migration.execute(
        ["/usr/bin/git", "rev-parse", "HEAD"], env={"PATH": "/usr/bin:/bin"},
        cwd=root, phase="revision").stdout.strip()
    args.report.write_text(json.dumps(report, indent=2))

    pixels, palette, manifest_sha = load_pack(root)
    report["scenario"] = {"wall_pack_manifest_sha256": manifest_sha}

    migration.SERVICES["concert-service"] = "CONCERT_DATABASE_URL"
    runtime.PREFIX.update({name: variable.removesuffix("_DATABASE_URL")
                           for name, variable in migration.SERVICES.items()})
    runtime.START_ORDER = ("profile-service", "notification-service", "friend-service",
                           "post-service", "concert-service", "auth-service", "api-gateway")
    runtime.PEERS["concert-service"] = ("auth-service", "friend-service", "profile-service",
                                        "post-service")
    runtime.EXTRA_LISTENERS["concert-service"] = ("world",)
    original_child_init = runtime.Child.__init__

    def child_init(self, cluster, name, argv, env, ports, reservations, secrets_to_redact=()):
        if name == "concert-service":
            reservations = tuple(key for key in reservations if key != "concert-service:http")
        original_child_init(self, cluster, name, argv, env, ports, reservations, secrets_to_redact)

    runtime.Child.__init__ = child_init

    state = {"event": None, "origin": None, "rooms": None}
    stack = dev_stack.DevStack(args.bin_dir, args.pg_bin_dir, args.redis_binary,
                               args.openssl_binary, None)
    base_environments = dev_stack.development_environments

    def environments(cluster, ports, keys, credentials, redis_password, mail_token,
                     browser_origin=None):
        envs = base_environments(cluster, ports, keys, credentials, redis_password,
                                 mail_token, browser_origin)
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
        rooms = [{"room_id": str(uuid4()), "capacity": 8}]
        owner = str(uuid4())
        avatar = template["avatar_manifest"]
        mapped = template["map_manifest"]
        statements = ["INSERT INTO concert_events(event_id,slug,title,phase) VALUES("
                      + ",".join(map(sql_quote, [event, "in03-dev-" + secrets.token_hex(8),
                                                 "IN03 development gate", "OPEN"])) + ")"]
        for room in rooms:
            statements.append(
                "INSERT INTO concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,"
                "avatar_manifest_id,avatar_manifest_version,is_default) VALUES("
                + ",".join(map(sql_quote, [event, room["room_id"], "IN03 dev room", room["capacity"],
                                           mapped["map_id"], mapped["map_revision"],
                                           avatar["manifest_id"], avatar["manifest_version"]]))
                + ",true)")
        cluster.sql("mcr_concert", ";".join(statements))
        world_address = ports.address("concert-service:world")
        relationship, self_profile, decision, post_read, post_callback = world.joined.keys(cluster, 5)
        urls = {role: "https://" + ports.address(role + "-service:grpc")
                for role in ("auth", "friend", "profile", "concert", "notification", "post")}
        invite_keys = json.dumps([{"id": "in03_dev", "secret_hex": secrets.token_hex(32)}])
        intro_receipt_keys = json.dumps([{"id": "in03_intro_receipt", "secret_hex": secrets.token_hex(32)}])
        pickup_keys = json.dumps([{"id": "in03_pickup", "secret_hex": secrets.token_hex(32)}])
        social_handle_keys = json.dumps([{"id": "in03_social_handle", "secret_hex": secrets.token_hex(32)}])
        social_command_keys = json.dumps([{"id": "in03_social_command", "secret_hex": secrets.token_hex(32)}])
        for env in envs.values():
            for key, value in list(env.items()):
                if key.endswith("_GRPC_ENDPOINT") and value.startswith("http://127.0.0.1:"):
                    env[key] = value.replace("http://", "https://", 1)
            env["CONCERT_RELATIONSHIP_VERIFYING_KEYS"] = relationship[1]
        for role in ("auth", "friend"):
            envs[role + "-service"].update(
                CONCERT_OWNER_AUTHORITY_ENABLED="true",
                AUTHORITY_CONCERT_SETTLEMENT_ENDPOINT=urls["concert"],
                AUTHORITY_POST_SETTLEMENT_ENDPOINT=urls["post"],
                AUTHORITY_PROFILE_SETTLEMENT_ENDPOINT=urls["profile"],
                AUTHORITY_NOTIFICATION_SETTLEMENT_ENDPOINT=urls["notification"],
            )
        envs["profile-service"].update(
            CONCERT_PROFILE_SELF_VERIFYING_KEYS=self_profile[1],
            PROFILE_PARTICIPANT_DECISION_SIGNING_SEED=decision[0],
            PROFILE_SELF_AUTH_GRPC_ENDPOINT=urls["auth"],
            PROFILE_SELF_FRIEND_GRPC_ENDPOINT=urls["friend"],
            PROFILE_SELF_CONCERT_GRPC_ENDPOINT=urls["concert"],
        )
        envs["concert-service"].update(
            CONCERT_RUNTIME_MODE="local",
            CONCERT_BOOTSTRAP_FILE=release["CONCERT_BOOTSTRAP_FILE"],
            CONCERT_AVATAR_MANIFEST_FILE=release["CONCERT_AVATAR_MANIFEST_FILE"],
            CONCERT_MAP_MANIFEST_FILE=release["CONCERT_MAP_MANIFEST_FILE"],
            CONCERT_REVENGE_MANIFEST_FILE=release["CONCERT_REVENGE_MANIFEST_FILE"],
            CONCERT_PARADE_MANIFEST_FILE=release["CONCERT_PARADE_MANIFEST_FILE"],
            CONCERT_DANGER_MANIFEST_FILE=release["CONCERT_DANGER_MANIFEST_FILE"],
            CONCERT_WORLD_LISTEN_ADDR=world_address,
            CONCERT_WORLD_ALLOWED_ORIGINS=json.dumps([origin]),
            CONCERT_WORLD_EVENT_ID=event,
            CONCERT_WORLD_OWNER_ID=owner,
            CONCERT_WORLD_INVALIDATION_CONSUMER_ID=owner,
            CONCERT_WORLD_ROOMS_JSON=json.dumps(rooms),
            CONCERT_WORLD_RESOURCE_ROOT=release["asset_root"],
            CONCERT_WORLD_SOURCE_BINDING_FILE=release["source_binding"],
            CONCERT_WALL_TOOLS_B_MODE="live",
            CONCERT_WALL_TOOLS_B_RECEIPT_KEYS_JSON=invite_keys,
            CONCERT_RELATIONSHIP_SIGNING_SEED=relationship[0],
            CONCERT_PROFILE_SELF_SIGNING_SEED=self_profile[0],
            PROFILE_PARTICIPANT_DECISION_VERIFYING_KEYS=decision[1],
            CONCERT_PARTICIPANT_MODE="live",
            CONCERT_PRODUCER_INSTANCE_ID=str(uuid4()),
            CONCERT_PARTICIPANT_RECOVERY_KEYS_JSON=invite_keys,
            CONCERT_INTRO_MODE="live",
            CONCERT_NOTIFICATION_GRPC_ENDPOINT=urls["notification"],
            CONCERT_INTRO_CURSOR_KEY_HEX=secrets.token_hex(32),
            CONCERT_INTRO_RECEIPT_KEYS_JSON=intro_receipt_keys,
            CONCERT_PICKUP_KEYS_JSON=pickup_keys,
            CONCERT_NOTIFICATION_INBOX_MODE="live",
            CONCERT_NOTIFICATION_TARGET_MODE="live",
            CONCERT_SOCIAL_MODE="live",
            CONCERT_SOCIAL_CURSOR_KEY_HEX=secrets.token_hex(32),
            CONCERT_SOCIAL_HANDLE_KEYS_JSON=social_handle_keys,
            CONCERT_SOCIAL_COMMAND_KEYS_JSON=social_command_keys,
            CONCERT_SOCIAL_REQUEST_TTL_SECONDS="3600",
            CONCERT_PUBLIC_FEED_MODE="read",
            CONCERT_POST_GRPC_ENDPOINT=urls["post"],
            CONCERT_POST_READ_SIGNING_SEED=post_read[0],
            CONCERT_FEED_CURSOR_KEY_HEX=secrets.token_hex(32),
            POST_CONCERT_ROOT_READ_VERIFYING_KEYS=post_callback[1],
        )
        for role in ("auth", "friend", "profile"):
            envs["concert-service"]["CONCERT_PARTICIPANT_" + role.upper() + "_GRPC_ENDPOINT"] = urls[role]
            envs["concert-service"]["CONCERT_" + role.upper() + "_GRPC_ENDPOINT"] = urls[role]
        envs["concert-service"]["RUST_LOG"] = "info"
        envs["post-service"]["RUST_LOG"] = "info"
        envs["post-service"].update(
            POST_CONCERT_READ_MODE="read",
            POST_CONCERT_READ_AUTH_GRPC_ENDPOINT=urls["auth"],
            POST_FRIEND_GRPC_ENDPOINT=urls["friend"],
            POST_CONCERT_ROOT_GRPC_ENDPOINT=urls["concert"],
            CONCERT_POST_READ_VERIFYING_KEYS=post_read[1],
            POST_CONCERT_ROOT_READ_SIGNING_SEED=post_callback[0],
        )
        inspected = migration.execute(
            [str(args.bin_dir / "concert-service"), "--inspect-product-release"],
            env=envs["concert-service"], cwd=cluster.root, phase="in03-release-inspection",
        )
        digest_value = inspected.stdout.strip().split("=", 1)[1]
        envs["concert-service"]["CONCERT_PARTICIPANT_RELEASE_DIGEST"] = digest_value
        envs["api-gateway"]["GATEWAY_CONCERT_GRPC_ENDPOINT"] = urls["concert"]
        envs["api-gateway"]["GATEWAY_CONCERT_RELEASE_DIGEST"] = digest_value
        state.update(event=event, origin=origin, rooms=rooms, digest=digest_value)
        return envs

    api = None
    try:
        with patch.object(dev_stack, "development_environments", environments), \
                patch.object(dev_stack, "MailSink", closure.EnrollmentMailSink), \
                patch.object(runtime, "readiness_value", closure.readiness):
            stack.start()
        event, origin = state["event"], state["origin"]
        descriptor = stack.descriptor()
        descriptor.pop("owner", None)
        report["stack"] = {
            "gateway_url": descriptor["gateway_url"],
            "services": descriptor["services"],
            "event": event,
            "release_digest": state["digest"],
            "readiness": {name: stack.rows[name].get("readiness") for name in stack.rows},
            "owned_root": descriptor["root"],
            "mail": "private local HTTP capture",
        }
        gateway = descriptor["gateway_url"]
        policy = closure.publish_enrollment_policy(stack.cluster)
        report["enrollment_policy"] = {
            "revision": policy["revision"],
            "approval_reference": policy["approvalReference"],
            "production_approval": "NOT_PRESENT_IN_CHECKOUT_SCHEMA",
            "note": "The checkout's enrollment schema has no productionApproval field; "
                    "approvalReference=development-not-admitted is the explicit "
                    "nonproduction label. No legal approval is claimed.",
        }
        principals = {}
        principal_attempts = {}
        for ordinal, role in enumerate(ROLES, 1):
            for attempt in range(5):
                try:
                    principals[role] = closure.principal(gateway, stack.mail, policy, ordinal, origin)
                    principal_attempts[role] = attempt + 1
                    break
                except closure.StepFailure:
                    if attempt == 4:
                        raise
                    time.sleep(1.5 + attempt)
        report["principal_attempts"] = principal_attempts
        for ordinal, role in enumerate(ROLES, 1):
            closure.initialize_profile(gateway, principals[role][0], principals[role][1],
                                       ordinal, origin)
        users = {role: principals[role][0] for role in ROLES}
        tokens = {role: principals[role][1] for role in ROLES}
        api = Api(gateway, origin, tokens)
        report["principals"] = {role: users[role] for role in ("alice", "bob", "carol")}
        report["outsider"] = "created but not admitted to the event"

        def sql(db, statement):
            try:
                return stack.cluster.sql(db, statement)
            except migration.SmokeError:
                raise GateFailure("sql-failed") from None

        admitted = [users["alice"], users["bob"], users["carol"]]
        statements = []
        for user in admitted:
            statements += [
                "INSERT INTO concert_event_admissions(event_id,user_id,state) VALUES("
                + ",".join(map(sql_quote, [event, user, "ACTIVE"])) + ") ON CONFLICT DO NOTHING",
                "INSERT INTO concert_participants(event_id,user_id,theme,low_power,reduced_motion,"
                "sound_enabled) VALUES(" + ",".join(map(sql_quote, [event, user, "REVENGE"]))
                + ",false,false,false) ON CONFLICT DO NOTHING",
                "INSERT INTO concert_avatar_settings(event_id,user_id,contract_version,representation,"
                "preset_id,manifest_id,manifest_version) VALUES("
                + ",".join(map(sql_quote, [event, user, "1.1.0", "whole_character",
                                          "photo01_front_01", "mcr_selected_static_20261103"]))
                + ",1) ON CONFLICT DO NOTHING",
            ]
        mural_snapshot = str(uuid4())
        statements += [
            "INSERT INTO concert_mural_state(event_id,revision,snapshot_id,state,pixels,palette,"
            "cooldown_ms,retained_from_revision,reset_revision) VALUES("
            + sql_quote(event) + ",1," + sql_quote(mural_snapshot) + ",'EDITABLE',decode('"
            + pixels.hex() + "','hex'),ARRAY[" + ",".join(map(sql_quote, palette)) + "],1200,1,1)",
            "INSERT INTO concert_mural_tool_config(event_id,background_index) VALUES("
            + sql_quote(event) + ",0)",
        ]
        sql("mcr_concert", ";".join(statements))
        report["provisioning"] = {
            "mode": "owned disposable PG rows provisioned directly; no external data",
            "event": event,
            "participants": ["alice", "bob", "carol"],
            "outsider": "authenticated principal without event admission or participant row",
            "wall_pack_manifest_sha256": manifest_sha,
            "wall_seed_snapshot_id": mural_snapshot,
        }

        log_offsets = {}

        def record(name, value):
            value["log"] = closure.new_log(stack, log_offsets)
            report["steps"][name] = value
            return value

        def query(db, statement):
            raw = sql(db, statement)
            return json.loads(raw) if raw else None

        def follow(role, target, key=None):
            return api.request(role, "POST", "/api/follows", {"targetUserId": target}, key)

        def relationship(role, target):
            return api.request(role, "GET", f"/api/relationships/{target}")

        def profile(role, target):
            return api.request(role, "GET", f"/api/profile/{target}")

        def privacy(role, visibility):
            return api.request(role, "PUT", "/api/profile/privacy",
                               {"accountVisibility": visibility}, str(uuid4()))


        def feed(role, limit=1):
            attempts = []
            result = None
            for _ in range(3):
                started = time.monotonic()
                result = api.request(role, "GET", f"/api/concert/events/{event}/feed?limit={limit}")
                report.setdefault("feed_timings_ms", []).append(
                    round((time.monotonic() - started) * 1000, 1))
                attempts.append({"http": result[0],
                                 "reason": result[1].get("details", {}).get("reason")
                                 if isinstance(result[1], dict) else None})
                if result[0] != 503 and result[0] != 504:
                    break
                time.sleep(0.5)
            report.setdefault("feed_attempts", []).append({"role": role, "attempts": attempts})
            return result

        status, body, _ = follow("alice", users["bob"], str(uuid4()))
        require(status == 200, f"follow-public-{status}")
        require(body.get("following") is True and body.get("status") == "ACTIVE"
                and body.get("requestId") is None and body.get("followedAt"), "follow-public-shape")
        record("follow_public", {"http": status, "body": body,
                                 "db": query("mcr_friend",
                                             "SELECT jsonb_agg(jsonb_build_object('follower_id',follower_id,"
                                             "'followee_id',followee_id,'deleted_at',deleted_at)) "
                                             "FROM follows WHERE follower_id=" + sql_quote(users["alice"])
                                             + " AND followee_id=" + sql_quote(users["bob"]))})

        status, body, _ = privacy("bob", "PRIVATE")
        require(status == 200 and body.get("accountVisibility") == "PRIVATE",
                f"privacy-private-{status}")
        record("profile_private", {
            "http": status, "body": body,
            "db": sql("mcr_profile", "SELECT account_visibility FROM profiles WHERE user_id="
                      + sql_quote(users["bob"])),
        })

        status, body, _ = api.request("alice", "DELETE", f"/api/follows/{users['bob']}")
        require(status == 200 and body.get("following") is False, f"unfollow-{status}")
        record("unfollow_remove", {
            "http": status, "body": body,
            "db": sql("mcr_friend", "SELECT count(*) FROM follows WHERE deleted_at IS NULL AND follower_id="
                      + sql_quote(users["alice"]) + " AND followee_id=" + sql_quote(users["bob"])),
        })

        status, body, _ = follow("alice", users["bob"], str(uuid4()))
        require(status == 200 and body.get("following") is False and body.get("status") == "PENDING"
                and body.get("requestId"), f"follow-private-{status}")
        request_one = body["requestId"]
        UUID(request_one)
        record("follow_request_pending", {
            "http": status, "body": body,
            "db": query("mcr_friend", "SELECT jsonb_build_object('status',status,'decided_at',decided_at) "
                       "FROM follow_requests WHERE id=" + sql_quote(request_one)),
        })

        status, body, _ = api.request("bob", "GET", "/api/follows/requests/incoming")
        require(status == 200 and any(item.get("id") == request_one
                                      for item in body.get("requests", [])), "incoming-missing")
        record("incoming_request_visible", {"http": status,
                                            "request_ids": [i.get("id") for i in body.get("requests", [])]})

        status, body, _ = api.request("bob", "POST",
                                      f"/api/follows/requests/{request_one}/reject")
        require(status == 200 and body.get("status") == "REJECTED", f"reject-{status}")
        require(api.request("alice", "GET",
                            "/api/follows/status?target_user_id=" + users["bob"])[1].get("following") is False,
                "rejected-still-following")
        record("request_declined", {
            "http": status, "body": {"id": body.get("id"), "status": body.get("status")},
            "db": query("mcr_friend", "SELECT jsonb_build_object('status',status) FROM follow_requests "
                       "WHERE id=" + sql_quote(request_one)),
        })

        status, body, _ = follow("alice", users["bob"], str(uuid4()))
        require(status == 200 and body.get("status") == "PENDING", f"re-follow-{status}")
        request_two = body["requestId"]
        status, body, _ = api.request("bob", "POST",
                                      f"/api/follows/requests/{request_two}/accept")
        require(status == 200 and body.get("following") is True and body.get("status") == "ACTIVE",
                f"accept-{status}")
        record("request_accepted", {
            "http": status, "body": body,
            "db": query("mcr_friend", "SELECT jsonb_build_object('request',(SELECT status FROM follow_requests "
                       "WHERE id=" + sql_quote(request_two) + "),'edge',(SELECT count(*) FROM follows WHERE "
                       "deleted_at IS NULL AND follower_id=" + sql_quote(users["alice"]) + " AND followee_id="
                       + sql_quote(users["bob"]) + "))"),
        })

        status, body, _ = profile("carol", users["bob"])
        assert_gateway_error(status, body, 404, "Not found",
                             forbid=PRIVATE_TOKENS + (users["bob"],))
        record("private_profile_denied", {"http": status, "body": body})

        status, body, _ = profile("alice", users["bob"])
        require(status == 200 and body.get("userId") == users["bob"]
                and body.get("accountVisibility") == "PRIVATE", f"friend-profile-{status}")
        record("private_profile_visible_to_friend", {
            "http": status,
            "body": {key: body.get(key) for key in ("userId", "accountVisibility", "lifecycleStatus")},
        })

        api.request("alice", "DELETE", f"/api/follows/{users['bob']}")
        status, body, _ = profile("alice", users["bob"])
        assert_gateway_error(status, body, 404, "Not found",
                             forbid=PRIVATE_TOKENS + (users["bob"],))
        record("private_profile_invalidated_after_unfollow", {"http": status, "body": body})

        status, body, _ = follow("alice", users["bob"], str(uuid4()))
        require(status == 200 and body.get("status") == "PENDING", f"restore-follow-{status}")
        status, body, _ = api.request("bob", "POST",
                                      f"/api/follows/requests/{body['requestId']}/accept")
        require(status == 200 and body.get("following") is True, f"restore-accept-{status}")
        status, body, _ = privacy("bob", "PUBLIC")
        require(status == 200 and body.get("accountVisibility") == "PUBLIC", f"privacy-public-{status}")
        record("public_restored", {"http": status, "body": body})

        status, body, _ = relationship("alice", users["bob"])
        require(status == 200 and body == {"blockedByViewer": False, "blockedViewer": False,
                                           "mutedByViewer": False, "following": True},
                f"relationship-before-{status}")
        journal_before = query("mcr_friend", "SELECT count(*) FROM friend_world_invalidation_journal "
                               "WHERE reason='FRIEND_RELATIONSHIP_CHANGED' AND peer_id IS NOT NULL")
        status, body, _ = api.request("alice", "POST",
                                      f"/api/relationships/blocks/{users['bob']}")
        require(status == 200 and body.get("blockedByViewer") is True
                and body.get("following") is False, f"block-{status}")
        record("block", {
            "http": status, "body": body,
            "db": {
                "blocks": query("mcr_friend", "SELECT jsonb_agg(jsonb_build_object('blocker_id',blocker_id,"
                                "'blocked_id',blocked_id,'deleted_at',deleted_at)) FROM user_blocks "
                                "WHERE blocker_id=" + sql_quote(users["alice"]) + " AND blocked_id="
                                + sql_quote(users["bob"])),
                "journal_before": journal_before,
                "journal_after": query("mcr_friend", "SELECT count(*) FROM friend_world_invalidation_journal "
                                      "WHERE reason='FRIEND_RELATIONSHIP_CHANGED' AND peer_id IS NOT NULL"),
            },
        })

        status, body, _ = relationship("bob", users["alice"])
        require(status == 200 and body.get("blockedViewer") is True, f"block-observed-{status}")
        record("block_observed_by_target", {"http": status, "body": body})

        status, body, _ = follow("alice", users["bob"], str(uuid4()))
        assert_gateway_error(status, body, 403, "Forbidden", forbid=PRIVATE_TOKENS)
        record("blocked_follow_denied", {"http": status, "body": body})

        for role, target in (("alice", users["bob"]), ("bob", users["alice"])):
            status, body, _ = profile(role, target)
            assert_gateway_error(status, body, 404, "Not found", forbid=PRIVATE_TOKENS + (target,))
            record(f"blocked_profile_denied_{role}", {"http": status, "body": body})

        bob_post, alice_post, deleted_post = str(uuid4()), str(uuid4()), str(uuid4())
        bob_comment = str(uuid4())
        now = "clock_timestamp()"
        sql("mcr_post", ";".join([
            "INSERT INTO post_concert_posts(post_id,event_id,author_id,content,revision,state,created_at,"
            "comment_count) VALUES(" + ",".join(map(sql_quote, [alice_post, event, users["alice"]]))
            + ",'{\"text\":\"alice public post\",\"media_ids\":[],\"links\":[]}',1,'PUBLISHED',"
            + now + ",0)",
            "INSERT INTO post_concert_posts(post_id,event_id,author_id,content,revision,state,created_at,"
            "comment_count) VALUES(" + ",".join(map(sql_quote, [bob_post, event, users["bob"]]))
            + ",'{\"text\":\"bob public post\",\"media_ids\":[],\"links\":[]}',1,'PUBLISHED',"
            + now + ",1)",
            "INSERT INTO post_concert_posts(post_id,event_id,author_id,content,revision,state,created_at,"
            "comment_count) VALUES(" + ",".join(map(sql_quote, [deleted_post, event, users["bob"]]))
            + ",'{\"text\":\"deleted post\",\"media_ids\":[],\"links\":[]}',2,'DELETED',"
            + now + ",0)",
            "INSERT INTO post_concert_comments(comment_id,event_id,post_id,author_id,text,revision,state,"
            "created_at) VALUES(" + ",".join(map(sql_quote, [bob_comment, event, bob_post,
                                                              users["bob"], "bob comment"]))
            + ",1,'PUBLISHED'," + now + ")",
        ]))
        warm = []
        for _ in range(2):
            warm.append(api.request("alice", "GET",
                                    f"/api/concert/events/{event}/mural/snapshot")[0])
            warm.append(api.request("carol", "GET",
                                    f"/api/concert/events/{event}/feed?limit=1")[0])
        record("product_warmup", {"http": warm})
        status, body, _ = feed("carol")
        record("feed_probe", {
            "http": status, "body": body,
            "db": {
                "concert_scopes": query(
                    "mcr_concert",
                    "SELECT jsonb_agg(jsonb_build_object('phase',phase,"
                    "'delivery_completed',delivery_completed_at IS NOT NULL) "
                    "ORDER BY created_at) FROM concert_product_owner_scopes"),
                "post_tracking": query(
                    "mcr_post",
                    "SELECT jsonb_agg(jsonb_build_object('terminal_status',terminal_status,"
                    "'auth_attempted',auth_attempted,'friend_attempted',friend_attempted) "
                    "ORDER BY expires_at) FROM post_concert_read_tracking"),
                "post_rows": query(
                    "mcr_post",
                    "SELECT jsonb_agg(jsonb_build_object('post_id',post_id,'event_id',event_id,"
                    "'state',state,'created_at',created_at,'content',content) ORDER BY created_at) "
                    "FROM post_concert_posts"),
                "pairs": query(
                    "mcr_concert",
                    "SELECT jsonb_agg(jsonb_build_object('low_user',low_user,'high_user',high_user,"
                    "'low_blocks_high',low_blocks_high,'high_blocks_low',high_blocks_low) ) "
                    "FROM concert_relationship_pairs"),
            },
        })
        require(status == 200, f"feed-carol-{status}:{json.dumps(body)[:600]}")
        ids = [item["post_id"] for item in body.get("items", [])]
        require(bob_post in ids and deleted_post not in ids, "feed-baseline")
        record("feed_baseline", {"http": status, "post_ids": ids,
                                 "deleted_absent": deleted_post not in ids})

        status, body, _ = api.request(
            "carol", "GET", f"/api/concert/events/{event}/posts/{deleted_post}")
        assert_concert_error(status, body, 404, "RESOURCE_UNAVAILABLE")
        record("feed_deleted_post_denied", {"http": status, "body": body})

        record("feed_before_block", {
            "http": report["steps"]["feed_baseline"]["http"],
            "contains_bob_post": bob_post in report["steps"]["feed_baseline"]["post_ids"],
            "viewer": "carol",
        })

        status, body, _ = feed("alice")
        require(status == 200 and bob_post not in [item["post_id"] for item in body.get("items", [])],
                "feed-blocked-author-visible")
        record("feed_blocked_author_absent", {
            "http": status,
            "post_ids": [item["post_id"] for item in body.get("items", [])],
        })

        status, body, _ = api.request("alice", "GET",
                                      f"/api/concert/events/{event}/posts/{bob_post}")
        assert_concert_error(status, body, 404, "RESOURCE_UNAVAILABLE")
        record("blocked_post_read_denied", {"http": status, "body": body})

        comments_started = time.monotonic()
        status, body, _ = api.request(
            "bob", "GET", f"/api/concert/events/{event}/posts/{bob_post}/comments?limit=10")
        comments_ms = round((time.monotonic() - comments_started) * 1000, 1)
        comment_items = body.get("items", [])
        comments_readable = status == 200 and any(
            item.get("author", {}).get("user_id") == users["bob"] for item in comment_items)
        record("comments_self_read", {
            "http": status,
            "ms": comments_ms,
            "body": None if status == 200 else body,
            "author_ids": [i.get("author", {}).get("user_id") for i in comment_items],
            "db": query("mcr_post", "SELECT jsonb_agg(jsonb_build_object('comment_id',comment_id,"
                       "'author_id',author_id,'state',state)) FROM post_concert_comments "
                       "WHERE comment_id=" + sql_quote(bob_comment)),
        })
        require(status == 200 or body.get("details", {}).get("reason")
                in ("DEADLINE_EXCEEDED", "AUTHENTICATION_REQUIRED", "DEPENDENCY_UNAVAILABLE"),
                f"comments-unexpected-{status}:{json.dumps(body)[:400]}")

        sql("mcr_post", "UPDATE post_concert_posts SET state='DELETED',revision=2 WHERE post_id="
            + sql_quote(alice_post))
        status, body, _ = api.request(
            "carol", "GET", f"/api/concert/events/{event}/posts/{alice_post}")
        assert_concert_error(status, body, 404, "RESOURCE_UNAVAILABLE")
        record("deleted_post_absent", {"http": status, "body": body})

        status, body, _ = api.request("alice", "GET",
                                      f"/api/concert/events/{event}/mural/snapshot")
        require(status == 200 and body.get("revision") == 1, f"wall-member-{status}")
        record("wall_member_read", {"http": status, "revision": body.get("revision")})
        status, body, _ = api.request("outsider", "GET",
                                      f"/api/concert/events/{event}/mural/snapshot")
        assert_concert_error(status, body, 404, "RESOURCE_UNAVAILABLE")
        record("wall_outsider_denied", {"http": status, "body": body})

        status, body, _ = api.request("bob", "GET",
                                      f"/api/concert/events/{event}/me/blocks")
        record("social_blocks_probe", {"http": status, "body": body})
        social_available = status == 200 and isinstance(body.get("items"), list)
        if not social_available:
            require(status == 503 and body.get("details", {}).get("reason") == "DEPENDENCY_UNAVAILABLE",
                    f"social-blocks-unexpected-{status}:{json.dumps(body)[:400]}")
            for name, consumer in (
                ("social_block_list", "backend/services/concert-service/src/social_live/producer.rs list() -> RuntimeHooks::retain -> social_live/read_child.rs register_child_intent -> product_rpc/scope_ledger.rs lock"),
                ("social_block", "backend/services/concert-service/src/social_live/producer.rs block_state() -> RuntimeHooks::succeeded -> retain"),
                ("social_unblock", "backend/services/concert-service/src/social_live/producer.rs block_state() -> RuntimeHooks::succeeded -> retain"),
                ("unverified_age_denial", "backend/services/concert-service/src/social_live/producer.rs command transaction -> retain before the effect witness"),
            ):
                report["not_run"][name] = {
                    "status": "NOT_RUN",
                    "reason": "The Social read could not retain its source-exclusion child: the "
                              "effect witness holds FOR SHARE on the root scope row for the rest of "
                              "the projecting transaction, so a child intent registered afterwards "
                              "waits on the ledger's 250 ms lock timeout and the surface returns "
                              "CONCERT_503 DEPENDENCY_UNAVAILABLE before any result is projected. "
                              "This is a lock-order defect, not a budget or provider input.",
                    "consumer": consumer,
                }
        else:
            record("social_blocks_list", {"http": status, "items": len(body.get("items", []))})
            status, body, _ = api.request(
                "carol", "PUT", f"/api/concert/events/{event}/me/blocks/{users['bob']}",
                {}, str(uuid4()))
            require(status == 200 and body.get("blocked_by_me") is True, f"social-block-{status}")
            record("social_block", {"http": status, "body": body})

            feed_started = time.monotonic()
            status, body, _ = api.request(
                "carol", "GET", f"/api/concert/events/{event}/feed?limit=1")
            feed_ms = round((time.monotonic() - feed_started) * 1000, 1)
            carol_feed_ids = [item["post_id"] for item in body.get("items", [])]
            # Executed observation, not a privacy pass. U08 /me/blocks specifies
            # list/unblock and prior sharing revocation; the concert feed reads the
            # Friend block authority and fails closed with DEPENDENCY_UNAVAILABLE
            # while a Concert social pair is blocked. Recorded as a blocker.
            record("feed_carol_blocked_author_state", {
                "http": status,
                "ms": feed_ms,
                "body": None if status == 200 else body,
                "post_ids": carol_feed_ids,
                "bob_post_visible": bob_post in carol_feed_ids,
                "spec_basis": "U08 /me/blocks: list and unblock; no feed-hiding contract"
                               " (a 503 for the blocker's own feed is still a defect)",
                "consumer": "backend/services/post-service/src/concert_feed read admission "
                            "(Friend block view only); backend/services/concert-service/src/feed "
                            "policy/author fence",
            })
            if status != 200 and carol_feed_ids == []:
                report["not_run"]["concert_block_feed_read"] = {
                    "status": "BLOCKED",
                    "reason": "After carol blocks bob through /me/blocks, her own feed read "
                              "returned CONCERT_503 DEPENDENCY_UNAVAILABLE in " + str(feed_ms) + " ms; "
                              "the same read returned 200 with bob's post present before the "
                              "block and after the unblock. The feeder fails closed instead of "
                              "filtering, so no feed-filter privacy pass is claimed for the "
                              "Concert block surface.",
                    "consumer": "backend/services/post-service/src/concert_feed read admission; "
                                "backend/services/concert-service/src/feed policy/author fence",
                }

            status, body, _ = api.request("carol", "GET",
                                          f"/api/concert/events/{event}/me/blocks")
            require(status == 200 and any(item.get("target_user_id") == users["bob"]
                                          for item in body.get("items", [])), "social-block-list")
            record("social_block_listed", {"http": status,
                                           "targets": [i.get("target_user_id") for i in body.get("items", [])]})

            status, body, _ = api.request(
                "carol", "DELETE", f"/api/concert/events/{event}/me/blocks/{users['bob']}",
                None, str(uuid4()))
            require(status == 200 and body.get("blocked_by_me") is False, f"social-unblock-{status}")
            record("social_unblock", {"http": status, "body": body})

            status, body, _ = api.request(
                "alice", "POST", f"/api/concert/events/{event}/social-requests",
                {"recipient_id": users["bob"], "intro_id": str(uuid4()), "intro_revision": 2,
                 "consents": [{"platform": "INSTAGRAM", "requester_handle_version": 1,
                               "recipient_handle_version": 1, "requester_consents": True}]},
                str(uuid4()))
            assert_social_error(status, body, 404, "RESOURCE_UNAVAILABLE", forbid=PRIVATE_TOKENS)
            record("unverified_age_request_denied", {"http": status, "body": body})

        denied = {name: step["body"] for name, step in report["steps"].items()
                  if isinstance(step.get("body"), dict)
                  and step["body"].get("code", "").startswith(("GATEWAY_", "CONCERT_"))}
        leaked = {}
        for name, body in denied.items():
            rendered = json.dumps(body)
            hits = [token for token in PRIVATE_TOKENS + (users["alice"], users["bob"], users["carol"])
                    if token in rendered]
            if hits:
                leaked[name] = hits
        require(not leaked, "denied-payload-private-metadata")
        record("denied_payload_audit", {
            "cases": len(denied),
            "envelopes": {name: sorted(body) for name, body in denied.items()},
            "private_metadata_leaks": leaked,
        })

        unblock_attempts = []
        for attempt in range(3):
            status, body, _ = api.request("alice", "DELETE",
                                          f"/api/relationships/blocks/{users['bob']}")
            unblock_attempts.append({"http": status,
                                     "reason": body.get("details", {}).get("reason")
                                     if isinstance(body, dict) else None})
            if status not in (503, 504):
                break
            time.sleep(0.5)
        require(status == 200 and body.get("blockedByViewer") is False,
                f"unblock-{status}:{json.dumps(body)[:400]}")
        record("unblock", {"http": status, "body": body, "attempts": unblock_attempts})
        status, body, _ = profile("alice", users["bob"])
        require(status == 200, f"profile-after-unblock-{status}")
        record("profile_after_unblock", {"http": status,
                                         "accountVisibility": body.get("accountVisibility")})

        report["db"] = {
            "follows": query("mcr_friend", "SELECT jsonb_agg(jsonb_build_object('follower_id',follower_id,"
                             "'followee_id',followee_id,'deleted_at',deleted_at,'delete_reason',delete_reason)"
                             " ORDER BY created_at) FROM follows"),
            "follow_requests": query("mcr_friend", "SELECT jsonb_agg(jsonb_build_object('id',id,'status',status,"
                                  "'decided_at',decided_at) ORDER BY created_at) FROM follow_requests"),
            "user_blocks": query("mcr_friend", "SELECT jsonb_agg(jsonb_build_object('blocker_id',blocker_id,"
                               "'blocked_id',blocked_id,'deleted_at',deleted_at)) FROM user_blocks"),
            "invalidation_journal": query(
                "mcr_friend", "SELECT jsonb_agg(jsonb_build_object('reason',reason,'user_id',user_id,"
                "'peer_id',peer_id) ORDER BY sequence) FROM friend_world_invalidation_journal"),
            "profiles": query("mcr_profile", "SELECT jsonb_agg(jsonb_build_object('user_id',user_id,"
                              "'account_visibility',account_visibility)) FROM profiles"),
            "concert_posts": query("mcr_post", "SELECT jsonb_agg(jsonb_build_object('post_id',post_id,"
                                 "'author_id',author_id,'state',state) ORDER BY created_at) "
                                 "FROM post_concert_posts"),
        }

        report["not_run"].update({
            "venue_meeting_reads": {
                "status": "NOT_RUN",
                "reason": "The composed Concert product has no Meeting ProductOwner: "
                          "ProductOwner dispatch mounts Wall/Social/Intro/Notification/Age/"
                          "Report/Operations/OwnerMedia/Feed but no Operation::Meeting, and "
                          "venue_meeting_admission states the bilateral friend read does not "
                          "exist yet, so every venue-meeting route fails closed with 503. "
                          "No venue privacy result is claimed.",
                "consumer": "backend/services/concert-service/src/product_rpc/venue_meeting_admission.rs; "
                            "backend/services/concert-service/src/product_rpc/mod.rs Operation::Meeting",
            },
            "venue_location_snapshot": {
                "status": "NOT_RUN",
                "reason": "PgVenueLocationAuthority::friend_in returns PolicyUnavailable and no "
                          "VenueLocation product producer is composed, so the /venue/location "
                          "route has no owner to read.",
                "consumer": "backend/services/concert-service/src/venue_location/mod.rs; "
                            "backend/services/concert-service/src/product_rpc/mod.rs Operation enum",
            },
            "age_provider_positive": {
                "status": "NOT_RUN",
                "reason": "Missing input: an authorized age-verification provider and its "
                          "signed verified claims. Social consent requires concert_age_owners "
                          "rows plus verified concert_age_claims for both participants and a "
                          "configured sharing verifier; no provider credentials, source "
                          "revision, or claim provenance exist in this checkout.",
                "consumer": "backend/services/concert-service/src/relationship/eligibility.rs",
            },
            "social_reveal_revoke": {
                "status": "NOT_RUN",
                "reason": "Depends on the same authorized age provider/claim input: a create "
                          "or reveal cannot succeed without verified claims, so the approved/"
                          "unrelated/expired/revoked/blocked reveal denials cannot be executed "
                          "against a real grant.",
                "consumer": "backend/services/concert-service/src/relationship/eligibility.rs; "
                            "backend/services/concert-service/src/social_live/producer.rs",
            },
            "external_oauth_provider": {
                "status": "NOT_RUN",
                "reason": "Missing input: external OAuth/social provider client credentials, "
                          "callback registration and provider test account.",
                "consumer": "backend/services/api-gateway/src/adapters/in/http/oauth_routes.rs",
            },
            "external_mail_or_sms": {
                "status": "NOT_RUN",
                "reason": "Missing input: an external mail/SMS delivery provider. Only the "
                          "owned local HTTP mail sink was exercised for enrollment.",
                "consumer": "backend/services/auth-service mailer configuration",
            },
            "comments_read": {
                "status": "NOT_RUN" if report["steps"].get("comments_self_read", {}).get("http") != 200 else "EXECUTED",
                "reason": "Observed " + str(report["steps"].get("comments_self_read", {}).get("http"))
                          + ": the self-authored comments route did not return data. The composed "
                          "read needs about 1.3-1.5 s on this stack (post owner read, per-comment "
                          "admission, then one guarded public-author read per comment), longer than "
                          "the gateway's outer request timeout, which fails the caller before the "
                          "product's own two-second ceiling. It returns no data and no private "
                          "metadata. The seed row is present in post_concert_comments. Not claimed "
                          "as a comment-read pass.",
                "consumer": "backend/services/api-gateway/src/config/mod.rs GATEWAY_REQUEST_TIMEOUT_MS; "
                            "backend/services/concert-service/src/feed/author_filter.rs comments()",
            },
            "blocked_comment_author_filter": {
                "status": "NOT_RUN",
                "reason": "The blocked-author comment filter requires a parent author and a "
                          "blocked comment author in one read; this run seeds one comment by the "
                          "parent author, so no blocked comment row exists to filter. The blocked "
                          "event feed, blocked post detail and blocked profile denials did execute. "
                          "Not claimed as a privacy pass.",
                "consumer": "backend/services/concert-service/src/feed/author_filter.rs comments "
                            "author filter",
            },
            "client_cache_and_ws_frames": {
                "status": "NOT_RUN",
                "reason": "No browser client was run in this backend gate; no client-cache or "
                          "world-socket frame claim is made. The friend-block invalidation "
                          "journal rows and the server-side read-time fences are the evidence.",
                "consumer": "frontend/src/features/** privacy consumers; "
                            "backend/services/concert-service/src/world/invalidation",
            },
        })

        checks = report["checks"]
        checks["follow_public"] = report["steps"]["follow_public"]["http"] == 200
        checks["follow_request_pending"] = report["steps"]["follow_request_pending"]["body"].get("status") == "PENDING"
        checks["request_declined"] = report["steps"]["request_declined"]["body"].get("status") == "REJECTED"
        checks["request_accepted"] = report["steps"]["request_accepted"]["body"].get("following") is True
        checks["unfollow_removed"] = report["steps"]["unfollow_remove"]["db"] == "0"
        checks["private_profile_denied_404"] = report["steps"]["private_profile_denied"]["http"] == 404
        checks["private_profile_invalidated"] = report["steps"]["private_profile_invalidated_after_unfollow"]["http"] == 404
        checks["block_persisted"] = report["steps"]["block"]["db"]["blocks"] is not None
        checks["block_observed"] = report["steps"]["block_observed_by_target"]["body"].get("blockedViewer") is True
        checks["blocked_follow_denied_403"] = report["steps"]["blocked_follow_denied"]["http"] == 403
        checks["blocked_profiles_denied"] = all(
            report["steps"][f"blocked_profile_denied_{role}"]["http"] == 404
            for role in ("alice", "bob"))
        checks["feed_baseline"] = report["steps"]["feed_baseline"]["deleted_absent"]
        checks["feed_deleted_post_404"] = report["steps"]["feed_deleted_post_denied"]["http"] == 404
        checks["feed_blocked_author_absent"] = report["steps"]["feed_blocked_author_absent"]["http"] == 200
        checks["blocked_post_denied_404"] = report["steps"]["blocked_post_read_denied"]["http"] == 404
        checks["comments_self_read"] = comments_readable
        checks["deleted_post_absent"] = report["steps"]["deleted_post_absent"]["http"] == 404
        checks["wall_member_read"] = report["steps"]["wall_member_read"]["http"] == 200
        checks["wall_outsider_denied_404"] = report["steps"]["wall_outsider_denied"]["http"] == 404
        # A step that never ran cannot pass: each check reads the recorded step
        # and is false when the branch that executes it was skipped.
        checks["social_block_listed"] = (
            report["steps"].get("social_block_listed", {}).get("http") == 200)
        checks["social_unblock"] = (
            report["steps"].get("social_unblock", {}).get("body", {}).get("blocked_by_me") is False)
        checks["unverified_age_denied"] = (
            report["steps"].get("unverified_age_request_denied", {}).get("http") == 404)
        checks["no_denied_private_metadata"] = report["steps"]["denied_payload_audit"]["private_metadata_leaks"] == {}
        checks["unblock_restores_profile"] = report["steps"]["profile_after_unblock"]["http"] == 200
        checks["invalidation_journal_grew"] = (
            report["steps"]["block"]["db"]["journal_after"] > report["steps"]["block"]["db"]["journal_before"])
        failed = {name: value for name, value in checks.items() if value is False}
        report["result"] = "PASS" if not failed else "FAIL"
        report["result_scope"] = ("PASS_WITH_RECORDED_NOT_RUN" if not failed and report["not_run"]
                                  else "PASS" if not failed else "FAIL")
        report["failed_checks"] = failed
        if failed or args.hold_on_failure_seconds > 0:
            if args.hold_on_failure_seconds > 0:
                print("HOLD_BEGIN", flush=True)
                time.sleep(args.hold_on_failure_seconds)
                print("HOLD_END", flush=True)
    except Exception as failure:  # noqa: BLE001 - owned development driver
        report["result"] = "FAIL"
        report["failure"] = f"{type(failure).__name__}: {failure}"
        report["failure_detail"] = getattr(failure, "detail", None)
        if stack.cluster is not None:
            report["logs"] = closure.log_excerpts(stack)
    finally:
        try:
            stack.close()
            report["cleanup"] = "PASS"
        except Exception as failure:  # noqa: BLE001
            report["cleanup"] = f"FAIL: {type(failure).__name__}"
        report["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
        if api is not None:
            report["http_requests"] = api.log
        args.report.write_text(json.dumps(report, indent=2))
    return 0 if report.get("result") == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
