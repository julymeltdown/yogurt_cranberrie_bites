#!/usr/bin/env python3
"""Run owned BE16–18 native SQL tests, restart/replay and marker-validated cleanup."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import re
import migration_smoke as migration

ROOT = Path(__file__).resolve().parents[3]
DATABASE = "mcr_community"

def require(value, label):
    if not value:
        raise RuntimeError(label)

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def run(args, report):
    cluster = None
    try:
        binary = migration.executable(args.repository_test)
        require(binary.is_absolute(), "explicit native binary required")
        tools = {n: migration.executable(args.pg_bin / n) for n in migration.PG_TOOLS}
        cluster = migration.Cluster(tools)
        report["owned_root"] = str(cluster.root)
        cluster.init()
        cluster.start()
        report["postgres_version"] = cluster.sql("postgres", "show server_version;")
        cluster.sql("postgres", f"create database {DATABASE} template template0;")
        # Deliberate isolated slice: no unrelated migration owners' files are needed.
        migrations=["0001_concert_core.sql","0002_command_receipts_outbox.sql"]
        if args.policy:
            migrations.append("0006_relationship_policy.sql")
        if not args.maintenance:
            migrations.append("0007_community.sql")
        migrations.append("0009_receipt_domain_failures.sql")
        if args.maintenance or args.giveaway or args.policy:
            migrations.append("0011_outbox_maintenance.sql")
        for name in migrations:
            path = ROOT / "backend/services/concert-service/migrations" / name
            cluster.sql(DATABASE, path.read_text())
        cluster.sql(DATABASE, "create table community_test_authority(event uuid,actor uuid,allowed boolean not null,primary key(event,actor)); create table community_test_places(event uuid,place uuid,allowed boolean not null,primary key(event,place));")
        env = {**cluster.env, "CONCERT_TEST_DATABASE_URL": cluster.url(DATABASE),
               "CONCERT_TEST_OWNER_ROOT": str(cluster.root),
               "CONCERT_TEST_OWNER_TOKEN": cluster.token,
               "CONCERT_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}"}
        def native(extra):
            result = subprocess.run([str(binary), "--ignored", "--test-threads=1", *extra], cwd=ROOT, env=env, capture_output=True, text=True, timeout=120)
            report.setdefault("native_runs", []).append({"arguments": extra, "exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr})
            require(result.returncode == 0, "native tests failed")
            match=re.search(r"test result: ok\. (\d+) passed",result.stdout)
            require(match is not None and int(match[1])>0,"native executable ran no tests")
            if "--exact" in extra:
                require(int(match[1])==1,"exact restart probe count")
        report["binary_sha256"] = digest(binary)
        native([])
        query = "select (select count(*) from " + ("concert_maintenance_invalidations" if args.maintenance else ("concert_community_giveaways" if args.giveaway else "concert_community_parties")) + ")||':'||(select count(*) from concert_command_receipts)||':'||(select count(*) from concert_outbox);"
        before = cluster.sql(DATABASE, query)
        pid = cluster.assert_postmaster()
        cluster.stop()
        cluster.start()
        require(cluster.assert_postmaster() != pid, "restart identity")
        require(cluster.sql(DATABASE, query) == before, "durable rows after restart")
        native(["--exact", "maintenance_restart_dedupe" if args.maintenance else ("community_gift_restart_expiry" if args.giveaway else ("community_policy_restart_receipt" if args.policy else "community_restart_replay"))])
        after = cluster.sql(DATABASE, query)
        if args.giveaway:
            old, new = list(map(int, before.split(":"))), list(map(int, after.split(":")))
            require(new[:2] == old[:2] and new[2] == old[2] + 1, "restart expiry has exactly one atomic maintenance invalidation")
            report["restart_expiry"] = "PASS"
        else:
            require(after == before, "restart replay did not repeat effects")
        report["durable_rows_before_restart"] = before
        report["durable_rows_after_restart"] = after
        report["restart_replay"] = "PASS"
        report["result"] = "PASS"
    except BaseException as error:
        report["result"] = "FAIL"
        report["failure"] = type(error).__name__ + ": " + (str(error) if isinstance(error, RuntimeError) else getattr(error, "detail", "native fixture failure"))
    finally:
        try:
            if cluster is not None:
                root = cluster.root
                cluster.cleanup()
                require(not root.exists(), "owned root remains")
            report["cleanup"] = "PASS"
        except BaseException:
            report["cleanup"] = "FAIL"
            report["result"] = "FAIL"

def migrations_for_evidence():
    return ["0001_concert_core.sql","0002_command_receipts_outbox.sql","0006_relationship_policy.sql","0007_community.sql","0009_receipt_domain_failures.sql","0011_outbox_maintenance.sql"]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository-test", type=Path, required=True)
    parser.add_argument("--pg-bin", type=Path, default=Path("/opt/homebrew/opt/postgresql@16/bin"))
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--policy", action="store_true", help="qualify concrete shared BE37 adapter")
    parser.add_argument("--giveaway", action="store_true", help="qualify BE20–23 gift slice")
    parser.add_argument("--maintenance", action="store_true", help="qualify delegated BE15 maintenance queue extension")
    args = parser.parse_args()
    require(args.report.is_absolute(), "absolute evidence file required")
    descriptor = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    source_paths = sorted(set([*ROOT.glob("backend/**/community/*.rs"), *ROOT.glob("backend/**/tests/community*.rs"), *ROOT.glob("backend/crates/concert-*/src/relationship/*.rs"), *ROOT.glob("backend/services/concert-service/src/relationship/*.rs"), *[ROOT / "backend/services/concert-service/migrations" / n for n in migrations_for_evidence()], *[ROOT / p for p in ["backend/crates/concert-domain/src/lib.rs", "backend/crates/concert-application/src/lib.rs", "backend/crates/concert-application/src/commands/mod.rs", "backend/crates/concert-application/src/commands/maintenance.rs", "backend/services/concert-service/src/lib.rs", "backend/services/concert-service/src/adapters/out/repository/outbox.rs", "backend/services/concert-service/src/adapters/out/repository/outbox_maintenance.rs", "backend/services/concert-service/tests/maintenance_outbox.rs", "backend/services/concert-service/tests/support/community/owned.rs"]], Path(__file__).resolve()]))
    pins = [ROOT / "contracts/domain/v1" / n for n in ("party.schema.json", "giveaway.schema.json", "operations.json", "policy.json")]
    report = {"scope": "BE15 maintenance origin" if args.maintenance else ("BE37-backed community adapter" if args.policy else ("BE20–23 internal gift transactions" if args.giveaway else "BE16–18 internal transaction slice")), "result": "NOT_RUN", "public_routes": "NOT_RUN", "real_policy": "NOT_RUN",
              "source_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "source_sha256": {str(p.relative_to(ROOT)): digest(p) for p in source_paths}, "contract_sha256": {str(p.relative_to(ROOT)): digest(p) for p in pins}}
    try:
        run(args, report)
        with os.fdopen(descriptor, "w") as stream:
            json.dump(report, stream, indent=2)
            stream.write("\n")
    finally:
        print(json.dumps({k: report.get(k) for k in ("result", "cleanup", "restart_replay", "failure")}))
    return 0 if report["result"] == "PASS" and report.get("cleanup") == "PASS" else 1

if __name__ == "__main__":
    raise SystemExit(main())
