#!/usr/bin/env python3
"""Run one frozen CR08 case in a fresh owned socket-only PG16/mTLS fixture."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/"owner_authority_support"))
import qualify as core

CASES=("returned_error_records_abandonment_without_delivery",
       "caller_cancellation_destroys_actual_work_before_owner_release",
       "unresolved_callback_and_expiry_never_prove_abandonment",
       "active_native_transaction_prevents_cancellation_settlement",
       "successful_handoff_retains_existing_unserialized_quarantine",
       "drained_late_callback_allows_true_abandonment",
       "expired_original_deadline_never_polls_ready_producer",
       "failed_work_destruction_never_reaches_terminal_proof",
       "unpublished_unsettled_abandonment_remains_in_durable_capacity")
QUARANTINE={CASES[2],CASES[3],CASES[4],CASES[7],CASES[8]}

def run(args,report):
    checkpoint=json.loads(args.checkpoint.read_text())
    head=subprocess.check_output(["git","rev-parse","HEAD"],cwd=core.ROOT,text=True).strip()
    if (checkpoint.get("source_commit")!=head or checkpoint.get("source_sha256")!=core.source_manifest()
        or checkpoint.get("exit_code")!=0 or checkpoint.get("target_name")!="owner_authority_cancellation"
        or subprocess.check_output(["git","status","--porcelain"],cwd=core.ROOT,text=True).strip()):
        raise core.SmokeError("preflight","clean exact successful build checkpoint required")
    binary=core.frozen_binary(Path(checkpoint["binary"]),checkpoint["binary_sha256"])
    if shutil.disk_usage(core.ROOT).free<8*1024**3:
        raise core.SmokeError("preflight","8GiB free required")
    report.update(source_commit=head,source_sha256=checkpoint["source_sha256"],binary_sha256=checkpoint["binary_sha256"])
    tools={name:core.executable(args.pg_bin_dir/name) for name in core.PG_TOOLS}
    cluster=core.Cluster(tools)
    try:
        cluster.init()
        with (cluster.data/"postgresql.conf").open("a") as stream:stream.write("\nmax_prepared_transactions=256\n")
        cluster.start();core.pki(cluster)
        if int(cluster.sql("postgres","show server_version_num;"))//10000!=16:
            raise core.SmokeError("preflight","PG16 required")
        env={**cluster.env,"CONCERT_SERVICE_CREDENTIAL":"concert-fixture",
             "MCR_CANONICAL_ROOT":str(cluster.root),"MCR_CANONICAL_ROOT_IDENTITY":f"{cluster.identity[0]}:{cluster.identity[1]}",
             "MCR_CANONICAL_ROOT_TOKEN":cluster.token,"MCR_CANONICAL_SYSTEM":cluster.sql("postgres","select system_identifier from pg_control_system();")}
        for owner in core.OWNERS:
            db="mcr_canonical_"+owner
            core.execute([str(tools["createdb"]),"--no-password","--maintenance-db=postgres",db],env=cluster.client_env("postgres"),cwd=cluster.root,phase="create-"+owner)
            env["MCR_CANONICAL_"+owner.upper()+"_URL"]=cluster.url(db)
        result=core.execute([str(binary),"--ignored","--exact","cases::"+args.case,"--test-threads=1"],env=env,cwd=core.ROOT/"backend",phase="cancellation-case",timeout=40)
        if "test result: ok. 1 passed; 0 failed;" not in result.stdout:
            raise core.SmokeError("inventory","one exact test must run")
        report["test"]={"exit_code":result.returncode,"passed":1}
        report["migration_receipts"]={owner:core.check_ledger(cluster,"mcr_canonical_"+owner,core.migration_manifest(core.ROOT/"backend",owner+"-service")) for owner in core.OWNERS}
        remaining={owner:int(cluster.sql("mcr_canonical_"+owner,"select count(*) from pg_prepared_xacts where database=current_database();")) for owner in core.OWNERS}
        report["remaining_prepared"]=remaining
        expected={"auth":1,"friend":1,"concert":0} if args.case in QUARANTINE else {owner:0 for owner in core.OWNERS}
        if remaining!=expected:raise core.SmokeError("native-evidence","unexpected exact prepared counts")
        report["normal_native_settlement"]=args.case not in QUARANTINE
    finally:
        cluster.cleanup()
        report["cleanup"]={"status":"PASS","owned_root_absent":not cluster.root.exists(),"certifies_native_settlement":False}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case",choices=CASES,required=True)
    parser.add_argument("--checkpoint",type=Path,required=True)
    parser.add_argument("--pg-bin-dir",type=Path,required=True)
    parser.add_argument("--report",type=Path,required=True)
    args=parser.parse_args()
    if not args.report.is_absolute() or args.report.exists() or not args.pg_bin_dir.is_absolute():parser.error("new absolute report and explicit PG16 directory required")
    report={"status":"FAIL","case":args.case,"limitations":["Core actual owner/ledger cancellation only; Participant HTTP and production main qualification are separate.","Expected quarantine is observed before exact owned cluster teardown; teardown is not settlement."]}
    try:run(args,report);report["status"]="PASS"
    except core.SmokeError as error:report["failure"]=error.detail
    except Exception as error:report["failure"]={"phase":"runner","diagnostic":type(error).__name__}
    finally:
        with os.fdopen(os.open(args.report,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600),"w") as stream:json.dump(report,stream,sort_keys=True,indent=2);stream.write("\n")
    print(json.dumps({"status":report["status"],"report":str(args.report)}))
    return 0 if report["status"]=="PASS" else 1
if __name__=="__main__":raise SystemExit(main())
