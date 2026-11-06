#!/usr/bin/env python3
"""Focused real-stack probe for a blocked author on the reader's own feed.

Development stack only; NOT release qualification, QA08 NOT_RUN. It composes the
same seven native services, owned private PG16 cluster, real mTLS and real
enrollment HTTP used by the IN03 social gate, then drives only:

  carol feed -> 200 with bob's post
  carol blocks bob through /api/concert/events/{event}/me/blocks/{bob}
  carol feed -> 200 with bob's post absent (and the pair row recorded)
  carol unblocks bob
  carol feed -> 200 with bob's post present
  profile owner stopped
  carol feed -> 503, so a real owner outage still fails closed

The report records every status, body and durable row actually observed, plus
the gateway request timeout and the SHA-256 of every binary it exercised.

On a base without the Social retain-order fix, the block and unblock HTTP
projections return 503 while still committing the pair row. This probe therefore
requires the durable pair transition, not the projection status, before reading
the feed; the feed itself is the subject under test.
"""
from __future__ import annotations

import argparse
import hashlib
import http.client
import json
import secrets
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from unittest.mock import patch
from uuid import uuid4

sys.path.insert(0, str(Path(__file__).resolve().parent))
import closure_vertical_stack as closure  # noqa: E402
import dev_stack  # noqa: E402
import migration_smoke as migration  # noqa: E402
import runtime_smoke as runtime  # noqa: E402
import world_in01_live as world  # noqa: E402
import world_in01_prepare as prep  # noqa: E402

ROLES = ("alice", "bob", "carol")
BINARIES = ("api-gateway", "auth-service", "concert-service", "friend-service",
            "notification-service", "post-service", "profile-service")


def file_digest(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


class ProbeFailure(RuntimeError):
    pass


def require(condition, label):
    if not condition:
        raise ProbeFailure(label)


def sql_quote(value):
    return prep.quote(value)


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
        connection = http.client.HTTPConnection(self.host, self.port, timeout=30)
        try:
            connection.request(method, path, body=raw, headers=hdrs)
            response = connection.getresponse()
            payload = response.read()
            try:
                parsed = json.loads(payload or b"{}")
            except ValueError:
                parsed = {"raw": payload[:400].decode("utf-8", "replace")}
            return response.status, parsed if parsed is not None else {}, dict(response.getheaders())
        finally:
            connection.close()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--pg-bin-dir", type=Path,
                        default=Path("/opt/homebrew/opt/postgresql@16/bin"))
    parser.add_argument("--redis-binary", type=Path,
                        default=Path("/opt/homebrew/bin/redis-server"))
    parser.add_argument("--openssl-binary", type=Path,
                        default=Path("/opt/homebrew/bin/openssl"))
    parser.add_argument("--gateway-timeout-ms", type=int, default=5000,
                        help="gateway outer request timeout; the 1 s default "
                             "flakes on multi-author reads")
    args = parser.parse_args(argv)

    root = Path(__file__).resolve().parents[3]
    report = {
        "scope": "development stack blocked-author feed probe",
        "release": "development stack, NOT release qualification",
        "qa08": "NOT_RUN",
        "production": "NOT_CLAIMED",
        "candidate": {
            "worktree": str(root),
            "revision": migration.execute(
                ["/usr/bin/git", "rev-parse", "HEAD"],
                env={"PATH": "/usr/bin:/bin"}, cwd=root, phase="revision").stdout.strip(),
            "script_sha256": file_digest(Path(__file__)),
            "gateway_request_timeout_ms": args.gateway_timeout_ms,
            "binaries": {
                name: {"sha256": file_digest(args.bin_dir / name),
                       "bytes": (args.bin_dir / name).stat().st_size}
                for name in BINARIES
            },
        },
        "started_at_utc": datetime.now(timezone.utc).isoformat(),
        "steps": {},
        "checks": {},
    }

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
        original_child_init(self, cluster, name, argv, env, ports, reservations,
                            secrets_to_redact)

    runtime.Child.__init__ = child_init

    state = {"event": None, "origin": None, "digest": None}
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
                      + ",".join(map(sql_quote, [event, "blocked-feed-" + secrets.token_hex(8),
                                                 "Blocked feed probe", "OPEN"])) + ")"]
        for room in rooms:
            statements.append(
                "INSERT INTO concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,"
                "avatar_manifest_id,avatar_manifest_version,is_default) VALUES("
                + ",".join(map(sql_quote, [event, room["room_id"], "Probe room", room["capacity"],
                                           mapped["map_id"], mapped["map_revision"],
                                           avatar["manifest_id"], avatar["manifest_version"]]))
                + ",true)")
        cluster.sql("mcr_concert", ";".join(statements))
        world_address = ports.address("concert-service:world")
        relationship, self_profile, decision, post_read, post_callback = world.joined.keys(cluster, 5)
        urls = {role: "https://" + ports.address(role + "-service:grpc")
                for role in ("auth", "friend", "profile", "concert", "notification", "post")}
        invite_keys = json.dumps([{"id": "probe_dev", "secret_hex": secrets.token_hex(32)}])
        intro_receipt_keys = json.dumps(
            [{"id": "probe_intro_receipt", "secret_hex": secrets.token_hex(32)}])
        pickup_keys = json.dumps([{"id": "probe_pickup", "secret_hex": secrets.token_hex(32)}])
        social_handle_keys = json.dumps(
            [{"id": "probe_social_handle", "secret_hex": secrets.token_hex(32)}])
        social_command_keys = json.dumps(
            [{"id": "probe_social_command", "secret_hex": secrets.token_hex(32)}])
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
        envs["api-gateway"]["GATEWAY_REQUEST_TIMEOUT_MS"] = str(args.gateway_timeout_ms)
        inspected = migration.execute(
            [str(args.bin_dir / "concert-service"), "--inspect-product-release"],
            env=envs["concert-service"], cwd=cluster.root, phase="probe-release-inspection",
        )
        digest_value = inspected.stdout.strip().split("=", 1)[1]
        envs["concert-service"]["CONCERT_PARTICIPANT_RELEASE_DIGEST"] = digest_value
        envs["api-gateway"]["GATEWAY_CONCERT_GRPC_ENDPOINT"] = urls["concert"]
        envs["api-gateway"]["GATEWAY_CONCERT_RELEASE_DIGEST"] = digest_value
        state.update(event=event, origin=origin, digest=digest_value)
        return envs

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
            "gateway_request_timeout_ms": args.gateway_timeout_ms,
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
        }
        principals = {}
        for ordinal, role in enumerate(ROLES, 1):
            for attempt in range(5):
                try:
                    principals[role] = closure.principal(gateway, stack.mail, policy, ordinal, origin)
                    break
                except closure.StepFailure:
                    if attempt == 4:
                        raise
                    time.sleep(1.5 + attempt)
        for ordinal, role in enumerate(ROLES, 1):
            closure.initialize_profile(gateway, principals[role][0], principals[role][1],
                                       ordinal, origin)
        users = {role: principals[role][0] for role in ROLES}
        tokens = {role: principals[role][1] for role in ROLES}
        api = Api(gateway, origin, tokens)
        report["principals"] = users

        def sql(db, statement):
            try:
                return stack.cluster.sql(db, statement)
            except migration.SmokeError:
                raise ProbeFailure("sql-failed") from None

        def query(db, statement):
            raw = sql(db, statement)
            return json.loads(raw) if raw else None

        statements = []
        for role in ROLES:
            user = users[role]
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
        sql("mcr_concert", ";".join(statements))
        report["provisioning"] = {"event": event, "participants": list(ROLES)}

        posts = {"alice": str(uuid4()), "bob": str(uuid4())}
        now = "clock_timestamp()"
        sql("mcr_post", ";".join([
            "INSERT INTO post_concert_posts(post_id,event_id,author_id,content,revision,state,"
            "created_at,comment_count) VALUES(" + ",".join(map(
                sql_quote, [posts["alice"], event, users["alice"]]))
            + ",'{\"text\":\"alice public post\",\"media_ids\":[],\"links\":[]}',1,'PUBLISHED',"
            + now + ",0)",
            "INSERT INTO post_concert_posts(post_id,event_id,author_id,content,revision,state,"
            "created_at,comment_count) VALUES(" + ",".join(
                map(sql_quote, [posts["bob"], event, users["bob"]]))
            + ",'{\"text\":\"bob public post\",\"media_ids\":[],\"links\":[]}',1,'PUBLISHED',"
            + now + ",0)",
        ]))
        report["posts"] = {
            "alice": posts["alice"],
            "bob": posts["bob"],
            "db": query("mcr_post",
                        "SELECT jsonb_agg(jsonb_build_object('post_id',post_id,'author_id',author_id,"
                        "'state',state,'created_at',created_at) ORDER BY created_at) "
                        "FROM post_concert_posts"),
        }

        def feed(role, limit):
            attempts = []
            for attempt in range(3):
                started = time.monotonic()
                status, body, _ = api.request(
                    role, "GET", f"/api/concert/events/{event}/feed?limit={limit}")
                elapsed = round((time.monotonic() - started) * 1000, 1)
                attempts.append({"http": status, "ms": elapsed,
                                 "reason": body.get("details", {}).get("reason")
                                 if isinstance(body, dict) else None})
                if status != 504:
                    break
                time.sleep(0.5)
            ids = [item.get("post_id") for item in
                   (body.get("items", []) if isinstance(body, dict) else [])
                   if isinstance(item, dict)]
            return {"http": status, "ms": elapsed, "body": body, "post_ids": ids,
                    "bob_present": posts["bob"] in ids, "attempts": attempts}

        warmup = [feed("carol", 1)["http"] for _ in range(2)]
        report["steps"]["warmup"] = {"http": warmup}
        before = feed("carol", 1)
        report["steps"]["feed_before_block"] = {"limit1": before,
                                                "viewer": "carol"}
        require(before["http"] == 200 and before["bob_present"],
                f"feed-baseline-{before['http']}")

        def block_state(role, method, path, body, key):
            attempts = []
            for attempt in range(2):
                status, value, _ = api.request(role, method, path, body, key)
                attempts.append({"http": status,
                                 "reason": value.get("details", {}).get("reason")
                                 if isinstance(value, dict) else None})
                if status != 503:
                    break
                time.sleep(0.3)
            return status, value, attempts

        def pair_rows():
            low, high = sorted([users["carol"], users["bob"]])
            return query("mcr_concert",
                         "SELECT jsonb_agg(jsonb_build_object('low_user',low_user,'high_user',high_user,"
                         "'low_blocks_high',low_blocks_high,'high_blocks_low',high_blocks_low,"
                         "'revision',revision)) FROM concert_relationship_pairs WHERE event_id="
                         + sql_quote(event) + " AND low_user=" + sql_quote(low)
                         + " AND high_user=" + sql_quote(high))

        def pair_blocked():
            rows = pair_rows() or []
            return any(row.get("low_blocks_high") or row.get("high_blocks_low") for row in rows)

        status, body, block_attempts = block_state(
            "carol", "PUT", f"/api/concert/events/{event}/me/blocks/{users['bob']}",
            {}, str(uuid4()))
        report["steps"]["social_block"] = {
            "http": status, "body": body, "attempts": block_attempts,
            "db": pair_rows(),
        }
        require(pair_blocked(), "social-block-not-persisted")

        blocked = feed("carol", 1)
        report["steps"]["feed_after_block"] = {"limit1": blocked,
                                               "viewer": "carol"}
        require(blocked["http"] == 200, f"feed-blocked-{blocked['http']}")
        require(not blocked["bob_present"], "feed-blocked-author-visible")

        status, body, unblock_attempts = block_state(
            "carol", "DELETE", f"/api/concert/events/{event}/me/blocks/{users['bob']}",
            None, str(uuid4()))
        report["steps"]["social_unblock"] = {
            "http": status, "body": body, "attempts": unblock_attempts,
            "db": pair_rows(),
        }
        require(not pair_blocked(), "social-unblock-not-persisted")

        restored = feed("carol", 1)
        report["steps"]["feed_after_unblock"] = {"limit1": restored,
                                                  "viewer": "carol"}
        require(restored["http"] == 200 and restored["bob_present"], "feed-restored")

        stop = stack.children["profile-service"].stop()
        report["steps"]["profile_owner_stopped"] = stop
        outage = feed("carol", 1)
        report["steps"]["feed_owner_down"] = outage
        require(outage["http"] == 503, f"owner-down-{outage['http']}")

        report["checks"] = {
            "feed_before_block_200": before["http"] == 200,
            "bob_present_before_block": before["bob_present"],
            "pair_row_blocked": any(
                row.get("low_blocks_high") or row.get("high_blocks_low")
                for row in report["steps"]["social_block"]["db"]),
            "feed_after_block_200": blocked["http"] == 200,
            "bob_absent_after_block": not blocked["bob_present"],
            "pair_row_unblocked": not any(
                row.get("low_blocks_high") or row.get("high_blocks_low")
                for row in report["steps"]["social_unblock"]["db"]),
            "feed_after_unblock_200": restored["http"] == 200,
            "bob_present_after_unblock": restored["bob_present"],
            "owner_down_503": outage["http"] == 503,
        }
        failed = {name: value for name, value in report["checks"].items() if not value}
        report["result"] = "PASS" if not failed else "FAIL"
        report["failed_checks"] = failed
    except Exception as failure:  # noqa: BLE001 - owned development probe
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
        args.report.write_text(json.dumps(report, indent=2))
    return 0 if report.get("result") == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
