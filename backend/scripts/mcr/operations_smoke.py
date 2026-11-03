#!/usr/bin/env python3
"""BE36/38 native PG16 evidence; creates, owns, restarts and removes a private cluster."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import migration_smoke as migration
from concert_core_smoke import command, digest, require, ROOT

DATABASE = "mcr_concert_be_operations"
CONTRACTS = ("admin.schema.json", "feed.schema.json", "operations.json", "policy.json")
PINNED = ("b8a4f8b91440d561bbcf23dc68bc196919a8893cf744ca09ac05ecaabff72cb8",
          "9c8d991df0bb707d7a7a2d43d4b132e7fadb878a52633e1c1f338afc9407f8ed",
          "42ed48ee0cad846b94f9f7a5d39a38beeeb5f5b41e0129f157e51ff31c635223",
          "508d22af964b718cdcbd5a25b32b9a1e3f7b651a5b8c25c87d56f1f6b68d0ebe")

def exercise(cluster, binary, tests, report):
    cluster.init()
    cluster.start()
    report["postgres_version"] = cluster.sql("postgres", "show server_version;")
    require(report["postgres_version"].startswith("16."), "postgres16-required")
    cluster.sql("postgres", f"create database {DATABASE} template template0;")
    url = cluster.url(DATABASE)
    env = {**cluster.env, "CONCERT_DATABASE_URL": url}
    secrets = (cluster.password, url)
    def cli(label, *args, success=True, override=None):
        return command(report, label, [binary, *args], override or env, cluster.root,
                       success=success, secrets=secrets)
    cli("missing-database-refused", "--migrate", success=False, override=cluster.env)
    cli("empty-ledger-refused", "--verify-migrations", success=False)
    cli("migrate", "--migrate")
    cli("verify", "--verify-migrations")
    manifest = migration.migration_manifest(ROOT / "backend", "concert-service")
    require(15 in manifest, "operations-migration-required")
    report["migration_sha384"] = manifest
    report["migration_ledger"] = migration.check_ledger(cluster, DATABASE, manifest)
    cli("reapply", "--migrate")
    cli("verify-reapply", "--verify-migrations")
    require(migration.check_ledger(cluster, DATABASE, manifest) == len(manifest), "stable-ledger")
    cluster.assert_owned()
    test_env = {**env, "CONCERT_TEST_DATABASE_URL": url,
                "CONCERT_TEST_OWNER_ROOT": str(cluster.root),
                "CONCERT_TEST_OWNER_TOKEN": cluster.token,
                "CONCERT_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}"}
    for label, test, count in tests:
        result = command(report, label, [test, "--test-threads=1"], test_env,
                         cluster.root, secrets=secrets)
        report[label + "_diagnostics"] = migration.redact((result.stdout + result.stderr).decode(errors="replace"), secrets)
        require(f"{count} passed; 0 failed".encode() in result.stdout, label + "-not-skipped")
    report["real_postgres_tests"] = sum(count for _, _, count in tests)
    tables = ("concert_reports", "concert_event_settings", "concert_operation_notices",
              "concert_command_receipts", "concert_outbox", "concert_operations_audit")
    before = {table: cluster.sql(DATABASE, f"select count(*) from {table};") for table in tables}
    require(all(int(count) > 0 for count in before.values()), "committed-restart-fixtures-required")
    prior = cluster.assert_postmaster()
    cluster.stop()
    cluster.start()
    require(prior != cluster.assert_postmaster(), "new-postmaster-required")
    for table, count in before.items():
        require(cluster.sql(DATABASE, f"select count(*) from {table};") == count, table + "-restart-persistence")
    cli("verify-after-restart", "--verify-migrations")
    report["restart_counts"] = before
    report["restart_persistence"] = "PASS"
    report["result"] = "PASS"

def run(binary, report_test, operations_test, place_test, pg_bin, report):
    cluster = None
    try:
        require(all(p.is_absolute() for p in (binary, report_test, operations_test, place_test, pg_bin)), "explicit-absolute-paths")
        paths = tuple(migration.executable(p) for p in (binary, report_test, operations_test, place_test))
        report["binary_sha256"] = {name: digest(p) for name, p in zip(("service", "report", "operations", "places"), paths)}
        tools = {name: migration.executable(pg_bin / name) for name in migration.PG_TOOLS}
        cluster = migration.Cluster(tools)
        report["owned_directory"] = str(cluster.root)
        exercise(cluster, paths[0], (("report-tests", paths[1], 6), ("operations-tests", paths[2], 7), ("place-tests",paths[3],3)), report)
    except (Exception, KeyboardInterrupt) as error:
        report["result"] = "FAIL"
        report["failure"] = str(error) if type(error).__name__ == "CheckFailure" else type(error).__name__
    finally:
        if cluster is not None:
            try:
                cluster.cleanup()
                require(not cluster.root.exists(), "owned-directory-remains")
                report["cleanup"] = "PASS"
            except (Exception, KeyboardInterrupt) as error:
                report.update(result="FAIL", cleanup="FAIL", cleanup_failure=type(error).__name__)

def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("binary", "report-test", "operations-test", "place-test", "pg-bin", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args(argv)
    if not args.report.is_absolute():
        parser.error("report must be a new absolute file")
    try:
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except OSError:
        parser.error("report must be new and writable")
    report = {"scope": "BE36/38 internal transaction producers", "result": "FAIL", "commands": {},
              "production_authority": "NOT_COMPOSED", "real_media_target_review": "NOT_RUN",
              "qualified_http_multiuser": "NOT_RUN", "place_owner_mutation": "NOT_COMPOSED"}
    previous = {}
    def interrupted(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.signal(sig, interrupted)
        report["script_sha256"] = digest(__file__)
        report["helper_sha256"] = {p.name: digest(p) for p in (Path(migration.__file__), ROOT / "backend/scripts/mcr/concert_core_smoke.py")}
        paths = [p for prefix in ("backend/crates/concert-domain", "backend/crates/concert-application", "backend/services/concert-service") for p in (ROOT / prefix).rglob("*") if p.suffix in (".rs", ".sql", ".toml")]
        paths += [ROOT / "backend/Cargo.toml", ROOT / "backend/Cargo.lock"]
        report["source_sha256"] = {str(p.relative_to(ROOT)): digest(p) for p in sorted(paths)}
        report["contract_sha256"] = {name: digest(ROOT / "contracts/domain/v1" / name) for name in CONTRACTS}
        require(tuple(report["contract_sha256"].values()) == PINNED, "contract-candidate-drift")
        report["source_revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        report["dirty_files"] = subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).splitlines()
        run(args.binary, args.report_test, args.operations_test, args.place_test, args.pg_bin, report)
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
