#!/usr/bin/env python3
"""Task 43 real-producer stack for FE24/FE33/FE34 browser closure.

Starts this checkout's seven native services on an owned private PG16 cluster with
an owned Redis, local mail capture and real mTLS. It provisions one event, two
admitted principals, a Wall seed, a profile preference baseline and a giveaway
whose image is an actual finalized protected media record. The UI harness reads
`runtime.json`, drives the real feature components through the real gateway and
captures 360/390/430 happy and unavailable states.

This is a development-stack check, not release, device or QA08 qualification.
"""
from __future__ import annotations

import argparse
import hashlib
import http.client
import json
import secrets
import sys
import time
from pathlib import Path
from unittest.mock import patch
from uuid import uuid4

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "backend/scripts/mcr"))
import closure_vertical_stack as closure  # noqa: E402
import dev_stack  # noqa: E402
import in04_wall_b_gate as in04  # noqa: E402
import migration_smoke as migration  # noqa: E402
import runtime_smoke as runtime  # noqa: E402
import world_in01_live as world  # noqa: E402
import world_in01_prepare as prep  # noqa: E402

ROOT = Path(__file__).resolve().parents[3]
PNG = bytes.fromhex(
    "89504e470d0a1a0a0000000d4948445200000060000000200806000000edc07d54"
    "0000006249444154789cedd1210100200c00b00742a30980c052829844a1068440"
    "dc4c2cc1a2af713395da529d3d5385000102040810204080000102040810204080"
    "000102040810204080000102040810204080000102040810204080000102040810"
    "204080000102040810204080000102040810204080000102040810e0df03105d72"
    "0e2cbe1d068b0000000049454e44ae426082"
)


def require(condition, label):
    if not condition:
        raise in04.GateFailure(label)


class MediaProbe:
    """Direct media probe that records the status even when a body stream truncates."""

    def __init__(self, gateway, origin, token):
        self.gateway, self.origin, self.token = gateway, origin, token

    def send(self, method, path, body=None, content_type=None):
        host, port = self.gateway.split("//", 1)[1].rsplit(":", 1)
        connection = http.client.HTTPConnection(host, int(port), timeout=30)
        headers = {"X-MCR-Intent": "browser", "Origin": self.origin,
                   "Authorization": "Bearer " + self.token}
        if content_type:
            headers["Content-Type"] = content_type
        if method != "GET":
            headers["Idempotency-Key"] = str(uuid4())
        result = {"method": method, "path": path, "body_bytes": len(body or b"")}
        try:
            connection.request(method, path, body=body, headers=headers)
            response = connection.getresponse()
            result["status"] = response.status
            try:
                payload, truncated = response.read(), False
            except http.client.IncompleteRead as failure:
                payload, truncated = failure.partial, True
            result["body_truncated"] = truncated
            try:
                parsed = json.loads(payload or b"{}")
            except ValueError:
                parsed = {}
            result["reason"] = (parsed.get("details") or {}).get("reason") or parsed.get("code")
        except (OSError, http.client.HTTPException) as failure:
            result.setdefault("status", None)
            result["transport_error"] = type(failure).__name__
        finally:
            connection.close()
        return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", type=Path, required=True)
    parser.add_argument("--vite-origin", required=True)
    parser.add_argument("--runtime-out", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--done-file", type=Path, required=True)
    parser.add_argument("--timeout-seconds", type=int, default=900)
    parser.add_argument("--pg-bin-dir", type=Path, default=Path("/opt/homebrew/opt/postgresql@16/bin"))
    parser.add_argument("--redis-binary", type=Path, default=Path("/opt/homebrew/bin/redis-server"))
    parser.add_argument("--openssl-binary", type=Path, default=Path("/opt/homebrew/bin/openssl"))
    args = parser.parse_args()

    report = {"result": "IN_PROGRESS", "steps": {}, "banner": {
        "scope": "development stack real-producer FE24/FE33/FE34 closure",
        "release": "development stack, NOT release qualification", "qa08": "NOT_RUN",
        "device": "NOT_RUN", "production": "NOT_CLAIMED"}}
    args.report.write_text(json.dumps(report, indent=2))
    args.done_file.unlink(missing_ok=True)

    pixels, palette, manifest_sha = in04.load_pack(ROOT)

    migration.SERVICES["concert-service"] = "CONCERT_DATABASE_URL"
    runtime.PREFIX.update({name: variable.removesuffix("_DATABASE_URL")
                           for name, variable in migration.SERVICES.items()})
    runtime.START_ORDER = ("profile-service", "notification-service", "friend-service", "post-service",
                           "concert-service", "auth-service", "api-gateway")
    runtime.PEERS["concert-service"] = ("auth-service", "friend-service", "profile-service")
    runtime.EXTRA_LISTENERS["concert-service"] = ("world",)
    original_child_init = runtime.Child.__init__

    def child_init(self, cluster, name, argv, env, ports, reservations, secrets_to_redact=()):
        if name == "concert-service":
            reservations = tuple(key for key in reservations if key != "concert-service:http")
        original_child_init(self, cluster, name, argv, env, ports, reservations, secrets_to_redact)

    runtime.Child.__init__ = child_init

    state = {"event": None, "origin": None, "rooms": None}
    stack = dev_stack.DevStack(args.bin_dir, args.pg_bin_dir, args.redis_binary,
                               args.openssl_binary, args.vite_origin)
    base_environments = dev_stack.development_environments

    def environments(cluster, ports, keys, credentials, redis_password, mail_token, browser_origin=None):
        envs = base_environments(cluster, ports, keys, credentials, redis_password, mail_token, browser_origin)
        cluster.assert_owned()
        cluster.stop()
        with (cluster.data / "postgresql.conf").open("a") as stream:
            stream.write("\nmax_prepared_transactions=256\n")
        cluster.start()
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
                      + ",".join(map(prep.quote, [event, "fe43-" + secrets.token_hex(8),
                                                  "FE43 development closure", "OPEN"])) + ")"]
        for room in rooms:
            statements.append(
                "INSERT INTO concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,"
                "avatar_manifest_id,avatar_manifest_version,is_default) VALUES("
                + ",".join(map(prep.quote, [event, room["room_id"], "FE43 dev room", room["capacity"],
                                            mapped["map_id"], mapped["map_revision"],
                                            avatar["manifest_id"], avatar["manifest_version"]]))
                + ",true)")
        statements += [
            "INSERT INTO concert_meeting_settings(event_id,event_cutoff) VALUES("
            + prep.quote(event) + ",clock_timestamp()+interval '1 day') ON CONFLICT DO NOTHING",
            "INSERT INTO concert_mural_state(event_id,revision,snapshot_id,state,pixels,palette,cooldown_ms,retained_from_revision,reset_revision) VALUES("
            + ",".join(map(prep.quote, [event, "1", str(uuid4()), "EDITABLE"]))
            + ",decode('" + pixels.hex() + "','hex'),ARRAY[" + ",".join(prep.quote(c) for c in palette)
            + "],1200,1,1)",
            "INSERT INTO concert_mural_tool_config(event_id,background_index) VALUES("
            + prep.quote(event) + ",0)",
        ]
        try:
            cluster.sql("mcr_concert", ";".join(statements))
        except migration.SmokeError as error:
            raise in04.GateFailure(str(error.detail.get("diagnostic"))[:500]) from None
        try:
            cluster.sql("mcr_concert", "BEGIN; SET LOCAL mcr.operations_authority_until='2099-01-01T00:00:00Z'; "
                "INSERT INTO concert_event_settings(event_id,parties,giveaways,intros,social,mural,"
                "beacon_ttl_seconds,pickup_grace_seconds,social_grant_ttl_seconds,mural_cooldown_ms) VALUES("
                + prep.quote(event) + ",true,true,true,true,true,300,0,3600,1200); COMMIT;")
        except migration.SmokeError as error:
            raise in04.GateFailure(str(error.detail.get("diagnostic"))[:500]) from None
        world_address = ports.address("concert-service:world")
        envs["concert-service"].update({
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
            "CONCERT_WORLD_INVALIDATION_CONSUMER_ID": owner,
            "CONCERT_WORLD_ROOMS_JSON": json.dumps(rooms),
            "CONCERT_WORLD_RESOURCE_ROOT": release["asset_root"],
            "CONCERT_WORLD_SOURCE_BINDING_FILE": release["source_binding"],
            "CONCERT_WALL_TOOLS_B_MODE": "live",
            "CONCERT_WALL_TOOLS_B_RECEIPT_KEYS_JSON": json.dumps(
                [{"id": "fe43", "secret_hex": secrets.token_hex(32)}]),
        })
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
                [{"id": "fe43_dev", "secret_hex": secrets.token_hex(32)}]),
            CONCERT_OWNER_MEDIA_MODE="uploader",
            CONCERT_MEDIA_SANITIZER_WORKERS="1",
            CONCERT_MEDIA_SANITIZER_QUEUE="4",
            CONCERT_MEDIA_RECEIPT_KEYS_JSON=json.dumps(
                [{"id": "fe43_media", "secret_hex": secrets.token_hex(32)}]),
        )
        for role in ("auth", "friend", "profile"):
            envs["concert-service"]["CONCERT_PARTICIPANT_" + role.upper() + "_GRPC_ENDPOINT"] = urls[role]
            envs["concert-service"]["CONCERT_" + role.upper() + "_GRPC_ENDPOINT"] = urls[role]
        envs["concert-service"].update(
            CONCERT_INTRO_MODE="live",
            CONCERT_NOTIFICATION_GRPC_ENDPOINT=urls["notification"],
            CONCERT_INTRO_CURSOR_KEY_HEX=secrets.token_hex(32),
            CONCERT_INTRO_RECEIPT_KEYS_JSON=json.dumps(
                [{"id": "fe43_dev", "secret_hex": secrets.token_hex(32)}]),
            CONCERT_PICKUP_KEYS_JSON=json.dumps(
                [{"id": "fe43_dev", "secret_hex": secrets.token_hex(32)}]),
        )
        inspected = migration.execute(
            [str(args.bin_dir / "concert-service"), "--inspect-product-release"],
            env=envs["concert-service"], cwd=cluster.root, phase="fe43-release-inspection",
        )
        digest_value = inspected.stdout.strip().split("=", 1)[1]
        envs["concert-service"]["CONCERT_PARTICIPANT_RELEASE_DIGEST"] = digest_value
        envs["api-gateway"]["GATEWAY_CONCERT_GRPC_ENDPOINT"] = urls["concert"]
        envs["api-gateway"]["GATEWAY_CONCERT_RELEASE_DIGEST"] = digest_value
        state.update(event=event, origin=origin, rooms=rooms, release=release, digest=digest_value)
        return envs

    try:
        with patch.object(dev_stack, "development_environments", environments), \
                patch.object(dev_stack, "MailSink", closure.EnrollmentMailSink), \
                patch.object(runtime, "readiness_value", closure.readiness):
            stack.start()
        gateway = stack.descriptor()["gateway_url"]
        origin, event = state["origin"], state["event"]
        policy = closure.publish_enrollment_policy(stack.cluster)
        principals = {}
        for ordinal, role in enumerate(("alice", "bob"), 1):
            principals[role] = closure.principal(gateway, stack.mail, policy, ordinal, origin)
        for ordinal, role in enumerate(("alice", "bob"), 1):
            closure.initialize_profile(gateway, principals[role][0], principals[role][1], ordinal, origin)
        users = {role: principals[role][0] for role in principals}
        tokens = {role: principals[role][1] for role in principals}
        api = in04.Api(gateway, origin, tokens)
        report["stack"] = {"gateway_url": gateway, "origin": origin, "event": event,
                           "users": users, "release_digest": state["digest"]}

        statements = []
        for user in users.values():
            statements += [
                "INSERT INTO concert_event_admissions(event_id,user_id,state) VALUES("
                + ",".join(map(prep.quote, [event, user, "ACTIVE"])) + ") ON CONFLICT DO NOTHING",
                "INSERT INTO concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) VALUES("
                + ",".join(map(prep.quote, [event, user, "REVENGE"])) + ",false,false,false) ON CONFLICT DO NOTHING",
                "INSERT INTO concert_avatar_settings(event_id,user_id,contract_version,representation,preset_id,manifest_id,manifest_version) SELECT "
                + prep.quote(event) + ",u::uuid,'1.1.0','whole_character','photo01_front_01','mcr_selected_static_20261103',1 "
                + "FROM (VALUES (" + prep.quote(user) + ")) v(u) ON CONFLICT DO NOTHING",
            ]
        stack.cluster.sql("mcr_concert", ";".join(statements))
        report["steps"]["provisioning"] = {"event": event, "users": users,
                                           "mural_seed_sha256": manifest_sha}

        place = "6a5c196b-58e1-47d5-99bb-000000000001"
        now = time.time()
        starts = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(now + 300))
        ends = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(now + 3600))
        media = str(uuid4())
        gift_id = str(uuid4())
        empty_id = str(uuid4())
        missing_id = str(uuid4())
        missing_media = str(uuid4())
        created = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(now))

        def aggregate(gid, image):
            return json.dumps({"id": gid, "event": event, "owner": users["alice"],
                "fields": {"title": f"Giveaway {gid[-4:]}", "description": "FE43 protected media",
                           "image_media_id": image, "quantity_total": 3, "pickup_starts_at": starts,
                           "pickup_ends_at": ends, "public_place_id": place},
                "state": "OPEN", "revision": 1, "pickup_grace_seconds": 0, "reserved_unclaimed": 0,
                "quantity_claimed": 0, "meeting": None, "meeting_revision": 0, "created_at": created,
                "reservations": []}, separators=(",", ":"))

        event_rev = stack.cluster.sql("mcr_concert", "select revision from concert_events where event_id=" + prep.quote(event))
        restriction = stack.cluster.sql("mcr_concert", "select restriction_revision from concert_event_admissions where event_id="
                                        + prep.quote(event) + " and user_id=" + prep.quote(users["alice"]))
        seed = [
            "INSERT INTO concert_media(media_id,event_id,owner_id,purpose,content_type,declared_bytes,input_sha256,"
            "policy_event_revision,policy_relationship_revision,policy_authority_revision,state,revision,sanitized,width,height,target_id) VALUES("
            + ",".join([prep.quote(media), prep.quote(event), prep.quote(users["alice"]), prep.quote("GIVEAWAY"),
                         prep.quote("image/png"), str(len(PNG)), "decode('" + hashlib.sha256(PNG).hexdigest() + "','hex')",
                         str(int(event_rev)), str(int(restriction)), "1", prep.quote("READY"), "1",
                         "decode('" + PNG.hex() + "','hex')", "96", "32", prep.quote(gift_id)]) + ")",
        ]
        for gid, image in ((gift_id, media), (empty_id, None), (missing_id, missing_media)):
            seed.append(
                "INSERT INTO concert_community_giveaways(event_id,giveaway_id,owner_id,revision,state,"
                "quantity_total,reserved_unclaimed,quantity_claimed,next_action_at,aggregate) VALUES("
                + ",".join([prep.quote(event), prep.quote(gid), prep.quote(users["alice"]), "1",
                              prep.quote("OPEN"), "3", "0", "0", prep.quote(ends), prep.quote(aggregate(gid, image))]) + ")")
        try:
            stack.cluster.sql("mcr_concert", ";".join(seed))
        except migration.SmokeError as error:
            raise in04.GateFailure(str(error.detail.get("diagnostic"))[:500]) from None
        report["steps"]["giveaways"] = {"media": gift_id, "empty": empty_id, "missing": missing_id,
                                        "media_id": media,
                                        "note": "media row and giveaway aggregates are seeded because the canonical owner-fence upload/link producer is not composed in this harness"}

        # FE24 media probe: the owner bearer's protected read/upload access is recorded from
        # the real stack instead of being asserted only in prose. Empty upload bytes are the
        # invalid input the producer rejects before admitting the owner fence.
        media_base = f"/api/concert/events/{event}/media/{media}"
        probe = MediaProbe(gateway, origin, tokens["alice"])
        metadata_probe = probe.send("GET", media_base)
        content_probe = probe.send("GET", media_base + "/content")
        bytes_probe = probe.send("PUT", media_base + "/bytes", b"", "image/png")
        report["fe24_media_probe"] = {
            "media_id": media, "role": "alice", "bearer": "owner",
            "metadata": metadata_probe, "content": content_probe, "bytes": bytes_probe,
            "expected": {"metadata": "403 PERMISSION_DENIED", "content": "403 PERMISSION_DENIED",
                         "bytes": "400 INVALID_INPUT"},
            "matches_expected": (metadata_probe.get("status"), content_probe.get("status"),
                                 bytes_probe.get("status")) == (403, 403, 400),
        }

        # Profile preferences: canonical PATCH, then a stale expected revision -> 409.
        prefs_status = 0
        prefs = {}
        original = 0
        for _ in range(8):
            me_status, me, _ = api.request("alice", "GET", f"/api/concert/events/{event}/me")
            require(me_status == 200, f"profile-get-{me_status}")
            original = me["revision"]
            updated_prefs = {**me["preferences"], "low_power": True}
            prefs_status, prefs, _ = api.request(
                "alice", "PATCH", f"/api/concert/events/{event}/me",
                {"expected_revision": original, "preferences": updated_prefs})
            if prefs_status == 200:
                break
            time.sleep(0.25)
        conflict_status, conflict, _ = api.request(
            "alice", "PATCH", f"/api/concert/events/{event}/me",
            {"expected_revision": original, "preferences": me["preferences"]})
        report["steps"]["profile_preferences"] = {
            "initial_revision": original, "patch": prefs_status, "patch_body": prefs,
            "saved_revision": prefs.get("revision"), "saved_low_power": prefs.get("preferences", {}).get("low_power"),
            "stale": conflict_status, "stale_reason": conflict.get("details", {}).get("reason")}
        require(prefs_status == 200 and conflict_status == 409, "profile-conflict")

        mural_status, snapshot, _ = api.request(
            "alice", "GET", f"/api/concert/events/{event}/mural/snapshot")
        delta_status, delta, _ = api.request(
            "alice", "GET", f"/api/concert/events/{event}/mural/operations?after_revision=1")
        report["steps"]["mural"] = {
            "snapshot": mural_status, "revision": snapshot.get("revision"),
            "state": snapshot.get("state"), "snapshot_id": snapshot.get("snapshot_id"),
            "delta": delta_status, "from_revision": delta.get("from_revision"),
            "to_revision": delta.get("to_revision")}
        require(mural_status == 200 and delta_status == 200 and snapshot.get("revision") == 1, "mural-read")

        bootstrap_status, bootstrap, _ = api.request(
            "alice", "GET", f"/api/concert/events/{event}/bootstrap")
        require(bootstrap_status == 200, f"bootstrap-{bootstrap_status}")

        runtime_packet = {
            "gateway_url": gateway, "vite_origin": args.vite_origin, "event_id": event,
            "users": users, "tokens": tokens, "media_id": media,
            "media_giveaway_id": gift_id, "empty_giveaway_id": empty_id,
            "missing_giveaway_id": missing_id,
            "profile_revision": prefs["revision"],
            "mural": {"revision": snapshot["revision"], "state": snapshot["state"],
                      "snapshot_id": snapshot["snapshot_id"]},
            "bootstrap": bootstrap, "source_revision": stack.report.get("source_revision"),
        }
        args.runtime_out.write_text(json.dumps(runtime_packet, indent=2))
        report["steps"]["runtime_packet"] = str(args.runtime_out)
        report["result"] = "READY"
        args.report.write_text(json.dumps(report, indent=2))

        deadline = time.monotonic() + args.timeout_seconds
        while not args.done_file.exists() and time.monotonic() < deadline:
            time.sleep(0.5)
        report["steps"]["browser_done"] = args.done_file.exists()
        report["result"] = "PASS" if args.done_file.exists() else "TIMEOUT"
    finally:
        try:
            stack.close()
            cleanup = {"result": "PASS"}
        except Exception as error:  # noqa: BLE001 - report the exact cleanup failure
            cleanup = {"result": "FAIL", "failure": type(error).__name__, "detail": str(error)[:300]}
        report["cleanup"] = cleanup
        args.report.write_text(json.dumps(report, indent=2))
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
