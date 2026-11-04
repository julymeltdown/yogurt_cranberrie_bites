#!/usr/bin/env python3
"""Qualify only the frozen Intro deletion artifact in a new original-owned PG16.

Requires a separate accepted finite allocation. This script grants no resources.
Canonical Intro fixture reuse does not certify the synthetic deletion source or
Sharing fixture as production Auth/Sharing transport, or whole BE42/IN03.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import sys
sys.dont_write_bytecode = True
import subprocess

ROOT = Path(__file__).resolve().parents[3]
ORIGINAL = ROOT / "backend/services/concert-service/tests/intro_product_support/qualify.py"
SPEC = importlib.util.spec_from_file_location("intro_original_qualifier", ORIGINAL)
original = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(original)
CASES = ("intro_deletion_draft", "intro_deletion_published", "intro_deletion_closed")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sources():
    return {**original.source_manifest(), str(Path(__file__).resolve().relative_to(ROOT)): sha(Path(__file__))}


def verify(checkpoint):
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if (checkpoint["source_commit"] != head or checkpoint["source_sha256"] != sources()
            or subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT).strip()):
        raise ValueError("clean exact source checkpoint required")
    compiled = checkpoint.get("binary_source_commit", head)
    if compiled != head:
        # A diagnostic/report-only successor may reuse the exact frozen EXE.
        # Every compiled/runtime input except this Python reporter stays exact.
        helper = str(Path(__file__).resolve().relative_to(ROOT))
        allowed = {helper, "docs/mcr/backend/intro-account-deletion.md"}
        subprocess.run(["git", "merge-base", "--is-ancestor", compiled, head], cwd=ROOT, check=True)
        changed = subprocess.check_output(["git", "diff", "--name-only", compiled, head], cwd=ROOT, text=True).splitlines()
        built = checkpoint.get("binary_source_sha256", {})
        if (not set(changed) <= allowed or set(built) != set(checkpoint["source_sha256"])
                or any(value != checkpoint["source_sha256"][name] for name, value in built.items() if name != helper)):
            raise ValueError("frozen binary inputs changed beyond diagnostic reporter/docs")
    cargo = Path(checkpoint["cargo_json"])
    if not cargo.is_absolute() or cargo.is_symlink() or sha(cargo) != checkpoint["cargo_json_sha256"]:
        raise ValueError("frozen Cargo receipt required")
    artifacts = [json.loads(line) for line in cargo.read_text().splitlines() if line.startswith("{")]
    tests = [v for v in artifacts if v.get("reason") == "compiler-artifact"
             and v.get("target", {}).get("name") == "intro_deletion" and v.get("executable")]
    if len(tests) != 1 or tests[0]["target"]["src_path"] != str(ROOT / "backend/services/concert-service/tests/intro_deletion.rs"):
        raise ValueError("exact Intro deletion test artifact required")
    return original.frozen_binary(Path(checkpoint["binary"]), checkpoint["binary_sha256"])


def qualify(checkpoint, pg, report):
    binary = verify(checkpoint)
    if not pg.is_absolute():
        raise ValueError("explicit absolute PostgreSQL tools required")
    tools = {name: original.executable(pg / name) for name in original.PG_TOOLS}
    cluster = original.Cluster(tools)
    report.update(source_commit=checkpoint["source_commit"], source_sha256=checkpoint["source_sha256"],
                  binary_source_commit=checkpoint.get("binary_source_commit", checkpoint["source_commit"]),
                  binary_sha256=checkpoint["binary_sha256"], phases=[], cleanup="NOT_RUN",
                  wholeBE42Accepted=False, wholeIN03Accepted=False,
                  limitations=["Deletion Auth source and sealed Sharing records are explicit test fixtures.",
                               "Actual canonical Intro authoring/owner stores are exercised; no public Gateway/browser credit."])
    try:
        cluster.init()
        with (cluster.data / "postgresql.conf").open("a") as f:
            f.write("\nmax_prepared_transactions=256\n")
        cluster.start()
        if int(cluster.sql("postgres", "show server_version_num;")) // 10000 != 16:
            raise ValueError("PostgreSQL16 required")
        original.pki(cluster)
        original.profile_pki(cluster)
        env = {**cluster.env, "CONCERT_SERVICE_CREDENTIAL": "concert-fixture", "RUST_MIN_STACK": "16777216",
               "MCR_CANONICAL_ROOT": str(cluster.root), "MCR_CANONICAL_ROOT_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
               "MCR_CANONICAL_ROOT_TOKEN": cluster.token,
               "MCR_CANONICAL_SYSTEM": cluster.sql("postgres", "select system_identifier from pg_control_system();")}

        def run(case, ignored=True):
            args = [str(binary), "--exact", case, "--test-threads=1", "--nocapture"]
            if ignored:
                args.append("--ignored")
            result = original.execute(args, env=env, phase=case, cwd=ROOT / "backend", timeout=60)
            if "test result: ok. 1 passed; 0 failed;" not in result.stdout:
                raise ValueError("exact one-test inventory required")
            report["phases"].append({"case": case, "exit": result.returncode})

        run("intro_deletion_wrong_target_and_rebound_source_fail_before_database", False)
        for index, case in enumerate(CASES):
            for owner in original.OWNERS:
                database = original.DATABASES[owner]
                if index:
                    cluster.sql("postgres", f"drop database {database} with (force);")
                original.execute([str(tools["createdb"]), "--no-password", "--maintenance-db=postgres", database],
                                 env=cluster.client_env("postgres"), phase="create-owned-database", cwd=cluster.root)
                env[f"MCR_CANONICAL_{owner.upper()}_URL"] = cluster.url(database)
            run(case)
            for owner in original.OWNERS:
                original.check_ledger(cluster, original.DATABASES[owner],
                                      original.migration_manifest(ROOT / "backend", f"{owner}-service"))
            if int(cluster.sql("postgres", "select count(*) from pg_prepared_xacts;")):
                raise ValueError("unexpected prepared owner work")
            before = cluster.assert_postmaster()
            cluster.stop()
            cluster.start()
            if cluster.assert_postmaster() == before:
                raise ValueError("actual postmaster restart required")
            run("intro_deletion_restart_replay")
        verify(checkpoint)
        report["state"] = "PASS"
    finally:
        cluster.cleanup()
        report["cleanup"] = "PASS" if not cluster.root.exists() else "FAIL"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--pg-bin-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if not args.report.is_absolute() or args.report.is_symlink():
        raise ValueError("new absolute private report required")
    descriptor = os.open(args.report, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    report = {"state": "FAIL"}
    try:
        qualify(json.loads(args.checkpoint.read_text()), args.pg_bin_dir, report)
        return 0
    except BaseException as error:
        report["failureClass"] = type(error).__name__
        report["privateDiagnostic"] = error.detail if isinstance(error, original.SmokeError) else str(error)[:16384]
        return 130 if isinstance(error, KeyboardInterrupt) else 1
    finally:
        with os.fdopen(descriptor, "w") as f:
            json.dump(report, f, indent=2)
            f.write("\n")


if __name__ == "__main__":
    def interrupted(_signum, _frame):
        raise KeyboardInterrupt
    signal.signal(signal.SIGTERM, interrupted)
    raise SystemExit(main())
