#!/usr/bin/env python3
"""Create real, disposable Auth principals and owned World startup inputs.

This command does not enable World or certify IN01. Native service binaries are
explicit inputs. All credentials and database URLs stay in the owned private
directory; stdout and the external report contain paths and sanitized results.
"""
from __future__ import annotations

import argparse
import http.client
import json
import os
from pathlib import Path
import secrets
import signal
import time
from unittest.mock import patch
from uuid import UUID, uuid4
from urllib.parse import urlsplit

import browser_auth_smoke as auth
from browser_refresh import current_readiness
import dev_stack as dev

ROOT = Path(__file__).resolve().parents[3]
RELEASE = ROOT / "backend/releases/operator-bundle/quality-v6"


def producer_profile(name="in01"):
    """Fixed original-task cohorts, never arbitrary account/room multiplication."""
    profiles = {"in01": (2, (2,)), "qa05": (2, (2, 2)), "qa08": (200, (50, 50, 50, 50))}
    auth.require(isinstance(name, str) and name in profiles, "producer-profile")
    principals, capacities = profiles[name]
    return {"name": name, "principals": principals, "capacities": capacities}


def provision_principals(stack, browser, profile, timeout_seconds=120):
    """Real signup/mail/verify/login only; fail on normal admission refusal."""
    from world_capacity_support.report import number
    number(timeout_seconds, 1, 3600, "provision-deadline")
    spec = producer_profile(profile)
    deadline = time.monotonic() + timeout_seconds
    principals, canonical = [], set()
    for ordinal in range(1, spec["principals"] + 1):
        auth.require(time.monotonic() < deadline, "principal-provision-deadline")
        item = principal(stack, browser, ordinal, deadline=deadline)
        auth.require(item[0] not in canonical, "distinct-canonical-principals")
        canonical.add(item[0])
        principals.append(item)
        auth.require(time.monotonic() < deadline, "principal-provision-deadline")
    return principals


def private_json(path, value):
    dev.runtime.private_file(path, dev.encode(value))


def quote(value):
    """SQL literal for validated local seed data, never shell interpolation."""
    return "'" + str(value).replace("'", "''") + "'"


def release_inputs():
    bundle = json.loads((RELEASE / "bundle.json").read_bytes())
    for pin in [bundle["map"], bundle["avatar"], bundle["source_binding"], *bundle["outputs"]]:
        path = (ROOT / pin["path"]).resolve()
        auth.require(path.is_relative_to(ROOT) and path.is_file(), "release-path")
        auth.require(path.stat().st_size == pin["bytes"]
                     and "sha256:" + dev.runtime.digest(path) == pin["sha256"], "release-pin")
    return {
        "CONCERT_BOOTSTRAP_FILE": str(RELEASE / "bootstrap.json"),
        "CONCERT_AVATAR_MANIFEST_FILE": str(ROOT / bundle["avatar"]["path"]),
        "CONCERT_MAP_MANIFEST_FILE": str(ROOT / bundle["map"]["path"]),
        **{f"CONCERT_{theme}_MANIFEST_FILE": str(RELEASE / f"theme-{theme.lower()}.json")
           for theme in ("REVENGE", "PARADE", "DANGER")},
        "source_binding": str(ROOT / bundle["source_binding"]["path"]),
        "asset_root": str(ROOT / "frontend/public"),
    }


def principal(stack, browser, ordinal, *, deadline=None):
    def budget():
        auth.require(deadline is None or time.monotonic() < deadline, "principal-provision-deadline")
    budget()
    email = f"mcr-in01-{secrets.token_hex(12)}@example.test"
    password = secrets.token_urlsafe(24)
    _, created = browser.request(f"principal-{ordinal}-signup", "/api/auth/email/signup",
                                  {"email": email, "password": password})
    user = str(UUID(created["userId"]))
    auth.require(created["userId"] == user and UUID(user).int != 0, "canonical-principal")
    delivery_deadline = min(time.monotonic() + 20, deadline) if deadline is not None else time.monotonic() + 20
    code = None
    while time.monotonic() < delivery_deadline and code is None:
        stack.cluster.assert_owned()
        for path in stack.mail.directory.glob("*.json"):
            auth.require(not path.is_symlink() and path.stat().st_mode & 0o077 == 0, "private-inbox")
            message = json.loads(path.read_bytes())
            if message["kind"] == "VERIFY_EMAIL" and message["payload"]["email"] == email:
                code = message["payload"]["verification_code"]
                break
        if code is None:
            time.sleep(0.1)
    auth.require(code is not None, "real-verification-delivery")
    budget()
    _, verified = browser.request(f"principal-{ordinal}-verify", "/api/auth/email/verify",
                                   {"email": email, "code": code})
    auth.require(verified["userId"] == user, "verified-principal")
    budget()
    headers, logged_in = browser.request(f"principal-{ordinal}-login", "/api/auth/login",
                                         {"email": email, "password": password})
    auth.require(logged_in["userId"] == user and logged_in["accessToken"]
                 and logged_in["refreshToken"] == "", "real-login-identity")
    auth.refresh_cookie(headers)
    budget()
    # No JWT decoding, forged session, canonical claim SQL, or token persistence.
    path = stack.cluster.root / f"in01-principal-{ordinal}.json"
    private_json(path, {"user_id": user, "email": email, "password": password,
                        "login_path": "/api/auth/login", "access_token": "MEMORY_ONLY"})
    return user, path, logged_in["accessToken"]


def public_json(browser, report, label, path, bearer, *, method="GET", payload=None, response_bytes=False):
    """Exercise the actual public route without Auth-only response assumptions."""
    headers = {"Authorization": "Bearer " + bearer,
               "X-Correlation-Id": "in01-" + label, "Origin": browser.origin,
               "X-MCR-Intent": "browser"}
    body = None
    if payload is not None:
        body = json.dumps(payload)
        headers["Content-Type"] = "application/json"
    target = urlsplit(browser.origin)
    connection = http.client.HTTPConnection(target.hostname, target.port, timeout=10)
    try:
        connection.request(method, path, body=body, headers=headers)
        response = connection.getresponse()
        raw = response.read(2_000_001)
        report["statuses"][label] = response.status
        auth.require(response.status == 200, label + ":status")
        auth.require(len(raw) <= 2_000_000, label + ":body-limit")
        if response_bytes:
            auth.require(response.getheader("Cache-Control") == "no-store"
                         and response.getheader("Content-Type", "").split(";", 1)[0] == "application/json",
                         "retained-bootstrap-response-policy")
        return raw if response_bytes else json.loads(raw)
    finally:
        connection.close()


def initialize_profiles(browser, report, principals):
    for ordinal, (user, _, token) in enumerate(principals, 1):
        result = public_json(browser, report, f"principal-{ordinal}-profile",
            "/api/profile/" + user, token, method="PUT", payload={
                "username": "in01_" + secrets.token_hex(6),
                "nickname": f"IN01 participant {ordinal}",
                "avatarUrl": "/world/avatars/derived-v1/photo01-front-idle-64-e124c88815b22220.png",
                "bio": "Owned IN01 runtime participant"})
        auth.require(result["userId"] == user, "public-profile-owner-identity")


def seed_concert(stack, binary, report, publish_catalog, profile="in01", *, runtime_manifest=None):
    """Use the real migration command; seed only this invocation's new database."""
    spec = producer_profile(profile)
    cluster = stack.cluster
    cluster.assert_owned()
    binary = dev.migration.executable(binary)
    database = "mcr_concert"
    dev.migration.execute([str(cluster.tools["createdb"]), "--no-password",
                           "--maintenance-db=postgres", database],
                          env=cluster.client_env("postgres"), cwd=cluster.root, phase="concert-create")
    env = {**cluster.env, "CONCERT_DATABASE_URL": cluster.url(database)}
    for flag in ("--migrate", "--verify-migrations"):
        dev.migration.execute([str(binary), flag], env=env, cwd=cluster.root, phase="concert" + flag)
    manifest = dev.migration.migration_manifest(ROOT / "backend", "concert-service") if runtime_manifest is None else runtime_manifest
    count = dev.migration.check_ledger(cluster, database, manifest)
    probe = (ROOT / "backend/services/concert-service/src/config/schema-probe.sql").read_text()
    auth.require(cluster.sql(database, probe) == "t", "current-concert-schema-probe")
    negative = cluster.sql(database, "BEGIN; ALTER TABLE concert_avatar_settings DROP CONSTRAINT "
                           "concert_avatar_representation; " + probe + "; ROLLBACK;")
    auth.require("f" in negative.splitlines() and cluster.sql(database, probe) == "t",
                 "missing-avatar-constraint-refused-and-restored")
    template = json.loads((RELEASE / "bootstrap.json").read_bytes())
    event = str(UUID(template["event"]["event_id"]))
    rooms = [{"room_id": str(uuid4()), "capacity": cap} for cap in spec["capacities"]]
    avatar = template["avatar_manifest"]
    # Event/room configuration is operator-owned. Admission, participant and
    # avatar creation must come from the actual authenticated bootstrap request.
    statements = ["BEGIN", "INSERT INTO concert_events(event_id,slug,title,phase) VALUES(" +
                  ",".join(map(quote, [event, "in01-owned-" + uuid4().hex[:16],
                                        "Owned IN01 startup", "OPEN"])) + ")"]
    map_ = template["map_manifest"]
    for index, room in enumerate(rooms):
        statements.append("INSERT INTO concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,"
        "avatar_manifest_id,avatar_manifest_version,is_default) VALUES(" +
        ",".join(map(quote, [event, room["room_id"], "Owned IN01 room" if profile == "in01" else "Owned World room " + str(index + 1), room["capacity"], map_["map_id"],
                             map_["map_revision"], avatar["manifest_id"], avatar["manifest_version"]]))
        + (",true)" if index == 0 else ",false)"))
    statements.append("COMMIT")
    cluster.sql(database, ";\n".join(statements) + ";")
    for table in ("concert_participants", "concert_event_admissions", "concert_avatar_settings"):
        auth.require(cluster.sql(database, f"SELECT count(*) FROM {table}") == "0", "bootstrap-owned-state-absent")
    catalog = "NOT_RUN"
    if publish_catalog:
        release = release_inputs()
        result = dev.migration.execute([str(binary), "--prepare-in01-catalog"],
            env={**env, **{name: value for name, value in release.items() if name.startswith("CONCERT_")},
                 "CONCERT_RUNTIME_MODE": "local", "CONCERT_WORLD_EVENT_ID": event,
                 "CONCERT_SOURCE_BINDING_FILE": release["source_binding"],
                 "CONCERT_ASSET_ROOT": release["asset_root"]},
            cwd=cluster.root, phase="real-source-bound-catalog")
        auth.require(result.stdout.strip() == "IN01_CATALOG_PUBLISHED=4", "catalog-command-result")
        catalog = "PASS"
    report["concert_seed"] = {"result": "PARTIAL", "migration_commands_exit": [0, 0],
                               "producer_profile": spec["name"], "principal_target": spec["principals"],
                               "registered_room_capacities": list(spec["capacities"]),
                               "ledger_count": count, "binary_sha256": dev.runtime.digest(binary),
                               "migration_sha384": manifest,
                               "schema_probe": "PASS; missing avatar representation constraint refused",
                               "source_bound_catalog_publication": catalog,
                               "limits": ["Owner runtime registration pending.",
                                          "Participants, admissions and avatars await real authenticated bootstrap.",
                                          "No World listener, ticket, or connection has been started."]}
    path = cluster.root / "in01-concert-inputs.json"
    private_json(path, {"CONCERT_DATABASE_URL": cluster.url(database), "CONCERT_RUNTIME_MODE": "local",
                        "CONCERT_MIGRATION_MODE": "external", "CONCERT_WORLD_EVENT_ID": event,
                        "CONCERT_WORLD_OWNER_ID": str(uuid4()),
                        "CONCERT_WORLD_ROOMS_JSON": json.dumps(rooms),
                        "CONCERT_PUBLIC_FEED_MODE": "off", "CONCERT_OWNER_MEDIA_MODE": "off",
                        "release_inputs": release_inputs(),
                        "world_activation": "CLOSED_PENDING_NATIVE_QUALIFICATION"})
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", type=Path, required=True)
    parser.add_argument("--concert-bin", type=Path)
    parser.add_argument("--publish-catalog", action="store_true",
                        help="Requires a freshly compiled binary with --prepare-in01-catalog")
    parser.add_argument("--bootstrap", action="store_true",
                        help="Start real Concert bootstrap RPC and initialize through public HTTP")
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--hold-seconds", type=int, default=0)
    args = parser.parse_args()
    if (not args.report.is_absolute() or not args.bin_dir.is_absolute()
            or (args.concert_bin is not None and not args.concert_bin.is_absolute())
            or (args.publish_catalog and args.concert_bin is None)
            or (args.bootstrap and not args.publish_catalog)
            or not 0 <= args.hold_seconds <= 3600):
        parser.error("absolute new report path and hold-seconds in 0..3600 required")
    fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    report = {"scope": "IN01 isolated startup preparation", "result": "FAIL", "statuses": {},
              "world_two_client": "NOT_RUN", "world_activation": "CLOSED",
              "script_sha256": dev.runtime.digest(Path(__file__))}
    stack = dev.DevStack(args.bin_dir, Path("/opt/homebrew/bin"),
                         Path("/opt/homebrew/bin/redis-server"), Path("/opt/homebrew/bin/openssl"))
    concert_child = None
    concert_env = {}
    original_environments = dev.development_environments

    def startup_environments(cluster, ports, keys, credentials, *rest):
        envs = original_environments(cluster, ports, keys, credentials, *rest)
        if args.bootstrap:
            ports.reserve("concert-service:grpc")
            ports.reserve("concert-service:metrics")
            envs["api-gateway"]["GATEWAY_CONCERT_GRPC_ENDPOINT"] = ports.url("concert-service:grpc")
            concert_env.update(cluster.env, **credentials,
                ACTOR_DELEGATION_VERIFYING_KEYS=keys["ACTOR_DELEGATION_VERIFYING_KEYS"],
                CONCERT_DATABASE_URL=cluster.url("mcr_concert"),
                CONCERT_RUNTIME_MODE="local", CONCERT_MIGRATION_MODE="external",
                CONCERT_GRPC_ADDR=ports.address("concert-service:grpc"),
                CONCERT_METRICS_ADDR=ports.address("concert-service:metrics"),
                CONCERT_PUBLIC_FEED_MODE="off", CONCERT_OWNER_MEDIA_MODE="off",
                TOKIO_WORKER_THREADS="2", RUST_LOG="warn",
                **{k: v for k, v in release_inputs().items() if k.startswith("CONCERT_")})
        return envs
    previous = {sig: signal.signal(sig, lambda *_: (_ for _ in ()).throw(KeyboardInterrupt()))
                for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        release = release_inputs()
        report["release_pins"] = "EXACT_BUNDLE_BYTES_VERIFIED; catalog constructor outcome recorded separately"
        with patch.object(dev.runtime, "readiness_value", current_readiness), \
                patch.object(dev, "development_environments", startup_environments):
            descriptor = stack.start()
        browser = auth.Browser(descriptor["gateway_url"], report)
        principals = [principal(stack, browser, n) for n in (1, 2)]
        auth.require(principals[0][0] != principals[1][0], "distinct-canonical-principals")
        inputs = {"gateway_url": descriptor["gateway_url"],
                  "principals": [str(path) for _, path, _ in principals],
                  "release_inputs": release,
                  "world": "CLOSED", "lifetime": "Valid only while this owned process is holding"}
        if args.concert_bin:
            inputs["concert"] = str(seed_concert(stack, args.concert_bin, report, args.publish_catalog))
        if args.bootstrap:
            initialize_profiles(browser, report, principals)
            concert_child = dev.runtime.Child(stack.cluster, "concert-service",
                [str(dev.migration.executable(args.concert_bin))], concert_env, stack.ports,
                ("concert-service:grpc", "concert-service:metrics"),
                tuple(concert_env.values()))
            concert_child.wait_listeners()
            status, _ = dev.runtime.http_probe(concert_child, "metrics", "/readyz")
            auth.require(status == 200, "actual-concert-ready")
            event = json.loads((RELEASE / "bootstrap.json").read_bytes())["event"]["event_id"]
            for ordinal, (user, _, token) in enumerate(principals, 1):
                result = public_json(browser, report, f"principal-{ordinal}-bootstrap",
                    f"/api/concert/events/{event}/bootstrap", token)
                auth.require(result["participant"]["user_id"] == user,
                             "authenticated-bootstrap-owner")
            for table in ("concert_participants", "concert_event_admissions", "concert_avatar_settings"):
                auth.require(stack.cluster.sql("mcr_concert", f"SELECT count(*) FROM {table}") == "2",
                             "actual-bootstrap-created-two-" + table)
            report["public_bootstrap"] = "PASS; actual Profile writes and bootstrap-created state"
            inputs["event_id"] = event
        path = stack.cluster.root / "in01-runtime-inputs.json"
        private_json(path, inputs)
        report.update(result="PARTIAL", principal_count=2, private_inputs=str(path),
                      input_lifetime="REMOVED_ON_OWNED_SHUTDOWN")
        print(json.dumps({"result": "PARTIAL", "private_inputs": str(path),
                          "world_activation": "CLOSED"}), flush=True)
        deadline = time.monotonic() + args.hold_seconds
        while time.monotonic() < deadline:
            stack.health()
            if concert_child is not None:
                concert_child.alive()
            time.sleep(min(0.25, max(0, deadline - time.monotonic())))
    except BaseException as error:
        report["failure"] = (str(error) if isinstance(error, auth.CheckFailure)
                             else error.detail if isinstance(error, dev.SmokeError)
                             else type(error).__name__)
        report["result"] = "FAIL"
    finally:
        if concert_child is not None:
            try:
                report["concert_stop"] = concert_child.stop()
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


if __name__ == "__main__":
    raise SystemExit(main())
