#!/usr/bin/env python3
"""WP-07: private PostgreSQL 16 cluster for the personal-room tests.

This is the `backend/scripts/mcr/migration_smoke.py` harness pattern reused
verbatim (the same shape as `wp08_avatar_cluster.py`): a marker-owned `mcr-pg-*`
cluster in `/tmp`, no TCP listener, no inherited credentials. It creates one
database, applies the concert-service migrations in order, and prints the
connection URL so a test process can use it through
`CONCERT_TEST_DATABASE_URL`.

Usage:
    python3 backend/scripts/mcr/wp07_room_cluster.py --report /abs/out.json
    python3 backend/scripts/mcr/wp07_room_cluster.py --report /abs/out.json --run-cmd '...'
    python3 backend/scripts/mcr/wp07_room_cluster.py --report /abs/out.json \
        --run-cmd '...' --run-cmd '...'

With `--run-cmd` the cluster stays up while every command runs and is removed
afterwards, so several `cargo test` invocations can be wrapped in one call.
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

DATABASE = "mcr_room"


def migrations() -> list[Path]:
    """Every forward concert-service migration, in file-name order.

    WP-03 and WP-08 were authored in parallel worktrees against the same 0085
    head, so both produced an ``0086``. The integration renumbered the avatar
    migration to ``0090_profile_avatars.sql``: neither file had been applied to
    any real database, so no applied migration was edited. The ledger check that
    owns numbering is ``wp02_migration_ledger.py``.
    """
    directory = ROOT / "backend/services/concert-service/migrations"
    files = [
        path
        for path in sorted(directory.glob("*.sql"))
        if not path.name.endswith(".down.sql")
    ]
    if not files:
        raise SystemExit("empty concert-service migration set")
    return files


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--pg-bin-dir", type=Path, default=Path("/opt/homebrew/bin"))
    parser.add_argument("--run-cmd", action="append", default=[])
    parser.add_argument("--keep", action="store_true")
    args = parser.parse_args(argv)
    if not args.report.is_absolute():
        parser.error("--report must be an absolute new file path")
    tools = {
        name: migration.executable(args.pg_bin_dir / name) for name in migration.PG_TOOLS
    }
    report: dict = {
        "scope": "WP-07 personal room private PostgreSQL cluster",
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
        runs: list[dict] = []
        for command in args.run_cmd:
            env = {**os.environ, "CONCERT_TEST_DATABASE_URL": url}
            completed = subprocess.run(
                command, shell=True, env=env, cwd=str(ROOT / "backend")
            )
            runs.append({"command": command, "exit_code": completed.returncode})
            if completed.returncode != 0:
                exit_code = completed.returncode
        if runs:
            report["runs"] = runs
            exit_code = next(
                (run["exit_code"] for run in runs if run["exit_code"] != 0), 0
            )
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
