#!/usr/bin/env python3
"""Run the six-entry consumer visibility probe on a new marker-owned PG16 cluster."""
from __future__ import annotations
import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(Path(__file__).resolve().parent))
from migration_smoke import Cluster, PG_TOOLS, executable

DATABASE = "mcr_consumer_probe"
MIGRATIONS = (
    "0001_concert_core.sql",
    "0002_command_receipts_outbox.sql",
    "0003_places_map_releases.sql",
    "0004_bootstrap_admission.sql",
    "0006_relationship_policy.sql",
)
TEST = "relationship::consumer_probes::committed_block_and_suspension_remove_all_six_read_entries_and_reread_after_revision"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-binary", type=Path, required=True)
    parser.add_argument("--pg-bin", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    binary = args.test_binary.resolve()
    if not args.test_binary.is_absolute() or not binary.is_file():
        parser.error("absolute test binary required")
    report = {"status": "FAIL", "test": TEST}
    cluster = Cluster({name: executable(args.pg_bin / name) for name in PG_TOOLS})
    try:
        cluster.init()
        cluster.start()
        subprocess.run(
            [str(cluster.tools["createdb"]), "--no-password", "--maintenance-db=postgres", DATABASE],
            env=cluster.client_env("postgres"),
            check=True,
        )
        concert = ROOT / "backend/services/concert-service"
        for name in MIGRATIONS:
            cluster.sql(DATABASE, (concert / "migrations" / name).read_text())
        cluster.sql(
            DATABASE,
            "create table concert_event_settings(event_id uuid primary key references concert_events(event_id),"
            "parties boolean not null,giveaways boolean not null,intros boolean not null,social boolean not null,"
            "mural boolean not null,beacon_ttl_seconds bigint not null,pickup_grace_seconds bigint not null,"
            "social_grant_ttl_seconds bigint not null,mural_cooldown_ms bigint not null);",
        )
        result = subprocess.run(
            [str(binary), "--exact", TEST, "--nocapture"],
            env={**cluster.env, "CONCERT_CONSUMER_TEST_URL": cluster.url(DATABASE)},
            cwd=ROOT / "backend",
            capture_output=True,
            text=True,
            timeout=120,
        )
        tail = (result.stdout + result.stderr).splitlines()[-12:]
        report.update(exit=result.returncode, tail=tail)
        report["status"] = "PASS" if result.returncode == 0 else "FAIL"
    finally:
        cluster.cleanup()
        report["cleanup"] = "PASS" if not cluster.root.exists() else "FAIL"
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({k: report[k] for k in ("status", "exit", "cleanup")}))
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
