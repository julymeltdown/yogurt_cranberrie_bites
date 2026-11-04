#!/usr/bin/env python3
"""Run only Gift local-resource tests in a marker-owned private socket PG cluster.

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
    parser.add_argument("--repository-test", type=Path, required=True)
    parser.add_argument("--queries-test", type=Path, help="frozen Concert lib test binary; only Gift native query/correlation tests run")
    parser.add_argument("--private-test", type=Path, help="same frozen Concert lib binary; native prepared exclusion/recovery only, no canonical or Root qualification")
    parser.add_argument("--pg-bin", type=Path, default=Path("/opt/homebrew/opt/postgresql@16/bin"))
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    require(args.report.is_absolute(), "absolute evidence path required")
    descriptor = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    report = {"scope": "BE-GIFTS-LIVE local Concert resources only", "result": "NOT_RUN",
              "canonical_auth_friend": "NOT_RUN", "media_link": "NOT_RUN", "public_profile": "NOT_RUN",
              "http_routes": "NOT_RUN", "two_user_flow": "NOT_RUN"}
    cluster = None
    try:
        binary = migration.executable(args.repository_test)
        require(binary.is_absolute(), "explicit native binary required")
        report["source_revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        report["source_sha256"] = pins()
        report["binary_sha256"] = sha(binary)
        private = None
        if args.private_test is not None:
            private = migration.executable(args.private_test)
            report["private_binary_sha256"] = sha(private)
            if args.queries_test is not None:
                require(sha(migration.executable(args.queries_test)) == sha(private), "query/private binaries must be the same frozen Concert lib build")
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
        command = [str(binary), "--ignored", "--test-threads=1"]
        result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True, timeout=60)
        report["native_run"] = {"command": command, "exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr}
        require(result.returncode == 0, "native Gift resource tests failed")
        match = re.search(r"test result: ok\. (\d+) passed", result.stdout)
        require(match is not None and int(match[1]) == 4, "expected all four exact Gift resource tests")
        if args.queries_test is not None:
            queries = migration.executable(args.queries_test)
            report["query_binary_sha256"] = sha(queries)
            for kind, namespace in [("query", "queries"), ("target", "correlation")]:
                command = [str(queries), "--ignored", "--test-threads=1", f"gift_live::{namespace}::native_tests"]
                result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True, timeout=60)
                report[f"{kind}_run"] = {"command": command, "exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr,
                                       "authority": "synthetic test policy only; canonical owner acceptance NOT_RUN"}
                require(result.returncode == 0, f"native Gift {kind} tests failed")
                match = re.search(r"test result: ok\. (\d+) passed", result.stdout)
                require(match is not None and int(match[1]) == 3, f"expected all three exact Gift {kind} tests")
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
        require(report["source_sha256"] == pins(), "source changed during native validation")
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
