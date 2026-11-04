#!/usr/bin/env python3
"""Qualify amended SELF SQL with frozen historical storage helper, not new RPCs."""
import argparse
import json
import os
from pathlib import Path
import re
import select
import shutil
import signal
import subprocess
import uuid

import concert_self_profile_native as native
import migration_smoke as migration
from concert_core_smoke import command, digest, require


def read_reference(cluster, report):
    """Keep an actual SQL connection alive while an independent ledger commits."""
    reference = str(uuid.uuid4())
    instance = str(uuid.uuid4())
    actor = cluster.sql(native.DB, "select user_id from profiles where deleted_at is null and not exists(select 1 from profile_user_deletion_tombstones t where t.user_id=profiles.user_id) limit 1;")
    # The historical first case tombstones its only subject. Add explicit fixture
    # content, never a production fallback or an authentication grant.
    if not actor:
        actor = str(uuid.uuid4())
        cluster.sql(native.DB, f"insert into profiles(user_id,username,nickname,avatar_url,bio) values('{actor}','read-{actor}','Read fixture','retained-avatar','Existing');")
    argv = [str(cluster.tools["psql"]), "-X", "--no-password", "-q", "-A", "-t", "-v", "ON_ERROR_STOP=1"]
    process = subprocess.Popen(argv, env=cluster.client_env(native.DB), cwd=cluster.root,
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        query = f"""begin;
select json_build_object('full_xid',pg_current_xact_id()::text,'system_identifier',(select system_identifier::text from pg_control_system()),'database_oid',(select oid from pg_database where datname=current_database()),'backend_pid',pg_backend_pid(),'backend_started',(select backend_start from pg_stat_activity where pid=pg_backend_pid()),'producer_instance','{instance}','effect_digest',array_fill(11,array[32]));
"""
        process.stdin.write(query)
        process.stdin.flush()
        require(bool(select.select([process.stdout], [], [], 5)[0]), "actual-read-identity-timeout")
        identity = json.loads(process.stdout.readline())
        xid = identity["full_xid"]
        require(str(int(xid)) == xid and int(xid) > 0, "actual-read-full-xid-required")
        require(cluster.sql(native.DB, f"select pg_xact_status('{xid}'::xid8);") == "in progress", "actual-read-must-be-live")
        payload = json.dumps(identity).replace("'", "''")
        cluster.sql(native.DB, f"insert into concert_self_profile_read_references(reference_id,binding_proto,binding_digest,transaction_identity) values('{reference}',decode('01','hex'),decode(repeat('0c',32),'hex'),'{payload}'::jsonb);")
        require(cluster.sql(native.DB, f"select transaction_identity->>'full_xid' from concert_self_profile_read_references where reference_id='{reference}';") == xid, "independent-read-reference-required")
        output, errors = process.communicate(f"select nickname from profiles where user_id='{actor}' for share; rollback;\n", timeout=6)
        require(process.returncode == 0 and output.strip() == "Read fixture" and not errors, "actual-read-rollback-failed")
        require(cluster.sql(native.DB, f"select pg_xact_status('{xid}'::xid8);") == "aborted", "actual-read-rollback-not-proved")
        require(cluster.sql(native.DB, f"select count(*) from concert_self_profile_read_references where reference_id='{reference}';") == "1", "read-reference-must-survive-effect-rollback")
        require(cluster.sql(native.DB, "select count(*) from concert_self_profile_receipts;") == "0", "read-must-not-fabricate-write-receipt")
        report["read_reference_actual_rollback"] = "PASS"
    finally:
        if process.poll() is None:
            process.kill()
            process.communicate(timeout=5)


def exercise(cluster, pinned, report):
    cluster.init()
    with (cluster.data / "postgresql.conf").open("a") as stream:
        stream.write("\nmax_prepared_transactions=16\n")
    cluster.start()
    require(cluster.sql("postgres", "show server_version_num;") == "160014", "postgres-16.14-required")
    cluster.sql("postgres", f"create database {native.DB} template template0;")
    sources = sorted((native.ROOT / "backend/services/profile-service/migrations").glob("*.sql"))
    require(any(p.name == "0008_concert_self_profile.sql" for p in sources), "owned-self-migration-required")
    report["migration_sha384"] = migration.migration_manifest(native.ROOT / "backend", "profile-service")
    for source in sources:
        if not source.name.endswith(".down.sql"):
            cluster.sql(native.DB, "begin;\n" + source.read_text() + "\ncommit;")
    report["source_sql_application"] = "PASS"
    env = {**cluster.env, "MCR_SELF_PROFILE_ROOT": str(cluster.root), "MCR_SELF_PROFILE_TOKEN": cluster.token,
           "MCR_SELF_PROFILE_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}", "MCR_SELF_PROFILE_URL": cluster.url(native.DB)}
    secrets = (cluster.password, cluster.token, env["MCR_SELF_PROFILE_URL"])
    def run(suffix):
        response = command(report, suffix, [pinned, "--ignored", "--exact", native.PREFIX.removesuffix("native_") + suffix, "--test-threads=1"], env, cluster.root, secrets=secrets)
        require(re.findall(rb"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;", response.stdout, re.MULTILINE) == [(b"1", b"0", b"0")], "native-case-missing")
    run("native_versions_cover_retained_updates_and_tombstone_fences")
    read_reference(cluster, report)
    run("native_prepare_preserves_visibility_and_blocks_legacy_writes")
    require(cluster.sql(native.DB, "select count(*) from pg_prepared_xacts where database=current_database();") == "2", "prepared-effects-required")
    before = cluster.assert_postmaster()
    cluster.stop()
    cluster.start()
    require(cluster.assert_postmaster() != before, "controlled-restart-required")
    require(cluster.sql(native.DB, "select count(*) from pg_prepared_xacts where database=current_database();") == "2", "prepared-survival-required")
    run("native_after_restart_resolves_actual_commit_and_abort_receipts")
    require(cluster.sql(native.DB, "select count(*) from pg_prepared_xacts where database=current_database();") == "0", "prepared-cleanup-required")
    require(cluster.sql(native.DB, "select count(*) from concert_self_profile_read_references;") == "1", "durable-read-reference-required")
    report.update(native_tests=3, restart="PASS", result="PASS")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cargo-json", "frozen-artifacts", "pg-bin", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    require(all(p.is_absolute() for p in vars(args).values()), "absolute-inputs-required")
    report = {"result": "FAIL", "scope": "amended Profile SELF migration SQL using historical frozen storage helper", "new_rust_rpc_compile": "NOT_RUN", "migration_cli": "NOT_RUN", "actual_canonical_guard_coordinator_rpc": "NOT_RUN", "commands": {}}
    fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    cluster = None
    previous = {}
    def interrupt(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.signal(sig, interrupt)
        require(shutil.disk_usage(native.ROOT).free >= 8 * 1024**3, "eight-gibibyte-floor-required")
        selected = native.artifacts(args.cargo_json, args.frozen_artifacts)
        report["historical_test_sha256"] = digest(selected["test"])
        report["cargo_json_sha256"] = digest(args.cargo_json)
        report["source_revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=native.ROOT, text=True).strip()
        report["controller_sha256"] = digest(Path(__file__))
        cluster = migration.Cluster({name: migration.executable(args.pg_bin / name) for name in migration.PG_TOOLS})
        report["owned_directory"] = str(cluster.root)
        exercise(cluster, selected["test"], report)
    except (Exception, KeyboardInterrupt) as error:
        report["failure"] = str(error) if type(error).__name__ == "CheckFailure" else type(error).__name__
    finally:
        try:
            if cluster is not None:
                cluster.cleanup()
                require(not cluster.root.exists(), "owned-root-remains")
            report["cleanup"] = "PASS"
        except BaseException:
            report.update(result="FAIL", cleanup="FAIL")
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        with os.fdopen(fd, "w") as stream:
            json.dump(report, stream, indent=2)
            stream.write("\n")
    print(json.dumps({key: report.get(key) for key in ("result", "failure", "native_tests", "read_reference_actual_rollback", "restart", "cleanup")}))
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
