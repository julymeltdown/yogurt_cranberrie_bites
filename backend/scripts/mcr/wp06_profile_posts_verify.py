#!/usr/bin/env python3
"""WP-06 T-512: run the whole profile post and media verification in one pass.

This is the package verification runner. It:

1. creates the private PostgreSQL 16 cluster
   (`backend/scripts/mcr/wp06_profile_posts_cluster.py`, the
   `migration_smoke.py` harness pattern);
2. runs every WP-06 backend test target against it;
3. runs the workspace `cargo check`, the gateway media route test, and the
   frontend typecheck and feature tests;
4. writes one JSON evidence document with every command and its exit code.

Usage:
    python3 backend/scripts/mcr/wp06_profile_posts_verify.py \\
        --report docs/mcr/integration/evidence/v07-profile-posts-run.json
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
BACKEND = ROOT / "backend"
FRONTEND = ROOT / "frontend"

CONCERT_TESTS = (
    "profile_media_upload",
    "profile_posts",
    "profile_post_read",
    "profile_post_routes",
    "profile_post_lifecycle",
)


def run(command: str, cwd: Path, env: dict[str, str] | None = None) -> dict:
    """Run one command and return its exit code and a short note."""
    completed = subprocess.run(
        command,
        shell=True,
        cwd=str(cwd),
        env={**os.environ, **(env or {})},
        capture_output=True,
        text=True,
    )
    output = completed.stdout + completed.stderr
    note = ""
    # Vitest prints `Test Files 4 passed (4)` and `Tests 55 passed (55)`; the
    # cargo summary is `test result: ok. N passed; ...`. Both streams are
    # scanned because vitest writes its summary to stderr on some runs.
    for line in output.splitlines():
        stripped = line.strip()
        if stripped.startswith("test result:"):
            note = stripped
        elif stripped.startswith("Tests ") and ("passed" in stripped or "failed" in stripped):
            note = stripped
    return {
        "command": command,
        "cwd": str(cwd.relative_to(ROOT)),
        "exit_code": completed.returncode,
        "note": note,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--pg-bin-dir", type=Path, default=Path("/opt/homebrew/opt/postgresql@16/bin"))
    args = parser.parse_args(argv)
    if not args.report.is_absolute():
        args.report = (ROOT / args.report).resolve()

    cluster_report = Path("/tmp/wp06-verify-cluster.json")
    cluster_report.unlink(missing_ok=True)
    cluster = subprocess.run(
        [
            sys.executable,
            str(BACKEND / "scripts/mcr/wp06_profile_posts_cluster.py"),
            "--report",
            str(cluster_report),
            "--pg-bin-dir",
            str(args.pg_bin_dir),
            "--keep",
        ],
        capture_output=True,
        text=True,
        cwd=str(ROOT),
    )
    if cluster.returncode != 0 or not cluster_report.is_file():
        print(cluster.stdout, cluster.stderr, file=sys.stderr)
        return 1
    state = json.loads(cluster_report.read_text())
    url = state["database_url"]
    report: dict = {
        "scope": "WP-06 — profile posts and profile media (T-501 … T-512)",
        "verification": "V-06, V-07, V-10",
        "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
        "postgres_version": state.get("postgres_version"),
        "migrations_applied": state.get("migrations_applied"),
        "commands": [],
    }
    try:
        report["commands"].append(run("cargo +1.88.0 check --locked --workspace", BACKEND))
        for target in CONCERT_TESTS:
            report["commands"].append(
                run(
                    f"cargo +1.88.0 test --locked -p concert-service --test {target}",
                    BACKEND,
                    {"CONCERT_TEST_DATABASE_URL": url},
                )
            )
        report["commands"].append(
            run(
                "cargo +1.88.0 test --locked -p concert-service --lib profile_",
                BACKEND,
            )
        )
        report["commands"].append(
            run(
                "cargo +1.88.0 test --locked -p api-gateway --test profile_media_routes",
                BACKEND,
            )
        )
        report["commands"].append(run("corepack pnpm@11.7.0 run typecheck", FRONTEND))
        report["commands"].append(
            run("corepack pnpm@11.7.0 exec vitest run src/features/profile-media", FRONTEND)
        )
    finally:
        stop = subprocess.run(
            [
                sys.executable,
                str(BACKEND / "scripts/mcr/wp06_profile_posts_cluster.py"),
                "--report",
                str(cluster_report),
            ],
            capture_output=True,
            text=True,
            cwd=str(ROOT),
        )
        report["cluster_cleanup"] = "PASS" if stop.returncode == 0 else "FAIL"
    failed = [entry for entry in report["commands"] if entry["exit_code"] != 0]
    report["result"] = "PASS" if not failed else "FAIL"
    report["failed_commands"] = [entry["command"] for entry in failed]
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"result": report["result"], "failed": report["failed_commands"]}))
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
