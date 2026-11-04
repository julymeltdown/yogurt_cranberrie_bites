#!/usr/bin/env python3
"""QA03 source-bound copied-binary qualification in an owned PostgreSQL16 store.

snapshot runs before the separately leased Cargo command; freeze checks that the
same source survived compilation. run refuses target binaries and stale inputs.
Cluster lifecycle/ledger code is reused read-only from migration_smoke.py at93ef0d8.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
import migration_smoke as migration

BEFORE = (
    "party::last_slot_and_identical_replay_are_forced",
    "gifts::five_buyers_forced_one_unit_and_duplicate_replay",
    "gifts::claim_cancel_forced_terminal_transition_once",
    "gifts::duplicate_claim_and_negative_actors_codes",
    "gifts::expiry_cancel_claim_release_once",
    "gifts::expiry_skip_locked_pending_worker_restart_seed",
    "wall::identical_idempotency_and_cooldown_wait_on_real_lock",
    "wall::storage_failure_rolls_back_all_effects_then_same_key_recovers",
)
AFTER = (
    "party::restart_preserves_approval_receipt",
    "gifts::restarted_worker_drains_expiry_exactly_once",
    "wall::restart_retains_operation_receipt_and_cooldown",
)
DATABASE = "mcr_qa_races"


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sources() -> dict[str, str]:
    files = subprocess.check_output(
        ["git", "ls-files", "-z", "backend", "contracts"], cwd=ROOT
    ).decode().split("\0")
    inputs = {p for p in files if p and Path(p).suffix in {".rs", ".toml", ".lock", ".proto", ".sql"}}
    inputs.update(str(p.relative_to(ROOT)) for p in (ROOT / "contracts").rglob("*.json") if "node_modules" not in p.parts)
    inputs.update(str(p.relative_to(ROOT)) for p in (ROOT / "backend/services/concert-service/tests/qa03_races_support").glob("*.rs"))
    inputs.update((
        "backend/services/concert-service/tests/qa03_races.rs",
        "backend/scripts/mcr/migration_smoke.py",
        str(Path(__file__).relative_to(ROOT)),
    ))
    return {p: sha(ROOT / p) for p in sorted(inputs)}


def create(path: Path, payload: dict) -> None:
    if not path.is_absolute() or path.is_symlink():
        raise RuntimeError("new absolute report path required")
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "w") as stream:
        json.dump(payload, stream, indent=2, sort_keys=True)
        stream.write("\n")


def copied(path: Path) -> None:
    if not path.is_absolute() or path.resolve() != path or path.is_symlink() or not path.is_file() or not os.access(path, os.X_OK):
        raise RuntimeError("absolute copied regular executable required")
    if any(p in {"target", "mcr-build"} for p in path.parts):
        raise RuntimeError("shared target execution refused")


def freeze(args) -> None:
    before = json.loads(args.snapshot.read_text())
    if before["source_sha256"] != sources():
        raise RuntimeError("source changed during compilation")
    messages = [json.loads(line) for line in args.cargo_json.read_text().splitlines() if line.startswith("{")]
    if not any(m.get("reason") == "build-finished" and m.get("success") is True for m in messages):
        raise RuntimeError("successful Cargo completion absent")
    binaries = {m["executable"] for m in messages if m.get("reason") == "compiler-artifact" and m.get("target", {}).get("name") == "qa03_races" and m.get("executable")}
    if len(binaries) != 1:
        raise RuntimeError("one exact QA03 binary required")
    # This command may read the leased target only before its explicit release.
    original = Path(binaries.pop())
    if args.binary.exists() or not args.binary.is_absolute() or any(p in {"target", "mcr-build"} for p in args.binary.parts):
        raise RuntimeError("new copied binary outside target required")
    shutil.copy2(original, args.binary)
    copied(args.binary)
    before.update(binary=str(args.binary), binary_sha256=sha(args.binary), cargo_json_sha256=sha(args.cargo_json), exit_code=0)
    create(args.receipt, before)


def run(args) -> dict:
    report = {"status": "FAIL", "scope": "QA03 real database adapters with isolated test ACL",
              "canonical_owner_rpc": "NOT_RUN", "public_http": "NOT_RUN", "IN02": "NOT_RUN", "IN04": "NOT_RUN",
              "tests": [], "cleanup": "NOT_RUN"}
    cluster = None
    previous = signal.signal(signal.SIGTERM, lambda *_: (_ for _ in ()).throw(KeyboardInterrupt()))
    try:
        copied(args.binary)
        receipt = json.loads(args.receipt.read_text())
        if receipt.get("exit_code") != 0 or receipt.get("binary") != str(args.binary) or receipt.get("binary_sha256") != sha(args.binary) or receipt.get("source_sha256") != sources():
            raise RuntimeError("stale source or unbound binary")
        report.update(build_receipt=receipt, build_receipt_sha256=sha(args.receipt))
        listing = subprocess.run([str(args.binary), "--list"], capture_output=True, text=True, timeout=20, check=True)
        inventory = {line.removesuffix(": test") for line in listing.stdout.splitlines() if line.endswith(": test")}
        if inventory != set(BEFORE + AFTER):
            raise RuntimeError("empty or unexpected QA03 test inventory")
        tools = {name: migration.executable(args.pg_bin / name) for name in migration.PG_TOOLS}
        cluster = migration.Cluster(tools)
        cluster.init()
        cluster.start()
        if int(cluster.sql("postgres", "show server_version_num")) // 10000 != 16:
            raise RuntimeError("PostgreSQL16 required")
        report["postgres_version"] = cluster.sql("postgres", "show server_version")
        cluster.sql("postgres", f"create database {DATABASE} template template0")
        environment = {**cluster.env, "QA03_ROOT": str(cluster.root), "QA03_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
                       "QA03_TOKEN": cluster.token, "QA03_DATABASE_URL": cluster.url(DATABASE),
                       "QA03_SYSTEM": cluster.sql("postgres", "select system_identifier from pg_control_system()")}

        def case(name: str, phase: str) -> None:
            command = [str(args.binary), "--ignored", "--exact", name, "--test-threads=1", "--nocapture"]
            result = subprocess.run(command, cwd=ROOT, env=environment, capture_output=True, text=True, timeout=75)
            output = result.stdout + result.stderr
            if cluster.password in output or cluster.url(DATABASE) in output:
                raise RuntimeError("database secret in test output")
            row = {"case": name, "phase": phase, "exit_code": result.returncode, "output_sha256": hashlib.sha256(output.encode()).hexdigest(),
                   "interleavings": re.findall(r"QA03_INTERLEAVING holder=\d+ blocked=\d+ required=\d+", output)}
            report["tests"].append(row)
            if result.returncode != 0 or not re.search(r"test result: ok\. 1 passed; 0 failed; 0 ignored;", result.stdout):
                row["failure_output"] = re.sub(r"(?:postgres(?:ql)?|redis)://[^\s]+", "[redacted-url]", output)[-8000:]
                raise RuntimeError(f"exact native case failed: {name}")
            row["status"] = "PASS"

        for name in BEFORE:
            case(name, "before-restart")
        manifest = migration.migration_manifest(ROOT / "backend", "concert-service")
        report["ledger_before"] = migration.check_ledger(cluster, DATABASE, manifest)
        before = cluster.assert_postmaster()
        cluster.stop()
        cluster.start()
        after = cluster.assert_postmaster()
        if before == after:
            raise RuntimeError("postmaster did not restart")
        report["restart"] = {"before_pid": before, "after_pid": after, "status": "PASS"}
        for name in AFTER:
            case(name, "after-restart-new-process")
        report["ledger_after"] = migration.check_ledger(cluster, DATABASE, manifest)
        report["migration_sha384"] = manifest
        if cluster.sql(DATABASE, "select count(*) from pg_prepared_xacts") != "0" or cluster.sql(DATABASE, "select count(*) from concert_command_receipts where state='PENDING'") != "0":
            raise RuntimeError("prepared or pending orphan state")
        report["orphan_prepared_pending"] = "ZERO"
        report["status"] = "PASS"
    except BaseException as error:
        report["failure"] = str(error) if isinstance(error, RuntimeError) else type(error).__name__
        if isinstance(error, migration.SmokeError):
            report["failure_detail"] = error.detail
    finally:
        signal.signal(signal.SIGTERM, previous)
        try:
            if cluster is not None:
                root = cluster.root
                cluster.cleanup()
                if root.exists():
                    raise RuntimeError("owned PostgreSQL root remains")
            report["cleanup"] = "PASS"
        except BaseException as error:
            report.update(status="FAIL", cleanup="FAIL", cleanup_failure=type(error).__name__)
        create(args.report, report)
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    snapshot = sub.add_parser("snapshot")
    snapshot.add_argument("--report", required=True, type=Path)
    frozen = sub.add_parser("freeze")
    for name in ("snapshot", "cargo-json", "binary", "receipt"):
        frozen.add_argument("--" + name, required=True, type=Path)
    qualify = sub.add_parser("run")
    for name in ("binary", "receipt", "report", "pg-bin"):
        qualify.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    if args.action == "snapshot":
        create(args.report, {"source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), "source_sha256": sources()})
    elif args.action == "freeze":
        freeze(args)
    else:
        if args.report.exists() or not args.report.is_absolute():
            parser.error("new absolute report required; previous failures are immutable")
        result = run(args)
        print(json.dumps({k: result.get(k) for k in ("status", "failure", "cleanup")}))
        return 0 if result["status"] == "PASS" else 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
