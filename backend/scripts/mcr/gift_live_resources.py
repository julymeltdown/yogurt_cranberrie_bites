#!/usr/bin/env python3
"""Run selected Gift native tests in a marker-owned private socket PG cluster.

Invoke only after an existing physical target is allocated and its exact source
snapshot is frozen. This runner never builds or allocates a Cargo target.
Private-cluster lifecycle is reused from migration_smoke.py at 1cbb1823.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import migration_smoke as migration

ROOT = Path(__file__).resolve().parents[3]
DATABASE = "mcr_gift_live"

def require(ok, reason):
    if not ok:
        raise RuntimeError(reason)

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def frozen_binary(path):
    require(path.is_absolute(), "explicit absolute frozen binary required")
    return migration.executable(path)

def pins():
    files = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", "backend"],
        cwd=ROOT,
    ).decode().split("\0")
    selected = {ROOT / n for n in files if n and Path(n).suffix in {".rs", ".toml", ".lock", ".sql", ".proto", ".py"}}
    selected.add(ROOT / "contracts/generated/mcr-domain.openapi.json")
    return {str(p.relative_to(ROOT)): sha(p) for p in sorted(selected)}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository-test", type=Path, help="optional frozen four-test Gift resource integration binary")
    parser.add_argument("--queries-test", type=Path, help="frozen Concert lib test binary; only Gift native query/correlation tests run")
    parser.add_argument("--private-test", type=Path, help="same frozen Concert lib binary; native prepared exclusion/recovery only, no canonical or Root qualification")
    parser.add_argument("--settlement-test", type=Path, help="same frozen lib; exact native/receipt witness SQL with synthetic ledger metadata")
    parser.add_argument("--pg-bin", type=Path, default=Path("/opt/homebrew/opt/postgresql@16/bin"))
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if not any((args.repository_test, args.queries_test, args.private_test, args.settlement_test)):
        parser.error("select at least one frozen native binary")
    require(args.report.is_absolute(), "absolute evidence path required")
    descriptor = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    report = {"scope": "BE-GIFTS-LIVE selected local Concert native checks only", "result": "NOT_RUN",
              "canonical_auth_friend": "NOT_RUN", "media_link": "NOT_RUN", "public_profile": "NOT_RUN",
              "http_routes": "NOT_RUN", "two_user_flow": "NOT_RUN",
              "checks": {"resources": "NOT_RUN", "queries": "NOT_RUN", "correlation": "NOT_RUN", "private_native": "NOT_RUN", "settlement_native": "NOT_RUN"},
              "binary_provenance": "Execution hashes below; compilation source/pins must be supplied by the producing build handoff separately. Harness checkout pins are not a binary build claim."}
    cluster = None
    try:
        binary = frozen_binary(args.repository_test) if args.repository_test is not None else None
        queries = frozen_binary(args.queries_test) if args.queries_test is not None else None
        private = frozen_binary(args.private_test) if args.private_test is not None else None
        settlement = frozen_binary(args.settlement_test) if args.settlement_test is not None else None
        lib_hashes = {sha(p) for p in (queries, private, settlement) if p is not None}
        require(len(lib_hashes) <= 1, "all selected library modes require the same exact frozen build")
        if settlement is not None:
            report["settlement_binary_sha256"] = sha(settlement)
        report["source_revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        report["source_sha256"] = pins()
        report["source_pin_scope"] = "Harness checkout and migrations used during native execution; compiled binary provenance is separate"
        if binary is not None:
            report["binary_sha256"] = sha(binary)
        if queries is not None:
            report["query_binary_sha256"] = sha(queries)
        if private is not None:
            report["private_binary_sha256"] = sha(private)
            if queries is not None:
                require(sha(queries) == sha(private), "query/private binaries must be the same frozen Concert lib build")
        cluster = migration.Cluster({n: migration.executable(args.pg_bin / n) for n in migration.PG_TOOLS})
        report["owned_root"] = str(cluster.root)
        cluster.init()
        cluster.start()
        if private is not None:
            # Only this newly created marker/inode-owned cluster is configured.
            cluster.sql("postgres", "alter system set max_prepared_transactions='256';")
            cluster.stop()
            cluster.start()
            require(cluster.sql("postgres", "show max_prepared_transactions;") == "256", "private prepared capacity unavailable")
        report["postgres_version"] = cluster.sql("postgres", "show server_version;")
        cluster.sql("postgres", f"create database {DATABASE} template template0;")
        for path in sorted((ROOT / "backend/services/concert-service/migrations").glob("*.sql")):
            cluster.sql(DATABASE, path.read_text())
        env = {**cluster.env, "GIFT_LIVE_TEST_DATABASE_URL": cluster.url(DATABASE),
               "GIFT_LIVE_TEST_OWNER_ROOT": str(cluster.root), "GIFT_LIVE_TEST_OWNER_TOKEN": cluster.token,
               "GIFT_LIVE_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}"}
        if binary is not None:
            command = [str(binary), "--ignored", "--test-threads=1"]
            result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True, timeout=60)
            report["native_run"] = {"command": command, "exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr}
            require(result.returncode == 0, "native Gift resource tests failed")
            match = re.search(r"test result: ok\. (\d+) passed", result.stdout)
            require(match is not None and int(match[1]) == 4, "expected all four exact Gift resource tests")
            report["checks"]["resources"] = "PASS"
        if queries is not None:
            for kind, namespace in [("query", "queries"), ("target", "correlation")]:
                command = [str(queries), "--ignored", "--test-threads=1", f"gift_live::{namespace}::native_tests"]
                result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True, timeout=60)
                report[f"{kind}_run"] = {"command": command, "exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr,
                                       "authority": "synthetic test policy only; canonical owner acceptance NOT_RUN"}
                require(result.returncode == 0, f"native Gift {kind} tests failed")
                match = re.search(r"test result: ok\. (\d+) passed", result.stdout)
                expected = 4 if kind == "query" else 3
                require(match is not None and int(match[1]) == expected, f"expected all {expected} Gift {kind} tests")
                report["checks"]["queries" if kind == "query" else "correlation"] = "PASS"
            require(report["query_binary_sha256"] == sha(queries), "query binary changed during native validation")
        if private is not None:
            namespace = "gift_live::private::native_tests::"
            def private_run(name, phase=None):
                command = [str(private), "--ignored", "--test-threads=1", "--exact", namespace + name]
                run_env = {**env, **({"GIFT_PRIVATE_RESTART_PHASE": phase} if phase else {})}
                result = subprocess.run(command, cwd=ROOT, env=run_env, capture_output=True, text=True, timeout=30)
                row = {"command": command, "exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr,
                       "authority": "synthetic correlation metadata only; native SQL exclusion/recovery, canonical/Root intent/before-byte NOT_RUN"}
                report.setdefault("private_runs", []).append(row)
                require(result.returncode == 0, "native Gift private exclusion test failed")
                require(re.search(r"test result: ok\. 1 passed", result.stdout) is not None, "expected exactly one named Gift private test")
            for name in ["drop_keeps_exclusion_until_exact_idempotent_abort", "expiry_denies_content_without_releasing_exclusion",
                         "mismatched_identity_cannot_rollback_and_commit_is_not_abort", "missing_gid_requires_real_full_xid_abort",
                         "sql_budget_never_widens_existing_timeouts"]:
                private_run(name)
            private_run("prepared_restart_recovery", "prepare")
            before = cluster.assert_postmaster()
            cluster.stop()
            cluster.start()
            require(before != cluster.assert_postmaster(), "owned postmaster did not restart")
            report["private_restart"] = "PASS"
            private_run("prepared_restart_recovery", "recover")
            require(cluster.sql(DATABASE, "select count(*) from pg_prepared_xacts where database=current_database();") == "0", "prepared Gift children remain")
            require(report["private_binary_sha256"] == sha(private), "private binary changed during validation")
            report["checks"]["private_native"] = "PASS"
        if settlement is not None:
            def settlement_run(name):
                command = [str(settlement), "--ignored", "--test-threads=1", "--exact", "gift_live::settlement::native_tests::" + name]
                result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True, timeout=45)
                report.setdefault("settlement_runs", []).append({"command": command, "exit_code": result.returncode,
                    "stdout": result.stdout, "stderr": result.stderr,
                    "authority": "synthetic ledger metadata only; canonical business/HTTP NOT_RUN"})
                require(result.returncode == 0 and re.search(r"test result: ok\. 1 passed", result.stdout) is not None,
                        "expected exactly one successful named Gift settlement case")
            for name in ["exact_commit_requires_witness_and_finished_root_and_rejects_forgeries",
                         "committed_without_witness_stays_unknown_and_rollback_is_not_commit",
                         "foreign_transaction_cannot_insert_witness_or_mutate_immutable_evidence",
                         "receipt_snapshot_is_same_transaction_parent_bound_and_survives_retirement"]:
                settlement_run(name)
            settlement_run("restart_preserves_exact_committed_snapshot_witness")
            before = cluster.assert_postmaster()
            cluster.stop()
            cluster.start()
            require(before != cluster.assert_postmaster(), "owned settlement postmaster did not restart")
            settlement_run("restart_preserves_exact_committed_snapshot_witness")
            require(report["settlement_binary_sha256"] == sha(settlement), "settlement binary changed")
            report["settlement_restart"] = "PASS"
            report["checks"]["settlement_native"] = "PASS"
        require(report["source_sha256"] == pins(), "source changed during native validation")
        if binary is not None:
            require(report["binary_sha256"] == sha(binary), "binary changed during native validation")
        report["result"] = "PASS"
    except BaseException as error:
        report["result"] = "FAIL"
        report["failure"] = type(error).__name__ + ": " + (str(error) if isinstance(error, RuntimeError) else "native fixture failure")
    finally:
        try:
            if cluster is not None:
                root = cluster.root
                cluster.cleanup()
                require(not root.exists(), "owned fixture root remains")
            report["cleanup"] = "PASS"
        except BaseException:
            report["cleanup"] = "FAIL"
            report["result"] = "FAIL"
        with os.fdopen(descriptor, "w") as stream:
            json.dump(report, stream, indent=2)
            stream.write("\n")
    print(json.dumps({key: report.get(key) for key in ("scope", "result", "cleanup", "failure")}))
    return 0 if report["result"] == "PASS" and report["cleanup"] == "PASS" else 1

if __name__ == "__main__":
    raise SystemExit(main())
