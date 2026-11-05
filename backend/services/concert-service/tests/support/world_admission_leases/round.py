#!/usr/bin/env python3
"""Run two frozen BE11 regressions in a fresh marker-owned PostgreSQL cluster.

Adapted from the BE06 deletion_round.py marker/migration-ledger protocol at
1289d90e9fe5bc6fee04858aee829c9e54b030ab. Uses the existing migration_smoke
Cluster; no fabricated schema, credential output, application server startup or installation.
The caller must hold a finite allocation covering every child and retained byte.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import sys
import time

ROOT = Path(__file__).resolve().parents[6]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
import migration_smoke as migration

TESTS = (
    "owner_takeover_does_not_relabel_an_existing_membership",
    "membership_leave_and_disconnect_grace_expire_canonical_rows",
)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-binary", required=True, type=Path)
    parser.add_argument("--test-sha256", required=True)
    parser.add_argument("--service-binary", required=True, type=Path)
    parser.add_argument("--service-sha256", required=True)
    parser.add_argument("--pg-bin", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--work-deadline", required=True, type=float)
    parser.add_argument("--release-deadline", required=True, type=float)
    args = parser.parse_args()
    if not all(p.is_absolute() for p in (args.test_binary, args.service_binary, args.pg_bin, args.report)):
        parser.error("explicit absolute paths required")
    if args.release_deadline - args.work_deadline < 60 or time.time() >= args.work_deadline:
        parser.error("unexpired work deadline and 60-second cleanup reserve required")
    fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    report = {"state": "FAIL", "cleanup": "NOT_RUN", "native_tests": 0,
              "scope": "BE11 owner-generation and membership lifecycle; fixture authority, not public Auth/World flow",
              "work_deadline": args.work_deadline, "release_deadline": args.release_deadline,
              "commands": {}, "test_output": {}}
    cluster = None
    previous = signal.signal(signal.SIGTERM, lambda *_: (_ for _ in ()).throw(KeyboardInterrupt()))

    def timeout():
        remaining = int(args.work_deadline - time.time())
        if remaining < 1:
            raise RuntimeError("original work deadline expired")
        return min(60, remaining)

    try:
        binaries = {"tests": migration.executable(args.test_binary),
                    "service": migration.executable(args.service_binary)}
        if digest(binaries["tests"]) != args.test_sha256 or digest(binaries["service"]) != args.service_sha256:
            raise RuntimeError("frozen binary hash mismatch")
        tools = {name: migration.executable(args.pg_bin / name) for name in migration.PG_TOOLS}
        report["binary_sha256"] = {key: digest(value) for key, value in binaries.items()}
        report["source_commit"] = migration.execute(
            ["git", "-C", str(ROOT), "rev-parse", "HEAD"],
            env={"PATH": "/usr/bin:/bin"}, phase="source-pin", timeout=timeout()).stdout.strip()
        report["source_sha256"] = {name: digest(ROOT / name) for name in (
            "backend/services/concert-service/src/world/mod.rs",
            "backend/services/concert-service/tests/world_admission.rs",
            "backend/crates/concert-application/src/world_ticket/mod.rs",
            "backend/crates/concert-application/tests/world_ticket.rs",
        )}
        report["helper_sha256"] = digest(Path(__file__))
        timeout()
        cluster = migration.Cluster(tools)
        cluster.init()
        timeout()
        cluster.start()
        report["postgres_version"] = cluster.sql("postgres", "show server_version;")
        if not report["postgres_version"].startswith("16.14"):
            raise RuntimeError("PostgreSQL16.14 required")
        cluster.sql("postgres", "create database mcr_concert template template0;")
        manifest = migration.migration_manifest(ROOT / "backend", "concert-service")
        env = {**cluster.env, "CONCERT_DATABASE_URL": cluster.url("mcr_concert")}
        for flag in ("--migrate", "--verify-migrations"):
            result = migration.execute([str(binaries["service"]), flag], env=env,
                                       phase=flag, cwd=cluster.root, timeout=timeout())
            report["commands"][flag] = result.returncode
            migration.check_ledger(cluster, "mcr_concert", manifest)
        # The production CLI requires mcr_concert; the immutable World test binary
        # deliberately admits only mcr_world. Clone the actual migrated database,
        # then prove the same ledger instead of bypassing either database guard.
        cluster.sql("postgres", "create database mcr_world template mcr_concert;")
        migration.check_ledger(cluster, "mcr_world", manifest)
        report["commands"]["clone-world-fixture"] = 0
        report["database_provenance"] = {
            "migrate_verify_database": "mcr_concert", "world_fixture": "mcr_world",
            "world_fixture_template": "mcr_concert", "cloned_ledger_matches": True,
        }
        report["migration_sha384"] = manifest
        env = {**cluster.env, "WORLD_TEST_DATABASE_URL": cluster.url("mcr_world"),
               "WORLD_TEST_OWNER_ROOT": str(cluster.root), "WORLD_TEST_OWNER_TOKEN": cluster.token,
               "WORLD_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}"}
        for name in TESTS:
            result = migration.execute([str(binaries["tests"]), "--exact", name, "--test-threads=1"],
                                       env=env, phase=name, cwd=cluster.root, timeout=timeout())
            if not re.search(r"test result: ok\. 1 passed; 0 failed; 0 ignored;", result.stdout):
                raise RuntimeError("native test missing or skipped")
            if any(secret in result.stdout for secret in (cluster.password, cluster.token, cluster.url("mcr_world"))):
                raise RuntimeError("credential output refused")
            report["commands"][name] = result.returncode
            report["test_output"][name] = result.stdout
            report["native_tests"] += 1
        report["state"] = "PASS"
    except Exception as error:
        report["error"] = error.detail if isinstance(error, migration.SmokeError) else type(error).__name__
    finally:
        try:
            if cluster is not None:
                cluster.cleanup()
                if cluster.root.exists():
                    raise RuntimeError("owned cluster retained")
            report["cleanup"] = "PASS"
        except BaseException:
            report["cleanup"] = "FAIL"
            report["state"] = "FAIL"
        report["released_at"] = time.time()
        report["within_release_deadline"] = report["released_at"] <= args.release_deadline
        if not report["within_release_deadline"]:
            report["state"] = "FAIL"
        with os.fdopen(fd, "w") as stream:
            json.dump(report, stream, indent=2)
            stream.write("\n")
        signal.signal(signal.SIGTERM, previous)
    print(json.dumps({key: report[key] for key in ("state", "native_tests", "cleanup")}))
    return 0 if report["state"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
