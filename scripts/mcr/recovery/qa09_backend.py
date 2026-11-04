#!/usr/bin/env python3
"""QA09 original-case orchestration and independent, read-only owner observations.

Native execution requires a separately accepted resource allocation. This module
never grants operator authority, creates business rows, or certifies a whole case
from a component test, an imported report, or an empty database.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import hmac
import importlib
import importlib.util
import json
import os
from pathlib import Path
import re
import secrets
import signal
import subprocess
import sys
from types import SimpleNamespace
import uuid

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
CASES = (
    "QA09-DB-RESTART", "QA09-BACKUP-RESTORE", "QA09-OUTBOX-REPLAY",
    "QA09-PENDING-PICKUP", "QA09-DELETION", "QA09-PARTIAL-CLOSURE",
)
RESTORE = ROOT / "backend/services/concert-service/tests/canonical_owner_restore_support/qualify.py"
# Closed SELECT projections: no payloads, contacts, codes, tokens or error text.
PROJECTIONS = {
    "outbox": ("concert", "concert_outbox", "outbox_id::text AS id, state, attempts", "event_id"),
    "reservations": ("concert", "concert_community_reservations", "reservation_id::text AS id, state, revision", "event_id"),
    "inventory": ("concert", "concert_community_giveaways", "giveaway_id::text AS id, quantity_total, reserved_unclaimed, quantity_claimed", "event_id"),
    "closure_jobs": ("concert", "concert_closure_jobs", "job_id::text AS id, progress->>'state' AS state, revision", "event_id"),
    "closure_effects": ("concert", "concert_closure_effects", "job_id::text || ':' || step AS id, owner_revision AS revision", "event_id"),
    "deletion_steps": ("auth", "user_deletion_steps s JOIN user_deletion_sagas g ON g.id=s.saga_id", "s.saga_id::text || ':' || s.target AS id, s.status AS state, s.attempt_count AS attempts", "g.user_id"),
}
STATES = {
    "outbox": {"READY", "LEASED", "DELIVERED", "DEAD"},
    "reservations": {"HELD", "READY", "CLAIMED", "CANCELLED", "EXPIRED"},
    "closure_jobs": {"PENDING", "RUNNING", "PARTIAL_FAILURE", "COMPLETED"},
    "deletion_steps": {"PENDING", "IN_PROGRESS", "RETRY", "SUCCEEDED", "DEAD_LETTERED"},
}
REQUIRED = {
    CASES[0]: tuple(PROJECTIONS), CASES[1]: tuple(PROJECTIONS),
    CASES[2]: ("outbox",), CASES[3]: ("reservations", "inventory"),
    CASES[4]: ("deletion_steps",), CASES[5]: ("closure_jobs", "closure_effects"),
}
LIMITATIONS = {
    CASES[0]: "No same-candidate application-state restart boundary observed.",
    CASES[1]: "Physical owner restore is supporting evidence, not domain recovery acceptance.",
    CASES[2]: "Actual downstream deduplication and business receipt binding remain unobserved.",
    CASES[3]: "Actual pickup bridge and committed pickup recovery remain unqualified.",
    CASES[4]: "Actual canonical deletion saga recovery and reconnect/reveal fencing remain unqualified.",
    CASES[5]: "Privileged operator and closure producer are unmounted; no trusted authority inferred.",
}


def now():
    return datetime.now(timezone.utc).isoformat()


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def revision():
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()


def canonical_uuid(value):
    parsed = uuid.UUID(value)
    if parsed.int == 0 or str(parsed) != value:
        raise ValueError("canonical nonnil UUID required")
    return value


def label(key, namespace, value):
    if not isinstance(key, bytes) or len(key) < 32:
        raise ValueError("private run-local key of at least 32 bytes required")
    return hmac.new(key, (namespace + "\0" + value).encode(), hashlib.sha256).hexdigest()


def plan(run_id=None):
    run_id = run_id or secrets.token_hex(16)
    if not re.fullmatch(r"[0-9a-f]{32}", run_id):
        raise ValueError("run-local hexadecimal identifier required")
    source = revision()
    return {
        "version": 1, "runId": run_id, "sourceRevisions": {"backend": source},
        "state": "IN_PROGRESS", "wholeQa09Accepted": False,
        "cases": [{"version": 1, "runId": run_id, "caseId": case,
                   "sourceRevisions": {"backend": source},
                   "producerEvidence": None,
                   "phases": {"before": None, "boundary": None, "recovered": None},
                   "state": "NOT_RUN", "observations": {},
                   "limitations": [LIMITATIONS[case]]} for case in CASES],
    }


def query_for(table, scope):
    _, relation, columns, field = PROJECTIONS[table]
    canonical_uuid(scope)
    # UUID validation above makes this fixed projection independent of caller SQL.
    return ("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY; "
            "SET LOCAL statement_timeout='5s'; SET LOCAL lock_timeout='1s'; "
            "SELECT coalesce(json_agg(row_to_json(q)), '[]'::json)::text FROM "
            f"(SELECT {columns} FROM public.{relation} WHERE {field}='{scope}'::uuid "
            "ORDER BY id LIMIT 1001) q; ROLLBACK;")


def sanitize_rows(table, rows, key):
    if not isinstance(rows, list) or len(rows) > 1000:
        raise ValueError("bounded row array required")
    allowed = {"id", "state", "attempts", "revision", "quantity_total", "reserved_unclaimed", "quantity_claimed"}
    result = {}
    for row in rows:
        if not isinstance(row, dict) or set(row) - allowed or not isinstance(row.get("id"), str):
            raise ValueError("unexpected owner projection")
        values = {k: v for k, v in row.items() if k != "id"}
        expected = {column.strip().split()[-1] for column in PROJECTIONS[table][2].split(",")}
        expected = {"state" if x == "state" else x for x in expected} - {"id"}
        if set(values) != expected:
            raise ValueError("incomplete owner projection")
        for name, value in values.items():
            if name == "state":
                if value not in STATES[table]:
                    raise ValueError("unknown owner state")
            elif type(value) is not int or value < 0 or value > 9007199254740991:
                raise ValueError("invalid bounded owner counter")
        token = label(key, table, row["id"])
        if token in result:
            raise ValueError("duplicate owner row")
        result[token] = values
    return result


def capture(cluster, owner, database, scope, key):
    """Called by the actual producer with its live Cluster instance, never a URL.

    Each table read has its own read-only transaction. This is not a cross-owner
    atomic snapshot. The caller retains the HMAC key privately across boundaries.
    """
    sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
    core = importlib.import_module("migration_smoke")
    if type(cluster) is not core.Cluster:
        raise ValueError("original live marker/inode-owned Cluster required")
    if owner not in {"concert", "auth"} or not re.fullmatch(r"mcr_[a-z0-9_]{1,59}", database):
        raise ValueError("isolated owner/database required")
    canonical_uuid(scope)
    cluster.assert_owned()
    cluster.assert_postmaster()
    schema_count = core.check_ledger(cluster, database, core.migration_manifest(ROOT / "backend", owner + "-service"))
    lineage = cluster.sql(database, "SELECT (pg_control_system()).system_identifier::text || ':' || (SELECT oid::text FROM pg_database WHERE datname=current_database());")
    result = {"kind": "READ_ONLY_OWNER_SNAPSHOT", "sourceRevision": revision(),
              "capturedAt": now(), "owner": owner, "runBinding": label(key, "run", "qa09"),
              "scopeBinding": label(key, owner, scope),
              "lineageBinding": label(key, "database", owner + ":" + database + ":" + lineage),
              "verifiedMigrationCount": schema_count, "tables": {}}
    for table, (table_owner, *_) in PROJECTIONS.items():
        if table_owner != owner:
            continue
        query = query_for(table, scope)
        item = {"state": "NOT_RUN", "querySha256": hashlib.sha256(query_for(table, "00000000-0000-0000-0000-000000000001").encode()).hexdigest()}
        try:
            # psql command tags must not be confused with the single JSON row.
            output = cluster.sql(database, query)
            lines = [line for line in output.splitlines() if line.startswith("[")]
            if len(lines) != 1:
                raise ValueError("single bounded projection required")
            item.update(state="IN_PROGRESS", rows=sanitize_rows(table, json.loads(lines[0]), key))
        except (core.SmokeError, ValueError, TypeError, KeyError):
            # No SQL diagnostics, connection credentials, IDs or payloads escape.
            item["reason"] = "OWNER_PROJECTION_UNAVAILABLE"
        result["tables"][table] = item
    cluster.assert_owned()
    cluster.assert_postmaster()
    return result


def compare(case_id, before, after):
    """Invariant observations only; even a clean comparison never certifies QA09."""
    if case_id not in CASES:
        raise ValueError("unknown original QA09 case")
    required = REQUIRED[case_id]
    for snapshot in (before, after):
        if snapshot.get("kind") != "READ_ONLY_OWNER_SNAPSHOT" or snapshot.get("owner") not in {"concert", "auth"}:
            raise ValueError("original observer snapshot required")
        for field, length in (("sourceRevision", 40), ("runBinding", 64), ("scopeBinding", 64), ("lineageBinding", 64)):
            if not re.fullmatch(r"[0-9a-f]{%d}" % length, snapshot.get(field, "")):
                raise ValueError("snapshot binding required")
    times = [datetime.fromisoformat(x["capturedAt"]) for x in (before, after)]
    if any(x.tzinfo is None for x in times) or times[1] < times[0]:
        raise ValueError("ordered timestamped observations required")
    if any(before.get(k) != after.get(k) for k in ("sourceRevision", "runBinding", "scopeBinding", "lineageBinding", "verifiedMigrationCount", "owner")):
        raise ValueError("candidate/run/scope mismatch")
    findings, compared = [], 0
    for table in required:
        old = before.get("tables", {}).get(table, {})
        new = after.get("tables", {}).get(table, {})
        if old.get("state") != "IN_PROGRESS" or new.get("state") != "IN_PROGRESS":
            continue
        if old.get("querySha256") != new.get("querySha256") or not re.fullmatch(r"[0-9a-f]{64}", old.get("querySha256", "")):
            raise ValueError("owner projection drift")
        a, b = old["rows"], new["rows"]
        if not a:  # Empty fixtures cannot prove persistence or replay.
            continue
        compared += 1
        for token, row in a.items():
            later = b.get(token)
            terminal = {"reservations": "CLAIMED", "outbox": "DELIVERED",
                        "deletion_steps": "SUCCEEDED", "closure_jobs": "COMPLETED"}.get(table)
            if (table == "closure_effects" or (terminal is not None and row.get("state") == terminal)) and later != row:
                findings.append("COMMITTED_STATE_CHANGED_OR_LOST")
            if later is not None and "revision" in row and later["revision"] < row["revision"]:
                findings.append("REVISION_REGRESSED")
        if table == "inventory":
            for row in b.values():
                if row["reserved_unclaimed"] + row["quantity_claimed"] > row["quantity_total"]:
                    findings.append("INVENTORY_OVERFLOW")
            for token, row in a.items():
                if row["quantity_claimed"] > 0 and (token not in b or b[token]["quantity_claimed"] < row["quantity_claimed"]):
                    findings.append("COMMITTED_PICKUP_COUNT_LOST")
    return {"caseId": case_id, "state": "IN_PROGRESS" if compared else "NOT_RUN",
            "wholeCaseAccepted": False, "comparedTables": compared,
            "violations": sorted(set(findings)), "limitations": [LIMITATIONS[case_id],
            "Read-only rows do not prove actual boundary execution, owner authority, downstream deduplication or client recovery."]}


def script_pins():
    return {str(path.relative_to(ROOT)): sha(path) for path in (
        Path(__file__), Path(__file__).with_name("test_qa09_backend.py"))}


def verify_checkpoint_scripts(checkpoint):
    if checkpoint.get("qa09_source_sha256") != script_pins():
        raise ValueError("QA09 orchestration source drift")


def restore_segment(checkpoint, pg_bin_dir, native_path, report):
    """Delegate to the existing real physical restore runner, without new fixtures."""
    if not checkpoint.is_absolute() or not pg_bin_dir.is_absolute():
        raise ValueError("explicit absolute checkpoint and PG16 tools required")
    verify_checkpoint_scripts(json.loads(checkpoint.read_text()))
    before = now()
    sys.path.insert(0, str(RESTORE.parent))
    spec = importlib.util.spec_from_file_location("qa09_existing_owner_restore", RESTORE)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    native = {"status": "FAIL", "scope": "Existing canonical-owner physical restore segment"}
    try:
        module.run(SimpleNamespace(checkpoint=checkpoint, pg_bin_dir=pg_bin_dir), native)
        native["status"] = "PASS"
    finally:
        write_report(native_path, native)
        # Deliberately no invented boundary timestamp or whole-case acceptance.
        case = next(x for x in report["cases"] if x["caseId"] == CASES[1])
        case["phases"] = {"before": before, "boundary": None, "recovered": now() if native["status"] == "PASS" else None}
        case["producerEvidence"] = {"path": native_path.name, "sha256": sha(native_path)}
        case["state"] = "IN_PROGRESS"
        case["observations"] = {"supportingPhysicalRestorePassed": native["status"] == "PASS"}
        case["limitations"].append("Existing runner does not expose a timestamped domain boundary; other original cases remain NOT_RUN.")


def write_report(path, report):
    if not path.is_absolute() or path.is_symlink() or path.exists():
        raise ValueError("new absolute private report required")
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "w") as stream:
        json.dump(report, stream, indent=2, sort_keys=True)
        stream.write("\n")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("plan", "restore-segment"))
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--run-id")
    parser.add_argument("--native-evidence", type=Path)
    parser.add_argument("--checkpoint", type=Path)
    parser.add_argument("--pg-bin-dir", type=Path)
    args = parser.parse_args(argv)
    if args.action == "restore-segment" and (args.checkpoint is None or args.pg_bin_dir is None or args.native_evidence is None):
        parser.error("restore segment requires frozen checkpoint, explicit PG16 tools and separate native evidence")
    if not args.report.is_absolute() or args.report.exists() or args.report.is_symlink():
        parser.error("new absolute report required")
    if args.native_evidence is not None and (not args.native_evidence.is_absolute() or args.native_evidence.parent != args.report.parent or args.native_evidence.exists() or args.native_evidence.is_symlink() or args.native_evidence == args.report):
        parser.error("separate new native evidence in the same private report directory required")
    report = plan(args.run_id)
    code = 0
    try:
        if args.action == "restore-segment":
            restore_segment(args.checkpoint, args.pg_bin_dir, args.native_evidence, report)
    except BaseException as error:
        # Class only: driver diagnostics may contain private owner material.
        report["failure"] = type(error).__name__
        code = 130 if isinstance(error, KeyboardInterrupt) else 1
    finally:
        write_report(args.report, report)
    print(json.dumps({"state": report["state"], "wholeQa09Accepted": False, "exitCode": code}))
    return code


if __name__ == "__main__":
    def interrupted(*_):
        # Unwind the existing runner's exact-owned cleanup before reporting failure.
        raise KeyboardInterrupt()
    signal.signal(signal.SIGTERM, interrupted)
    sys.exit(main())
