#!/usr/bin/env python3
"""Freeze exact Operations Cargo JSON artifacts; run copies in a private PG cluster.

Never invokes Cargo. Release shared build resources before --pinned execution.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time
sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
TARGETS = ("operations_live_contract", "operations_live_native")
EXPECTED_CASES = {"operations_live_contract": 5, "operations_live_native": 6}
def sha(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def source_pins():
    paths = subprocess.check_output(["git", "ls-files", "backend", "contracts"], cwd=ROOT, text=True).splitlines()
    paths += subprocess.check_output(["git", "ls-files", "--others", "--exclude-standard", "backend/services/concert-service/src/operations_live", "backend/services/concert-service/tests/operations_live_support", "backend/services/concert-service/tests/operations_live_native.rs", "backend/services/concert-service/tests/operations_live_contract.rs", "backend/scripts/mcr/operations_live_smoke.py"], cwd=ROOT, text=True).splitlines()
    return {p:sha(ROOT/p) for p in sorted(set(paths)) if Path(p).suffix in {".rs", ".sql", ".proto", ".json", ".toml", ".lock", ".py"} and (ROOT/p).is_file()}
def freeze(paths):
    items=[json.loads(line) for path in paths for line in path.read_text().splitlines() if line.startswith("{")]
    artifacts={}
    for name in TARGETS:
        found=[a for a in items if a.get("reason")=="compiler-artifact" and a.get("executable") and a.get("target",{}).get("name")==name and a.get("target",{}).get("kind")==["test"] and a.get("profile",{}).get("test") and a.get("profile",{}).get("debuginfo")==0 and a.get("manifest_path")==str(ROOT/"backend/services/concert-service/Cargo.toml")]
        if len(found)!=1: raise ValueError("One exact debug-zero test artifact required: "+name)
        artifacts[name]=found[0]
    out=Path(tempfile.mkdtemp(prefix="mcr-operations-live-frozen-",dir="/tmp")).resolve()
    report={"head":subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),"source_sha256":source_pins(),"cargo_json_sha256":[sha(p) for p in paths],"artifacts":{}}
    for name,artifact in artifacts.items():
        src=Path(artifact["executable"])
        if src.is_symlink() or not src.is_file(): raise ValueError("Regular executable required")
        dst=out/name;shutil.copy2(src,dst)
        if sha(src)!=sha(dst): raise ValueError("Artifact copy mismatch")
        report["artifacts"][name]={"binary":str(dst),"sha256":sha(dst),"artifact":artifact}
    pin=out/"pin.json";pin.write_text(json.dumps(report,indent=2)+"\n");return pin

def verify(pin):
    out=pin.resolve(strict=True).parent
    if out.stat().st_mode & 0o077 or not out.name.startswith("mcr-operations-live-frozen-"): raise ValueError("Private artifact directory required")
    report=json.loads(pin.read_text())
    if set(report["artifacts"])!=set(TARGETS) or report["source_sha256"]!=source_pins():raise ValueError("Exact artifact/source inventory required")
    for name,item in report["artifacts"].items():
        p=Path(item["binary"])
        if p.is_symlink() or p.resolve().parent!=out or p.name!=name or sha(p)!=item["sha256"]:raise ValueError("Frozen artifact changed")
    return out,report

def qualify(pin,pg):
    out,report=verify(pin);cluster=None;report.update(result="FAIL",commands={})
    def save():(out/"native-report.json").write_text(json.dumps(report,indent=2)+"\n")
    def run(name,options,env):
        if cluster:cluster.assert_owned()
        item=report["artifacts"][name]
        if sha(item["binary"])!=item["sha256"]:raise ValueError("Frozen artifact changed")
        # Preserve the first failure even when a later case blocks on its retained
        # owner quarantine and the whole binary reaches the bounded timeout.
        log=out/(name+".log");args=[item["binary"],*options,"--nocapture","--test-threads=1"];started=time.monotonic()
        try:
            with log.open("w") as stream:code=subprocess.run(args,cwd=ROOT,env=env,stdout=stream,stderr=subprocess.STDOUT,timeout=60).returncode
        except subprocess.TimeoutExpired:code="TIMEOUT"
        text=log.read_text();passed=sum(int(n) for n in re.findall(r"test result: ok\. (\d+) passed;",text))
        report["commands"][name]={"args":args,"exit_code":code,"passed":passed,"seconds":time.monotonic()-started,"log_sha256":sha(log)};save()
        if code!=0 or passed!=EXPECTED_CASES[name]:raise ValueError("Test execution failed or incomplete: "+name)
        if cluster and cluster.password in text:raise ValueError("Private fixture secret in output")
    try:
        env={k:v for k,v in os.environ.items() if not k.startswith(("PG","MCR_CANONICAL_")) and not k.endswith("DATABASE_URL")}
        run("operations_live_contract",[],env)
        import migration_smoke as native
        from invalidation_clients_smoke import pki
        cluster=native.Cluster({n:native.executable(pg/n) for n in native.PG_TOOLS});report["owned_root"]=str(cluster.root);report["owner_identity"]=list(cluster.identity);save()
        cluster.init()
        with (cluster.data/"postgresql.conf").open("a") as stream:stream.write("\nmax_prepared_transactions = 256\n")
        cluster.start();pki(cluster)
        report["postgres_version"]=cluster.sql("postgres","show server_version;");report["max_prepared_transactions"]=cluster.sql("postgres","show max_prepared_transactions;")
        canonical={**env,**cluster.env,"CONCERT_SERVICE_CREDENTIAL":"concert-fixture","MCR_CANONICAL_ROOT":str(cluster.root),"MCR_CANONICAL_ROOT_IDENTITY":":".join(map(str,cluster.identity)),"MCR_CANONICAL_ROOT_TOKEN":cluster.token,"MCR_CANONICAL_SYSTEM":cluster.sql("postgres","select system_identifier from pg_control_system();")}
        for owner in ("auth","friend","concert"):
            db="mcr_canonical_"+owner
            native.execute([str(pg/"createdb"),"--no-password","--maintenance-db=postgres",db],env=cluster.client_env("postgres"),cwd=cluster.root,phase="create-"+owner)
            canonical["MCR_CANONICAL_"+owner.upper()+"_URL"]=cluster.url(db)
        run("operations_live_native",["--ignored"],canonical)
        report["migrations"]={owner:native.check_ledger(cluster,"mcr_canonical_"+owner,native.migration_manifest(ROOT/"backend",owner+"-service")) for owner in ("auth","friend","concert")}
        def durable_snapshot():
            statements={
                "witnesses":"select coalesce(jsonb_agg(to_jsonb(w) order by instance_id,scope_id,identity_digest),'[]'::jsonb)::text from concert_operations_native_witnesses w",
                "receipts":"select coalesce(jsonb_agg(jsonb_build_object('id',receipt_id,'state',state,'resource',resource_id,'revision',resource_revision) order by receipt_id),'[]'::jsonb)::text from concert_command_receipts",
                "outbox":"select coalesce(jsonb_agg(jsonb_build_object('id',outbox_id,'receipt',receipt_id,'resource',resource_id,'revision',resource_revision) order by outbox_id),'[]'::jsonb)::text from concert_outbox",
            }
            return {key:hashlib.sha256(cluster.sql("mcr_canonical_concert",sql).encode()).hexdigest() for key,sql in statements.items()}
        before=durable_snapshot();pid=cluster.assert_postmaster()
        if cluster.sql("postgres","select count(*) from pg_prepared_xacts;")!="0":raise ValueError("Unsettled native owner work before restart")
        cluster.stop();cluster.start()
        if cluster.assert_postmaster()==pid or durable_snapshot()!=before:raise ValueError("Native evidence restart persistence failed")
        if cluster.sql("mcr_canonical_concert","select count(*) from concert_operations_native_witnesses where pg_xact_status((identity->>'full_xid')::xid8) is distinct from 'committed';")!="0":raise ValueError("Restarted native outcome unresolved")
        after={owner:native.check_ledger(cluster,"mcr_canonical_"+owner,native.migration_manifest(ROOT/"backend",owner+"-service")) for owner in ("auth","friend","concert")}
        if after!=report["migrations"]:raise ValueError("Migration ledger changed after restart")
        report["restart"]={"result":"PASS","new_postmaster":True,"durable_sha256":before,"migration_ledgers_unchanged":True,"native_witnesses_committed":True}
        report["result"]="PASS"
    except BaseException as exc:report.update(result="FAIL",failure_type=type(exc).__name__)
    finally:
        if cluster:
            root=cluster.root
            try:
                if cluster.started:
                    rows=cluster.sql("postgres","select database||E'\\t'||gid from pg_prepared_xacts;").splitlines();report["cleanup_forced_private_rollbacks"]=len(rows)
                    for row in rows:
                        db,gid=row.split("\t")
                        prefix={"mcr_canonical_auth":"mcr_oa_a_","mcr_canonical_friend":"mcr_oa_f_"}.get(db)
                        if not prefix or not re.fullmatch(prefix+r"[0-9a-f]{32}",gid):raise ValueError("Unknown prepared quarantine retained")
                        cluster.sql(db,"rollback prepared '"+gid+"';")
                    if cluster.sql("postgres","select count(*) from pg_prepared_xacts;")!="0":raise ValueError("Prepared work remains")
                cluster.cleanup();report["cleanup"]={"prepared_quarantine_resolved":True,"exact_owned_root_absent":not root.exists(),"owned_server_stopped":True}
            except BaseException as exc:report.update(result="FAIL",cleanup={"failure_type":type(exc).__name__,"owned_root_retained":True})
        else:report["cleanup"]={"no_cluster_started":True}
        save()
    return report,out/"native-report.json"
def main():
    parser=argparse.ArgumentParser(description=__doc__);group=parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--cargo-json",action="append",type=Path);group.add_argument("--pinned",type=Path);parser.add_argument("--pg-bin",type=Path,default=Path("/opt/homebrew/opt/postgresql@16/bin"));args=parser.parse_args()
    if args.cargo_json:print(json.dumps({"pinned":str(freeze(args.cargo_json))}));return 0
    report,path=qualify(args.pinned,args.pg_bin);print(json.dumps({"result":report["result"],"report":str(path),"cleanup":report["cleanup"]}));return 0 if report["result"]=="PASS" else 1
if __name__=="__main__":raise SystemExit(main())
