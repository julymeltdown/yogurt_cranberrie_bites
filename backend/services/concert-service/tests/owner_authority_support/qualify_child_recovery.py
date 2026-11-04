#!/usr/bin/env python3
"""Owned PG16/native-adapter component qualification; not CR01-CR06 acceptance."""
from __future__ import annotations
import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path
sys.dont_write_bytecode = True
import qualify as core

CASES = (
    "finished_native_children_progress_past_unknown_and_retain_capacity",
    "active_and_sealed_expired_native_children_remain_quarantined",
    "retained_key_timeout_and_concurrent_retry_require_real_native_abort",
)

def source_manifest():
    # The public reader validates real source-bound art/release files at runtime.
    # Pin those resources too, as Public's separate native controller does.
    paths = set(core.source_manifest())
    tracked = subprocess.check_output([
        "git", "ls-files", "backend/releases", "contracts", "frontend/public",
        "frontend/src/entities/avatar/character-source-pack.json",
    ], cwd=core.ROOT, text=True).splitlines()
    paths.update(p for p in tracked if (core.ROOT/p).is_file())
    return {p: core.sha(core.ROOT/p) for p in sorted(paths)}

def run(checkpoint: dict, pg: Path, report: dict) -> None:
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=core.ROOT, text=True).strip()
    if checkpoint["source_commit"] != commit or checkpoint["source_sha256"] != source_manifest():
        raise core.SmokeError("preflight", "source checkpoint changed")
    if checkpoint.get("target_name") != "child_recovery_runtime":
        raise core.SmokeError("preflight", "exact child recovery test target required")
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=core.ROOT, text=True).strip():
        raise core.SmokeError("preflight", "clean source required")
    binary = core.frozen_binary(Path(checkpoint["binary"]), checkpoint["binary_sha256"])
    if shutil.disk_usage(core.ROOT).free < 8*1024**3:
        raise core.SmokeError("preflight", "8 GiB free required")
    tools = {name: core.executable(pg/name) for name in core.PG_TOOLS}
    cluster = core.Cluster(tools)
    report.update(source_commit=commit, source_sha256=checkpoint["source_sha256"],
                  binary_sha256=checkpoint["binary_sha256"], phases={})
    try:
        cluster.init()
        with (cluster.data/"postgresql.conf").open("a") as stream:
            stream.write("\nmax_prepared_transactions=256\n")
        cluster.start()
        core.pki(cluster)
        if int(cluster.sql("postgres", "show server_version_num;"))//10000 != 16:
            raise core.SmokeError("preflight", "PG16 required")
        report["postgres_version"] = cluster.sql("postgres", "show server_version;")
        db = "mcr_canonical_concert"
        core.execute([str(tools["createdb"]), "--no-password", "--maintenance-db=postgres", db],
                     env=cluster.client_env("postgres"), phase="create-concert", cwd=cluster.root)
        env = {**cluster.env, "MCR_CANONICAL_ROOT":str(cluster.root),
               "MCR_CANONICAL_ROOT_IDENTITY":f"{cluster.identity[0]}:{cluster.identity[1]}",
               "MCR_CANONICAL_ROOT_TOKEN":cluster.token,
               "MCR_CANONICAL_SYSTEM":cluster.sql("postgres", "select system_identifier from pg_control_system();"),
               "MCR_CANONICAL_CONCERT_URL":cluster.url(db)}
        for case in CASES:
            result = core.execute([str(binary), "--ignored", "--exact", case, "--test-threads=1", "--nocapture"],
                                  env=env, phase=case, cwd=core.ROOT/"backend", timeout=45)
            if "test result: ok. 1 passed; 0 failed;" not in result.stdout:
                raise core.SmokeError("inventory", "one exact passing case required")
            report["phases"][case] = {"status":"PASS", "exit_code":result.returncode, "stdout":result.stdout}
        report["migration_receipts"] = core.check_ledger(cluster, db, core.migration_manifest(core.ROOT/"backend", "concert-service"))
        remaining = int(cluster.sql(db, "select count(*) from pg_prepared_xacts where database=current_database();"))
        report["remaining_prepared"] = remaining
        if remaining:
            raise core.SmokeError("cleanup", "native prepared work remains")
    finally:
        cluster.cleanup()
        report["cleanup"]={"status":"PASS", "owned_root_absent":not cluster.root.exists()}

def main() -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint",type=Path,required=True)
    parser.add_argument("--pg-bin-dir",type=Path,required=True)
    parser.add_argument("--report",type=Path,required=True)
    args=parser.parse_args()
    if not args.report.is_absolute() or args.report.exists() or not args.pg_bin_dir.is_absolute():
        parser.error("new absolute report and explicit absolute PG directory required")
    report={"status":"FAIL", "limitations":[
        "Real SQL lifecycle/native-adapter components with fixture-authored ledger setup, not actual canonical Root terminal proof.",
        "Unavailable/timeout adapters only inject negative failures; all successful releases delegate to the actual PublicProfileReader LocalPublic native adapter.",
        "Same-process adapter reconstruction is not a physical Root process restart. Actual Auth/Friend/Profile acquisition, CR01-CR06 and production main activation remain NOT_RUN.",
        "Unfinished native work is explicitly torn down without ACK or phase promotion; production worker leaves it quarantined."]}
    try:
        run(json.loads(args.checkpoint.read_text()),args.pg_bin_dir,report)
        report["status"]="PASS"
    except core.SmokeError as error:
        report["failure"]=error.detail
    except Exception as error:
        report["failure"]={"phase":"runner", "diagnostic":type(error).__name__, "detail":str(error)}
    finally:
        args.report.write_text(json.dumps(report,indent=2,sort_keys=True)+"\n")
    print(json.dumps({"status":report["status"], "report":str(args.report)}))
    return 0 if report["status"]=="PASS" else 1

if __name__=="__main__":
    sys.exit(main())
