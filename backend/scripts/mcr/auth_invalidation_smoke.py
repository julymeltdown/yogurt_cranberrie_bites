#!/usr/bin/env python3
"""Qualify Auth invalidation only in a fresh marker/inode-owned PG16 cluster."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
from migration_smoke import BACKEND, Cluster, PG_TOOLS, SmokeError, check_ledger, executable, execute, migration_manifest


def frozen_binary(path: Path, sha256: str) -> Path:
    if not path.is_absolute() or path.is_symlink():
        raise SmokeError("preflight", "explicit absolute copied regular executable required")
    path = executable(path)
    if any(p in {"target", "mcr-build"} for p in path.parts) or hashlib.sha256(path.read_bytes()).hexdigest() != sha256:
        raise SmokeError("preflight", "refusing target artifact or mismatched digest")
    return path


def run_test(binary: Path, env: dict, report: dict, phase: str, exact: str | None = None) -> None:
    args = [str(binary), "--test-threads=1"]
    count = 2
    if exact:
        args += ["--ignored", "--exact", "native::" + exact]
        count = 1
    result = execute(args, env=env, phase=phase, timeout=60, cwd=BACKEND)
    if f"test result: ok. {count} passed; 0 failed;" not in result.stdout:
        raise SmokeError(phase, "exact test inventory did not execute")
    report[phase] = {"status": "PASS", "exit_code": result.returncode, "passed": count}


def qualify(binary: Path, pg: Path, report: dict) -> None:
    tools = {n: executable(pg / n) for n in PG_TOOLS}
    cluster = Cluster(tools)
    report["owned_root"] = str(cluster.root)
    try:
        cluster.init()
        cluster.start()
        report["postgres_version"] = cluster.sql("postgres", "show server_version;")
        if int(cluster.sql("postgres", "show server_version_num;")) // 10000 != 16:
            raise SmokeError("preflight", "PG16 required")
        execute([str(tools["createdb"]), "--no-password", "--maintenance-db=postgres", "mcr_auth_invalidation"],
                env=cluster.client_env("postgres"), phase="createdb", cwd=cluster.root)
        env = {**cluster.env, "MCR_INVALIDATION_DATABASE_URL": cluster.url("mcr_auth_invalidation"),
               "MCR_INVALIDATION_SYSTEM_IDENTIFIER": cluster.sql("postgres", "select system_identifier from pg_control_system();")}
        run_test(binary, cluster.env, report, "pure")
        run_test(binary, env, report, "mutators", "migration_and_real_mutators_capture_atomically")
        run_test(binary, env, report, "concurrency_replay", "commit_order_rollback_binding_and_bounded_replay")
        manifest = migration_manifest(BACKEND, "auth-service")
        report["migration_sha384"] = manifest
        report["migration_receipts"] = check_ledger(cluster, "mcr_auth_invalidation", manifest)
        report["committed_cursor_and_count"] = cluster.sql("mcr_auth_invalidation", "select committed_cursor,(select count(*) from auth_concert_invalidation_journal) from auth_concert_invalidation_head;")
        before = cluster.assert_postmaster()
        cluster.stop()
        cluster.start()
        after = cluster.assert_postmaster()
        if before == after:
            raise SmokeError("restart", "postmaster identity did not change")
        report["restart"] = {"status": "PASS", "before_pid": before, "after_pid": after}
        run_test(binary, env, report, "after_restart", "restart_replays_stable_identity_and_refuses_changed_checksum")
        check_ledger(cluster, "mcr_auth_invalidation", manifest)
    finally:
        cluster.cleanup()
        report["cleanup"] = {"status": "PASS", "owned_root_absent": not cluster.root.exists()}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-binary", type=Path, required=True)
    parser.add_argument("--sha256", required=True)
    parser.add_argument("--pg-bin-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    report = {"status": "FAIL", "binary_sha256": args.sha256,
              "limitations": ["Owner-local producer/replay only; no authenticated network transport or World consumer readiness.",
                              "Signals are hints requiring existing typed authority rereads; they grant or extend nothing."]}
    try:
        binary = frozen_binary(args.test_binary, args.sha256)
        if not args.pg_bin_dir.is_absolute():
            raise SmokeError("preflight", "explicit PG binary directory required")
        report["source_commit"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=BACKEND, text=True).strip()
        report["source_dirty"] = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=BACKEND, text=True).strip())
        report["source_sha256"] = {str(p.relative_to(BACKEND.parent)): hashlib.sha256(p.read_bytes()).hexdigest()
                                   for p in [Path(__file__).resolve(), BACKEND / "services/auth-service/src/concert_invalidation/mod.rs",
                                             BACKEND / "services/auth-service/tests/concert_invalidation.rs",
                                             BACKEND / "services/auth-service/tests/concert_invalidation_support/native.rs"]}
        qualify(binary, args.pg_bin_dir, report)
        report["status"] = "PASS"
    except SmokeError as error:
        report["failure"] = error.detail
    except Exception as error:
        report["failure"] = {"phase": "runner", "diagnostic": type(error).__name__}
    args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"status": report["status"], "report": str(args.report)}))
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
