#!/usr/bin/env python3
"""World READ_ONLY component and actual four-owner/native publication drain in a new private socket-only PG.
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
parser.add_argument("--lib-binary",required=True,type=Path)
args=parser.parse_args()
if not all(p.is_absolute() for p in [args.binary,args.lib_binary,args.report,args.freeze]):
    parser.error("absolute binary and report required")
if shutil.disk_usage(ROOT).free < 8*1024**3:
    parser.error("8 GiB floor required")
binary=migration.executable(args.binary)
lib_binary=migration.executable(args.lib_binary)
freeze=json.loads(args.freeze.read_text())
if Path(freeze["source_root"])!=ROOT or freeze["cargo_exit"]!=0:
    parser.error("foreign or failed build receipt")
for path,digest in freeze["source_files"].items():
    source=ROOT/path
    if not source.is_relative_to(ROOT) or hashlib.sha256(source.read_bytes()).hexdigest()!=digest:
        parser.error("candidate source differs from frozen build")
pinned=[item for item in freeze["executables"] if item["name"]=="world_read_only" and Path(item["path"])==binary]
if len(pinned)!=1 or pinned[0]["sha256"]!=hashlib.sha256(binary.read_bytes()).hexdigest():
    parser.error("binary differs from frozen Cargo JSON executable")
lib_pinned=[item for item in freeze["executables"] if item["name"]=="concert_service" and Path(item["path"])==lib_binary]
if len(lib_pinned)!=1 or lib_pinned[0]["sha256"]!=hashlib.sha256(lib_binary.read_bytes()).hexdigest():parser.error("library executable differs from freeze")
report={"lib_binary_sha256":hashlib.sha256(lib_binary.read_bytes()).hexdigest(),"freeze_sha256":hashlib.sha256(args.freeze.read_bytes()).hexdigest(),"base":freeze["base"],"scope":"World component SQL effects/rollback/replay/event-first guard; actual Auth/Friend/Profile and native socket publication permit drain. Operator-role and art metadata fixtures; not full BE41 composite/root qualification or IN01", "binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"result":"FAIL"}
cluster=None
active=None
try:
    tools={name:migration.executable(Path("/opt/homebrew/opt/postgresql@16/bin") / name) for name in migration.PG_TOOLS}
    cluster=migration.Cluster(tools)
    report["owned_root"]=str(cluster.root)
    cluster.init(); cluster.start()
    report["postgres"]=cluster.sql("postgres","show server_version;")
    dbs={owner:f"mcr_{owner}_world_read_only" for owner in ["auth","friend","profile","concert"]}
    report["migration_sha256"]={}
    for owner,db in dbs.items():
        cluster.sql("postgres",f"create database {db} template template0;")
        for path in sorted((ROOT / f"backend/services/{owner}-service/migrations").glob("*.sql")):
            cluster.sql(db,path.read_text())
            report["migration_sha256"][str(path.relative_to(ROOT))]=hashlib.sha256(path.read_bytes()).hexdigest()
    urls={owner:cluster.url(db) for owner,db in dbs.items()}
    env={**cluster.env,"MCR_WORLD_READ_ONLY_ROOT":str(cluster.root),"MCR_WORLD_READ_ONLY_TOKEN":cluster.token,"MCR_WORLD_READ_ONLY_IDENTITY":f"{cluster.identity[0]}:{cluster.identity[1]}",**{f"MCR_WORLD_READ_ONLY_{owner.upper()}_URL":url for owner,url in urls.items()}}
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
    def run(binary,name):
        result=subprocess.run([str(binary),"--ignored","--exact",name,"--test-threads=1","--nocapture"],env=env,cwd=cluster.root,capture_output=True,text=True,timeout=60)
        record(name,result)
    run(binary,"native_event_first_guards_deny_all_authority_mutations")
    run(lib_binary,"world::closure::tests::native_component_rollback_replay_and_owner_capture")
    run(lib_binary,"world::closure::tests::native_real_publication_permit_and_socket_handoff_drain_before_start")
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
