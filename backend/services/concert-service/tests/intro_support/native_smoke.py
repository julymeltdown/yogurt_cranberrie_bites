#!/usr/bin/env python3
"""Owned marker/socket PostgreSQL BE27 smoke with restart and cleanup."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
import migration_smoke as migration


DATABASE = "mcr_intro_be"
MIGRATIONS = (
    "0001_concert_core.sql",
    "0002_command_receipts_outbox.sql",
    "0006_relationship_policy.sql",
    "0008_meeting_beacons.sql",
    "intro_schema_v1.sql",
)


def apply(cluster: migration.Cluster) -> None:
    for name in MIGRATIONS:
        base = (
            ROOT / "backend/services/concert-service/src/adapters/out/repository"
            if name == "intro_schema_v1.sql"
            else ROOT / "backend/services/concert-service/migrations"
        )
        sql = (base / name).read_text()
        cluster.sql(DATABASE, sql)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args(argv)
    if not args.binary.is_absolute() or not args.report.is_absolute():
        parser.error("binary/report paths must be absolute")
    tools = {
        name: migration.executable(Path("/opt/homebrew/bin") / name)
        for name in migration.PG_TOOLS
    }
    report: dict[str, object] = {"result": "FAIL", "cleanup": "NOT_RUN"}
    cluster = migration.Cluster(tools)
    # Pretty output preserves long values; automatic JSON conversion silently
    # stringifies result rows that contain random UUIDs as long numbers.
    cluster.sql = lambda database, query: migration.execute(
        [str(cluster.tools["psql"]), "-X", "--no-password", "-A", "-t",
         "-v", "ON_ERROR_STOP=1", "-c", query],
        env=cluster.client_env(database), phase="sql", cwd=cluster.root,
    ).stdout.strip()
    try:
        report["owned_directory"] = str(cluster.root)
        report["binary_sha256"] = hashlib.sha256(args.binary.read_bytes()).hexdigest()
        cluster.init()
        cluster.start()
        report["postgres_version"] = cluster.sql("postgres", "show server_version;")
        cluster.sql("postgres", f"create database {DATABASE} template template0;")
        apply(cluster)
        environment = {
            **cluster.env,
            "CONCERT_TEST_DATABASE_URL": cluster.url(DATABASE),
            "CONCERT_TEST_OWNER_ROOT": str(cluster.root),
            "CONCERT_TEST_OWNER_TOKEN": cluster.token,
            "CONCERT_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
        }
        result = subprocess.run(
            [str(args.binary), "--test-threads=1"],
            env=environment,
            cwd=cluster.root,
            capture_output=True,
            text=True,
            timeout=180,
        )
        combined = result.stdout + result.stderr
        report["test_exit"] = result.returncode
        report["test_output_sha256"] = hashlib.sha256(combined.encode()).hexdigest()
        if result.returncode:
            report["tail"] = combined[-12000:]
            raise RuntimeError("intro test failed")
        if cluster.password in combined or cluster.url(DATABASE) in combined:
            raise RuntimeError("secret leakage")
        row = cluster.sql(
            DATABASE,
            "select state,revision from concert_intros order by created_at,intro_id limit 1;",
        )
        report["persisted_row"] = row
        before = cluster.assert_postmaster()
        cluster.stop()
        cluster.start()
        report["restarted_pid_differs"] = before != cluster.assert_postmaster()
        report["restart_row"] = cluster.sql(
            DATABASE,
            "select state,revision from concert_intros order by created_at,intro_id limit 1;",
        )
        report["result"] = "PASS"
    except Exception as error:
        report["failure"] = getattr(error, "detail", type(error).__name__)
    finally:
        try:
            cluster.cleanup()
            report["cleanup"] = "PASS"
        except BaseException:
            report["cleanup"] = "FAIL"
            report["result"] = "FAIL"
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({k: report.get(k) for k in ("result", "cleanup", "test_exit")}))
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
