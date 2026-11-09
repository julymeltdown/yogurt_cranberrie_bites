#!/usr/bin/env python3
"""WP-05: private PostgreSQL 16 cluster for the intro post tests.

The mechanics live in `wp_cluster_harness.py`: a marker-owned `mcr-pg-*` cluster
in `/tmp`, no TCP listener, no inherited credentials, one database with the
concert-service migrations applied in order, and the requested test binaries run
with `CONCERT_TEST_DATABASE_URL` set.

Usage:
    python3 backend/scripts/mcr/wp05_intro_cluster.py --report /abs/out.json \\
        --test intro_post_commands --test intro_snapshot
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
import wp_cluster_harness as harness  # noqa: E402

DATABASE = "mcr_intro_posts"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    # `--report` is optional here: the script is also used interactively, where a
    # report is not wanted. When it is given it must be an absolute path.
    parser.add_argument("--report", type=Path, default=None)
    parser.add_argument("--pg-bin-dir", type=Path, default=harness.DEFAULT_PG_BIN_DIR)
    parser.add_argument("--test", action="append", default=[])
    parser.add_argument("--keep", action="store_true")
    args = parser.parse_args(argv)
    if args.report is not None and not args.report.is_absolute():
        parser.error("--report must be an absolute new file path")
    # The harness always writes a report; an interactive run writes it to a
    # temporary file that is removed on exit.
    report_path = args.report
    temporary = None
    if report_path is None:
        handle, name = tempfile.mkstemp(prefix="mcr-wp05-", suffix=".json")
        os.close(handle)
        temporary = Path(name)
        report_path = temporary

    def body(_cluster, url: str, report: dict) -> int:
        failures = 0
        for test in args.test:
            command = ["cargo", "+1.88.0", "test", "--locked", "-p", "concert-service",
                       "--test", test, "--", "--test-threads=1"]
            print(f"$ {' '.join(command)}", flush=True)
            completed = subprocess.run(
                command, env={**os.environ, "CONCERT_TEST_DATABASE_URL": url},
                cwd=str(ROOT / "backend"))
            print(f"{test}: exit {completed.returncode}", flush=True)
            report.setdefault("tests", []).append(
                {"test": test, "exit_code": completed.returncode})
            failures += completed.returncode != 0
        return 0 if failures == 0 else 1

    try:
        return harness.cluster_main(
            scope="WP-05 intro post private PostgreSQL cluster",
            database=DATABASE,
            service="concert-service",
            report_path=report_path,
            pg_bin_dir=args.pg_bin_dir,
            body=body,
            keep=args.keep,
        )
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


if __name__ == "__main__":
    raise SystemExit(main())
