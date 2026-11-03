#!/usr/bin/env python3
"""Actual Concert block/receipt/outbox to World native handoff in a new private socket-only PG.
Migrations are applied directly: this is not serving-ledger or IN01 evidence.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
ROOT=Path(__file__).resolve().parents[5]
sys.path.insert(0,str(ROOT / "backend/scripts/mcr"))
import migration_smoke as migration
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument("--binary",required=True,type=Path)
parser.add_argument("--report",required=True,type=Path)
parser.add_argument("--freeze",required=True,type=Path)
args=parser.parse_args()
if not all(p.is_absolute() for p in [args.binary,args.report,args.freeze]):
    parser.error("absolute binary and report required")
if shutil.disk_usage(ROOT).free < 8*1024**3:
    parser.error("8 GiB floor required")
binary=migration.executable(args.binary)
freeze=json.loads(args.freeze.read_text())
if Path(freeze["source_root"])!=ROOT or freeze["cargo_exit"]!=0:
    parser.error("foreign or failed build receipt")
for path,digest in freeze["source_files"].items():
    source=ROOT/path
    if not source.is_relative_to(ROOT) or hashlib.sha256(source.read_bytes()).hexdigest()!=digest:
        parser.error("candidate source differs from frozen build")
pinned=[item for item in freeze["executables"] if item["name"]=="world_invalidation" and Path(item["path"])==binary]
if len(pinned)!=1 or pinned[0]["sha256"]!=hashlib.sha256(binary.read_bytes()).hexdigest():
    parser.error("binary differs from frozen Cargo JSON executable")
report={"freeze_sha256":hashlib.sha256(args.freeze.read_bytes()).hexdigest(),"base":freeze["base"],"scope":"Actual Concert local block/receipt/outbox/consumer and native WebSocket handoff; synthetic missing remote authority and metadata art; production unavailable; NOT IN01", "binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"result":"FAIL"}
cluster=None
active=None
try:
    tools={name:migration.executable(Path("/opt/homebrew/opt/postgresql@16/bin") / name) for name in migration.PG_TOOLS}
    cluster=migration.Cluster(tools)
    report["owned_root"]=str(cluster.root)
    cluster.init(); cluster.start()
    report["postgres"]=cluster.sql("postgres","show server_version;")
    dbs={"concert":"mcr_world_invalidation"}
    report["migration_sha256"]={}
    for owner,db in dbs.items():
        cluster.sql("postgres",f"create database {db} template template0;")
        for path in sorted((ROOT / f"backend/services/{owner}-service/migrations").glob("*.sql")):
            cluster.sql(db,path.read_text())
            report["migration_sha256"][str(path.relative_to(ROOT))]=hashlib.sha256(path.read_bytes()).hexdigest()
    urls={owner:cluster.url(db) for owner,db in dbs.items()}
    env={**cluster.env,"WORLD_INVALIDATION_TEST_OWNER_ROOT":str(cluster.root),"WORLD_INVALIDATION_TEST_OWNER_TOKEN":cluster.token,"WORLD_INVALIDATION_TEST_OWNER_IDENTITY":f"{cluster.identity[0]}:{cluster.identity[1]}","WORLD_INVALIDATION_TEST_DATABASE_URL":urls["concert"]}
    report["runs"]=[]
    def record(name,result):
        raw=result.stdout+result.stderr
        if any(value and value in raw for value in [cluster.password,cluster.token,*urls.values()]):
            raise RuntimeError("native output redaction required")
        log=args.report.with_name(args.report.stem+"-"+name+".log");log.write_text(raw)
        counts=re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;",result.stdout)
        item={"test":name,"exit":result.returncode,"log_sha256":hashlib.sha256(log.read_bytes()).hexdigest(),"passed":counts==[("1","0","0")]}
        report["runs"].append(item)
        if result.returncode!=0 or not item["passed"]:
            raise RuntimeError("native test failed or absent")
    def command(name):
        return [str(binary),"--ignored","--exact","native::"+name,"--test-threads=1","--nocapture"]
    def run(name):
        result=subprocess.run(command(name),env=env,cwd=cluster.root,capture_output=True,text=True,timeout=60)
        record(name,result)
    run("native_block_outbox_fence_commit_retry_and_native_send")
    first=cluster.assert_postmaster();cluster.stop();cluster.start();second=cluster.assert_postmaster()
    if first==second:raise RuntimeError("postmaster did not change")
    report["restart_postmasters"]=[first,second]
    run("native_restart_durable_receipt_preserves_fresh_connection")
    run("native_failed_commit_retry_duplicate_reorder_and_metadata_fences")
    run("native_captured_credential_expiry_closes_without_renewal")
    run("native_inflight_handoff_serializes_before_invalidation_ack")
    name="native_database_outage_never_acknowledges_and_expires_live_views"
    active=subprocess.Popen(command(name),env=env,cwd=cluster.root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    began=time.monotonic()
    while not (cluster.root/"outage-ready").exists():
        if active.poll() is not None or time.monotonic()-began>10:raise RuntimeError("outage handshake failed")
        time.sleep(0.01)
    report["outage_postmaster"]=cluster.assert_postmaster()
    cluster.stop();(cluster.root/"outage-stopped").write_text("stopped")
    stdout,stderr=active.communicate(timeout=20)
    record(name,subprocess.CompletedProcess(command(name),active.returncode,stdout,stderr));active=None
    cluster.start()
    report["outage_unacknowledged"]=cluster.sql("mcr_world_invalidation","select count(*) from concert_outbox where state='LEASED';")
    if report["outage_unacknowledged"]!="1":raise RuntimeError("outage queue state changed")
    report["native_tests"]=len(report["runs"])
    report["result"]="PASS"
except BaseException as error:
    report.update(result="FAIL",failure=type(error).__name__)
finally:
    try:
        if active is not None and active.poll() is None:
            active.terminate();active.communicate(timeout=5)
        if cluster is not None:
            cluster.cleanup()
            if cluster.root.exists(): raise RuntimeError("owned root remains")
        report["cleanup"]="PASS"
    except BaseException as error:
        report.update(result="FAIL",cleanup="FAIL",cleanup_error=type(error).__name__)
    args.report.write_text(json.dumps(report,indent=2)+"\n")
print(json.dumps({key:report.get(key) for key in ["result","native_exit","native_tests","cleanup","failure"]}))
sys.exit(0 if report["result"]=="PASS" else 1)
