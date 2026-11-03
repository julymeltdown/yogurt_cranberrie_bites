#!/usr/bin/env python3
"""Run only explicit bridge tests in a new marker-owned socket-only PG cluster.
Reuses committed migration_smoke.Cluster ownership/cleanup implementation.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
import migration_smoke as migration

parser = argparse.ArgumentParser()
parser.add_argument("--binary", required=True, type=Path)
parser.add_argument("--report", required=True, type=Path)
args = parser.parse_args()
if not args.binary.is_absolute():
    parser.error("an explicit absolute binary path is required")
binary = migration.executable(args.binary)
report = {"scope": "Actual isolated PostgreSQL bridge; synthetic external authority, NOT IN01", "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest()}
cluster = None
try:
    tools = {name: migration.executable(Path("/opt/homebrew/opt/postgresql@16/bin") / name) for name in migration.PG_TOOLS}
    cluster = migration.Cluster(tools)
    report["owned_root"] = str(cluster.root)
    cluster.init()
    cluster.start()
    report["postgres"] = cluster.sql("postgres", "show server_version;")
    cluster.sql("postgres", "create database mcr_world template template0;")
    for name in ["0001_concert_core.sql", "0002_command_receipts_outbox.sql", "0003_places_map_releases.sql", "0004_bootstrap_admission.sql", "0005_world_admission.sql"]:
        cluster.sql("mcr_world", (ROOT / "backend/services/concert-service/migrations" / name).read_text())
    env = {**cluster.env, "WORLD_TEST_DATABASE_URL": cluster.url("mcr_world"), "WORLD_TEST_OWNER_ROOT": str(cluster.root), "WORLD_TEST_OWNER_TOKEN": cluster.token, "WORLD_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}"}
    result = subprocess.run([str(binary), "--ignored", "native_database_bridge", "--test-threads=1", "--nocapture"], env=env, cwd=cluster.root, capture_output=True, text=True, timeout=45)
    log = args.report.with_suffix(".log")
    log.write_text(result.stdout + result.stderr)
    report.update(native_exit=result.returncode, log_sha256=hashlib.sha256(log.read_bytes()).hexdigest(), result="PASS" if result.returncode == 0 else "FAIL")
except BaseException as error:
    report.update(result="FAIL", failure=type(error).__name__)
    raise
finally:
    if cluster is not None:
        cluster.cleanup()
        report["cleanup"] = "PASS"
    args.report.write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
sys.exit(0 if report["result"] == "PASS" else 1)
