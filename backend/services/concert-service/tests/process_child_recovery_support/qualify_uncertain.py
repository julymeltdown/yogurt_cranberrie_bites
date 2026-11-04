#!/usr/bin/env python3
"""CR02/CR05 owned socket-only PG16 runner. Inert without --execute."""
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
sys.dont_write_bytecode = True
# Staged beside checkpoint.py/controller.py in process_child_recovery_support.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]/"owner_authority_support"))
import qualify as core
import checkpoint as pin
import migration_smoke as migration
OWNERS = ("auth", "friend", "concert", "profile")

def run(args, report):
    prefix = "process-" + args.case.lower()
    repo = args.repo.resolve(strict=True)
    binary, digest, commit = pin.verify_checkpoint(repo, args.checkpoint)
    pin_receipt = json.loads(args.checkpoint.read_text())
    manifest = pin_receipt.get("source_manifest", pin_receipt.get("source_sha256", {}))
    self_path = "backend/services/concert-service/tests/process_child_recovery_support/qualify_uncertain.py"
    if manifest.get(self_path) != hashlib.sha256(Path(__file__).read_bytes()).hexdigest():
        raise core.SmokeError("preflight", "executing uncertainty runner source mismatch")
    if shutil.disk_usage(repo).free < 8*1024**3:
        raise core.SmokeError("preflight", "8 GiB free required")
    receipt = json.loads(args.checkpoint.read_text())
    if receipt.get("exit_code") != 0:
        raise core.SmokeError("preflight", "successful build checkpoint required")
    report.update(source_commit=commit, binary_sha256=digest,
                  source_sha256=receipt.get("source_sha256",receipt.get("source_manifest")))
    tools = {name:core.executable(args.pg_bin_dir/name) for name in core.PG_TOOLS}
    cluster = core.Cluster(tools)
    env = {}
    try:
        cluster.init()
        with (cluster.data/"postgresql.conf").open("a") as stream:
            stream.write("\nmax_prepared_transactions=256\n")
        cluster.start()
        core.pki(cluster)
        if int(cluster.sql("postgres","show server_version_num;"))//10000 != 16:
            raise core.SmokeError("preflight","PG16 required")
        report["postgres_version"] = cluster.sql("postgres","show server_version;")
        env = {**cluster.env, "CONCERT_SERVICE_CREDENTIAL":"concert-fixture",
               "MCR_CANONICAL_ROOT":str(cluster.root),
               "MCR_CANONICAL_ROOT_IDENTITY":f"{cluster.identity[0]}:{cluster.identity[1]}",
               "MCR_CANONICAL_ROOT_TOKEN":cluster.token,
               "MCR_CANONICAL_SYSTEM":cluster.sql("postgres","select system_identifier from pg_control_system();")}
        for owner in OWNERS:
            db = "mcr_canonical_"+owner
            core.execute([str(tools["createdb"]),"--no-password","--maintenance-db=postgres",db],
                         env=cluster.client_env("postgres"),cwd=cluster.root,phase="create-"+owner)
            env["MCR_CANONICAL_"+owner.upper()+"_URL"] = cluster.url(db)
        command = [sys.executable, str(Path(__file__).with_name("controller_uncertain.py")),
                   "--execute", "--case", args.case, "--checkpoint", str(args.checkpoint), "--repo", str(repo)]
        with subprocess.Popen(command, cwd=repo/"backend", env=env,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) as child:
            try:
                stdout, stderr = child.communicate(timeout=300)
            except subprocess.TimeoutExpired:
                # SIGTERM enters the controller's finally and kills/reaps its
                # exact two Popen children before PostgreSQL cleanup.
                child.terminate()
                try:
                    child.communicate(timeout=30)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.communicate(timeout=10)
                raise core.SmokeError("physical-root-restart", "owned controller deadline exceeded")
            report["controller"] = {"exit_code":child.returncode,
                "stdout":migration.redact(stdout,(cluster.password,cluster.token)),
                "stderr":migration.redact(stderr,(cluster.password,cluster.token))}
            if child.returncode != 0:
                raise core.SmokeError("physical-root-restart","exact owned process controller failed")
        observations = list(cluster.root.glob(f"{prefix}-*-observations.json"))
        if len(observations) != 1:
            raise core.SmokeError("physical-root-restart","one exact process observation required")
        observed = json.loads(observations[0].read_text())
        if observed.get("old_pid") == observed.get("new_pid") or not observed.get("old_reaped_after_sigkill"):
            raise core.SmokeError("physical-root-restart","distinct PID/kill/reap proof missing")
        if (observed.get("case") != args.case or observed.get("runtime_case_status") != "PASS"
                or observed.get("original_Profile_prepared") != (1 if args.case == "CR02" else 0)
                or observed.get("exact_Profile_terminal_receipt") != ("SETTLED" if args.case == "CR02" else "CANCELLED")
                or not observed.get("actual_Profile_Complete_proof")
                or not observed.get("immutable_nonce_digest_native_owner")):
            raise core.SmokeError("physical-root-restart", "exact uncertainty process evidence missing")
        if args.case == "CR05" and (not observed.get("actual_SDK_future_explicitly_dropped")
                or observed.get("Profile_ReadCurrent_handler_calls") != 0):
            raise core.SmokeError("physical-root-restart", "actual pre-dispatch cancellation proof missing")
        report["process_observations"] = observed
        handoffs = [p for p in cluster.root.glob("process-cr01-*.json") if not p.name.endswith("-observations.json")]
        if len(handoffs) != 1:
            raise core.SmokeError("physical-root-restart","one exact private handoff required")
        saved = json.loads(handoffs[0].read_text())
        report["saved_root_view_sha256"] = hashlib.sha256(bytes(saved["root_view"])).hexdigest()
        report["encrypted_intents_sha256"] = hashlib.sha256(json.dumps(saved["intents"],sort_keys=True,separators=(",",":")).encode()).hexdigest()
        report["prepared_before_sha256"] = hashlib.sha256(json.dumps(saved["prepared"],sort_keys=True,separators=(",",":")).encode()).hexdigest()
        report["migration_receipts"] = {owner:core.check_ledger(cluster,"mcr_canonical_"+owner,
            core.migration_manifest(repo/"backend",owner+"-service")) for owner in OWNERS}
        remaining = {owner:int(cluster.sql("mcr_canonical_"+owner,
            "select count(*) from pg_prepared_xacts where database=current_database();")) for owner in OWNERS}
        report["remaining_prepared"] = remaining
        if any(remaining.values()):
            raise core.SmokeError("cleanup","normal actual owner/child recovery must leave zero prepared work")
    finally:
        # Preserve only bounded fixed error diagnostics before removing private
        # logs/capabilities. Never copy arbitrary raw child output into reports.
        diagnostics = []
        if cluster.root.exists():
            for path in cluster.root.glob(f"{prefix}-*.log"):
                for line in path.read_text(errors="replace").splitlines():
                    if line.startswith("Error: "):
                        value = migration.redact(line[:600],tuple(env.values())+(cluster.password,cluster.token))
                        value = re.sub(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}","[identity]",value)
                        value = re.sub(r"(?:pp:|mcr-[a-z-]+:)[A-Za-z0-9:_-]+","[native-reference]",value)
                        diagnostics.append(value)
        if diagnostics:
            report["fixed_child_error_diagnostics"] = diagnostics[:8]
        cluster.cleanup()
        report["cleanup"]={"status":"PASS","owned_root_absent":not cluster.root.exists(),
            "kind":"OWNED_CLUSTER_TEARDOWN", "certifies_native_settlement":False}
        env.clear()

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--case", choices=("CR02", "CR05"), default="CR02")
    parser.add_argument("--checkpoint",type=Path)

    parser.add_argument("--pg-bin-dir",type=Path)
    parser.add_argument("--report",type=Path)
    parser.add_argument("--repo",type=Path,default=core.ROOT)
    args=parser.parse_args()
    if not args.execute:
        print("DRAFT ONLY: CR02/CR05 compile/runtime NOT_RUN. No process started.")
        return 0
    if args.report is None or args.checkpoint is None or args.pg_bin_dir is None:
        parser.error("--execute requires --checkpoint, --pg-bin-dir and --report")
    if not args.report.is_absolute() or args.report.exists() or not args.pg_bin_dir.is_absolute():
        parser.error("new absolute report and explicit absolute PG directory required")
    report={"status":"FAIL","case":args.case,"limitations":[
        "Actual Product/Delivery services and retained body EOF run in the owned Root child; external Gateway HTTP and actual main activation are separate gates.",
        "Actual Auth/Friend/Profile fixture RPC services share each Root child PID; independently surviving owner service processes are not claimed.",
        "Other CR cases remain separate; one physical restart does not complete the persistent owner-authority goal."]}
    try:
        run(args,report)
        report["status"]="PASS"
    except core.SmokeError as error:
        report["failure"]=error.detail
    except Exception as error:
        report["failure"]={"phase":"runner","diagnostic":type(error).__name__}
    finally:
        fd=os.open(args.report,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
        with os.fdopen(fd,"w") as stream:
            json.dump(report,stream,indent=2,sort_keys=True);stream.write("\n")
    print(json.dumps({"status":report["status"],"report":str(args.report)}))
    return 0 if report["status"]=="PASS" else 1

if __name__=="__main__":
    raise SystemExit(main())
