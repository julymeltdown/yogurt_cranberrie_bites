#!/usr/bin/env python3
"""Run copied BE42 tests in a new, inode/marker-owned PostgreSQL16 cluster."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
from urllib.parse import quote

from migration_smoke import (BACKEND, Cluster, PG_TOOLS, SmokeError, check_ledger,
                             executable, execute, migration_manifest)


def binary(path: Path, digest: str) -> Path:
    if not path.is_absolute() or path.is_symlink():
        raise SmokeError("preflight", "test binary must be an explicit copied absolute regular file")
    resolved = executable(path)
    if any(part in {"target", "mcr-build"} for part in resolved.parts) or hashlib.sha256(resolved.read_bytes()).hexdigest() != digest:
        raise SmokeError("preflight", "refusing target artifact or mismatched binary digest")
    return resolved


def run_tests(path: Path, env: dict, phase: str, report: dict, exact: str | None = None) -> None:
    args = [str(path), "--test-threads=1"]
    count = 5
    if exact:
        args += ["--ignored", "--exact", exact]
        count = 1
    result = execute(args, env=env, phase=phase, timeout=60, cwd=BACKEND)
    if f"test result: ok. {count} passed; 0 failed;" not in result.stdout:
        raise SmokeError(phase, "test inventory/result mismatch")
    report[phase] = {"status": "PASS", "exit_code": result.returncode, "passed": count}


def qualify(path: Path, pg: Path, report: dict, legacy: Path | None = None) -> None:
    tools = {name: executable(pg / name) for name in PG_TOOLS}
    cluster = Cluster(tools)
    root = cluster.root
    report["owned_cluster_root"] = str(root)
    report["cleanup"] = {"status": "NOT_RUN"}
    try:
        cluster.init()
        cluster.start()
        report["postgres_version"] = cluster.sql("postgres", "show server_version;")
        if int(cluster.sql("postgres", "show server_version_num;")) // 10000 != 16:
            raise SmokeError("preflight", "PostgreSQL16 required")
        execute([str(tools["createdb"]), "--no-password", "--maintenance-db=postgres",
                 "mcr_account_deletion"], env=cluster.client_env("postgres"), phase="createdb", cwd=root)
        lineage = cluster.sql("postgres", "select system_identifier from pg_control_system();")
        env = {**cluster.env, "MCR_DELETION_TEST_DATABASE_URL": cluster.url("mcr_account_deletion"),
               "MCR_DELETION_TEST_SYSTEM_IDENTIFIER": lineage}
        run_tests(path, cluster.env, "pure", report)
        run_tests(path, env, "native", report,
                  "native::canonical_upgrade_retry_recovery_and_partial_failure")
        expected = migration_manifest(BACKEND, "auth-service")
        report["migration_receipts"] = check_ledger(cluster, "mcr_account_deletion", expected)
        report["migration_manifest"] = expected
        before = cluster.assert_postmaster()
        cluster.stop()
        cluster.start()
        after = cluster.assert_postmaster()
        if before == after:
            raise SmokeError("restart", "postmaster did not change")
        report["restart"] = {"status": "PASS", "different_postmaster": True,
                             "before_pid": before, "after_pid": after}
        run_tests(path, env, "after_restart", report,
                  "native::restart_keeps_required_failures_and_success_receipts")
        check_ledger(cluster, "mcr_account_deletion", expected)
        if legacy is not None:
            # The inherited fixture replaces the URL's final database segment.
            # Encode our owned Unix socket in the authority so that replacement
            # cannot lose a query-string host override or fall back to TCP.
            authority_url = (f"postgresql://mcr_owner:{cluster.password}@"
                             f"{quote(str(cluster.socket), safe='')}:5432/postgres")
            legacy_env = {**cluster.env, "AUTH_RUNTIME_TEST_DATABASE_URL": authority_url,
                          "AUTH_RUNTIME_TEST_INITDB_SYSTEM_IDENTIFIER": lineage}
            run_tests(legacy, legacy_env, "inherited_durable", report)
    finally:
        cluster.cleanup()
        report["cleanup"] = {"status": "PASS" if not root.exists() else "FAIL", "owned_root_absent": not root.exists()}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-binary", required=True, type=Path)
    parser.add_argument("--sha256", required=True)
    parser.add_argument("--pg-bin-dir", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--inherited-test-binary", type=Path)
    parser.add_argument("--inherited-sha256")
    args = parser.parse_args()
    report = {"status": "FAIL", "binary_sha256": args.sha256,
              "limitations": ["Typed owner fixtures do not certify real Concert owner integration or reconnect/reveal fencing.",
                              "No HTTP/Gateway, browser, deployment or whole-task acceptance claimed."]}
    try:
        path = binary(args.test_binary, args.sha256)
        if bool(args.inherited_test_binary) != bool(args.inherited_sha256):
            raise SmokeError("preflight", "inherited binary and digest must be supplied together")
        legacy = binary(args.inherited_test_binary, args.inherited_sha256) if args.inherited_test_binary else None
        if legacy is not None:
            report["inherited_binary_sha256"] = args.inherited_sha256
        if not args.pg_bin_dir.is_absolute():
            raise SmokeError("preflight", "PG binaries must use an explicit absolute path")
        report["source_commit"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=BACKEND, text=True).strip()
        report["source_dirty"] = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=BACKEND, text=True).strip())
        report["source_sha256"] = {
            str(p.relative_to(BACKEND.parent)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in [Path(__file__).resolve(), BACKEND / "services/auth-service/tests/concert_deletion.rs",
                      BACKEND / "services/auth-service/tests/concert_deletion_support/native.rs",
                      BACKEND / "services/auth-service/src/concert_deletion/mod.rs",
                      BACKEND / "crates/auth-application/src/concert_deletion/mod.rs",
                      BACKEND / "crates/auth-domain/src/user_deletion.rs",
                      BACKEND / "crates/auth-application/src/dto/deletion_dto.rs",
                      BACKEND / "services/auth-service/src/adapters/out/grpc/user_deletion_dispatcher.rs",
                      BACKEND / "services/auth-service/src/adapters/out/persistence/user_deletion_saga_repository.rs"]
        }
        qualify(path, args.pg_bin_dir, report, legacy)
        report["status"] = "PASS"
    except SmokeError as error:
        report["failure"] = error.detail
    except Exception as error:
        # Never serialize environment, URLs, exception causes or dependency text.
        report["failure"] = {"phase": "runner", "diagnostic": type(error).__name__}
    args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"status": report["status"], "report": str(args.report)}))
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
