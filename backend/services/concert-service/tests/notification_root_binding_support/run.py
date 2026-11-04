#!/usr/bin/env python3
"""Run only the frozen NB01 test in a fresh owned socket-only PG16 cluster.

Reuses the marker/inode/postmaster-guarded fixture from migration_smoke.py at
Root 73e89bce. No Cargo, live services, ambient database or authority fixtures.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import time

BACKEND = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(BACKEND / "scripts/mcr"))
import migration_smoke as migration


def require(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(args, report):
    receipt = json.loads(args.receipt.read_text())
    artifact = args.artifact.resolve(strict=True)
    require(receipt["exit_code"] == 0 and receipt["locked"] and receipt["offline"], "Successful locked offline receipt required")
    require(receipt["rustc_version"].startswith("rustc 1.88.0") and receipt["jobs"] == 2
            and receipt["debug"] == "0" and receipt["incremental"] == "0", "Bounded Rust1.88 receipt required")
    emitted = receipt["cargo_artifact"]
    require(emitted["reason"] == "compiler-artifact" and emitted["target"]["name"] == "notification_root_binding"
            and emitted["profile"]["test"] is True, "Exact emitted test required")
    require(artifact == Path(receipt["private_artifact"]).resolve() and sha(artifact) == receipt["artifact_sha256"], "Frozen artifact differs")
    require(receipt["source_sha256"], "Source manifest required")
    required = {BACKEND / "Cargo.lock", BACKEND / "Cargo.toml", Path(__file__).resolve(),
                BACKEND / "scripts/mcr/migration_smoke.py"}
    for directory in ("services/concert-service", "crates/platform", "crates/concert-application", "crates/concert-domain"):
        base = BACKEND / directory
        required.add(base / "Cargo.toml")
        required.update(base.rglob("*.rs"))
    required.update((BACKEND / "proto").rglob("*.proto"))
    required.update((BACKEND / "services/concert-service/migrations").glob("*.sql"))
    require({str(p.relative_to(BACKEND.parent)) for p in required} <= receipt["source_sha256"].keys(), "Source manifest incomplete")
    for name, digest in receipt["source_sha256"].items():
        path = (BACKEND.parent / name).resolve(strict=True)
        require(not Path(name).is_absolute() and path.is_relative_to(BACKEND.parent), "Invalid source path")
        require(sha(path) == digest, "Source pin differs")
    report.update(artifact_sha256=sha(artifact), receipt_sha256=sha(args.receipt), source_commit=receipt["source_commit"])
    tools = {name: migration.executable(args.pg_bin / name) for name in migration.PG_TOOLS}
    cluster = migration.Cluster(tools)
    try:
        cluster.init()
        cluster.start()
        report["postgres_version"] = cluster.sql("postgres", "SHOW server_version")
        require(report["postgres_version"].startswith("16."), "PostgreSQL16 required")
        database = "mcr_notification_root_binding"
        cluster.sql("postgres", f"CREATE DATABASE {database} TEMPLATE template0")
        report["migration_sha256"] = {}
        for name in ("0028_product_owner_scope_lifecycle.sql", "0048_root_response_cancellation.sql"):
            path = BACKEND / "services/concert-service/migrations" / name
            result = subprocess.run([str(tools["psql"]), "-X", "--no-password", "-v", "ON_ERROR_STOP=1", "-1", "-f", str(path)],
                                    cwd=cluster.root, env=cluster.client_env(database), capture_output=True, timeout=30)
            report["commands"][name] = result.returncode
            require(result.returncode == 0, "Migration failed")
            report["migration_sha256"][name] = sha(path)
        env = {**cluster.env, "NB01_DATABASE_URL": cluster.url(database), "NB01_OWNER_ROOT": str(cluster.root),
               "NB01_OWNER_TOKEN": cluster.token, "NB01_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
               "NB01_SYSTEM": cluster.sql("postgres", "SELECT system_identifier::text FROM pg_control_system()")}
        result = subprocess.run([str(artifact), "--test-threads=1"], cwd=cluster.root, env=env, capture_output=True, timeout=45)
        output = (result.stdout + result.stderr).decode(errors="replace")
        require(all(v not in output for v in (cluster.password, cluster.token, cluster.url(database))), "Diagnostic secret leak")
        report["commands"]["native_binding_tests"] = result.returncode
        report["diagnostic"] = output[-10000:]
        summary = re.search(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored", output)
        require(summary is not None, "Missing native summary")
        report["tests"] = dict(zip(("passed", "failed", "ignored"), map(int, summary.groups())))
        require(result.returncode == 0 and report["tests"] == {"passed": 7, "failed": 0, "ignored": 0}, "Native binding tests failed")
        require(sha(artifact) == receipt["artifact_sha256"], "Artifact changed")
        report["status"] = "PASS"
    finally:
        report["owned_fixture_peak_allocated_bytes"] = sum(p.stat().st_blocks * 512 for p in cluster.root.rglob("*") if p.is_file())
        cluster.cleanup()
        report["cleanup"] = "PASS"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--pg-bin", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    report = {"status": "FAIL", "commands": {}, "scope": "NB01 lookup mechanics only; canonical owner/public acceptance NOT_RUN"}
    start = time.monotonic()
    try:
        run(args, report)
    except Exception as error:
        report["status"] = "FAIL"
        report["error"] = error.detail if isinstance(error, migration.SmokeError) else type(error).__name__ + ": " + str(error)
    finally:
        report["elapsed_seconds"] = time.monotonic() - start
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({k: v for k, v in report.items() if k != "diagnostic"}))
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
