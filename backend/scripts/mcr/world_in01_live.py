#!/usr/bin/env python3
"""Owned real-Auth, mTLS C03 and optional World startup; no fixture principals."""
from __future__ import annotations

import argparse
import asyncio
import hashlib
from datetime import datetime, timedelta, timezone
import importlib.util
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import signal
import socket
import subprocess
import threading
import time
from unittest.mock import patch
from uuid import UUID, uuid4

import dev_stack as dev
import participant_live_join_smoke as joined
import world_in01_prepare as prep

ROOT = prep.ROOT
CLEANUP_RESERVE_SECONDS = 240  # Covers bounded direct-child stops plus both PG stop attempts.


class QualificationGuard:
    """Fresh offer bounds over retained carry plus only this producer's outputs."""
    def __init__(self, args, stack):
        self.stack, self.output = stack, args.report.parent
        info = self.output.lstat()
        prep.auth.require(self.output.is_dir() and not self.output.is_symlink()
                          and info.st_uid == os.getuid() and info.st_mode & 0o077 == 0, "qualification-private-output")
        self.identity = info.st_dev, info.st_ino
        self.started_monotonic = time.monotonic()
        self.deadline = self.started_monotonic + args.whole_timeout_seconds - CLEANUP_RESERVE_SECONDS
        self.hard_deadline = self.deadline + CLEANUP_RESERVE_SECONDS
        self.started_at = datetime.now(timezone.utc)
        self.work_deadline_at = self.started_at + timedelta(seconds=args.whole_timeout_seconds - CLEANUP_RESERVE_SECONDS)
        self.hard_deadline_at = self.started_at + timedelta(seconds=args.whole_timeout_seconds)
        self.phase_history, self.phase_deadline = [], self.deadline
        self.process_peak = 0
        self.limit, self.carry = args.whole_byte_limit, args.retained_carry_bytes
        self.maximum, self.failure = self.carry, None
        self.stop_event, self.thread = threading.Event(), None

    def phase(self, name, seconds):
        now = time.monotonic()
        self.phase_deadline = min(self.deadline, now + seconds)
        self.phase_history.append({"name": name, "started_at": datetime.now(timezone.utc).isoformat(),
            "deadline_at": (datetime.now(timezone.utc) + timedelta(seconds=max(0, self.phase_deadline-now))).isoformat()})

    def process_check(self):
        # Only PID/parent metadata is read, never environment or command arguments.
        rows = subprocess.run(["/bin/ps", "-axo", "pid=,ppid="], capture_output=True,
                              text=True, timeout=2, check=True).stdout.splitlines()
        parents = {int(row.split()[0]): int(row.split()[1]) for row in rows if len(row.split()) == 2}
        owned = {os.getpid()}
        if self.stack.cluster is not None:
            cluster = self.stack.cluster
            cluster.assert_owned()
            if cluster.started and (cluster.data / "postmaster.pid").exists():
                owned.add(cluster.assert_postmaster())
        while True:
            more = {pid for pid, parent in parents.items() if parent in owned} - owned
            if not more: break
            owned.update(more)
        self.process_peak = max(self.process_peak, len(owned))
        prep.auth.require(len(owned) <= 32, "qualification-owned-process-cap")

    def check(self):
        info = self.output.lstat()
        prep.auth.require((info.st_dev, info.st_ino) == self.identity and not self.output.is_symlink(), "qualification-output-replaced")
        roots = [self.output]
        if self.stack.cluster is not None:
            self.stack.cluster.assert_owned()
            if not self.stack.cluster.root.is_relative_to(self.output):
                roots.append(self.stack.cluster.root)
        total = self.carry
        for root in roots:
            for directory, folders, files in os.walk(root, followlinks=False):
                try:
                    total += Path(directory).lstat().st_blocks * 512
                except FileNotFoundError:
                    continue
                for name in folders + files:
                    path = Path(directory) / name
                    try:
                        info = path.lstat()
                    except FileNotFoundError:
                        continue  # WAL/log rotation; the next sample observes the replacement.
                    prep.auth.require(not path.is_symlink(), "qualification-output-symlink")
                    if name in files:
                        total += info.st_blocks * 512
        self.maximum = max(self.maximum, total)
        prep.auth.require(total <= self.limit, "qualification-whole-byte-cap")
        prep.auth.require(time.monotonic() < self.deadline, "qualification-work-deadline")
        prep.auth.require(time.monotonic() < self.phase_deadline, "qualification-phase-deadline")
        prep.auth.require(shutil.disk_usage(self.output).free >= 8 * 1024**3, "qualification-free-space")
        self.process_check()

    def start(self):
        prep.auth.require(shutil.disk_usage(self.output).free >= 16 * 1024**3, "qualification-start-free-space")
        self.phase("native-startup", 180)
        self.check()
        def watch():
            while not self.stop_event.wait(.5):
                try:
                    self.check()
                except BaseException:
                    self.failure = "qualification-resource-bound"
                    os.kill(os.getpid(), signal.SIGTERM)  # This producer only; direct children clean up in finally.
                    return
        self.thread = threading.Thread(target=watch, daemon=True)
        self.thread.start()

    def close(self):
        self.stop_event.set()
        if self.thread is not None:
            self.thread.join(timeout=1)
        return {"max_sampled_whole_bytes": self.maximum, "hard_whole_bytes": self.limit,
                "retained_carry_bytes": self.carry, "cleanup_reserve_seconds": CLEANUP_RESERVE_SECONDS,
                "start_free_floor_bytes": 16 * 1024**3, "continuous_stop_floor_bytes": 8 * 1024**3,
                "owned_process_cap": 32, "max_sampled_owned_processes": self.process_peak,
                "started_at": self.started_at.isoformat(),
                "work_deadline_at": self.work_deadline_at.isoformat(),
                "hard_deadline_at": self.hard_deadline_at.isoformat(),
                "phase_cutoffs": self.phase_history,
                "failure": self.failure, "sample_interval_seconds": .5}


def qualified_release():
    """This reusable binary cohort admits Map4/static1, never V10/Map6."""
    bootstrap = json.loads((prep.RELEASE / "bootstrap.json").read_bytes())
    prep.auth.require((bootstrap["map_manifest"]["map_id"], bootstrap["map_manifest"]["map_revision"],
                       bootstrap["avatar_manifest"]["manifest_id"], bootstrap["avatar_manifest"]["manifest_version"])
                      == ("mcr_reference_scene_r3", 4, "mcr_selected_static_20261103", 1), "qualification-release-pin")
    prep.release_inputs()


def qualified_binaries(args):
    """Read and verify retained qualified objects, never execute their metadata."""
    manifest = args.bin_dir.parent / "cohort.json"
    prep.auth.require(not manifest.is_symlink() and dev.runtime.digest(manifest)
                      == "dd43f3f629a958f3b126bacc039f6d3413f012b4e42c7f59528b0f3e617b8da5", "qualification-peer-cohort")
    cohort = json.loads(manifest.read_bytes())
    for name, row in cohort.items():
        path = args.bin_dir / name
        prep.auth.require(not path.is_symlink() and path.is_file() and path.stat().st_size == row["size"]
                          and dev.runtime.digest(path) == row["sha256"], "qualification-peer-object")
    prep.auth.require(not args.concert_bin.is_symlink() and args.concert_bin.is_file()
                      and dev.runtime.digest(args.concert_bin) == "b1ea0a07eab66aa2c1c313476b063b61cf54ee678ad0a0e44633944815f65433", "qualification-concert-object")
    return {"peers": cohort, "cohort_manifest_sha256": dev.runtime.digest(manifest),
            "concert_sha256": dev.runtime.digest(args.concert_bin),
            "concert_compiled_source": "7cd1e900d6bd1b4994482819486c828e43736822"}


def qualification_config(args, descriptor, context, principals, bootstrap, ws_url):
    """One in-process handoff. Never serialize this object: it contains bearers."""
    from world_capacity_support.scenarios import validate_config
    spec = prep.producer_profile(args.qualification_profile)
    prep.auth.require(spec["name"] != "in01" and len(principals) == spec["principals"], "qualification-cohort")
    seconds = args.measurement_seconds
    capacity = spec["name"] == "qa08"
    config = {"allocation_id": args.allocation_id, "gateway_url": descriptor["gateway_url"],
              "ws_url": ws_url, "browser_origin": args.browser_origin, "event_id": context["event"],
              "bootstrap": bootstrap, "rooms": context["rooms"],
              "principals": [{"bearer": token, "room_index": n // 50 if capacity else 0}
                             for n, (_, _, token) in enumerate(principals)],
              "schedule": {"arrival_concurrency": min(args.arrival_concurrency, len(principals)),
                           "operation_timeout_seconds": 60 if args.restart_world else 20,
                           "measurement_seconds": seconds, "total_timeout_seconds": args.probe_timeout_seconds,
                           "input_hz": args.input_hz, "http_interval_seconds": .12 if capacity else 0,
                           "reconnect": [{"after_seconds": seconds / 4, "participants": list(range(len(principals)))}],
                           "slow_reader": {"participant_index": len(principals) - 1,
                                           "after_seconds": seconds / 2, "pause_seconds": min(2, seconds / 4)}}}
    if not capacity:
        config["faults"] = ["ticket_replay", "application_gap", "slow_receiver", "room_switch"]
        if args.restart_world:
            config["faults"].append("world_restart")
    validate_config(config, capacity)
    return config


def membership_observation(cluster, event):
    """Actual owned PG observation after probe cleanup; no hardcoded ticket count."""
    cluster.assert_owned()
    event = prep.quote(str(UUID(event)))
    deadline = time.monotonic() + 3
    while True:
        memberships = int(cluster.sql("mcr_concert", "SELECT count(*) FROM concert_world_memberships WHERE event_id=" + event))
        prepared = int(cluster.sql("postgres", "SELECT count(*) FROM pg_prepared_xacts"))
        if memberships == 0 and prepared == 0 or time.monotonic() >= deadline:
            return {"memberships": memberships, "prepared_transactions": prepared}
        time.sleep(.05)


def retain_public_projection(browser, report, event, principal, directory, provenance=None):
    """Validate full C03 only in memory; retain an explicitly scoped public projection."""
    from world_capacity_support.transport import Contracts
    from world_capacity_support.report import json_value, source_pins
    observed_at = datetime.now(timezone.utc).isoformat()
    raw = public_json(browser, report, "post-admission-bootstrap-retention",
                      f"/api/concert/events/{event}/bootstrap", principal[2], response_bytes=True)
    value = json_value(raw)
    prep.auth.require(Contracts().bootstrap(value, event) == principal[0], "retained-bootstrap-identity")
    expected = json.loads((prep.RELEASE / "bootstrap.json").read_bytes())
    prep.auth.require(value["event"]["phase"] == "OPEN"
                      and all(value[key] == expected[key] for key in ("map_manifest", "avatar_manifest", "theme_manifests")), "retained-bootstrap-resource-pins")
    forbidden = {"accesstoken", "access_token", "refreshtoken", "refresh_token", "password",
                 "email", "bearer", "ticket", "authorization", "cookie", "sessionid", "session_id"}
    def inspect(node, public=False):
        if isinstance(node, dict):
            denied = forbidden | ({"participant", "identity", "profile", "preferences", "age_status", "user_id", "userid"} if public else set())
            prep.auth.require(not any(k.lower() in denied for k in node), "retained-bootstrap-credential-field")
            for child in node.values(): inspect(child, public)
        elif isinstance(node, list):
            for child in node: inspect(child, public)
        elif public and isinstance(node, str):
            prep.auth.require(not re.search(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", node, re.I), "projection-uuid")
    inspect(value)
    prep.auth.require(principal[2].encode() not in raw and len(raw) <= 2_000_000, "retained-bootstrap-private-bound")
    projection = {"kind": "mcr-public-resource-capability-projection", "fidelity": "NOT_BYTE_EXACT_FULL_C03",
                  "contract_version": value["contract_version"],
                  "resources_are_complete_manifests": False,
                  "map_descriptor": {key: value["map_manifest"][key] for key in ("map_id", "map_revision", "content_hash", "geometry_hash", "tilemap")},
                  "avatar_descriptor": value["avatar_manifest"],
                  "theme_descriptors": [{key: item for key, item in theme.items() if key != "place_ids"} for theme in value["theme_manifests"]],
                  "capabilities": value["capabilities"]}
    inspect(projection, True)
    projected = json.dumps(projection, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()
    artifact = directory / "public-resource-capability-projection.json"
    dev.runtime.private_file(artifact, projected)
    return {"status": "RETAINED_PUBLIC_RESOURCE_CAPABILITY_PROJECTION", "filename": artifact.name,
            "fidelity": "NOT_BYTE_EXACT_FULL_C03", "projection_sha256": hashlib.sha256(projected).hexdigest(),
            "projection_bytes": len(projected), "full_body_retained_or_hashed": False,
            "contains_credentials": False, "contains_participant_identity_preferences_age": False,
            "visibility": "MODE_0600_PUBLIC_SCOPE_ONLY", "capabilities_modified": False,
            "http_status": 200, "observed_at": observed_at, "contract_version": value["contract_version"],
            "contract_asset_validation": "PASS", "privacy": "PUBLIC_RESOURCES_AND_CAPABILITY_OUTCOMES_ONLY",
            "map_content_hash": value["map_manifest"]["content_hash"],
            "geometry_hash": value["map_manifest"]["geometry_hash"],
            "custody": "PRODUCER_RETAINS_OUTSIDE_EPHEMERAL_PG_UNTIL_CONSUMER_READBACK_OR_TRANSFER",
            "contract_source_pins": {p: h for p, h in source_pins().items() if p.startswith("contracts/") or p.endswith("mcr-core.openapi.json")},
            "source_provenance": provenance or {"status": "NOT_SUPPLIED_FIXTURE_ONLY"}}


def qualification_run(config, stack, restart_hook=None):
    """Keep all tokens inside the producer/probe process and return safe evidence."""
    from world_capacity_support.scenarios import run as probe
    async def observe():
        return membership_observation(stack.cluster, config["event_id"])
    result = asyncio.run(probe(config, capacity=len(config["principals"]) == 200,
                              restart_hook=restart_hook, observation_hook=observe))
    oracle = result["server_metrics_and_cleanup_oracle"]
    if oracle.get("status") != "OBSERVED_PRODUCER_HOOK" or oracle.get("memberships") != 0 or oracle.get("prepared_transactions") != 0:
        result.update(result="FAIL", failure="producer-cleanup-oracle")
    return result


class RestartPorts(dev.runtime.Ports):
    """Reserve just-released owned addresses, allowing ordinary TCP TIME_WAIT."""
    def reserve(self, name, port=0):
        prep.auth.require(name.startswith("concert-service:") and name not in self.ports
                          and type(port) is int and 0 < port < 65536
                          and port not in dev.runtime.DEFAULT_PORTS, "qualification-restart-port")
        handle = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            handle.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            handle.bind(("127.0.0.1", port))
            self.held[name], self.ports[name] = handle, port
            self.used.add(port)
            return port
        except BaseException:
            handle.close()
            raise


def restart_owned_concert(stack, child, env, binary, names, records):
    """Stop only our direct child, reserve its released ports anew, then restart."""
    stack.cluster.assert_owned()
    before = child.process.pid
    stopped = child.stop()
    prep.auth.require(stopped.get("exit_code") == 0 and not stopped.get("forced")
                      and not stopped.get("unexpected_exit"), "qualification-restart-stop")
    ports, replacement = RestartPorts(), None
    fresh_env = {**env, "CONCERT_WORLD_OWNER_ID": str(uuid4()),
                 "CONCERT_PRODUCER_INSTANCE_ID": str(uuid4())}
    fresh_env["CONCERT_WORLD_INVALIDATION_CONSUMER_ID"] = fresh_env["CONCERT_WORLD_OWNER_ID"]
    try:
        for name in names:
            ports.reserve(name, stack.ports.ports[name])
        with dev.runtime.protected_signals():
            replacement = dev.runtime.Child(stack.cluster, "concert-service", [str(binary)],
                fresh_env, ports, names, tuple(fresh_env.values()))
        replacement.wait_listeners()
        status, _ = dev.runtime.http_probe(replacement, "metrics", "/readyz")
        prep.auth.require(status == 200 and replacement.process.pid != before, "qualification-restart-ready")
        records.append({"before_pid": before, "after_pid": replacement.process.pid,
                        "stop": stopped, "ready_status": status})
        return replacement, fresh_env
    except BaseException:
        if replacement is not None:
            records.append({"failed_replacement_cleanup": replacement.stop()})
        raise
    finally:
        ports.close()


def pki(cluster):
    joined.pki(cluster)
    directory = cluster.root / "pki"
    for role in ("post", "notification"):
        extension = directory / (role + ".ext")
        extension.write_text("basicConstraints=critical,CA:FALSE\n"
            "keyUsage=critical,digitalSignature,keyEncipherment\n"
            "extendedKeyUsage=serverAuth,clientAuth\n"
            f"subjectAltName=DNS:{role}.svc.internal,URI:spiffe://internal/{role}\n")
        for arguments in (
            ["req", "-new", "-newkey", "rsa:2048", "-nodes", "-keyout", role + ".key",
             "-out", role + ".csr", "-subj", "/CN=" + role + ".svc.internal"],
            ["x509", "-req", "-in", role + ".csr", "-CA", "ca.crt", "-CAkey", "ca.key",
             "-CAcreateserial", "-out", role + ".crt", "-days", "2", "-extfile", str(extension)],
        ):
            dev.migration.execute(["/usr/bin/openssl", *arguments], env=cluster.env,
                                  cwd=directory, phase="in01-private-pki")
        (directory / (role + ".key")).chmod(0o600)


def tls(cluster, role):
    return {f"{role.upper()}_GRPC_TLS_{suffix}_PATH": str(cluster.root / "pki" / name)
            for suffix, name in (("CERT", role + ".crt"), ("KEY", role + ".key"), ("CA", "ca.crt"))}


def c03_database_checks(cluster, principals, report):
    cluster.assert_owned()
    for user, _, _ in principals:
        expected = "C03 runtime " + user[:8]
        count = cluster.sql("mcr_profile", "SELECT count(*) FROM profiles WHERE user_id="
            + prep.quote(user) + " AND nickname=" + prep.quote(expected) + " AND bio=''")
        prep.auth.require(count == "1", "canonical-profile-c03-write")
    checks = {
        "root_delivery_16": ("mcr_concert", "SELECT count(*) FROM concert_product_owner_scopes "
            "WHERE phase='ROOT_FINISHED' AND delivery_completed_at IS NOT NULL", "16"),
        "profile_commit_2": ("mcr_concert", "SELECT count(*) FROM concert_participant_profile_terminal_receipts "
            "WHERE outcome='COMMITTED'", "2"),
        "concert_native_commit_2": ("mcr_concert", "SELECT count(*) FROM concert_participant_profile_operations "
            "WHERE decision='COMMIT' AND pg_xact_status(concert_full_xid::xid8)='committed'", "2"),
        "auth_settlement_16": ("mcr_auth", "SELECT count(*) FROM concert_owner_guard_leases "
            "WHERE scope_settlement IS NOT NULL AND state='COMPLETED'", "16"),
        "friend_settlement_16": ("mcr_friend", "SELECT count(*) FROM concert_owner_guard_leases "
            "WHERE scope_settlement IS NOT NULL AND state='COMPLETED'", "16"),
        "auth_pending_participants_0": ("mcr_auth", "SELECT count(*) FROM "
            "concert_owner_guard_participants WHERE state='PENDING'", "0"),
        "friend_pending_participants_0": ("mcr_friend", "SELECT count(*) FROM "
            "concert_owner_guard_participants WHERE state='PENDING'", "0"),
        "no_prepared_transactions": ("postgres", "SELECT count(*) FROM pg_prepared_xacts", "0"),
    }
    deadline = time.monotonic() + 3
    while True:
        observed = {label: cluster.sql(database, query)
                    for label, (database, query, _) in checks.items()}
        report["c03_database_observed"] = observed
        if all(observed[label] == expected for label, (_, _, expected) in checks.items()):
            break
        if time.monotonic() >= deadline:
            report["root_phase_counts"] = json.loads(cluster.sql("mcr_concert",
                "SELECT coalesce(json_agg(x),'[]') FROM (SELECT phase,"
                "serialized_at IS NOT NULL AS serialized,delivery_completed_at IS NOT NULL AS completed,"
                "count(*) FROM concert_product_owner_scopes GROUP BY 1,2,3) x"))
            for label, (_, _, expected) in checks.items():
                prep.auth.require(observed[label] == expected, label)
        time.sleep(0.05)
    return {label: "PASS" for label in checks}


def world_database_checks(cluster, event, report):
    cluster.assert_owned()
    event = prep.quote(event)
    checks = {
        "issued_tickets_2": ("mcr_concert", "SELECT count(*) FROM concert_world_tickets "
            "WHERE event_id=" + event, "2"),
        "consumed_tickets_2": ("mcr_concert", "SELECT count(*) FROM concert_world_tickets "
            "WHERE event_id=" + event + " AND consumed_at IS NOT NULL", "2"),
        "released_memberships_0": ("mcr_concert", "SELECT count(*) FROM concert_world_memberships "
            "WHERE event_id=" + event, "0"),
        "no_prepared_transactions": ("postgres", "SELECT count(*) FROM pg_prepared_xacts", "0"),
    }
    deadline = time.monotonic() + 3
    while True:
        observed = {label: cluster.sql(database, query)
                    for label, (database, query, _) in checks.items()}
        report["world_database_observed"] = observed
        if all(observed[label] == expected for label, (_, _, expected) in checks.items()):
            return {label: "PASS" for label in checks}
        if time.monotonic() >= deadline:
            for label, (_, _, expected) in checks.items():
                prep.auth.require(observed[label] == expected, "world-" + label)
        time.sleep(0.05)


def run(args):
    profile = prep.producer_profile(getattr(args, "qualification_profile", "in01"))["name"]
    if profile != "in01":
        validate_qualification_arguments(args)
        qualified_release()
        binary_provenance = qualified_binaries(args)
    fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    report = {"result": "FAIL", "scope": "actual real-Auth mTLS C03/World startup",
              "statuses": {}, "world_two_client": "NOT_RUN", "browser_in01": "NOT_RUN",
              "driver_sha256": dev.runtime.digest(Path(__file__))}
    report["producer_profile"] = profile
    if profile != "in01":
        report["qualified_binary_cohort"] = binary_provenance
    report["gateway_admission"] = {"mode": "enforce", "provisioning_budget":
        args.qa08_provisioning_budget if profile == "qa08" else "UNCHANGED_DEFAULTS"}
    stack = dev.DevStack(args.bin_dir, Path("/opt/homebrew/bin"),
        Path("/opt/homebrew/bin/redis-server"), Path("/opt/homebrew/bin/openssl"), args.browser_origin)
    child = None
    guard = None
    context = {}
    original = dev.development_environments

    def environments(cluster, ports, keys, credentials, *rest):
        # The retained helper validates its unchanged HTTP endpoints first.
        credentials["CONCERT_SERVICE_CREDENTIAL"] = secrets.token_hex(32)
        envs = original(cluster, ports, keys, credentials, *rest)
        if profile == "qa08":
            envs["api-gateway"].update(GATEWAY_ADMISSION_BUDGETS=args.qa08_provisioning_budget,
                                      GATEWAY_ADMISSION_MODE="enforce")
        cluster.assert_owned()
        cluster.stop()
        with (cluster.data / "postgresql.conf").open("a") as stream:
            stream.write("\nmax_prepared_transactions=128\n")
        cluster.start()
        prep.auth.require(cluster.sql("postgres", "SHOW max_prepared_transactions") == "128",
                          "owned-prepared-transaction-config")
        pki(cluster)
        relationship, self_profile, decision = joined.keys(cluster, 3)
        for name in ("concert-service:grpc", "concert-service:metrics"):
            ports.reserve(name)
        if args.world:
            ports.reserve("concert-service:world")
        urls = {role: "https://" + ports.address(role + "-service:grpc")
                for role in ("auth", "friend", "profile", "concert")}
        for name, env in envs.items():
            for key, value in list(env.items()):
                if key.endswith("_GRPC_ENDPOINT"):
                    prep.auth.require(value.startswith("http://127.0.0.1:"), "owned-pre-tls-endpoint")
                    env[key] = value.replace("http://", "https://", 1)
            env.update(tls(cluster, dev.runtime.PREFIX[name].lower()))
            env.update(CONCERT_RELATIONSHIP_VERIFYING_KEYS=relationship[1])
        for role in ("auth", "friend"):
            envs[role + "-service"].update(CONCERT_OWNER_AUTHORITY_ENABLED="true",
                AUTHORITY_CONCERT_SETTLEMENT_ENDPOINT=urls["concert"],
                AUTHORITY_PROFILE_SETTLEMENT_ENDPOINT=urls["profile"])
        envs["profile-service"].update(CONCERT_PROFILE_SELF_VERIFYING_KEYS=self_profile[1],
            PROFILE_PARTICIPANT_DECISION_SIGNING_SEED=decision[0],
            PROFILE_SELF_AUTH_GRPC_ENDPOINT=urls["auth"], PROFILE_SELF_FRIEND_GRPC_ENDPOINT=urls["friend"],
            PROFILE_SELF_CONCERT_GRPC_ENDPOINT=urls["concert"])
        inputs_path = prep.seed_concert(stack, args.concert_bin, report, True, profile=profile)
        inputs = json.loads(inputs_path.read_bytes())
        release = inputs.pop("release_inputs")
        inputs.pop("world_activation")
        world_owner = inputs.pop("CONCERT_WORLD_OWNER_ID")
        rooms = inputs.pop("CONCERT_WORLD_ROOMS_JSON")
        event = inputs.pop("CONCERT_WORLD_EVENT_ID")
        env = {**cluster.env, **credentials, **inputs, **tls(cluster, "concert"),
            **{k: v for k, v in release.items() if k.startswith("CONCERT_")},
            "ACTOR_DELEGATION_VERIFYING_KEYS": keys["ACTOR_DELEGATION_VERIFYING_KEYS"],
            "CONCERT_GRPC_ADDR": ports.address("concert-service:grpc"),
            "CONCERT_METRICS_ADDR": ports.address("concert-service:metrics"),
            "CONCERT_RELATIONSHIP_SIGNING_SEED": relationship[0],
            "CONCERT_PROFILE_SELF_SIGNING_SEED": self_profile[0],
            "PROFILE_PARTICIPANT_DECISION_VERIFYING_KEYS": decision[1],
            "CONCERT_PARTICIPANT_MODE": "live", "CONCERT_PRODUCER_INSTANCE_ID": str(uuid4()),
            "CONCERT_PARTICIPANT_RECOVERY_KEYS_JSON": json.dumps([
                {"id": "in01_active", "secret_hex": secrets.token_hex(32)}]),
            "TOKIO_WORKER_THREADS": "2", "RUST_LOG": "warn"}
        for role in ("auth", "friend", "profile"):
            env["CONCERT_PARTICIPANT_" + role.upper() + "_GRPC_ENDPOINT"] = urls[role]
        if args.world:
            env.update(CONCERT_WORLD_LISTEN_ADDR=ports.address("concert-service:world"),
                CONCERT_WORLD_ALLOWED_ORIGINS=json.dumps([args.browser_origin]),
                CONCERT_WORLD_OWNER_ID=world_owner, CONCERT_WORLD_INVALIDATION_CONSUMER_ID=world_owner,
                CONCERT_WORLD_EVENT_ID=event, CONCERT_WORLD_ROOMS_JSON=rooms,
                CONCERT_WORLD_RESOURCE_ROOT=release["asset_root"],
                CONCERT_WORLD_SOURCE_BINDING_FILE=release["source_binding"])
            for role in ("auth", "friend", "profile"):
                env["CONCERT_" + role.upper() + "_GRPC_ENDPOINT"] = urls[role]
        inspected = dev.migration.execute([str(args.concert_bin), "--inspect-product-release"],
            env=env, cwd=cluster.root, phase="exact-active-release-inspection")
        match = re.fullmatch(r"CONCERT_PRODUCT_RELEASE_DIGEST=([0-9a-f]{64})\n?", inspected.stdout)
        prep.auth.require(match is not None, "actual-release-inspection")
        digest = match[1]
        env["CONCERT_PARTICIPANT_RELEASE_DIGEST"] = digest
        envs["api-gateway"].update(GATEWAY_CONCERT_GRPC_ENDPOINT=urls["concert"],
                                  GATEWAY_CONCERT_RELEASE_DIGEST=digest)
        context.update(env=env, event=event, rooms=json.loads(rooms), release=release, digest=digest)
        return envs

    previous = {sig: signal.signal(sig, lambda *_: (_ for _ in ()).throw(KeyboardInterrupt()))
                for sig in (signal.SIGTERM, signal.SIGINT)}
    try:
        if profile != "in01":
            guard = QualificationGuard(args, stack)
            guard.start()
        with patch.object(dev.runtime, "readiness_value", prep.current_readiness), \
                patch.object(dev, "development_environments", environments):
            descriptor = stack.start()
        browser = prep.auth.Browser(descriptor["gateway_url"], report)
        # Keep HTTP connection target separate from the actual browser Origin.
        browser.origin = args.browser_origin
        if guard: guard.phase("real-auth-provisioning", args.provision_timeout_seconds)
        principals = prep.provision_principals(stack, browser, profile, getattr(args, "provision_timeout_seconds", 120))
        report["provisioned_principals"] = len(principals)
        # The retained public helper addresses browser.origin; supply its actual
        # Gateway target while preserving the browser header through a wrapper.
        if guard: guard.phase("profile-and-concert-startup", 60)
        initialize_profiles(browser, report, principals)
        names = ["concert-service:grpc", "concert-service:metrics"]
        if args.world:
            names.append("concert-service:world")
        env = context["env"]
        child = dev.runtime.Child(stack.cluster, "concert-service", [str(args.concert_bin)],
            env, stack.ports, names, tuple(env.values()))
        child.wait_listeners()
        status, _ = dev.runtime.http_probe(child, "metrics", "/readyz")
        prep.auth.require(status == 200, "actual-concert-readiness")
        for ordinal, (user, _, token) in enumerate(principals[:1] if profile != "in01" else principals, 1):
            value = public_json(browser, report, f"principal-{ordinal}-bootstrap",
                f"/api/concert/events/{context['event']}/bootstrap", token)
            prep.auth.require(value["participant"]["user_id"] == user, "actual-bootstrap-owner")
        if profile != "in01":
            guard.phase("original-qa05-qa08-probe", args.probe_timeout_seconds)
            config = qualification_config(args, descriptor, context, principals, value,
                "ws://" + stack.ports.address("concert-service:world") + "/world-ws")
            report["qualification_restarts"] = []
            async def restart():
                nonlocal child, env
                child, env = restart_owned_concert(stack, child, env, args.concert_bin, names,
                                                   report["qualification_restarts"])
                await asyncio.sleep(0)
            report["qualification"] = qualification_run(config, stack, restart if args.restart_world else None)
            report["result"] = "PARTIAL" if report["qualification"]["result"] == "COMPLETED" else "FAIL"
            report["acceptance"] = "NOT_CLAIMED"
            report["token_handoff"] = "IN_PROCESS_MEMORY_ONLY"
            report["descriptor"] = {"producer_profile": profile, "principal_count": len(principals),
                                    "room_capacities": [r["capacity"] for r in context["rooms"]],
                                    "expires_on": "OWNED_SHUTDOWN", "contains_tokens": False}
            if args.retain_public_projection and report["qualification"]["result"] == "COMPLETED":
                guard.phase("public-projection", 20)
                report["post_admission_projection"] = retain_public_projection(browser, report, context["event"],
                    principals[0], args.report.parent, {"producer_source_revision": stack.report["source_revision"],
                        "backend_runtime": binary_provenance, "frontend_runtime": "NOT_RUN",
                        "frontend_consumer_source_revision": args.frontend_source_revision or "NOT_SUPPLIED"})
        else:
            path = ROOT / "backend/services/api-gateway/tests/concert_participant_support/public_probe.py"
            spec = importlib.util.spec_from_file_location("in01_c03_public_probe", path)
            probe = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(probe)
            report["public_c03"] = probe.run_pair(descriptor["gateway_url"], context["event"],
                args.browser_origin, [{"user_id": user, "access_token": token} for user, _, token in principals])
            report["c03_database"] = c03_database_checks(stack.cluster, principals, report)
            if args.world:
                from world_two_principal_probe import run_probe
                report["world_probe"] = run_probe(descriptor["gateway_url"], args.browser_origin,
                    context["event"], context["rooms"][0]["room_id"],
                    [token for _, _, token in principals],
                    "ws://" + stack.ports.address("concert-service:world") + "/world-ws", value)
                report["world_two_client"] = report["world_probe"]["result"]
                prep.auth.require(report["world_two_client"] == "PASS", "actual-world-two-client")
                report["world_database"] = world_database_checks(stack.cluster, context["event"], report)
            private = {"gateway_url": descriptor["gateway_url"], "browser_origin": args.browser_origin,
                "event_id": context["event"], "rooms": context["rooms"],
                "release_digest": context["digest"], "release_inputs": context["release"],
                "principals": [str(path) for _, path, _ in principals],
                "world_ws_url": ("ws://" + stack.ports.address("concert-service:world") + "/world-ws")
                    if args.world else None,
                "lifetime": "REMOVED_ON_OWNED_SHUTDOWN", "fault_control": "OWNER_STOP_ONLY",
                "expires_at": (datetime.now(timezone.utc) + timedelta(seconds=args.hold_seconds)).isoformat(),
                "runtime_owner_pid": os.getpid(), "source_revision": stack.report["source_revision"],
                "binary_cohort_manifest": str(args.bin_dir.parent / "cohort.json"),
                "gateway_binary_sha256": dev.runtime.digest(args.bin_dir / "api-gateway"),
                "concert_binary_sha256": dev.runtime.digest(args.concert_bin),
                "world_probe_sha256": dev.runtime.digest(Path(__file__).with_name("world_two_principal_probe.py"))}
            inputs = stack.cluster.root / "in01-live-inputs.json"
            prep.private_json(inputs, private)
            report.update(result="PARTIAL", private_inputs=str(inputs),
                world_activation="ACTUAL_LISTENER_READY" if args.world else "OFF",
                concert_binary_sha256=dev.runtime.digest(args.concert_bin))
            if args.hold_seconds:
                ready = {key: report[key] for key in ("result", "scope", "world_two_client", "browser_in01",
                    "driver_sha256", "private_inputs", "c03_database", "c03_database_observed")}
                ready.update(cleanup="PENDING_HELD_RUNTIME", expires_at=private["expires_at"])
                for key in ("world_probe", "world_database", "world_database_observed"):
                    if key in report:
                        ready[key] = report[key]
                prep.private_json(args.report.with_suffix(".ready.json"), ready)
            print(json.dumps({"result": "PARTIAL", "private_inputs": str(inputs),
                              "world_two_client": report["world_two_client"]}), flush=True)
            deadline = time.monotonic() + args.hold_seconds
            while time.monotonic() < deadline:
                with patch.object(dev.runtime, "readiness_value", prep.current_readiness):
                    stack.health()
                child.alive()
                time.sleep(min(0.25, max(0, deadline - time.monotonic())))
    except BaseException as error:
        report["failure"] = "qualification-producer-failure" if profile != "in01" else (str(error) if isinstance(error, prep.auth.CheckFailure)
            or type(error).__name__ == "ProbeFailure" else error.detail if isinstance(error, dev.SmokeError)
            else type(error).__name__)
        report["result"] = "FAIL"
        if child is not None and profile == "in01":
            report["concert_diagnostic"] = child.diagnostic()
    finally:
        if guard is not None:
            report["qualification_resource_bounds"] = guard.close()
            if guard.failure:
                report["result"] = "FAIL"
        if child is not None:
            try:
                report["concert_stop"] = child.stop()
                if (report["concert_stop"].get("exit_code") != 0
                        or report["concert_stop"].get("forced")
                        or report["concert_stop"].get("unexpected_exit")):
                    report["result"] = "FAIL"
            except BaseException:
                report["concert_stop"] = {"result": "FAIL"}
                report["result"] = "FAIL"
        try:
            stack.close()
        except BaseException:
            stack.report["cleanup"] = "FAIL"
        report["owned_stack"] = stack.report
        if guard is not None:
            report["qualification_total_seconds"] = time.monotonic() - guard.started_monotonic
            if time.monotonic() >= guard.hard_deadline:
                report["result"] = "FAIL"
                report["whole_hard_deadline_exceeded"] = True
        if stack.report.get("cleanup") != "PASS":
            report["result"] = "FAIL"
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        dev.write_fd(fd, report)
    print(json.dumps({"result": report["result"], "cleanup": stack.report.get("cleanup")}), flush=True)
    return int(report["result"] == "FAIL")


def public_json(browser, report, label, path, token, **kwargs):
    # prep.public_json's target is separate here, never the browser Origin.
    with patch.object(prep, "urlsplit", lambda _: browser.target):
        return ORIGINAL_PUBLIC_JSON(browser, report, label, path, token, **kwargs)


def initialize_profiles(browser, report, principals):
    with patch.object(prep, "public_json", public_json):
        prep.initialize_profiles(browser, report, principals)


# Capture the original to avoid recursion through the profile helper patch.
ORIGINAL_PUBLIC_JSON = prep.public_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", type=Path, required=True)
    parser.add_argument("--concert-bin", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--browser-origin", default="http://127.0.0.1:5173")
    parser.add_argument("--world", action="store_true")
    parser.add_argument("--hold-seconds", type=int, default=0)
    parser.add_argument("--qualification-profile", choices=("in01", "qa05", "qa08"), default="in01")
    parser.add_argument("--allocation-id", help="identifier of the separately accepted resource allocation")
    parser.add_argument("--provision-timeout-seconds", type=float, default=120)
    parser.add_argument("--probe-timeout-seconds", type=float, default=300)
    parser.add_argument("--measurement-seconds", type=float, default=20)
    parser.add_argument("--input-hz", type=float, default=15)
    parser.add_argument("--arrival-concurrency", type=int, default=4)
    parser.add_argument("--restart-world", action="store_true", help="QA05 only: restart this producer's direct Concert child")
    parser.add_argument("--qa08-provisioning-budget", choices=("signup=400,login=200",),
                        help="QA08 only; requires separately authorized normal configuration in the resource offer")
    parser.add_argument("--whole-timeout-seconds", type=float, default=600)
    parser.add_argument("--whole-byte-limit", type=int, help="whole carry plus outputs cap from the fresh resource offer")
    parser.add_argument("--retained-carry-bytes", type=int, help="measured reusable cohort/source/proof carry from the fresh offer")
    parser.add_argument("--retain-public-projection", action="store_true",
                        help="QA05 only: retain public resource/capability projection after in-memory actual C03 validation")
    parser.add_argument("--frontend-source-revision", help="consumer frontend commit for retention provenance; no frontend process is run")
    args = parser.parse_args()
    dev.local_origin(args.browser_origin)
    if (not all(path.is_absolute() for path in (args.bin_dir, args.concert_bin, args.report))
            or not 0 <= args.hold_seconds <= 3600):
        parser.error("absolute input/report paths and hold in 0..3600 required")
    validate_qualification_arguments(args)
    return run(args)


def validate_qualification_arguments(args):
    from world_capacity_support.report import number, integer
    profile = prep.producer_profile(args.qualification_profile)
    number(args.provision_timeout_seconds, 1, 3600, "provision-timeout")
    number(args.probe_timeout_seconds, 1, 7200, "probe-timeout")
    number(args.measurement_seconds, 10, 3600, "measurement-duration")
    number(args.input_hz, .1, 30, "input-rate")
    integer(args.arrival_concurrency, 1, 200, "arrival-concurrency")
    prep.auth.require(not args.restart_world or profile["name"] == "qa05", "restart-profile")
    prep.auth.require(not args.retain_public_projection or profile["name"] == "qa05", "bootstrap-retention-profile")
    frontend = getattr(args, "frontend_source_revision", None)
    prep.auth.require(frontend is None or re.fullmatch(r"[0-9a-f]{40}", frontend), "frontend-provenance-revision")
    budget = getattr(args, "qa08_provisioning_budget", None)
    prep.auth.require((profile["name"] == "qa08" and budget == "signup=400,login=200")
                      or (profile["name"] != "qa08" and budget is None), "qa08-explicit-normal-budget")
    if profile["name"] != "in01":
        prep.auth.require(args.world and args.hold_seconds == 0, "qualification-world-no-hold")
        prep.auth.require(isinstance(args.allocation_id, str) and 1 <= len(args.allocation_id) <= 128, "qualification-allocation")
        prep.auth.require(args.probe_timeout_seconds > args.measurement_seconds, "qualification-deadline")
        number(args.whole_timeout_seconds, CLEANUP_RESERVE_SECONDS + 60, 3600, "qualification-whole-timeout")
        integer(args.whole_byte_limit, 1, 4 * 1024**3, "qualification-whole-byte-limit")
        integer(args.retained_carry_bytes, 309342208, args.whole_byte_limit - 1, "qualification-retained-carry")


if __name__ == "__main__":
    raise SystemExit(main())
