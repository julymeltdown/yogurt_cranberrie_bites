#!/usr/bin/env python3
"""Owned actual C03 HTTP reset, Auth login recovery, and unknown-work regression.

Reuses the retained marker/inode/PID/port controller and real email-auth helpers.
The dedicated copied Rust host gates the actual publisher, never HTTP EOF.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import signal
import socket
import struct
import subprocess
import sys
import time
from unittest.mock import patch
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
import dev_stack as dev
import participant_live_join_smoke as joined
import world_in01_prepare as prep
import world_in01_live as live

TEST = "public_read_cancellation_releases_fences_and_unknown_callback_quarantines"


def require(condition, label):
    prep.auth.require(condition, label)


def sources():
    # Include all Rust/build inputs and retained controller dependencies. Art is
    # source-bound by the real release constructor rather than synthetic data.
    files = [ROOT / "backend/Cargo.lock", ROOT / "backend/Cargo.toml"]
    for directory in ("backend/services", "backend/crates", "backend/proto", "backend/scripts/mcr"):
        files += [p for p in (ROOT / directory).rglob("*")
                  if p.is_file() and p.suffix in {".rs", ".toml", ".proto", ".sql", ".py"}]
    return {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(set(files))}


def wait_file(path, process, timeout=10):
    stop = time.monotonic() + timeout
    while not path.is_file():
        require(process.poll() is None, "actual-cancellation-host-exited")
        require(time.monotonic() < stop, "actual-cancellation-host-marker-timeout")
        time.sleep(0.01)
    return json.loads(path.read_bytes())


def pending(cluster):
    return int(cluster.sql("postgres", "SELECT count(*) FROM pg_prepared_xacts"))


def owners(cluster):
    return {role: json.loads(cluster.sql("mcr_" + role,
        "SELECT coalesce(json_agg(x),'[]') FROM (SELECT state,scope_settlement IS NOT NULL AS settled,"
        "count(*) FROM concert_owner_guard_leases GROUP BY 1,2) x"))
        for role in ("auth", "friend")}


def reset_read(gateway, event, token, cluster, process, ordinal):
    target = urlsplit(gateway)
    prep.private_json(cluster.root / f"cancel-arm-{ordinal}.json", {"gate_next_request":True})
    connection = socket.create_connection((target.hostname, target.port), timeout=5)
    try:
        payload = (json.dumps({"expected_revision":1,"preferences":{"theme":"DANGER","low_power":False,
                   "reduced_motion":False,"sound_enabled":False}}).encode() if ordinal == 3 else b"")
        method = "PATCH" if ordinal == 3 else "GET"
        extra = (f"Origin: {gateway}\r\nX-MCR-Intent: browser\r\nContent-Type: application/json\r\nContent-Length: {len(payload)}\r\n" if payload else "")
        request = (f"{method} /api/concert/events/{event}/me HTTP/1.1\r\nHost: {target.netloc}\r\n"
                   f"Authorization: Bearer {token}\r\n{extra}Connection: close\r\n\r\n").encode()+payload
        connection.sendall(request)
        gated = wait_file(cluster.root / f"cancel-gated-{ordinal}.json", process)
        require(gated["published"], "durable-real-delivery-not-published")
        # A real TCP reset interrupts the actual Gateway's in-flight gRPC read.
        # No body is read or collected, and no CompleteResponse RPC is called.
        connection.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 0))
    finally:
        connection.close()
    prep.private_json(cluster.root / f"cancel-disconnected-{ordinal}.json", {"tcp_reset": True})


def run(args):
    report = {"result": "FAIL", "scope": "C03-CANCEL01 actual native public HTTP cancellation and real Auth login",
              "statuses": {}, "browser_in01": "NOT_RUN", "production_wiring": "ROOT_OWNED",
              "driver_sha256": dev.runtime.digest(Path(__file__))}
    receipt = json.loads(args.receipt.read_bytes())
    require(receipt["compile_exit"] == 0 and receipt["sha256"] == dev.runtime.digest(args.binary)
            and receipt["source_sha256"] == sources(), "copied-host-source-receipt")
    require(receipt["artifact"]["target"]["name"] == "participant_cancellation"
            and receipt["artifact"]["profile"]["test"] is True, "exact-cargo-test-artifact")
    require(args.binary.is_relative_to("/private/tmp") and not args.binary.is_symlink(), "private-copied-host")
    cohort = json.loads(args.cohort.read_bytes())
    for name in dev.runtime.START_ORDER:
        require(dev.runtime.digest(args.bin_dir / name) == cohort[name]["sha256"], "immutable-peer-cohort")
    report["binary_sha256"] = receipt["sha256"]
    report["compiled_source_commit"] = receipt["source_commit"]
    report["peer_binary_sha256"] = {name: cohort[name]["sha256"] for name in dev.runtime.START_ORDER}
    stack = dev.DevStack(args.bin_dir, Path("/opt/homebrew/bin"), Path("/opt/homebrew/bin/redis-server"), Path("/opt/homebrew/bin/openssl"))
    process, output = None, None
    original = dev.development_environments

    def environments(cluster, ports, keys, credentials, *rest):
        nonlocal process, output
        credentials["CONCERT_SERVICE_CREDENTIAL"] = secrets.token_hex(32)
        envs = original(cluster, ports, keys, credentials, *rest)
        cluster.stop()
        with (cluster.data / "postgresql.conf").open("a") as stream:
            stream.write("\nmax_prepared_transactions=128\n")
        cluster.start()
        require(cluster.sql("postgres", "SHOW server_version_num") == "160014", "exact-owned-PG16.14")
        live.pki(cluster)
        relationship, self_profile, decision = joined.keys(cluster, 3)
        ports.reserve("concert-service:grpc")
        urls = {role: "https://" + ports.address(role + "-service:grpc") for role in ("auth", "friend", "profile", "concert")}
        for name, env in envs.items():
            for key, value in list(env.items()):
                if key.endswith("_GRPC_ENDPOINT"):
                    require(value.startswith("http://127.0.0.1:"), "owned-plaintext-before-mtls")
                    env[key] = value.replace("http://", "https://", 1)
            env.update(live.tls(cluster, dev.runtime.PREFIX[name].lower()))
            env.update(CONCERT_RELATIONSHIP_VERIFYING_KEYS=relationship[1])
        for role in ("auth", "friend"):
            envs[role + "-service"].update(CONCERT_OWNER_AUTHORITY_ENABLED="true",
                AUTHORITY_CONCERT_SETTLEMENT_ENDPOINT=urls["concert"], AUTHORITY_PROFILE_SETTLEMENT_ENDPOINT=urls["profile"])
        envs["profile-service"].update(CONCERT_PROFILE_SELF_VERIFYING_KEYS=self_profile[1],
            PROFILE_PARTICIPANT_DECISION_SIGNING_SEED=decision[0], PROFILE_SELF_AUTH_GRPC_ENDPOINT=urls["auth"],
            PROFILE_SELF_FRIEND_GRPC_ENDPOINT=urls["friend"], PROFILE_SELF_CONCERT_GRPC_ENDPOINT=urls["concert"])
        cluster.sql("postgres", "CREATE DATABASE mcr_concert TEMPLATE template0")
        env = {**cluster.env, **credentials, **live.tls(cluster,"concert"),
            "ACTOR_DELEGATION_VERIFYING_KEYS": keys["ACTOR_DELEGATION_VERIFYING_KEYS"],
            "CONCERT_RELATIONSHIP_SIGNING_SEED": relationship[0], "CONCERT_PROFILE_SELF_SIGNING_SEED": self_profile[0],
            "CONCERT_DATABASE_URL": cluster.url("mcr_concert"), "CONCERT_GRPC_ADDR": ports.address("concert-service:grpc"),
            "MCR_CANCEL_ROOT": str(cluster.root), "MCR_CANCEL_ROOT_TOKEN": cluster.token,
            "MCR_CANCEL_ROOT_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
            "MCR_CANCEL_SYSTEM": cluster.sql("postgres", "SELECT system_identifier FROM pg_control_system()"),
            "MCR_CANCEL_REPO": str(ROOT), "RUST_LOG": "warn"}
        for role in ("auth", "friend", "profile"):
            env["CONCERT_PARTICIPANT_" + role.upper() + "_GRPC_ENDPOINT"] = urls[role]
        fd = os.open(cluster.root / "cancel-host.log", os.O_RDWR | os.O_CREAT | os.O_EXCL, 0o600)
        output = os.fdopen(fd, "w+")
        ports.release("concert-service:grpc")
        process = subprocess.Popen([str(args.binary), "--ignored", "--exact", TEST, "--test-threads=1", "--nocapture"],
            env=env, cwd=ROOT, stdout=output, stderr=subprocess.STDOUT)
        ready = wait_file(cluster.root / "cancel-ready.json", process, 15)
        report["event_id"] = ready["event_id"]
        report["release_digest"] = ready["release_digest"]
        envs["api-gateway"].update(GATEWAY_CONCERT_GRPC_ENDPOINT=urls["concert"], GATEWAY_CONCERT_RELEASE_DIGEST=ready["release_digest"])
        return envs

    try:
        with patch.object(dev.runtime, "readiness_value", prep.current_readiness), patch.object(dev,"development_environments",environments):
            descriptor = stack.start()
        browser = prep.auth.Browser(descriptor["gateway_url"],report)
        principals = [prep.principal(stack,browser,index) for index in (1,2,3)]
        live.initialize_profiles(browser,report,principals)
        for index,(user,_,token) in enumerate(principals,1):
            value = live.public_json(browser,report,f"principal-{index}-bootstrap",f"/api/concert/events/{report['event_id']}/bootstrap",token)
            require(value["participant"]["user_id"] == user,"real-authenticated-bootstrap")
        normal = live.public_json(browser,report,"normal-c03-read",f"/api/concert/events/{report['event_id']}/me",principals[0][2])
        require(normal["user_id"] == principals[0][0],"normal-c03-principal")
        stop = time.monotonic()+5
        while pending(stack.cluster):
            require(time.monotonic()<stop,"normal-c03-must-settle-through-real-HTTP-EOF")
            time.sleep(0.01)
        require(stack.cluster.sql("mcr_concert","SELECT count(*) FROM concert_product_owner_scopes WHERE phase='ROOT_FINISHED' AND serialized_at IS NOT NULL AND delivery_completed_at IS NOT NULL AND response_abandoned_at IS NULL") == "1","normal-c03-delivery-remains-distinct")
        report["normal_public_c03"] = "PASS"
        reset_read(descriptor["gateway_url"],report["event_id"],principals[0][2],stack.cluster,process,1)
        report["positive"] = wait_file(stack.cluster.root / "cancel-recovered.json",process)
        require(pending(stack.cluster) == 0,"positive-zero-prepared-fences")
        report["owners_after_positive"] = owners(stack.cluster)
        require(all(rows == [{"state":"COMPLETED","settled":True,"count":2}] for rows in report["owners_after_positive"].values()),"both-owner-completes")
        credentials = json.loads(principals[0][1].read_bytes())
        headers,login = browser.request("fresh-login-after-cancellation","/api/auth/login",{"email":credentials["email"],"password":credentials["password"]})
        require(login["userId"] == principals[0][0] and login["accessToken"] and login["refreshToken"] == "","actual-fresh-login-recovered")
        prep.auth.refresh_cookie(headers)
        report["fresh_real_login"] = "PASS"
        prep.private_json(stack.cluster.root / "cancel-login-recovered.json",{"actual_login":True})
        reset_read(descriptor["gateway_url"],report["event_id"],principals[1][2],stack.cluster,process,2)
        report["unknown_callback"] = wait_file(stack.cluster.root / "cancel-unknown.json",process)
        require(pending(stack.cluster) == 2,"unknown-callback-must-retain-two-fences")
        reset_read(descriptor["gateway_url"],report["event_id"],principals[2][2],stack.cluster,process,3)
        report["unknown_native"] = wait_file(stack.cluster.root / "cancel-unknown-native.json",process)
        require(pending(stack.cluster) == 4,"unknown-native-must-retain-additional-two-fences")
        report["owners_after_unknown"] = owners(stack.cluster)
        report["prepared_before_owned_shutdown"] = pending(stack.cluster)
        prep.private_json(stack.cluster.root / "cancel-stop.json",{})
        require(process.wait(timeout=5) == 0,"cancellation-host-result")
        output.seek(0)
        text = output.read()
        require("1 passed; 0 failed; 0 ignored" in text,"exact-native-host-passed")
        report["native_host"] = "PASS"
        report["result"] = "PASS"
    except (Exception,KeyboardInterrupt) as error:
        report["failure"] = (str(error) if isinstance(error,prep.auth.CheckFailure) else type(error).__name__)
        if stack.cluster is not None and stack.cluster.started:
            try:
                report["prepared_at_failure"] = pending(stack.cluster)
                report["owners_at_failure"] = owners(stack.cluster)
                report["roots_at_failure"] = json.loads(stack.cluster.sql("mcr_concert",
                    "SELECT coalesce(json_agg(x),'[]') FROM (SELECT phase,serialized_at IS NOT NULL AS serialized,"
                    "delivery_completed_at IS NOT NULL AS delivered,response_abandoned_at IS NOT NULL AS abandoned,"
                    "cancellation_settled_at IS NOT NULL AS cancellation_settled FROM concert_product_owner_scopes) x"))
            except Exception:
                report["failure_diagnostics"] = "UNAVAILABLE"
        if output is not None:
            output.flush(); output.seek(0)
            # Closed Rust errors deliberately exclude private config/HTTP bodies.
            report["host_output"] = output.read()[-5000:]
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            try: process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill(); process.wait(timeout=3)
                report["forced_host_stop"] = True
        if output is not None: output.close()
        try: stack.close()
        except BaseException: stack.report["cleanup"] = "FAIL"
        report["owned_cleanup"] = stack.report
        if stack.report.get("cleanup") != "PASS": report["result"] = "FAIL"
    report["limits"] = ["Deterministic gate after actual durable publication; spontaneous browser timing is Root/FE owned.",
        "No fabricated EOF, CompleteResponse, prepared rollback or TTL release.",
        "Expected unknown-work fences remain until the entire owned disposable cluster is stopped; shutdown is not recovery proof."]
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("binary","receipt","bin-dir","cohort","report"):
        parser.add_argument("--"+name,type=Path,required=True)
    args = parser.parse_args()
    require(all(p.is_absolute() for p in vars(args).values()),"absolute-explicit-inputs")
    fd = os.open(args.report,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
    previous = {s:signal.signal(s,lambda *_:(_ for _ in ()).throw(KeyboardInterrupt())) for s in (signal.SIGTERM,signal.SIGINT)}
    try: report = run(args)
    finally:
        for s,handler in previous.items(): signal.signal(s,handler)
    with os.fdopen(fd,"w") as stream: json.dump(report,stream,indent=2); stream.write("\n")
    print(json.dumps({k:report.get(k) for k in ("result","failure","fresh_real_login","native_host")}))
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__": raise SystemExit(main())
