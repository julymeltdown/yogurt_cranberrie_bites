#!/usr/bin/env python3
"""Run the Party producer against a new marker-owned Unix PostgreSQL cluster."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

import migration_smoke as migration

ROOT = Path(__file__).resolve().parents[3]
DATABASE = "mcr_party_live"
TEST = "party_live_native"
MIGRATIONS = (
    "0001_concert_core.sql",
    "0002_command_receipts_outbox.sql",
    "0003_places_map_releases.sql",
    "0004_bootstrap_admission.sql",
    "0006_relationship_policy.sql",
    "0007_community.sql",
    "0009_receipt_domain_failures.sql",
    "0011_outbox_maintenance.sql",
)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def regular(path: Path, executable: bool = False) -> Path:
    if not path.is_absolute() or path.resolve() != path or not path.is_file():
        raise RuntimeError("canonical absolute regular artifact required")
    if path.is_symlink() or "target" in path.parts or "mcr-build" in path.parts:
        raise RuntimeError("artifact must be frozen outside shared build target")
    if executable and not os.access(path, os.X_OK):
        raise RuntimeError("test executable is not executable")
    return path


def run(binary: Path, report: Path, pg_bin: Path) -> dict:
    binary = regular(binary, executable=True)
    binary_sha = digest(binary)
    cluster = None
    result: dict = {
        "status": "FAIL",
        "scope": "native-postgres",
        "binary": str(binary),
        "binary_sha256": binary_sha,
        "native_postgres": "NOT_RUN",
        "canonical_owner_holder": "NOT_RUN",
        "public_profile_holder": "NOT_RUN",
        "product_route": "NOT_RUN",
        "live_http": "NOT_RUN",
    }
    try:
        tools = {
            name: migration.executable(pg_bin / name) for name in migration.PG_TOOLS
        }
        cluster = migration.Cluster(tools)
        cluster.init()
        cluster.start()
        result["postgres_version"] = cluster.sql("postgres", "show server_version;")
        if not result["postgres_version"].startswith("16."):
            raise RuntimeError("PostgreSQL 16 required")
        cluster.sql("postgres", f"create database {DATABASE} template template0;")
        for name in MIGRATIONS:
            cluster.sql(
                DATABASE,
                (ROOT / "backend/services/concert-service/migrations" / name).read_text(),
            )
        cluster.sql(
            DATABASE,
            "create table party_live_authority(event_id uuid primary key,members uuid[] not null,blocked uuid[] not null,unavailable boolean not null);"
            "create table party_live_pair_policy(event_id uuid,low_user uuid,high_user uuid,low_blocks_high boolean not null,high_blocks_low boolean not null,revision bigint not null default 1,primary key(event_id,low_user,high_user));",
        )
        env = {
            **cluster.env,
            "CONCERT_TEST_DATABASE_URL": cluster.url(DATABASE),
            "CONCERT_TEST_OWNER_ROOT": str(cluster.root),
            "CONCERT_TEST_OWNER_TOKEN": cluster.token,
            "CONCERT_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
        }
        completed = subprocess.run(
            [str(binary), "--ignored", "--exact", "--test-threads=1", TEST],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            timeout=120,
        )
        result["command_exit_code"] = completed.returncode
        result["stdout_sha256"] = hashlib.sha256(completed.stdout.encode()).hexdigest()
        result["stderr_tail"] = re.sub(
            r"(?:postgres(?:ql)?|redis)://[^\s]+", "[redacted-url]", completed.stderr
        )[-4000:]
        match = re.search(
            r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;",
            completed.stdout,
        )
        if completed.returncode != 0 or match is None:
            raise RuntimeError("exact native test did not pass")
        result["native_postgres"] = "PASS"
        result["status"] = "PASS"
    except BaseException as error:
        result["failure"] = type(error).__name__
        if isinstance(error, RuntimeError):
            result["diagnostic"] = str(error)
    finally:
        try:
            if cluster is not None:
                root = cluster.root
                cluster.cleanup()
                if root.exists():
                    raise RuntimeError("owned root remains")
            result["cleanup"] = "PASS"
        except BaseException as error:
            result["cleanup"] = "FAIL"
            result["status"] = "FAIL"
            result["cleanup_failure"] = type(error).__name__
        report.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
        os.chmod(report, 0o600)
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-binary", type=Path, required=True)
    parser.add_argument("--pg-bin", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args(argv)
    if not args.report.is_absolute() or args.report.exists():
        parser.error("new absolute report path required")
    result = run(args.test_binary, args.report, args.pg_bin)
    print(json.dumps({key: result.get(key) for key in ("status", "cleanup", "failure", "diagnostic")}))
    return 0 if result.get("status") == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
