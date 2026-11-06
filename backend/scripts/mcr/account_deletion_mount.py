#!/usr/bin/env python3
"""Native qualification for the mounted account-deletion composer.

Runs the ignored `account_deletion_runtime` case that composes
`AccountDeletionRuntime::compose` on the real serving pool, serves it over
mTLS HTTP/2, and drives the canonical Auth source. The case proves a sealed
PrivateData ACK, caller refusal before the deleter, live-session source refusal,
service-restart replay without duplicate effects, zero private rows for the
deleted user and an intact control user. A physical PostgreSQL restart then
reruns the same binary against the same durable stores.

Uses the existing bounded test fixture helpers (migration_smoke.Cluster and
invalidation_clients_smoke.pki); no Cargo/cache access inside the cluster.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path
from uuid import UUID

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
import migration_smoke as core  # noqa: E402
from invalidation_clients_smoke import pki  # noqa: E402

CASE = "mounted_private_data_ack_restart_and_session_revocation_boundary"
OWNERS = ("auth", "concert")


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def probes(report: dict, phase: str) -> list[dict]:
    values = []
    for line in (report.get(phase) or {}).get("stdout", "").splitlines():
        if "QA09_DELETION_PROBE " in line:
            values.append(json.loads(line.split("QA09_DELETION_PROBE ", 1)[1]))
    return values


def run_case(binary: Path, env: dict, report: dict, phase: str, timeout: int) -> None:
    command = [str(binary), "--ignored", "--exact", CASE, "--test-threads=1", "--nocapture"]
    child = subprocess.Popen(
        command,
        env=env,
        cwd=ROOT / "backend",
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    )
    began = time.monotonic()
    try:
        out, err = child.communicate(timeout=timeout)
    except BaseException:
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.communicate(timeout=8)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.communicate(timeout=8)
        raise
    report[phase] = {
        "exit_code": child.returncode,
        "stdout": out,
        "stderr": err,
        "elapsed_seconds": round(time.monotonic() - began, 3),
    }
    if child.returncode or "test result: ok. 1 passed; 0 failed;" not in out:
        raise RuntimeError(f"{phase} case failed; see sanitized report")


def qualify(args, report: dict) -> None:
    binary = args.binary
    if not binary.is_absolute() or binary.is_symlink() or not binary.is_file():
        raise RuntimeError("explicit copied absolute test binary required")
    report["binary_sha256"] = sha(binary)
    tools = {name: core.executable(args.pg_bin / name) for name in core.PG_TOOLS}
    cluster = core.Cluster(tools)
    report["owned_root"] = str(cluster.root)
    try:
        cluster.init()
        with (cluster.data / "postgresql.conf").open("a") as f:
            f.write(
                "\nmax_prepared_transactions=256\nmax_connections=48\nshared_buffers=32MB\n"
                "work_mem=4MB\nmax_worker_processes=0\nmax_parallel_workers=0\n"
                "max_wal_senders=0\nautovacuum=off\n"
            )
        cluster.start()
        pki(cluster)
        version = int(cluster.sql("postgres", "show server_version_num;"))
        if version // 10000 != 16:
            raise RuntimeError("PG16 required")
        report["postgres_version"] = cluster.sql("postgres", "show server_version;")
        env = {
            **cluster.env,
            "CONCERT_SERVICE_CREDENTIAL": "concert-fixture",
            "MCR_CANONICAL_ROOT": str(cluster.root),
            "MCR_CANONICAL_ROOT_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
            "MCR_CANONICAL_ROOT_TOKEN": cluster.token,
            "MCR_CANONICAL_SYSTEM": cluster.sql(
                "postgres", "select system_identifier from pg_control_system();"
            ),
        }
        for owner in OWNERS:
            db = "mcr_canonical_" + owner
            core.execute(
                [str(tools["createdb"]), "--no-password", "--maintenance-db=postgres", db],
                env=cluster.client_env("postgres"),
                cwd=cluster.root,
                phase="create-" + owner,
            )
            env["MCR_CANONICAL_" + owner.upper() + "_URL"] = cluster.url(db)
        run_case(binary, env, report, "source", 240)
        report["migration_receipts"] = {
            o: core.check_ledger(
                cluster,
                "mcr_canonical_" + o,
                core.migration_manifest(ROOT / "backend", o + "-service"),
            )
            for o in OWNERS
        }
        before = cluster.assert_postmaster()
        cluster.stop()
        cluster.start()
        after = cluster.assert_postmaster()
        if before == after:
            raise RuntimeError("physical PG restart required")
        report["restart"] = {"before_pid": before, "after_pid": after}
        run_case(binary, env, report, "after_pg_restart", 240)
        measured = {phase: probes(report, phase) for phase in ("source", "after_pg_restart")}
        for phase, values in measured.items():
            if len(values) != 2 or [value.get("phase") for value in values] != ["before", "after"]:
                raise RuntimeError(f"{phase} deletion probe did not report before/after rows")
        report["probes"] = measured
        deleted = str(UUID(measured["source"][1]["deleted_user"]))
        report["persisted_after_pg_restart"] = {
            "deleted_user": deleted,
            "private_rows": cluster.sql(
                "mcr_canonical_concert",
                "select (select count(*) from concert_participants where user_id='" + deleted + "')||'|'||"
                "(select count(*) from concert_avatar_settings where user_id='" + deleted + "')||'|'||"
                "(select count(*) from concert_age_owners where user_id='" + deleted + "')",
            ),
            "saga": cluster.sql(
                "mcr_canonical_auth",
                "select status||'|'||coalesce(completed_at::text,'') from user_deletion_sagas "
                "where user_id='" + deleted + "'",
            ),
            "steps": cluster.sql(
                "mcr_canonical_auth",
                "select s.target||'|'||s.status||'|'||s.attempt_count from user_deletion_steps s "
                "join user_deletion_sagas g on g.id=s.saga_id where g.user_id='" + deleted + "' order by s.target",
            ),
        }
        report["migration_receipts_after"] = {
            o: core.check_ledger(
                cluster,
                "mcr_canonical_" + o,
                core.migration_manifest(ROOT / "backend", o + "-service"),
            )
            for o in OWNERS
        }
    finally:
        cluster.cleanup()
        report["cleanup"] = {"owned_root_absent": not cluster.root.exists()}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--pg-bin", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if not all(x.is_absolute() for x in (args.binary, args.pg_bin, args.report)):
        parser.error("explicit absolute binary/PG/report paths required")
    if args.report.exists():
        parser.error("new report path required")

    def stop(_signum, _frame):
        raise KeyboardInterrupt("bounded mount qualification interrupted")

    signal.signal(signal.SIGTERM, stop)
    report = {
        "status": "FAIL",
        "limitations": [
            "Authenticated Apply uses the production factory/service on a test-owned mTLS loopback listener; the real main.rs process is covered by source-composition assertion, not a process launch.",
            "The Sharing terminal row is an exact durable fixture for the preceding saga step; the Social owner flow itself is not re-qualified here.",
            "No Gateway HTTP, browser, deployment or whole-task acceptance is claimed.",
        ],
    }
    try:
        qualify(args, report)
        report["status"] = "PASS"
    except (Exception, KeyboardInterrupt) as error:  # noqa: BLE001
        report["failure"] = type(error).__name__ + ": " + str(error)
    finally:
        args.report.write_text(json.dumps(report, indent=2, sort_keys=True, default=str) + "\n")
        args.report.chmod(0o600)
    print(json.dumps({"status": report["status"], "report": str(args.report)}))
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
