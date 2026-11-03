#!/usr/bin/env python3
"""BE15 real PG16 qualification in a new marker/inode-owned cluster only."""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import migration_smoke as migration
from concert_core_smoke import command, digest, require, ROOT

DATABASE = "mcr_concert_be_commands"
FAILURE_DATABASE = "mcr_concert_be_commands_failure"
CONTRACTS = ("common.schema.json", "operations.json", "policy.json")
PINNED = ("303f3019611c636679282d6c29106deab83d7c0c9717bd8b9ba526d2ff865a35",
          "0acd5e1c68b5136916d1560dfbb2eef7e59410c81987bf0ea24b0d605c11083e",
          "728b397bd175c55b35eddef8b925f585676d7589a0b1250fbde5ef3c7c9d4019")

def exercise(cluster, binary, test, report):
    cluster.init()
    cluster.start()
    report["postgres_version"] = cluster.sql("postgres", "show server_version;")
    require(report["postgres_version"].startswith("16."), "postgres16-required")
    for database in (DATABASE, FAILURE_DATABASE):
        cluster.sql("postgres", f"create database {database} template template0;")
    url = cluster.url(DATABASE)
    env = {**cluster.env, "CONCERT_DATABASE_URL": url}
    secrets = (cluster.password, url, cluster.url(FAILURE_DATABASE))
    def cli(label, *args, success=True, override=None):
        return command(report, label, [binary, *args], override or env, cluster.root,
                       success=success, secrets=secrets)
    cli("missing-explicit-database", "--migrate", override=cluster.env, success=False)
    cli("foreign-database-refused", "--migrate", success=False,
        override={**cluster.env, "CONCERT_DATABASE_URL": "postgresql://untrusted@localhost/postgres"})
    cli("verify-empty-refused", "--verify-migrations", success=False)
    # Deliberately collide at late DDL inside migration 0002. Earlier statements
    # of 0002 and its receipt must roll back; 0001 remains forward-recoverable.
    cluster.sql(FAILURE_DATABASE, "create table concert_outbox(failure_fixture boolean);")
    cli("migration-failure", "--migrate", success=False,
        override={**cluster.env, "CONCERT_DATABASE_URL": cluster.url(FAILURE_DATABASE)})
    require(cluster.sql(FAILURE_DATABASE, "select count(*) from pg_tables where schemaname='public' and tablename='concert_command_receipts';") == "0", "migration-no-partial-receipt-table")
    require(cluster.sql(FAILURE_DATABASE, "select count(*) from _sqlx_migrations where version=2;") == "0", "migration-no-false-success-receipt")
    cluster.sql(FAILURE_DATABASE, "drop table concert_outbox;")  # Only our deliberate disposable fixture.
    cli("migration-forward-recovery", "--migrate",
        override={**cluster.env, "CONCERT_DATABASE_URL": cluster.url(FAILURE_DATABASE)})
    cli("migrate", "--migrate")
    cli("verify", "--verify-migrations")
    manifest = migration.migration_manifest(ROOT / "backend", "concert-service")
    require(set(manifest) == {1, 2}, "exact-migration-set")
    report["migration_sha384"] = manifest
    report["migration_ledger"] = migration.check_ledger(cluster, DATABASE, manifest)
    cli("reapply", "--migrate")
    cli("verify-reapply", "--verify-migrations")
    require(migration.check_ledger(cluster, DATABASE, manifest) == 2, "stable-ledger")
    # Missing/changed checksum is rejected, then restore only our own fixture row.
    original = cluster.sql(DATABASE, "select encode(checksum,'hex') from _sqlx_migrations where version=2;")
    cluster.sql(DATABASE, "update _sqlx_migrations set checksum=decode(repeat('00',48),'hex') where version=2;")
    cli("damaged-ledger-refused", "--verify-migrations", success=False)
    cluster.sql(DATABASE, f"update _sqlx_migrations set checksum=decode('{original}','hex') where version=2;")
    test_env = {**env, "CONCERT_TEST_DATABASE_URL": url,
                "CONCERT_TEST_OWNER_ROOT": str(cluster.root),
                "CONCERT_TEST_OWNER_TOKEN": cluster.token,
                "CONCERT_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}"}
    result = subprocess.run([str(test), "--test-threads=1"], env=test_env,
                            cwd=cluster.root, capture_output=True, timeout=30)
    report["commands"]["real-command-tests"] = result.returncode
    require(all(value.encode() not in result.stdout + result.stderr for value in secrets), "test-output-redaction")
    report["test_diagnostics"] = migration.redact((result.stdout + result.stderr).decode(errors="replace"), secrets)
    require(result.returncode == 0, "real-command-tests:exit")
    require(b"17 passed; 0 failed" in result.stdout, "tests-not-skipped")
    report["real_postgres_tests"] = 17
    # Already committed pending rows and safe results must survive actual server restart.
    before = cluster.sql(DATABASE, "select count(*) from concert_command_receipts;")
    queue = cluster.sql(DATABASE, "select count(*) from concert_outbox;")
    require(int(before) > 0 and int(queue) > 0, "restart-fixtures-required")
    previous = cluster.assert_postmaster()
    cluster.stop()
    cluster.start()
    require(previous != cluster.assert_postmaster(), "new-postmaster-required")
    require(cluster.sql(DATABASE, "select count(*) from concert_command_receipts;") == before, "receipt-restart-persistence")
    require(cluster.sql(DATABASE, "select count(*) from concert_outbox;") == queue, "outbox-restart-persistence")
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
        # Safe, bounded diagnostics only. Never include a connection URL or raw DB error.
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
    report = {"scope": "BE15 bounded internal command/outbox foundation", "result": "FAIL", "commands": {},
              "production_producer": "NOT_COMPOSED", "external_exactly_once": "NOT_RUN", "real_auth_policy": "NOT_RUN"}
    previous = {}
    def interrupted(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.signal(sig, interrupted)
        report["script_sha256"] = digest(__file__)
        report["helper_sha256"] = {p.name: digest(p) for p in (Path(migration.__file__), ROOT / "backend/scripts/mcr/concert_core_smoke.py")}
        paths = [p for prefix in ("backend/crates/concert-application", "backend/services/concert-service") for p in (ROOT / prefix).rglob("*") if p.suffix in (".rs", ".sql", ".toml")]
        paths += [ROOT / "backend/Cargo.toml", ROOT / "backend/Cargo.lock"]
        report["source_sha256"] = {str(p.relative_to(ROOT)): digest(p) for p in sorted(paths)}
        report["contract_sha256"] = {name: digest(ROOT / "contracts/domain/v1" / name) for name in CONTRACTS}
        require(tuple(report["contract_sha256"].values()) == PINNED, "contract-candidate-drift")
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
