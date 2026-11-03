#!/usr/bin/env python3
"""Actual Auth/Friend/Profile/Concert owner composition in a new private socket-only PG.
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
ROOT=Path(__file__).resolve().parents[5]
sys.path.insert(0,str(ROOT / "backend/scripts/mcr"))
import migration_smoke as migration
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument("--binary",required=True,type=Path)
parser.add_argument("--report",required=True,type=Path)
args=parser.parse_args()
if not args.binary.is_absolute() or not args.report.is_absolute():
    parser.error("absolute binary and report required")
if shutil.disk_usage(ROOT).free < 8*1024**3:
    parser.error("8 GiB floor required")
binary=migration.executable(args.binary)
report={"scope":"Actual guarded Auth/Friend/Profile and Concert local publication guard; synthetic art metadata; remote invalidation missing; production startup unavailable; NOT IN01", "binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"result":"FAIL"}
cluster=None
try:
    tools={name:migration.executable(Path("/opt/homebrew/opt/postgresql@16/bin") / name) for name in migration.PG_TOOLS}
    cluster=migration.Cluster(tools)
    report["owned_root"]=str(cluster.root)
    cluster.init(); cluster.start()
    report["postgres"]=cluster.sql("postgres","show server_version;")
    dbs={owner:f"mcr_{owner}_world_composition" for owner in ["auth","friend","profile","concert"]}
    report["migration_sha256"]={}
    for owner,db in dbs.items():
        cluster.sql("postgres",f"create database {db} template template0;")
        for path in sorted((ROOT / f"backend/services/{owner}-service/migrations").glob("*.sql")):
            cluster.sql(db,path.read_text())
            report["migration_sha256"][str(path.relative_to(ROOT))]=hashlib.sha256(path.read_bytes()).hexdigest()
    urls={owner:cluster.url(db) for owner,db in dbs.items()}
    env={**cluster.env,"MCR_WORLD_COMPOSITION_ROOT":str(cluster.root),"MCR_WORLD_COMPOSITION_TOKEN":cluster.token,"MCR_WORLD_COMPOSITION_IDENTITY":f"{cluster.identity[0]}:{cluster.identity[1]}",**{f"MCR_WORLD_COMPOSITION_{owner.upper()}_URL":url for owner,url in urls.items()}}
    name="world::admission_bridge::owner_composition_tests::native_actual_owners_admit_switch_renew_and_revocation"
    result=subprocess.run([str(binary),"--ignored","--exact",name,"--test-threads=1","--nocapture"],env=env,cwd=cluster.root,capture_output=True,text=True,timeout=60)
    raw=result.stdout+result.stderr
    if any(value and value in raw for value in [cluster.password,cluster.token,*urls.values()]):
        raise RuntimeError("native output redaction required")
    log=args.report.with_suffix(".log"); log.write_text(raw)
    counts=re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;",result.stdout)
    report.update(native_exit=result.returncode,log_sha256=hashlib.sha256(log.read_bytes()).hexdigest(),native_tests=1 if counts==[("1","0","0")] else 0)
    if result.returncode!=0 or report["native_tests"]!=1:
        raise RuntimeError("native composition failed or absent")
    report["result"]="PASS"
except BaseException as error:
    report.update(result="FAIL",failure=type(error).__name__)
finally:
    try:
        if cluster is not None:
            cluster.cleanup()
            if cluster.root.exists(): raise RuntimeError("owned root remains")
        report["cleanup"]="PASS"
    except BaseException as error:
        report.update(result="FAIL",cleanup="FAIL",cleanup_error=type(error).__name__)
    args.report.write_text(json.dumps(report,indent=2)+"\n")
print(json.dumps({key:report.get(key) for key in ["result","native_exit","native_tests","cleanup","failure"]}))
sys.exit(0 if report["result"]=="PASS" else 1)
