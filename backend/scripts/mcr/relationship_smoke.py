#!/usr/bin/env python3
"""BE37/BE28–30 in a newly owned PostgreSQL16 cluster; never attaches existing data."""
from __future__ import annotations
import argparse
import json
import os
import re
from pathlib import Path
import signal
import subprocess
import migration_smoke as migration
from concert_core_smoke import require, digest, command, ROOT

DATABASE = "mcr_concert_relationship"

def exercise(cluster, test, binary, report):
    cluster.init()
    cluster.start()
    report["postgres_version"] = cluster.sql("postgres", "show server_version;")
    require(report["postgres_version"].startswith("16."), "postgres16-required")
    cluster.sql("postgres", f"create database {DATABASE} template template0;")
    migrations = ROOT / "backend/services/concert-service/migrations"
    report["migration_sha256"] = {}
    # This slice owns 0006; 0004/5 belong to concurrent workers. Qualify exact
    # prerequisite 0001/2/3 and owned 0006, not unintegrated foreign migrations.
    names = ("0001_concert_core.sql", "0002_command_receipts_outbox.sql", "0003_places_map_releases.sql", "0006_relationship_policy.sql")
    report["migration_sha256"] = {name:digest(migrations/name) for name in names}
    def cli(label, flag, database=DATABASE, success=True):
        return command(report,label,[binary,flag],{**cluster.env,"CONCERT_DATABASE_URL":cluster.url(database)},ROOT,
                       success=success,secrets=(cluster.password,cluster.url(database)))
    if binary is not None:
        cli("empty-ledger-refused","--verify-migrations",success=False)
        cli("runtime-migrate","--migrate")
        cli("runtime-verify","--verify-migrations")
        cli("runtime-reapply","--migrate")
        expected = migration.migration_manifest(ROOT/"backend","concert-service")
        require(set(expected)=={1,2,3,6},"exact-bounded-migration-set")
        report["migration_sha384"] = expected
        report["migration_ledger_rows"] = migration.check_ledger(cluster,DATABASE,expected)
        failure_db = DATABASE+"_failure"
        cluster.sql("postgres", f"create database {failure_db} template template0;")
        cluster.sql(failure_db,"create table concert_social_handles(test_only boolean);")
        cli("0006-collision-refused","--migrate",failure_db,False)
        require(cluster.sql(failure_db,"select count(*) from pg_tables where schemaname='public' and tablename='concert_relationship_pairs';")=="0","0006-no-partial-ddl")
        require(cluster.sql(failure_db,"select count(*) from _sqlx_migrations where version=6;")=="0","0006-no-false-receipt")
        cluster.sql(failure_db,"drop table concert_social_handles;")
        cli("0006-forward-recovery","--migrate",failure_db)
        cli("0006-recovered-verify","--verify-migrations",failure_db)
        original=cluster.sql(DATABASE,"select encode(checksum,'hex') from _sqlx_migrations where version=6;")
        cluster.sql(DATABASE,"update _sqlx_migrations set checksum=decode(repeat('00',48),'hex') where version=6;")
        cli("0006-checksum-drift-refused","--verify-migrations",success=False)
        cluster.sql(DATABASE,f"update _sqlx_migrations set checksum=decode('{original}','hex') where version=6;")
        report["runtime_migration_qualification"]="PASS"
    else:
        for name in names:
            cluster.sql(DATABASE, "begin;\n" + (migrations/name).read_text() + "\ncommit;")
        report["commands"]["transactional-sql-migrations"] = 0
        report["runtime_migration_qualification"]="NOT_RUN"
    cluster.sql(DATABASE, "create table relationship_test_authority(event_id uuid,user_id uuid,allowed boolean not null,inherited_block boolean not null,revision bigint not null,primary key(event_id,user_id));")
    cluster.sql(DATABASE, "create table relationship_test_intro(event_id uuid,intro_id uuid,requester_id uuid,recipient_id uuid,revision bigint,allowed boolean,expires_at timestamptz,primary key(event_id,intro_id)); create table relationship_test_outbox_fail(event_id uuid primary key); create table relationship_test_outbox_delay(event_id uuid primary key); create table relationship_test_roots(event_id uuid primary key); create function relationship_test_reject_outbox() returns trigger language plpgsql as $$ begin if exists(select 1 from relationship_test_outbox_fail where event_id=NEW.event_id) then raise exception 'test-only outbox failure'; end if; if exists(select 1 from relationship_test_outbox_delay where event_id=NEW.event_id) then perform pg_sleep(0.2); end if; return NEW; end; $$; create trigger relationship_test_reject_outbox before insert on concert_outbox for each row execute function relationship_test_reject_outbox();")
    if test is not None:
        env = {**cluster.env, "CONCERT_TEST_DATABASE_URL": cluster.url(DATABASE),
               "CONCERT_TEST_OWNER_ROOT": str(cluster.root),
               "CONCERT_TEST_OWNER_TOKEN": cluster.token,
               "CONCERT_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}"}
        result = subprocess.run([str(test), "--test-threads=1"], cwd=ROOT, env=env, capture_output=True, timeout=60)
        secrets = (cluster.password, cluster.url(DATABASE))
        require(all(s.encode() not in result.stdout+result.stderr for s in secrets), "test-output-redaction")
        report["commands"]["real-repository-tests"] = result.returncode
        output = (result.stdout+result.stderr).decode(errors="replace")
        report["test_diagnostics"] = migration.redact(output, secrets)
        require(result.returncode == 0, "real-repository-tests:exit")
        require("0 failed" in output and "0 ignored" in output and "running 0 tests" not in output, "tests-not-skipped")
        expected_tests = re.findall(r"#\[tokio::test\]\s*async fn (\w+)", (ROOT/"backend/services/concert-service/tests/relationship_policy.rs").read_text())
        require(len(expected_tests) == 22 and all(f"test {name} ... ok" in output for name in expected_tests), "all-owned-tests-required")
        report["real_postgres_test_count"] = len(expected_tests)
        report["repository_tests"] = "PASS"
    else:
        report["repository_tests"] = "NOT_RUN"
    # Durable policy state survives a real postmaster restart.
    event = "00000000-0000-0000-0000-000000000037"
    low = "00000000-0000-0000-0000-000000000028"
    high = "00000000-0000-0000-0000-000000000030"
    cluster.sql(DATABASE, f"begin; select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '1 minute')::text,true); insert into concert_events(event_id,slug,title,phase) values('{event}','restart-policy','Restart fixture','LIVE'); insert into concert_relationship_pairs(event_id,low_user,high_user,low_blocks_high) values('{event}','{low}','{high}',true); commit;")
    previous = cluster.assert_postmaster()
    cluster.stop()
    cluster.start()
    require(previous != cluster.assert_postmaster(), "new-postmaster-required")
    require(cluster.sql(DATABASE, f"select low_blocks_high::text||':'||revision from concert_relationship_pairs where event_id='{event}';") == "true:1", "block-restart-persistence")
    if binary is not None:
        cli("runtime-verify-after-restart","--verify-migrations")
        require(migration.check_ledger(cluster,DATABASE,expected)==report["migration_ledger_rows"],"stable-ledger-after-restart")
    report["restart_persistence"] = "PASS"
    report["result"] = "PASS"

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository-test", type=Path)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--pg-bin", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    require(args.pg_bin.is_absolute() and args.report.is_absolute(), "absolute-paths-required")
    fd = os.open(args.report, os.O_CREAT|os.O_EXCL|os.O_WRONLY, 0o600)
    cluster = None
    report = {"scope":"BE37/BE28–30 owned persistence", "result":"FAIL", "commands":{},
              "real_owner_authority":"NOT_RUN", "http_ws_integration":"NOT_RUN"}
    previous = {}
    def interrupted(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.signal(sig, interrupted)
        report["script_sha256"] = digest(__file__)
        report["source_revision"] = subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip()
        report["dirty_files"] = subprocess.check_output(["git","status","--porcelain"],cwd=ROOT,text=True).splitlines()
        paths = [p for prefix in ("backend/crates/concert-domain/src/relationship", "backend/crates/concert-application/src/relationship", "backend/services/concert-service/src/relationship", "backend/services/concert-service/tests/support/relationship") for p in (ROOT/prefix).rglob("*.rs")]
        paths += [ROOT/p for p in ("backend/services/concert-service/tests/relationship_policy.rs", "backend/crates/concert-domain/tests/relationship_policy.rs", "backend/crates/concert-domain/src/lib.rs", "backend/crates/concert-application/src/lib.rs", "backend/services/concert-service/src/lib.rs", "backend/services/concert-service/Cargo.toml", "backend/Cargo.lock")]
        report["source_sha256"] = {str(p.relative_to(ROOT)):digest(p) for p in paths}
        report["helper_sha256"] = {p.name:digest(p) for p in (Path(migration.__file__),ROOT/"backend/scripts/mcr/concert_core_smoke.py")}
        test = None if args.repository_test is None else migration.executable(args.repository_test)
        if test:
            require(args.repository_test.is_absolute(), "absolute-test-path-required")
            report["test_binary_sha256"] = digest(test)
        binary = None if args.binary is None else migration.executable(args.binary)
        if binary:
            require(args.binary.is_absolute(),"absolute-binary-path-required")
            report["service_binary_sha256"] = digest(binary)
        tools = {name:migration.executable(args.pg_bin/name) for name in migration.PG_TOOLS}
        cluster = migration.Cluster(tools)
        exercise(cluster,test,binary,report)
    except (Exception,KeyboardInterrupt) as error:
        report["failure"] = str(error) if type(error).__name__ == "CheckFailure" else type(error).__name__
    finally:
        try:
            if cluster:
                cluster.cleanup()
                require(not cluster.root.exists(),"owned-directory-remains")
            report["cleanup"] = "PASS"
        except BaseException:
            report.update(result="FAIL",cleanup="FAIL")
        for sig,handler in previous.items():
            signal.signal(sig,handler)
        with os.fdopen(fd,"w") as out:
            json.dump(report,out,indent=2)
            out.write("\n")
    print(json.dumps({k:report.get(k) for k in ("result","failure","cleanup","repository_tests","restart_persistence")}))
    return 0 if report["result"] == "PASS" else 1

if __name__ == "__main__":
    raise SystemExit(main())
