#!/usr/bin/env python3
"""CU01 frozen component tests in a newly owned socket-only PostgreSQL 16."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
from migration_smoke import (Cluster, PG_TOOLS, SmokeError, check_ledger,
                             executable, execute, migration_manifest)
from auth_invalidation_smoke import frozen_binary
from invalidation_clients_smoke import pki

CASE = "native::actual_producer_witnesses_dispatch_without_unknown_or_error_fallback"
DATABASES = {"auth": "mcr_canonical_auth", "friend": "mcr_canonical_friend", "concert": "mcr_concert_cu01"}

def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()

def sources() -> dict[str, str]:
    paths = {ROOT / "backend/Cargo.toml", ROOT / "backend/Cargo.lock", Path(__file__).resolve()}
    for prefix in ("backend/crates", "backend/proto", "backend/services"):
        paths.update(p for p in (ROOT / prefix).rglob("*")
                     if p.is_file() and p.suffix in {".rs", ".sql", ".toml", ".proto", ".py"})
    for config in (".cargo/config.toml", "backend/.cargo/config.toml", "rust-toolchain.toml", "backend/rust-toolchain.toml"):
        if (ROOT / config).is_file(): paths.add(ROOT / config)
    for name in ("migration_smoke.py", "auth_invalidation_smoke.py", "invalidation_clients_smoke.py"):
        paths.add(ROOT / "backend/scripts/mcr" / name)
    paths.add(ROOT / "contracts/fixtures/assets/selected-static/avatar-manifest.json")
    return {str(p.relative_to(ROOT)): sha(p) for p in sorted(paths)}

def allocated(path: Path) -> int:
    # The controller owns the still-live fixture throughout this loop. A bounded
    # retry handles ephemeral files; a persistent observer error gets no PASS.
    for _ in range(3):
        result = subprocess.run(["du", "-sk", str(path)], capture_output=True, text=True)
        if result.returncode == 0: return int(result.stdout.split()[0]) * 1024
        time.sleep(.03)
    raise SmokeError("capacity", "owned allocation observation failed")

def run_case(binary: Path, argv: list[str], env: dict, cluster: Cluster,
             args, report: dict) -> str:
    log = args.report.parent / ("cu01-" + ("native" if "--ignored" in argv else "routes") + ".log")
    if log.exists(): raise SmokeError("preflight", "new owned output required")
    p = None
    try:
        with log.open("x+b") as output:
            os.chmod(log, 0o600)
            p = subprocess.Popen([str(binary), *argv], env=env, cwd=ROOT / "backend",
                                 stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
            started = time.monotonic()
            while p.poll() is None:
                growth = allocated(cluster.root) + allocated(args.report.parent) + args.fixed_target_growth
                report["capacity"]["max_sampled_whole_additional"] = max(report["capacity"]["max_sampled_whole_additional"], growth)
                if shutil.disk_usage(ROOT).free < 8 * 1024**3 or growth > args.hard_whole_bytes or time.monotonic()-started > 90:
                    raise SmokeError("capacity", "native floor/growth/time boundary reached")
                time.sleep(.1)
            code = p.wait()
            output.seek(0); text = output.read().decode("utf-8", "replace")
            report.setdefault("commands", []).append({"argv": ["<frozen-native-resolver-union>", *argv],
                "exit_code": code, "elapsed_seconds": round(time.monotonic()-started, 3), "log_sha256": sha(log)})
            if code != 0: raise SmokeError("native-test", text[-6000:], code)
            return text
    finally:
        if p is not None and p.poll() is None:
            os.killpg(p.pid, signal.SIGTERM)
            try: p.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(p.pid, signal.SIGKILL); p.wait(timeout=3)
            report["forced_test_stop"] = True

def qualify(args, report: dict) -> None:
    checkpoint = json.loads(args.checkpoint.read_bytes())
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if checkpoint["source_commit"] != commit or checkpoint["source_sha256"] != sources():
        raise SmokeError("preflight", "source checkpoint changed")
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip():
        raise SmokeError("preflight", "clean committed fixture source required")
    if checkpoint["compile_exit"] != 0 or checkpoint["artifact"]["target"]["name"] != "native_resolver_union" or checkpoint["artifact"]["profile"]["test"] is not True:
        raise SmokeError("preflight", "exact successful Cargo test receipt required")
    binary = frozen_binary(Path(checkpoint["binary"]), checkpoint["binary_sha256"])
    tools = {name: executable(args.pg_bin_dir / name) for name in PG_TOOLS}
    cluster = Cluster(tools)
    report.update(source_commit=commit, source_sha256=checkpoint["source_sha256"],
        binary_sha256=checkpoint["binary_sha256"], owned_root=str(cluster.root),
        capacity={"hard_whole_bytes": args.hard_whole_bytes, "fixed_target_growth": args.fixed_target_growth,
                  "max_sampled_whole_additional": 0, "sampling_limit": "Sampled allocation; no peak between samples inferred"})
    try:
        cluster.init()
        with (cluster.data / "postgresql.conf").open("a") as stream:
            stream.write("\nmax_prepared_transactions=128\n")
        cluster.start()
        if int(cluster.sql("postgres", "show server_version_num;")) // 10000 != 16:
            raise SmokeError("preflight", "PG16 required")
        report["postgres_version"] = cluster.sql("postgres", "show server_version;")
        pki(cluster)
        env = {**cluster.env, "CONCERT_SERVICE_CREDENTIAL": "concert-fixture",
            "MCR_CANONICAL_ROOT": str(cluster.root), "MCR_CANONICAL_ROOT_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
            "MCR_CANONICAL_ROOT_TOKEN": cluster.token,
            "MCR_CANONICAL_SYSTEM": cluster.sql("postgres", "select system_identifier from pg_control_system();")}
        for owner, db in DATABASES.items():
            execute([str(tools["createdb"]), "--no-password", "--maintenance-db=postgres", db],
                    env=cluster.client_env("postgres"), phase=f"create-{owner}", cwd=cluster.root)
            env[f"MCR_CANONICAL_{owner.upper()}_URL"] = cluster.url(db)
        output = run_case(binary, ["--test-threads=1", "--nocapture"], env, cluster, args, report)
        if "test result: ok. 3 passed; 0 failed; 1 ignored;" not in output:
            raise SmokeError("inventory", "expected three route/constructor checks and one ignored native case")
        report["route_constructor_tests"] = "PASS"
        output = run_case(binary, ["--ignored", "--exact", CASE, "--test-threads=1", "--nocapture"], env, cluster, args, report)
        if "test result: ok. 1 passed; 0 failed;" not in output or "CU01: four actual producer-specific commit witnesses" not in output:
            raise SmokeError("inventory", "exact actual native test did not execute")
        report["actual_native_case"] = "PASS"
        report["migration_receipts"] = {owner: check_ledger(cluster, db, migration_manifest(ROOT / "backend", f"{owner}-service")) for owner, db in DATABASES.items()}
        report["prepared_before_owned_shutdown"] = int(cluster.sql("postgres", "select count(*) from pg_prepared_xacts;"))
        if report["prepared_before_owned_shutdown"] != 4:
            raise SmokeError("negative-quarantine", "expected exact four retained owner fences")
        if sources() != checkpoint["source_sha256"] or sha(binary) != checkpoint["binary_sha256"]:
            raise SmokeError("provenance", "source or frozen binary drifted")
        report["status"] = "PASS"
    finally:
        cluster.cleanup()
        report["cleanup"] = {"status": "PASS", "owned_root_absent": not cluster.root.exists(),
            "certifies_native_settlement": False, "boundary": "Expected negative fences removed only with whole disposable cluster"}

def main() -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ("checkpoint", "pg-bin-dir", "report"): parser.add_argument("--"+name, type=Path, required=True)
    parser.add_argument("--hard-whole-bytes",type=int,required=True)
    parser.add_argument("--fixed-target-growth",type=int,required=True)
    args=parser.parse_args()
    if any(not p.is_absolute() for p in (args.checkpoint,args.pg_bin_dir,args.report)) or args.report.exists() or args.hard_whole_bytes<=0 or args.fixed_target_growth<0:
        parser.error("new absolute report/explicit paths and positive named resource bounds required")
    report={"status":"FAIL","task":"CU01","limits":["Native resolver component qualification only; activation/global factory routing and full BE15/BE44 acceptance are Root-owned.","No browser/provider qualification, fabricated witness, prepared rollback or status-based COMMIT promotion."]}
    try: qualify(args,report)
    except SmokeError as error: report["failure"]=error.detail
    except Exception as error: report["failure"]={"type":type(error).__name__,"message":str(error)}
    args.report.write_text(json.dumps(report,indent=2,sort_keys=True)+"\n");args.report.chmod(0o600)
    print(json.dumps({"status":report["status"],"report":str(args.report),"failure":report.get("failure")}))
    return 0 if report["status"]=="PASS" else 1

if __name__=="__main__": raise SystemExit(main())
