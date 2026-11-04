#!/usr/bin/env python3
"""Run the frozen canonical consumer against a newly owned socket-only PG16."""
from __future__ import annotations
import argparse
import hashlib
import json
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
from migration_smoke import (Cluster, PG_TOOLS, SmokeError, check_ledger,
                             executable, execute, migration_manifest)
from auth_invalidation_smoke import frozen_binary
from invalidation_clients_smoke import pki

OWNERS = ("auth", "friend", "concert")
# Exact names are required: a filtered or wholly ignored inventory is not proof.
CASES = ("native::both_owners_exclude_writers_through_actual_scope_settlement",
         "native::full_page_and_callback_drain_obey_original_scope",
         "native::registered_actual_transaction_quarantines_until_native_abort",
         "native::cancelled_unexposed_acquisition_recovers_from_actual_root_proof",
         "native::subject_denial_never_masks_whole_scope_failure")

def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()

def source_manifest() -> dict[str, str]:
    paths = {ROOT / "backend/Cargo.toml", ROOT / "backend/Cargo.lock", Path(__file__).resolve()}
    for prefix in ("backend/crates", "backend/proto", "backend/services/auth-service",
                   "backend/services/friend-service", "backend/services/concert-service",
                   "backend/services/profile-service", "contracts"):
        paths.update(p for p in (ROOT / prefix).rglob("*")
                     if p.is_file() and p.suffix in {".rs", ".sql", ".toml", ".proto", ".py", ".json"})
    for config in (".cargo/config.toml", "backend/.cargo/config.toml", "rust-toolchain.toml", "backend/rust-toolchain.toml"):
        if (ROOT / config).is_file():
            paths.add(ROOT / config)
    for name in ("migration_smoke.py", "auth_invalidation_smoke.py", "invalidation_clients_smoke.py"):
        paths.add(ROOT / "backend/scripts/mcr" / name)
    return {str(p.relative_to(ROOT)): sha(p) for p in sorted(paths)}

def qualify(checkpoint: dict, pg: Path, report: dict) -> None:
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if checkpoint["source_commit"] != commit or checkpoint["source_sha256"] != source_manifest():
        raise SmokeError("preflight", "source checkpoint changed")
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip():
        raise SmokeError("preflight", "clean committed fixture source required")
    binary = frozen_binary(Path(checkpoint["binary"]), checkpoint["binary_sha256"])
    if shutil.disk_usage(ROOT).free < 8 * 1024**3:
        raise SmokeError("preflight", "8 GiB free required")
    tools = {name: executable(pg / name) for name in PG_TOOLS}
    cluster = Cluster(tools)
    report.update(source_commit=commit, source_sha256=checkpoint["source_sha256"],
                  binary_sha256=checkpoint["binary_sha256"], owned_root=str(cluster.root), phases={})
    try:
        cluster.init()
        with (cluster.data / "postgresql.conf").open("a") as stream:
            # Holds both source-owner guards plus full-page local children.
            stream.write("\nmax_prepared_transactions=256\n")
        cluster.start()
        pki(cluster)
        version = cluster.sql("postgres", "show server_version_num;")
        if int(version) // 10000 != 16:
            raise SmokeError("preflight", "PG16 required")
        report["postgres_version"] = cluster.sql("postgres", "show server_version;")
        env = {**cluster.env,
               "CONCERT_SERVICE_CREDENTIAL": "concert-fixture",
               "MCR_CANONICAL_ROOT": str(cluster.root),
               "MCR_CANONICAL_ROOT_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
               "MCR_CANONICAL_ROOT_TOKEN": cluster.token,
               "MCR_CANONICAL_SYSTEM": cluster.sql("postgres", "select system_identifier from pg_control_system();")}
        for owner in OWNERS:
            db = f"mcr_canonical_{owner}"
            execute([str(tools["createdb"]), "--no-password", "--maintenance-db=postgres", db],
                    env=cluster.client_env("postgres"), phase=f"create-{owner}", cwd=cluster.root)
            env[f"MCR_CANONICAL_{owner.upper()}_URL"] = cluster.url(db)
        for case in CASES:
            result = execute([str(binary), "--ignored", "--exact", case,
                              "--test-threads=1", "--nocapture"],
                             env=env, phase=case, cwd=ROOT / "backend", timeout=60)
            if "test result: ok. 1 passed; 0 failed;" not in result.stdout:
                raise SmokeError("inventory", "expected one exact native test")
            report["phases"][case] = {"status": "PASS", "exit_code": result.returncode,
                                       "stdout": result.stdout}
        report["migration_receipts"] = {}
        for owner in OWNERS:
            db = f"mcr_canonical_{owner}"
            report["migration_receipts"][owner] = check_ledger(
                cluster, db, migration_manifest(ROOT / "backend", f"{owner}-service"))
            if cluster.sql(db, "select count(*) from pg_prepared_xacts where database=current_database();") != "0":
                raise SmokeError("cleanup", "prepared transactions remain")
    finally:
        cluster.cleanup()
        report["cleanup"] = {"status": "PASS", "owned_root_absent": not cluster.root.exists()}

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--pg-bin-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if not args.report.is_absolute() or args.report.exists() or not args.pg_bin_dir.is_absolute():
        parser.error("new absolute report and explicit absolute PG directory required")
    report = {"status": "FAIL", "limitations": [
        "Core owner/consumer qualification only; root HTTP boundary has its separate production harness.",
        "Full Profile/Post/Media and process/PostgreSQL restart qualification remain separate obligations."]}
    try:
        qualify(json.loads(args.checkpoint.read_text()), args.pg_bin_dir, report)
        report["status"] = "PASS"
    except SmokeError as error:
        report["failure"] = error.detail
    except Exception as error:
        report["failure"] = {"phase": "runner", "diagnostic": type(error).__name__, "detail": str(error)}
    finally:
        args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"status": report["status"], "report": str(args.report)}))
    return 0 if report["status"] == "PASS" else 1

if __name__ == "__main__":
    sys.exit(main())
