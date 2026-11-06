#!/usr/bin/env python3
"""IN04 development-stack multiuser Wall B and moderation gate.

Banner: development stack, NOT IN03/release qualification, QA08 NOT_RUN. This
driver starts this checkout's seven native services on an owned private PG16
cluster with real mTLS, real Auth/Friend/Concert/Gateway and an owned temp root.
It provisions an event and the qualified initial-motif pack, then drives two
independent member clients plus one real OPERATOR through the public HTTP
surface. Every step records its request, status, closed body summary, server
sync frame and durable row. A step the composed product cannot reach is
recorded NOT_RUN with its exact blocking consumer, never PASS.
"""
from __future__ import annotations

import argparse
import asyncio
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
from uuid import uuid4

sys.path.insert(0, str(Path(__file__).resolve().parent))
import closure_vertical_stack as closure  # noqa: E402
import dev_stack  # noqa: E402
import migration_smoke as migration  # noqa: E402
import runtime_smoke as runtime  # noqa: E402
import world_in01_live as world  # noqa: E402
import world_in01_prepare as prep  # noqa: E402

BANNER = {
    "scope": "development stack multiuser Wall B and moderation gate",
    "release": "development stack, NOT IN03/release qualification",
    "qa08": "NOT_RUN",
    "production": "NOT_CLAIMED",
    "device": "NOT_RUN",
}
PACK = Path("frontend/public/wall/initial-motif-v1")
COOLDOWN_MS = 1200
ROLES = ("operator", "alice", "bob")
FORBIDDEN_PUBLIC_KEYS = {
    "actor_id",
    "before",
    "cells",
    "command",
    "idempotency_key",
    "author",
}


class GateFailure(RuntimeError):
    pass


def require(condition, label):
    if not condition:
        raise GateFailure(label)


def iso(value):
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def load_pack(root):
    directory = root / PACK
    require(directory.is_dir(), "seed-pack-missing")
    pixels = (directory / "pixels.bin").read_bytes()
    palette = json.loads((directory / "palette.json").read_bytes())
    manifest = json.loads((directory / "manifest.json").read_bytes())
    require(len(pixels) == 16384 and all(c < 16 for c in pixels), "seed-pixels-invalid")
    require(
        isinstance(palette, list)
        and len(palette) == 16
        and all(isinstance(c, str) and len(c) == 7 and c.startswith("#")
                and all(ch.isdigit() or "A" <= ch <= "F" for ch in c[1:]) for c in palette),
        "seed-palette-invalid",
    )
    require(manifest.get("width") == 128 and manifest.get("height") == 128, "seed-manifest-invalid")
    return pixels, palette, hashlib.sha256((directory / "manifest.json").read_bytes()).hexdigest()


def component(pixels, seed):
    color = pixels[seed]
    seen = bytearray(len(pixels))
    seen[seed] = 1
    queue = [seed]
    cells = []
    cursor = 0
    while cursor < len(queue):
        index = queue[cursor]
        cursor += 1
        cells.append(index)
        x, y = index % 128, index // 128
        for neighbour in (
            index - 1 if x > 0 else -1,
            index + 1 if x + 1 < 128 else -1,
            index - 128 if y > 0 else -1,
            index + 128 if y + 1 < 128 else -1,
        ):
            if neighbour >= 0 and not seen[neighbour] and pixels[neighbour] == color:
                seen[neighbour] = 1
                queue.append(neighbour)
    return color, sorted(cells)


def forbidden_keys(value, found=None):
    found = set() if found is None else found
    if isinstance(value, dict):
        for key, child in value.items():
            if key in FORBIDDEN_PUBLIC_KEYS:
                found.add(key)
            forbidden_keys(child, found)
    elif isinstance(value, list):
        for child in value:
            forbidden_keys(child, found)
    return found


class Api:
    def __init__(self, gateway, origin, tokens):
        self.gateway, self.origin, self.tokens = gateway, origin, tokens
        self.log = []

    def request(self, role, method, path, body=None, key=None, headers=None):
        token = self.tokens[role]
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
        connection = http.client.HTTPConnection(
            self.gateway.split("//", 1)[1].split(":")[0],
            int(self.gateway.split(":")[-1]),
            timeout=30,
        )
        try:
            connection.request(method, path, body=raw, headers=hdrs)
            response = connection.getresponse()
            payload = response.read()
            try:
                body = json.loads(payload or b"{}")
            except ValueError:
                body = {"raw": payload[:200].decode("utf-8", "replace")}
            return response.status, body, dict(response.getheaders())
        except (http.client.IncompleteRead, http.client.RemoteDisconnected,
                ConnectionResetError, BrokenPipeError) as failure:
            if attempt < 1:
                self.log.append({"transport_retry": type(failure).__name__,
                                 "method": method, "path": path})
                return self._send(method, path, raw, hdrs, attempt + 1)
            raise
        finally:
            connection.close()

    def send_no_read(self, role, path, body, key):
        host = self.gateway.split("//", 1)[1].split(":")[0]
        port = int(self.gateway.rsplit(":", 1)[1])
        raw = json.dumps(body).encode()
        connection = http.client.HTTPConnection(host, port, timeout=5)
        connection.putrequest("POST", path)
        connection.putheader("Authorization", "Bearer " + self.tokens[role])
        connection.putheader("X-MCR-Intent", "browser")
        connection.putheader("Origin", self.origin)
        connection.putheader("Content-Type", "application/json")
        connection.putheader("Idempotency-Key", key)
        connection.putheader("Content-Length", str(len(raw)))
        connection.endheaders(raw)
        time.sleep(0.5)
        connection.close()


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
    report = {"banner": BANNER, "steps": {}, "checks": {}, "not_run": {}}
    args.report.write_text(json.dumps(report, indent=2))
    pixels, palette, manifest_sha = load_pack(root)
    background_component = component(pixels, 0)[1]
    fill_seed = 2361
    fill_color, fill_cells = component(pixels, fill_seed)
    require(fill_color == 1 and 2 <= len(fill_cells) <= 256, "fill-region-not-bounded")
    require(len(background_component) > 256, "fill-too-large-region-missing")
    run_colors = [3, 7, 8, 10, 11, 12]
    report["scenario"] = {
        "pack_manifest_sha256": manifest_sha,
        "fill_seed": fill_seed,
        "fill_cells": len(fill_cells),
        "background_cells": len(background_component),
        "cooldown_ms": COOLDOWN_MS,
        "run_colors": run_colors,
    }

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
    stack = dev_stack.DevStack(args.bin_dir, args.pg_bin_dir, args.redis_binary, args.openssl_binary, None)
    base_environments = dev_stack.development_environments

    def environments(cluster, ports, keys, credentials, redis_password, mail_token, browser_origin=None):
        envs = base_environments(cluster, ports, keys, credentials, redis_password, mail_token, browser_origin)
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
                      + ",".join(map(prep.quote, [event, "in04-dev-" + secrets.token_hex(8),
                                                  "IN04 development gate", "OPEN"])) + ")"]
        for room in rooms:
            statements.append(
                "INSERT INTO concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,"
                "avatar_manifest_id,avatar_manifest_version,is_default) VALUES("
                + ",".join(map(prep.quote, [event, room["room_id"], "IN04 dev room", room["capacity"],
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
                "CONCERT_WORLD_INVALIDATION_CONSUMER_ID": owner,
                "CONCERT_WORLD_ROOMS_JSON": json.dumps(rooms),
                "CONCERT_WORLD_RESOURCE_ROOT": release["asset_root"],
                "CONCERT_WORLD_SOURCE_BINDING_FILE": release["source_binding"],
                "CONCERT_WALL_TOOLS_B_MODE": "live",
                "CONCERT_WALL_TOOLS_B_RECEIPT_KEYS_JSON": json.dumps(
                    [{"id": "in04_dev", "secret_hex": secrets.token_hex(32)}]),
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
                [{"id": "in04_dev", "secret_hex": secrets.token_hex(32)}]
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
                [{"id": "in04_dev", "secret_hex": secrets.token_hex(32)}]
            ),
            CONCERT_PICKUP_KEYS_JSON=json.dumps(
                [{"id": "in04_dev", "secret_hex": secrets.token_hex(32)}]
            ),
        )
        inspected = migration.execute(
            [str(args.bin_dir / "concert-service"), "--inspect-product-release"],
            env=envs["concert-service"], cwd=cluster.root, phase="in04-release-inspection",
        )
        digest_value = inspected.stdout.strip().split("=", 1)[1]
        envs["concert-service"]["CONCERT_PARTICIPANT_RELEASE_DIGEST"] = digest_value
        envs["api-gateway"]["GATEWAY_CONCERT_GRPC_ENDPOINT"] = urls["concert"]
        envs["api-gateway"]["GATEWAY_CONCERT_RELEASE_DIGEST"] = digest_value
        state.update(event=event, origin=origin, rooms=rooms, release=release, digest=digest_value)
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
        principals = {}
        principal_attempts = {}
        for ordinal, role in enumerate(ROLES, 1):
            for attempt in range(3):
                try:
                    principals[role] = closure.principal(gateway, stack.mail, policy, ordinal, origin)
                    principal_attempts[role] = attempt + 1
                    break
                except closure.StepFailure:
                    if attempt == 2:
                        raise
                    time.sleep(1)
        report["principal_attempts"] = principal_attempts
        for ordinal, role in enumerate(ROLES, 1):
            closure.initialize_profile(gateway, principals[role][0], principals[role][1], ordinal, origin)
        operator, alice, bob = (principals[role][0] for role in ROLES)
        tokens = {role: principals[role][1] for role in ROLES}
        api = Api(gateway, origin, tokens)

        def sql(db, statement):
            try:
                return stack.cluster.sql(db, statement)
            except migration.SmokeError:
                raise GateFailure("sql-failed") from None

        seed_snapshot = str(uuid4())
        statements = [
            "INSERT INTO concert_events(event_id,slug,title,phase) SELECT "
            + prep.quote(event) + ",'in04-extra','IN04 gate extra','OPEN' "
            + "WHERE NOT EXISTS(SELECT 1 FROM concert_events WHERE event_id=" + prep.quote(event) + ")"
        ]
        for user in (operator, alice, bob):
            statements += [
                "INSERT INTO concert_event_admissions(event_id,user_id,state) VALUES("
                + ",".join(map(prep.quote, [event, user, "ACTIVE"])) + ") ON CONFLICT DO NOTHING",
                "INSERT INTO concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) VALUES("
                + ",".join(map(prep.quote, [event, user, "REVENGE"])) + ",false,false,false) ON CONFLICT DO NOTHING",
            ]
        statements += [
            "INSERT INTO concert_event_operator_roles(event_id,user_id,role,granted_by) VALUES("
            + ",".join(map(prep.quote, [event, operator, "OPERATOR", operator]))
            + ") ON CONFLICT DO NOTHING",
            "INSERT INTO concert_meeting_settings(event_id,event_cutoff) VALUES("
            + prep.quote(event) + ",clock_timestamp()+interval '1 day') ON CONFLICT DO NOTHING",
            "INSERT INTO concert_avatar_settings(event_id,user_id,contract_version,representation,preset_id,manifest_id,manifest_version) SELECT "
            + prep.quote(event) + ",u::uuid,'1.1.0','whole_character','photo01_front_01','mcr_selected_static_20261103',1 "
            + "FROM (VALUES (" + "),(".join(prep.quote(user) for user in (operator, alice, bob)) + ")) v(u) ON CONFLICT DO NOTHING",
            "INSERT INTO concert_mural_state(event_id,revision,snapshot_id,state,pixels,palette,cooldown_ms,retained_from_revision,reset_revision) VALUES("
            + prep.quote(event) + ",1," + prep.quote(seed_snapshot) + ",'EDITABLE',decode('"
            + pixels.hex() + "','hex'),ARRAY[" + ",".join(prep.quote(c) for c in palette) + "],"
            + str(COOLDOWN_MS) + ",1,1)",
            "INSERT INTO concert_mural_tool_config(event_id,background_index) VALUES("
            + prep.quote(event) + ",0)",
        ]
        sql("mcr_concert", ";".join(statements))
        report["provisioning"] = {
            "mode": "direct owned-PG insert of the qualified six-file pack bytes",
            "pack_manifest_sha256": manifest_sha,
            "seed_snapshot_id": seed_snapshot,
            "mural_seed_producer": "NOT_RUN: mural_seed::provision has no CLI/HTTP entry; the actual "
            "pack pixels/palette are inserted into the owned disposable cluster",
            "operator_role": sql("mcr_concert",
                "SELECT role,revision,coalesce(revoked_at::text,'') FROM concert_event_operator_roles WHERE event_id="
                + prep.quote(event) + " AND user_id=" + prep.quote(operator)),
        }

        log_offsets = {}

        def record(name, value):
            value["log"] = closure.new_log(stack, log_offsets)
            report["steps"][name] = value
            return value

        def snapshot(role):
            status, body, _ = api.request(role, "GET", f"/api/concert/events/{event}/mural/snapshot")
            if status == 200:
                body["pixels_sha256"] = hashlib.sha256(bytes(body["pixels"])).hexdigest()
            return status, body

        def frame(role, after, snapshot_body=None):
            status, delta, _ = api.request(
                role, "GET", f"/api/concert/events/{event}/mural/operations?after_revision={after}")
            return {
                "actor": role,
                "after_revision": after,
                "status": status,
                "delta": {
                    "from_revision": delta.get("from_revision"),
                    "to_revision": delta.get("to_revision"),
                    "reset_required": delta.get("reset_required"),
                    "operations": len(delta.get("operations") or []),
                },
                "snapshot": None if snapshot_body is None else {
                    "revision": snapshot_body.get("revision"),
                    "state": snapshot_body.get("state"),
                    "snapshot_id": snapshot_body.get("snapshot_id"),
                    "pixels_sha256": snapshot_body.get("pixels_sha256"),
                },
            }

        def tools(role):
            status, body, _ = api.request(role, "GET", f"/api/concert/events/{event}/mural/tools")
            require(status == 200, f"tools-{role}-{status}")
            return body

        def wait_allowed(role):
            body = tools(role)
            next_at = body.get("next_allowed_at")
            if not next_at:
                return
            delay = (iso(next_at) - iso(body["server_time"])).total_seconds()
            if delay > 0:
                time.sleep(min(delay + 0.08, 5.0))

        def action(role, suffix, body):
            wait_allowed(role)
            key = str(uuid4())
            status, reply, _ = api.request(role, "POST", f"/api/concert/events/{event}/mural/{suffix}",
                                           body, key)
            return status, reply, key

        def current_grid(role):
            status, body = snapshot(role)
            require(status == 200, f"snapshot-{role}-{status}")
            return body

        # Step 1: both independent clients read the same authoritative revision.
        alice0, bob0 = current_grid("alice"), current_grid("bob")
        record("initial_snapshots", {
            "alice": {"status": 200, "revision": alice0["revision"], "state": alice0["state"],
                      "snapshot_id": alice0["snapshot_id"], "pixels_sha256": alice0["pixels_sha256"]},
            "bob": {"status": 200, "revision": bob0["revision"], "state": bob0["state"],
                    "snapshot_id": bob0["snapshot_id"], "pixels_sha256": bob0["pixels_sha256"]},
            "converged": alice0["revision"] == bob0["revision"]
            and alice0["pixels_sha256"] == bob0["pixels_sha256"] == hashlib.sha256(pixels).hexdigest(),
        })

        # Step 2: pencil, opaque erase, and bounded four-connected fill across two clients.
        pencil_index = next(i for i, c in enumerate(pixels) if c not in (0, 2))
        pencil_x, pencil_y = pencil_index % 128, pencil_index // 128
        pencil_status, pencil_reply, pencil_key = action(
            "alice", "pixels",
            {"expected_revision": 1, "x": pencil_x, "y": pencil_y, "color_index": 2})
        require(pencil_status == 200, f"pencil-{pencil_status}")
        pencil_revision = pencil_reply["revision"]
        bob_after_pencil = current_grid("bob")
        record("pencil_and_convergence", {
            "alice": {"status": pencil_status, "revision": pencil_revision,
                      "operation_id": pencil_reply["operation_id"]},
            "bob_frame": frame("bob", 1, bob_after_pencil),
            "converged": bob_after_pencil["revision"] == pencil_revision
            and bob_after_pencil["pixels"][pencil_index] == 2,
        })
        erase_status, erase_reply, erase_key = action(
            "bob", "erase", {"expected_revision": pencil_revision, "x": pencil_x, "y": pencil_y})
        require(erase_status == 200, f"erase-{erase_status}")
        erase_revision = erase_reply["revision"]
        alice_after_erase = current_grid("alice")
        record("opaque_erase_and_convergence", {
            "bob": {"status": erase_status, "revision": erase_revision,
                    "action_id": erase_reply["action_id"], "kind": erase_reply["kind"],
                    "changed_cells": erase_reply["changed_cells"]},
            "alice_frame": frame("alice", pencil_revision, alice_after_erase),
            "converged": alice_after_erase["revision"] == erase_revision
            and alice_after_erase["pixels"][pencil_index] == 0,
        })
        fill_status, fill_reply, fill_key = action(
            "bob", "fill",
            {"expected_revision": erase_revision, "x": fill_seed % 128, "y": fill_seed // 128,
             "color_index": run_colors[0]})
        require(fill_status == 200, f"fill-{fill_status}")
        fill_revision = fill_reply["revision"]
        alice_after_fill = current_grid("alice")
        changed = sorted(i for i, before in enumerate(alice_after_erase["pixels"])
                         if alice_after_fill["pixels"][i] != before)
        record("bounded_fill_and_convergence", {
            "bob": {"status": fill_status, "revision": fill_revision,
                    "action_id": fill_reply["action_id"], "changed_cells": fill_reply["changed_cells"],
                    "kind": fill_reply["kind"]},
            "alice_frame": frame("alice", erase_revision, alice_after_fill),
            "component_cells": len(fill_cells),
            "changed_cells_actual": len(changed),
            "converged": alice_after_fill["revision"] == fill_revision
            and all(alice_after_fill["pixels"][i] == run_colors[0] for i in fill_cells),
            "exact_component": changed == fill_cells,
        })

        # Step 3: cooldown is explicit, leaves no state or charge behind.
        wait_allowed("alice")
        cooldown_key = str(uuid4())
        cool_a, cool_b = background_component[-1], background_component[-2]
        first_status, first_reply, _ = api.request(
            "alice", "POST", f"/api/concert/events/{event}/mural/pixels",
            {"expected_revision": fill_revision, "x": cool_a % 128,
             "y": cool_a // 128, "color_index": 4}, cooldown_key)
        require(first_status == 200, f"cooldown-first-{first_status}")
        cooldown_revision = first_reply["revision"]
        second_status, second_reply, _ = api.request(
            "alice", "POST", f"/api/concert/events/{event}/mural/pixels",
            {"expected_revision": cooldown_revision, "x": cool_b % 128,
             "y": cool_b // 128, "color_index": 5}, str(uuid4()))
        charges_before = sql("mcr_concert", "SELECT count(*) FROM concert_mural_tool_charges WHERE event_id="
                             + prep.quote(event) + " AND actor_id=" + prep.quote(alice))
        state_after_cooldown = current_grid("bob")
        record("cooldown_rejection", {
            "first": {"status": first_status, "revision": cooldown_revision},
            "second": {"status": second_status, "reason": second_reply.get("details", {}).get("reason")},
            "revision_unchanged": state_after_cooldown["revision"] == cooldown_revision,
            "charges": charges_before,
        })

        # Step 4: four-connected fill above 256 is refused with no state change.
        wait_allowed("bob")
        before_large = current_grid("bob")
        large_status, large_reply, _ = action(
            "bob", "fill", {"expected_revision": before_large["revision"], "x": 0, "y": 0,
                            "color_index": 6})
        after_large = current_grid("alice")
        record("fill_too_large_rejection", {
            "request_cells": len(background_component),
            "status": large_status,
            "reason": large_reply.get("details", {}).get("reason"),
            "revision_unchanged": after_large["revision"] == before_large["revision"],
            "state_sha256_equal": after_large["pixels_sha256"] == before_large["pixels_sha256"],
        })

        # Step 5: undo requires the unchanged global revision; a latest own action succeeds.
        fill2_status, fill2_reply, fill2_key = action(
            "bob", "fill", {"expected_revision": after_large["revision"],
                            "x": fill_seed % 128, "y": fill_seed // 128, "color_index": run_colors[1]})
        require(fill2_status == 200, f"undo-fill-{fill2_status}")
        fill2_revision = fill2_reply["revision"]
        wait_allowed("alice")
        bob_move_status, bob_move_reply, _ = api.request(
            "alice", "POST", f"/api/concert/events/{event}/mural/pixels",
            {"expected_revision": fill2_revision, "x": pencil_x,
             "y": pencil_y, "color_index": 4}, str(uuid4()))
        require(bob_move_status == 200, f"foreign-move-{bob_move_status}")
        moved_revision = bob_move_reply["revision"]
        wait_allowed("bob")
        stale_status, stale_reply, _ = api.request(
            "bob", "POST", f"/api/concert/events/{event}/mural/undo",
            {"expected_revision": moved_revision, "target_action_id": fill2_reply["action_id"]},
            str(uuid4()))
        record("undo_revision_guard", {
            "fill": {"status": fill2_status, "revision": fill2_revision},
            "foreign_write": {"status": bob_move_status, "revision": moved_revision},
            "undo": {"status": stale_status,
                     "reason": stale_reply.get("details", {}).get("reason")},
            "revision_unchanged": current_grid("alice")["revision"] == moved_revision,
        })
        wait_allowed("bob")
        before_undo_source = current_grid("alice")
        erase2_status, erase2_reply, _ = action(
            "bob", "erase", {"expected_revision": moved_revision, "x": pencil_x, "y": pencil_y})
        require(erase2_status == 200, f"undo-source-{erase2_status}")
        erase2_revision = erase2_reply["revision"]
        wait_allowed("bob")
        undo_status, undo_reply, _ = api.request(
            "bob", "POST", f"/api/concert/events/{event}/mural/undo",
            {"expected_revision": erase2_revision,
             "target_action_id": erase2_reply["action_id"]}, str(uuid4()))
        after_undo = current_grid("alice")
        record("undo_latest_own_action", {
            "source": {"status": erase2_status, "revision": erase2_revision,
                       "kind": erase2_reply["kind"]},
            "undo": {"status": undo_status, "revision": undo_reply.get("revision"),
                     "kind": undo_reply.get("kind"), "target_action_id": erase2_reply["action_id"]},
            "restored_exact_grid": after_undo["pixels_sha256"] == before_undo_source["pixels_sha256"],
            "revision_advanced": after_undo["revision"] == erase2_revision + 1,
        })

        # Step 6: exhaust this actor's shared 512-cell ten-minute budget.
        budget_reply = None
        for color in (run_colors[2], run_colors[3]):
            status, budget_reply, _ = action(
                "alice", "fill", {"expected_revision": current_grid("alice")["revision"],
                                  "x": fill_seed % 128, "y": fill_seed // 128, "color_index": color})
            require(status == 200, f"budget-fill-{status}")
        wait_allowed("alice")
        budget_pencil = background_component[-3]
        pencil2_status, pencil2_reply, _ = api.request(
            "alice", "POST", f"/api/concert/events/{event}/mural/pixels",
            {"expected_revision": current_grid("alice")["revision"],
             "x": budget_pencil % 128, "y": budget_pencil // 128, "color_index": 6},
            str(uuid4()))
        require(pencil2_status == 200, f"budget-pencil-{pencil2_status}")
        before_budget = current_grid("alice")
        budget_status, budget_error, _ = action(
            "alice", "fill", {"expected_revision": before_budget["revision"],
                              "x": fill_seed % 128, "y": fill_seed // 128,
                              "color_index": run_colors[4]})
        after_budget = current_grid("bob")
        record("shared_cell_budget_rejection", {
            "before_revision": before_budget["revision"],
            "status": budget_status,
            "reason": budget_error.get("details", {}).get("reason"),
            "revision_unchanged": after_budget["revision"] == before_budget["revision"],
            "state_sha256_equal": after_budget["pixels_sha256"] == before_budget["pixels_sha256"],
            "alice_allowance": tools("alice")["remaining_cells"],
        })

        # Step 7: lost response, then authoritative reread and exact-key replay only.
        wait_allowed("bob")
        recovery_revision = current_grid("bob")["revision"]
        recovery_key = str(uuid4())
        recovery_index = background_component[-4]
        recovery_body = {"expected_revision": recovery_revision, "x": recovery_index % 128,
                         "y": recovery_index // 128, "color_index": 7}
        api.send_no_read("bob", f"/api/concert/events/{event}/mural/pixels", recovery_body, recovery_key)
        deadline = time.monotonic() + 10
        status_body = None
        uncommitted = None
        while time.monotonic() < deadline:
            status, status_body, _ = api.request(
                "bob", "GET",
                f"/api/concert/events/{event}/commands?operation_id=placeConcertPixel&idempotency_key={recovery_key}")
            if status == 200:
                break
            time.sleep(0.05)
        if status_body is None or status_body.get("state") != "SUCCEEDED":
            wait_allowed("bob")
            uncommitted_status, uncommitted, _ = api.request(
                "bob", "POST", f"/api/concert/events/{event}/mural/pixels", recovery_body, recovery_key)
            require(uncommitted_status == 200, f"recovery-uncommitted-replay-{uncommitted_status}")
            status, status_body, _ = api.request(
                "bob", "GET",
                f"/api/concert/events/{event}/commands?operation_id=placeConcertPixel&idempotency_key={recovery_key}")
            uncommitted = {"http": uncommitted_status, "outcome": "SAME_KEY_REPLAY_ACCEPTED"}
        require(status_body is not None and status_body.get("state") == "SUCCEEDED",
                "recovery-status-not-succeeded")
        own_status, own, _ = api.request(
            "bob", "GET", f"/api/concert/events/{event}/mural/commands/{recovery_key}/result")
        require(own_status == 200, f"recovery-result-{own_status}")
        authoritative = current_grid("bob")
        charges_after = sql("mcr_concert",
                            "SELECT count(*),coalesce(sum(changed_cells),0) FROM concert_mural_tool_charges WHERE event_id="
                            + prep.quote(event) + " AND actor_id=" + prep.quote(bob))
        replay_key_status, replay, _ = api.request(
            "bob", "POST", f"/api/concert/events/{event}/mural/pixels", recovery_body, recovery_key)
        replay_grid = current_grid("alice")
        charges_after_replay = sql("mcr_concert",
                            "SELECT count(*),coalesce(sum(changed_cells),0) FROM concert_mural_tool_charges WHERE event_id="
                            + prep.quote(event) + " AND actor_id=" + prep.quote(bob))
        record("uncertain_outcome_recovery", {
            "lost_request": {"key": recovery_key, "body": recovery_body,
                             "response": "DISCARDED_AFTER_SEND"},
            "status": {"http": status, "state": status_body.get("state"),
                       "resource_revision": status_body.get("resource_revision")},
            "own_result": {"http": own_status,
                           "revision": own["result"]["revision"],
                           "operation_id": own["result"]["operation_id"]},
            "authoritative_snapshot_revision": authoritative["revision"],
            "same_key_replay": {"http": replay_key_status,
                                "revision": replay.get("revision") if isinstance(replay, dict) else None},
            "uncommitted_replay": uncommitted,
            "no_duplicate_after_replay": replay_grid["revision"] == authoritative["revision"],
            "charges_before_replay": charges_after,
            "charges_after_replay": charges_after_replay,
            "same_receipt": isinstance(replay, dict) and replay.get("revision") == own["result"]["revision"],
            "reposted_with_new_key": False,
        })

        # Step 8: restore the snapshot id that the admin read advertises.
        advertised_revision = current_grid("bob")["revision"]
        wait_allowed("alice")
        advertised_index = background_component[-5]
        advertised_pencil_status, advertised_pencil, _ = api.request(
            "alice", "POST", f"/api/concert/events/{event}/mural/pixels",
            {"expected_revision": advertised_revision, "x": advertised_index % 128,
             "y": advertised_index // 128, "color_index": 9}, str(uuid4()))
        require(advertised_pencil_status == 200, f"advertised-pencil-{advertised_pencil_status}")
        advertised_revision = advertised_pencil["revision"]
        advertised_expected_sha = sql(
            "mcr_concert", "SELECT encode(sha256(pixels),'hex') FROM concert_mural_state WHERE event_id="
            + prep.quote(event))
        advertised_status, advertised_state, _ = api.request(
            "operator", "GET", f"/api/concert/events/{event}/admin/mural")
        require(advertised_status == 200, f"advertised-admin-{advertised_status}")
        advertised_pause_status, advertised_pause, _ = api.request(
            "operator", "POST", f"/api/concert/events/{event}/admin/mural/control",
            {"expected_revision": advertised_state["revision"], "action": "PAUSE",
             "reason": "advertised restore precondition"}, str(uuid4()))
        advertise_restore_status, advertise_restore, _ = api.request(
            "operator", "POST", f"/api/concert/events/{event}/admin/mural/restore",
            {"expected_revision": advertised_pause["revision"],
             "snapshot_id": advertised_state["snapshot_id"],
             "reason": "restore advertised snapshot"}, str(uuid4()))
        advertised_restored = current_grid("alice") if advertise_restore_status == 200 else None
        record("restore_advertised_snapshot", {
            "pencil": {"status": advertised_pencil_status, "revision": advertised_revision},
            "admin_read": {"http": advertised_status,
                           "revision": advertised_state.get("revision"),
                           "snapshot_id": advertised_state.get("snapshot_id")},
            "pause": {"http": advertised_pause_status,
                      "state": advertised_pause.get("state"),
                      "revision": advertised_pause.get("revision")},
            "restore": {"http": advertise_restore_status,
                        "state": advertise_restore.get("state"),
                        "revision": advertise_restore.get("revision"),
                        "reason": advertise_restore.get("details", {}).get("reason"),
                        "target_snapshot_id": advertised_state.get("snapshot_id")},
            "expected_pixels_sha256": advertised_expected_sha,
            "restored_pixels_sha256": None if advertised_restored is None
            else advertised_restored["pixels_sha256"],
            "restored_exact_grid": advertised_restored is not None
            and advertised_restored["pixels_sha256"] == advertised_expected_sha,
        })
        require(advertise_restore_status == 200
                and advertise_restore.get("state") == "PAUSED"
                and advertise_restore.get("revision") == advertised_pause["revision"] + 1
                and advertised_restored is not None
                and advertised_restored["pixels_sha256"] == advertised_expected_sha,
                f"restore-advertised-{advertise_restore_status}")
        advertised_resume_status, advertised_resume, _ = api.request(
            "operator", "POST", f"/api/concert/events/{event}/admin/mural/control",
            {"expected_revision": advertise_restore["revision"], "action": "RESUME",
             "reason": "resume after advertised restore"}, str(uuid4()))
        require(advertised_resume_status == 200, f"advertised-resume-{advertised_resume_status}")

        # Step 9: moderation pause/resume/lock/restore/freeze through the real OPERATOR.
        admin_status, admin_state, _ = api.request(
            "operator", "GET", f"/api/concert/events/{event}/admin/mural")
        require(admin_status == 200, f"admin-state-{admin_status}")
        member_admin_status, member_admin, _ = api.request(
            "bob", "GET", f"/api/concert/events/{event}/admin/mural")
        revision = admin_state["revision"]
        member_pause_status, member_pause, _ = api.request(
            "bob", "POST", f"/api/concert/events/{event}/admin/mural/control",
            {"expected_revision": revision, "action": "PAUSE", "reason": "member attempt"}, str(uuid4()))
        pause_status, pause_reply, _ = api.request(
            "operator", "POST", f"/api/concert/events/{event}/admin/mural/control",
            {"expected_revision": revision, "action": "PAUSE", "reason": "development gate"}, str(uuid4()))
        require(pause_status == 200 and pause_reply["state"] == "PAUSED", "operator-pause")
        paused_revision = pause_reply["revision"]
        blocked_status, blocked, _ = api.request(
            "bob", "POST", f"/api/concert/events/{event}/mural/pixels",
            {"expected_revision": paused_revision, "x": (fill_seed + 9) % 128,
             "y": (fill_seed + 9) // 128, "color_index": 8}, str(uuid4()))
        resume_status, resume_reply, _ = api.request(
            "operator", "POST", f"/api/concert/events/{event}/admin/mural/control",
            {"expected_revision": paused_revision, "action": "RESUME", "reason": "development gate"}, str(uuid4()))
        require(resume_status == 200 and resume_reply["state"] == "EDITABLE", "operator-resume")
        resumed_revision = resume_reply["revision"]
        wait_allowed("bob")
        live_status, live_reply, _ = api.request(
            "bob", "POST", f"/api/concert/events/{event}/mural/pixels",
            {"expected_revision": resumed_revision, "x": (fill_seed + 9) % 128,
             "y": (fill_seed + 9) // 128, "color_index": 8}, str(uuid4()))
        require(live_status == 200, f"resumed-write-{live_status}")
        live_revision = live_reply["revision"]
        lock_status, lock_reply, _ = api.request(
            "operator", "POST", f"/api/concert/events/{event}/admin/mural/locks",
            {"expected_revision": live_revision, "x_min": (fill_seed + 9) % 128, "y_min": (fill_seed + 9) // 128,
             "x_max_exclusive": (fill_seed + 9) % 128 + 1, "y_max_exclusive": (fill_seed + 9) // 128 + 1,
             "locked": True, "reason": "protect development cell"}, str(uuid4()))
        require(lock_status == 200 and len(lock_reply["locks"]) == 1, "operator-lock")
        locked_revision = lock_reply["revision"]
        wait_allowed("alice")
        region_status, region_error, _ = api.request(
            "alice", "POST", f"/api/concert/events/{event}/mural/pixels",
            {"expected_revision": locked_revision, "x": (fill_seed + 9) % 128,
             "y": (fill_seed + 9) // 128, "color_index": 9}, str(uuid4()))
        unlock_status, unlock_reply, _ = api.request(
            "operator", "POST", f"/api/concert/events/{event}/admin/mural/locks",
            {"expected_revision": locked_revision, "x_min": (fill_seed + 9) % 128, "y_min": (fill_seed + 9) // 128,
             "x_max_exclusive": (fill_seed + 9) % 128 + 1, "y_max_exclusive": (fill_seed + 9) // 128 + 1,
             "locked": False, "reason": "release development cell"}, str(uuid4()))
        require(unlock_status == 200 and unlock_reply["locks"] == [], "operator-unlock")
        baseline = current_grid("operator")
        pause2_status, pause2, _ = api.request(
            "operator", "POST", f"/api/concert/events/{event}/admin/mural/control",
            {"expected_revision": unlock_reply["revision"], "action": "PAUSE",
             "reason": "restore baseline"}, str(uuid4()))
        require(pause2_status == 200, "restore-pause")
        restore_status, restore_reply, _ = api.request(
            "operator", "POST", f"/api/concert/events/{event}/admin/mural/restore",
            {"expected_revision": pause2["revision"], "snapshot_id": baseline["snapshot_id"],
             "reason": "named baseline"}, str(uuid4()))
        require(restore_status == 200, f"restore-{restore_status}")
        restored = current_grid("alice")
        resume2_status, resume2, _ = api.request(
            "operator", "POST", f"/api/concert/events/{event}/admin/mural/control",
            {"expected_revision": restore_reply["revision"], "action": "RESUME",
             "reason": "resume after restore"}, str(uuid4()))
        freeze_status, freeze_reply, _ = api.request(
            "operator", "POST", f"/api/concert/events/{event}/admin/mural/control",
            {"expected_revision": resume2["revision"], "action": "FREEZE",
             "reason": "development final freeze"}, str(uuid4()))
        require(freeze_status == 200 and freeze_reply["state"] == "FROZEN", "operator-freeze")
        frozen_revision = freeze_reply["revision"]
        frozen_status, frozen_error, _ = api.request(
            "bob", "POST", f"/api/concert/events/{event}/mural/pixels",
            {"expected_revision": frozen_revision, "x": 1, "y": 1, "color_index": 2}, str(uuid4()))
        terminal_status, terminal_error, _ = api.request(
            "operator", "POST", f"/api/concert/events/{event}/admin/mural/control",
            {"expected_revision": frozen_revision, "action": "RESUME", "reason": "must fail"}, str(uuid4()))
        record("moderation_pause_resume_lock_restore_freeze", {
            "operator_state": {"http": admin_status, "state": admin_state["state"],
                               "revision": admin_state["revision"]},
            "member_admin_read": {"http": member_admin_status,
                                  "reason": member_admin.get("details", {}).get("reason")},
            "member_pause": {"http": member_pause_status,
                             "reason": member_pause.get("details", {}).get("reason")},
            "pause": {"http": pause_status, "state": pause_reply["state"],
                      "revision": pause_reply["revision"]},
            "write_while_paused": {"http": blocked_status,
                                   "reason": blocked.get("details", {}).get("reason")},
            "resume": {"http": resume_status, "state": resume_reply["state"]},
            "write_after_resume": {"http": live_status, "revision": live_revision},
            "lock": {"http": lock_status, "locks": lock_reply["locks"]},
            "write_in_locked_region": {"http": region_status,
                                       "reason": region_error.get("details", {}).get("reason")},
            "unlock": {"http": unlock_status, "locks": unlock_reply["locks"]},
            "restore": {"http": restore_status, "state": restore_reply["state"],
                        "revision": restore_reply["revision"],
                        "snapshot_id": restore_reply["snapshot_id"],
                        "target_snapshot_id": baseline["snapshot_id"],
                        "restored_exact_grid": restored["pixels_sha256"] == baseline["pixels_sha256"]},
            "resume_after_restore": {"http": resume2_status, "state": resume2["state"]},
            "freeze": {"http": freeze_status, "state": freeze_reply["state"],
                       "revision": freeze_reply["revision"]},
            "write_while_frozen": {"http": frozen_status,
                                   "reason": frozen_error.get("details", {}).get("reason")},
            "terminal_frozen_transition": {"http": terminal_status,
                                           "reason": terminal_error.get("details", {}).get("reason")},
        })

        # Step 9: privacy and final convergence.
        def socket_witness():
            async def observe(role):
                from world_capacity_support.transport import Network, WebSocket
                status, boot, _ = api.request(role, "GET", f"/api/concert/events/{event}/bootstrap")
                if status != 200:
                    return {"status": "NOT_RUN", "reason": f"bootstrap-{status}"}
                mapped, avatar = boot["map_manifest"], boot["avatar_manifest"]
                status, rooms, _ = api.request(role, "GET", f"/api/concert/events/{event}/rooms")
                if status != 200:
                    return {"status": "NOT_RUN", "reason": f"rooms-{status}"}
                request = {
                    "room_id": rooms["default_room_id"],
                    "map_id": mapped["map_id"],
                    "map_revision": mapped["map_revision"],
                    "avatar_manifest_id": avatar["manifest_id"],
                    "avatar_manifest_version": avatar["manifest_version"],
                }
                status, ticket, _ = api.request(
                    role, "POST", f"/api/concert/events/{event}/world-tickets", request)
                if status != 200:
                    return {"status": "NOT_RUN", "reason": f"world-ticket-{status}"}
                network = Network({"gateway_url": gateway,
                                   "ws_url": "ws://" + stack.ports.address("concert-service:world") + "/world-ws",
                                   "browser_origin": origin})
                ws = await WebSocket.open(network.ws_target, network.origin)
                try:
                    await ws.send({"protocolVersion": "1.0.0", "type": "authenticate",
                                   "ticket": ticket["ticket"], "eventId": event,
                                   "roomId": request["room_id"],
                                   "mapId": request["map_id"],
                                   "mapRevision": request["map_revision"],
                                   "avatarManifestId": request["avatar_manifest_id"],
                                   "avatarManifestVersion": request["avatar_manifest_version"]})
                    frames = []
                    deadline = time.monotonic() + 5
                    while time.monotonic() < deadline and len(frames) < 8:
                        frame = await asyncio.wait_for(ws.receive(), 5)
                        frames.append({"type": frame.get("type"),
                                       "streamId": frame.get("streamId"),
                                       "roomEpoch": frame.get("roomEpoch")})
                        if frame.get("type") == "snapshot":
                            break
                    return {"status": "PASS", "frames": frames,
                            "counters": ws.counters()}
                finally:
                    await ws.abort()

            try:
                async def both():
                    return {"alice": await observe("alice"), "bob": await observe("bob")}

                return asyncio.run(both())
            except BaseException as failure:  # noqa: BLE001 - non-gating observation
                return {"status": "NOT_RUN", "reason": type(failure).__name__,
                        "close_code": getattr(failure, "code", None)}

        witness = socket_witness()
        record("world_socket_frames", {
            "status": "PASS" if witness.get("alice", {}).get("status") == "PASS"
            and witness.get("bob", {}).get("status") == "PASS" else "NOT_RUN",
            "observation": witness,
            "mural_transport": "HTTP snapshot/delta frames (per-step sync_frames above); the world socket carries avatar/position replication only",
        })

        foreign_action_status, foreign_action, _ = api.request(
            "bob", "GET", f"/api/concert/events/{event}/mural/actions/{pencil_key}")
        foreign_result_status, _, _ = api.request(
            "bob", "GET", f"/api/concert/events/{event}/mural/commands/{pencil_key}/result")
        alice_final, bob_final = current_grid("alice"), current_grid("bob")
        private_keys_snapshot = forbidden_keys(alice_final)
        delta_status, delta_body, _ = api.request(
            "bob", "GET", f"/api/concert/events/{event}/mural/operations?after_revision=1")
        record("privacy_and_final_convergence", {
            "foreign_action_read": {"http": foreign_action_status,
                                    "reason": foreign_action.get("details", {}).get("reason")},
            "foreign_pixel_result_read": {"http": foreign_result_status},
            "snapshot_forbidden_keys": sorted(private_keys_snapshot),
            "delta_forbidden_keys": sorted(forbidden_keys(delta_body)),
            "alice": {"revision": alice_final["revision"], "state": alice_final["state"],
                      "pixels_sha256": alice_final["pixels_sha256"]},
            "bob": {"revision": bob_final["revision"], "state": bob_final["state"],
                    "pixels_sha256": bob_final["pixels_sha256"]},
            "converged": alice_final["revision"] == bob_final["revision"]
            and alice_final["pixels_sha256"] == bob_final["pixels_sha256"],
            "delta_reset_after_moderation": delta_body.get("reset_required"),
        })

        # Durable evidence and honest NOT_RUN records.
        report["db"] = {
            "state": sql("mcr_concert", "SELECT jsonb_build_object('revision',revision,'state',state,"
                         "'snapshot_id',snapshot_id,'reset_revision',reset_revision,"
                         "'pixels_sha256',encode(sha256(pixels),'hex')) FROM concert_mural_state WHERE event_id="
                         + prep.quote(event)),
            "revisions": sql("mcr_concert", "SELECT jsonb_agg(jsonb_build_object('revision',revision,"
                             "'state',state,'snapshot_id',snapshot_id,'pixels_sha256',encode(sha256(pixels),'hex')) ORDER BY revision) "
                             "FROM concert_mural_snapshots WHERE event_id=" + prep.quote(event)),
            "tool_actions": sql("mcr_concert", "SELECT jsonb_agg(jsonb_build_object('revision',revision,"
                               "'kind',kind,'actor_id',actor_id,'changed_cells',changed_cells,"
                               "'undo_expires_at',undo_expires_at,'undoes',undoes) ORDER BY revision) "
                               "FROM concert_mural_tool_actions WHERE event_id=" + prep.quote(event)),
            "charges": sql("mcr_concert", "SELECT jsonb_agg(jsonb_build_object('actor_id',actor_id,"
                          "'revision',(SELECT revision FROM concert_mural_tool_actions a WHERE a.action_id=c.action_id),"
                          "'changed_cells',changed_cells,'committed_at',committed_at) ORDER BY committed_at) "
                          "FROM concert_mural_tool_charges c WHERE event_id=" + prep.quote(event)),
            "correlations": sql("mcr_concert", "SELECT count(*) FROM concert_mural_tool_correlations WHERE event_id="
                                + prep.quote(event)),
            "receipts": sql("mcr_concert", "SELECT jsonb_agg(jsonb_build_object('operation_id',operation_id,"
                           "'state',state,'resource_revision',resource_revision,'failure_reason',failure_reason,"
                           "'actor_id',actor_id) ORDER BY created_at) FROM concert_command_receipts WHERE event_id="
                           + prep.quote(event)),
            "outbox": sql("mcr_concert", "SELECT jsonb_agg(jsonb_build_object('resource_revision',resource_revision,"
                        "'state',state,'kind',kind) ORDER BY created_at) FROM concert_outbox WHERE event_id="
                        + prep.quote(event)),
            "locks": sql("mcr_concert", "SELECT count(*) FROM concert_mural_locks WHERE event_id=" + prep.quote(event)),
        }
        not_run = {
            "undo_wall_clock_expiry": {
                "status": "NOT_RUN",
                "reason": "A real >600 s wait was not performed in this round; the positive in-window "
                          "undo and the revision guard executed. Expiry remains proven by the existing "
                          "native unit cases and is recorded here as not re-run.",
                "consumer": "backend/services/concert-service/src/mural_tools_b/repository.rs undo guard",
            },
            "device_and_visual": {
                "status": "NOT_RUN",
                "reason": "No browser/device surface was exercised; this is a backend multiuser gate.",
                "consumer": "IN04 frontend runner in docs/mcr/qa/in04-wall-b.md",
            },
        }
        socket_step = report["steps"]["world_socket_frames"]
        if socket_step["status"] != "PASS":
            observation = socket_step.get("observation") or {}
            not_run["socket_frames"] = {
                "status": "NOT_RUN",
                "reason": "The world socket did not admit: " + str(observation.get("reason"))
                          + " close_code=" + str(observation.get("close_code")),
                "consumer": "frontend/src/world/network/transport/engine.ts (world socket), "
                            "frontend/src/features/mural/sync/controller.ts (HTTP mural sync)",
            }
        report["not_run"] = not_run
        checks = report["checks"]
        checks["initial_convergence"] = report["steps"]["initial_snapshots"]["converged"]
        checks["pencil_bob_convergence"] = report["steps"]["pencil_and_convergence"]["converged"]
        checks["erase_convergence"] = report["steps"]["opaque_erase_and_convergence"]["converged"]
        checks["fill_convergence"] = report["steps"]["bounded_fill_and_convergence"]["converged"]
        checks["fill_256_or_less"] = report["steps"]["bounded_fill_and_convergence"]["bob"]["changed_cells"] <= 256
        checks["fill_exact_component"] = report["steps"]["bounded_fill_and_convergence"]["exact_component"]
        checks["cooldown_429"] = report["steps"]["cooldown_rejection"]["second"]["status"] == 429
        checks["cooldown_reason"] = report["steps"]["cooldown_rejection"]["second"]["reason"] == "MURAL_COOLDOWN"
        checks["cooldown_no_state_change"] = report["steps"]["cooldown_rejection"]["revision_unchanged"]
        checks["fill_too_large_409"] = report["steps"]["fill_too_large_rejection"]["status"] == 409
        checks["fill_too_large_reason"] = report["steps"]["fill_too_large_rejection"]["reason"] == "MURAL_FILL_TOO_LARGE"
        checks["fill_too_large_no_state_change"] = report["steps"]["fill_too_large_rejection"]["revision_unchanged"]
        checks["undo_guard_409"] = report["steps"]["undo_revision_guard"]["undo"]["status"] == 409
        checks["undo_guard_reason"] = report["steps"]["undo_revision_guard"]["undo"]["reason"] == "MURAL_UNDO_NOT_LATEST"
        checks["undo_success"] = report["steps"]["undo_latest_own_action"]["undo"]["status"] == 200
        checks["undo_restores_grid"] = report["steps"]["undo_latest_own_action"]["restored_exact_grid"]
        checks["budget_429"] = report["steps"]["shared_cell_budget_rejection"]["status"] == 429
        checks["budget_reason"] = report["steps"]["shared_cell_budget_rejection"]["reason"] == "MURAL_BUDGET_EXCEEDED"
        checks["budget_no_state_change"] = report["steps"]["shared_cell_budget_rejection"]["revision_unchanged"]
        checks["recovery_succeeded"] = report["steps"]["uncertain_outcome_recovery"]["status"]["state"] == "SUCCEEDED"
        checks["recovery_no_duplicate"] = report["steps"]["uncertain_outcome_recovery"]["no_duplicate_after_replay"]
        checks["recovery_charges_unchanged"] = report["steps"]["uncertain_outcome_recovery"]["charges_before_replay"] == report["steps"]["uncertain_outcome_recovery"]["charges_after_replay"]
        checks["recovery_same_receipt"] = report["steps"]["uncertain_outcome_recovery"]["same_receipt"]
        pause = report["steps"]["moderation_pause_resume_lock_restore_freeze"]
        checks["member_admin_denied"] = pause["member_admin_read"]["http"] == 403
        checks["member_pause_denied"] = pause["member_pause"]["http"] == 403
        checks["pause_blocks_write"] = pause["write_while_paused"]["http"] == 409
        checks["pause_reason"] = pause["write_while_paused"]["reason"] == "MURAL_PAUSED"
        checks["resume_allows_write"] = pause["write_after_resume"]["http"] == 200
        checks["lock_blocks_write"] = pause["write_in_locked_region"]["http"] == 409
        checks["lock_reason"] = pause["write_in_locked_region"]["reason"] == "MURAL_REGION_LOCKED"
        checks["restore_exact"] = pause["restore"]["restored_exact_grid"]
        checks["restore_advertised_snapshot"] = report["steps"]["restore_advertised_snapshot"]["restore"]["http"] == 200 and report["steps"]["restore_advertised_snapshot"]["restored_exact_grid"]
        checks["freeze_blocks_write"] = pause["write_while_frozen"]["http"] == 409
        checks["freeze_reason"] = pause["write_while_frozen"]["reason"] == "MURAL_FROZEN"
        checks["frozen_terminal"] = pause["terminal_frozen_transition"]["http"] == 409
        checks["foreign_action_denied"] = report["steps"]["privacy_and_final_convergence"]["foreign_action_read"]["http"] in (403, 404)
        checks["no_private_keys"] = not report["steps"]["privacy_and_final_convergence"]["snapshot_forbidden_keys"] \
            and not report["steps"]["privacy_and_final_convergence"]["delta_forbidden_keys"]
        checks["final_convergence"] = report["steps"]["privacy_and_final_convergence"]["converged"]
        checks["world_socket_frames"] = socket_step["status"] == "PASS"
        failed = {name: value for name, value in checks.items() if value is False}
        report["result"] = "PASS" if not failed else "FAIL"
        report["failed_checks"] = failed
        if failed or args.hold_on_failure_seconds > 0:
            if args.hold_on_failure_seconds > 0:
                print("HOLD_BEGIN", flush=True)
                time.sleep(args.hold_on_failure_seconds)
                print("HOLD_END", flush=True)
    except Exception as failure:  # noqa: BLE001 - owned development driver
        report["result"] = "FAIL"
        report["failure"] = f"{type(failure).__name__}: {failure}"
        if stack.cluster is not None:
            report["logs"] = closure.log_excerpts(stack)
    finally:
        try:
            stack.close()
            report["cleanup"] = "PASS"
        except Exception as failure:  # noqa: BLE001
            report["cleanup"] = f"FAIL: {type(failure).__name__}"
        if api is not None:
            report["http_requests"] = api.log
        args.report.write_text(json.dumps(report, indent=2))
    return 0 if report.get("result") == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
