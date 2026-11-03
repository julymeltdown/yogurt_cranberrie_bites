#!/usr/bin/env python3
"""Extend retained Auth native qualification with sealed projection contexts."""
import importlib.util
from pathlib import Path
import re
import subprocess

ROOT = next(p for p in Path(__file__).resolve().parents if (p / "backend/Cargo.lock").is_file())
BASE = ROOT / "backend/services/concert-service/tests/world_auth_read/native_smoke.py"
spec = importlib.util.spec_from_file_location("world_auth_native_smoke", BASE)
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)
original_exercise = base.exercise
PREFIX = "adapters::out::relationship_authority::world_auth::tests::native::projection::native_"


def exercise(cluster, pinned, report):
    report["scope"] = "Sealed World projection context plus retained signed Auth native regressions"
    report["source_sha256"][str(Path(__file__).relative_to(ROOT))] = base.digest(Path(__file__))
    listing = base.command(report, "list-context-native", [pinned["test"], "--list"], cluster.env, cluster.root)
    names = re.findall(r"^(" + re.escape(PREFIX) + r"[a-z0-9_]+): test$", listing.stdout.decode(), re.MULTILINE)
    base.require(len(names) == 1, "exact-context-native-test-required")
    base.command(report, "context-missing-owner-refused", [pinned["test"], "--ignored", "--exact", names[0]], cluster.env, cluster.root, success=False)
    original_exercise(cluster, pinned, report)
    urls = {owner: cluster.url(db) for owner, db in base.DATABASES.items()}
    secrets = (cluster.password, cluster.token, *urls.values())
    env = {**cluster.env, "MCR_WORLD_AUTH_ROOT": str(cluster.root), "MCR_WORLD_AUTH_TOKEN": cluster.token,
           "MCR_WORLD_AUTH_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
           **{f"MCR_WORLD_AUTH_{owner.upper()}_URL": url for owner, url in urls.items()}}
    result = subprocess.run([str(pinned["test"]), PREFIX, "--ignored", "--test-threads=1"], env=env, cwd=cluster.root, capture_output=True, timeout=30)
    report["commands"]["native-projection-context"] = result.returncode
    raw = result.stdout + result.stderr
    base.require(all(value.encode() not in raw for value in secrets if value), "context-output-redaction")
    report["context_native_diagnostics"] = base.migration.redact(raw.decode(errors="replace"), secrets)
    base.require(result.returncode == 0, "native-projection-context:exit")
    base.summary(result.stdout, 1)
    state_sql = "select (select count(*) from concert_world_tickets where consumed_at is not null)::text||':'||(select count(*) from concert_world_memberships)::text;"
    db = base.DATABASES["concert"]
    before = cluster.sql(db, state_sql)
    base.require(before == "2:2", "context-consume-membership-required")
    old = cluster.assert_postmaster()
    cluster.stop()
    cluster.start()
    base.require(cluster.assert_postmaster() != old, "context-new-postmaster-required")
    base.require(cluster.sql(db, state_sql) == before, "context-restart-state-changed")
    for owner, db in base.DATABASES.items():
        base.migration.check_ledger(cluster, db, report["migration_sha384"][owner])
        base.command(report, owner + "-verify-after-context-restart", [pinned[owner + "-bin"], "--verify-migrations"],
                     {**cluster.env, owner.upper()+"_DATABASE_URL": urls[owner]}, cluster.root, secrets=secrets)
    report.update(scope="Sealed World projection context plus retained signed Auth native regressions",
                  context_native_test_names=names, context_native_tests=1, context_restart_persistence="PASS", result="PASS")
    report["source_sha256"][str(Path(__file__).relative_to(ROOT))] = base.digest(Path(__file__))


if __name__ == "__main__":
    base.exercise = exercise
    raise SystemExit(base.main())
