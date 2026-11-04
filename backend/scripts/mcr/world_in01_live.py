#!/usr/bin/env python3
"""Owned real-Auth, mTLS C03 and optional World startup; no fixture principals."""
from __future__ import annotations

import argparse
from datetime import datetime, timedelta, timezone
import importlib.util
import json
import os
from pathlib import Path
import re
import secrets
import signal
import time
from unittest.mock import patch
from uuid import uuid4

import dev_stack as dev
import participant_live_join_smoke as joined
import world_in01_prepare as prep

ROOT = prep.ROOT


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
    fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    report = {"result": "FAIL", "scope": "actual real-Auth mTLS C03/World startup",
              "statuses": {}, "world_two_client": "NOT_RUN", "browser_in01": "NOT_RUN",
              "driver_sha256": dev.runtime.digest(Path(__file__))}
    stack = dev.DevStack(args.bin_dir, Path("/opt/homebrew/bin"),
        Path("/opt/homebrew/bin/redis-server"), Path("/opt/homebrew/bin/openssl"), args.browser_origin)
    child = None
    context = {}
    original = dev.development_environments

    def environments(cluster, ports, keys, credentials, *rest):
        # The retained helper validates its unchanged HTTP endpoints first.
        credentials["CONCERT_SERVICE_CREDENTIAL"] = secrets.token_hex(32)
        envs = original(cluster, ports, keys, credentials, *rest)
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
        inputs_path = prep.seed_concert(stack, args.concert_bin, report, True)
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
        with patch.object(dev.runtime, "readiness_value", prep.current_readiness), \
                patch.object(dev, "development_environments", environments):
            descriptor = stack.start()
        browser = prep.auth.Browser(descriptor["gateway_url"], report)
        # Keep HTTP connection target separate from the actual browser Origin.
        browser.origin = args.browser_origin
        principals = [prep.principal(stack, browser, i) for i in (1, 2)]
        prep.auth.require(principals[0][0] != principals[1][0], "independent-auth-principals")
        # The retained public helper addresses browser.origin; supply its actual
        # Gateway target while preserving the browser header through a wrapper.
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
        for ordinal, (user, _, token) in enumerate(principals, 1):
            value = public_json(browser, report, f"principal-{ordinal}-bootstrap",
                f"/api/concert/events/{context['event']}/bootstrap", token)
            prep.auth.require(value["participant"]["user_id"] == user, "actual-bootstrap-owner")
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
        report["failure"] = (str(error) if isinstance(error, prep.auth.CheckFailure)
            or type(error).__name__ == "ProbeFailure" else error.detail if isinstance(error, dev.SmokeError)
            else type(error).__name__)
        report["result"] = "FAIL"
        if child is not None:
            report["concert_diagnostic"] = child.diagnostic()
    finally:
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
    args = parser.parse_args()
    dev.local_origin(args.browser_origin)
    if (not all(path.is_absolute() for path in (args.bin_dir, args.concert_bin, args.report))
            or not 0 <= args.hold_seconds <= 3600):
        parser.error("absolute input/report paths and hold in 0..3600 required")
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
