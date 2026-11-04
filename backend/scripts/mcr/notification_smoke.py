#!/usr/bin/env python3
"""BE33 real PostgreSQL qualification, in a newly owned socket-only fixture."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import stat
import subprocess
import migration_smoke as migration

ROOT = Path(__file__).resolve().parents[3]
NOTIFICATION_DB = "mcr_concert_notification"
CONCERT_DB = "mcr_concert_notification_source"


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def pin_binary(path, expected, cluster, name):
    require(path.is_absolute() and re.fullmatch(r"[0-9a-f]{64}", expected), "explicit-binary-and-sha-required")
    info = path.lstat()
    require(stat.S_ISREG(info.st_mode) and not path.is_symlink() and os.access(path, os.X_OK), "regular-executable-required")
    require(digest(path) == expected, "binary-hash-mismatch")
    cluster.assert_owned()
    destination = cluster.root / name
    with destination.open("xb") as out, path.open("rb") as source:
        shutil.copyfileobj(source, out)
    destination.chmod(0o700)
    require(digest(destination) == expected and digest(path) == expected, "binary-changed-during-pin")
    return destination


def tests(cluster, executable, source, database, report, label, exact=None):
    # No inherited DATABASE_URL, token, provider or arbitrary network fallback.
    env = {**cluster.env, "CONCERT_TEST_DATABASE_URL": cluster.url(database),
           "CONCERT_TEST_NOTIFICATION_DATABASE_URL": cluster.url(NOTIFICATION_DB),
           "CONCERT_TEST_OWNER_ROOT": str(cluster.root),
           "CONCERT_TEST_OWNER_TOKEN": cluster.token,
           "CONCERT_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}"}
    argv = [str(executable), "--test-threads=1"]
    if exact:
        argv += ["--exact", exact]
    result = subprocess.run(argv, env=env, cwd=ROOT, capture_output=True, timeout=60)
    output = result.stdout + result.stderr
    secrets = (cluster.password, cluster.url(database), cluster.url(NOTIFICATION_DB))
    require(all(s.encode() not in output for s in secrets), "test-output-redaction")
    text = output.decode(errors="replace")
    report["commands"][label] = result.returncode
    report[label] = {"output": migration.redact(text, secrets), "result": "FAIL"}
    require(result.returncode == 0, f"{label}:exit")
    expected = [exact] if exact else re.findall(r"#\[tokio::test\]\s*async fn (\w+)", source.read_text())
    require(expected and all(f"test {name} ... ok" in text for name in expected), "all-tests-must-execute")
    require("0 failed" in text and "0 ignored" in text, "no-skipped-or-failed-tests")
    report[label].update(result="PASS", test_count=len(expected))


def run(cluster, args, report):
    pinned = {}
    for name in ("notification_test", "concert_test", "service_binary"):
        path = getattr(args, name)
        expected = getattr(args, name + "_sha256")
        if path is not None:
            pinned[name] = pin_binary(path, expected or "", cluster, name)
            report["binary_sha256"][name] = expected
    require("notification_test" in pinned and "concert_test" in pinned, "both-real-test-binaries-required")
    cluster.init()
    cluster.start()
    version = cluster.sql("postgres", "show server_version;")
    require(version.startswith("16."), "postgres16-required")
    report["postgres_version"] = version
    for db in (NOTIFICATION_DB, CONCERT_DB):
        cluster.sql("postgres", f"create database {db} template template0;")
    n_migrations = ROOT / "backend/services/notification-service/migrations"
    n_paths = sorted(n_migrations.glob("*.sql"))
    require([int(p.name.split("_")[0]) for p in n_paths] == [1, 2, 3, 4, 5, 6], "exact-notification-migration-set")
    c_dir = ROOT / "backend/services/concert-service/migrations"
    c_paths = [c_dir / n for n in ("0001_concert_core.sql", "0002_command_receipts_outbox.sql", "0011_outbox_maintenance.sql")]
    report["migration_sha256"] = {str(p.relative_to(ROOT)): digest(p) for p in n_paths + c_paths}
    if "service_binary" in pinned:
        def cli(label, flag, success=True, database=NOTIFICATION_DB):
            result = subprocess.run([str(pinned["service_binary"]), flag], env={**cluster.env, "NOTIFICATION_DATABASE_URL": cluster.url(database)}, cwd=ROOT, capture_output=True, timeout=30)
            require(all(s.encode() not in result.stdout + result.stderr for s in (cluster.password, cluster.url(database))), "cli-output-redaction")
            report["commands"][label] = result.returncode
            require((result.returncode == 0) == success, label)
        cli("empty-ledger-refused", "--verify-migrations", False)
        cli("runtime-migrate", "--migrate")
        cli("runtime-verify", "--verify-migrations")
        cli("runtime-reapply", "--migrate")
        manifest = migration.migration_manifest(ROOT / "backend", "notification-service")
        report["migration_ledger_rows"] = migration.check_ledger(cluster, NOTIFICATION_DB, manifest)
        failure_db = NOTIFICATION_DB + "_failure"
        cluster.sql("postgres", f"create database {failure_db} template template0;")
        cluster.sql(failure_db, "create table concert_notification_items(fixture_only boolean);")
        cli("0005-collision-refused", "--migrate", False, failure_db)
        require(cluster.sql(failure_db, "select count(*) from pg_tables where schemaname='public' and tablename='concert_notification_deliveries';") == "0", "0005-transactional-ddl-rollback")
        require(cluster.sql(failure_db, "select count(*) from _sqlx_migrations where version=5;") == "0", "0005-no-false-ledger")
        cluster.sql(failure_db, "drop table concert_notification_items;")
        cli("0005-forward-recovery", "--migrate", True, failure_db)
        cli("0005-recovered-verify", "--verify-migrations", True, failure_db)
        require(migration.check_ledger(cluster, failure_db, manifest) == 6, "recovered-ledger")
        original = cluster.sql(NOTIFICATION_DB, "select encode(checksum,'hex') from _sqlx_migrations where version=5;")
        cluster.sql(NOTIFICATION_DB, "update _sqlx_migrations set checksum=decode(repeat('00',48),'hex') where version=5;")
        cli("checksum-drift-refused", "--verify-migrations", False)
        cluster.sql(NOTIFICATION_DB, f"update _sqlx_migrations set checksum=decode('{original}','hex') where version=5;")
        report["runtime_migrations"] = "PASS"
    else:
        for path in n_paths:
            cluster.sql(NOTIFICATION_DB, "begin;\n" + path.read_text() + "\ncommit;")
        report["runtime_migrations"] = "NOT_RUN"
    for path in c_paths:
        cluster.sql(CONCERT_DB, "begin;\n" + path.read_text() + "\ncommit;")
    cluster.sql(NOTIFICATION_DB, """
        create table be33_fixture_authority(event_id uuid,user_id uuid,allowed boolean not null,primary key(event_id,user_id));
        create table be33_fixture_targets(event_id uuid,target_id uuid,visible boolean not null,stale boolean not null,primary key(event_id,target_id));
        create table be33_fixture_delay(actor_id uuid primary key);
        create function be33_fixture_delay_receipt() returns trigger language plpgsql as $$ begin
          if exists(select 1 from be33_fixture_delay where actor_id=NEW.actor_id) then perform pg_sleep(0.15); end if;
          return NEW;
        end $$;
        create trigger be33_fixture_delay before insert on concert_notification_read_receipts for each row execute function be33_fixture_delay_receipt();
        create table be33_fixture_ingress_failure(user_id uuid primary key);
        create function be33_fixture_fail_ingress() returns trigger language plpgsql as $$ begin
          if exists(select 1 from be33_fixture_ingress_failure where user_id=NEW.user_id) then raise exception 'fixture ingress failure'; end if;
          return NEW;
        end $$;
        create trigger be33_fixture_fail before insert on notifications for each row execute function be33_fixture_fail_ingress();
        create table be33_fixture_subject_gate(subject_id uuid primary key,gate_key bigint not null);
        create function be33_fixture_gate_subject() returns trigger language plpgsql as $$ declare gate bigint; begin
          select gate_key into gate from be33_fixture_subject_gate where subject_id=NEW.subject_id;
          if gate is not null then perform pg_advisory_xact_lock(gate); end if;
          return NEW;
        end $$;
        create trigger be33_fixture_gate_subject after insert on notification_subject_states for each row execute function be33_fixture_gate_subject();
    """)
    n_source = ROOT / "backend/services/notification-service/tests/concert_inbox.rs"
    c_source = ROOT / "backend/services/concert-service/tests/notification.rs"
    tests(cluster, pinned["notification_test"], n_source, NOTIFICATION_DB, report, "notification_tests")
    tests(cluster, pinned["concert_test"], c_source, CONCERT_DB, report, "concert_tests")
    before = cluster.sql(NOTIFICATION_DB, "select md5(string_agg(row_to_json(t)::text,',' order by id)) from notifications t;")
    postmaster = cluster.assert_postmaster()
    # Exercise WAL crash recovery, not only orderly shutdown. Every process action
    # remains guarded by the same owned root/token/inode/postmaster identity.
    migration.execute([str(cluster.tools["pg_ctl"]), "-D", str(cluster.data), "-w", "-t", "20", "-m", "immediate", "stop"], env=cluster.env, phase="owned-crash-stop", timeout=30, cwd=cluster.root)
    require(not (cluster.data / "postmaster.pid").exists(), "crashed-postmaster-remains")
    cluster.started = False
    cluster.start()
    require(cluster.assert_postmaster() != postmaster, "new-postmaster-required")
    require(cluster.sql(NOTIFICATION_DB, "select md5(string_agg(row_to_json(t)::text,',' order by id)) from notifications t;") == before, "durable-rows-after-restart")
    tests(cluster, pinned["notification_test"], n_source, NOTIFICATION_DB, report, "restart_receipt_replay", exact="durable_restart_replays_exact_delivery_and_read_all_receipt")
    if "service_binary" in pinned:
        cli("runtime-verify-after-restart", "--verify-migrations")
        require(migration.check_ledger(cluster, NOTIFICATION_DB, manifest) == 6, "ledger-after-restart")
    report["restart_persistence"] = "PASS"
    report["restart_mode"] = "owned-pg-ctl-immediate-WAL-recovery"
    report["result"] = "PASS"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("notification-test", "concert-test", "service-binary"):
        parser.add_argument("--" + name, type=Path)
        parser.add_argument("--" + name + "-sha256")
    parser.add_argument("--pg-bin", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    require(args.pg_bin.is_absolute() and args.report.is_absolute(), "absolute-paths-required")
    fd = os.open(args.report, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    cluster = None
    report = {"scope": "BE33 durable Notification owner and BE15 sink", "result": "FAIL", "commands": {}, "binary_sha256": {},
              "canonical_auth_target_authority": "NOT_RUN", "public_http_rpc": "NOT_RUN", "release_acceptance": "NOT_RUN"}
    previous = {}
    def interrupted(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.signal(sig, interrupted)
        report["source_revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        report["dirty_files"] = subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).splitlines()
        prefixes = ("backend/crates/concert-domain/src/notification", "backend/crates/concert-application/src/notification", "backend/services/notification-service/src/concert", "backend/services/concert-service/src/adapters/out/notification", "backend/services/notification-service/tests/concert_inbox_support")
        paths = [p for prefix in prefixes for p in (ROOT / prefix).rglob("*.rs")]
        paths += [ROOT / p for p in ("backend/services/notification-service/tests/concert_inbox.rs", "backend/services/concert-service/tests/notification.rs", "backend/scripts/mcr/notification_smoke.py", "backend/scripts/mcr/migration_smoke.py", "backend/Cargo.lock")]
        report["source_sha256"] = {str(p.relative_to(ROOT)): digest(p) for p in paths}
        tools = {name: migration.executable(args.pg_bin / name) for name in migration.PG_TOOLS}
        cluster = migration.Cluster(tools)
        run(cluster, args, report)
    except (Exception, KeyboardInterrupt) as error:
        # Never serialize subprocess/connection exceptions containing credentials.
        report["failure"] = str(error) if type(error) is RuntimeError else type(error).__name__
    finally:
        try:
            if cluster:
                cluster.cleanup()
                require(not cluster.root.exists(), "owned-directory-remains")
            report["cleanup"] = "PASS"
        except BaseException:
            report.update(result="FAIL", cleanup="FAIL")
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        with os.fdopen(fd, "w") as out:
            json.dump(report, out, indent=2)
            out.write("\n")
    print(json.dumps({key: report.get(key) for key in ("result", "failure", "cleanup", "restart_persistence")}))
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
