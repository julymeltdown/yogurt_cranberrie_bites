#!/usr/bin/env python3
"""Auth current-admission read qualification in new marker/inode-owned PG16 only."""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import migration_smoke as migration
from concert_core_smoke import command, digest, require, ROOT

DATABASE = "mcr_auth_identity"
EXHAUSTION_DATABASE = "mcr_auth_identity_exhaustion"
TEST_COUNT = 22


def exercise(cluster, binary, test, report):
    cluster.init()
    cluster.start()
    report["postgres_version"] = cluster.sql("postgres", "show server_version;")
    require(report["postgres_version"].startswith("16."), "postgres16-required")
    for database in (DATABASE, EXHAUSTION_DATABASE):
        cluster.sql("postgres", f"create database {database} template template0;")
    url = cluster.url(DATABASE)
    exhaustion_url = cluster.url(EXHAUSTION_DATABASE)
    secrets = (cluster.password, url, exhaustion_url)
    env = {**cluster.env, "AUTH_DATABASE_URL": url}

    def cli(label, *args, success=True, override=None):
        return command(report, label, [binary, *args], override or env,
                       cluster.root, success=success, secrets=secrets)

    cli("help-without-database", "--help", override=cluster.env)
    cli("missing-explicit-database", "--migrate", override=cluster.env, success=False)
    cli("verify-empty-refused", "--verify-migrations", success=False)
    cli("migrate", "--migrate")
    cli("verify", "--verify-migrations")
    manifest = migration.migration_manifest(ROOT / "backend", "auth-service")
    require(set(manifest) == set(range(1, 14)), "exact-auth-migration-set")
    report["migration_sha384"] = manifest
    report["migration_ledger"] = migration.check_ledger(cluster, DATABASE, manifest)
    cli("migrate-reapply", "--migrate")
    cli("verify-reapply", "--verify-migrations")
    require(migration.check_ledger(cluster, DATABASE, manifest) == 13, "stable-ledger")
    cli("exhaustion-migrate", "--migrate", override={**cluster.env, "AUTH_DATABASE_URL": exhaustion_url})
    require(migration.check_ledger(cluster, EXHAUSTION_DATABASE, manifest) == 13, "exhaustion-ledger")
    test_env = {**env, "AUTH_IDENTITY_TEST_DATABASE_URL": url,
                "AUTH_IDENTITY_TEST_EXHAUSTION_URL": exhaustion_url,
                "AUTH_IDENTITY_TEST_OWNER_ROOT": str(cluster.root),
                "AUTH_IDENTITY_TEST_OWNER_TOKEN": cluster.token,
                "AUTH_IDENTITY_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}"}
    # Fail, do not skip, if ownership environment is missing.
    command(report, "missing-owner-refused", [test, "--exact", "email_only_requires_no_provider_relation_access"],
            cluster.env, cluster.root, success=False, secrets=secrets)
    result = command(report, "real-owner-tests", [test, "--test-threads=1"], test_env,
                     cluster.root, secrets=secrets)
    report["test_diagnostics"] = migration.redact((result.stdout + result.stderr).decode(errors="replace"), secrets)
    require(f"{TEST_COUNT} passed; 0 failed; 0 ignored".encode() in result.stdout, "owner-tests-not-skipped")
    report["real_postgres_tests"] = TEST_COUNT
    before = cluster.sql(DATABASE, "select count(*) from user_identities;")
    epoch = cluster.sql(DATABASE, "select last_value from public.auth_identity_binding_epoch;")
    require(int(before) > 0 and int(epoch) > 0, "restart-fixtures-required")
    previous = cluster.assert_postmaster()
    cluster.stop()
    cluster.start()
    require(previous != cluster.assert_postmaster(), "new-postmaster-required")
    require(cluster.sql(DATABASE, "select count(*) from user_identities;") == before, "identity-restart-persistence")
    require(cluster.sql(DATABASE, "select last_value from public.auth_identity_binding_epoch;") == epoch, "epoch-restart-persistence")
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
            report["cleanup"] = "FAIL"
            report["result"] = "FAIL"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--repository-test", type=Path, required=True)
    parser.add_argument("--pg-bin", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args(argv)
    if not args.report.is_absolute():
        parser.error("report must be a new absolute file")
    try:
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except OSError:
        parser.error("report must be new and writable")
    report = {"scope": "BE04 prerequisite: Auth-owned current admission read", "result": "FAIL", "commands": {},
              "authenticated_composition": "NOT_RUN", "concert_admission": "NOT_RUN",
              "real_provider_login": "NOT_RUN", "production_privileges_restore_invalidation": "NOT_RUN"}
    previous = {}

    def interrupted(*_):
        raise KeyboardInterrupt

    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.signal(sig, interrupted)
        report["script_sha256"] = digest(__file__)
        report["helper_sha256"] = {p.name: digest(p) for p in (Path(migration.__file__), ROOT / "backend/scripts/mcr/concert_core_smoke.py")}
        paths = [p for prefix in ("backend/crates/auth-application", "backend/crates/auth-domain", "backend/services/auth-service")
                 for p in (ROOT / prefix).rglob("*") if p.suffix in (".rs", ".sql", ".toml")]
        paths += [ROOT / "backend/Cargo.toml", ROOT / "backend/Cargo.lock"]
        report["source_sha256"] = {str(p.relative_to(ROOT)): digest(p) for p in sorted(paths)}
        report["source_revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        report["dirty_files"] = subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).splitlines()
        run(args.binary, args.repository_test, args.pg_bin, report)
    except (Exception, KeyboardInterrupt) as error:
        report.update(result="FAIL", failure=type(error).__name__, cleanup="NOT_RUN")
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        with os.fdopen(fd, "w") as stream:
            json.dump(report, stream, indent=2)
            stream.write("\n")
    print(json.dumps({k: report.get(k) for k in ("result", "failure", "cleanup", "real_postgres_tests", "restart_persistence")}))
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
