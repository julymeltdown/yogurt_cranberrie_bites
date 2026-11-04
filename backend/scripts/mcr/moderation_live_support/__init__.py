"""BE39 private PostgreSQL lifecycle, using retained marker/inode ownership guards.

Provenance: migration_smoke.Cluster at published 980fc4d8, read-only reuse.
No imported service defaults or inherited provider/database credentials.
"""
from __future__ import annotations
import hashlib
import os
from pathlib import Path
import subprocess
import migration_smoke as migration

DATABASE = "mcr_concert_be_moderation"
ROOT = Path(__file__).resolve().parents[4]

def require(condition, phase):
    if not condition:
        raise RuntimeError(phase)

def run(test_binary, target, pg_bin, lease_id, report):
    cluster = None
    try:
        require(bool(lease_id.strip()), "explicit-accepted-lease-required")
        require(all(p.is_absolute() for p in (test_binary, target, pg_bin)), "absolute-paths-required")
        target = target.resolve(strict=True)
        test_binary = migration.executable(test_binary)
        require(test_binary.is_relative_to(target), "binary-outside-accepted-target")
        tools = {name: migration.executable(pg_bin / name) for name in migration.PG_TOOLS}
        report.update(lease_id=lease_id, test_binary_sha256=hashlib.sha256(test_binary.read_bytes()).hexdigest(),
                      scope="synthetic roles/owner behavior; actual isolated PostgreSQL identities and ledger")
        cluster = migration.Cluster(tools)
        report["owned_directory"] = str(cluster.root)
        cluster.init(); cluster.start()
        require(cluster.sql("postgres", "show server_version;").startswith("16."), "PG16-required")
        cluster.sql("postgres", f"create database {DATABASE} template template0;")
        url = cluster.url(DATABASE)
        secrets = (cluster.password, url)
        env = {**cluster.env, "CONCERT_TEST_DATABASE_URL": url,
               "CONCERT_TEST_OWNER_ROOT": str(cluster.root), "CONCERT_TEST_OWNER_TOKEN": cluster.token,
               "CONCERT_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}"}
        def native(label, args, count, ignored=0, success=True):
            cluster.assert_owned(); cluster.assert_postmaster()
            result = subprocess.run([str(test_binary), *args, "--test-threads=1"], env=env,
                                    cwd=cluster.root, capture_output=True, timeout=60)
            output = result.stdout + result.stderr
            require(all(secret.encode() not in output for secret in secrets), label + "-secret-redaction")
            report.setdefault("commands", {})[label] = result.returncode
            report.setdefault("diagnostics", {})[label] = migration.redact(output.decode(errors="replace"), secrets)
            if success:
                require(result.returncode == 0, label + "-exit")
                require(f"test result: ok. {count} passed; 0 failed; {ignored} ignored;".encode() in output, label + "-exact-count")
            else:
                require(result.returncode != 0, label + "-fault-not-observed")
        manifest = migration.migration_manifest(ROOT / "backend", "concert-service")
        baseline = {k:v for k,v in manifest.items() if k != 43}
        require(43 in manifest, "0043-required")
        native("published-baseline", ["--ignored", "--exact", "published_checkpoint"], 1)
        report["baseline_ledger"] = migration.check_ledger(cluster, DATABASE, baseline)
        def published_snapshot():
            return hashlib.sha256(cluster.sql(DATABASE, "select row_to_json(e)::text from concert_events e where event_id='00000000-0000-4000-8000-000000000039';").encode()).hexdigest()
        before = published_snapshot()
        # A late DDL fault must roll back earlier 0043 tables and its SQLx receipt.
        cluster.sql(DATABASE, "create table concert_moderation_audit(be39_owned_fault boolean);")
        native("0043-late-fault", ["--ignored", "--exact", "apply_migrations"], 0, success=False)
        migration.check_ledger(cluster, DATABASE, baseline)
        require(cluster.sql(DATABASE, "select count(*) from pg_tables where schemaname='public' and tablename in ('concert_moderation_commands','concert_moderation_owner_results');") == "0", "late-fault-partial-DDL")
        require(published_snapshot() == before, "baseline-state-changed")
        cluster.sql(DATABASE, "drop table concert_moderation_audit;")
        native("0043-forward", ["--ignored", "--exact", "apply_migrations"], 1)
        report["migration_ledger"] = migration.check_ledger(cluster, DATABASE, manifest)
        report["migration_sha384"] = manifest
        native("0043-reapply", ["--ignored", "--exact", "apply_migrations"], 1)
        native("native-ledger-tests", [], 9, ignored=2)
        require(published_snapshot() == before, "published-state-not-preserved")
        tables = ("concert_moderation_commands", "concert_moderation_owner_results", "concert_moderation_audit", "concert_command_receipts", "concert_outbox", "be39_fixture_owner")
        counts = {t:int(cluster.sql(DATABASE, f"select count(*) from {t};")) for t in tables}
        require(all(counts.values()), "committed-fixtures-required")
        identity_before = cluster.sql(DATABASE, "select md5(string_agg(encode(native_identity,'hex'),',' order by result_id)) from concert_moderation_owner_results where outcome='APPLIED';")
        prior=cluster.assert_postmaster();cluster.stop();cluster.start()
        require(cluster.assert_postmaster() != prior, "new-postmaster-required")
        for table,count in counts.items():
            require(int(cluster.sql(DATABASE,f"select count(*) from {table};")) == count, table + "-restart-persistence")
        require(cluster.sql(DATABASE, "select md5(string_agg(encode(native_identity,'hex'),',' order by result_id)) from concert_moderation_owner_results where outcome='APPLIED';") == identity_before, "native-identity-changed")
        require(published_snapshot() == before, "published-restart-state-changed")
        migration.check_ledger(cluster,DATABASE,manifest)
        report.update(result="PASS", native_tests=9, forward_rollback="PASS", restart_persistence="PASS", restart_counts=counts,
                      actual_cross_owner_live_flow="NOT_RUN", production_role_fence="NOT_RUN", root_http_terminal_flow="NOT_RUN")
    except (Exception, KeyboardInterrupt) as error:
        report.update(result="FAIL", failure=str(error) if isinstance(error,RuntimeError) else type(error).__name__)
        if isinstance(error,migration.SmokeError): report["failure_detail"]=error.detail
    finally:
        if cluster is not None:
            try:
                cluster.cleanup();require(not cluster.root.exists(),"owned-cluster-remains");report["cleanup"]="PASS"
            except (Exception,KeyboardInterrupt) as error:
                report.update(result="FAIL",cleanup="FAIL",cleanup_failure=type(error).__name__)
