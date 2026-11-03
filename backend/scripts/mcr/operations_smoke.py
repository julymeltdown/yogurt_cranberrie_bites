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
FORWARD_DATABASE = "mcr_concert_be_operations_forward"
CONTRACTS = ("admin.schema.json", "feed.schema.json", "operations.json", "policy.json")
PINNED = ("953e78a482d998e35c9612c30467600adcead3c9e9d4fb6ab7679a2164aef640",
          "590cce6afc00c7a869b5a9711104550ba280ffa5875c9dc49a0753d272320801",
          "622c0e0eba6e1c835c5e953a3acd6825f97ed43aff9bc32b982c207fdfe31839",
          "81b0da4c007543765f120c4ea58163cc1eadea41e10d24db54e1d177ef4e5fbb")
PUBLISHED = (1,2,3,4,5,6,7,8,9,10,11,12,16,18,19,20)

def exercise(cluster, binary, tests, report):
    cluster.init()
    cluster.start()
    report["postgres_version"] = cluster.sql("postgres", "show server_version;")
    require(report["postgres_version"].startswith("16."), "postgres16-required")
    for database in (DATABASE, FORWARD_DATABASE):
        cluster.sql("postgres", f"create database {database} template template0;")
    urls = {name: cluster.url(name) for name in (DATABASE, FORWARD_DATABASE)}
    secrets = (cluster.password, *urls.values())
    def environment(database):
        return {**cluster.env, "CONCERT_DATABASE_URL": urls[database]}
    def cli(label, *args, database=DATABASE, success=True, override=None):
        return command(report, label, [binary, *args], override or environment(database), cluster.root,
                       success=success, secrets=secrets)
    def native(label, test, database, arguments, count, ignored=0):
        cluster.assert_owned()
        env = {**environment(database), "CONCERT_TEST_DATABASE_URL": urls[database],
               "CONCERT_TEST_OWNER_ROOT": str(cluster.root), "CONCERT_TEST_OWNER_TOKEN": cluster.token,
               "CONCERT_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}"}
        result = subprocess.run([str(test), *arguments, "--test-threads=1"], env=env, cwd=cluster.root,
                                capture_output=True, timeout=60)
        report["commands"][label] = result.returncode
        report[label + "_diagnostics"] = migration.redact((result.stdout + result.stderr).decode(errors="replace"), secrets)
        require(all(value.encode() not in result.stdout + result.stderr for value in secrets), label + "-redaction")
        require(result.returncode == 0, label + "-exit")
        require(f"test result: ok. {count} passed; 0 failed; {ignored} ignored;".encode() in result.stdout,
                label + "-complete-exact-count")
    def snapshot():
        event = cluster.sql(FORWARD_DATABASE, "select row_to_json(e)::text from concert_events e where event_id='00000000-0000-4000-8000-000000000021';")
        receipt = cluster.sql(FORWARD_DATABASE, "select row_to_json(r)::text from concert_command_receipts r where receipt_id='00000000-0000-4000-8000-000000000022';")
        require(bool(event) and bool(receipt), "published-state-required")
        return {"event_sha256": hashlib.sha256(event.encode()).hexdigest(),
                "receipt_sha256": hashlib.sha256(receipt.encode()).hexdigest()}

    cli("missing-database-refused", "--migrate", success=False, override=cluster.env)
    cli("empty-ledger-refused", "--verify-migrations", success=False)
    cli("fresh-migrate", "--migrate")
    cli("fresh-verify", "--verify-migrations")
    manifest = migration.migration_manifest(ROOT / "backend", "concert-service")
    require(tuple(manifest) == (*PUBLISHED, 21), "exact-current-migration-set")
    report["migration_sha384"] = manifest
    report["fresh_migration_ledger"] = migration.check_ledger(cluster, DATABASE, manifest)
    cli("fresh-reapply", "--migrate")
    cli("fresh-verify-reapply", "--verify-migrations")
    require(migration.check_ledger(cluster, DATABASE, manifest) == len(manifest), "fresh-stable-ledger")

    # A compiled, ownership-checked helper runs the actual SQLx baseline migrations.
    # No SQLx success/checksum row is manufactured by this harness.
    operations_test = next(test for label, test, _ in tests if label == "operations-tests")
    native("published-0020-checkpoint", operations_test, FORWARD_DATABASE,
           ["--ignored", "--exact", "operations_support::published_0020_checkpoint"], 1)
    baseline = {version: checksum for version, checksum in manifest.items() if version <= 20}
    report["forward_baseline_ledger"] = migration.check_ledger(cluster, FORWARD_DATABASE, baseline)
    before_forward = snapshot()
    cli("forward-pending-migration-refused", "--verify-migrations", database=FORWARD_DATABASE, success=False)
    cli("forward-0021-late-fault", "--migrate", database=FORWARD_DATABASE, success=False)
    require(migration.check_ledger(cluster, FORWARD_DATABASE, baseline) == len(baseline), "failed-forward-ledger-unchanged")
    require(cluster.sql(FORWARD_DATABASE, "select count(*) from pg_tables where schemaname='public' and tablename in ('concert_reports','concert_event_settings','concert_operation_notices','concert_operations_audit');") == "0", "failed-forward-no-partial-tables")
    require(snapshot() == before_forward, "failed-forward-published-state-unchanged")
    cluster.sql(FORWARD_DATABASE, "drop table concert_operated_place_releases;")
    cli("forward-0020-to-0021", "--migrate", database=FORWARD_DATABASE)
    cli("forward-verify", "--verify-migrations", database=FORWARD_DATABASE)
    report["forward_migration_ledger"] = migration.check_ledger(cluster, FORWARD_DATABASE, manifest)
    require(snapshot() == before_forward, "forward-published-state-preserved")
    cli("forward-reapply", "--migrate", database=FORWARD_DATABASE)
    cli("forward-verify-reapply", "--verify-migrations", database=FORWARD_DATABASE)
    require(migration.check_ledger(cluster, FORWARD_DATABASE, manifest) == len(manifest), "forward-stable-ledger")
    report["forward_rollback"] = "PASS"
    report["forward_published_state"] = before_forward

    for label, test, count in tests:
        native(label, test, DATABASE, [], count, 1 if label == "operations-tests" else 0)
    report["native_adapter_tests"] = sum(count for _, _, count in tests)
    report["native_ledger_tests"] = 1
    report["real_postgres_tests"] = report["native_adapter_tests"] + report["native_ledger_tests"]
    tables = ("concert_reports", "concert_event_settings", "concert_operation_notices",
              "concert_command_receipts", "concert_outbox", "concert_operations_audit",
              "concert_operated_place_releases")
    before = {table: cluster.sql(DATABASE, f"select count(*) from {table};") for table in tables}
    require(all(int(count) > 0 for count in before.values()), "committed-restart-fixtures-required")
    prior = cluster.assert_postmaster()
    cluster.stop()
    cluster.start()
    require(prior != cluster.assert_postmaster(), "new-postmaster-required")
    for table, count in before.items():
        require(cluster.sql(DATABASE, f"select count(*) from {table};") == count, table + "-restart-persistence")
    require(snapshot() == before_forward, "forward-published-state-survives-restart")
    for database in (DATABASE, FORWARD_DATABASE):
        cli(database + "-verify-after-restart", "--verify-migrations", database=database)
        require(migration.check_ledger(cluster, database, manifest) == len(manifest), "restarted-ledger-preserved")
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
