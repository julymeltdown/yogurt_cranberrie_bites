#!/usr/bin/env python3
"""CR10 actual production Participant/main and Gateway router error-EOF qualification.

Setup derives from world_in01_live.py at c9dd4c980f39b12eda5a79fff871c8de48f8df71.
Owned native PG16/mTLS only, frozen peers with independently checked input receipts.
The proxy schedules genuine typed replies and shortens only their content ceiling.
"""
from __future__ import annotations
import argparse
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import secrets
import signal
import socket
import struct
import subprocess
import sys
import time
from unittest.mock import patch
from urllib.parse import urlsplit
from uuid import uuid4
sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parents[5]
sys.path.insert(0,str(ROOT/"backend/scripts/mcr"))
import dev_stack as dev
import participant_live_join_smoke as joined
import world_in01_prepare as prep
import world_in01_live as live
pki,tls,public_json,initialize_profiles=live.pki,live.tls,live.public_json,live.initialize_profiles
TEST="native::actual_native_participant_delivery_retention_host"

def require(value,label):prep.auth.require(value,label)
def wait_file(path,process,timeout=15):
 stop=time.monotonic()+timeout
 while not path.is_file():
  require(process.poll() is None,"actual-retention-host-exited")
  require(time.monotonic()<stop,"retention-marker-timeout")
  time.sleep(.01)
 return json.loads(path.read_bytes())
def pending(cluster):return int(cluster.sql("postgres","SELECT count(*) FROM pg_prepared_xacts"))
def observe(cluster,scope):
 return json.loads(cluster.sql("mcr_concert","SELECT row_to_json(x) FROM (SELECT phase,serialized_at IS NOT NULL AS serialized,delivery_completed_at IS NOT NULL AS delivered,response_abandoned_at IS NOT NULL AS abandoned,cancellation_settled_at IS NOT NULL AS cancellation_settled FROM concert_product_owner_scopes WHERE scope_id="+prep.quote(scope)+") x"))
def wait_pending(cluster,count):
 stop=time.monotonic()+4
 while pending(cluster)!=count:
  require(time.monotonic()<stop,"prepared-count")
  time.sleep(.01)
def source_manifest():
 files=[ROOT/"backend/Cargo.toml",ROOT/"backend/Cargo.lock"]
 for folder in ("backend/services","backend/crates","backend/proto","backend/scripts/mcr"):
  files.extend(p for p in (ROOT/folder).rglob("*") if p.is_file() and p.suffix in {".rs",".toml",".sql",".proto",".py",".json"})
 return {str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(set(files))}
def request(url,event,token,origin,partial=False):
 target=urlsplit(url);connection=socket.create_connection((target.hostname,target.port),timeout=5)
 payload=(f"GET /api/concert/events/{event}/me HTTP/1.1\r\nHost: {target.netloc}\r\nAuthorization: Bearer {token}\r\nOrigin: {origin}\r\nX-MCR-Intent: browser\r\nConnection: close\r\n\r\n").encode()
 connection.sendall(payload)
 return connection

def run(args):
    receipt=json.loads(args.receipt.read_bytes())
    require(receipt["source_sha256"]==source_manifest() and receipt["exit_code"]==0 and receipt["target_name"]=="participant_delivery_retention","actual-compiled-inputs")
    require(dev.runtime.digest(args.binary)==receipt["binary_sha256"],"actual-host-hash")
    cohort=json.loads(args.cohort.read_bytes())
    for name in dev.runtime.START_ORDER:
        require(dev.runtime.digest(args.bin_dir/name)==cohort[name]["sha256"],"frozen-peer-hash")
    fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    report = {"result": "FAIL", "scope": "CR10 real Auth/PG16/mTLS/Participant/error HTTP EOF",
              "statuses": {}, "world_two_client": "NOT_RUN", "browser_in01": "NOT_RUN",
              "driver_sha256": dev.runtime.digest(Path(__file__)), "compiled_source":receipt["source_commit"], "binary_sha256":receipt["binary_sha256"]}
    stack = dev.DevStack(args.bin_dir, Path("/opt/homebrew/bin"),
        Path("/opt/homebrew/bin/redis-server"), Path("/opt/homebrew/opt/openssl@3/bin/openssl"), args.browser_origin)
    child = None
    host = None
    host_output = None
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
        context.update(env=env, gateway_env=envs["api-gateway"].copy(), event=event, rooms=json.loads(rooms), release=release, digest=digest)
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
        principals = [prep.principal(stack, browser, i) for i in (1, 2, 3)]
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
        # A separate source-built actual Gateway router forwards genuine signed
        # requests through an mTLS fixture proxy to the production Concert main.
        host_env={**context["gateway_env"],"MCR_RETENTION_ROOT":str(stack.cluster.root),
            "MCR_RETENTION_ROOT_TOKEN":stack.cluster.token,
            "MCR_RETENTION_ROOT_IDENTITY":f"{stack.cluster.identity[0]}:{stack.cluster.identity[1]}"}
        host_output=(stack.cluster.root/"retention-host.log").open("w+")
        host=subprocess.Popen([str(args.binary),"--ignored","--exact",TEST,"--test-threads=1","--nocapture"],env=host_env,cwd=ROOT,stdout=host_output,stderr=subprocess.STDOUT)
        ready=wait_file(stack.cluster.root/"retention-host-ready.json",host)
        proxy_url=ready["http_url"]
        credentials=json.loads(principals[0][1].read_bytes())
        headers,login=browser.request("login-before-retention","/api/auth/login",{"email":credentials["email"],"password":credentials["password"]})
        cookie=prep.auth.refresh_cookie(headers)
        require(login["userId"]==principals[0][0],"real-refresh-principal")
        prep.private_json(stack.cluster.root/"retention-mode.json",1)
        connection=request(proxy_url,context["event"],principals[0][2],browser.origin)
        response=http.client.HTTPResponse(connection);response.begin()
        payload=response.read(65537)
        require(response.status==503 and len(payload)<65536,"closed-expired-response-status")
        require(response.getheader("cache-control")=="no-store","closed-no-store")
        value=json.loads(payload)
        require(value["details"]["reason"]=="DEPENDENCY_UNAVAILABLE","closed-error-reason")
        require(all(hidden.encode() not in payload for hidden in [principals[0][0],context["event"],"completion_secret","binding_digest","Owned"]),"no-protected-content")
        connection.close()
        reply=wait_file(stack.cluster.root/"retention-reply-1.json",host)
        wait_pending(stack.cluster,0)
        report["positive_root"]=observe(stack.cluster,reply["scope_id"])
        require(report["positive_root"]=={"phase":"ROOT_FINISHED","serialized":True,"delivered":True,"abandoned":False,"cancellation_settled":False},"actual-error-EOF-delivery-settlement")
        report["owners_after_positive"]={role:json.loads(stack.cluster.sql("mcr_"+role,"SELECT coalesce(json_agg(x),'[]') FROM (SELECT state,scope_settlement IS NOT NULL AS settled,count(*) FROM concert_owner_guard_leases GROUP BY 1,2) x")) for role in ("auth","friend")}
        require(all(rows==[{"state":"COMPLETED","settled":True,"count":1}] for rows in report["owners_after_positive"].values()),"both-real-owners-completed")
        headers,refreshed=browser.request("refresh-after-error-EOF","/api/auth/refresh",cookie=cookie)
        require(refreshed["userId"]==principals[0][0] and refreshed["accessToken"] and refreshed["refreshToken"]=="","genuine-refresh-after-settlement")
        prep.auth.refresh_cookie(headers)
        report["actual_terminal_error_EOF"]="PASS";report["fresh_real_auth_refresh"]="PASS"
        negatives=[]
        for selected,principal in [(3,principals[1]),(4,principals[2])]:
            (stack.cluster.root/"retention-mode.json").unlink()
            prep.private_json(stack.cluster.root/"retention-mode.json",selected)
            connection=request(proxy_url,context["event"],principal[2],browser.origin)
            observed=b""
            # mode3 gets headers but no body; mode4 gets only half a DATA frame.
            while b"\r\n\r\n" not in observed or (selected==4 and not observed.split(b"\r\n\r\n",1)[1]):
                part=connection.recv(8192);require(bool(part),"negative-http-open");observed+=part
            require(observed.startswith(b"HTTP/1.1 503"),"negative-closed-status")
            connection.setsockopt(socket.SOL_SOCKET,socket.SO_LINGER,struct.pack("ii",1,0));connection.close()
            dropped=wait_file(stack.cluster.root/f"retention-body-dropped-{selected}.json",host)
            require(dropped["partial_data"]==(selected==4),"actual-body-drop-boundary")
            reply=wait_file(stack.cluster.root/f"retention-reply-{selected}.json",host)
            wait_pending(stack.cluster,len(negatives)*2+2)
            # TTL passage never substitutes for body/producer terminal proof.
            time.sleep(1.1)
            state=observe(stack.cluster,reply["scope_id"])
            require(state["serialized"] and not state["delivered"] and not state["abandoned"] and not state["cancellation_settled"],"negative-stays-quarantined")
            negatives.append({"mode":selected,"root":state,"partial_data":dropped["partial_data"],"prepared":pending(stack.cluster)})
        report["negative_resets"]=negatives
        require(pending(stack.cluster)==4,"four-real-fences-retained")
        report["prepared_before_owned_shutdown"]=4
        prep.private_json(stack.cluster.root/"retention-stop.json",{})
        require(host.wait(timeout=5)==0,"native-host-result")
        host_output.seek(0);require("1 passed; 0 failed; 0 ignored" in host_output.read(),"exact-native-host-passed")
        report["result"]="PASS"
    except (Exception,KeyboardInterrupt) as error:
        report["failure"]=str(error) if isinstance(error,prep.auth.CheckFailure) else type(error).__name__
        if stack.cluster is not None and stack.cluster.started:
            try:
                report["prepared_at_failure"]=pending(stack.cluster)
                report["roots_at_failure"]=json.loads(stack.cluster.sql("mcr_concert","SELECT coalesce(json_agg(x),'[]') FROM (SELECT phase,serialized_at IS NOT NULL AS serialized,delivery_completed_at IS NOT NULL AS delivered,response_abandoned_at IS NOT NULL AS abandoned,cancellation_settled_at IS NOT NULL AS cancellation_settled FROM concert_product_owner_scopes) x"))
            except Exception:report["failure_diagnostics"]="UNAVAILABLE"
        if host_output is not None:
            host_output.flush();host_output.seek(0);report["host_output"]=host_output.read()[-4000:]
    finally:
        for process in [host]:
            if process is not None and process.poll() is None:
                process.terminate()
                try:process.wait(timeout=3)
                except subprocess.TimeoutExpired:process.kill();process.wait(timeout=3);report["forced_host_stop"]=True
        if host_output is not None:host_output.close()
        if child is not None:
            try:report["concert_stop"]=child.stop()
            except Exception:report["concert_stop"]="FAIL"
        try:stack.close()
        except BaseException:stack.report["cleanup"]="FAIL"
        report["owned_cleanup"]=stack.report
        if stack.report.get("cleanup")!="PASS":report["result"]="FAIL"
        for sig,handler in previous.items():signal.signal(sig,handler)
    report["limits"]=["Test-owned typed mTLS reply proxy shortens disclosure ceiling; no signed-JWT or clock mutation.","Exact production incident/JWT expiry scheduling and full browser IN01 not reproduced.","No fabricated capability, EOF, CompleteResponse, TTL fence release, canonical data rewrite or detached producer work.","Negative reset/partial-body fences survive until disposable owned-cluster teardown, which is not settlement proof."]
    with os.fdopen(fd,"w") as stream:json.dump(report,stream,indent=2);stream.write("\n")
    return report

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ("binary","receipt","bin-dir","cohort","concert-bin","report"):parser.add_argument("--"+name,type=Path,required=True)
    args=parser.parse_args();require(all(p.is_absolute() for p in vars(args).values()),"absolute-explicit-inputs")
    args.world=False;args.browser_origin="http://localhost:5173"
    report=run(args);print(json.dumps({k:report.get(k) for k in ("result","failure","actual_terminal_error_EOF","fresh_real_auth_refresh")}))
    return 0 if report["result"]=="PASS" else 1
if __name__=="__main__":raise SystemExit(main())
