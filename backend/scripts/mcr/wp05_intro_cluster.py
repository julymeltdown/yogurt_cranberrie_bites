#!/usr/bin/env python3
"""WP-05: private PostgreSQL 16 cluster for the intro post tests.

This is the `migration_smoke.py` harness pattern reused from
`wp06_profile_posts_cluster.py`: a marker-owned `mcr-pg-*` cluster in `/tmp`, no
TCP listener, no inherited credentials. It creates one database, applies the
concert-service migrations in order, and runs the requested test binaries with
`CONCERT_TEST_DATABASE_URL` set.

Usage:
    python3 backend/scripts/mcr/wp05_intro_cluster.py --report /abs/out.json \
        --test intro_post_commands --test intro_snapshot
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import sys
import traceback

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
import migration_smoke as migration  # noqa: E402

DATABASE = "mcr_intro_posts"


def migrations() -> list[Path]:
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
    parser.add_argument("--report", type=Path, default=None)
    parser.add_argument("--pg-bin-dir", type=Path, default=Path("/opt/homebrew/opt/postgresql@16/bin"))
    parser.add_argument("--test", action="append", default=[])
    parser.add_argument("--keep", action="store_true")
    args = parser.parse_args(argv)
    tools = {
        name: migration.executable(args.pg_bin_dir / name) for name in migration.PG_TOOLS
    }
    report: dict = {
        "scope": "WP-05 intro post private PostgreSQL cluster",
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
        migration.execute(
            [str(cluster.tools["createdb"]), "--no-password",
             "--maintenance-db=postgres", DATABASE],
            env=cluster.client_env("postgres"), phase="createdb", cwd=cluster.root,
        )
        applied: list[str] = []
        for path in migrations():
            cluster.sql(DATABASE, f"begin; {path.read_text()} commit;")
            applied.append(path.name)
        report["migrations_applied"] = applied
        url = cluster.url(DATABASE)
        report["database_url"] = url
        report["result"] = "PASS"
        failures = 0
        for test in args.test:
            env = {**os.environ, "CONCERT_TEST_DATABASE_URL": url}
            command = ["cargo", "+1.88.0", "test", "--locked", "-p", "concert-service",
                       "--test", test, "--", "--test-threads=1"]
            print(f"$ {' '.join(command)}", flush=True)
            completed = subprocess.run(command, env=env, cwd=str(ROOT / "backend"))
            print(f"{test}: exit {completed.returncode}", flush=True)
            report.setdefault("tests", []).append({"test": test, "exit_code": completed.returncode})
            failures += completed.returncode != 0
        exit_code = 0 if failures == 0 else 1
    except migration.SmokeError as error:
        report["failure"] = error.detail
        exit_code = 1
    except BaseException as error:  # noqa: BLE001 - the report must always be written
        report["failure"] = {"phase": "harness", "diagnostic": type(error).__name__}
        traceback.print_exc()
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
        if args.report:
            args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"result": report["result"], "cleanup": report["cleanup"]}))
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
