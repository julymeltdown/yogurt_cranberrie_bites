#!/usr/bin/env python3
"""WP-08: private PostgreSQL 16 cluster for the profile-avatar tests.

This is the `backend/scripts/mcr/migration_smoke.py` harness pattern reused
verbatim: a marker-owned `mcr-pg-*` cluster in `/tmp`, no TCP listener, no
inherited credentials. It creates one database, applies the concert-service
migrations in order, and prints the connection URL so a test process can use it
through `CONCERT_TEST_DATABASE_URL`.

Usage:
    python3 backend/scripts/mcr/wp08_avatar_cluster.py --report /abs/out.json
    python3 backend/scripts/mcr/wp08_avatar_cluster.py --report /abs/out.json --run-cmd '...'

With `--run-cmd` the cluster stays up while the command runs and is removed
afterwards, so a `cargo test` invocation can be wrapped in one call.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
import migration_smoke as migration  # noqa: E402

DATABASE = "mcr_avatar"


def migrations() -> list[Path]:
    directory = ROOT / "backend/services/concert-service/migrations"
    files = [
        path
        for path in sorted(directory.glob("*.sql"))
        if not path.name.endswith(".down.sql")
    ]
    if not files:
        raise SystemExit("empty concert-service migration set")
    numbers = [int(path.name.split("_", 1)[0]) for path in files]
    if len(numbers) != len(set(numbers)):
        raise SystemExit("duplicate concert-service migration number")
    return files


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--pg-bin-dir", type=Path, default=Path("/opt/homebrew/bin"))
    parser.add_argument("--run-cmd", default=None)
    parser.add_argument("--keep", action="store_true")
    args = parser.parse_args(argv)
    if not args.report.is_absolute():
        parser.error("--report must be an absolute new file path")
    tools = {
        name: migration.executable(args.pg_bin_dir / name) for name in migration.PG_TOOLS
    }
    report: dict = {
        "scope": "WP-08 profile avatar private PostgreSQL cluster",
        "result": "FAIL",
        "cleanup": "NOT_RUN",
        "started_at_utc": datetime.now(timezone.utc).isoformat(),
    }
    cluster = migration.Cluster(tools)
    exit_code = 1
    try:
        cluster.init()
        cluster.start()
        report["postgres_version"] = cluster.sql("postgres", "show server_version;")
        report["cluster_root"] = str(cluster.root)
        migration.execute(
            [
                str(cluster.tools["createdb"]),
                "--no-password",
                "--maintenance-db=postgres",
                DATABASE,
            ],
            env=cluster.client_env("postgres"),
            phase="createdb",
            cwd=cluster.root,
        )
        applied: list[int] = []
        for path in migrations():
            number = int(path.name.split("_", 1)[0])
            cluster.sql(DATABASE, f"begin; {path.read_text()} commit;")
            applied.append(number)
        report["migrations_applied"] = applied
        url = cluster.url(DATABASE)
        report["database_url"] = url
        report["result"] = "PASS"
        if args.run_cmd:
            env = {**os.environ, "CONCERT_TEST_DATABASE_URL": url}
            completed = subprocess.run(
                args.run_cmd, shell=True, env=env, cwd=str(ROOT / "backend")
            )
            report["run_cmd"] = args.run_cmd
            report["run_cmd_exit_code"] = completed.returncode
            exit_code = completed.returncode
        else:
            exit_code = 0
            print(url)
    except migration.SmokeError as error:
        report["failure"] = error.detail
        exit_code = 1
    except BaseException as error:  # noqa: BLE001 - the report must always be written
        report["failure"] = {"phase": "harness", "diagnostic": type(error).__name__}
        exit_code = 1
    finally:
        if not args.keep:
            try:
                cluster.cleanup()
                report["cleanup"] = "PASS"
            except BaseException as error:  # noqa: BLE001
                report["cleanup"] = "FAIL"
                report["cleanup_failure"] = type(error).__name__
                report["result"] = "FAIL"
                exit_code = 1
        else:
            report["cleanup"] = "KEPT"
        report["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"result": report["result"], "cleanup": report["cleanup"]}))
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
