#!/usr/bin/env python3
"""Generated guarded Auth RPC over fresh owned PG16, plus retained owner regressions."""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import migration_smoke as migration
import auth_identity_smoke as owner_smoke
from concert_core_smoke import command, digest, require, ROOT

DATABASE = "mcr_auth_admission_rpc"


def native_names(test, env, root, secrets, report):
    result = command(report, "list-rpc-tests", [test, "--list"], env, root, secrets=secrets)
    names = re.findall(r"^(native_[a-z0-9_]+): test$", result.stdout.decode(), re.MULTILINE)
    require(bool(names) and len(names) == len(set(names)), "native-rpc-tests-required")
    return names


def require_test_summary(raw, count):
    summaries = re.findall(rb"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;", raw, re.MULTILINE)
    require(summaries == [(str(count).encode(), b"0", b"0")], "native-rpc-tests-not-skipped")


def exercise(cluster, binary, test, report):
    cluster.init()
    cluster.start()
    report["postgres_version"] = cluster.sql("postgres", "show server_version;")
    require(report["postgres_version"].startswith("16."), "postgres16-required")
    cluster.sql("postgres", f"create database {DATABASE} template template0;")
    url = cluster.url(DATABASE)
    secrets = (cluster.password, url)
    env = {**cluster.env, "AUTH_DATABASE_URL": url}

    def cli(label, *args, success=True, override=None):
        return command(report, label, [binary, *args], override or env, cluster.root,
                       success=success, secrets=secrets)

    cli("missing-explicit-database", "--migrate", success=False, override=cluster.env)
    cli("verify-empty-refused", "--verify-migrations", success=False)
    cli("migrate", "--migrate")
    cli("verify", "--verify-migrations")
    manifest = migration.migration_manifest(ROOT / "backend", "auth-service")
    require(bool(manifest) and len(manifest) == max(manifest) and set(manifest) == set(range(1, max(manifest) + 1)), "exact-auth-migration-set")
    report["migration_sha384"] = manifest
    report["migration_ledger"] = migration.check_ledger(cluster, DATABASE, manifest)
    cli("migrate-reapply", "--migrate")
    cli("verify-reapply", "--verify-migrations")
    require(migration.check_ledger(cluster, DATABASE, manifest) == len(manifest), "stable-ledger")
    names = native_names(test, cluster.env, cluster.root, secrets, report)
    report["native_test_names"] = names
    command(report, "missing-owner-refused", [test, "--exact", names[0]], cluster.env,
            cluster.root, success=False, secrets=secrets)
    test_env = {**env, "AUTH_ADMISSION_RPC_TEST_DATABASE_URL": url,
                "AUTH_ADMISSION_RPC_TEST_OWNER_ROOT": str(cluster.root),
                "AUTH_ADMISSION_RPC_TEST_OWNER_TOKEN": cluster.token,
                "AUTH_ADMISSION_RPC_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}"}
    result = command(report, "real-rpc-tests", [test, "native_", "--test-threads=1"],
                     test_env, cluster.root, secrets=secrets)
    report["test_diagnostics"] = migration.redact((result.stdout + result.stderr).decode(errors="replace"), secrets)
    require_test_summary(result.stdout, len(names))
    report["real_postgres_rpc_tests"] = len(names)
    before = cluster.sql(DATABASE, "select count(*) from users;")
    require(int(before) > 0, "restart-fixtures-required")
    previous = cluster.assert_postmaster()
    cluster.stop()
    cluster.start()
    require(previous != cluster.assert_postmaster(), "new-postmaster-required")
    require(cluster.sql(DATABASE, "select count(*) from users;") == before, "restart-persistence")
    cli("verify-after-restart", "--verify-migrations")
    report["restart_persistence"] = "PASS"
    report["result"] = "PASS"


def run(binary, test, pg_bin, report):
    cluster = None
    try:
        require(all(p.is_absolute() for p in (binary, test, pg_bin)), "explicit-absolute-paths")
        binary, test = migration.executable(binary), migration.executable(test)
        report["binary_sha256"], report["test_sha256"] = digest(binary), digest(test)
        tools = {name: migration.executable(pg_bin / name) for name in migration.PG_TOOLS}
        cluster = migration.Cluster(tools)
        report["owned_directory"] = str(cluster.root)
        exercise(cluster, binary, test, report)
    except (Exception, KeyboardInterrupt) as error:
        report["result"] = "FAIL"
        report["failure"] = str(error) if type(error).__name__ == "CheckFailure" else type(error).__name__
    finally:
        try:
            if cluster is not None:
                cluster.cleanup()
                require(not cluster.root.exists(), "owned-directory-remains")
            report["cleanup"] = "PASS"
        except BaseException:
            report.update(result="FAIL", cleanup="FAIL")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--rpc-test", type=Path, required=True)
    parser.add_argument("--owner-test", type=Path, required=True)
    parser.add_argument("--pg-bin", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args(argv)
    if not args.report.is_absolute():
        parser.error("report must be a new absolute file")
    try:
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except OSError:
        parser.error("report must be new and writable")
    report = {"scope": "BE04 authenticated Auth owner-read RPC prerequisite", "result": "FAIL", "commands": {},
              "gateway_jwt_profile_event_admission": "NOT_RUN", "production_concert_policy": "UNAVAILABLE",
              "production_tls_privileges_restore_replica_replay_invalidation": "NOT_RUN"}
    previous = {}

    def interrupted(*_):
        raise KeyboardInterrupt

    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.signal(sig, interrupted)
        require(args.owner_test.is_absolute(), "explicit-owner-test-required")
        owner_test = migration.executable(args.owner_test)
        report["script_sha256"] = digest(__file__)
        report["helper_sha256"] = {p.name: digest(p) for p in (Path(migration.__file__), Path(owner_smoke.__file__), ROOT / "backend/scripts/mcr/concert_core_smoke.py")}
        paths = [p for prefix in ("backend/crates/auth-application", "backend/crates/auth-domain", "backend/services/auth-service")
                 for p in (ROOT / prefix).rglob("*") if p.suffix in (".rs", ".sql", ".toml")]
        paths += [ROOT / p for p in ("backend/Cargo.toml", "backend/Cargo.lock", "backend/proto/auth/v1/auth.proto",
                  "backend/crates/platform/src/actor_delegation.rs", "backend/crates/platform/src/actor_delegation/read_authority.rs",
                  "backend/crates/platform/src/actor_delegation/authority_deadline.rs", "backend/crates/platform/src/grpc_method_policy.rs",
                  "backend/crates/platform/src/grpc_auth.rs", "backend/crates/platform/src/call_deadline.rs")]
        report["source_sha256"] = {str(p.relative_to(ROOT)): digest(p) for p in sorted(paths)}
        report["source_revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        report["dirty_files"] = subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).splitlines()
        run(args.binary, args.rpc_test, args.pg_bin, report)
        if report["result"] == "PASS":
            listed = command(report, "list-retained-owner-tests", [owner_test, "--list"],
                             {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}, ROOT)
            owner_names = re.findall(r"^([a-z0-9_]+): test$", listed.stdout.decode(), re.MULTILINE)
            require(bool(owner_names) and len(owner_names) == len(set(owner_names)), "retained-owner-tests-required")
            regression = {"commands": {}}
            owner_smoke.run(args.binary, owner_test, args.pg_bin, regression)
            report["retained_owner_regressions"] = regression
            require(regression.get("result") == "PASS" and regression.get("cleanup") == "PASS", "retained-owner-regression-failed")
            require(regression.get("real_postgres_tests") == len(owner_names), "retained-owner-tests-not-skipped")
            regression["native_test_names"] = owner_names
    except (Exception, KeyboardInterrupt) as error:
        report.update(result="FAIL", failure=type(error).__name__)
        report.setdefault("cleanup", "NOT_RUN")
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        with os.fdopen(fd, "w") as stream:
            json.dump(report, stream, indent=2)
            stream.write("\n")
    print(json.dumps({k: report.get(k) for k in ("result", "failure", "cleanup", "real_postgres_rpc_tests", "restart_persistence")}))
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
