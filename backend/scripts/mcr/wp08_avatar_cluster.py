#!/usr/bin/env python3
"""WP-08: private PostgreSQL 16 cluster for the profile-avatar tests.

The mechanics live in `wp_cluster_harness.py`: a marker-owned `mcr-pg-*` cluster
in `/tmp`, no TCP listener, no inherited credentials, one database with the
concert-service migrations applied in order, and the connection URL printed so a
test process can use it through `CONCERT_TEST_DATABASE_URL`.

Usage:
    python3 backend/scripts/mcr/wp08_avatar_cluster.py --report /abs/out.json
    python3 backend/scripts/mcr/wp08_avatar_cluster.py --report /abs/out.json --run-cmd '...'

With `--run-cmd` the cluster stays up while the command runs and is removed
afterwards, so a `cargo test` invocation can be wrapped in one call.
"""
from __future__ import annotations

import argparse
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
import wp_cluster_harness as harness  # noqa: E402

DATABASE = "mcr_avatar"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--pg-bin-dir", type=Path, default=harness.LEGACY_PG_BIN_DIR)
    parser.add_argument("--run-cmd", default=None)
    parser.add_argument("--keep", action="store_true")
    args = parser.parse_args(argv)
    if not args.report.is_absolute():
        parser.error("--report must be an absolute new file path")

    def body(_cluster, url: str, report: dict) -> int:
        if args.run_cmd:
            exit_code = harness.run_command(args.run_cmd, url)
            report["run_cmd"] = args.run_cmd
            report["run_cmd_exit_code"] = exit_code
            return exit_code
        print(url)
        return 0

    return harness.cluster_main(
        scope="WP-08 profile avatar private PostgreSQL cluster",
        database=DATABASE,
        service="concert-service",
        report_path=args.report,
        pg_bin_dir=args.pg_bin_dir,
        body=body,
        keep=args.keep,
    )


if __name__ == "__main__":
    raise SystemExit(main())
